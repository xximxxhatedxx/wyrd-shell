use crate::compositor::CompositorIntegration;
use crate::interaction::{sync_slider_state, RecentSliderRelease, SliderDragState};
use crate::popup::{apply_tree_layout_with_dynamic_height, PopupKey};
use crate::runtime::popups::handle_module_surface_message;
use crate::runtime::render::update_bar_input_region;
use log::error;
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::Proxy;
use wyrd_engine::config::lua::LuaRuntime;
use wyrd_engine::config::BarConfig;
use wyrd_engine::modules::supervisor::Supervisor;
use wyrd_engine::modules::{CoreMessage, ModuleLevel, ModuleMessage};
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::WidgetTree;
use wyrd_engine::BarState;

pub fn ensure_companion_daemon(module_name: &str) {
    let (daemon_name, socket_name) = match module_name {
        "audio" | "microphone" => ("wyrd-audio", "wyrd-audio.sock"),
        "tray" => ("wyrd-tray", "wyrd-tray.sock"),
        "workspaces" | "window" | "keyboard" => ("wyrd-windows", "wyrd-windows.sock"),
        "notifications" => ("wyrd-notifications", "wyrd-notifications.sock"),
        "clipboard" => ("wyrd-clipboard", "wyrd-clipboard.sock"),
        "wallpaper" => ("wyrd-wallpaper", "wyrd-wallpaper.sock"),
        _ => return,
    };

    let socket_path = if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime_dir).join(socket_name)
    } else {
        std::path::PathBuf::from("/tmp").join(socket_name)
    };

    if socket_path.exists() && std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
        return;
    }

    let service_name = format!("{}.service", daemon_name);
    let systemd_started = std::process::Command::new("systemctl")
        .args(["--user", "start", &service_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if systemd_started {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if socket_path.exists() && std::os::unix::net::UnixStream::connect(&socket_path).is_ok()
            {
                log::info!("Started companion daemon via systemd: {}", service_name);
                return;
            }
        }
    }

    let mut candidate_paths = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidate_paths.push(dir.join(daemon_name));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        candidate_paths.push(
            std::path::PathBuf::from(home)
                .join(".local/bin")
                .join(daemon_name),
        );
    }
    candidate_paths.push(std::path::PathBuf::from(daemon_name));

    for candidate in candidate_paths {
        let is_valid = candidate.is_file() || candidate.as_os_str() == daemon_name;
        if is_valid {
            match std::process::Command::new(&candidate)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => {
                    log::info!(
                        "Spawned companion daemon {:?} for module '{}'",
                        candidate,
                        module_name
                    );
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    break;
                }
                Err(e) => {
                    log::debug!("Could not spawn {:?}: {}", candidate, e);
                }
            }
        }
    }
}

