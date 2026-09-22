use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path as StdPath, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1;
use wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::ExtWorkspaceManagerV1;
pub use wyrd_engine::compositor::{
    create_compositor_integration, CompositorChoice, CompositorIntegration, KeyboardInfo,
    ToplevelInfo, WindowInfo, WindowStatus, WorkspaceApp, WorkspaceInfo,
};

#[derive(Default)]
struct WorkspaceProtocolState {
    handles: std::collections::HashMap<String, ExtWorkspaceHandleV1>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WorkspaceProtocolState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for WorkspaceProtocolState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtWorkspaceManagerV1,
        _event: wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }

    wayland_client::event_created_child!(WorkspaceProtocolState, ExtWorkspaceManagerV1, [
        0 => (wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1, ()),
        1 => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1, ()> for WorkspaceProtocolState {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
        _event: wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}

    wayland_client::event_created_child!(WorkspaceProtocolState, wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1, [
        0 => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for WorkspaceProtocolState {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceHandleV1,
        event: wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::Event::Id { id }
            | wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::Event::Name { name: id } => {
                state.handles.insert(id, proxy.clone());
            }
            _ => {}
        }
    }
}

fn activate_standard_workspace(id: &str) -> bool {
    let Ok(connection) = Connection::connect_to_env() else {
        return false;
    };
    let Ok((globals, mut queue)) = registry_queue_init::<WorkspaceProtocolState>(&connection)
    else {
        return false;
    };
    let qh = queue.handle();
    let Ok(manager) = globals.bind::<ExtWorkspaceManagerV1, _, _>(&qh, 1..=1, ()) else {
        return false;
    };
    let mut state = WorkspaceProtocolState::default();
    if queue.roundtrip(&mut state).is_err() {
        return false;
    }
    let Some(handle) = state.handles.get(id) else {
        return false;
    };
    handle.activate();
    manager.commit();
    connection.flush().is_ok() && queue.roundtrip(&mut state).is_ok()
}

#[derive(Parser, Debug)]
#[command(name = "wyrd-windows", about = "Wyrd Windows Daemon")]
struct Cli {
    #[arg(short, long)]
    socket_path: Option<PathBuf>,
}

pub fn deduce_short_layout_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "EN".to_string();
    }
    if let Some(start) = trimmed.find('(') {
        if let Some(end) = trimmed[start..].find(')') {
            let inner = trimmed[start + 1..start + end].trim();
            if !inner.is_empty() {
                return inner.to_uppercase();
            }
        }
    }
    match trimmed.to_lowercase().as_str() {
        "english" | "english (us)" | "us" => "US".to_string(),
        "russian" | "ru" => "RU".to_string(),
        "ukrainian" | "ua" => "UA".to_string(),
        "german" | "de" | "deutsch" => "DE".to_string(),
        "french" | "fr" | "français" => "FR".to_string(),
        "spanish" | "es" | "español" => "ES".to_string(),
        "italian" | "it" | "italiano" => "IT".to_string(),
        "polish" | "pl" | "polski" => "PL".to_string(),
        "portuguese" | "pt" | "português" => "PT".to_string(),
        "japanese" | "ja" | "jp" => "JP".to_string(),
        "chinese" | "zh" | "cn" => "CN".to_string(),
        "korean" | "ko" | "kr" => "KR".to_string(),
        "turkish" | "tr" | "türkçe" => "TR".to_string(),
        "swedish" | "sv" | "svenska" => "SE".to_string(),
        "norwegian" | "no" | "norsk" => "NO".to_string(),
        "danish" | "da" | "dansk" => "DK".to_string(),
        "finnish" | "fi" | "suomi" => "FI".to_string(),
        "czech" | "cs" | "čeština" => "CZ".to_string(),
        s if s.len() <= 3 => s.to_uppercase(),
        s => s.chars().take(2).collect::<String>().to_uppercase(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WindowSocketRequest {
    cmd: String,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Clone)]
struct WindowService {
    state: Arc<RwLock<WindowStatus>>,
    subscribers: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<String>>>>,
}

impl WindowService {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(WindowStatus::default())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn status(&self) -> WindowStatus {
        self.state.read().unwrap().clone()
    }

    fn set_status(&self, status: WindowStatus) {
        *self.state.write().unwrap() = status;
        let message = serde_json::json!({"event": "changed"}).to_string();
        self.subscribers
            .lock()
            .unwrap()
            .retain(|tx| tx.send(message.clone()).is_ok());
    }
}

fn query_status(compositor: &dyn CompositorIntegration) -> WindowStatus {
    compositor.query_status()
}

fn start_backend(service: Arc<WindowService>, compositor: Arc<dyn CompositorIntegration>) {
    std::thread::spawn(move || {
        let comp = compositor.clone();
        let comp_inner = compositor.clone();
        let s = service.clone();
        s.set_status(query_status(&*comp));

        comp.run_event_loop(&move || {
            s.set_status(query_status(&*comp_inner));
        });
    });
}

fn socket_path(cli_override: Option<&StdPath>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wyrd-windows.sock")
    } else {
        PathBuf::from("/tmp/wyrd-windows.sock")
    }
}

async fn handle_client(
    stream: UnixStream,
    service: Arc<WindowService>,
    compositor: Arc<dyn CompositorIntegration>,
) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let req: WindowSocketRequest = match serde_json::from_str(trimmed) {
                    Ok(value) => value,
                    Err(_) => {
                        write_half
                            .write_all(b"{\"error\":\"invalid_json\"}\n")
                            .await?;
                        continue;
                    }
                };

                let response = match req.cmd.as_str() {
                    "status" => serde_json::to_string(&service.status()).unwrap(),
                    "active" | "active_window" => {
                        let status = service.status();
                        serde_json::json!({ "active": status.active }).to_string()
                    }
                    "list" | "windows" => {
                        let status = service.status();
                        serde_json::json!({ "list": status.list }).to_string()
                    }
                    "workspaces" => serde_json::to_string(
                        &serde_json::json!({ "workspaces": service.status().workspaces }),
                    )
                    .unwrap(),
                    "subscribe" => {
                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                        service.subscribers.lock().unwrap().push(tx);
                        write_half
                            .write_all(b"{\"status\":\"subscribed\"}\n")
                            .await?;
                        while let Some(event) = rx.recv().await {
                            write_half.write_all(event.as_bytes()).await?;
                            write_half.write_all(b"\n").await?;
                        }
                        break;
                    }
                    "switch_workspace" => {
                        let id = req.id.clone().unwrap_or_default();
                        let result = if activate_standard_workspace(&id) {
                            serde_json::json!({"status": "ok", "backend": "ext-workspace-v1"})
                        } else {
                            match compositor.switch_workspace(&id) {
                                Ok(()) => {
                                    serde_json::json!({"status": "ok", "backend": compositor.name()})
                                }
                                Err(e) => {
                                    let err_str = e.to_string();
                                    if err_str.contains("unsupported") {
                                        serde_json::json!({"status": "unsupported"})
                                    } else {
                                        serde_json::json!({"status": "error"})
                                    }
                                }
                            }
                        };
                        result.to_string()
                    }
                    "keyboard" | "layout" => {
                        let status = service.status();
                        serde_json::json!({ "keyboard": status.keyboard }).to_string()
                    }
                    "switch_layout" => {
                        let target = req.id.clone().unwrap_or_else(|| "next".to_string());
                        let result = match compositor.switch_keyboard_layout(&target) {
                            Ok(()) => {
                                serde_json::json!({"status": "ok", "backend": compositor.name()})
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("unsupported") {
                                    serde_json::json!({"status": "unsupported"})
                                } else {
                                    serde_json::json!({"status": "error"})
                                }
                            }
                        };
                        result.to_string()
                    }
                    "exit" | "quit" => {
                        let result = match compositor.exit() {
                            Ok(()) => {
                                serde_json::json!({"status": "ok", "backend": compositor.name()})
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("unsupported") {
                                    serde_json::json!({"status": "unsupported"})
                                } else {
                                    serde_json::json!({"status": "error", "message": err_str})
                                }
                            }
                        };
                        result.to_string()
                    }
                    _ => serde_json::json!({ "error": "unknown_command" }).to_string(),
                };

                write_half.write_all(response.as_bytes()).await?;
                write_half.write_all(b"\n").await?;
            }
            Err(_) => break,
        }
    }

    Ok(())
}

