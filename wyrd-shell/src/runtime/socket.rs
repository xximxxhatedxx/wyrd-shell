use crate::compositor::CompositorIntegration;
use crate::interaction::*;
use crate::popup::*;
use anyhow::{Context, Result};
use log::{error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::BarState;

pub fn shell_socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("wyrd-shell.sock"))
        .unwrap_or_else(|_| {
            // SAFETY: `libc::getuid` is a standard, stateless POSIX syscall that has no side effects and is always safe.
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/run/user/{}/wyrd-shell.sock", uid))
        })
}

pub fn get_settings_file_path() -> PathBuf {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home).join("wyrd/settings.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/wyrd/settings.toml")
    } else {
        PathBuf::from(".config/wyrd/settings.toml")
    }
}

pub fn update_settings_file<F>(updater: F) -> Result<()>
where
    F: FnOnce(&mut toml::Table),
{
    let path = get_settings_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut table = if path.is_file() {
        let content = std::fs::read_to_string(&path)?;
        content.parse::<toml::Table>().unwrap_or_default()
    } else {
        toml::Table::new()
    };
    updater(&mut table);
    let serialized = toml::to_string_pretty(&table)?;
    std::fs::write(&path, serialized)?;
    Ok(())
}

pub fn read_current_setting(key: &str) -> Option<String> {
    let path = get_settings_file_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    table
        .get(key)
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

pub fn available_themes() -> Vec<String> {
    let mut themes = Vec::new();
    let themes_dirs = [
        get_settings_file_path().parent().map(|p| p.join("themes")),
        Some(PathBuf::from("/etc/wyrd/themes")),
    ];
    for dir_opt in themes_dirs.iter().flatten() {
        if let Ok(entries) = std::fs::read_dir(dir_opt) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !themes.contains(&name.to_string()) {
                            themes.push(name.to_string());
                        }
                    }
                } else if path
                    .extension()
                    .is_some_and(|ext| ext == "lua" || ext == "toml")
                {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !themes.contains(&stem.to_string()) {
                            themes.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    if themes.is_empty() {
        themes.push("catppuccin".to_string());
        themes.push("aetheria".to_string());
        themes.push("nord".to_string());
        themes.push("gruvbox".to_string());
        themes.push("tokyo-night".to_string());
    }
    themes
}

pub fn available_layouts() -> Vec<String> {
    let mut layouts = Vec::new();
    let layouts_dirs = [
        get_settings_file_path().parent().map(|p| p.join("layouts")),
        Some(PathBuf::from("/etc/wyrd/layouts")),
    ];
    for dir_opt in layouts_dirs.iter().flatten() {
        if let Ok(entries) = std::fs::read_dir(dir_opt) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !layouts.contains(&name.to_string()) {
                            layouts.push(name.to_string());
                        }
                    }
                } else if path
                    .extension()
                    .is_some_and(|ext| ext == "toml" || ext == "lua")
                {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !layouts.contains(&stem.to_string()) {
                            layouts.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    if layouts.is_empty() {
        layouts.push("top".to_string());
        layouts.push("left".to_string());
    }
    layouts
}

#[allow(clippy::too_many_arguments)]
pub async fn dispatch_shell_action(
    action_str: &str,
    surface_id: wayland_client::backend::ObjectId,
    axis_x: f64,
    axis_y: f64,
    trigger_rect: Option<(f32, f32, f32, f32)>,
    backend: &mut WaylandBackend,
    state: &Arc<std::sync::Mutex<BarState>>,
    open_popups: &mut std::collections::HashSet<PopupKey>,
    surface_trees: &mut HashMap<
        wayland_client::backend::ObjectId,
        Arc<wyrd_engine::widgets::WidgetTree>,
    >,
    render_ctx: &mut RenderContext,
    module_commands: &HashMap<String, tokio::sync::mpsc::Sender<wyrd_engine::modules::CoreMessage>>,
    lua: &wyrd_engine::config::lua::LuaRuntime,
    compositor: &dyn CompositorIntegration,
) {
    let action_args = shell_words::split(action_str).unwrap_or_default();
    let is_popup_toggle = action_args.first().map(String::as_str) == Some("popup:toggle");

    if action_str.starts_with("theme:") {
        let cmd = action_str.trim_start_matches("theme:").trim();
        if cmd == "toggle" {
            let current = read_current_setting("theme").unwrap_or_else(|| "catppuccin".to_string());
            let themes = available_themes();
            let cur_idx = themes.iter().position(|t| t == &current).unwrap_or(0);
            let next = &themes[(cur_idx + 1) % themes.len()];
            info!("Toggling theme from {} to {}", current, next);
            let _ = update_settings_file(|t| {
                t.insert("theme".to_string(), toml::Value::String(next.to_string()));
            });
        } else if cmd.starts_with("set ") {
            let target = cmd.trim_start_matches("set ").trim();
            info!("Setting theme to {}", target);
            let _ = update_settings_file(|t| {
                t.insert("theme".to_string(), toml::Value::String(target.to_string()));
            });
        }
        return;
    }

    if action_str.starts_with("layout:") {
        let cmd = action_str.trim_start_matches("layout:").trim();
        if cmd == "toggle" {
            let current = read_current_setting("layout").unwrap_or_else(|| "top".to_string());
            let layouts = available_layouts();
            let cur_idx = layouts.iter().position(|l| l == &current).unwrap_or(0);
            let next = &layouts[(cur_idx + 1) % layouts.len()];
            info!("Toggling layout from {} to {}", current, next);
            let _ = update_settings_file(|t| {
                t.insert("layout".to_string(), toml::Value::String(next.to_string()));
            });
        } else if cmd.starts_with("set ") {
            let target = cmd.trim_start_matches("set ").trim();
            info!("Setting layout to {}", target);
            let _ = update_settings_file(|t| {
                t.insert(
                    "layout".to_string(),
                    toml::Value::String(target.to_string()),
                );
            });
        }
        return;
    }

    if action_str == "reload" {
        info!("Manual reload action requested");
        let _ = update_settings_file(|_| {});
        return;
    }

    if action_str == "rebuild_all" || action_str == "rebuild *" {
        info!("Manual rebuild_all requested");
        lua.request_rebuild_all();
        return;
    }

    if action_str.starts_with("rebuild ") {
        let target = action_str.trim_start_matches("rebuild ").trim();
        info!("Manual rebuild surface '{}' requested", target);
        lua.request_rebuild(target);
        return;
    }

    if action_str.starts_with("workspace:") {
        let target = action_str.trim_start_matches("workspace:").trim();
        let target = target
            .trim_start_matches("switch ")
            .trim_start_matches("set ")
            .trim();
        if let Err(e) = compositor.switch_workspace(target) {
            log::warn!("Failed to switch workspace to {}: {}", target, e);
        }
        return;
    }

    if action_str.starts_with("event:") {
        let payload = action_str.trim_start_matches("event:");
        let parts: Vec<&str> = payload.splitn(3, ':').collect();
        if let Some(&target_mod) = parts.first() {
            let widget_id = parts.get(1).copied().unwrap_or("").to_string();
            let event_name = parts.get(2).copied().unwrap_or("click").to_string();
            let (gx, gy) = compositor.cursor_position().unwrap_or_else(|| {
                state
                    .lock()
                    .unwrap()
                    .input_state
                    .try_read()
                    .map(|inp| (inp.pointer.x as i32, inp.pointer.y as i32))
                    .unwrap_or((0, 0))
            });
            if let Some(sender) = module_commands.get(target_mod).or_else(|| {
                module_commands
                    .iter()
                    .find(|(k, _)| k.replace('-', "_") == target_mod.replace('-', "_"))
                    .map(|(_, v)| v)
            }) {
                send_module_event(
                    sender,
                    wyrd_engine::modules::CoreMessage::Event {
                        widget_id,
                        event: event_name,
                    },
                );
            } else {
                let _ = spawn_action(
                    &format!(
                        "wyrd-module-{} --event '{}:{}'",
                        target_mod, widget_id, event_name
                    ),
                    gx as f64,
                    gy as f64,
                );
            }
        }
        return;
    }

    if action_str.starts_with("popup:") {
        let sub_action = action_str.trim_start_matches("popup:").trim();
        if sub_action.starts_with("close") {
            let target = sub_action
                .trim_start_matches("close")
                .trim_start_matches(':')
                .trim();
            if !target.is_empty() {
                close_popup(backend, state, open_popups, surface_trees, target);
            } else {
                let config_arc = state.lock().unwrap().config.clone();
                let current_config = config_arc.read().await.clone();
                close_unpinned_popups(backend, state, &current_config, open_popups, surface_trees);
            }
        } else if sub_action.starts_with("exec-term ") {
            let cmd = sub_action.trim_start_matches("exec-term ").trim();
            let term = std::env::var("TERMINAL").unwrap_or_default();
            let full_cmd = if !term.is_empty() {
                format!("{} -e {}", term, cmd)
            } else {
                format!(
                    "sh -c \"command -v xdg-terminal-exec >/dev/null 2>&1 && xdg-terminal-exec {0} || {0}\"",
                    cmd
                )
            };
            let _ = spawn_action(&full_cmd, 0.0, 0.0);
            close_all_popups(backend, state, open_popups, surface_trees);
        } else if sub_action.starts_with("exec ") {
            let cmd = sub_action.trim_start_matches("exec ").trim();
            let _ = spawn_action(cmd, 0.0, 0.0);
            close_all_popups(backend, state, open_popups, surface_trees);
        } else {
            let popup_name = if is_popup_toggle {
                action_args.get(1).map(String::as_str)
            } else {
                Some(sub_action)
            };
            if let Some(popup_name) = popup_name {
                let config_arc = state.lock().unwrap().config.clone();
                let current_config = config_arc.read().await.clone();
                let unrelated: Vec<String> = open_popups
                    .iter()
                    .map(|(p, _)| p.clone())
                    .filter(|p| {
                        !is_popup_pinned(p, &current_config)
                            && !are_popups_related(p, popup_name, &current_config)
                    })
                    .collect();
                for p in unrelated {
                    close_popup(backend, state, open_popups, surface_trees, &p);
                }
                let mut params_obj = serde_json::Map::new();
                if let Some((tx, ty, tw, th)) = trigger_rect {
                    params_obj.insert("trigger_x".to_string(), serde_json::json!(tx));
                    params_obj.insert("trigger_y".to_string(), serde_json::json!(ty));
                    params_obj.insert("trigger_w".to_string(), serde_json::json!(tw));
                    params_obj.insert("trigger_h".to_string(), serde_json::json!(th));
                }
                let params_val = serde_json::Value::Object(params_obj);
                let params_ref = if trigger_rect.is_some() {
                    Some(&params_val)
                } else {
                    None
                };

                if let Err(error) = toggle_configured_popup(
                    compositor,
                    backend,
                    state,
                    &current_config,
                    open_popups,
                    surface_trees,
                    popup_name,
                    surface_id,
                    render_ctx,
                    module_commands,
                    params_ref,
                ) {
                    error!("Popup toggle failed: {}", error);
                }
            }
        }
        state.lock().unwrap().frame_ready = true;
    } else {
        let _ = spawn_action(action_str, axis_x, axis_y);
    }
}

pub fn spawn_socket_listener(
    action_tx: tokio::sync::mpsc::Sender<String>,
) -> Result<(tokio::task::JoinHandle<()>, PathBuf)> {
    let sock_path = shell_socket_path();
    if sock_path.exists() {
        if std::os::unix::net::UnixStream::connect(&sock_path).is_ok() {
            anyhow::bail!(
                "Another instance of wyrd-shell is already running and listening on {}",
                sock_path.display()
            );
        }
        let _ = std::fs::remove_file(&sock_path);
    }
    let socket_listener = tokio::net::UnixListener::bind(&sock_path).context(format!(
        "Failed to bind wyrd-shell UNIX control socket at {}",
        sock_path.display()
    ))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600));
    }
    let sock_path_clone = sock_path.clone();

    let task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        while let Ok((stream, _)) = socket_listener.accept().await {
            let tx = action_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let _ = tx.send(trimmed.to_string()).await;
                    }
                }
            });
        }
    });

    Ok((task, sock_path_clone))
}
