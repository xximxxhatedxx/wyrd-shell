use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path as StdPath, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;
use zbus::object_server::SignalEmitter;

#[derive(Parser, Debug)]
#[command(name = "wyrd-tray", about = "Wyrd Tray Daemon")]
struct Cli {
    #[arg(short, long)]
    socket_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrayItem {
    pub id: String,
    pub service: String,
    pub path: String,
    pub menu: String,
    pub title: String,
    pub image: Option<String>,
    pub item_is_menu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrayStatus {
    pub items: Vec<TrayItem>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraySocketRequest {
    cmd: String,
    #[serde(default)]
    item: Option<TrayItem>,
    #[serde(default)]
    items: Option<Vec<TrayItem>>,
}

#[derive(Clone)]
struct TrayService {
    state: Arc<RwLock<TrayStatus>>,
}

struct StatusNotifierWatcher {
    registered: Arc<RwLock<Vec<String>>>,
}

struct FreedesktopWatcher {
    registered: Arc<RwLock<Vec<String>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        service: String,
    ) {
        let entry = if service.starts_with('/') {
            if let Some(sender) = hdr.sender() {
                format!("{}{}", sender, service)
            } else {
                service
            }
        } else {
            service
        };
        let is_new = {
            let mut registered = self.registered.write().unwrap();
            if !registered.contains(&entry) {
                registered.push(entry.clone());
                true
            } else {
                false
            }
        };
        if is_new {
            let _ = Self::status_notifier_item_registered(&emitter, &entry).await;
        }
    }

    async fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        self.registered.read().unwrap().clone()
    }

    #[zbus(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;
}

#[interface(name = "org.freedesktop.StatusNotifierWatcher")]
impl FreedesktopWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        service: String,
    ) {
        let entry = if service.starts_with('/') {
            if let Some(sender) = hdr.sender() {
                format!("{}{}", sender, service)
            } else {
                service
            }
        } else {
            service
        };
        let is_new = {
            let mut registered = self.registered.write().unwrap();
            if !registered.contains(&entry) {
                registered.push(entry.clone());
                true
            } else {
                false
            }
        };
        if is_new {
            let _ = Self::status_notifier_item_registered(&emitter, &entry).await;
        }
    }

    async fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        self.registered.read().unwrap().clone()
    }

    #[zbus(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;
}

impl TrayService {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(TrayStatus::default())),
        }
    }

    fn status(&self) -> TrayStatus {
        self.state.read().unwrap().clone()
    }

    fn set_items(&self, items: Vec<TrayItem>) {
        let mut state = self.state.write().unwrap();
        state.items = items;
        state.count = state.items.len();
    }
}

fn socket_path(cli_override: Option<&StdPath>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wyrd-tray.sock")
    } else {
        PathBuf::from("/tmp/wyrd-tray.sock")
    }
}

async fn handle_client(stream: UnixStream, service: Arc<TrayService>) -> Result<()> {
    let mut reader = BufReader::new(stream);
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

                let req: TraySocketRequest = match serde_json::from_str(trimmed) {
                    Ok(value) => value,
                    Err(_) => {
                        let stream = reader.get_mut();
                        stream.write_all(b"{\"error\":\"invalid_json\"}\n").await?;
                        continue;
                    }
                };

                let response = match req.cmd.as_str() {
                    "status" | "list" => serde_json::to_string(&service.status()).unwrap(),
                    "set_items" => {
                        let items: Vec<TrayItem> = if let Some(items) = req.items {
                            items
                        } else if let Some(item) = req.item {
                            vec![item]
                        } else {
                            Vec::new()
                        };
                        service.set_items(items);
                        serde_json::json!({ "status": "ok" }).to_string()
                    }
                    _ => serde_json::json!({ "error": "unknown_command" }).to_string(),
                };

                let stream = reader.get_mut();
                stream.write_all(response.as_bytes()).await?;
                stream.write_all(b"\n").await?;
            }
            Err(_) => break,
        }
    }

    Ok(())
}

