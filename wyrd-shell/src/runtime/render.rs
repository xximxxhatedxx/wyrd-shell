use crate::interaction::DragState;
use anyhow::Result;
use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::render::damage::DamageTracker;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::WidgetTree;
use wyrd_engine::BarState;

#[cfg(feature = "systemd")]
pub fn notify_ready() {
    if let Err(error) = libsystemd::daemon::notify(false, &[libsystemd::daemon::NotifyState::Ready])
    {
        log::warn!("systemd readiness notification failed: {}", error);
    }
}

#[cfg(not(feature = "systemd"))]
pub fn notify_ready() {}

pub fn set_surface_input_region(
    compositor: &wayland_client::protocol::wl_compositor::WlCompositor,
    qh: &wayland_client::QueueHandle<BarState>,
    surface: &wyrd_engine::wayland::surface::ShellSurface,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    let region = compositor.create_region(qh, ());
    region.add(x, y, width as i32, height as i32);
    surface.surface.set_input_region(Some(&region));
    region.destroy();
}

pub fn update_bar_input_region(
    compositor: &wayland_client::protocol::wl_compositor::WlCompositor,
    qh: &wayland_client::QueueHandle<BarState>,
    surface: &wyrd_engine::wayland::surface::ShellSurface,
) {
    let bar_x = surface.shadow_insets.0 as i32;
    let bar_y = surface.shadow_insets.2 as i32;
    let bar_w =
        (surface.width as f32 - surface.shadow_insets.0 - surface.shadow_insets.1).max(1.0) as u32;
    let bar_h =
        (surface.height as f32 - surface.shadow_insets.2 - surface.shadow_insets.3).max(1.0) as u32;
    set_surface_input_region(compositor, qh, surface, bar_x, bar_y, bar_w, bar_h);
    surface.surface.commit();
}

pub fn apply_surface_configures_and_scales(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
) {
    let (fractional_scales, layer_sizes) = {
        let s = state.lock().unwrap();
        (s.fractional_scales.clone(), s.layer_surface_sizes.clone())
    };

    let compositor = &backend.compositor;
    let qh = &backend.qh;
    let mut any_dirty = false;
    for surface in &mut backend.surface_manager.surfaces {
        if let Some(scale) = fractional_scales.get(&surface.surface.id()) {
            if (surface.scale - *scale).abs() > 0.001 {
                surface.scale = *scale;
                surface.dirty = true;
                any_dirty = true;
            }
        }
        if let Some(layer_surface) = &surface.layer_surface {
            if let Some(&(w, h)) = layer_sizes.get(&layer_surface.id()) {
                let size_changed = (w > 0 && surface.width != w) || (h > 0 && surface.height != h);
                if size_changed {
                    info!(
                        "Surface {} reconfigured by compositor: {}x{} (was: {}x{})",
                        surface.config_name, w, h, surface.width, surface.height
                    );
                }
                if w > 0 && surface.width != w {
                    surface.width = w;
                    surface.dirty = true;
                    any_dirty = true;
                }
                if h > 0 && surface.height != h {
                    surface.height = h;
                    surface.dirty = true;
                    any_dirty = true;
                }
                if size_changed && surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                    update_bar_input_region(compositor, qh, surface);
                }
                if size_changed && surface.config_name == "__monitor_dimmer__" {
                    let empty_region = compositor.create_region(qh, ());
                    surface.surface.set_input_region(Some(&empty_region));
                    empty_region.destroy();
                }
            }
        }
    }
    if any_dirty {
        state.lock().unwrap().frame_ready = true;
    }
}

pub fn wait_for_surface_configures(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
) -> Result<()> {
    backend.event_queue.roundtrip(&mut *state.lock().unwrap())?;
    Ok(())
}

pub fn trim_process_memory() {
    #[cfg(target_os = "linux")]
    // SAFETY: `libc::malloc_trim(0)` is a glibc extension that releases free memory back to the OS.
    // Passing 0 safely frees as much unused heap memory as possible without altering allocator state invariants.
    unsafe {
        libc::malloc_trim(0);
    }
}

pub fn is_connection_closed(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
            );
        }
        false
    })
}

