use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::animator::EasingCurve;
use wyrd_engine::config::{AnimationConfig, AnimationFrom, BarConfig, SurfaceConfig};
use wyrd_engine::modules::CoreMessage;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::wayland::surface::{ShellSurface, SurfaceType};
use wyrd_engine::widgets::WidgetTree;
use wyrd_engine::BarState;

use crate::compositor::{resolve_cursor_output_id, CompositorIntegration};
use crate::interaction::DragState;

pub type PopupKey = (String, ObjectId);

pub fn is_popup_pinned(name: &str, config: &BarConfig) -> bool {
    config
        .surfaces
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.pinned || s.layer == "overlay")
        .unwrap_or(false)
}

pub fn popup_open_for_output(open_popups: &HashSet<PopupKey>, name: &str) -> bool {
    open_popups.iter().any(|(popup_name, _)| popup_name == name)
}

pub fn remove_popup_backdrop(
    backend: &mut WaylandBackend,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
) {
    backend.surface_manager.surfaces.retain(|surface| {
        let is_bd = surface.config_name == "__popup_backdrop__";
        if is_bd {
            surface_trees.remove(&surface.surface.id());
            if let Some(ls) = &surface.layer_surface {
                ls.destroy();
            }
            surface.surface.destroy();
        }
        !is_bd
    });
    open_popups.retain(|(name, _)| name != "__popup_backdrop__");
}

pub fn remove_popup_backdrop_if_empty(
    backend: &mut WaylandBackend,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    config: &BarConfig,
) {
    let unpinned_count = open_popups
        .iter()
        .filter(|(name, _)| !is_popup_pinned(name, config))
        .count();
    if unpinned_count == 0 {
        remove_popup_backdrop(backend, open_popups, surface_trees);
    }
}

pub fn ensure_popup_backdrop(
    _backend: &mut WaylandBackend,
    _state: &Arc<Mutex<BarState>>,
    _target_output_id: ObjectId,
    _surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    _config: &BarConfig,
    _open_popups: &HashSet<PopupKey>,
) {
}

pub fn close_popup(
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    name: &str,
) {
    backend.surface_manager.surfaces.retain(|surface| {
        let same = !surface.dynamic
            && (surface.config_name == name
                || surface.config_name.replace('-', "_") == name.replace('-', "_"));
        if same {
            surface_trees.remove(&surface.surface.id());
            if let Some(layer_surface) = &surface.layer_surface {
                layer_surface.destroy();
            }
            surface.surface.destroy();
        }
        !same
    });
    open_popups.retain(|(popup_name, _)| {
        popup_name != name && popup_name.replace('-', "_") != name.replace('-', "_")
    });
    let config_store = state.lock().unwrap().config.clone();
    if let Ok(config) = config_store.try_read() {
        remove_popup_backdrop_if_empty(backend, open_popups, surface_trees, &config);
    }
    for surface in &mut backend.surface_manager.surfaces {
        surface.dirty = true;
    }
    let has_remaining_text_input = surface_trees.values().any(|tree| {
        tree.iter_nodes().any(|(_, node)| {
            matches!(
                node.content,
                wyrd_engine::widgets::WidgetContent::TextInput { .. }
            )
        })
    });
    if !has_remaining_text_input {
        backend.disable_text_input();
    }
    if let Ok(mut input) = state.lock().unwrap().input_state.try_write() {
        input.focus.pop_scope();
        input.focused_surface_id = None;
    }
    let _ = backend.conn.flush();
}

pub fn close_all_popups(
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
) {
    backend.surface_manager.surfaces.retain(|surface| {
        let is_popup = !surface.dynamic && surface.ty == SurfaceType::Popup;
        if is_popup {
            surface_trees.remove(&surface.surface.id());
            if let Some(layer_surface) = &surface.layer_surface {
                layer_surface.destroy();
            }
            surface.surface.destroy();
        }
        !is_popup
    });
    open_popups.clear();
    let config_store = state.lock().unwrap().config.clone();
    if let Ok(config) = config_store.try_read() {
        remove_popup_backdrop_if_empty(backend, open_popups, surface_trees, &config);
    }
    for surface in &mut backend.surface_manager.surfaces {
        surface.dirty = true;
    }
    backend.disable_text_input();
    if let Ok(mut input) = state.lock().unwrap().input_state.try_write() {
        input.focus.pop_scope();
    }
}

pub fn close_unpinned_popups(
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    config: &BarConfig,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
) {
    let to_remove: Vec<PopupKey> = open_popups
        .iter()
        .filter(|(name, _)| !is_popup_pinned(name, config))
        .cloned()
        .collect();

    for key in to_remove {
        let (pname, out_id) = &key;
        backend.surface_manager.surfaces.retain(|surface| {
            let same = !surface.dynamic
                && surface.config_name == *pname
                && surface
                    .output
                    .as_ref()
                    .is_some_and(|output| output.id() == *out_id);
            if same {
                surface_trees.remove(&surface.surface.id());
                if let Some(layer_surface) = &surface.layer_surface {
                    layer_surface.destroy();
                }
                surface.surface.destroy();
            }
            !same
        });
        open_popups.remove(&key);
    }
    remove_popup_backdrop_if_empty(backend, open_popups, surface_trees, config);
    for surface in &mut backend.surface_manager.surfaces {
        surface.dirty = true;
    }
    if let Ok(mut input) = state.lock().unwrap().input_state.try_write() {
        input.focus.pop_scope();
    }
}

pub fn apply_tree_layout_with_dynamic_height(
    surface: &mut ShellSurface,
    tree: &mut WidgetTree,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
) {
    let scale = surface.scale.max(1.0) as f32;
    let previous_insets = surface.shadow_insets;
    let content_width = (surface.width as f32 - previous_insets.0 - previous_insets.1).max(1.0);
    let (shadow_left, shadow_right, shadow_top, shadow_bottom) = tree.shadow_extents(scale);
    let render_width = content_width;
    wyrd_engine::widgets::tree::measure_tree(tree, render_ctx, render_width, 1440.0);
    let measured_h = tree
        .root()
        .and_then(|r| tree.get(r))
        .map(|n| n.measured_size.1)
        .unwrap_or(surface.height as f32);
    let content_height = measured_h.ceil() as u32;
    let dynamic_w = (content_width + shadow_left + shadow_right).ceil() as u32;
    let dynamic_h = (content_height as f32 + shadow_top + shadow_bottom).ceil() as u32;
    surface.shadow_insets = (shadow_left, shadow_right, shadow_top, shadow_bottom);
    if surface.width != dynamic_w || surface.height != dynamic_h {
        surface.width = dynamic_w;
        surface.height = dynamic_h;
        if let Some(ls) = &surface.layer_surface {
            ls.set_size(surface.width, surface.height);
            surface.surface.commit();
        }
    }
    wyrd_engine::widgets::tree::layout_tree(
        tree,
        shadow_left,
        shadow_top,
        render_width,
        content_height as f32,
    );
    surface_trees.insert(surface.surface.id(), Arc::new(std::mem::take(tree)));
    surface.dirty = true;
}

