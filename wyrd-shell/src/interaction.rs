use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::modules::CoreMessage;
use wyrd_engine::widgets::{WidgetContent, WidgetId, WidgetTree};

pub struct DragState {
    pub surface_id: ObjectId,
    pub popup_name: String,
    pub start_global_x: i32,
    pub start_global_y: i32,
    pub start_margin: (i32, i32, i32, i32),
    pub output_id: ObjectId,
}

#[derive(Debug, Clone)]
pub struct SliderDragState {
    pub surface_id: ObjectId,
    pub widget: WidgetId,
    pub id: Option<String>,
    pub current_value: f32,
    pub last_dispatched_int: Option<i32>,
    pub last_dispatched_time: Instant,
    pub on_change: Option<String>,
    pub node_id: Option<String>,
    pub parent: Option<WidgetId>,
}

#[derive(Debug, Clone)]
pub struct RecentSliderRelease {
    pub surface_id: ObjectId,
    pub widget_id: Option<String>,
    pub committed_value: f32,
    pub released_at: Instant,
}

pub fn spawn_action(action: &str, axis_x: f64, axis_y: f64) -> anyhow::Result<()> {
    let raw = action.trim();
    if raw.is_empty() {
        anyhow::bail!("empty action command");
    }

    let parsed_words = shell_words::split(raw)
        .map_err(|e| anyhow::anyhow!("failed to parse command words: {}", e))?;
    if parsed_words.is_empty() {
        anyhow::bail!("empty action command words");
    }

    let is_simple_exec = parsed_words.iter().all(|w| {
        !w.contains('|')
            && !w.contains(';')
            && !w.contains('&')
            && !w.contains('>')
            && !w.contains('<')
            && !w.contains('$')
            && !w.contains('`')
    });

    if is_simple_exec {
        if let Some((prog, args)) = parsed_words.split_first() {
            let mut cmd = tokio::process::Command::new(prog);
            cmd.args(args);
            cmd.env("WYRD_SCROLL_X", axis_x.to_string());
            cmd.env("WYRD_SCROLL_Y", axis_y.to_string());
            let _ = cmd.spawn()?;
            return Ok(());
        }
    }

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(raw);
    cmd.env("WYRD_SCROLL_X", axis_x.to_string());
    cmd.env("WYRD_SCROLL_Y", axis_y.to_string());
    let _ = cmd.spawn()?;
    Ok(())
}

pub fn send_module_event(sender: &tokio::sync::mpsc::Sender<CoreMessage>, msg: CoreMessage) {
    if let Err(tokio::sync::mpsc::error::TrySendError::Full(msg)) = sender.try_send(msg) {
        let s = sender.clone();
        tokio::spawn(async move {
            let _ = s.send(msg).await;
        });
    }
}

pub fn parse_module_event(payload: &str) -> Option<(&str, String, String)> {
    let (target_mod, rest) = payload.split_once(':')?;
    let (widget_id, event_name) = match rest.rsplit_once(':') {
        Some((w, e))
            if e == "click"
                || e == "right_click"
                || e == "middle_click"
                || e == "context"
                || e.parse::<f64>().is_ok() =>
        {
            (w.to_string(), e.to_string())
        }
        _ => {
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            (
                parts.first().copied().unwrap_or(rest).to_string(),
                parts.get(1).copied().unwrap_or("click").to_string(),
            )
        }
    };
    Some((target_mod, widget_id, event_name))
}