pub fn tick_animations(
    backend: &mut WaylandBackend,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
    dragging_popup: &Option<DragState>,
) -> bool {
    if !render_ctx.animator.tick(std::time::Instant::now()) {
        return false;
    }
    let mut is_animating = false;
    for surface in &mut backend.surface_manager.surfaces {
        let anim_key = format!("popup_{:?}", surface.surface.id());
        let offset_y_key = format!("popup_y_{:?}", surface.surface.id());
        let offset_x_key = format!("popup_x_{:?}", surface.surface.id());
        let alpha_opt = render_ctx.animator.get_named(&anim_key);
        let offset_y_opt = render_ctx.animator.get_named(&offset_y_key);
        let offset_x_opt = render_ctx.animator.get_named(&offset_x_key);
        if alpha_opt.is_some() || offset_y_opt.is_some() || offset_x_opt.is_some() {
            if let Some(tree) = surface_trees.get_mut(&surface.surface.id()) {
                let is_dragging = dragging_popup
                    .as_ref()
                    .map(|d| d.surface_id == surface.surface.id())
                    .unwrap_or(false);
                let mut_tree = Arc::make_mut(tree);
                if let Some(root_id) = mut_tree.root() {
                    if let Some(alpha) = alpha_opt {
                        if let Some(node) = mut_tree.get_mut(root_id) {
                            node.style.opacity = (alpha as f32).clamp(0.0, 1.0);
                        }
                    }
                    if !is_dragging {
                        if let (Some(oy), Some(ox)) = (offset_y_opt, offset_x_opt) {
                            let (top, right, bottom, left) = surface.margin;
                            let target_top = top + oy as i32;
                            let target_left = left + ox as i32;
                            if let Some(ls) = &surface.layer_surface {
                                ls.set_margin(target_top, right, bottom, target_left);
                            }
                        }
                    }
                }
            }
            surface.dirty = true;
            is_animating = true;
        }
    }
    is_animating
}

