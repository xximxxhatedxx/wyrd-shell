//! Wyrd Wayland Clipboard History Daemon.
//!
//! Standalone daemon providing Wayland data-control clipboard monitoring,
//! disk persistence, and a Unix domain socket control API.

use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, mpsc};
use wayland_client::{
    backend::ObjectId,
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_seat,
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
};

pub const MAX_ENTRIES: usize = 50;
pub const MAX_IMAGE_ENTRIES: usize = 16;

#[derive(Parser, Debug)]
#[command(
    name = "wyrd-clipboard",
    about = "Wyrd Wayland Clipboard History Daemon"
)]
struct Cli {
    /// Override Unix domain socket path
    #[arg(short, long)]
    socket_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: u64,
    pub text: String,
    pub mime: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    pub id: u64,
    pub path: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardEntryPreview {
    pub id: u64,
    pub kind: String,
    pub text_preview: String,
    pub mime: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardState {
    pub entries: VecDeque<ClipboardEntry>,
    pub images: VecDeque<ImageEntry>,
    pub active_selection: Option<String>,
    next_id: u64,
}

impl ClipboardState {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_ENTRIES),
            images: VecDeque::with_capacity(MAX_IMAGE_ENTRIES),
            active_selection: None,
            next_id: 1,
        }
    }

    pub fn load_from_entries(&mut self, mut loaded: Vec<ClipboardEntry>) {
        if loaded.len() > MAX_ENTRIES {
            loaded.truncate(MAX_ENTRIES);
        }
        let max_id = loaded.iter().map(|e| e.id).max().unwrap_or(0);
        self.next_id = max_id.wrapping_add(1);
        self.entries = VecDeque::from(loaded);
    }

    pub fn add_text(&mut self, text: String, mime: String) -> Option<u64> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Deduplicate if identical to the most recent entry
        if let Some(front) = self.entries.front() {
            if front.text == text {
                return Some(front.id);
            }
        }
        // Remove prior duplicate so it moves to front
        self.entries.retain(|e| e.text != text);

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.entries.push_front(ClipboardEntry {
            id,
            text: text.clone(),
            mime,
            timestamp,
        });

        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop_back();
        }

        self.active_selection = Some(text);
        Some(id)
    }

    pub fn add_image(&mut self, path: String, bytes: Vec<u8>) -> u64 {
        self.images.retain(|img| img.path != path);
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.images.push_front(ImageEntry {
            id,
            path,
            bytes,
            timestamp,
        });

        if self.images.len() > MAX_IMAGE_ENTRIES {
            self.images.pop_back();
        }

        id
    }

    pub fn get_entries(&self) -> Vec<ClipboardEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn get_entry_by_id(&self, id: u64) -> Option<ClipboardEntry> {
        self.entries.iter().find(|e| e.id == id).cloned()
    }

    pub fn get_image_by_id(&self, id: u64) -> Option<ImageEntry> {
        self.images.iter().find(|i| i.id == id).cloned()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.images.clear();
        self.active_selection = None;
    }
}

pub type SharedClipboard = Arc<Mutex<ClipboardState>>;

fn get_history_file_path() -> PathBuf {
    let base_dir = if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(state_home).join("wyrd-clipboard")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".local/state/wyrd-clipboard")
    } else {
        PathBuf::from("/tmp/wyrd-clipboard")
    };
    let _ = std::fs::create_dir_all(&base_dir);
    base_dir.join("history.jsonl")
}

fn load_history_from_disk(path: &Path) -> Vec<ClipboardEntry> {
    use std::io::BufRead;
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<ClipboardEntry>(&line) {
            entries.push(entry);
        }
    }
    entries
}

fn save_history_to_disk(path: &Path, entries: &[ClipboardEntry]) {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut temp_path = path.to_path_buf();
    temp_path.set_extension("tmp");

    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp_path)
    else {
        return;
    };

    let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));

    for entry in entries {
        if let Ok(line) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{}", line);
        }
    }
    let _ = file.flush();
    drop(file);

    let _ = std::fs::rename(&temp_path, path);
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
}