pub async fn setup_modules_and_supervisor(
    initial_config: &BarConfig,
) -> (
    HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    tokio::sync::mpsc::Sender<(String, ModuleMessage)>,
    tokio::sync::mpsc::Receiver<(String, ModuleMessage)>,
    tokio::task::JoinHandle<()>,
) {
    let (module_tx, module_rx) = tokio::sync::mpsc::channel::<(String, ModuleMessage)>(64);
    let mut supervisor = Supervisor::new();
    let mut module_commands: HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>> =
        HashMap::new();

    for surface in &initial_config.surfaces {
        for module_cfg in &surface.widgets {
            if let Some(module) = &module_cfg.module {
                ensure_companion_daemon(module);
            }
            if module_cfg.ty == "workspaces" {
                ensure_companion_daemon("workspaces");
            } else if module_cfg.ty == "tray" {
                ensure_companion_daemon("tray");
            }
            let Some(module) = &module_cfg.module else {
                continue;
            };
            if module_commands.contains_key(module) {
                continue;
            }
            let mut binary = format!("wyrd-module-{}", module);
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let local_bin = dir.join(&binary);
                    if local_bin.is_file() {
                        binary = local_bin.to_string_lossy().to_string();
                    }
                }
            }
            let manifest = wyrd_engine::modules::find_manifest(module, &binary);
            if matches!(manifest.level, ModuleLevel::Native | ModuleLevel::Lua) {
                continue;
            }
            if let Err(e) = supervisor
                .spawn_module(module, &binary, serde_json::json!({}), module_tx.clone())
                .await
            {
                error!("Failed to spawn module {}: {}", module, e);
            } else if let Some(sender) = supervisor.command_sender(module) {
                module_commands.insert(module.clone(), sender);
            }
        }
    }

    for module_cfg in &initial_config.modules {
        ensure_companion_daemon(&module_cfg.name);
        if module_commands.contains_key(&module_cfg.name) {
            continue;
        }
        let mut binary = format!("wyrd-module-{}", module_cfg.name);
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let local_bin = dir.join(&binary);
                if local_bin.is_file() {
                    binary = local_bin.to_string_lossy().to_string();
                }
            }
        }
        let manifest = wyrd_engine::modules::find_manifest(&module_cfg.name, &binary);
        if matches!(manifest.level, ModuleLevel::Native | ModuleLevel::Lua) {
            continue;
        }
        if let Err(e) = supervisor
            .spawn_module(
                &module_cfg.name,
                &binary,
                module_cfg.options.clone(),
                module_tx.clone(),
            )
            .await
        {
            error!("Failed to spawn module {}: {}", module_cfg.name, e);
        } else if let Some(sender) = supervisor.command_sender(&module_cfg.name) {
            module_commands.insert(module_cfg.name.clone(), sender);
        }
    }

    let supervisor_task = tokio::spawn(async move {
        supervisor.run_supervision().await;
    });

    (module_commands, module_tx, module_rx, supervisor_task)
}

