use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path as StdPath, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

type Subscribers = Arc<parking_lot::Mutex<Vec<tokio::sync::mpsc::Sender<String>>>>;

#[derive(Parser, Debug)]
#[command(name = "wyrd-notifications", about = "Wyrd Notification Daemon")]
struct Cli {
    #[arg(short, long)]
    socket_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationRecord {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub image_path: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationStatus {
    pub notifications: Vec<NotificationRecord>,
    pub count: usize,
    pub dnd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationSocketRequest {
    cmd: String,
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    reason: Option<u32>,
    #[serde(default)]
    dnd: Option<bool>,
}

#[derive(Clone)]
struct NotificationService {
    state: Arc<RwLock<NotificationStatus>>,
}

impl NotificationService {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(NotificationStatus::default())),
        }
    }

    fn status(&self) -> NotificationStatus {
        self.state.read().unwrap().clone()
    }

    fn push(&self, record: NotificationRecord) {
        let mut state = self.state.write().unwrap();
        state.notifications.insert(0, record);
        state.notifications.truncate(32);
        state.count = state.notifications.len();
    }

    fn dismiss(&self, id: u32, reason: u32) -> bool {
        let mut state = self.state.write().unwrap();
        let before = state.notifications.len();
        state.notifications.retain(|n| n.id != id);
        let removed = before != state.notifications.len();
        state.count = state.notifications.len();
        let _ = reason;
        removed
    }

    fn clear(&self) {
        let mut state = self.state.write().unwrap();
        state.notifications.clear();
        state.count = 0;
    }

    fn set_dnd(&self, enabled: bool) {
        let mut state = self.state.write().unwrap();
        state.dnd = enabled;
    }
}

struct NotificationDbus {
    service: Arc<NotificationService>,
    next_id: Arc<std::sync::atomic::AtomicU32>,
    subscribers: Subscribers,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDbus {
    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "wyrd-notifications".into(),
            "Wyrd Project".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.2".into(),
        )
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into(), "persistence".into()]
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        _replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, zbus::zvariant::OwnedValue>,
        _expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let image_path = hints
            .get("image-path")
            .and_then(|value| value.downcast_ref::<String>().ok())
            .or_else(|| {
                hints
                    .get("image_path")
                    .and_then(|value| value.downcast_ref::<String>().ok())
            });
        self.service.push(NotificationRecord {
            id,
            app_name,
            summary,
            body,
            icon: app_icon,
            image_path,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });
        let message = serde_json::json!({
            "event": "notification.new",
            "notification": self.service.status().notifications.first(),
        })
        .to_string();
        self.subscribers
            .lock()
            .retain(|client| client.try_send(message.clone()).is_ok());
        Ok(id)
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        self.service.dismiss(id, 2);
        Ok(())
    }
}

async fn start_dbus_service(
    service: Arc<NotificationService>,
    subscribers: Subscribers,
) -> Result<zbus::Connection> {
    let dbus_service = NotificationDbus {
        service,
        next_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        subscribers,
    };
    let connection = zbus::connection::Builder::session()?
        .serve_at("/org/freedesktop/Notifications", dbus_service)?
        .build()
        .await
        .context("failed to establish session D-Bus notification service")?;
    let flags = RequestNameFlags::ReplaceExisting | RequestNameFlags::DoNotQueue;
    match connection
        .request_name_with_flags("org.freedesktop.Notifications", flags)
        .await
    {
        Ok(RequestNameReply::PrimaryOwner) | Ok(RequestNameReply::AlreadyOwner) => {}
        Ok(RequestNameReply::Exists) | Ok(RequestNameReply::InQueue) => {
            log::warn!(
                "org.freedesktop.Notifications is already owned; daemon socket remains available"
            );
        }
        Err(error) => log::warn!("could not claim org.freedesktop.Notifications: {error}"),
    }
    Ok(connection)
}

fn socket_path(cli_override: Option<&StdPath>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wyrd-notifications.sock")
    } else {
        PathBuf::from("/tmp/wyrd-notifications.sock")
    }
}

