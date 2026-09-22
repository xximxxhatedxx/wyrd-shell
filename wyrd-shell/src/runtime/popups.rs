use crate::compositor::{resolve_cursor_output_id, CompositorIntegration};
use crate::popup::{
    close_popup, module_surface_request, popup_open_for_output, toggle_configured_popup, PopupKey,
};
use crate::runtime::render::{
    apply_surface_configures_and_scales, set_surface_input_region, update_bar_input_region,
    wait_for_surface_configures,
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::WidgetTree;
use wyrd_engine::BarState;

pub fn reconcile_surfaces(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    config: &wyrd_engine::config::BarConfig,
    open_popups: &std::collections::HashSet<PopupKey>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
) -> Result<()> {
    let mut created_surface = false;
    let outputs = state
        .lock()
        .unwrap()
        .outputs
        .values()
        .cloned()
        .collect::<Vec<_>>();
    backend.surface_manager.surfaces.retain_mut(|surface| {
        if surface.dynamic {
            return true;
        }
        if surface.config_name == "__monitor_dimmer__" {
            let keep = surface
                .output
                .as_ref()
                .map(|o| outputs.iter().any(|out| out.output.id() == o.id()))
                .unwrap_or(false);
            if !keep {
                surface_trees.remove(&surface.surface.id());
                if let Some(ls) = &surface.layer_surface {
                    ls.destroy();
                }
                surface.surface.destroy();
            }
            return keep;
        }
        if surface.config_name == "__popup_backdrop__" {
            let keep = !open_popups.is_empty();
            if !keep {
                surface_trees.remove(&surface.surface.id());
                if let Some(ls) = &surface.layer_surface {
                    ls.destroy();
                }
                surface.surface.destroy();
            }
            return keep;
        }
        if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
            if let Some(output_id) = surface.output.as_ref().map(wayland_client::Proxy::id) {
                let keep = open_popups.contains(&(surface.config_name.clone(), output_id));
                if !keep {
                    surface_trees.remove(&surface.surface.id());
                    if let Some(ls) = &surface.layer_surface {
                        ls.destroy();
                    }
                    surface.surface.destroy();
                }
                return keep;
            }
        }
        let keep = config
            .surfaces
            .iter()
            .find(|sc| sc.name == surface.config_name)
            .map(|sc| {
                if !sc.visible && !popup_open_for_output(open_popups, &sc.name) {
                    return false;
                }
                if surface.output.is_none() && !outputs.is_empty() {
                    return false;
                }
                let mut cur_anchors = surface.anchors.clone();
                cur_anchors.sort();
                let mut target_anchors = sc.anchor.clone();
                target_anchors.sort();
                if cur_anchors != target_anchors {
                    return false;
                }
                let target_outputs: Vec<_> = if let Some(target_name) = sc.output.as_deref() {
                    if target_name == "all" {
                        outputs.clone()
                    } else {
                        outputs
                            .iter()
                            .filter(|o| o.name == target_name)
                            .cloned()
                            .collect()
                    }
                } else {
                    outputs.clone()
                };
                if let Some(output) = &surface.output {
                    let out_name = state
                        .lock()
                        .unwrap()
                        .outputs
                        .values()
                        .find(|o| o.output.id() == output.id())
                        .map(|o| o.name.clone());
                    if let Some(name) = out_name {
                        if !target_outputs.iter().any(|o| o.name == name) {
                            return false;
                        }
                    }
                }
                true
            })
            .unwrap_or(false);

        if !keep {
            surface_trees.remove(&surface.surface.id());
            if let Some(ls) = &surface.layer_surface {
                ls.destroy();
            }
            surface.surface.destroy();
        }
        keep
    });

    for surface_config in &config.surfaces {
        if surface_config.ty == "popup" || !surface_config.visible {
            continue;
        }
        let surface_type = match surface_config.ty.as_str() {
            "bar" => wyrd_engine::wayland::surface::SurfaceType::Bar,
            "panel" => wyrd_engine::wayland::surface::SurfaceType::Panel,
            "popup" => wyrd_engine::wayland::surface::SurfaceType::Popup,
            _ => wyrd_engine::wayland::surface::SurfaceType::Background,
        };
        let width = surface_config.width.unwrap_or(0);
        let height = surface_config.height;
        let target_outputs: Vec<_> = if let Some(output_name) = surface_config.output.as_deref() {
            if output_name == "all" {
                outputs.clone()
            } else {
                outputs
                    .iter()
                    .filter(|o| o.name == output_name)
                    .cloned()
                    .collect()
            }
        } else {
            outputs.clone()
        };

        let is_bar_config = surface_type == wyrd_engine::wayland::surface::SurfaceType::Bar;
        let initial_margin = if is_bar_config {
            (0, 0, 0, 0)
        } else {
            (
                surface_config.margin.top,
                surface_config.margin.right,
                surface_config.margin.bottom,
                surface_config.margin.left,
            )
        };
        let initial_exclusive = if is_bar_config {
            surface_config.margin.top
                + if surface_config.height > 0 {
                    surface_config.height as i32
                } else {
                    38
                }
        } else {
            surface_config.exclusive_zone
        };
        let initial_height = if is_bar_config {
            (surface_config.margin.top as u32)
                + (if surface_config.height > 0 {
                    surface_config.height
                } else {
                    38
                })
                + 60
        } else {
            height
        };

        if target_outputs.is_empty() {
            let exists =
                backend.surface_manager.surfaces.iter().any(|s| {
                    !s.dynamic && s.config_name == surface_config.name && s.output.is_none()
                });
            if !exists {
                backend.surface_manager.create_surface(
                    &backend.compositor,
                    &backend.qh,
                    surface_type,
                    &surface_config.name,
                    false,
                    None,
                    width,
                    initial_height,
                    &surface_config.layer,
                    &surface_config.anchor,
                    initial_margin,
                    initial_exclusive,
                    &surface_config.keyboard,
                    1.0,
                );
                created_surface = true;
            }
        } else {
            for output in target_outputs {
                let exists = backend.surface_manager.surfaces.iter().any(|s| {
                    !s.dynamic
                        && s.config_name == surface_config.name
                        && s.output.as_ref().map(Proxy::id) == Some(output.output.id())
                });
                if !exists {
                    backend.surface_manager.create_surface(
                        &backend.compositor,
                        &backend.qh,
                        surface_type,
                        &surface_config.name,
                        false,
                        Some(&output.output),
                        width,
                        initial_height,
                        &surface_config.layer,
                        &surface_config.anchor,
                        initial_margin,
                        initial_exclusive,
                        &surface_config.keyboard,
                        output.scale as f64,
                    );
                    created_surface = true;
                }
            }
        }
    }

    // Create monitor dimmer surface for displays without physical backlight
    let has_hw_backlight = std::path::Path::new("/sys/class/backlight")
        .read_dir()
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);

    for output in &outputs {
        let is_internal = output.name.starts_with("eDP")
            || output.name.starts_with("LVDS")
            || output.name.starts_with("DSI");
        let needs_dimmer = !is_internal || !has_hw_backlight;
        if !needs_dimmer {
            continue;
        }

        let exists = backend.surface_manager.surfaces.iter().any(|s| {
            s.config_name == "__monitor_dimmer__"
                && s.output.as_ref().map(Proxy::id) == Some(output.output.id())
        });
        if !exists {
            backend.surface_manager.create_surface(
                &backend.compositor,
                &backend.qh,
                wyrd_engine::wayland::surface::SurfaceType::Background,
                "__monitor_dimmer__",
                false,
                Some(&output.output),
                0,
                0,
                "overlay",
                &[
                    "top".to_string(),
                    "bottom".to_string(),
                    "left".to_string(),
                    "right".to_string(),
                ],
                (0, 0, 0, 0),
                -1,
                "none",
                output.scale as f64,
            );
            if let Some(new_surf) = backend.surface_manager.surfaces.last() {
                let empty_region = backend.compositor.create_region(&backend.qh, ());
                new_surf.surface.set_input_region(Some(&empty_region));
                empty_region.destroy();
                new_surf.surface.commit();
            }
            created_surface = true;
        }
    }

    if created_surface {
        wait_for_surface_configures(backend, state)?;
    }
    apply_surface_configures_and_scales(backend, state);

    let data_store = state
        .lock()
        .unwrap()
        .module_data
        .try_read()
        .map(|s| s.clone())
        .unwrap_or_default();
    for surface in &mut backend.surface_manager.surfaces {
        if surface.dynamic
            || surface.config_name == "__popup_backdrop__"
            || surface.config_name == "__monitor_dimmer__"
            || surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup
        {
            continue;
        }
        if let Some(surface_config) = config
            .surfaces
            .iter()
            .find(|s| s.name == surface.config_name)
        {
            let is_vert = surface_config.anchor.iter().any(|a| a == "top")
                && surface_config.anchor.iter().any(|a| a == "bottom");
            let is_horiz = surface_config.anchor.iter().any(|a| a == "left")
                && surface_config.anchor.iter().any(|a| a == "right");
            let out_opt = surface
                .output
                .as_ref()
                .and_then(|output| outputs.iter().find(|o| o.output.id() == output.id()));
            let output_h = out_opt
                .and_then(|o| {
                    if o.logical_height > 0 {
                        Some(o.logical_height as u32)
                    } else {
                        o.current_mode.map(|(_, h)| (h / o.scale.max(1)) as u32)
                    }
                })
                .unwrap_or(1080);
            let output_w = out_opt
                .and_then(|o| {
                    if o.logical_width > 0 {
                        Some(o.logical_width as u32)
                    } else {
                        o.current_mode.map(|(w, _)| (w / o.scale.max(1)) as u32)
                    }
                })
                .unwrap_or(1920);

            let is_bar = surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar;
            let margin_tuple = if is_bar {
                (0, 0, 0, 0)
            } else {
                (
                    surface_config.margin.top,
                    surface_config.margin.right,
                    surface_config.margin.bottom,
                    surface_config.margin.left,
                )
            };
            if surface.margin != margin_tuple {
                surface.margin = margin_tuple;
                if let Some(ls) = &surface.layer_surface {
                    ls.set_margin(
                        margin_tuple.0,
                        margin_tuple.1,
                        margin_tuple.2,
                        margin_tuple.3,
                    );
                }
                surface.dirty = true;
            }
            if is_vert && (surface.height == 0 || surface_config.height == 0) {
                let avail_h = output_h.saturating_sub(
                    (surface_config.margin.top + surface_config.margin.bottom) as u32,
                );
                if avail_h > 0 {
                    surface.height = avail_h;
                }
            }
            if is_bar {
                if output_w > 0 && surface_config.width.is_none() {
                    surface.width = output_w;
                }
            } else if is_horiz && (surface.width == 0 && surface_config.width.is_none()) {
                let avail_w = output_w.saturating_sub(
                    (surface_config.margin.left + surface_config.margin.right) as u32,
                );
                if avail_w > 0 {
                    surface.width = avail_w;
                }
            }
            let mut tree = wyrd_engine::widgets::from_config_with_store(
                &surface_config.widgets,
                &config.styles,
                &data_store,
            );
            let scale = surface.scale.max(1.0) as f32;
            let (_sl, _sr, st, sb) = if is_bar {
                tree.shadow_extents(scale)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            let nominal_w = surface_config.width.unwrap_or(0);
            let nominal_h = if surface_config.height > 0 {
                surface_config.height
            } else {
                0
            };

            if is_bar {
                let geom =
                    compute_bar_geometry(surface_config, output_w, nominal_h, (_sl, _sr, st, sb));
                surface.height = geom.surface_h;
                surface.shadow_insets = geom.shadow_insets;

                // Wayland layer surface covers full width starting from screen edge (margin = 0)
                // so shadow can naturally extend all the way to the screen borders without clipping
                let surface_margin = (0, 0, 0, 0);
                if surface.margin != surface_margin {
                    surface.margin = surface_margin;
                    if let Some(ls) = &surface.layer_surface {
                        ls.set_margin(0, 0, 0, 0);
                    }
                }

                if let Some(ls) = &surface.layer_surface {
                    ls.set_size(nominal_w, geom.surface_h);
                    ls.set_exclusive_zone(geom.exclusive_zone);
                    surface.surface.commit();
                }

                wyrd_engine::widgets::tree::measure_tree(
                    &mut tree,
                    render_ctx,
                    geom.content_w,
                    geom.content_h,
                );
                wyrd_engine::widgets::tree::layout_tree(
                    &mut tree,
                    geom.bar_x,
                    geom.bar_y,
                    geom.content_w,
                    geom.content_h,
                );
                update_bar_input_region(&backend.compositor, &backend.qh, surface);
            } else {
                let width = nominal_w;
                let height = nominal_h;
                if let Some(ls) = &surface.layer_surface {
                    ls.set_size(width, height);
                    surface.surface.commit();
                }

                surface.shadow_insets = (0.0, 0.0, 0.0, 0.0);
                let content_width = surface.width.max(1) as f32;
                let content_height = surface.height.max(1) as f32;
                wyrd_engine::widgets::tree::measure_tree(
                    &mut tree,
                    render_ctx,
                    content_width,
                    content_height,
                );
                wyrd_engine::widgets::tree::layout_tree(
                    &mut tree,
                    0.0,
                    0.0,
                    content_width,
                    content_height,
                );
                set_surface_input_region(
                    &backend.compositor,
                    &backend.qh,
                    surface,
                    0,
                    0,
                    surface.width,
                    surface.height,
                );
            }

            if let std::collections::hash_map::Entry::Vacant(e) =
                surface_trees.entry(surface.surface.id())
            {
                e.insert(Arc::new(tree));
                surface.dirty = true;
            } else if let Some(existing) = surface_trees.get_mut(&surface.surface.id()) {
                *existing = Arc::new(tree);
                surface.dirty = true;
            }
        }
    }

    for (surface_id, tree) in surface_trees.iter_mut() {
        if let Some(surface) = backend
            .surface_manager
            .surfaces
            .iter()
            .find(|s| s.surface.id() == *surface_id)
        {
            if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
                if let Some(surface_config) = config
                    .surfaces
                    .iter()
                    .find(|sc| sc.name == surface.config_name)
                {
                    if !surface_config.widgets.is_empty() {
                        let candidate = wyrd_engine::widgets::from_popup_config_with_store(
                            &surface_config.widgets,
                            &config.styles,
                            &data_store,
                        );
                        let diff = wyrd_engine::widgets::diff_trees(tree, &candidate);
                        let mut mut_tree = (**tree).clone();
                        mut_tree.apply_diff(&candidate, &diff);
                        let (shadow_left, shadow_right, shadow_top, shadow_bottom) =
                            mut_tree.shadow_extents(surface.scale.max(1.0) as f32);
                        let content_width =
                            (surface.width as f32 - shadow_left - shadow_right).max(1.0);
                        let content_height =
                            (surface.height as f32 - shadow_top - shadow_bottom).max(1.0);
                        wyrd_engine::widgets::tree::measure_tree(
                            &mut mut_tree,
                            render_ctx,
                            content_width,
                            content_height,
                        );
                        wyrd_engine::widgets::tree::layout_tree(
                            &mut mut_tree,
                            shadow_left,
                            shadow_top,
                            content_width,
                            content_height,
                        );
                        *tree = Arc::new(mut_tree);
                    } else {
                        let mut mut_tree = (**tree).clone();
                        mut_tree.styles = Arc::new(config.styles.clone());
                        *tree = Arc::new(mut_tree);
                    }
                } else {
                    let mut mut_tree = (**tree).clone();
                    mut_tree.styles = Arc::new(config.styles.clone());
                    *tree = Arc::new(mut_tree);
                }
            }
        }
    }

    if let Ok(mut lock) = state.lock().unwrap().surface_trees.try_write() {
        *lock = surface_trees.clone();
    }
    state.lock().unwrap().frame_ready = true;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_module_surface_message(
    module: &str,
    action: &str,
    params: &serde_json::Value,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<wyrd_engine::modules::CoreMessage>>,
    compositor: &dyn CompositorIntegration,
    surface_timeout_tx: &tokio::sync::mpsc::Sender<(String, wyrd_engine::modules::ModuleMessage)>,
) -> Result<()> {
    let target_name = params
        .get("id")
        .or_else(|| params.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(module);

    if matches!(action, "close" | "destroy") {
        close_popup(backend, state, open_popups, surface_trees, target_name);
        let _ = module_surface_request(compositor, backend, state, module, action, params);
    } else if matches!(action, "open" | "toggle") {
        let config_arc = state.lock().unwrap().config.clone();
        let current_config = config_arc.read().await.clone();
        let is_configured_popup = current_config
            .surfaces
            .iter()
            .any(|s| s.name == target_name && s.ty == "popup");
        if is_configured_popup {
            let explicit_out = params.get("output").and_then(serde_json::Value::as_str);
            let active_out_id = resolve_cursor_output_id(compositor, state, explicit_out)
                .or_else(|| {
                    backend
                        .surface_manager
                        .surfaces
                        .iter()
                        .find(|s| s.ty == wyrd_engine::wayland::surface::SurfaceType::Bar)
                        .and_then(|s| s.output.as_ref().map(Proxy::id))
                })
                .unwrap_or_else(|| backend.compositor.id());

            let current_out_id = open_popups
                .iter()
                .find(|(name, _)| name == target_name)
                .map(|(_, out_id)| out_id.clone());

            if let Some(old_out_id) = current_out_id {
                if old_out_id != active_out_id {
                    close_popup(backend, state, open_popups, surface_trees, target_name);
                }
            }

            let is_open = popup_open_for_output(open_popups, target_name);
            if is_open && action == "open" {
                close_popup(backend, state, open_popups, surface_trees, target_name);
            }
            let is_open = popup_open_for_output(open_popups, target_name);
            if !is_open || action == "toggle" {
                let _ = toggle_configured_popup(
                    compositor,
                    backend,
                    state,
                    &current_config,
                    open_popups,
                    surface_trees,
                    target_name,
                    active_out_id,
                    render_ctx,
                    module_commands,
                    Some(params),
                );
            }
        } else {
            let _ = module_surface_request(compositor, backend, state, module, action, params);
            let _ = reconcile_surfaces(
                backend,
                state,
                &current_config,
                open_popups,
                surface_trees,
                render_ctx,
            );
        }
    } else {
        let config_arc = state.lock().unwrap().config.clone();
        let current_config = config_arc.read().await.clone();
        let _ = module_surface_request(compositor, backend, state, module, action, params);
        let _ = reconcile_surfaces(
            backend,
            state,
            &current_config,
            open_popups,
            surface_trees,
            render_ctx,
        );
    }
    if action == "open" {
        if let Some(timeout_ms) = params.get("timeout_ms").and_then(serde_json::Value::as_u64) {
            if timeout_ms > 0 {
                let tx = surface_timeout_tx.clone();
                let mod_name = module.to_string();
                let target = target_name.to_string();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(timeout_ms)).await;
                    let _ = tx
                        .send((
                            mod_name,
                            wyrd_engine::modules::ModuleMessage::Surface {
                                action: "close".to_string(),
                                params: serde_json::json!({ "name": target }),
                            },
                        ))
                        .await;
                });
            }
        }
    }
    for surface in &mut backend.surface_manager.surfaces {
        surface.dirty = true;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct BarGeometry {
    pub bar_x: f32,
    pub bar_y: f32,
    pub content_w: f32,
    pub content_h: f32,
    pub surface_w: u32,
    pub surface_h: u32,
    pub shadow_insets: (f32, f32, f32, f32),
    pub exclusive_zone: i32,
}

pub fn compute_bar_geometry(
    surface_config: &wyrd_engine::config::SurfaceConfig,
    output_w: u32,
    nominal_h: u32,
    shadow_extents: (f32, f32, f32, f32),
) -> BarGeometry {
    let (_sl, _sr, st, sb) = shadow_extents;
    let is_top = surface_config.anchor.iter().any(|a| a == "top");
    let is_bottom = surface_config.anchor.iter().any(|a| a == "bottom");
    let content_h = if surface_config.height > 0 {
        surface_config.height as f32
    } else if nominal_h > 0 {
        nominal_h as f32
    } else {
        38.0
    };

    let bar_x = surface_config.margin.left as f32;
    let bar_y = surface_config.margin.top as f32;
    let bar_margin_r = surface_config.margin.right as f32;
    let bar_margin_b = surface_config.margin.bottom as f32;

    let (surface_h, insets) = if is_top {
        let s_h = (bar_y + content_h + sb).ceil() as u32;
        (s_h, (bar_x, bar_margin_r, bar_y, sb))
    } else if is_bottom {
        let s_h = (bar_margin_b + content_h + st).ceil() as u32;
        (s_h, (bar_x, bar_margin_r, st, bar_margin_b))
    } else {
        let s_h = (content_h + st + sb).ceil() as u32;
        (s_h, (bar_x, bar_margin_r, st, sb))
    };

    let exclusive_zone = if is_top {
        (bar_y + content_h) as i32
    } else if is_bottom {
        (bar_margin_b + content_h) as i32
    } else {
        surface_config.exclusive_zone
    };

    let content_w = (output_w as f32 - bar_x - bar_margin_r).max(1.0);

    BarGeometry {
        bar_x,
        bar_y,
        content_w,
        content_h,
        surface_w: output_w,
        surface_h,
        shadow_insets: insets,
        exclusive_zone,
    }
}
