use crate::compositor::CompositorIntegration;
use crate::interaction::*;
use crate::popup::*;
use crate::runtime::socket::dispatch_shell_action;
use crate::runtime::tooltip::{handle_tooltip_pointer_move, HoverTooltipState};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::config::lua::LuaRuntime;
use wyrd_engine::input::pointer::CursorShape;
use wyrd_engine::input::InputEvent;
use wyrd_engine::modules::CoreMessage;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::{WidgetId, WidgetTree};
use wyrd_engine::BarState;

pub fn cursor_shape_for_hover(tree: &WidgetTree, widget: WidgetId) -> CursorShape {
    let mut current = Some(widget);
    while let Some(id) = current {
        let Some(node) = tree.get(id) else { break };
        match &node.content {
            wyrd_engine::widgets::WidgetContent::TextInput { .. } => return CursorShape::Text,
            wyrd_engine::widgets::WidgetContent::Slider { .. } => return CursorShape::Pointer,
            wyrd_engine::widgets::WidgetContent::Button { .. } => return CursorShape::Pointer,
            _ if node.on_click.is_some() => return CursorShape::Pointer,
            _ => current = node.parent,
        }
    }
    CursorShape::Default
}

#[allow(clippy::too_many_arguments)]
pub fn handle_key_input(
    keysym: u32,
    utf8: Option<String>,
    focused_surface_id: Option<ObjectId>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) {
    if keysym == xkbcommon::xkb::keysyms::KEY_Escape {
        let current_config = state
            .lock()
            .unwrap()
            .config
            .try_read()
            .map(|c| c.clone())
            .unwrap_or_default();
        close_unpinned_popups(backend, state, &current_config, open_popups, surface_trees);
        return;
    }
    let (is_ctrl, is_shift) = state
        .lock()
        .unwrap()
        .input_state
        .try_read()
        .map(|i| (i.keyboard.modifiers.ctrl, i.keyboard.modifiers.shift))
        .unwrap_or((false, false));

    let mut text_events = Vec::new();
    let mut activate_action = None;

    for (surface_id, tree) in surface_trees.iter_mut() {
        if focused_surface_id.as_ref() != Some(surface_id) {
            continue;
        }
        let mut_tree = Arc::make_mut(tree);
        let text_input_ids: Vec<WidgetId> = mut_tree
            .iter_nodes()
            .filter(|(_, n)| {
                matches!(
                    n.content,
                    wyrd_engine::widgets::WidgetContent::TextInput { .. }
                )
            })
            .map(|(id, _)| id)
            .collect();
        let mut text_input_found = false;
        for tid in text_input_ids {
            let mut target_mod = None;
            let mut cur = mut_tree.get(tid).and_then(|n| n.parent);
            while let Some(pid) = cur {
                if let Some(pn) = mut_tree.get(pid) {
                    if let wyrd_engine::widgets::WidgetContent::Module { ref module, .. } =
                        pn.content
                    {
                        target_mod = Some(module.clone());
                        break;
                    }
                    cur = pn.parent;
                } else {
                    break;
                }
            }
            let target_module = target_mod.unwrap_or_else(|| {
                backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| s.surface.id() == *surface_id)
                    .map(|s| s.config_name.clone())
                    .unwrap_or_else(|| "launcher".to_string())
            });

            let node = match mut_tree.get_mut(tid) {
                Some(n) => n,
                None => continue,
            };
            if let wyrd_engine::widgets::WidgetContent::TextInput {
                ref mut text,
                ref mut focused,
                ref mut cursor_pos,
                ref mut selection,
                ..
            } = node.content
            {
                *focused = true;
                text_input_found = true;
                let wid_opt = node.id.clone();

                if is_ctrl && (keysym == 0x0061 || keysym == 0x0041) {
                    // Ctrl + A (Select All)
                    *selection = Some((0, text.len()));
                    *cursor_pos = text.len();
                } else if is_ctrl && (keysym == 0x0063 || keysym == 0x0043) {
                    // Ctrl + C (Copy)
                    let to_copy = if let Some((s, e)) = *selection {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text[s..e].to_string()
                    } else {
                        text.clone()
                    };
                    if !to_copy.is_empty() {
                        std::thread::spawn(move || {
                            if let Ok(mut child) = std::process::Command::new("wl-copy")
                                .stdin(std::process::Stdio::piped())
                                .spawn()
                            {
                                use std::io::Write;
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(to_copy.as_bytes());
                                }
                                let _ = child.wait();
                            }
                        });
                    }
                } else if is_ctrl && (keysym == 0x0078 || keysym == 0x0058) {
                    // Ctrl + X (Cut)
                    let to_copy = if let Some((s, e)) = *selection {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text[s..e].to_string()
                    } else {
                        text.clone()
                    };
                    if !to_copy.is_empty() {
                        let copy_clone = to_copy.clone();
                        std::thread::spawn(move || {
                            if let Ok(mut child) = std::process::Command::new("wl-copy")
                                .stdin(std::process::Stdio::piped())
                                .spawn()
                            {
                                use std::io::Write;
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(copy_clone.as_bytes());
                                }
                                let _ = child.wait();
                            }
                        });
                    }
                    if let Some((s, e)) = selection.take() {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text.drain(s..e);
                        *cursor_pos = s;
                    } else {
                        text.clear();
                        *cursor_pos = 0;
                    }
                    if let Some(wid) = &wid_opt {
                        text_events.push((target_module.clone(), wid.clone(), text.clone()));
                    }
                } else if is_ctrl && (keysym == 0x0076 || keysym == 0x0056) {
                    // Ctrl + V (Paste)
                    let pasted = std::process::Command::new("wl-paste")
                        .arg("--no-newline")
                        .output()
                        .ok()
                        .and_then(|out| String::from_utf8(out.stdout).ok())
                        .unwrap_or_default();
                    let clean_pasted = pasted.replace(['\r', '\n'], " ");
                    if !clean_pasted.is_empty() {
                        if let Some((s, e)) = selection.take() {
                            let s = s.min(text.len());
                            let e = e.min(text.len());
                            text.drain(s..e);
                            *cursor_pos = s;
                        }
                        let pos = (*cursor_pos).min(text.len());
                        let safe_pos = if text.is_char_boundary(pos) {
                            pos
                        } else {
                            text.len()
                        };
                        text.insert_str(safe_pos, &clean_pasted);
                        *cursor_pos = safe_pos + clean_pasted.len();
                        if let Some(wid) = &wid_opt {
                            text_events.push((target_module.clone(), wid.clone(), text.clone()));
                        }
                    }
                } else if is_ctrl
                    && (keysym == 0x0077
                        || keysym == 0x0057
                        || keysym == xkbcommon::xkb::keysyms::KEY_BackSpace)
                {
                    // Ctrl + W / Ctrl + Backspace (Delete Word Backwards)
                    if let Some((s, e)) = selection.take() {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text.drain(s..e);
                        *cursor_pos = s;
                    } else {
                        let pos = (*cursor_pos).min(text.len());
                        let before = &text[..pos];
                        let trimmed = before.trim_end();
                        let word_start = trimmed
                            .rfind(|c: char| c.is_whitespace() || c == '_' || c == '-')
                            .map(|idx| idx + 1)
                            .unwrap_or(0);
                        text.drain(word_start..pos);
                        *cursor_pos = word_start;
                    }
                    if let Some(wid) = &wid_opt {
                        text_events.push((target_module.clone(), wid.clone(), text.clone()));
                    }
                } else if is_ctrl && (keysym == 0x0075 || keysym == 0x0055) {
                    // Ctrl + U (Clear Line)
                    text.clear();
                    *cursor_pos = 0;
                    *selection = None;
                    if let Some(wid) = &wid_opt {
                        text_events.push((target_module.clone(), wid.clone(), text.clone()));
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_BackSpace {
                    if let Some((s, e)) = selection.take() {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text.drain(s..e);
                        *cursor_pos = s;
                        if let Some(wid) = &wid_opt {
                            text_events.push((target_module.clone(), wid.clone(), text.clone()));
                        }
                    } else if *cursor_pos > 0 {
                        let pos = (*cursor_pos).min(text.len());
                        let prev_boundary = text[..pos]
                            .char_indices()
                            .last()
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);
                        text.drain(prev_boundary..pos);
                        *cursor_pos = prev_boundary;
                        if let Some(wid) = &wid_opt {
                            text_events.push((target_module.clone(), wid.clone(), text.clone()));
                        }
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Delete {
                    if let Some((s, e)) = selection.take() {
                        let s = s.min(text.len());
                        let e = e.min(text.len());
                        text.drain(s..e);
                        *cursor_pos = s;
                        if let Some(wid) = &wid_opt {
                            text_events.push((target_module.clone(), wid.clone(), text.clone()));
                        }
                    } else if *cursor_pos < text.len() {
                        let pos = (*cursor_pos).min(text.len());
                        if let Some((_, ch)) = text[pos..].char_indices().next() {
                            let next_boundary = pos + ch.len_utf8();
                            text.drain(pos..next_boundary);
                            if let Some(wid) = &wid_opt {
                                text_events.push((
                                    target_module.clone(),
                                    wid.clone(),
                                    text.clone(),
                                ));
                            }
                        }
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Left {
                    *selection = None;
                    if *cursor_pos > 0 {
                        let pos = (*cursor_pos).min(text.len());
                        *cursor_pos = text[..pos]
                            .char_indices()
                            .last()
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Right {
                    *selection = None;
                    if *cursor_pos < text.len() {
                        let pos = (*cursor_pos).min(text.len());
                        if let Some((_, ch)) = text[pos..].char_indices().next() {
                            *cursor_pos = pos + ch.len_utf8();
                        }
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Home {
                    *selection = None;
                    *cursor_pos = 0;
                } else if keysym == xkbcommon::xkb::keysyms::KEY_End {
                    *selection = None;
                    *cursor_pos = text.len();
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Return
                    || keysym == xkbcommon::xkb::keysyms::KEY_KP_Enter
                {
                    if let Some(wid) = &wid_opt {
                        text_events.push((
                            target_module.clone(),
                            wid.clone(),
                            "submit".to_string(),
                        ));
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Down
                    || (!is_shift && keysym == xkbcommon::xkb::keysyms::KEY_Tab)
                {
                    if let Some(wid) = &wid_opt {
                        text_events.push((target_module.clone(), wid.clone(), "down".to_string()));
                    }
                } else if keysym == xkbcommon::xkb::keysyms::KEY_Up
                    || (is_shift && keysym == xkbcommon::xkb::keysyms::KEY_Tab)
                {
                    if let Some(wid) = &wid_opt {
                        text_events.push((target_module.clone(), wid.clone(), "up".to_string()));
                    }
                } else if !is_ctrl {
                    if let Some(ch) = &utf8 {
                        if !ch.chars().any(|c| c.is_control()) {
                            if let Some((s, e)) = selection.take() {
                                let s = s.min(text.len());
                                let e = e.min(text.len());
                                text.drain(s..e);
                                *cursor_pos = s;
                            }
                            let pos = (*cursor_pos).min(text.len());
                            let safe_pos = if text.is_char_boundary(pos) {
                                pos
                            } else {
                                text.len()
                            };
                            text.insert_str(safe_pos, ch);
                            *cursor_pos = safe_pos + ch.len();
                            if let Some(wid) = &wid_opt {
                                text_events.push((
                                    target_module.clone(),
                                    wid.clone(),
                                    text.clone(),
                                ));
                            }
                        }
                    }
                }
                node.mark_dirty();
            }
        }
        if text_input_found {
            for surface in &mut backend.surface_manager.surfaces {
                if surface.surface.id() == *surface_id {
                    surface.dirty = true;
                }
            }
            state.lock().unwrap().frame_ready = true;
        } else {
            let navigable: Vec<WidgetId> = mut_tree
                .iter_nodes()
                .filter(|(_, n)| {
                    matches!(
                        n.content,
                        wyrd_engine::widgets::WidgetContent::Button { .. }
                    ) || n.on_click.is_some()
                })
                .map(|(id, _)| id)
                .collect();
            if !navigable.is_empty() {
                let cur_focus = state
                    .lock()
                    .unwrap()
                    .input_state
                    .try_read()
                    .ok()
                    .and_then(|i| i.focus.current_focus());
                let cur_idx = cur_focus.and_then(|cf| navigable.iter().position(|&w| w == cf));
                let is_next = keysym == xkbcommon::xkb::keysyms::KEY_Tab
                    || keysym == xkbcommon::xkb::keysyms::KEY_Down
                    || keysym == xkbcommon::xkb::keysyms::KEY_Right;
                let is_prev = keysym == 0xfe20
                    || keysym == xkbcommon::xkb::keysyms::KEY_Up
                    || keysym == xkbcommon::xkb::keysyms::KEY_Left;
                let is_act = keysym == xkbcommon::xkb::keysyms::KEY_Return
                    || keysym == xkbcommon::xkb::keysyms::KEY_KP_Enter
                    || keysym == xkbcommon::xkb::keysyms::KEY_space;

                if is_next {
                    let next_idx = cur_idx.map_or(0, |i| (i + 1) % navigable.len());
                    if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
                        inp.focus.set_focus(navigable[next_idx]);
                    }
                    for surface in &mut backend.surface_manager.surfaces {
                        if surface.surface.id() == *surface_id {
                            surface.dirty = true;
                        }
                    }
                    state.lock().unwrap().frame_ready = true;
                } else if is_prev {
                    let prev_idx = cur_idx.map_or(navigable.len().saturating_sub(1), |i| {
                        if i == 0 {
                            navigable.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    });
                    if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
                        inp.focus.set_focus(navigable[prev_idx]);
                    }
                    for surface in &mut backend.surface_manager.surfaces {
                        if surface.surface.id() == *surface_id {
                            surface.dirty = true;
                        }
                    }
                    state.lock().unwrap().frame_ready = true;
                } else if is_act {
                    if let Some(f_wid) = cur_focus {
                        if let Some(node) = mut_tree.get(f_wid) {
                            if let Some(ref cmd) = node.on_click {
                                activate_action = Some(cmd.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    for (target_mod, wid, val) in text_events {
        dispatch_custom_action(
            &format!("event:{}:{}:{}", target_mod, wid, val),
            0.0,
            0.0,
            module_commands,
        );
    }
    if let Some(cmd) = activate_action {
        dispatch_custom_action(&cmd, 0.0, 0.0, module_commands);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process_input_events(
    input_events: Vec<InputEvent>,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    lua: &LuaRuntime,
    compositor: &dyn CompositorIntegration,
    dragging_popup: &mut Option<DragState>,
    hover_tooltip: &mut Option<HoverTooltipState>,
    active_slider: &mut Option<SliderDragState>,
    recent_slider_release: &mut Option<RecentSliderRelease>,
) {
    for input_event in input_events {
        match input_event {
            InputEvent::PointerMove { x, y } => {
                if let Some(ref mut active) = active_slider {
                    handle_slider_pointer_move(
                        active,
                        x,
                        surface_trees,
                        backend,
                        state,
                        module_commands,
                    );
                } else if let Some(ref mut drag) = dragging_popup {
                    handle_popup_drag(
                        drag,
                        compositor,
                        backend,
                        state,
                        open_popups,
                        surface_trees,
                        render_ctx,
                        module_commands,
                    )
                    .await;
                }
                let mut found_hover = None;
                if let Some(surf_id) = &state.lock().unwrap().pointer_surface_id {
                    if let Some(tree) = surface_trees.get(surf_id) {
                        found_hover = tree.hit_test(x, y);
                    }
                }
                let hover_changed =
                    if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
                        let changed = inp.pointer.hovered_widget != found_hover;
                        inp.pointer.hovered_widget = found_hover;
                        inp.pointer.x = x;
                        inp.pointer.y = y;
                        changed
                    } else {
                        false
                    };
                if hover_changed {
                    if let Some(surf_id) = &state.lock().unwrap().pointer_surface_id {
                        if let Some(surf) = backend
                            .surface_manager
                            .surfaces
                            .iter_mut()
                            .find(|s| &s.surface.id() == surf_id)
                        {
                            surf.dirty = true;
                        }
                    }
                    state.lock().unwrap().frame_ready = true;
                    let serial = state.lock().unwrap().pointer_enter_serial;
                    let shape = state
                        .lock()
                        .unwrap()
                        .pointer_surface_id
                        .as_ref()
                        .and_then(|surface_id| surface_trees.get(surface_id))
                        .and_then(|tree| {
                            found_hover.map(|widget| cursor_shape_for_hover(tree, widget))
                        })
                        .unwrap_or(CursorShape::Default);
                    backend.set_cursor_shape(serial, shape);
                }

                handle_tooltip_pointer_move(
                    backend,
                    state,
                    surface_trees,
                    open_popups,
                    hover_tooltip,
                    found_hover,
                );

                if hover_changed || active_slider.is_some() || dragging_popup.is_some() {
                    for surface in &mut backend.surface_manager.surfaces {
                        if let Some(surf_id) = &state.lock().unwrap().pointer_surface_id {
                            if &surface.surface.id() == surf_id {
                                surface.dirty = true;
                            }
                        } else {
                            surface.dirty = true;
                        }
                    }
                    state.lock().unwrap().frame_ready = true;
                }
            }
            InputEvent::PointerButton {
                surface_id,
                widget: _,
                x,
                y,
                button,
                pressed,
            } => {
                if !pressed {
                    if let Some(active) = active_slider.take() {
                        handle_slider_pointer_release(
                            active,
                            surface_trees,
                            recent_slider_release,
                            module_commands,
                        );
                    }
                    *dragging_popup = None;
                    continue;
                }
                if let Some(ht) = hover_tooltip.take() {
                    if ht.is_shown {
                        close_popup(backend, state, open_popups, surface_trees, "__tooltip__");
                    }
                }
                if button != 0x110 && button != 0x111 {
                    continue;
                }

                if handle_popup_click_outside(
                    backend,
                    state,
                    open_popups,
                    surface_trees,
                    &surface_id,
                )
                .await
                {
                    *dragging_popup = None;
                    *active_slider = None;
                    *recent_slider_release = None;
                    continue;
                }

                let hit_widget = surface_trees
                    .get(&surface_id)
                    .and_then(|tree| tree.hit_test(x, y));
                let resolved_widget = hit_widget.unwrap_or_default();

                if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
                    inp.focused_surface_id = Some(surface_id.clone());
                    inp.focus.set_focus(resolved_widget);
                }
                if let Some(tree) = surface_trees.get_mut(&surface_id) {
                    let mut_tree = Arc::make_mut(tree);
                    for (nid, node) in mut_tree.iter_nodes_mut() {
                        if let wyrd_engine::widgets::WidgetContent::TextInput {
                            ref mut focused,
                            ..
                        } = node.content
                        {
                            *focused = nid == resolved_widget;
                        }
                    }
                }
                for surface in &mut backend.surface_manager.surfaces {
                    if surface.surface.id() == surface_id {
                        surface.dirty = true;
                    }
                }
                state.lock().unwrap().frame_ready = true;

                if button == 0x110
                    && handle_slider_pointer_press(
                        surface_id.clone(),
                        resolved_widget,
                        x,
                        surface_trees,
                        active_slider,
                        backend,
                        state,
                        module_commands,
                    )
                {
                    *dragging_popup = None;
                    continue;
                }

                let is_interactive = surface_trees
                    .get(&surface_id)
                    .map(|tree| {
                        let mut cur = Some(resolved_widget);
                        while let Some(w) = cur {
                            if let Some(n) = tree.get(w) {
                                if matches!(
                                    n.content,
                                    wyrd_engine::widgets::WidgetContent::Button { .. }
                                        | wyrd_engine::widgets::WidgetContent::Slider { .. }
                                        | wyrd_engine::widgets::WidgetContent::TextInput { .. }
                                ) || n.on_click.is_some()
                                    || n.on_right_click.is_some()
                                    || n.on_change.is_some()
                                {
                                    return true;
                                }
                                cur = n.parent;
                            } else {
                                break;
                            }
                        }
                        false
                    })
                    .unwrap_or(false);

                if button == 0x110 && !is_interactive {
                    if let Some(surf) = backend.surface_manager.surfaces.iter().find(|s| {
                        s.surface.id() == surface_id
                            && s.ty == wyrd_engine::wayland::surface::SurfaceType::Popup
                            && s.config_name != "__popup_backdrop__"
                    }) {
                        let header_start = surf.shadow_insets.2 as f64;
                        let current_config = state
                            .lock()
                            .unwrap()
                            .config
                            .try_read()
                            .map(|c| c.clone())
                            .unwrap_or_default();
                        let strip_height = current_config
                            .surfaces
                            .iter()
                            .find(|sc| sc.name == surf.config_name)
                            .and_then(|sc| sc.drag_strip_height)
                            .map(|h| h as f64)
                            .unwrap_or(28.0)
                            .max(16.0);
                        let content_left = surf.shadow_insets.0 as f64;
                        let content_right =
                            (surf.width as f64 - surf.shadow_insets.1 as f64).max(content_left);

                        if x >= content_left
                            && x <= content_right
                            && y >= header_start
                            && y <= (header_start + strip_height)
                        {
                            let output_id = surf
                                .output
                                .as_ref()
                                .map(Proxy::id)
                                .unwrap_or_else(|| surface_id.clone());
                            let (gx, gy) =
                                compositor.cursor_position().unwrap_or((x as i32, y as i32));
                            *dragging_popup = Some(DragState {
                                surface_id: surface_id.clone(),
                                popup_name: surf.config_name.clone(),
                                start_global_x: gx,
                                start_global_y: gy,
                                start_margin: surf.margin,
                                output_id,
                            });
                        }
                    }
                }

                let mut action_opt = None;
                let mut trigger_rect = None;
                if let Some(tree) = surface_trees.get(&surface_id) {
                    let mut cur = Some(resolved_widget);
                    while let Some(wid) = cur {
                        if let Some(node) = tree.get(wid) {
                            if button == 0x111 && node.on_right_click.is_some() {
                                action_opt = node.on_right_click.clone();
                                trigger_rect = Some(node.final_rect);
                                break;
                            } else if button == 0x110 && node.on_click.is_some() {
                                action_opt = node.on_click.clone();
                                trigger_rect = Some(node.final_rect);
                                break;
                            }
                            cur = node.parent;
                        } else {
                            break;
                        }
                    }
                }

                if let Some(action_str) = action_opt {
                    dispatch_shell_action(
                        &action_str,
                        surface_id,
                        0.0,
                        0.0,
                        trigger_rect,
                        backend,
                        state,
                        open_popups,
                        surface_trees,
                        render_ctx,
                        module_commands,
                        lua,
                        compositor,
                    )
                    .await;
                }
            }
            InputEvent::PointerScroll {
                surface_id,
                widget,
                axis_x,
                axis_y,
            } => {
                let mut custom_action = None;
                let mut scroll_handled = false;

                if let Some(tree) = surface_trees.get_mut(&surface_id) {
                    let mut_tree = Arc::make_mut(tree);
                    let mut cur = Some(widget);
                    while let Some(wid) = cur {
                        let is_scrollable = mut_tree
                            .get(wid)
                            .map(|n| n.layout.scroll_y)
                            .unwrap_or(false);
                        if is_scrollable {
                            let (gap, pad_top, pad_bot, container_h, old_offset) = {
                                let Some(node) = mut_tree.get(wid) else {
                                    break;
                                };
                                (
                                    node.layout.gap,
                                    node.layout.padding.0,
                                    node.layout.padding.2,
                                    node.final_rect.3,
                                    node.layout.scroll_offset_y,
                                )
                            };
                            let children = mut_tree
                                .get(wid)
                                .map(|n| n.children.clone())
                                .unwrap_or_default();
                            let total_children_h: f32 = children
                                .iter()
                                .filter_map(|cid| mut_tree.get(*cid))
                                .map(|c| {
                                    c.layout
                                        .fixed_height
                                        .unwrap_or(c.measured_size.1.max(c.final_rect.3))
                                })
                                .sum::<f32>()
                                + (gap * children.len().saturating_sub(1) as f32)
                                + pad_top
                                + pad_bot;
                            let max_scroll = (total_children_h - container_h).max(0.0);
                            let delta = axis_y as f32 * 2.5;
                            let new_offset = (old_offset + delta).clamp(0.0, max_scroll);
                            if (new_offset - old_offset).abs() > 0.01 {
                                if let Some(node) = mut_tree.get_mut(wid) {
                                    node.layout.scroll_offset_y = new_offset;
                                    node.mark_dirty();
                                    scroll_handled = true;
                                }
                            }
                            break;
                        }

                        if let Some(node) = mut_tree.get(wid) {
                            if let Some(ref cmd) = node.on_scroll {
                                custom_action = Some((cmd.clone(), axis_x, axis_y));
                                break;
                            }
                            cur = node.parent;
                        } else {
                            break;
                        }
                    }
                }

                if scroll_handled {
                    for surface in &mut backend.surface_manager.surfaces {
                        if surface.surface.id() == surface_id {
                            surface.dirty = true;
                        }
                    }
                    state.lock().unwrap().frame_ready = true;
                }

                if let Some((cmd, ax, ay)) = custom_action {
                    dispatch_custom_action(&cmd, ax, ay, module_commands);
                }
            }
            InputEvent::PointerLeave { .. } => {
                if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
                    inp.pointer.hovered_widget = None;
                }
                if let Some(ht) = hover_tooltip.take() {
                    if ht.is_shown {
                        close_popup(backend, state, open_popups, surface_trees, "__tooltip__");
                    }
                }
                for surface in &mut backend.surface_manager.surfaces {
                    surface.dirty = true;
                }
                state.lock().unwrap().frame_ready = true;
            }
            InputEvent::KeyPress { keysym, utf8 } => {
                let focused_surf = state
                    .lock()
                    .unwrap()
                    .input_state
                    .try_read()
                    .ok()
                    .and_then(|i| i.focused_surface_id.clone());
                handle_key_input(
                    keysym,
                    utf8,
                    focused_surf,
                    surface_trees,
                    backend,
                    state,
                    open_popups,
                    module_commands,
                );
            }
            InputEvent::PointerEnter { .. } => {}
            InputEvent::KeyRelease { .. } => {}
        }
    }
}