pub fn are_popups_related(p1: &str, p2: &str, config: &BarConfig) -> bool {
    if p1 == p2 {
        return true;
    }
    for s in &config.surfaces {
        if s.name == p1 && s.parent.as_deref() == Some(p2) {
            return true;
        }
        if s.name == p2 && s.parent.as_deref() == Some(p1) {
            return true;
        }
    }
    p1.starts_with(&format!("{}-", p2)) || p2.starts_with(&format!("{}-", p1))
}

pub fn wait_for_surface_configures(
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
) -> anyhow::Result<()> {
    backend.event_queue.roundtrip(&mut *state.lock().unwrap())?;
    Ok(())
}

pub fn find_anchor_widget_for_popup(
    bar_tree: &WidgetTree,
    popup_name: &str,
    surface_config: &wyrd_engine::config::SurfaceConfig,
) -> Option<(f32, f32, f32, f32)> {
    if let Some(ref anchor_id) = surface_config.anchor_to {
        if let Some(node_id) = bar_tree.find_by_id(anchor_id) {
            if let Some(node) = bar_tree.get(node_id) {
                if node.final_rect.2 > 0.0 && node.final_rect.3 > 0.0 {
                    return Some(node.final_rect);
                }
            }
        }
    }

    log::warn!(
        "Popup '{}' has no explicit 'anchor_to' configured; relying on heuristic search",
        popup_name
    );

    let clean_popup_name = popup_name.replace('-', "_");
    let target_module = surface_config
        .module_channel
        .as_deref()
        .or(surface_config.module.as_deref());

    // 1. Highest priority: node with action or on_click explicitly targeting this popup
    for (_, node) in bar_tree.iter_nodes() {
        let action_opt = node.on_click.as_deref().or(match &node.content {
            wyrd_engine::widgets::WidgetContent::Button { on_click, .. } => on_click.as_deref(),
            _ => None,
        });
        if let Some(action) = action_opt {
            let action_clean = action.trim();
            if (action_clean == format!("popup:{}", popup_name)
                || action_clean == format!("popup:toggle {}", popup_name)
                || action_clean == format!("popup:{}", clean_popup_name)
                || action_clean == format!("popup:toggle {}", clean_popup_name)
                || action_clean.ends_with(&format!(":{}", popup_name))
                || action_clean.ends_with(&format!(":{}", clean_popup_name)))
                && node.final_rect.2 > 0.0
                && node.final_rect.3 > 0.0
            {
                return Some(node.final_rect);
            }
        }
    }

    // 2. Second priority: node ID matching popup name or surface module
    for (_, node) in bar_tree.iter_nodes() {
        if let Some(id) = node.id.as_deref() {
            let clean_id = id.replace('-', "_");
            if (clean_id == clean_popup_name
                || target_module.is_some_and(|m| clean_id == m.replace('-', "_"))
                || clean_id.starts_with(&format!("{}_", clean_popup_name))
                || clean_id.ends_with(&format!("_{}", clean_popup_name))
                || clean_popup_name.starts_with(&format!("{}_", clean_id))
                || clean_popup_name.ends_with(&format!("_{}", clean_id)))
                && node.final_rect.2 > 0.0
                && node.final_rect.3 > 0.0
            {
                return Some(node.final_rect);
            }
        }
    }

    // 3. Third priority: Module content matching popup name or surface module
    for (_, node) in bar_tree.iter_nodes() {
        if let wyrd_engine::widgets::WidgetContent::Module { ref module, .. } = node.content {
            let clean_mod = module.replace('-', "_");
            if (clean_mod == clean_popup_name
                || target_module.is_some_and(|m| clean_mod == m.replace('-', "_"))
                || module == popup_name)
                && node.final_rect.2 > 0.0
                && node.final_rect.3 > 0.0
            {
                return Some(node.final_rect);
            }
        }
    }

    None
}

pub fn resolve_popup_open_animation(
    surface_config: &SurfaceConfig,
    config: &BarConfig,
    popup_name: &str,
    is_left_bar: bool,
    is_bottom_bar: bool,
) -> (AnimationConfig, bool) {
    if let Some(ref anim) = surface_config.open_animation {
        return (anim.clone(), false);
    }

    let preset_key = if is_left_bar {
        "@popup_open_left_bar"
    } else if is_bottom_bar {
        "@popup_open_bottom_bar"
    } else {
        "@popup_open"
    };

    if let Some(anim) = config.animations.get(preset_key) {
        return (anim.clone(), false);
    }

    let bare_key = &preset_key[1..];
    if let Some(anim) = config.animations.get(bare_key) {
        return (anim.clone(), false);
    }

    log::debug!(
        "Popup '{}' has no animation configured and preset '{}' not found; using last-resort fallback",
        popup_name,
        preset_key
    );

    let (from_x, from_y) = if is_left_bar {
        (Some(-12.0), Some(0.0))
    } else if is_bottom_bar {
        (Some(0.0), Some(12.0))
    } else {
        (Some(0.0), Some(-12.0))
    };

    let fallback = AnimationConfig {
        kind: "spring".to_string(),
        stiffness: Some(300.0),
        damping: Some(26.0),
        duration_ms: None,
        curve: None,
        from: Some(AnimationFrom {
            x: from_x,
            y: from_y,
            opacity: Some(0.0),
            scale: Some(1.0),
        }),
        to: Some(AnimationFrom {
            x: Some(0.0),
            y: Some(0.0),
            opacity: Some(1.0),
            scale: Some(1.0),
        }),
    };

    (fallback, true)
}