async fn run_socket_server(
    service: Arc<WindowService>,
    compositor: Arc<dyn CompositorIntegration>,
    socket_override: Option<&StdPath>,
) -> Result<()> {
    let socket = socket_path(socket_override);
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
    }

    let listener =
        UnixListener::bind(&socket).context("failed to bind wyrd-windows UNIX socket")?;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket, permissions)
        .context("failed to set wyrd-windows socket permissions")?;

    loop {
        let (stream, _) = listener.accept().await?;
        let service_clone = service.clone();
        let compositor_clone = compositor.clone();
        tokio::spawn(async move {
            let _ = handle_client(stream, service_clone, compositor_clone).await;
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let compositor = create_compositor_integration(CompositorChoice::Auto);
    let service = Arc::new(WindowService::new());
    start_backend(service.clone(), compositor.clone());
    run_socket_server(service, compositor, cli.socket_path.as_deref()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_service_state_management() {
        let service = WindowService::new();
        assert!(service.status().active.is_none());
        assert!(service.status().list.is_empty());
        assert!(service.status().workspaces.is_empty());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        service.subscribers.lock().unwrap().push(tx);

        let test_status = WindowStatus {
            active: Some(ToplevelInfo {
                title: "Editor".to_string(),
                app_id: "code".to_string(),
                active: true,
                maximized: false,
                minimized: false,
                fullscreen: false,
            }),
            list: vec![ToplevelInfo {
                title: "Editor".to_string(),
                app_id: "code".to_string(),
                active: true,
                maximized: false,
                minimized: false,
                fullscreen: false,
            }],
            workspaces: vec![WorkspaceInfo {
                id: "1".to_string(),
                name: "dev".to_string(),
                monitor: "DP-1".to_string(),
                active: true,
                apps: vec![WorkspaceApp {
                    app_id: "code".to_string(),
                    title: "Editor".to_string(),
                }],
            }],
            keyboard: None,
        };

        service.set_status(test_status);

        let current = service.status();
        assert_eq!(current.list.len(), 1);
        assert_eq!(current.workspaces.len(), 1);
        assert_eq!(current.active.as_ref().unwrap().app_id, "code");

        // Verify subscriber received event
        let event = rx.try_recv().expect("subscriber should receive event");
        assert!(event.contains("changed"));
    }

    #[test]
    fn test_window_socket_request_deserialization() {
        let req: WindowSocketRequest = serde_json::from_str(r#"{"cmd":"status"}"#).unwrap();
        assert_eq!(req.cmd, "status");
        assert_eq!(req.id, None);

        let req: WindowSocketRequest = serde_json::from_str(r#"{"cmd":"active"}"#).unwrap();
        assert_eq!(req.cmd, "active");

        let req: WindowSocketRequest =
            serde_json::from_str(r#"{"cmd":"switch_workspace","id":"2"}"#).unwrap();
        assert_eq!(req.cmd, "switch_workspace");
        assert_eq!(req.id, Some("2".to_string()));
    }

    #[test]
    fn test_socket_path_resolution() {
        let custom = StdPath::new("/tmp/custom_windows.sock");
        assert_eq!(socket_path(Some(custom)), custom);

        let default_path = socket_path(None);
        assert!(default_path.ends_with("wyrd-windows.sock"));
    }

    #[test]
    fn test_window_status_serialization_roundtrip() {
        let status = WindowStatus {
            active: Some(ToplevelInfo {
                title: "Firefox".to_string(),
                app_id: "firefox".to_string(),
                active: true,
                maximized: true,
                minimized: false,
                fullscreen: false,
            }),
            list: vec![],
            workspaces: vec![],
            keyboard: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        let decoded: WindowStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.active.unwrap().title, "Firefox");
    }

    #[test]
    fn test_deduce_short_layout_name() {
        assert_eq!(deduce_short_layout_name("English (US)"), "US");
        assert_eq!(deduce_short_layout_name("Russian"), "RU");
        assert_eq!(deduce_short_layout_name("Ukrainian"), "UA");
        assert_eq!(deduce_short_layout_name("German"), "DE");
        assert_eq!(deduce_short_layout_name("us"), "US");
        assert_eq!(deduce_short_layout_name(""), "EN");
    }

    #[test]
    fn test_keyboard_socket_request_deserialization() {
        let req: WindowSocketRequest =
            serde_json::from_str(r#"{"cmd":"switch_layout","id":"next"}"#).unwrap();
        assert_eq!(req.cmd, "switch_layout");
        assert_eq!(req.id, Some("next".to_string()));

        let req: WindowSocketRequest =
            serde_json::from_str(r#"{"cmd":"switch_layout","id":"1"}"#).unwrap();
        assert_eq!(req.cmd, "switch_layout");
        assert_eq!(req.id, Some("1".to_string()));

        let req: WindowSocketRequest = serde_json::from_str(r#"{"cmd":"keyboard"}"#).unwrap();
        assert_eq!(req.cmd, "keyboard");
    }

    #[test]
    fn test_window_status_with_keyboard_serialization_roundtrip() {
        let status = WindowStatus {
            active: None,
            list: vec![],
            workspaces: vec![],
            keyboard: Some(KeyboardInfo {
                layout: "Russian".to_string(),
                short_name: "RU".to_string(),
                variant: "".to_string(),
                index: 1,
                layouts: vec!["English (US)".to_string(), "Russian".to_string()],
                short_layouts: vec!["US".to_string(), "RU".to_string()],
                device_name: "at-translated-set-2-keyboard".to_string(),
            }),
        };

        let json = serde_json::to_string(&status).unwrap();
        let decoded: WindowStatus = serde_json::from_str(&json).unwrap();
        let kb = decoded.keyboard.unwrap();
        assert_eq!(kb.short_name, "RU");
        assert_eq!(kb.index, 1);
        assert_eq!(kb.short_layouts, vec!["US", "RU"]);
    }
}