fn get_socket_path(cli_override: Option<&Path>) -> PathBuf {
    if let Some(p) = cli_override {
        return p.to_path_buf();
    }
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wyrd-clipboard.sock")
    } else {
        PathBuf::from("/tmp/wyrd-clipboard.sock")
    }
}

#[derive(Debug, Clone)]
pub enum DataSourceData {
    Text(String),
    Image(Vec<u8>),
}

#[derive(Debug)]
pub enum WaylandCommand {
    Select(String),
    SelectImage(Vec<u8>),
    Clear,
}

pub struct DaemonWaylandState {
    pub clipboard: SharedClipboard,
    pub history_file_path: PathBuf,
    pub offers: HashMap<ObjectId, Vec<String>>,
    pub current_source: Option<ZwlrDataControlSourceV1>,
    /// Broadcast sender — fires "changed" whenever a new item is added.
    pub notify_tx: broadcast::Sender<()>,
}

// Delegate no-op for ZwlrDataControlManagerV1
wayland_client::delegate_noop!(DaemonWaylandState: ZwlrDataControlManagerV1);

impl Dispatch<wl_seat::WlSeat, ()> for DaemonWaylandState {
    fn event(
        _state: &mut Self,
        _seat: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, GlobalListContents>
    for DaemonWaylandState
{
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_registry::WlRegistry,
        _event: wayland_client::protocol::wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for DaemonWaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrDataControlDeviceV1,
        event: wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_device_v1::Event;
        match event {
            Event::DataOffer { id } => {
                state.offers.insert(id.id(), Vec::new());
            }
            Event::Selection { id: Some(offer) } => {
                let offer_id = offer.id();
                let mimes = state.offers.get(&offer_id).cloned().unwrap_or_default();

                // Skip entirely any offer whose mime list includes "x-kde-passwordManagerHint" or "secret"
                let is_secret = mimes.iter().any(|m| {
                    let lower = m.to_ascii_lowercase();
                    lower.contains("passwordmanagerhint") || lower.contains("secret")
                });

                if is_secret {
                    debug!("Skipping secret/password manager clipboard offer");
                    state.offers.remove(&offer_id);
                    offer.destroy();
                    return;
                }

                // Pick the best available text mime in order:
                // "text/plain;charset=utf-8", "text/plain", "UTF8_STRING"
                let chosen_mime = if mimes.iter().any(|m| m == "text/plain;charset=utf-8") {
                    Some("text/plain;charset=utf-8")
                } else if mimes.iter().any(|m| m == "text/plain") {
                    Some("text/plain")
                } else if mimes.iter().any(|m| m == "UTF8_STRING") {
                    Some("UTF8_STRING")
                } else {
                    None
                };

                let chosen_mime = match chosen_mime {
                    Some(m) => m,
                    None => {
                        debug!(
                            "Skipping clipboard offer with no supported text mime: {:?}",
                            mimes
                        );
                        state.offers.remove(&offer_id);
                        offer.destroy();
                        return;
                    }
                };

                let mut fds = [0i32; 2];
                // SAFETY: `fds` is a stack-allocated 2-element array passed as a valid mutable pointer to `pipe(2)`.
                // If `pipe` succeeds, both `fds[0]` and `fds[1]` are open, valid file descriptors.
                // Ownership of the read and write ends is immediately transferred to `File` and `OwnedFd`.
                if unsafe { libc::pipe(fds.as_mut_ptr()) } == 0 {
                    use std::os::fd::{AsFd, FromRawFd, OwnedFd};
                    let mut read_file = unsafe { std::fs::File::from_raw_fd(fds[0]) };
                    let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
                    offer.receive(chosen_mime.to_string(), write_fd.as_fd());
                    drop(write_fd);

                    let shared_clip = state.clipboard.clone();
                    let mime_string = chosen_mime.to_string();
                    let disk_path = state.history_file_path.clone();
                    let notify_tx = state.notify_tx.clone();

                    std::thread::spawn(move || {
                        use std::io::Read;
                        let mut text = String::new();
                        let _ = read_file.read_to_string(&mut text);
                        if !text.trim().is_empty() {
                            let mut clip = shared_clip.lock().unwrap();
                            if clip.add_text(text, mime_string).is_some() {
                                let entries = clip.get_entries();
                                drop(clip);
                                save_history_to_disk(&disk_path, &entries);
                                // Notify all subscribers that clipboard changed
                                let _ = notify_tx.send(());
                            }
                        }
                    });
                }
                state.offers.remove(&offer_id);
                offer.destroy();
            }
            Event::Selection { id: None } => {
                debug!("Compositor cleared clipboard selection");
            }
            Event::PrimarySelection { .. } => {}
            _ => {}
        }
    }

    wayland_client::event_created_child!(DaemonWaylandState, ZwlrDataControlDeviceV1, [
        0 => (ZwlrDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for DaemonWaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrDataControlOfferV1,
        event: wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1::Event;
        if let Event::Offer { mime_type } = event {
            state.offers.entry(proxy.id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<ZwlrDataControlSourceV1, DataSourceData> for DaemonWaylandState {
    fn event(
        _state: &mut Self,
        proxy: &ZwlrDataControlSourceV1,
        event: wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_source_v1::Event,
        data: &DataSourceData,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_source_v1::Event;
        match event {
            Event::Send { mime_type: _, fd } => {
                use std::io::Write;
                let mut file = std::fs::File::from(fd);
                match data {
                    DataSourceData::Text(text) => {
                        let _ = file.write_all(text.as_bytes());
                    }
                    DataSourceData::Image(bytes) => {
                        let _ = file.write_all(bytes);
                    }
                }
            }
            Event::Cancelled => {
                proxy.destroy();
            }
            _ => {}
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ClientCommand {
    List,
    Select { id: u64 },
    SetImage { path: String },
    Clear,
    Subscribe,
}

#[derive(Debug)]
struct WaylandFd(std::os::unix::io::RawFd);
impl std::os::unix::io::AsRawFd for WaylandFd {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.0
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    let history_path = get_history_file_path();
    info!("Clipboard history path: {:?}", history_path);

    let initial_entries = load_history_from_disk(&history_path);
    info!(
        "Loaded {} existing clipboard entries from disk",
        initial_entries.len()
    );

    let clipboard = Arc::new(Mutex::new(ClipboardState::new()));
    clipboard.lock().unwrap().load_from_entries(initial_entries);

    let socket_path = get_socket_path(cli.socket_path.as_deref());
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("Failed to bind Unix socket at {:?}", socket_path))?;
    let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
    info!("Unix socket server listening at {:?}", socket_path);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<WaylandCommand>();
    // Broadcast channel: Wayland clipboard events → all active subscribers
    let (notify_tx, _notify_rx) = broadcast::channel::<()>(16);

    // Wayland connection setup
    let conn = Connection::connect_to_env()
        .context("WAYLAND_DISPLAY not set; cannot connect to compositor")?;
    let (globals, mut event_queue) = registry_queue_init::<DaemonWaylandState>(&conn)
        .context("Failed to init Wayland registry queue")?;
    let qh = event_queue.handle();

    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=9, ())
        .context("wl_seat not available in compositor")?;

    let data_control_manager: ZwlrDataControlManagerV1 = globals
        .bind(&qh, 1..=2, ())
        .context("zwlr_data_control_manager_v1 not available in compositor")?;

    let data_control_device = data_control_manager.get_data_device(&seat, &qh, ());

    let mut wayland_state = DaemonWaylandState {
        clipboard: clipboard.clone(),
        history_file_path: history_path.clone(),
        offers: HashMap::new(),
        current_source: None,
        notify_tx: notify_tx.clone(),
    };

    use std::os::fd::AsRawFd;
    use tokio::io::unix::AsyncFd;
    let wayland_fd = WaylandFd(conn.backend().poll_fd().as_raw_fd());
    let async_fd = AsyncFd::new(wayland_fd)?;

    // Spawn Unix Socket Server Task
    let server_clipboard = clipboard.clone();
    let server_cmd_tx = cmd_tx.clone();
    let server_history_path = history_path.clone();
    let server_notify_tx = notify_tx.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let clip_ref = server_clipboard.clone();
                    let tx_ref = server_cmd_tx.clone();
                    let notify_tx_ref = server_notify_tx.clone();
                    let disk_path = server_history_path.clone();

                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut buf_reader = BufReader::new(reader);
                        let mut line = String::new();

                        while let Ok(n) = buf_reader.read_line(&mut line).await {
                            if n == 0 {
                                break;
                            }
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                line.clear();
                                continue;
                            }

                            match serde_json::from_str::<ClientCommand>(trimmed) {
                                Ok(ClientCommand::List) => {
                                    let (entries, images) = {
                                        let clip = clip_ref.lock().unwrap();
                                        (
                                            clip.get_entries(),
                                            clip.images.iter().cloned().collect::<Vec<_>>(),
                                        )
                                    };
                                    let mut previews: Vec<ClipboardEntryPreview> = Vec::new();
                                    for e in entries {
                                        let text_preview = if e.text.chars().count() > 120 {
                                            e.text.chars().take(120).collect::<String>()
                                        } else {
                                            e.text
                                        };
                                        previews.push(ClipboardEntryPreview {
                                            id: e.id,
                                            kind: "text".to_string(),
                                            text_preview,
                                            mime: e.mime,
                                            timestamp: e.timestamp,
                                        });
                                    }
                                    for img in images {
                                        previews.push(ClipboardEntryPreview {
                                            id: img.id,
                                            kind: "image".to_string(),
                                            text_preview: img.path.clone(),
                                            mime: "image/png".to_string(),
                                            timestamp: img.timestamp,
                                        });
                                    }
                                    previews.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
                                    let resp = serde_json::json!({ "entries": previews });
                                    let mut out = serde_json::to_string(&resp)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    out.push('\n');
                                    let _ = writer.write_all(out.as_bytes()).await;
                                }
                                Ok(ClientCommand::Select { id }) => {
                                    let (found_text, found_image) = {
                                        let clip = clip_ref.lock().unwrap();
                                        (clip.get_entry_by_id(id), clip.get_image_by_id(id))
                                    };
                                    if let Some(entry) = found_text {
                                        let _ = tx_ref.send(WaylandCommand::Select(entry.text));
                                        let _ = writer.write_all(b"{\"status\":\"ok\"}\n").await;
                                    } else if let Some(img) = found_image {
                                        let bytes = if img.bytes.is_empty() {
                                            std::fs::read(&img.path).unwrap_or_default()
                                        } else {
                                            img.bytes
                                        };
                                        let _ = tx_ref.send(WaylandCommand::SelectImage(bytes));
                                        let _ = writer.write_all(b"{\"status\":\"ok\"}\n").await;
                                    } else {
                                        let _ = writer.write_all(b"{\"status\":\"error\",\"message\":\"not_found\"}\n").await;
                                    }
                                }
                                Ok(ClientCommand::SetImage { path }) => {
                                    match std::fs::read(&path) {
                                        Ok(bytes) => {
                                            let id = clip_ref
                                                .lock()
                                                .unwrap()
                                                .add_image(path, bytes.clone());
                                            let _ = tx_ref.send(WaylandCommand::SelectImage(bytes));
                                            let resp =
                                                serde_json::json!({ "status": "ok", "id": id });
                                            let mut out = serde_json::to_string(&resp)
                                                .unwrap_or_else(|_| "{}".to_string());
                                            out.push('\n');
                                            let _ = writer.write_all(out.as_bytes()).await;
                                        }
                                        Err(e) => {
                                            let err_msg = format!("{{\"status\":\"error\",\"message\":\"failed to read image: {}\"}}\n", e);
                                            let _ = writer.write_all(err_msg.as_bytes()).await;
                                        }
                                    }
                                }
                                Ok(ClientCommand::Clear) => {
                                    clip_ref.lock().unwrap().clear();
                                    let dpath = disk_path.clone();
                                    tokio::task::spawn_blocking(move || {
                                        save_history_to_disk(&dpath, &[]);
                                    });
                                    let _ = tx_ref.send(WaylandCommand::Clear);
                                    // Also notify subscribers of the clear
                                    let _ = notify_tx_ref.send(());
                                    let _ = writer.write_all(b"{\"status\":\"ok\"}\n").await;
                                }
                                Ok(ClientCommand::Subscribe) => {
                                    // Acknowledge subscription
                                    let _ =
                                        writer.write_all(b"{\"status\":\"subscribed\"}\n").await;
                                    // Subscribe to the broadcast channel and stream events
                                    let mut rx = notify_tx_ref.subscribe();
                                    loop {
                                        match rx.recv().await {
                                            Ok(()) => {
                                                if writer
                                                    .write_all(b"{\"event\":\"changed\"}\n")
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                                if writer.flush().await.is_err() {
                                                    break;
                                                }
                                            }
                                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                                // Missed some events, send one notification to catch up
                                                if writer
                                                    .write_all(b"{\"event\":\"changed\"}\n")
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                                let _ = writer.flush().await;
                                            }
                                            Err(broadcast::error::RecvError::Closed) => break,
                                        }
                                    }
                                    return; // Connection done
                                }
                                Err(e) => {
                                    let err_msg =
                                        format!("{{\"status\":\"error\",\"message\":\"{}\"}}\n", e);
                                    let _ = writer.write_all(err_msg.as_bytes()).await;
                                }
                            }
                            line.clear();
                        }
                    });
                }
                Err(e) => {
                    warn!("Unix socket accept error: {}", e);
                }
            }
        }
    });

    info!("wyrd-clipboard daemon running");

    // Wayland event loop
    loop {
        if let Err(e) = conn.flush() {
            match e {
                wayland_client::backend::WaylandError::Io(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock => {}
                other => return Err(other.into()),
            }
        }

        tokio::select! {
            readable = async_fd.readable() => {
                let mut readable = readable?;
                if let Some(guard) = conn.prepare_read() {
                    if let Err(e) = guard.read() {
                        readable.clear_ready();
                        return Err(e.into());
                    }
                }
                readable.clear_ready();
                event_queue.dispatch_pending(&mut wayland_state)?;
            }
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    WaylandCommand::Select(text) => {
                        let source = data_control_manager.create_data_source(&qh, DataSourceData::Text(text));
                        source.offer("text/plain;charset=utf-8".to_string());
                        source.offer("text/plain".to_string());
                        source.offer("UTF8_STRING".to_string());
                        data_control_device.set_selection(Some(&source));
                        wayland_state.current_source = Some(source);
                        let _ = conn.flush();
                    }
                    WaylandCommand::SelectImage(bytes) => {
                        let source = data_control_manager.create_data_source(&qh, DataSourceData::Image(bytes));
                        source.offer("image/png".to_string());
                        data_control_device.set_selection(Some(&source));
                        wayland_state.current_source = Some(source);
                        let _ = conn.flush();
                    }
                    WaylandCommand::Clear => {
                        data_control_device.set_selection(None);
                        wayland_state.current_source = None;
                        let _ = conn.flush();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_state_dedup_and_move_to_front() {
        let mut state = ClipboardState::new();
        let id1 = state
            .add_text("First".to_string(), "text/plain".to_string())
            .unwrap();
        let id2 = state
            .add_text("Second".to_string(), "text/plain".to_string())
            .unwrap();
        assert_ne!(id1, id2);

        let entries = state.get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "Second");
        assert_eq!(entries[1].text, "First");

        // Re-adding "First" should move it to the front with new entry id
        let id3 = state
            .add_text("First".to_string(), "text/plain".to_string())
            .unwrap();
        assert_eq!(id3, 3);
        let entries = state.get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "First");
        assert_eq!(entries[0].id, id3);
        assert_eq!(entries[1].text, "Second");
        assert_eq!(entries[1].id, id2);
    }

    #[test]
    fn test_clipboard_state_max_entries_cap() {
        let mut state = ClipboardState::new();
        for i in 0..100 {
            state.add_text(format!("Item {}", i), "text/plain".to_string());
        }
        let entries = state.get_entries();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].text, "Item 99");
        assert_eq!(
            entries[MAX_ENTRIES - 1].text,
            format!("Item {}", 100 - MAX_ENTRIES)
        );
    }

    #[test]
    fn test_clipboard_disk_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("wyrd_clip_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("history.jsonl");

        let entries = vec![
            ClipboardEntry {
                id: 1,
                text: "Saved entry 1".to_string(),
                mime: "text/plain".to_string(),
                timestamp: 12345,
            },
            ClipboardEntry {
                id: 2,
                text: "Saved entry 2".to_string(),
                mime: "text/plain;charset=utf-8".to_string(),
                timestamp: 12346,
            },
        ];

        save_history_to_disk(&path, &entries);
        assert!(path.exists());

        let loaded = load_history_from_disk(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "Saved entry 1");
        assert_eq!(loaded[1].text, "Saved entry 2");

        // Test clear
        save_history_to_disk(&path, &[]);
        let empty = load_history_from_disk(&path);
        assert_eq!(empty.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_client_command_serialization() {
        let cmd_list: ClientCommand = serde_json::from_str(r#"{"cmd":"list"}"#).unwrap();
        assert!(matches!(cmd_list, ClientCommand::List));

        let cmd_select: ClientCommand =
            serde_json::from_str(r#"{"cmd":"select","id":42}"#).unwrap();
        assert!(matches!(cmd_select, ClientCommand::Select { id: 42 }));

        let cmd_clear: ClientCommand = serde_json::from_str(r#"{"cmd":"clear"}"#).unwrap();
        assert!(matches!(cmd_clear, ClientCommand::Clear));

        let cmd_set_image: ClientCommand =
            serde_json::from_str(r#"{"cmd":"set_image","path":"/tmp/test.png"}"#).unwrap();
        assert!(matches!(
            cmd_set_image,
            ClientCommand::SetImage { ref path } if path == "/tmp/test.png"
        ));
    }

    #[test]
    fn test_clipboard_image_entries_and_cap() {
        let mut state = ClipboardState::new();
        let bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header dummy
        for i in 0..25 {
            state.add_image(format!("/tmp/shot_{}.png", i), bytes.clone());
        }

        assert_eq!(state.images.len(), MAX_IMAGE_ENTRIES);
        assert_eq!(state.images.front().unwrap().path, "/tmp/shot_24.png");

        let id = state.images.front().unwrap().id;
        let retrieved = state.get_image_by_id(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().path, "/tmp/shot_24.png");

        // Adding an existing path should deduplicate and move to front
        let id_new = state.add_image("/tmp/shot_20.png".to_string(), bytes.clone());
        assert_eq!(state.images.front().unwrap().path, "/tmp/shot_20.png");
        assert_eq!(state.images.front().unwrap().id, id_new);
        assert_eq!(state.images.len(), MAX_IMAGE_ENTRIES);
    }
}