pub fn sync_slider_state(
    tree: &mut WidgetTree,
    surface_id: &ObjectId,
    active_slider: &mut Option<SliderDragState>,
    recent_release: &mut Option<RecentSliderRelease>,
) {
    if let Some(ref mut active) = active_slider {
        if &active.surface_id == surface_id {
            let target_id = if let Some(ref sid) = active.id {
                tree.iter_nodes()
                    .find(|(_, n)| {
                        n.id.as_ref() == Some(sid)
                            && matches!(n.content, WidgetContent::Slider { .. })
                    })
                    .map(|(id, _)| id)
            } else if tree.get(active.widget).is_some() {
                Some(active.widget)
            } else {
                tree.iter_nodes()
                    .find(|(_, n)| matches!(n.content, WidgetContent::Slider { .. }))
                    .map(|(id, _)| id)
            };
            if let Some(tid) = target_id {
                active.widget = tid;
                if let Some(n) = tree.get_mut(tid) {
                    if let WidgetContent::Slider { ref mut value, .. } = n.content {
                        *value = active.current_value;
                        n.mark_dirty();
                    }
                }
            }
            return;
        }
    }

    if let Some(ref rel) = recent_release {
        if &rel.surface_id == surface_id
            && rel.released_at.elapsed() < std::time::Duration::from_millis(600)
        {
            let target_id = if let Some(ref sid) = rel.widget_id {
                tree.iter_nodes()
                    .find(|(_, n)| {
                        n.id.as_ref() == Some(sid)
                            && matches!(n.content, WidgetContent::Slider { .. })
                    })
                    .map(|(id, _)| id)
            } else {
                tree.iter_nodes()
                    .find(|(_, n)| matches!(n.content, WidgetContent::Slider { .. }))
                    .map(|(id, _)| id)
            };
            if let Some(tid) = target_id {
                if let Some(n) = tree.get_mut(tid) {
                    if let WidgetContent::Slider { ref mut value, .. } = n.content {
                        if (value.round() as i32) != (rel.committed_value.round() as i32) {
                            *value = rel.committed_value;
                            n.mark_dirty();
                        }
                    }
                }
            }
        } else if recent_release
            .as_ref()
            .map(|r| r.released_at.elapsed() >= std::time::Duration::from_millis(600))
            .unwrap_or(false)
        {
            *recent_release = None;
        }
    }
}

pub fn dispatch_custom_action(
    cmd: &str,
    axis_x: f64,
    axis_y: f64,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) {
    if cmd.starts_with("event:") {
        let payload = cmd.trim_start_matches("event:");
        if let Some((target_mod, widget_id, event_name)) = parse_module_event(payload) {
            if let Some(sender) = module_commands.get(target_mod).or_else(|| {
                module_commands
                    .iter()
                    .find(|(k, _)| k.replace('-', "_") == target_mod.replace('-', "_"))
                    .map(|(_, v)| v)
            }) {
                send_module_event(
                    sender,
                    CoreMessage::Event {
                        widget_id,
                        event: event_name,
                    },
                );
            }
        }
    } else {
        let _ = spawn_action(cmd, axis_x, axis_y);
    }
}

pub fn resolve_slider_action(
    on_change: Option<&str>,
    node_id: Option<&str>,
    parent: Option<WidgetId>,
    tree: &WidgetTree,
    val: f32,
) -> Option<String> {
    if let Some(template) = on_change {
        return Some(template.replace("{value}", &format!("{:.0}", val)));
    }
    let mut cur = parent;
    while let Some(pid) = cur {
        if let Some(pn) = tree.get(pid) {
            if let WidgetContent::Module { ref module, .. } = pn.content {
                let wid = node_id.unwrap_or("slider");
                return Some(format!("event:{}:{}:{:.0}", module, wid, val));
            }
            cur = pn.parent;
        } else {
            break;
        }
    }
    if let Some(wid) = node_id {
        if wid.contains("volume") || wid.contains("audio") {
            return Some(format!("event:audio:volume_slider:{:.0}", val));
        } else if wid.contains("brightness") {
            return Some(format!("event:brightness:slider_brightness:{:.0}", val));
        } else if wid.contains("mic") {
            return Some(format!("event:audio:microphone_slider:{:.0}", val));
        }
    }
    None
}