async fn handle_client(
    stream: UnixStream,
    service: Arc<NotificationService>,
    subscribers: Subscribers,
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

                let req: NotificationSocketRequest = match serde_json::from_str(trimmed) {
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
                    "list" => serde_json::to_string(&service.status()).unwrap(),
                    "dismiss" => {
                        let id = req.id.unwrap_or(0);
                        let reason = req.reason.unwrap_or(2);
                        let ok = service.dismiss(id, reason);
                        serde_json::json!({ "status": if ok { "ok" } else { "error" }, "id": id, "reason": reason }).to_string()
                    }
                    "clear" => {
                        service.clear();
                        serde_json::json!({ "status": "ok" }).to_string()
                    }
                    "dnd" => {
                        let enabled = req.dnd.unwrap_or(false);
                        service.set_dnd(enabled);
                        serde_json::json!({ "status": "ok", "dnd": enabled }).to_string()
                    }
                    "subscribe" => {
                        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
                        subscribers.lock().push(tx);
                        write_half
                            .write_all(b"{\"status\":\"subscribed\"}\n")
                            .await?;
                        while let Some(event) = rx.recv().await {
                            write_half.write_all(event.as_bytes()).await?;
                            write_half.write_all(b"\n").await?;
                        }
                        break;
                    }
                    "notify" => {
                        let id = req.id.unwrap_or(1);
                        let record = NotificationRecord {
                            id,
                            app_name: "daemon".to_string(),
                            summary: "Notification".to_string(),
                            body: "Queued via socket".to_string(),
                            icon: String::new(),
                            image_path: None,
                            timestamp_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        };
                        service.push(record.clone());
                        serde_json::to_string(&record).unwrap()
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
    service: Arc<NotificationService>,
    socket_override: Option<&StdPath>,
    subscribers: Subscribers,
) -> Result<()> {
    let socket = socket_path(socket_override);
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
    }

    let listener =
        UnixListener::bind(&socket).context("failed to bind wyrd-notifications UNIX socket")?;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket, permissions)
        .context("failed to set wyrd-notifications socket permissions")?;

    loop {
        let (stream, _) = listener.accept().await?;
        let service_clone = service.clone();
        let subscribers_clone = subscribers.clone();
        tokio::spawn(async move {
            let _ = handle_client(stream, service_clone, subscribers_clone).await;
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let service = Arc::new(NotificationService::new());
    let subscribers: Subscribers = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let _dbus_connection = start_dbus_service(service.clone(), subscribers.clone()).await?;
    run_socket_server(service, cli.socket_path.as_deref(), subscribers).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_service_push_and_cap() {
        let service = NotificationService::new();
        assert_eq!(service.status().count, 0);

        // Push 40 notifications
        for i in 1..=40 {
            service.push(NotificationRecord {
                id: i,
                app_name: "test-app".to_string(),
                summary: format!("Summary {i}"),
                body: format!("Body {i}"),
                icon: "dialog-information".to_string(),
                image_path: None,
                timestamp_ms: i as u64,
            });
        }

        let status = service.status();
        // Capped at 32 notifications
        assert_eq!(status.count, 32);
        assert_eq!(status.notifications.len(), 32);
        // Newest is at index 0 (id 40)
        assert_eq!(status.notifications[0].id, 40);
        // Oldest kept is at index 31 (id 9)
        assert_eq!(status.notifications[31].id, 9);
    }

    #[test]
    fn test_notification_service_dismiss() {
        let service = NotificationService::new();
        for id in [10, 20, 30] {
            service.push(NotificationRecord {
                id,
                app_name: "test".to_string(),
                summary: format!("Item {id}"),
                body: String::new(),
                icon: String::new(),
                image_path: None,
                timestamp_ms: id as u64,
            });
        }

        assert_eq!(service.status().count, 3);
        // Dismiss middle item
        assert!(service.dismiss(20, 2));
        assert_eq!(service.status().count, 2);
        let ids: Vec<u32> = service
            .status()
            .notifications
            .iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(ids, vec![30, 10]);

        // Dismiss non-existent item returns false
        assert!(!service.dismiss(999, 2));
        assert_eq!(service.status().count, 2);
    }

    #[test]
    fn test_notification_service_clear_and_dnd() {
        let service = NotificationService::new();
        service.push(NotificationRecord {
            id: 1,
            app_name: "app".to_string(),
            summary: "hi".to_string(),
            body: "".to_string(),
            icon: "".to_string(),
            image_path: None,
            timestamp_ms: 100,
        });
        assert_eq!(service.status().count, 1);

        service.clear();
        assert_eq!(service.status().count, 0);
        assert!(service.status().notifications.is_empty());

        assert!(!service.status().dnd);
        service.set_dnd(true);
        assert!(service.status().dnd);
        service.set_dnd(false);
        assert!(!service.status().dnd);
    }

    #[test]
    fn test_socket_path_resolution() {
        let custom = StdPath::new("/tmp/custom_notif.sock");
        assert_eq!(socket_path(Some(custom)), custom);

        let default_path = socket_path(None);
        assert!(default_path.ends_with("wyrd-notifications.sock"));
    }

    #[test]
    fn test_notification_socket_request_deserialization() {
        let req: NotificationSocketRequest = serde_json::from_str(r#"{"cmd":"status"}"#).unwrap();
        assert_eq!(req.cmd, "status");
        assert_eq!(req.id, None);

        let req: NotificationSocketRequest =
            serde_json::from_str(r#"{"cmd":"dismiss","id":42,"reason":3}"#).unwrap();
        assert_eq!(req.cmd, "dismiss");
        assert_eq!(req.id, Some(42));
        assert_eq!(req.reason, Some(3));

        let req: NotificationSocketRequest =
            serde_json::from_str(r#"{"cmd":"dnd","dnd":true}"#).unwrap();
        assert_eq!(req.cmd, "dnd");
        assert_eq!(req.dnd, Some(true));
    }
}
