use crate::popup::{apply_tree_layout_with_dynamic_height, close_popup, PopupKey};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::Proxy;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::BarState;

#[derive(Clone, Debug)]
pub struct HoverTooltipState {
    pub widget: wyrd_engine::widgets::WidgetId,
    pub surface_id: wayland_client::backend::ObjectId,
    pub text: String,
    pub rect: (f32, f32, f32, f32),
    pub hover_start: std::time::Instant,
    pub is_shown: bool,
}

pub type TooltipTarget = (wyrd_engine::widgets::WidgetId, String, (f32, f32, f32, f32));

pub fn show_tooltip_surface(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    surface_trees: &mut HashMap<
        wayland_client::backend::ObjectId,
        Arc<wyrd_engine::widgets::WidgetTree>,
    >,
    render_ctx: &mut RenderContext,
    tooltip: &HoverTooltipState,
) {
    close_popup(backend, state, open_popups, surface_trees, "__tooltip__");

    let (text_w, text_h) = wyrd_engine::render::text::TextRenderer::measure(
        render_ctx,
        &tooltip.text,
        "sans-serif",
        12.0,
    );
    let pad_h = 16.0;
    let pad_v = 8.0;
    let content_w = (text_w + pad_h).ceil().max(32.0);
    let content_h = (text_h + pad_v).ceil().max(20.0);

    let mut tree = wyrd_engine::widgets::WidgetTree::new();
    let mut root_node =
        wyrd_engine::widgets::WidgetNode::new(wyrd_engine::widgets::WidgetContent::Container);
    root_node.id = Some("__tooltip_root__".to_string());
    root_node.layout.mode = wyrd_engine::widgets::layout::LayoutMode::Flex(
        wyrd_engine::widgets::layout::FlexDirection::Horizontal,
    );
    root_node.layout.justify = wyrd_engine::widgets::layout::JustifyContent::Center;
    root_node.layout.align = wyrd_engine::widgets::layout::Align::Center;
    root_node.layout.padding = (4.0, 8.0, 4.0, 8.0);
    root_node.layout.fixed_width = Some(content_w);
    root_node.layout.fixed_height = Some(content_h);
    root_node.style.background = Some(wyrd_engine::render::scene::Fill::Solid(
        tiny_skia::Color::from_rgba8(15, 17, 23, 245),
    ));
    root_node.style.radius = 6.0;
    root_node.style.outline_color = Some(tiny_skia::Color::from_rgba8(255, 255, 255, 28));
    root_node.style.outline_width = 1.0;
    root_node.style.shadow = Some((10.0, 0.45, 0.0, 3.0));

    let mut text_node =
        wyrd_engine::widgets::WidgetNode::new(wyrd_engine::widgets::WidgetContent::Text {
            text: tooltip.text.clone(),
        });
    text_node.style.foreground = Some(tiny_skia::Color::from_rgba8(235, 240, 250, 255));
    text_node.style.font_size = 12.0;

    let r_id = tree.insert(root_node);
    let t_id = tree.insert(text_node);
    tree.append_child(r_id, t_id);

    let parent_surface_opt = backend
        .surface_manager
        .surfaces
        .iter()
        .find(|s| s.surface.id() == tooltip.surface_id)
        .or_else(|| {
            backend
                .surface_manager
                .surfaces
                .iter()
                .find(|s| s.ty == wyrd_engine::wayland::surface::SurfaceType::Bar)
        });

    let is_bottom = parent_surface_opt.is_some_and(|s| s.anchors.iter().any(|a| a == "bottom"));
    let parent_margin_left = parent_surface_opt.map(|s| s.margin.3).unwrap_or(16);
    let widget_center_x = parent_margin_left as f32 + tooltip.rect.0 + (tooltip.rect.2 / 2.0);

    let output_id = parent_surface_opt
        .and_then(|s| s.output.as_ref().map(Proxy::id))
        .or_else(|| {
            state
                .lock()
                .unwrap()
                .outputs
                .values()
                .next()
                .map(|o| o.output.id())
        })
        .unwrap_or_else(|| backend.compositor.id());

    let (output_w, _) = {
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

    let (shadow_left, shadow_right, shadow_top, shadow_bottom) = tree.shadow_extents(1.0);
    let surf_w = (content_w + shadow_left + shadow_right).ceil() as u32;
    let surf_h = (content_h + shadow_top + shadow_bottom).ceil() as u32;

    let ideal_left = widget_center_x - (surf_w as f32 / 2.0);
    let clamped_left = ideal_left.clamp(8.0, ((output_w as f32) - (surf_w as f32) - 8.0).max(8.0));

    let (anchors, margin) = if is_bottom {
        let bar_margin_bottom = parent_surface_opt.map(|s| s.margin.2).unwrap_or(0);
        let bar_h = parent_surface_opt.map(|s| s.height as i32).unwrap_or(0);
        let v_margin = bar_margin_bottom + bar_h + 4;
        (
            vec!["bottom".to_string(), "left".to_string()],
            (0, 0, v_margin, clamped_left as i32),
        )
    } else {
        let parent_margin_top = parent_surface_opt.map(|s| s.margin.0).unwrap_or(0);
        let v_margin = if parent_surface_opt
            .is_some_and(|s| s.ty == wyrd_engine::wayland::surface::SurfaceType::Bar)
        {
            let bar_h = parent_surface_opt.map(|s| s.height as i32).unwrap_or(0);
            parent_margin_top + bar_h + 4
        } else {
            parent_margin_top + (tooltip.rect.1 + tooltip.rect.3) as i32 + 4
        };
        (
            vec!["top".to_string(), "left".to_string()],
            (v_margin, 0, 0, clamped_left as i32),
        )
    };

    let target_out = backend
        .surface_manager
        .surfaces
        .iter()
        .find(|s| s.surface.id() == tooltip.surface_id)
        .and_then(|s| s.output.clone());

    backend.surface_manager.create_surface(
        &backend.compositor,
        &backend.qh,
        wyrd_engine::wayland::surface::SurfaceType::Popup,
        "__tooltip__",
        false,
        target_out.as_ref(),
        surf_w,
        surf_h,
        "overlay",
        &anchors,
        margin,
        0,
        "none",
        1.0,
    );

    let _ = backend.event_queue.roundtrip(&mut *state.lock().unwrap());

    if let Some(surf) = backend
        .surface_manager
        .surfaces
        .iter_mut()
        .find(|s| s.config_name == "__tooltip__")
    {
        surf.shadow_insets = (shadow_left, shadow_right, shadow_top, shadow_bottom);
        apply_tree_layout_with_dynamic_height(surf, &mut tree, surface_trees, render_ctx);
        let empty_region = backend.compositor.create_region(&backend.qh, ());
        surf.surface.set_input_region(Some(&empty_region));
        empty_region.destroy();
        surf.surface.commit();
        surf.dirty = true;
    }
    open_popups.insert(("__tooltip__".to_string(), output_id));
    state.lock().unwrap().frame_ready = true;
    let _ = backend.conn.flush();
}

#[allow(dead_code)]
pub fn close_tooltip(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    surface_trees: &mut HashMap<
        wayland_client::backend::ObjectId,
        Arc<wyrd_engine::widgets::WidgetTree>,
    >,
) {
    close_popup(backend, state, open_popups, surface_trees, "__tooltip__");
}

pub fn handle_tooltip_pointer_move(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<
        wayland_client::backend::ObjectId,
        Arc<wyrd_engine::widgets::WidgetTree>,
    >,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    hover_tooltip: &mut Option<HoverTooltipState>,
    found_hover: Option<wyrd_engine::widgets::WidgetId>,
) {
    let mut hovered_tooltip: Option<TooltipTarget> = None;
    if let Some(surf_id) = &state.lock().unwrap().pointer_surface_id {
        let is_special = backend.surface_manager.surfaces.iter().any(|s| {
            &s.surface.id() == surf_id
                && (s.config_name == "__tooltip__"
                    || s.config_name == "__popup_backdrop__"
                    || s.config_name == "__monitor_dimmer__")
        });
        if !is_special {
            if let Some(tree) = surface_trees.get(surf_id) {
                let mut cur = found_hover;
                while let Some(wid) = cur {
                    if let Some(node) = tree.get(wid) {
                        if let Some(ref tt) = node.tooltip {
                            if !tt.trim().is_empty() {
                                hovered_tooltip = Some((wid, tt.clone(), node.final_rect));
                                break;
                            }
                        }
                        cur = node.parent;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    if let Some((wid, text, rect)) = hovered_tooltip {
        let current_surf_id = state.lock().unwrap().pointer_surface_id.clone().unwrap();
        let is_same = hover_tooltip.as_ref().is_some_and(|ht| {
            ht.widget == wid && ht.surface_id == current_surf_id && ht.text == text
        });
        if !is_same {
            if hover_tooltip.as_ref().is_some_and(|ht| ht.is_shown) {
                close_popup(backend, state, open_popups, surface_trees, "__tooltip__");
            }
            *hover_tooltip = Some(HoverTooltipState {
                widget: wid,
                surface_id: current_surf_id,
                text,
                rect,
                hover_start: std::time::Instant::now(),
                is_shown: false,
            });
        }
    } else if hover_tooltip.is_some() {
        if hover_tooltip.as_ref().is_some_and(|ht| ht.is_shown) {
            close_popup(backend, state, open_popups, surface_trees, "__tooltip__");
        }
        *hover_tooltip = None;
    }
}

pub fn tick_tooltip(
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    surface_trees: &mut HashMap<
        wayland_client::backend::ObjectId,
        Arc<wyrd_engine::widgets::WidgetTree>,
    >,
    render_ctx: &mut RenderContext,
    hover_tooltip: &mut Option<HoverTooltipState>,
) {
    if let Some(ref mut ht) = hover_tooltip {
        if !ht.is_shown && ht.hover_start.elapsed() >= std::time::Duration::from_millis(400) {
            ht.is_shown = true;
            let tooltip_clone = ht.clone();
            show_tooltip_surface(
                backend,
                state,
                open_popups,
                surface_trees,
                render_ctx,
                &tooltip_clone,
            );
        }
    }
}