pub fn handle_slider_pointer_move(
    active: &mut SliderDragState,
    x: f64,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    backend: &mut wyrd_engine::wayland::backend::WaylandBackend,
    state: &Arc<std::sync::Mutex<wyrd_engine::BarState>>,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) {
    let mut slider_action = None;
    if let Some(tree) = surface_trees.get_mut(&active.surface_id) {
        let mut_tree = Arc::make_mut(tree);
        let target_id = if mut_tree.get(active.widget).is_some() {
            active.widget
        } else if let Some(ref sid) = active.id {
            mut_tree
                .iter_nodes()
                .find(|(_, n)| {
                    n.id.as_ref() == Some(sid) && matches!(n.content, WidgetContent::Slider { .. })
                })
                .map(|(id, _)| id)
                .unwrap_or(active.widget)
        } else {
            active.widget
        };
        active.widget = target_id;

        let mut changed_info = None;
        let mut slider_update = None;
        if let Some(node) = mut_tree.get_mut(target_id) {
            if let WidgetContent::Slider {
                ref mut value,
                min,
                max,
                ..
            } = node.content
            {
                let (rx, _ry, rw, _rh) = node.final_rect;
                let ratio = ((x as f32 - rx) / rw.max(1.0)).clamp(0.0, 1.0);
                let new_val = min + ratio * (max - min);
                let new_int = new_val.round() as i32;
                *value = new_val;
                active.current_value = new_val;
                node.mark_dirty();

                slider_update = Some((
                    new_val,
                    new_int,
                    node.on_change.clone(),
                    node.id.clone(),
                    node.parent,
                ));
            }
        }

        if let Some((new_val, new_int, on_change, node_id, node_parent)) = slider_update {
            // Live-update sibling label (e.g. "50%")
            if let Some(pid) = node_parent {
                let sib_ids = mut_tree
                    .get(pid)
                    .map(|p| p.children.clone())
                    .unwrap_or_default();
                for sib_id in sib_ids {
                    if sib_id == target_id {
                        continue;
                    }
                    if let Some(sib_node) = mut_tree.get_mut(sib_id) {
                        if let WidgetContent::Text { ref mut text } = sib_node.content {
                            if text.ends_with('%') || text.parse::<f64>().is_ok() {
                                *text = format!("{}%", new_int);
                                sib_node.mark_dirty();
                            }
                        }
                    }
                }
            }

            let should_dispatch = match active.last_dispatched_int {
                Some(last) => {
                    last != new_int
                        && active.last_dispatched_time.elapsed()
                            >= std::time::Duration::from_millis(30)
                }
                None => true,
            };

            if should_dispatch {
                active.last_dispatched_int = Some(new_int);
                active.last_dispatched_time = std::time::Instant::now();
                active.on_change = on_change.clone();
                active.node_id = node_id.clone();
                active.parent = node_parent;
                changed_info = Some((new_val, on_change, node_id, node_parent));

                // Immediately synchronize module_data store
                if let Ok(mut store) = state.lock().unwrap().module_data.try_write() {
                    let is_audio = active
                        .node_id
                        .as_deref()
                        .is_some_and(|id| id.contains("volume") || id.contains("audio"))
                        || active
                            .on_change
                            .as_deref()
                            .is_some_and(|c| c.contains("audio"));
                    let is_brightness = active
                        .node_id
                        .as_deref()
                        .is_some_and(|id| id.contains("bright"))
                        || active
                            .on_change
                            .as_deref()
                            .is_some_and(|c| c.contains("bright"));
                    if is_audio {
                        if let Some(audio_val) = store.get_mut("audio") {
                            if let Some(obj) = audio_val.as_object_mut() {
                                obj.insert("volume".to_string(), serde_json::json!(new_int));
                            }
                        }
                    } else if is_brightness {
                        let entry = store
                            .entry("brightness".to_string())
                            .or_insert_with(|| serde_json::json!({}));
                        if let Some(obj) = entry.as_object_mut() {
                            obj.insert("percent".to_string(), serde_json::json!(new_int));
                            obj.insert("value".to_string(), serde_json::json!(new_int));
                        }
                    }
                }
            }
        }
        for surface in &mut backend.surface_manager.surfaces {
            if surface.surface.id() == active.surface_id
                || surface.config_name == "__monitor_dimmer__"
            {
                surface.dirty = true;
            }
        }
        state.lock().unwrap().frame_ready = true;
        if let Some((new_val, on_change, node_id, parent)) = changed_info {
            slider_action = resolve_slider_action(
                on_change.as_deref(),
                node_id.as_deref(),
                parent,
                mut_tree,
                new_val,
            );
        }
    }
    if let Some(cmd) = slider_action {
        dispatch_custom_action(&cmd, 0.0, 0.0, module_commands);
    }
}