pub async fn rebuild_surface_by_name(
    target_name: &str,
    lua: &LuaRuntime,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    render_ctx: &mut RenderContext,
) {
    let data_store = state
        .lock()
        .unwrap()
        .module_data
        .try_read()
        .map(|s| s.clone())
        .unwrap_or_default();
    let config_arc = state.lock().unwrap().config.clone();
    let current_config = config_arc.read().await.clone();

    let is_theme_refresh = target_name == "*";
    let surfaces_to_rebuild: Vec<String> = if is_theme_refresh {
        current_config
            .surfaces
            .iter()
            .map(|s| s.name.clone())
            .collect()
    } else {
        vec![target_name.to_string()]
    };

    for surf_name in surfaces_to_rebuild {
        if let Some(surface_config) = current_config.surfaces.iter().find(|s| s.name == surf_name) {
            let candidate_widgets_opt = if surface_config.has_build_fn {
                lua.build_surface_with_store(&surf_name, &data_store)
                    .ok()
                    .flatten()
            } else {
                Some(surface_config.widgets.clone())
            };

            if let Some(new_widgets) = candidate_widgets_opt {
                if !is_theme_refresh
                    && surface_config.has_build_fn
                    && serde_json::to_value(&new_widgets).ok()
                        == serde_json::to_value(&surface_config.widgets).ok()
                {
                    log::debug!(
                        "Surface '{}' build output is structurally identical, skipping rebuild",
                        surf_name
                    );
                    continue;
                }
                if surface_config.has_build_fn {
                    let config_arc = state.lock().unwrap().config.clone();
                    let mut cfg_write = config_arc.write().await;
                    if let Some(sc) = cfg_write.surfaces.iter_mut().find(|s| s.name == surf_name) {
                        sc.widgets = new_widgets.clone();
                    }
                }

                for surface in backend
                    .surface_manager
                    .surfaces
                    .iter_mut()
                    .filter(|s| s.config_name == surf_name)
                {
                    let candidate_tree =
                        if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
                            wyrd_engine::widgets::from_popup_config_with_store(
                                &new_widgets,
                                &current_config.styles,
                                &data_store,
                            )
                        } else {
                            let ct = wyrd_engine::widgets::from_config_with_store(
                                &new_widgets,
                                &current_config.styles,
                                &data_store,
                            );
                            if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                                let extents = ct.shadow_extents(surface.scale.max(1.0) as f32);
                                let geom = crate::runtime::popups::compute_bar_geometry(
                                    surface_config,
                                    surface.width,
                                    surface_config.height,
                                    extents,
                                );
                                surface.shadow_insets = geom.shadow_insets;
                            }
                            ct
                        };

                    let updated_tree = if let Some(existing_tree) =
                        surface_trees.get(&surface.surface.id())
                    {
                        let diff = wyrd_engine::widgets::diff_trees(existing_tree, &candidate_tree);
                        let mut mut_tree = (**existing_tree).clone();
                        mut_tree.apply_diff(&candidate_tree, &diff);
                        mut_tree
                    } else {
                        candidate_tree
                    };

                    let mut tree = updated_tree;
                    if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
                        let mut temp_map = HashMap::new();
                        apply_tree_layout_with_dynamic_height(
                            surface,
                            &mut tree,
                            &mut temp_map,
                            render_ctx,
                        );
                        if let Some(res_tree) = temp_map.remove(&surface.surface.id()) {
                            surface_trees.insert(surface.surface.id(), res_tree);
                        } else {
                            surface_trees.insert(surface.surface.id(), Arc::new(tree));
                        }
                    } else {
                        let (bar_x, bar_y, content_width, content_height) =
                            if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                                let extents =
                                    (0.0, 0.0, surface.shadow_insets.2, surface.shadow_insets.3);
                                let geom = crate::runtime::popups::compute_bar_geometry(
                                    surface_config,
                                    surface.width,
                                    surface_config.height,
                                    extents,
                                );
                                (geom.bar_x, geom.bar_y, geom.content_w, geom.content_h)
                            } else {
                                (
                                    0.0,
                                    0.0,
                                    surface.width.max(1) as f32,
                                    surface.height.max(1) as f32,
                                )
                            };
                        wyrd_engine::widgets::tree::measure_tree(
                            &mut tree,
                            render_ctx,
                            content_width,
                            content_height,
                        );
                        wyrd_engine::widgets::tree::layout_tree(
                            &mut tree,
                            bar_x,
                            bar_y,
                            content_width,
                            content_height,
                        );
                        surface_trees.insert(surface.surface.id(), Arc::new(tree));
                        if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                            update_bar_input_region(&backend.compositor, &backend.qh, surface);
                        }
                    }
                    surface.dirty = true;
                }
                log::info!(
                    "Dynamically rebuilt surface '{}' from Lua build function",
                    surf_name
                );
            }
        }
    }
    state.lock().unwrap().frame_ready = true;
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_module_message(
    module: String,
    message: ModuleMessage,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    surface_trees: &mut HashMap<ObjectId, Arc<WidgetTree>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    lua: &LuaRuntime,
    compositor: &dyn CompositorIntegration,
    active_slider: &mut Option<SliderDragState>,
    recent_slider_release: &mut Option<RecentSliderRelease>,
    surface_timeout_tx: &tokio::sync::mpsc::Sender<(String, ModuleMessage)>,
) -> anyhow::Result<()> {
    match message {
        ModuleMessage::Update { widget_id, payload } => {
            let wid = if !widget_id.is_empty() {
                widget_id.clone()
            } else {
                module.clone()
            };

            if let Ok(mut store) = state.lock().unwrap().module_data.try_write() {
                store.insert(wid.clone(), payload.clone());
                if (wid == module || wid.is_empty()) && !wid.starts_with("popup:") {
                    store.insert(module.clone(), payload.clone());
                }
            }

            let data_store = state
                .lock()
                .unwrap()
                .module_data
                .try_read()
                .map(|s| s.clone())
                .unwrap_or_default();
            let config_arc = state.lock().unwrap().config.clone();
            let current_config = config_arc.read().await.clone();

            for surface_cfg in current_config.surfaces.iter().filter(|s| s.has_build_fn) {
                lua.request_rebuild(&surface_cfg.name);
            }

            for (surface_id, tree) in surface_trees.iter_mut() {
                let surf_opt = backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| &s.surface.id() == surface_id);
                if let Some(surf) = surf_opt {
                    let has_build = current_config
                        .surfaces
                        .iter()
                        .find(|sc| sc.name == surf.config_name)
                        .is_some_and(|sc| sc.has_build_fn);
                    if has_build {
                        continue;
                    }
                    if surf.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
                        let matches_popup = if let Some((target_popup, _)) =
                            open_popups.iter().find(|(_, out_id)| {
                                surf.output
                                    .as_ref()
                                    .map(|o| o.id() == *out_id)
                                    .unwrap_or(true)
                            }) {
                            let s_name = &surf.config_name;
                            let target_id = wid.strip_prefix("popup:").unwrap_or(&wid);
                            let surface_mod = current_config
                                .surfaces
                                .iter()
                                .find(|s| s.name == *s_name)
                                .and_then(|s| s.module.as_deref())
                                .unwrap_or("");
                            let surface_channel = current_config
                                .surfaces
                                .iter()
                                .find(|s| s.name == *s_name)
                                .and_then(|s| s.module_channel.as_deref());
                            surface_channel == Some(module.as_str())
                                || surface_channel == Some(target_id)
                                || s_name == target_popup
                                || s_name == &module
                                || s_name == target_id
                                || surface_mod == module
                                || surface_mod == target_id
                        } else {
                            let s_name = &surf.config_name;
                            let target_id = wid.strip_prefix("popup:").unwrap_or(&wid);
                            let surface_mod = current_config
                                .surfaces
                                .iter()
                                .find(|s| s.name == *s_name)
                                .and_then(|s| s.module.as_deref())
                                .unwrap_or("");
                            let surface_channel = current_config
                                .surfaces
                                .iter()
                                .find(|s| s.name == *s_name)
                                .and_then(|s| s.module_channel.as_deref());
                            surface_channel == Some(module.as_str())
                                || surface_channel == Some(target_id)
                                || s_name == &module
                                || s_name == target_id
                                || surface_mod == module
                                || surface_mod == target_id
                        };
                        if !matches_popup {
                            continue;
                        }
                    }
                }

                let surf_config_opt = backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| &s.surface.id() == surface_id)
                    .and_then(|s| {
                        current_config.surfaces.iter().find(|sc| {
                            sc.name == s.config_name
                                || sc.module.as_deref() == Some(s.config_name.as_str())
                        })
                    });

                let out_name = backend
                    .surface_manager
                    .surfaces
                    .iter()
                    .find(|s| &s.surface.id() == surface_id)
                    .and_then(|s| s.output.as_ref())
                    .and_then(|out| {
                        state
                            .lock()
                            .unwrap()
                            .outputs
                            .values()
                            .find(|o| o.output.id() == out.id())
                            .map(|o| o.name.clone())
                    });

                let mut custom_popup_rebuilt = false;
                if let Some(sc) = surf_config_opt {
                    let target_id = wid.strip_prefix("popup:").unwrap_or(&wid);
                    let is_dynamic_module_popup = sc.ty == "popup"
                        && !sc.widgets.is_empty()
                        && wid.starts_with("popup:")
                        && payload
                            .get("children")
                            .and_then(serde_json::Value::as_array)
                            .is_some()
                        && target_id == sc.name;

                    fn widget_cfg_mentions_module(
                        w: &wyrd_engine::widgets::WidgetConfig,
                        m: &str,
                    ) -> bool {
                        if w.module.as_deref() == Some(m) {
                            return true;
                        }
                        if let Some(ref b) = w.bind {
                            if b.starts_with(m)
                                || b.starts_with(&format!("{}.", m))
                                || b.starts_with(&format!("module.{}.", m))
                            {
                                return true;
                            }
                        }
                        if let Some(ref t) = w.text {
                            if t.contains(&format!("{{{}.", m))
                                || t.contains(&format!("{{module.{}.", m))
                            {
                                return true;
                            }
                        }
                        if let Some(ref act) = w.on_click {
                            if act.contains(&format!("event:{}:", m))
                                || act.contains(&format!("popup:{}", m))
                            {
                                return true;
                            }
                        }
                        if let Some(ref act) = w.on_change {
                            if act.contains(&format!("event:{}:", m))
                                || act.contains(&format!("popup:{}", m))
                            {
                                return true;
                            }
                        }
                        w.children.iter().any(|c| widget_cfg_mentions_module(c, m))
                    }

                    let popup_mentions_module = sc.module.as_deref() == Some(module.as_str())
                        || sc.module_channel.as_deref() == Some(module.as_str())
                        || sc.name == module
                        || sc.name == target_id
                        || sc
                            .widgets
                            .iter()
                            .any(|w| widget_cfg_mentions_module(w, &module));

                    let is_open_custom_popup = sc.ty == "popup"
                        && !sc.widgets.is_empty()
                        && open_popups.iter().any(|(p_name, _)| p_name == &sc.name)
                        && popup_mentions_module;

                    if is_dynamic_module_popup || is_open_custom_popup {
                        let candidate_tree = wyrd_engine::widgets::from_popup_config_with_store(
                            &sc.widgets,
                            &current_config.styles,
                            &data_store,
                        );
                        let diff = wyrd_engine::widgets::diff_trees(tree, &candidate_tree);
                        let mut mut_tree = (**tree).clone();
                        mut_tree.apply_diff(&candidate_tree, &diff);
                        mut_tree.update_module_for_output(
                            &module,
                            &wid,
                            payload.clone(),
                            out_name.as_deref(),
                        );
                        sync_slider_state(
                            &mut mut_tree,
                            surface_id,
                            active_slider,
                            recent_slider_release,
                        );
                        if let Some(surface) = backend
                            .surface_manager
                            .surfaces
                            .iter_mut()
                            .find(|s| &s.surface.id() == surface_id)
                        {
                            let mut temp_map = HashMap::new();
                            apply_tree_layout_with_dynamic_height(
                                surface,
                                &mut mut_tree,
                                &mut temp_map,
                                render_ctx,
                            );
                            if let Some(res_tree) = temp_map.remove(&surface.surface.id()) {
                                *tree = res_tree;
                            } else {
                                *tree = Arc::new(mut_tree);
                            }
                            surface.dirty = true;
                        } else {
                            *tree = Arc::new(mut_tree);
                        }
                        custom_popup_rebuilt = true;
                    }
                }

                if !custom_popup_rebuilt {
                    let mut mut_tree = (**tree).clone();
                    let changed = mut_tree.update_module_for_output(
                        &module,
                        &wid,
                        payload.clone(),
                        out_name.as_deref(),
                    );
                    if changed {
                        sync_slider_state(
                            &mut mut_tree,
                            surface_id,
                            active_slider,
                            recent_slider_release,
                        );
                        if let Some(surface) = backend
                            .surface_manager
                            .surfaces
                            .iter_mut()
                            .find(|s| &s.surface.id() == surface_id)
                        {
                            if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Popup {
                                let mut temp_map = HashMap::new();
                                apply_tree_layout_with_dynamic_height(
                                    surface,
                                    &mut mut_tree,
                                    &mut temp_map,
                                    render_ctx,
                                );
                                if let Some(res_tree) = temp_map.remove(&surface.surface.id()) {
                                    *tree = res_tree;
                                } else {
                                    *tree = Arc::new(mut_tree);
                                }
                            } else {
                                let content_width = surface.width.max(1) as f32;
                                let content_height = surface.height.max(1) as f32;
                                wyrd_engine::widgets::tree::measure_tree(
                                    &mut mut_tree,
                                    render_ctx,
                                    content_width,
                                    content_height,
                                );
                                wyrd_engine::widgets::tree::layout_tree(
                                    &mut mut_tree,
                                    0.0,
                                    0.0,
                                    content_width,
                                    content_height,
                                );
                                *tree = Arc::new(mut_tree);
                                if surface.ty == wyrd_engine::wayland::surface::SurfaceType::Bar {
                                    update_bar_input_region(
                                        &backend.compositor,
                                        &backend.qh,
                                        surface,
                                    );
                                }
                            }
                            surface.dirty = true;
                        } else {
                            *tree = Arc::new(mut_tree);
                        }
                    }
                }
            }

            if module == "brightness" || wid == "brightness" || wid.starts_with("popup:brightness")
            {
                for surface in backend.surface_manager.surfaces.iter_mut() {
                    if surface.config_name == "__monitor_dimmer__" {
                        surface.dirty = true;
                    }
                }
            }

            if backend.surface_manager.surfaces.iter().any(|s| s.dirty) {
                state.lock().unwrap().frame_ready = true;
            }
        }
        ModuleMessage::Surface { action, params } => {
            handle_module_surface_message(
                &module,
                &action,
                &params,
                backend,
                state,
                surface_trees,
                open_popups,
                render_ctx,
                module_commands,
                compositor,
                surface_timeout_tx,
            )
            .await?;
        }
        ModuleMessage::Publish { topic, value } => {
            state
                .lock()
                .unwrap()
                .event_bus
                .publish_topic(&topic, value.clone());

            for (mod_name, cmd_tx) in module_commands.iter() {
                if mod_name != &module {
                    let _ = cmd_tx.try_send(CoreMessage::TopicEvent {
                        topic: topic.clone(),
                        value: value.clone(),
                    });
                }
            }

            if topic == "theme.palette" {
                log::info!("Reactive dynamic theme palette update: {:?}", value);
                let _ = lua.update_palette(&value);

                if let Some(accent_str) = value
                    .get("primary")
                    .or_else(|| value.get("accent"))
                    .and_then(|v| v.as_str())
                {
                    if let Some(color) = wyrd_engine::widgets::parse_color(accent_str) {
                        let style_arc = state.lock().unwrap().style.clone();
                        style_arc.write().await.set_accent(color);
                    }
                }

                {
                    let config_arc = state.lock().unwrap().config.clone();
                    let mut cfg_write = config_arc.write().await;
                    if let Some(updated_styles) = lua.get_styles() {
                        cfg_write.styles = updated_styles;
                    }
                }

                // Rebuild and refresh all surfaces immediately
                rebuild_surface_by_name("*", lua, backend, state, surface_trees, render_ctx).await;

                for surface in backend.surface_manager.surfaces.iter_mut() {
                    surface.dirty = true;
                }
                state.lock().unwrap().frame_ready = true;
            }

            if topic == "theme.accent" {
                let accent_candidate = value
                    .get("accent")
                    .and_then(|v| v.as_str())
                    .or_else(|| value.as_str());

                if let Some(hex_accent) = accent_candidate {
                    let hex_accent = hex_accent.to_string();
                    log::info!("Reactive dynamic theme accent update: {}", hex_accent);

                    let _ = lua.update_accent(&hex_accent);

                    if let Some(color) = wyrd_engine::widgets::parse_color(&hex_accent) {
                        let style_arc = state.lock().unwrap().style.clone();
                        style_arc.write().await.set_accent(color);
                    }

                    {
                        let config_arc = state.lock().unwrap().config.clone();
                        let mut cfg_write = config_arc.write().await;
                        if let Some(updated_styles) = lua.get_styles() {
                            cfg_write.styles = updated_styles;
                        } else {
                            for style in cfg_write.styles.values_mut() {
                                if style.accent.is_some() {
                                    style.accent = Some(hex_accent.clone());
                                }
                            }
                        }
                    }

                    // Rebuild and refresh all surfaces immediately
                    rebuild_surface_by_name("*", lua, backend, state, surface_trees, render_ctx)
                        .await;

                    for surface in backend.surface_manager.surfaces.iter_mut() {
                        surface.dirty = true;
                    }
                    state.lock().unwrap().frame_ready = true;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