pub fn render_frame(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
    damage_trackers: &mut HashMap<ObjectId, DamageTracker>,
) -> Result<bool> {
    let (hovered_widget, focused_widget, focused_surface_id) = {
        let input_snapshot = state.lock().unwrap().input_state.clone();
        input_snapshot
            .try_read()
            .map(|inp| {
                (
                    inp.pointer.hovered_widget,
                    inp.focus.current_focus(),
                    inp.focused_surface_id.clone(),
                )
            })
            .unwrap_or((None, None, None))
    };

    let mut rendered_surfaces = Vec::new();
    for (idx, surface) in backend.surface_manager.surfaces.iter_mut().enumerate() {
        if !surface.dirty {
            continue;
        }
        surface.dirty = false;
        let surface_hovered_widget = state
            .lock()
            .unwrap()
            .pointer_surface_id
            .clone()
            .filter(|pointer_surface_id| *pointer_surface_id == surface.surface.id())
            .and(hovered_widget);

        if surface.config_name == "__popup_backdrop__" {
            let Some(mut pixmap) =
                tiny_skia::Pixmap::new(surface.width.max(1), surface.height.max(1))
                    .or_else(|| tiny_skia::Pixmap::new(1, 1))
            else {
                continue;
            };
            let backdrop_color = {
                let cfg_lock = state.lock().unwrap().config.clone();
                cfg_lock
                    .try_read()
                    .ok()
                    .and_then(|c| c.styles.get("backdrop").cloned())
                    .and_then(|s| s.background)
                    .and_then(|bg| wyrd_engine::widgets::parse_color(&bg))
                    .unwrap_or(tiny_skia::Color::from_rgba8(0, 0, 0, 80))
            };
            pixmap.fill(backdrop_color);
            let tracker = damage_trackers.entry(surface.surface.id()).or_default();
            rendered_surfaces.push((idx, pixmap, tracker.clone()));
            continue;
        }

        if surface.config_name == "__monitor_dimmer__" {
            let percent = {
                let store_lock = state.lock().unwrap().module_data.clone();
                store_lock
                    .try_read()
                    .ok()
                    .and_then(|store| {
                        store.get("brightness").and_then(|b| {
                            b.get("percent")
                                .or_else(|| b.get("value"))
                                .and_then(|v| v.as_u64())
                        })
                    })
                    .unwrap_or(100) as u8
            };

            let alpha = if percent >= 100 {
                0
            } else {
                (((100 - percent) as f32 / 100.0) * 0.85 * 255.0).round() as u8
            };

            let Some(mut pixmap) =
                tiny_skia::Pixmap::new(surface.width.max(1), surface.height.max(1))
                    .or_else(|| tiny_skia::Pixmap::new(1, 1))
            else {
                continue;
            };

            if alpha > 0 {
                pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, alpha));
            } else {
                pixmap.fill(tiny_skia::Color::TRANSPARENT);
            }

            let empty_region = backend.compositor.create_region(&backend.qh, ());
            surface.surface.set_input_region(Some(&empty_region));
            empty_region.destroy();

            let tracker = damage_trackers.entry(surface.surface.id()).or_default();
            rendered_surfaces.push((idx, pixmap, tracker.clone()));
            continue;
        }

        if let Some(tree_arc) = surface_trees.get_mut(&surface.surface.id()) {
            if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                let bar_x = surface.shadow_insets.0;
                let bar_y = surface.shadow_insets.2;
                let content_w =
                    (surface.width as f32 - surface.shadow_insets.0 - surface.shadow_insets.1)
                        .max(1.0);
                let nominal_h = state
                    .lock()
                    .unwrap()
                    .config
                    .try_read()
                    .ok()
                    .and_then(|c| {
                        c.surfaces
                            .iter()
                            .find(|s| s.name == surface.config_name)
                            .map(|s| s.height)
                    })
                    .unwrap_or(0);
                let content_h = if nominal_h > 0 {
                    nominal_h as f32
                } else {
                    (surface.height as f32 - surface.shadow_insets.2 - surface.shadow_insets.3)
                        .max(1.0)
                };
                let mut_tree = Arc::make_mut(tree_arc);
                wyrd_engine::widgets::tree::measure_tree(
                    mut_tree, render_ctx, content_w, content_h,
                );
                wyrd_engine::widgets::tree::layout_tree(
                    mut_tree, bar_x, bar_y, content_w, content_h,
                );
                update_bar_input_region(&backend.compositor, &backend.qh, surface);
            }
            let needs_layout = (surface.ty != wyrd_engine::wayland::surface::SurfaceType::Bar
                && tree_arc.is_dirty())
                || tree_arc
                    .root()
                    .and_then(|r| tree_arc.get(r))
                    .is_some_and(|n| n.final_rect.2 <= 0.0 || n.final_rect.3 <= 0.0);
            if needs_layout {
                let mut mut_tree = (**tree_arc).clone();
                let shadow_left = surface.shadow_insets.0;
                let shadow_top = surface.shadow_insets.2;
                let content_w =
                    (surface.width as f32 - shadow_left - surface.shadow_insets.1).max(1.0);
                let content_h =
                    (surface.height as f32 - shadow_top - surface.shadow_insets.3).max(1.0);
                wyrd_engine::widgets::tree::measure_tree(
                    &mut mut_tree,
                    render_ctx,
                    content_w,
                    content_h,
                );
                wyrd_engine::widgets::tree::layout_tree(
                    &mut mut_tree,
                    shadow_left,
                    shadow_top,
                    content_w,
                    content_h,
                );
                *tree_arc = Arc::new(mut_tree);
            }
            let tracker = damage_trackers.entry(surface.surface.id()).or_default();
            let anim_key = format!("popup_{:?}", surface.surface.id());
            let anim_progress = render_ctx.animator.get_named(&anim_key).unwrap_or(1.0);
            if let Some(rendered) = wyrd_engine::render::render_surface(
                idx,
                surface.width,
                surface.height,
                surface.scale,
                true,
                tree_arc.as_ref(),
                render_ctx,
                tracker,
                surface_hovered_widget,
                if focused_surface_id.as_ref() == Some(&surface.surface.id()) {
                    focused_widget
                } else {
                    None
                },
                anim_progress,
            ) {
                rendered_surfaces.push(rendered);
            }
        }
    }

    if let Ok(mut lock) = state.lock().unwrap().surface_trees.try_write() {
        *lock = surface_trees.clone();
    }

    if !rendered_surfaces.is_empty() {
        let mut shell_state = state.lock().unwrap();
        if let Err(e) = backend.surface_manager.attach_pixmap_buffers(
            &backend.shm,
            &backend.qh,
            &rendered_surfaces,
            &mut shell_state.released_buffers,
        ) {
            if is_connection_closed(&e) {
                info!("Wayland connection closed while submitting a frame");
                return Ok(false);
            }
            error!("Failed to submit buffer: {}", e);
        }
    }
    let _ = backend.conn.flush();
    Ok(true)
}
