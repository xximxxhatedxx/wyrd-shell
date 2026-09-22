use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

mod audio_service {
    include!("audio_service.rs");
}

use audio_service::AudioService;

#[derive(Parser, Debug)]
#[command(name = "wyrd-audio", about = "Wyrd Audio Daemon")]
struct Cli {
    #[arg(short, long)]
    socket_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AudioSocketRequest {
    cmd: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    mute: Option<bool>,
    #[serde(default)]
    name: Option<String>,
}

fn is_source_target(target: &str) -> bool {
    target.contains("source") || target.contains("mic") || target == "@DEFAULT_AUDIO_SOURCE@"
}

fn socket_path(cli_override: Option<&StdPath>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wyrd-audio.sock")
    } else {
        PathBuf::from("/tmp/wyrd-audio.sock")
    }
}

async fn handle_client(
    stream: UnixStream,
    service: Arc<AudioService>,
    subscribers: Arc<parking_lot::Mutex<Vec<mpsc::UnboundedSender<String>>>>,
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

                let req: AudioSocketRequest = match serde_json::from_str(trimmed) {
                    Ok(value) => value,
                    Err(_) => {
                        write_half
                            .write_all(b"{\"error\":\"invalid_json\"}\n")
                            .await?;
                        continue;
                    }
                };

                let response = match req.cmd.as_str() {
                    "status" => serde_json::to_string(&service.get_status()).unwrap(),
                    "set_volume" => {
                        let target = req.target;
                        let value = req.value.unwrap_or(0.0);
                        let ok = if !target.is_empty() {
                            if is_source_target(&target) {
                                service.set_source_volume(&target, value)
                            } else {
                                service.set_sink_volume(&target, value)
                            }
                        } else {
                            service.set_sink_volume("", value)
                        };
                        serde_json::json!({ "status": if ok { "ok" } else { "error" } }).to_string()
                    }
                    "set_mute" => {
                        let target = req.target;
                        let mute = req.mute.unwrap_or(false);
                        let ok = if !target.is_empty() {
                            if is_source_target(&target) {
                                service.set_source_mute(&target, mute)
                            } else {
                                service.set_sink_mute(&target, mute)
                            }
                        } else {
                            service.set_sink_mute("", mute)
                        };
                        serde_json::json!({ "status": if ok { "ok" } else { "error" } }).to_string()
                    }
                    "toggle_mute" => {
                        let target = req.target;
                        let ok = if !target.is_empty() {
                            if is_source_target(&target) {
                                service.toggle_source_mute(&target)
                            } else {
                                service.toggle_sink_mute(&target)
                            }
                        } else {
                            service.toggle_sink_mute("")
                        };
                        serde_json::json!({ "status": if ok { "ok" } else { "error" } }).to_string()
                    }
                    "set_default" => {
                        let target = req.target;
                        let name = req.name.unwrap_or_default();
                        let ok = if target == "sink" {
                            service.set_default_sink(&name)
                        } else {
                            service.set_default_source(&name)
                        };
                        serde_json::json!({ "status": if ok { "ok" } else { "error" } }).to_string()
                    }
                    "subscribe" => {
                        let (tx, mut rx) = mpsc::unbounded_channel();
                        subscribers.lock().push(tx);
                        let initial = serde_json::json!({
                            "event": "changed",
                            "status": service.get_status(),
                        });
                        write_half.write_all(initial.to_string().as_bytes()).await?;
                        write_half.write_all(b"\n").await?;
                        write_half
                            .write_all(b"{\"status\":\"subscribed\"}\n")
                            .await?;
                        while let Some(event) = rx.recv().await {
                            write_half.write_all(event.as_bytes()).await?;
                            write_half.write_all(b"\n").await?;
                        }
                        break;
                    }
                    _ => "{\"error\":\"unknown_command\"}".to_string(),
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
    service: Arc<AudioService>,
    socket_override: Option<&StdPath>,
) -> Result<()> {
    let socket = socket_path(socket_override);
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
    }

    let listener = UnixListener::bind(&socket).context("failed to bind wyrd-audio UNIX socket")?;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket, permissions)
        .context("failed to set wyrd-audio socket permissions")?;

    let subscribers: Arc<parking_lot::Mutex<Vec<mpsc::UnboundedSender<String>>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));
    let mut events = service.subscribe();
    let event_subscribers = subscribers.clone();
    let event_service = service.clone();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            let message = serde_json::json!({
                "event": event,
                "status": event_service.get_status(),
            })
            .to_string();
            let mut clients = event_subscribers.lock();
            clients.retain(|client| client.send(message.clone()).is_ok());
        }
    });

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
    let service = Arc::new(AudioService::new()?);
    run_socket_server(service, cli.socket_path.as_deref()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_source_target_detection() {
        assert!(is_source_target("@DEFAULT_AUDIO_SOURCE@"));
        assert!(is_source_target("mic"));
        assert!(is_source_target("built-in_microphone"));
        assert!(is_source_target("alsa_input.source.1"));

        assert!(!is_source_target("@DEFAULT_AUDIO_SINK@"));
        assert!(!is_source_target("speaker"));
        assert!(!is_source_target("headphones"));
        assert!(!is_source_target("alsa_output.pci"));
    }

    #[test]
    fn test_socket_path_resolution() {
        let custom = StdPath::new("/tmp/custom_audio.sock");
        assert_eq!(socket_path(Some(custom)), custom);

        let default_path = socket_path(None);
        assert!(default_path.ends_with("wyrd-audio.sock"));
    }

    #[test]
    fn test_audio_socket_request_deserialization() {
        let json_status = r#"{"cmd":"status"}"#;
        let req: AudioSocketRequest = serde_json::from_str(json_status).unwrap();
        assert_eq!(req.cmd, "status");
        assert!(req.target.is_empty());
        assert_eq!(req.value, None);
        assert_eq!(req.mute, None);

        let json_vol = r#"{"cmd":"set_volume","target":"mic","value":0.85}"#;
        let req: AudioSocketRequest = serde_json::from_str(json_vol).unwrap();
        assert_eq!(req.cmd, "set_volume");
        assert_eq!(req.target, "mic");
        assert_eq!(req.value, Some(0.85));

        let json_mute = r#"{"cmd":"set_mute","target":"speaker","mute":true}"#;
        let req: AudioSocketRequest = serde_json::from_str(json_mute).unwrap();
        assert_eq!(req.cmd, "set_mute");
        assert_eq!(req.target, "speaker");
        assert_eq!(req.mute, Some(true));

        let json_default = r#"{"cmd":"set_default","target":"sink","name":"alsa_output.usb"}"#;
        let req: AudioSocketRequest = serde_json::from_str(json_default).unwrap();
        assert_eq!(req.cmd, "set_default");
        assert_eq!(req.target, "sink");
        assert_eq!(req.name, Some("alsa_output.usb".to_string()));
    }
}