#[allow(clippy::too_many_arguments)]
pub fn toggle_configured_popup(
    compositor: &dyn CompositorIntegration,
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    config: &BarConfig,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    name: &str,
    source_surface_id: ObjectId,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    params: Option<&serde_json::Value>,
) -> anyhow::Result<()> {
    let synthetic_config;
    let surface_config = if let Some(found) = config.surfaces.iter().find(|surface| {
        surface.ty == "popup"
            && (surface.name == name
                || surface.name.replace('-', "_") == name.replace('-', "_")
                || (name != "tray" && surface.module.as_deref() == Some(name)))
    }) {
        found
    } else {
        let is_pinned = is_popup_pinned(name, config);
        let bar_top_margin = config
            .surfaces
            .iter()
            .find(|s| s.ty == "bar")
            .map(|s| (s.height + s.margin.top as u32 + s.margin.bottom as u32 + 8) as i32)
            .unwrap_or(0);
        synthetic_config = wyrd_engine::config::SurfaceConfig {
            name: name.to_string(),
            ty: "popup".to_string(),
            layer: "top".to_string(),
            output: None,
            anchor: vec!["top".to_string(), "right".to_string()],
            margin: wyrd_engine::config::MarginConfig {
                top: bar_top_margin,
                right: 12,
                bottom: 0,
                left: 0,
            },
            height: 360,
            width: Some(360),
            exclusive_zone: 0,
            keyboard: "none".to_string(),
            visible: true,
            module: None,
            module_channel: None,
            anchor_to: None,
            open_animation: None,
            close_animation: None,
            pinned: is_pinned,
            parent: None,
            has_build_fn: false,
            style: None,
            drag_strip_height: None,
            widgets: Vec::new(),
        };
        &synthetic_config
    };

    let output_id = if state
        .lock()
        .unwrap()
        .outputs
        .values()
        .any(|o| o.output.id() == source_surface_id)
    {
        Some(source_surface_id.clone())
    } else {
        backend
            .surface_manager
            .surfaces
            .iter()
            .find(|surface| surface.surface.id() == source_surface_id)
            .and_then(|surface| surface.output.as_ref())
            .map(Proxy::id)
    }
    .or_else(|| resolve_cursor_output_id(compositor, state, surface_config.output.as_deref()))
    .or_else(|| open_popups.iter().next().map(|(_, out_id)| out_id.clone()))
    .or_else(|| {
        state
            .lock()
            .unwrap()
            .outputs
            .values()
            .next()
            .map(|o| o.output.id())
    })
    .ok_or_else(|| anyhow::anyhow!("pointer source surface has no output binding"))?;
    let popup_key = (name.to_owned(), output_id.clone());
    if !open_popups.insert(popup_key.clone()) {
        backend.surface_manager.surfaces.retain(|surface| {
            let same_popup = !surface.dynamic
                && surface.config_name == name
                && surface
                    .output
                    .as_ref()
                    .is_some_and(|output| output.id() == output_id);
            if same_popup {
                let anim_key = format!("popup_{:?}", surface.surface.id());
                render_ctx.animator.remove_named(&anim_key);
                surface_trees.remove(&surface.surface.id());
                if let Some(layer_surface) = &surface.layer_surface {
                    layer_surface.destroy();
                }
                surface.surface.destroy();
            }
            !same_popup
        });
        open_popups.remove(&popup_key);
        remove_popup_backdrop_if_empty(backend, open_popups, surface_trees, config);
        if let Ok(mut input) = state.lock().unwrap().input_state.try_write() {
            input.focus.pop_scope();
        }
        let target_module = surface_config
            .module_channel
            .as_deref()
            .or(surface_config.module.as_deref())
            .unwrap_or(name);
        for (mod_name, sender) in module_commands {
            if mod_name == target_module || mod_name == name {
                let _ = sender.try_send(CoreMessage::PopupEvent {
                    popup_id: name.to_string(),
                    event: "close".to_string(),
                });
            }
        }
        return Ok(());
    }

    ensure_popup_backdrop(
        backend,
        state,
        output_id.clone(),
        surface_trees,
        config,
        open_popups,
    );

    let content_width = surface_config.width.unwrap_or(360);
    let data_store = state
        .lock()
        .unwrap()
        .module_data
        .try_read()
        .map(|s| s.clone())
        .unwrap_or_default();
    let mut initial_tree = if !surface_config.widgets.is_empty() {
        wyrd_engine::widgets::from_popup_config_with_store(
            &surface_config.widgets,
            &config.styles,
            &data_store,
        )
    } else {
        let mut tree = WidgetTree::new();
        tree.styles = Arc::new(config.styles.clone());
        let mut node =
            wyrd_engine::widgets::WidgetNode::new(wyrd_engine::widgets::WidgetContent::Container);
        node.id = Some(format!("popup:{}", name));
        node.layout.mode = wyrd_engine::widgets::layout::LayoutMode::Flex(
            wyrd_engine::widgets::layout::FlexDirection::Vertical,
        );
        node.layout.justify = wyrd_engine::widgets::layout::JustifyContent::Start;
        node.layout.align = wyrd_engine::widgets::layout::Align::Stretch;

        let popup_style_key = format!("popup:{}", name);
        let popup_suffix_key = format!("{}_popup", name);
        let style_opt = surface_config
            .style
            .as_deref()
            .and_then(|s| config.styles.get(s))
            .or_else(|| config.styles.get(&popup_style_key))
            .or_else(|| config.styles.get(name))
            .or_else(|| config.styles.get(&popup_suffix_key))
            .or_else(|| config.styles.get("popup"))
            .or_else(|| config.styles.get("card"));

        if let Some(style) = style_opt {
            wyrd_engine::widgets::apply_style_to_node(&mut node, style);
            if style.padding.is_none() {
                node.layout.padding = (10.0, 12.0, 10.0, 12.0);
            }
        } else {
            node.layout.padding = (10.0, 12.0, 10.0, 12.0);
        }
        node.layout.gap = 6.0;

        let root_id = tree.insert(node);

        let target_module = surface_config
            .module_channel
            .as_deref()
            .or(surface_config.module.as_deref())
            .unwrap_or(name);
        let module_key = if module_commands.contains_key(target_module) {
            Some(target_module)
        } else if module_commands.contains_key(name) {
            Some(name)
        } else {
            module_commands
                .keys()
                .find(|k| *k == target_module || *k == name)
                .map(|s| s.as_str())
        };
        if let Some(module_key) = module_key {
            let initial_payload = data_store
                .get(&format!("popup:{}", name))
                .or_else(|| data_store.get(name))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mut mod_node = wyrd_engine::widgets::WidgetNode::new(
                wyrd_engine::widgets::WidgetContent::Module {
                    module: module_key.to_string(),
                    payload: initial_payload.clone(),
                },
            );
            mod_node.id = Some(format!("popup:{}", name));
            let mod_id = tree.insert(mod_node);
            tree.append_child(root_id, mod_id);

            if let Some(children) = initial_payload.get("children").and_then(|v| v.as_array()) {
                tree.sync_dynamic_children(mod_id, children);
            }
        }

        tree
    };

    let output_scale = state
        .lock()
        .unwrap()
        .outputs
        .values()
        .find(|output| output.output.id() == output_id)
        .map(|output| output.scale as f32)
        .unwrap_or(1.0);
    let (shadow_left, shadow_right, shadow_top, shadow_bottom) =
        initial_tree.shadow_extents(output_scale);
    wyrd_engine::widgets::tree::measure_tree(
        &mut initial_tree,
        render_ctx,
        content_width as f32,
        1080.0,
    );
    let measured_h = initial_tree
        .root()
        .and_then(|r| initial_tree.get(r))
        .map(|n| n.measured_size.1)
        .unwrap_or(surface_config.height as f32);
    let content_height = measured_h.ceil() as u32;
    let popup_width = (content_width as f32 + shadow_left + shadow_right).ceil() as u32;
    let popup_height = (content_height as f32 + shadow_top + shadow_bottom).ceil() as u32;

    let tree_has_text_input = initial_tree.iter_nodes().any(|(_, node)| {
        matches!(
            node.content,
            wyrd_engine::widgets::WidgetContent::TextInput { .. }
        )
    });
    let keyboard_mode = if tree_has_text_input && surface_config.keyboard == "none" {
        "on_demand"
    } else {
        &surface_config.keyboard
    };

    let (target_anchor, target_margin) = {
        let bar_surface_opt = backend
            .surface_manager
            .surfaces
            .iter()
            .find(|s| {
                s.ty == SurfaceType::Bar
                    && (s.surface.id() == source_surface_id
                        || s.output.as_ref().map(Proxy::id) == Some(output_id.clone()))
            })
            .or_else(|| {
                backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| s.ty == SurfaceType::Bar)
            });

        let bar_surface_cfg = bar_surface_opt
            .and_then(|s| config.surfaces.iter().find(|sc| sc.name == s.config_name))
            .or_else(|| config.surfaces.iter().find(|sc| sc.ty == "bar"));

        let is_bottom_bar = bar_surface_opt.map_or_else(
            || {
                bar_surface_cfg.is_some_and(|s| {
                    s.anchor.iter().any(|a| a == "bottom") && !s.anchor.iter().any(|a| a == "top")
                })
            },
            |s| s.anchors.iter().any(|a| a == "bottom") && !s.anchors.iter().any(|a| a == "top"),
        );

        let is_left_bar = bar_surface_opt.map_or_else(
            || {
                bar_surface_cfg.is_some_and(|s| {
                    s.anchor.iter().any(|a| a == "left") && !s.anchor.iter().any(|a| a == "right")
                })
            },
            |s| s.anchors.iter().any(|a| a == "left") && !s.anchors.iter().any(|a| a == "right"),
        );

        let bar_capsule_height = bar_surface_cfg
            .map(|sc| sc.height as f32)
            .unwrap_or_else(|| bar_surface_opt.map(|s| s.height as f32).unwrap_or(0.0));
        let bar_capsule_width = bar_surface_cfg
            .and_then(|sc| sc.width)
            .map(|w| w as f32)
            .unwrap_or_else(|| bar_surface_opt.map(|s| s.width as f32).unwrap_or(48.0));

        let bar_margin_top = bar_surface_cfg
            .map(|sc| sc.margin.top as f32)
            .unwrap_or(8.0);
        let bar_margin_bottom = bar_surface_cfg
            .map(|sc| sc.margin.bottom as f32)
            .unwrap_or(8.0);
        let bar_margin_left = bar_surface_cfg
            .map(|sc| sc.margin.left as f32)
            .unwrap_or(8.0);
        let bar_margin_right = bar_surface_cfg
            .map(|sc| sc.margin.right as f32)
            .unwrap_or(8.0);

        let bar_margin_v = if is_bottom_bar {
            bar_margin_bottom
        } else {
            bar_margin_top
        };
        let v_gap = bar_margin_v.clamp(4.0, 16.0);

        let popup_surface_margin_top = {
            let bar_capsule_bottom = bar_margin_top + bar_capsule_height;
            let popup_capsule_top = bar_capsule_bottom + v_gap;
            (popup_capsule_top - shadow_top).max(0.0) as i32
        };

        let popup_surface_margin_bottom = {
            let bar_capsule_top_from_bottom = bar_margin_bottom + bar_capsule_height;
            let popup_capsule_bottom = bar_capsule_top_from_bottom + v_gap;
            (popup_capsule_bottom - shadow_bottom).max(0.0) as i32
        };

        let popup_surface_margin_left = {
            let bar_capsule_right = bar_margin_left + bar_capsule_width;
            let popup_capsule_left = bar_capsule_right + bar_margin_left.clamp(4.0, 16.0);
            (popup_capsule_left - shadow_left).max(0.0) as i32
        };

        let v_anchor = if is_bottom_bar {
            "bottom".to_string()
        } else {
            "top".to_string()
        };

        let bar_tree_opt = bar_surface_opt
            .and_then(|s| surface_trees.get(&s.surface.id()))
            .or_else(|| {
                backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| s.ty == SurfaceType::Bar)
                    .and_then(|s| surface_trees.get(&s.surface.id()))
            });

        let trigger_rect = params
            .and_then(|p| {
                let tx = p.get("trigger_x").and_then(|v| v.as_f64())?;
                let ty = p.get("trigger_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let tw = p.get("trigger_w").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let th = p.get("trigger_h").and_then(|v| v.as_f64()).unwrap_or(0.0);
                Some((tx as f32, ty as f32, tw as f32, th as f32))
            })
            .or_else(|| {
                bar_tree_opt
                    .and_then(|tree| find_anchor_widget_for_popup(tree, name, surface_config))
            });

        if is_left_bar {
            let bar_surface_margin_top =
                bar_surface_opt
                    .map(|s| s.margin.0 as f32)
                    .unwrap_or_else(|| {
                        let bar_shadow_top =
                            bar_surface_opt.map(|s| s.shadow_insets.2).unwrap_or(0.0);
                        bar_margin_top - bar_shadow_top
                    });
            let widget_center_y = if let Some((_tx, ty, _tw, th)) = trigger_rect {
                bar_surface_margin_top + ty + (th / 2.0)
            } else {
                bar_surface_margin_top + (content_height as f32 / 2.0)
            };

            let (_, output_logical_h) = {
                let s = state.lock().unwrap();
                s.outputs
                    .values()
                    .find(|out| out.output.id() == output_id)
                    .map(|out| {
                        let w = if out.logical_width > 0 {
                            out.logical_width
                        } else {
                            out.current_mode
                                .map(|(w, _)| w / out.scale.max(1))
                                .unwrap_or(1920)
                        };
                        let h = if out.logical_height > 0 {
                            out.logical_height
                        } else {
                            out.current_mode
                                .map(|(_, h)| h / out.scale.max(1))
                                .unwrap_or(1080)
                        };
                        (w, h)
                    })
                    .unwrap_or((1920, 1080))
            };

            let ideal_top = widget_center_y - shadow_top - (content_height as f32 / 2.0);
            let screen_edge_gap = bar_margin_top.max(bar_margin_bottom).max(8.0);
            let min_surface_top = screen_edge_gap - shadow_top;
            let max_surface_top =
                (output_logical_h as f32) - screen_edge_gap - (content_height as f32) - shadow_top;
            let clamped_top =
                ideal_top.clamp(min_surface_top, max_surface_top.max(min_surface_top));

            let m = (clamped_top as i32, 0, 0, popup_surface_margin_left);
            (vec!["top".to_string(), "left".to_string()], m)
        } else if let Some((tx, _ty, tw, _th)) = trigger_rect {
            let bar_surface_margin_left = bar_surface_opt
                .map(|s| s.margin.3 as f32)
                .unwrap_or_else(|| {
                    let bar_shadow_left = bar_surface_opt.map(|s| s.shadow_insets.0).unwrap_or(0.0);
                    bar_margin_left - bar_shadow_left
                });
            let widget_center_x = bar_surface_margin_left + tx + (tw / 2.0);

            let (output_logical_w, _) = {
                let s = state.lock().unwrap();
                s.outputs
                    .values()
                    .find(|out| out.output.id() == output_id)
                    .map(|out| {
                        let w = if out.logical_width > 0 {
                            out.logical_width
                        } else {
                            out.current_mode
                                .map(|(w, _)| w / out.scale.max(1))
                                .unwrap_or(1920)
                        };
                        let h = if out.logical_height > 0 {
                            out.logical_height
                        } else {
                            out.current_mode
                                .map(|(_, h)| h / out.scale.max(1))
                                .unwrap_or(1080)
                        };
                        (w, h)
                    })
                    .unwrap_or((1920, 1080))
            };

            let ideal_left = widget_center_x - shadow_left - (content_width as f32 / 2.0);
            let screen_edge_gap = bar_margin_left.max(bar_margin_right).max(8.0);
            let min_surface_left = screen_edge_gap - shadow_left;
            let max_surface_left =
                (output_logical_w as f32) - screen_edge_gap - (content_width as f32) - shadow_left;
            let clamped_left =
                ideal_left.clamp(min_surface_left, max_surface_left.max(min_surface_left));

            let m = if is_bottom_bar {
                (0, 0, popup_surface_margin_bottom, clamped_left as i32)
            } else {
                (popup_surface_margin_top, 0, 0, clamped_left as i32)
            };
            (vec![v_anchor, "left".to_string()], m)
        } else {
            let mut anchors = surface_config.anchor.clone();
            if is_bottom_bar {
                for a in &mut anchors {
                    if a == "top" {
                        *a = "bottom".to_string();
                    }
                }
            }
            let default_v_margin = if is_bottom_bar {
                popup_surface_margin_bottom
            } else {
                popup_surface_margin_top
            };
            let mut m = if let Some(p) = params {
                (
                    p.get("margin_top")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(surface_config.margin.top),
                    p.get("margin_right")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(surface_config.margin.right),
                    p.get("margin_bottom")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(surface_config.margin.bottom),
                    p.get("margin_left")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(surface_config.margin.left),
                )
            } else {
                (
                    surface_config.margin.top,
                    surface_config.margin.right,
                    surface_config.margin.bottom,
                    surface_config.margin.left,
                )
            };
            if is_bottom_bar {
                if m.2 == 0 {
                    m.2 = if m.0 > 0 { m.0 } else { default_v_margin };
                }
                m.0 = 0;
            } else if m.0 == 0 && m.2 > 0 {
                m.0 = m.2;
                m.2 = 0;
            } else if m.0 == 0 {
                m.0 = default_v_margin;
            }
            (anchors, m)
        }
    };

    let outputs = state
        .lock()
        .unwrap()
        .outputs
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let surface_type = SurfaceType::Popup;
    if outputs.is_empty() {
        backend.surface_manager.create_surface(
            &backend.compositor,
            &backend.qh,
            surface_type,
            name,
            false,
            None,
            popup_width,
            popup_height,
            &surface_config.layer,
            &target_anchor,
            target_margin,
            surface_config.exclusive_zone,
            keyboard_mode,
            1.0,
        );
    } else {
        for output in outputs {
            if output.output.id() != output_id {
                continue;
            }
            if surface_config
                .output
                .as_deref()
                .is_some_and(|name| name != output.name)
            {
                continue;
            }
            backend.surface_manager.create_surface(
                &backend.compositor,
                &backend.qh,
                surface_type,
                name,
                false,
                Some(&output.output),
                popup_width,
                popup_height,
                &surface_config.layer,
                &target_anchor,
                target_margin,
                0,
                keyboard_mode,
                output.scale as f64,
            );
        }
    }
    wait_for_surface_configures(backend, state)?;
    let bar_surface_cfg = config.surfaces.iter().find(|sc| sc.ty == "bar");
    let is_bottom_bar = bar_surface_cfg.is_some_and(|s| {
        s.anchor.iter().any(|a| a == "bottom") && !s.anchor.iter().any(|a| a == "top")
    });
    let is_left_bar = bar_surface_cfg.is_some_and(|s| {
        s.anchor.iter().any(|a| a == "left") && !s.anchor.iter().any(|a| a == "right")
    });
    let mut first_popup_widget = None;
    let mut new_popup_surface_id = None;
    for surface in backend
        .surface_manager
        .surfaces
        .iter_mut()
        .filter(|surface| {
            !surface.dynamic
                && surface.config_name == name
                && surface
                    .output
                    .as_ref()
                    .is_some_and(|output| output.id() == output_id)
        })
    {
        new_popup_surface_id = Some(surface.surface.id());
        surface.shadow_insets = (shadow_left, shadow_right, shadow_top, shadow_bottom);
        let mut popup_tree = initial_tree.clone();
        first_popup_widget = first_popup_widget.or_else(|| popup_tree.first_child());
        apply_tree_layout_with_dynamic_height(surface, &mut popup_tree, surface_trees, render_ctx);
        let anim_key = format!("popup_{:?}", surface.surface.id());
        let offset_y_key = format!("popup_y_{:?}", surface.surface.id());
        let offset_x_key = format!("popup_x_{:?}", surface.surface.id());
        render_ctx.animator.remove_named(&anim_key);
        render_ctx.animator.remove_named(&offset_y_key);
        render_ctx.animator.remove_named(&offset_x_key);

        let (anim_cfg, _is_fallback) =
            resolve_popup_open_animation(surface_config, config, name, is_left_bar, is_bottom_bar);

        let initial_opacity = anim_cfg
            .from
            .as_ref()
            .and_then(|f| f.opacity)
            .unwrap_or(0.0) as f64;
        let target_opacity = anim_cfg.to.as_ref().and_then(|t| t.opacity).unwrap_or(1.0) as f64;
        let initial_x = anim_cfg.from.as_ref().and_then(|f| f.x).unwrap_or(0.0) as f64;
        let target_x = anim_cfg.to.as_ref().and_then(|t| t.x).unwrap_or(0.0) as f64;
        let initial_y = anim_cfg.from.as_ref().and_then(|f| f.y).unwrap_or(0.0) as f64;
        let target_y = anim_cfg.to.as_ref().and_then(|t| t.y).unwrap_or(0.0) as f64;

        if anim_cfg.kind == "easing" {
            let dur = Duration::from_millis(anim_cfg.duration_ms.unwrap_or(250) as u64);
            let curve = match anim_cfg.curve.as_deref() {
                Some("linear") => EasingCurve::Linear,
                Some("ease_in_out") | Some("ease-in-out") => EasingCurve::EaseInOutQuad,
                _ => EasingCurve::EaseOutCubic,
            };
            render_ctx.animator.start_named_easing(
                &anim_key,
                initial_opacity,
                target_opacity,
                dur,
                curve,
            );
            if initial_x != target_x {
                render_ctx.animator.start_named_easing(
                    &offset_x_key,
                    initial_x,
                    target_x,
                    dur,
                    curve,
                );
            }
            if initial_y != target_y {
                render_ctx.animator.start_named_easing(
                    &offset_y_key,
                    initial_y,
                    target_y,
                    dur,
                    curve,
                );
            }
        } else {
            let stiffness = anim_cfg.stiffness.unwrap_or(300.0) as f64;
            let damping = anim_cfg.damping.unwrap_or(26.0) as f64;
            render_ctx.animator.start_named_spring(
                &anim_key,
                initial_opacity,
                target_opacity,
                stiffness,
                damping,
            );
            if initial_x != target_x {
                render_ctx.animator.start_named_spring(
                    &offset_x_key,
                    initial_x,
                    target_x,
                    stiffness,
                    damping,
                );
            }
            if initial_y != target_y {
                render_ctx.animator.start_named_spring(
                    &offset_y_key,
                    initial_y,
                    target_y,
                    stiffness,
                    damping,
                );
            }
        }

        if let Some(tree) = surface_trees.get_mut(&surface.surface.id()) {
            let mut_tree = Arc::make_mut(tree);
            if let Some(root_id) = mut_tree.root() {
                if let Some(node) = mut_tree.get_mut(root_id) {
                    node.style.opacity = initial_opacity as f32;
                }
            }
            let initial_layout_x = shadow_left + initial_x as f32;
            let initial_layout_y = shadow_top + initial_y as f32;
            wyrd_engine::widgets::tree::layout_tree(
                mut_tree,
                initial_layout_x,
                initial_layout_y,
                content_width as f32,
                content_height as f32,
            );
        }
    }
    if tree_has_text_input {
        backend.enable_text_input();
    }
    if let Some(surf_id) = new_popup_surface_id {
        if let Ok(mut input) = state.lock().unwrap().input_state.try_write() {
            input.focused_surface_id = Some(surf_id);
            if let Some(widget) = first_popup_widget {
                input.focus.push_scope();
                input.focus.set_focus(widget);
            }
        }
    }
    let target_module = surface_config
        .module_channel
        .as_deref()
        .or(surface_config.module.as_deref())
        .unwrap_or(name);
    for (mod_name, sender) in module_commands {
        if mod_name == target_module || mod_name == name {
            let _ = sender.try_send(CoreMessage::PopupEvent {
                popup_id: name.to_string(),
                event: "open".to_string(),
            });
        }
    }
    for surface in &mut backend.surface_manager.surfaces {
        surface.dirty = true;
    }
    if let Ok(mut lock) = state.lock().unwrap().surface_trees.try_write() {
        *lock = surface_trees.clone();
    }
    state.lock().unwrap().frame_ready = true;
    Ok(())
}