async fn run_socket_server(
    service: Arc<TrayService>,
    socket_override: Option<&StdPath>,
) -> Result<()> {
    let socket = socket_path(socket_override);
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
    }

    let listener = UnixListener::bind(&socket).context("failed to bind wyrd-tray UNIX socket")?;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket, permissions)
        .context("failed to set wyrd-tray socket permissions")?;

    loop {
        let (stream, _) = listener.accept().await?;
        let service_clone = service.clone();
        tokio::spawn(async move {
            let _ = handle_client(stream, service_clone).await;
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let service = Arc::new(TrayService::new());
    let registered = Arc::new(RwLock::new(Vec::new()));
    let reg_task = registered.clone();
    let watcher = StatusNotifierWatcher { registered };
    let fdo_watcher = FreedesktopWatcher {
        registered: reg_task.clone(),
    };
    let connection = zbus::connection::Builder::session()?
        .serve_at("/StatusNotifierWatcher", watcher)?
        .serve_at("/StatusNotifierWatcher", fdo_watcher)?
        .build()
        .await
        .context("failed to create tray D-Bus connection")?;
    let flags = RequestNameFlags::ReplaceExisting | RequestNameFlags::DoNotQueue;
    for name in [
        "org.kde.StatusNotifierWatcher",
        "org.freedesktop.StatusNotifierWatcher",
    ] {
        match connection.request_name_with_flags(name, flags).await {
            Ok(RequestNameReply::PrimaryOwner) | Ok(RequestNameReply::AlreadyOwner) => {}
            Ok(RequestNameReply::Exists) | Ok(RequestNameReply::InQueue) => {
                log::warn!("D-Bus name {name} is already owned")
            }
            Err(error) => log::warn!("could not claim D-Bus name {name}: {error}"),
        }
    }

    let conn_task = connection.clone();
    tokio::spawn(async move {
        use futures_util::StreamExt;
        if let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(&conn_task).await {
            if let Ok(mut name_owner_changed) = dbus_proxy.receive_name_owner_changed().await {
                while let Some(signal) = name_owner_changed.next().await {
                    let Ok(args) = signal.args() else { continue };
                    let new_owner = args.new_owner;
                    if new_owner.as_ref().is_none() {
                        let name_str = args.name.as_str();
                        let dead_items: Vec<String> = {
                            let mut reg = reg_task.write().unwrap();
                            let mut removed = Vec::new();
                            reg.retain(|item| {
                                let s = item.split('/').next().unwrap_or(item);
                                if s == name_str || s.starts_with(name_str) {
                                    removed.push(item.clone());
                                    false
                                } else {
                                    true
                                }
                            });
                            removed
                        };
                        for item in dead_items {
                            log::info!(
                                "StatusNotifierItem unregistered on name lost {}: {}",
                                name_str,
                                item
                            );
                            if let Ok(emitter) =
                                SignalEmitter::new(&conn_task, "/StatusNotifierWatcher")
                            {
                                let _ = StatusNotifierWatcher::status_notifier_item_unregistered(
                                    &emitter, &item,
                                )
                                .await;
                                let _ = FreedesktopWatcher::status_notifier_item_unregistered(
                                    &emitter, &item,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
    });

    run_socket_server(service, cli.socket_path.as_deref()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_service_state_and_items() {
        let service = TrayService::new();
        assert_eq!(service.status().count, 0);
        assert!(service.status().items.is_empty());

        let item = TrayItem {
            id: "org.telegram.desktop".to_string(),
            service: ":1.42".to_string(),
            path: "/StatusNotifierItem".to_string(),
            menu: "/MenuBar".to_string(),
            title: "Telegram".to_string(),
            image: Some("/tmp/telegram.png".to_string()),
            item_is_menu: false,
        };

        service.set_items(vec![item.clone()]);
        let status = service.status();
        assert_eq!(status.count, 1);
        assert_eq!(status.items[0].id, "org.telegram.desktop");
        assert_eq!(status.items[0].title, "Telegram");
    }

    #[test]
    fn test_tray_socket_request_deserialization() {
        let req: TraySocketRequest = serde_json::from_str(r#"{"cmd":"status"}"#).unwrap();
        assert_eq!(req.cmd, "status");
        assert!(req.item.is_none());

        let req: TraySocketRequest = serde_json::from_str(
            r#"{"cmd":"set_items","item":{"id":"test","service":":1.1","path":"/item","menu":"","title":"Test","item_is_menu":false}}"#,
        )
        .unwrap();
        assert_eq!(req.cmd, "set_items");
        assert_eq!(req.item.unwrap().title, "Test");

        let req: TraySocketRequest = serde_json::from_str(
            r#"{"cmd":"set_items","items":[{"id":"test1","service":":1.1","path":"/item","menu":"","title":"Test1","item_is_menu":false},{"id":"test2","service":":1.2","path":"/item","menu":"","title":"Test2","item_is_menu":false}]}"#,
        )
        .unwrap();
        assert_eq!(req.cmd, "set_items");
        assert_eq!(req.items.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_socket_path_resolution() {
        let custom = StdPath::new("/tmp/custom_tray.sock");
        assert_eq!(socket_path(Some(custom)), custom);

        let default_path = socket_path(None);
        assert!(default_path.ends_with("wyrd-tray.sock"));
    }

    #[test]
    fn test_tray_item_serialization_roundtrip() {
        let item = TrayItem {
            id: "steam".to_string(),
            service: ":1.99".to_string(),
            path: "/StatusNotifierItem".to_string(),
            menu: "/Menu".to_string(),
            title: "Steam".to_string(),
            image: None,
            item_is_menu: true,
        };
        let json = serde_json::to_string(&item).unwrap();
        let decoded: TrayItem = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "steam");
        assert!(decoded.item_is_menu);
    }
}