pub fn handle_slider_pointer_release(
    active: SliderDragState,
    surface_trees: &HashMap<ObjectId, Arc<WidgetTree>>,
    recent_slider_release: &mut Option<RecentSliderRelease>,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) {
    let final_int = active.current_value.round() as i32;
    if active.last_dispatched_int != Some(final_int) {
        let mut final_action = None;
        if let Some(tree) = surface_trees.get(&active.surface_id) {
            final_action = resolve_slider_action(
                active.on_change.as_deref(),
                active.node_id.as_deref(),
                active.parent,
                tree,
                active.current_value,
            );
        }
        if let Some(cmd) = final_action {
            dispatch_custom_action(&cmd, 0.0, 0.0, module_commands);
        }
    }
    *recent_slider_release = Some(RecentSliderRelease {
        surface_id: active.surface_id,
        widget_id: active.id.clone(),
        committed_value: active.current_value,
        released_at: std::time::Instant::now(),
    });
}

#[allow(clippy::too_many_arguments)]
pub fn handle_slider_pointer_press(
    surface_id: ObjectId,
    resolved_widget: WidgetId,
    x: f64,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    active_slider: &mut Option<SliderDragState>,
    backend: &mut wyrd_engine::wayland::backend::WaylandBackend,
    state: &Arc<std::sync::Mutex<wyrd_engine::BarState>>,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) -> bool {
    let mut slider_action = None;
    if let Some(tree) = surface_trees.get_mut(&surface_id) {
        let target_widget = {
            let mut cur = Some(resolved_widget);
            let mut found = None;
            while let Some(w) = cur {
                if let Some(n) = tree.get(w) {
                    if matches!(n.content, WidgetContent::Slider { .. }) {
                        found = Some(w);
                        break;
                    }
                    cur = n.parent;
                } else {
                    break;
                }
            }
            found
        };

        if let Some(sw) = target_widget {
            let mut_tree = Arc::make_mut(tree);
            let mut click_update = None;
            if let Some(node) = mut_tree.get_mut(sw) {
                if let WidgetContent::Slider {
                    ref mut value,
                    min,
                    max,
                    ..
                } = node.content
                {
                    let (rx, _ry, rw, _rh) = node.final_rect;
                    let ratio = ((x as f32 - rx) / rw.max(1.0)).clamp(0.0, 1.0);
                    let new_val = min + ratio * (max - min);
                    let new_int = new_val.round() as i32;
                    *value = new_val;
                    node.mark_dirty();
                    let node_id = node.id.clone();
                    let node_parent = node.parent;
                    let on_change = node.on_change.clone();
                    click_update = Some((new_val, new_int, node_id, node_parent, on_change));
                }
            }

            if let Some((new_val, new_int, node_id, node_parent, on_change)) = click_update {
                // Live-update sibling label (e.g. "50%")
                if let Some(pid) = node_parent {
                    let sib_ids = mut_tree
                        .get(pid)
                        .map(|p| p.children.clone())
                        .unwrap_or_default();
                    for sib_id in sib_ids {
                        if sib_id == sw {
                            continue;
                        }
                        if let Some(sib_node) = mut_tree.get_mut(sib_id) {
                            if let WidgetContent::Text { ref mut text } = sib_node.content {
                                if text.ends_with('%') || text.parse::<f64>().is_ok() {
                                    *text = format!("{}%", new_int);
                                    sib_node.mark_dirty();
                                }
                            }
                        }
                    }
                }

                // Immediately synchronize module_data store
                if let Ok(mut store) = state.lock().unwrap().module_data.try_write() {
                    let is_audio = node_id
                        .as_deref()
                        .is_some_and(|id| id.contains("volume") || id.contains("audio"))
                        || on_change.as_deref().is_some_and(|c| c.contains("audio"));
                    let is_brightness = node_id.as_deref().is_some_and(|id| id.contains("bright"))
                        || on_change.as_deref().is_some_and(|c| c.contains("bright"));
                    if is_audio {
                        if let Some(audio_val) = store.get_mut("audio") {
                            if let Some(obj) = audio_val.as_object_mut() {
                                obj.insert("volume".to_string(), serde_json::json!(new_int));
                            }
                        }
                    } else if is_brightness {
                        let entry = store
                            .entry("brightness".to_string())
                            .or_insert_with(|| serde_json::json!({}));
                        if let Some(obj) = entry.as_object_mut() {
                            obj.insert("percent".to_string(), serde_json::json!(new_int));
                            obj.insert("value".to_string(), serde_json::json!(new_int));
                        }
                    }
                }

                *active_slider = Some(SliderDragState {
                    surface_id: surface_id.clone(),
                    widget: sw,
                    id: node_id.clone(),
                    current_value: new_val,
                    last_dispatched_int: Some(new_int),
                    last_dispatched_time: std::time::Instant::now(),
                    on_change: on_change.clone(),
                    node_id: node_id.clone(),
                    parent: node_parent,
                });
                for surface in &mut backend.surface_manager.surfaces {
                    if surface.surface.id() == surface_id
                        || surface.config_name == "__monitor_dimmer__"
                    {
                        surface.dirty = true;
                    }
                }
                state.lock().unwrap().frame_ready = true;
                slider_action = resolve_slider_action(
                    on_change.as_deref(),
                    node_id.as_deref(),
                    node_parent,
                    mut_tree,
                    new_val,
                );
            }
        }
    }
    if let Some(cmd) = slider_action {
        dispatch_custom_action(&cmd, 0.0, 0.0, module_commands);
    }
    active_slider.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_arguments_preserve_quotes_without_shell_expansion() {
        let raw = "notify-send 'Hello World' \"Foo Bar\"";
        let parsed = shell_words::split(raw).unwrap();
        assert_eq!(parsed, vec!["notify-send", "Hello World", "Foo Bar"]);
    }

    #[test]
    fn empty_action_is_rejected() {
        assert!(spawn_action("", 0.0, 0.0).is_err());
        assert!(spawn_action("   ", 0.0, 0.0).is_err());
    }

    #[test]
    fn test_parse_module_event() {
        let (m, w, e) = parse_module_event("clock:cal_prev:click").unwrap();
        assert_eq!(m, "clock");
        assert_eq!(w, "cal_prev");
        assert_eq!(e, "click");

        let (m, w, e) = parse_module_event("tray:menu_item_0_23:click").unwrap();
        assert_eq!(m, "tray");
        assert_eq!(w, "menu_item_0_23");
        assert_eq!(e, "click");

        let (m, w, e) =
            parse_module_event("tray:menu_click|:1.28873|/com/canonical/dbusmenu|23:click")
                .unwrap();
        assert_eq!(m, "tray");
        assert_eq!(w, "menu_click|:1.28873|/com/canonical/dbusmenu|23");
        assert_eq!(e, "click");

        let (m, w, e) = parse_module_event("audio:volume:75").unwrap();
        assert_eq!(m, "audio");
        assert_eq!(w, "volume");
        assert_eq!(e, "75");

        let (m, w, e) = parse_module_event("workspaces:ws_1:click").unwrap();
        assert_eq!(m, "workspaces");
        assert_eq!(w, "ws_1");
        assert_eq!(e, "click");
    }

    #[test]
    fn test_slider_hierarchy_resolution_and_interactive_check() {
        use wyrd_engine::widgets::WidgetNode;
        let mut tree = WidgetTree::new();
        let mut slider_node = WidgetNode::new(WidgetContent::Slider {
            value: 50.0,
            min: 0.0,
            max: 100.0,
            props: Default::default(),
        });
        slider_node.id = Some("volume_slider".to_string());
        let slider_id = tree.insert(slider_node);

        let child_node = WidgetNode::new(WidgetContent::Text {
            text: "subtext".to_string(),
        });
        let child_id = tree.insert(child_node);
        tree.append_child(slider_id, child_id);

        // Verify resolving slider via ancestor traversal from child:
        let mut cur = Some(child_id);
        let mut found_slider = None;
        while let Some(w) = cur {
            if let Some(n) = tree.get(w) {
                if matches!(n.content, WidgetContent::Slider { .. }) {
                    found_slider = Some(w);
                    break;
                }
                cur = n.parent;
            } else {
                break;
            }
        }
        assert_eq!(found_slider, Some(slider_id));

        // Verify interactive element check matches Slider:
        let mut cur2 = Some(child_id);
        let mut is_interactive = false;
        while let Some(w) = cur2 {
            if let Some(n) = tree.get(w) {
                if matches!(
                    n.content,
                    WidgetContent::Button { .. }
                        | WidgetContent::Slider { .. }
                        | WidgetContent::TextInput { .. }
                ) || n.on_click.is_some()
                {
                    is_interactive = true;
                    break;
                }
                cur2 = n.parent;
            } else {
                break;
            }
        }
        assert!(is_interactive);
    }
}