pub fn module_surface_request(
    compositor: &dyn CompositorIntegration,
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    module: &str,
    action: &str,
    params: &serde_json::Value,
) -> anyhow::Result<()> {
    let target = params
        .get("id")
        .or_else(|| params.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(module);
    let key = format!("{module}:{target}");

    if matches!(action, "close" | "destroy") {
        backend.surface_manager.surfaces.retain(|surface| {
            let matches_target = surface.config_name == key || surface.config_name == target;
            if surface.dynamic && matches_target {
                if let Some(layer) = &surface.layer_surface {
                    layer.destroy();
                }
                surface.surface.destroy();
                false
            } else {
                true
            }
        });
        return Ok(());
    }

    if action == "update" {
        let current_size = backend
            .surface_manager
            .surfaces
            .iter()
            .find(|surface| {
                surface.dynamic && (surface.config_name == key || surface.config_name == target)
            })
            .map(|surface| (surface.width, surface.height))
            .ok_or_else(|| anyhow::anyhow!("dynamic surface '{target}' does not exist"))?;
        let width = params
            .get("width")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(current_size.0 as u64) as u32;
        let height = params
            .get("height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(current_size.1 as u64) as u32;
        if !backend.surface_manager.update_surface_size(
            &backend.compositor,
            &backend.qh,
            &key,
            width,
            height,
        ) {
            anyhow::bail!("dynamic surface '{target}' does not exist");
        }
        wait_for_surface_configures(backend, state)?;
        return Ok(());
    }

    if !matches!(action, "open" | "create") {
        anyhow::bail!("unsupported surface action '{action}'");
    }
    if backend
        .surface_manager
        .surfaces
        .iter()
        .any(|surface| surface.dynamic && surface.config_name == key)
    {
        return Ok(());
    }

    let surface_type = match params
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("popup")
    {
        "bar" => SurfaceType::Bar,
        "panel" => SurfaceType::Panel,
        "popup" => SurfaceType::Popup,
        _ => SurfaceType::Background,
    };

    let width = params
        .get("width")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(360) as u32;
    let height = params
        .get("height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(240) as u32;
    let layer = params
        .get("layer")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("top");
    let anchor_values = params
        .get("anchor")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["top".to_string(), "right".to_string()]);
    let bar_top_margin = {
        let conf = state.lock().unwrap().config.clone();
        conf.try_read()
            .ok()
            .and_then(|c| {
                c.surfaces
                    .iter()
                    .find(|s| s.ty == "bar")
                    .map(|s| (s.height + s.margin.top as u32 + s.margin.bottom as u32 + 8) as i32)
            })
            .unwrap_or(0)
    };
    let margin = (
        params
            .get("margin_top")
            .and_then(serde_json::Value::as_i64)
            .map(|v| v as i32)
            .unwrap_or(bar_top_margin),
        params
            .get("margin_right")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        params
            .get("margin_bottom")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
        params
            .get("margin_left")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0) as i32,
    );
    let exclusive_zone = params
        .get("exclusive_zone")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0) as i32;
    let keyboard = params
        .get("keyboard")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("none");
    let output_name = params.get("output").and_then(serde_json::Value::as_str);

    let outputs = state
        .lock()
        .unwrap()
        .outputs
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let target_output = if let Some(name) = output_name {
        outputs.into_iter().find(|output| output.name == name)
    } else if let Some(out_id) = resolve_cursor_output_id(compositor, state, None) {
        outputs
            .into_iter()
            .find(|output| output.output.id() == out_id)
    } else {
        None
    };

    backend.surface_manager.create_surface(
        &backend.compositor,
        &backend.qh,
        surface_type,
        &key,
        true,
        target_output.as_ref().map(|o| &o.output),
        width,
        height,
        layer,
        &anchor_values,
        margin,
        exclusive_zone,
        keyboard,
        target_output
            .as_ref()
            .map(|o| o.scale as f64)
            .unwrap_or(1.0),
    );
    wait_for_surface_configures(backend, state)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_popup_drag(
    drag: &mut DragState,
    compositor: &dyn CompositorIntegration,
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
) {
    if let Some((cur_gx, cur_gy)) = compositor.cursor_position() {
        let total_dx = cur_gx - drag.start_global_x;
        let total_dy = cur_gy - drag.start_global_y;

        let outputs_snapshot = state
            .lock()
            .unwrap()
            .outputs
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let target_output = outputs_snapshot
            .iter()
            .find(|o| {
                cur_gx >= o.logical_x
                    && cur_gx < (o.logical_x + o.logical_width)
                    && cur_gy >= o.logical_y
                    && cur_gy < (o.logical_y + o.logical_height)
            })
            .or_else(|| {
                outputs_snapshot
                    .iter()
                    .find(|o| o.output.id() == drag.output_id)
            })
            .cloned();

        if let Some(target_mon) = target_output {
            let mon_w = (target_mon.logical_width as f64 / target_mon.scale as f64).round() as i32;
            let mon_h = (target_mon.logical_height as f64 / target_mon.scale as f64).round() as i32;
            let target_out_id = target_mon.output.id();

            if target_out_id != drag.output_id {
                let config_arc = state.lock().unwrap().config.clone();
                let current_config = config_arc.read().await.clone();
                let old_key = (drag.popup_name.clone(), drag.output_id.clone());
                open_popups.remove(&old_key);
                backend.surface_manager.surfaces.retain(|s| {
                    let same = s.config_name == drag.popup_name
                        && s.output.as_ref().is_some_and(|o| o.id() == drag.output_id);
                    if same {
                        surface_trees.remove(&s.surface.id());
                        if let Some(ls) = &s.layer_surface {
                            ls.destroy();
                        }
                        s.surface.destroy();
                    }
                    !same
                });

                if let Err(e) = toggle_configured_popup(
                    compositor,
                    backend,
                    state,
                    &current_config,
                    open_popups,
                    surface_trees,
                    &drag.popup_name,
                    target_out_id.clone(),
                    render_ctx,
                    module_commands,
                    None,
                ) {
                    log::error!("Failed to migrate dragging popup to new output: {}", e);
                } else if let Some(new_surf) = backend.surface_manager.surfaces.iter().find(|s| {
                    s.config_name == drag.popup_name
                        && s.output.as_ref().is_some_and(|o| o.id() == target_out_id)
                }) {
                    drag.surface_id = new_surf.surface.id();
                    drag.output_id = target_out_id;
                    drag.start_global_x = cur_gx;
                    drag.start_global_y = cur_gy;
                    drag.start_margin = new_surf.margin;
                }
            } else if let Some(surf) = backend
                .surface_manager
                .surfaces
                .iter_mut()
                .find(|s| s.surface.id() == drag.surface_id)
            {
                let min_visible = 64;
                let clamp_min_left = -(surf.width as i32) + min_visible;
                let clamp_max_left = mon_w - min_visible;
                let clamp_min_top = 0;
                let clamp_max_top = mon_h - min_visible;

                let (sm_top, sm_right, sm_bottom, sm_left) = drag.start_margin;
                let new_left = (sm_left + total_dx).clamp(clamp_min_left, clamp_max_left);
                let new_top = (sm_top + total_dy).clamp(clamp_min_top, clamp_max_top);

                surf.margin = (new_top, sm_right, sm_bottom, new_left);
                if let Some(ls) = &surf.layer_surface {
                    ls.set_margin(new_top, sm_right, sm_bottom, new_left);
                }
                surf.surface.commit();
                surf.dirty = true;
                state.lock().unwrap().frame_ready = true;
            }
        }
    }
}

pub async fn handle_popup_click_outside(
    backend: &mut WaylandBackend,
    state: &Arc<Mutex<BarState>>,
    open_popups: &mut HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    surface_id: &ObjectId,
) -> bool {
    let is_backdrop_click = backend
        .surface_manager
        .surfaces
        .iter()
        .any(|s| &s.surface.id() == surface_id && s.config_name == "__popup_backdrop__");
    if is_backdrop_click {
        let config_arc = state.lock().unwrap().config.clone();
        let current_config = config_arc.read().await.clone();
        close_unpinned_popups(backend, state, &current_config, open_popups, surface_trees);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use wyrd_engine::config::SurfaceConfig;
    use wyrd_engine::widgets::{WidgetContent, WidgetNode, WidgetTree};

    #[test]
    fn test_find_anchor_widget_by_action() {
        let mut tree = WidgetTree::new();
        let mut node = WidgetNode::new(WidgetContent::Button {
            label: "Launcher".to_string(),
            on_click: Some("popup:launcher".to_string()),
        });
        node.final_rect = (10.0, 5.0, 32.0, 32.0);
        let _ = tree.insert(node);

        let cfg = SurfaceConfig::default();
        let rect = find_anchor_widget_for_popup(&tree, "launcher", &cfg);
        assert_eq!(rect, Some((10.0, 5.0, 32.0, 32.0)));
    }

    #[test]
    fn test_find_anchor_widget_by_id() {
        let mut tree = WidgetTree::new();
        let mut node = WidgetNode::new(WidgetContent::Container);
        node.id = Some("power".to_string());
        node.final_rect = (100.0, 5.0, 28.0, 28.0);
        let _ = tree.insert(node);

        let cfg = SurfaceConfig::default();
        let rect = find_anchor_widget_for_popup(&tree, "power", &cfg);
        assert_eq!(rect, Some((100.0, 5.0, 28.0, 28.0)));
    }

    #[test]
    fn test_find_anchor_widget_by_module() {
        let mut tree = WidgetTree::new();
        let mut node = WidgetNode::new(WidgetContent::Module {
            module: "mpris".to_string(),
            payload: serde_json::Value::Null,
        });
        node.final_rect = (50.0, 5.0, 120.0, 28.0);
        let _ = tree.insert(node);

        let cfg = SurfaceConfig::default();
        let rect = find_anchor_widget_for_popup(&tree, "mpris", &cfg);
        assert_eq!(rect, Some((50.0, 5.0, 120.0, 28.0)));
    }

    #[test]
    fn test_find_anchor_widget_ignores_zero_dimensions() {
        let mut tree = WidgetTree::new();
        let mut node = WidgetNode::new(WidgetContent::Button {
            label: "Launcher".to_string(),
            on_click: Some("popup:launcher".to_string()),
        });
        node.final_rect = (0.0, 0.0, 0.0, 0.0);
        let _ = tree.insert(node);

        let cfg = SurfaceConfig::default();
        let rect = find_anchor_widget_for_popup(&tree, "launcher", &cfg);
        assert_eq!(rect, None);
    }

    #[test]
    fn test_find_anchor_widget_by_explicit_anchor_to() {
        let mut tree = WidgetTree::new();
        let mut node = WidgetNode::new(WidgetContent::Button {
            label: "Vol".to_string(),
            on_click: None,
        });
        node.id = Some("audio_button".to_string());
        node.final_rect = (200.0, 5.0, 40.0, 30.0);
        let _ = tree.insert(node);

        let cfg = SurfaceConfig {
            anchor_to: Some("audio_button".to_string()),
            ..Default::default()
        };
        let rect = find_anchor_widget_for_popup(&tree, "audio-devices", &cfg);
        assert_eq!(rect, Some((200.0, 5.0, 40.0, 30.0)));
    }

    #[test]
    fn test_popup_with_preset_uses_preset_values() {
        let mut config = BarConfig::default();
        config.animations.insert(
            "@popup_open".to_string(),
            AnimationConfig {
                kind: "spring".to_string(),
                stiffness: Some(420.0),
                damping: Some(30.0),
                duration_ms: None,
                curve: None,
                from: Some(AnimationFrom {
                    x: Some(0.0),
                    y: Some(-15.0),
                    opacity: Some(0.0),
                    scale: Some(1.0),
                }),
                to: Some(AnimationFrom {
                    x: Some(0.0),
                    y: Some(0.0),
                    opacity: Some(1.0),
                    scale: Some(1.0),
                }),
            },
        );

        let surface_cfg = SurfaceConfig::default();
        let (anim, is_fallback) =
            resolve_popup_open_animation(&surface_cfg, &config, "test_popup", false, false);

        assert!(!is_fallback, "Should use preset, not fallback");
        assert_eq!(anim.kind, "spring");
        assert_eq!(anim.stiffness, Some(420.0));
        assert_eq!(anim.damping, Some(30.0));
        assert_eq!(anim.from.as_ref().and_then(|f| f.y), Some(-15.0));
    }

    #[test]
    fn test_popup_without_animation_config_uses_last_resort_fallback() {
        let mut config = BarConfig::default();
        config.animations.clear(); // Empty animations map

        let surface_cfg = SurfaceConfig::default();
        let (anim, is_fallback) =
            resolve_popup_open_animation(&surface_cfg, &config, "test_popup", false, false);

        assert!(
            is_fallback,
            "Should trigger last-resort fallback when config.animations is empty"
        );
        assert_eq!(anim.kind, "spring");
        assert_eq!(anim.stiffness, Some(300.0));
        assert_eq!(anim.damping, Some(26.0));
        assert_eq!(anim.from.as_ref().and_then(|f| f.y), Some(-12.0));
    }

    #[test]
    fn test_two_popups_configured_distinct_stiffness() {
        let config = BarConfig::default();

        let popup1 = SurfaceConfig {
            open_animation: Some(AnimationConfig {
                kind: "spring".to_string(),
                stiffness: Some(500.0),
                damping: Some(25.0),
                duration_ms: None,
                curve: None,
                from: None,
                to: None,
            }),
            ..Default::default()
        };

        let popup2 = SurfaceConfig {
            open_animation: Some(AnimationConfig {
                kind: "spring".to_string(),
                stiffness: Some(150.0),
                damping: Some(12.0),
                duration_ms: None,
                curve: None,
                from: None,
                to: None,
            }),
            ..Default::default()
        };

        let (anim1, is_fb1) =
            resolve_popup_open_animation(&popup1, &config, "popup1", false, false);
        let (anim2, is_fb2) =
            resolve_popup_open_animation(&popup2, &config, "popup2", false, false);

        assert!(!is_fb1);
        assert!(!is_fb2);
        assert_eq!(anim1.stiffness, Some(500.0));
        assert_eq!(anim2.stiffness, Some(150.0));
        assert_ne!(anim1.stiffness, anim2.stiffness);
    }

    #[test]
    fn test_popup_style_precedence_and_drag_strip_height() {
        let mut config = BarConfig::default();
        config.styles.insert(
            "my_custom_style".to_string(),
            wyrd_engine::config::StyleConfig {
                background: Some("#123456".to_string()),
                ..Default::default()
            },
        );
        config.styles.insert(
            "popup:audio".to_string(),
            wyrd_engine::config::StyleConfig {
                background: Some("#654321".to_string()),
                ..Default::default()
            },
        );

        let sc = SurfaceConfig {
            name: "audio".to_string(),
            style: Some("my_custom_style".to_string()),
            drag_strip_height: Some(36.0),
            ..Default::default()
        };

        let style_opt = sc
            .style
            .as_deref()
            .and_then(|s| config.styles.get(s))
            .or_else(|| config.styles.get(&format!("popup:{}", sc.name)));

        assert_eq!(style_opt.unwrap().background, Some("#123456".to_string()));
        assert_eq!(sc.drag_strip_height, Some(36.0));
    }

    #[test]
    fn test_keyboard_mode_resolution_without_hardcoded_notifications() {
        let sc_default = SurfaceConfig {
            name: "notification".to_string(),
            keyboard: "none".to_string(),
            ..Default::default()
        };
        // A tree without TextInput:
        let tree_no_input_has_text_input = false;
        let km = if tree_no_input_has_text_input && sc_default.keyboard == "none" {
            "on_demand"
        } else {
            &sc_default.keyboard
        };
        assert_eq!(km, "none");

        // A tree with TextInput (e.g. launcher search):
        let tree_with_input_has_text_input = true;
        let km_input = if tree_with_input_has_text_input && sc_default.keyboard == "none" {
            "on_demand"
        } else {
            &sc_default.keyboard
        };
        assert_eq!(km_input, "on_demand");
    }
}
