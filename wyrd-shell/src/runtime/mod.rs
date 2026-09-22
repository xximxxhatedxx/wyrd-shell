pub mod input;
pub mod modules;
pub mod popups;
pub mod render;
pub mod socket;
pub mod state;
pub mod tooltip;

use crate::compositor::{create_compositor_integration, resolve_cursor_output_id, sync_keybinds};
use crate::interaction::{DragState, RecentSliderRelease, SliderDragState};
use crate::popup::PopupKey;
use crate::runtime::input::process_input_events;
use crate::runtime::modules::{
    handle_module_message, rebuild_surface_by_name, setup_modules_and_supervisor,
};
use crate::runtime::popups::reconcile_surfaces;
use crate::runtime::render::{
    apply_surface_configures_and_scales, is_connection_closed, notify_ready, render_frame,
    tick_animations, trim_process_memory,
};
use crate::runtime::socket::{dispatch_shell_action, spawn_socket_listener};
use crate::runtime::tooltip::{tick_tooltip, HoverTooltipState};
use anyhow::Result;
use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::Proxy;
use wyrd_engine::config::lua::LuaRuntime;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::render::damage::DamageTracker;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::WidgetTree;

pub async fn run(cli: crate::cli::Cli) -> Result<()> {
    let (mut backend, state) = WaylandBackend::connect().await?;
    let compositor = create_compositor_integration(cli.compositor);
    let config_path = cli.config;
    let lua = LuaRuntime::new(config_path.clone())?;

    let initial_config = lua.load_config().await?;
    let config_store = Arc::new(tokio::sync::RwLock::new(initial_config.clone()));
    let tree_store = Arc::new(tokio::sync::RwLock::new(WidgetTree::new()));
    let mut surface_trees: HashMap<wayland_client::backend::ObjectId, Arc<WidgetTree>> =
        HashMap::new();

    {
        let mut state_guard = state.lock().unwrap();
        state_guard.config = config_store.clone();
        state_guard.widget_tree = tree_store.clone();
    }

    let (module_commands, module_tx, mut module_rx, supervisor_task) =
        setup_modules_and_supervisor(&initial_config).await;
    let surface_timeout_tx = module_tx.clone();

    let (reload_tx, mut reload_rx) = tokio::sync::mpsc::channel(4);
    let lua_watcher = LuaRuntime::new(config_path)?;
    let watcher_task = tokio::spawn(async move {
        if let Err(e) = lua_watcher.watch_config(reload_tx).await {
            error!("Config watcher failed: {}", e);
        }
    });

    let mut render_ctx = RenderContext::new(1.0);
    let mut open_popups: std::collections::HashSet<PopupKey> = std::collections::HashSet::new();
    let initial_config_for_surfaces = initial_config.clone();
    reconcile_surfaces(
        &mut backend,
        &state,
        &initial_config_for_surfaces,
        &open_popups,
        &mut surface_trees,
        &mut render_ctx,
    )?;
    notify_ready();
    sync_keybinds(compositor.as_ref(), &initial_config.keybinds);

    let (action_tx, mut action_rx) = tokio::sync::mpsc::channel::<String>(32);
    let (_socket_task, sock_path_for_cleanup) = spawn_socket_listener(action_tx)?;

    trim_process_memory();

    let mut damage_trackers: HashMap<wayland_client::backend::ObjectId, DamageTracker> =
        HashMap::new();
    for surface in &backend.surface_manager.surfaces {
        damage_trackers.insert(surface.surface.id(), DamageTracker::default());
    }

    let mut frame_interval = tokio::time::interval(std::time::Duration::from_millis(16));
    let mut trim_interval = tokio::time::interval(std::time::Duration::from_secs(15));
    let mut dragging_popup: Option<DragState> = None;
    let mut active_slider: Option<SliderDragState> = None;
    let mut recent_slider_release: Option<RecentSliderRelease> = None;
    let mut hover_tooltip: Option<HoverTooltipState> = None;
    let mut output_signature = state
        .lock()
        .unwrap()
        .outputs
        .keys()
        .copied()
        .collect::<Vec<_>>();
    output_signature.sort_unstable();

    loop {
        let mut input_events = Vec::new();
        let tooltip_delay = match &hover_tooltip {
            Some(ht) if !ht.is_shown => {
                let elapsed = ht.hover_start.elapsed();
                if elapsed >= std::time::Duration::from_millis(300) {
                    std::time::Duration::from_millis(0)
                } else {
                    std::time::Duration::from_millis(300) - elapsed
                }
            }
            _ => std::time::Duration::from_secs(3600),
        };

        tokio::select! {
            biased;

            _ = tokio::time::sleep(tooltip_delay), if hover_tooltip.as_ref().is_some_and(|h| !h.is_shown) => {}

            Some(ext_action) = action_rx.recv() => {
                // Pick the bar surface on the currently active monitor (cursor position or focused monitor)
                // so that popups like the launcher open on the active screen instead of defaulting to the first monitor.
                let active_out_id = resolve_cursor_output_id(compositor.as_ref(), &state, None);
                let target_surf_id = active_out_id
                    .and_then(|out_id| {
                        backend.surface_manager.surfaces.iter()
                            .find(|s| {
                                s.ty == wyrd_engine::wayland::surface::SurfaceType::Bar
                                    && s.output.as_ref().map(|o| o.id()) == Some(out_id.clone())
                            })
                            .map(|s| s.surface.id())
                    })
                    .or_else(|| {
                        backend.surface_manager.surfaces.iter()
                            .find(|s| s.ty == wyrd_engine::wayland::surface::SurfaceType::Bar)
                            .map(|s| s.surface.id())
                    })
                    .or_else(|| backend.surface_manager.surfaces.first().map(|s| s.surface.id()));
                if let Some(surf_id) = target_surf_id {
                    dispatch_shell_action(
                        &ext_action,
                        surf_id,
                        0.0,
                        0.0,
                        None,
                        &mut backend,
                        &state,
                        &mut open_popups,
                        &mut surface_trees,
                        &mut render_ctx,
                        &module_commands,
                        &lua,
                        compositor.as_ref(),
                    ).await;
                }
            }

            dispatch_res = backend.dispatch_once(state.clone()) => {
                if let Err(e) = dispatch_res {
                    if is_connection_closed(&e) {
                        info!("Wayland connection closed");
                        break;
                    }
                    error!("Wayland dispatch error: {}", e);
                }
                apply_surface_configures_and_scales(&mut backend, &state);
                let (input_events_taken, config_to_reconcile) = {
                    let mut s = state.lock().unwrap();
                    let events = std::mem::take(&mut s.pending_input_events);
                    let mut new_signature = s.outputs.keys().copied().collect::<Vec<_>>();
                    new_signature.sort_unstable();
                    let cfg = if new_signature != output_signature {
                        output_signature = new_signature;
                        Some(s.config.clone())
                    } else {
                        None
                    };
                    (events, cfg)
                };
                input_events = input_events_taken;
                if let Some(current_config) = config_to_reconcile {
                    let config = current_config.read().await.clone();
                    if let Err(error) = reconcile_surfaces(
                        &mut backend,
                        &state,
                        &config,
                        &open_popups,
                        &mut surface_trees,
                        &mut render_ctx,
                    ) {
                        error!("Failed to reconcile surfaces after output change: {}", error);
                    }
                    state.lock().unwrap().frame_ready = true;
                }
            }

            Some((module, message)) = module_rx.recv() => {
                if let Err(e) = handle_module_message(
                    module,
                    message,
                    &mut backend,
                    &state,
                    &mut surface_trees,
                    &mut open_popups,
                    &mut render_ctx,
                    &module_commands,
                    &lua,
                    compositor.as_ref(),
                    &mut active_slider,
                    &mut recent_slider_release,
                    &surface_timeout_tx,
                ).await {
                    error!("Module message error: {}", e);
                }
            }

            Some(new_config) = reload_rx.recv() => {
                info!("Configuration reload detected, updating shell layout");
                {
                    let config_arc = state.lock().unwrap().config.clone();
                    *config_arc.write().await = new_config.clone();
                }
                if let Err(e) = reconcile_surfaces(
                    &mut backend,
                    &state,
                    &new_config,
                    &open_popups,
                    &mut surface_trees,
                    &mut render_ctx,
                ) {
                    error!("Surface reconciliation failed on reload: {}", e);
                }
                sync_keybinds(compositor.as_ref(), &new_config.keybinds);
                state.lock().unwrap().frame_ready = true;
            }

            _ = frame_interval.tick() => {
                let is_animating = tick_animations(&mut backend, &mut surface_trees, &mut render_ctx, &dragging_popup);
                let has_repeating = state
                    .lock()
                    .unwrap()
                    .input_state
                    .try_read()
                    .map(|inp| inp.keyboard.repeating_key.is_some())
                    .unwrap_or(false);
                if is_animating || has_repeating || backend.surface_manager.surfaces.iter().any(|s| s.dirty) {
                    state.lock().unwrap().frame_ready = true;
                }
            }

            _ = trim_interval.tick() => {
                trim_process_memory();
            }
        }

        if let Ok(mut inp) = state.lock().unwrap().input_state.try_write() {
            if let Some((keysym, utf8)) = inp.keyboard.poll_repeat(std::time::Instant::now()) {
                input_events.push(wyrd_engine::input::InputEvent::KeyPress { keysym, utf8 });
            }
        }

        if lua.has_pending_rebuilds() {
            let pending = lua.drain_pending_rebuilds();
            for target in pending {
                rebuild_surface_by_name(
                    &target,
                    &lua,
                    &mut backend,
                    &state,
                    &mut surface_trees,
                    &mut render_ctx,
                )
                .await;
            }
        }

        tick_tooltip(
            &mut backend,
            &state,
            &mut open_popups,
            &mut surface_trees,
            &mut render_ctx,
            &mut hover_tooltip,
        );

        if !input_events.is_empty() {
            process_input_events(
                input_events,
                &mut backend,
                &state,
                &mut surface_trees,
                &mut open_popups,
                &mut render_ctx,
                &module_commands,
                &lua,
                compositor.as_ref(),
                &mut dragging_popup,
                &mut hover_tooltip,
                &mut active_slider,
                &mut recent_slider_release,
            )
            .await;
        }

        let is_ready = state.lock().unwrap().frame_ready;
        if is_ready {
            state.lock().unwrap().frame_ready = false;
            match render_frame(
                &mut backend,
                &state,
                &mut surface_trees,
                &mut render_ctx,
                &mut damage_trackers,
            ) {
                Ok(false) => break,
                Err(e) => {
                    if is_connection_closed(&e) {
                        info!("Wayland connection closed");
                        break;
                    }
                    error!("Render frame error: {}", e);
                }
                _ => {}
            }
        }
    }

    supervisor_task.abort();
    watcher_task.abort();
    let _ = std::fs::remove_file(&sock_path_for_cleanup);
    info!("Wyrd Shell exiting gracefully");
    Ok(())
}
