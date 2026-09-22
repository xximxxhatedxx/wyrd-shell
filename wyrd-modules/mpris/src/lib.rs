//! WASM MPRIS Media Player Module for Wyrd Shell.
//! Integrates with session D-Bus for media player status and control.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, log_info, publish, resolve_icon, send_update,
    WasmModule,
};

#[derive(Default, Debug, Clone)]
pub struct MprisModule {
    config: Value,
    player: String,
    status: String, // "Playing", "Paused", "Stopped"
    title: String,
    artist: String,
    album: String,
    art_url: String,
    position: f64, // seconds
    length: f64,   // seconds
}

impl WasmModule for MprisModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("MPRIS WASM module initializing");
        self.config = config;
        let _ = dbus_subscribe(
            "session",
            "org.mpris.MediaPlayer2.*",
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        );
        self.query_state();
        self.emit_bar_updates();
        self.send_mpris_popup();
        Ok(())
    }

    fn tick(&mut self) {
        self.query_state();
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn on_dbus_signal(&mut self, _interface: &str, _member: &str, _payload: &Value) {
        self.query_state();
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        log_info(&format!("MPRIS event on {}: {}", widget_id, event));

        let is_root = widget_id == "mpris"
            || widget_id == "media"
            || widget_id == "player"
            || widget_id == "popup:mpris";

        if event == "open" || (is_root && (event == "click" || event == "toggle")) {
            self.send_mpris_popup();
            return;
        }

        match widget_id {
            "mpris_play_pause" | "play_pause" | "play" | "pause" | "toggle" => {
                self.toggle_playback();
            }
            "mpris_next" | "next" => {
                self.next_track();
            }
            "mpris_prev" | "prev" | "previous" => {
                self.prev_track();
            }
            "mpris_stop" | "stop" => {
                self.stop_playback();
            }
            "mpris_seek" | "seek" => {
                let val_str = event.split(':').next().unwrap_or(event);
                if let Ok(sec) = val_str.parse::<f64>() {
                    self.seek(sec);
                }
            }
            _ => {
                if event == "click" {
                    if widget_id.contains("next") {
                        self.next_track();
                    } else if widget_id.contains("prev") {
                        self.prev_track();
                    } else if widget_id.contains("play") || widget_id.contains("pause") {
                        self.toggle_playback();
                    } else if widget_id.contains("stop") {
                        self.stop_playback();
                    }
                }
            }
        }
    }
}

impl MprisModule {
    fn query_state(&mut self) {
        // Native D-Bus inspection via standard org.mpris.MediaPlayer2 specification
        if let Some(names_val) = dbus_call(
            "session",
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "ListNames",
            &json!([]),
        ) {
            let mut active_player: Option<String> = None;
            if let Some(arr) = names_val.as_array() {
                for name in arr {
                    if let Some(n) = name.as_str() {
                        if n.starts_with("org.mpris.MediaPlayer2.") {
                            active_player = Some(n.to_string());
                            break;
                        }
                    }
                }
            }

            if let Some(player_service) = active_player {
                self.player = player_service
                    .strip_prefix("org.mpris.MediaPlayer2.")
                    .unwrap_or(&player_service)
                    .to_string();

                if let Some(props) = dbus_call(
                    "session",
                    &player_service,
                    "/org/mpris/MediaPlayer2",
                    "org.freedesktop.DBus.Properties",
                    "GetAll",
                    &json!(["org.mpris.MediaPlayer2.Player"]),
                ) {
                    if let Some(st) = props.get("PlaybackStatus").and_then(Value::as_str) {
                        self.status = st.to_string();
                    }
                    if let Some(meta) = props.get("Metadata") {
                        if let Some(t) = meta.get("xesam:title").and_then(Value::as_str) {
                            self.title = t.to_string();
                        }
                        if let Some(a) = meta.get("xesam:artist") {
                            if let Some(s) = a.as_str() {
                                self.artist = s.to_string();
                            } else if let Some(arr) = a.as_array() {
                                let artists: Vec<&str> =
                                    arr.iter().filter_map(Value::as_str).collect();
                                self.artist = artists.join(", ");
                            }
                        }
                        if let Some(alb) = meta.get("xesam:album").and_then(Value::as_str) {
                            self.album = alb.to_string();
                        }
                        if let Some(art) = meta.get("mpris:artUrl").and_then(Value::as_str) {
                            self.art_url = art.strip_prefix("file://").unwrap_or(art).to_string();
                        }
                        if let Some(l) = meta.get("mpris:length").and_then(Value::as_f64) {
                            self.length = if l > 10_000.0 { l / 1_000_000.0 } else { l };
                        }
                    }
                    if let Some(p) = props.get("Position").and_then(Value::as_f64) {
                        self.position = if p > 10_000.0 { p / 1_000_000.0 } else { p };
                    }
                    return;
                }
            }
        }

        // No player active
        self.status = "Stopped".to_string();
        self.title.clear();
        self.artist.clear();
        self.album.clear();
        self.art_url.clear();
        self.position = 0.0;
        self.length = 0.0;
    }

    fn toggle_playback(&mut self) {
        if !self.player.is_empty() {
            let _ = dbus_call(
                "session",
                &format!("org.mpris.MediaPlayer2.{}", self.player),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "PlayPause",
                &json!([]),
            );
        }
        self.status = if self.status == "Playing" {
            "Paused".to_string()
        } else {
            "Playing".to_string()
        };
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn next_track(&mut self) {
        if !self.player.is_empty() {
            let _ = dbus_call(
                "session",
                &format!("org.mpris.MediaPlayer2.{}", self.player),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "Next",
                &json!([]),
            );
        }
        self.query_state();
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn prev_track(&mut self) {
        if !self.player.is_empty() {
            let _ = dbus_call(
                "session",
                &format!("org.mpris.MediaPlayer2.{}", self.player),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "Previous",
                &json!([]),
            );
        }
        self.query_state();
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn stop_playback(&mut self) {
        if !self.player.is_empty() {
            let _ = dbus_call(
                "session",
                &format!("org.mpris.MediaPlayer2.{}", self.player),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "Stop",
                &json!([]),
            );
        }
        self.status = "Stopped".to_string();
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn seek(&mut self, sec: f64) {
        let microsecs = (sec * 1_000_000.0) as i64;
        if !self.player.is_empty() {
            let _ = dbus_call(
                "session",
                &format!("org.mpris.MediaPlayer2.{}", self.player),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "SetPosition",
                &json!(["/org/mpris/MediaPlayer2/TrackList/NoTrack", microsecs]),
            );
        }
        self.position = sec;
        self.emit_bar_updates();
        self.send_mpris_popup();
    }

    fn emit_bar_updates(&self) {
        let is_stopped =
            self.status == "Stopped" || (self.title.is_empty() && self.artist.is_empty());
        let icon_name = if is_stopped {
            "multimedia-player"
        } else if self.status == "Playing" {
            "media-playback-start"
        } else {
            "media-playback-pause"
        };
        let icon_path = resolve_icon(icon_name).or_else(|| resolve_icon("multimedia-player"));

        let display_text = if is_stopped {
            "󰎆 Media".to_string()
        } else {
            let label = if self.artist.is_empty() {
                self.title.clone()
            } else {
                format!("{} - {}", self.artist, self.title)
            };
            let short = if label.chars().count() > 28 {
                let s: String = label.chars().take(25).collect();
                format!("{}…", s)
            } else {
                label
            };
            if self.status == "Playing" {
                format!("󰐊 {}", short)
            } else {
                format!("󰏤 {}", short)
            }
        };

        let tooltip = if is_stopped {
            "Media Player (No active media)\nClick: Toggle player console\nRight-click: Play/Pause"
                .to_string()
        } else {
            format!(
                "{}\nArtist: {}\nAlbum: {}\nStatus: {}\nClick: Toggle player console\nRight-click: Play/Pause",
                if self.title.is_empty() { "Unknown Track" } else { &self.title },
                if self.artist.is_empty() { "Unknown Artist" } else { &self.artist },
                if self.album.is_empty() { "Unknown Album" } else { &self.album },
                self.status
            )
        };

        let progress_pct = if self.length > 0.0 {
            ((self.position / self.length) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let safe_title = if self.title.is_empty() {
            "No media playing".to_string()
        } else {
            truncate_text(&self.title, 40)
        };
        let safe_artist = if self.artist.is_empty() {
            "Media Player".to_string()
        } else {
            truncate_text(&self.artist, 40)
        };

        let payload = json!({
            "text": display_text,
            "title": self.title,
            "artist": self.artist,
            "safe_title": safe_title,
            "safe_artist": safe_artist,
            "album": self.album,
            "status": self.status,
            "playing": self.status == "Playing",
            "icon": icon_name,
            "icon_path": icon_path,
            "tooltip": tooltip,
            "player": self.player,
            "position": self.position,
            "length": self.length,
            "progress": progress_pct,
            "position_str": format_duration(self.position),
            "length_str": format_duration(self.length),
        });

        send_update("mpris", &payload);
        send_update("media", &payload);
        send_update("player", &payload);
        let _ = publish("mpris.status", &payload);
    }

    fn send_mpris_popup(&self) {
        let is_stopped =
            self.status == "Stopped" || (self.title.is_empty() && self.artist.is_empty());

        let mut children = Vec::new();

        // 1. Header Row
        let player_label = if self.player.is_empty() {
            "MEDIA PLAYER".to_string()
        } else {
            self.player.to_uppercase()
        };
        let status_badge = if self.status == "Playing" {
            "PLAYING"
        } else if self.status == "Paused" {
            "PAUSED"
        } else {
            "IDLE"
        };
        let status_style = if self.status == "Playing" {
            "clean_accent"
        } else {
            "audio_device_desc"
        };

        children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "justify": "space-between", "align": "center" },
            "children": [
                {
                    "type": "text",
                    "text": player_label,
                    "style": "section_title",
                },
                {
                    "type": "text",
                    "text": status_badge,
                    "style": status_style,
                }
            ]
        }));

        // 2. Track & Album Card
        let mut track_card_children = Vec::new();

        // Left side: Art / Icon box
        let has_art_file = !self.art_url.is_empty() && std::path::Path::new(&self.art_url).exists();
        if has_art_file {
            track_card_children.push(json!({
                "type": "image",
                "path": self.art_url,
                "layout": { "width": 64.0, "height": 64.0 },
                "style": { "radius": 12.0 }
            }));
        } else {
            track_card_children.push(json!({
                "type": "container",
                "layout": { "width": 64.0, "height": 64.0, "justify": "center", "align": "center" },
                "style": "card_nested",
                "children": [
                    {
                        "type": "text",
                        "text": "󰝚",
                        "style": "stat_hero"
                    }
                ]
            }));
        }

        // Right side: Track Info
        let track_title = if is_stopped {
            "Ready for playback".to_string()
        } else if self.title.is_empty() {
            "Unknown Track".to_string()
        } else {
            truncate_text(&self.title, 34)
        };

        let track_artist = if is_stopped {
            "Open Spotify, Firefox or MPV".to_string()
        } else if self.artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            truncate_text(&self.artist, 34)
        };

        track_card_children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_col", "gap": 4.0, "weight": 1.0, "justify": "center" },
            "children": [
                {
                    "type": "text",
                    "text": track_title,
                    "style": "card_title",
                },
                {
                    "type": "text",
                    "text": track_artist,
                    "style": "audio_device_desc",
                }
            ]
        }));

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "gap": 12.0, "align": "center" },
            "children": track_card_children
        }));

        // 3. Progress timeline & seeker
        if !is_stopped && self.length > 0.0 {
            let pos_str = format_duration(self.position);
            let len_str = format_duration(self.length);

            children.push(json!({
                "type": "container",
                "layout": { "mode": "flex_col", "gap": 4.0 },
                "children": [
                    {
                        "type": "container",
                        "layout": { "mode": "flex_row", "justify": "space-between", "align": "center" },
                        "children": [
                            {
                                "type": "text",
                                "text": pos_str,
                                "style": "audio_device_desc",
                            },
                            {
                                "type": "text",
                                "text": len_str,
                                "style": "audio_device_desc",
                            }
                        ]
                    },
                    {
                        "type": "slider",
                        "id": "mpris_seek",
                        "value": self.position as f32,
                        "min": 0.0,
                        "max": self.length as f32,
                        "on_change": "event:mpris:mpris_seek:{value}",
                        "layout": { "height": 18.0 }
                    }
                ]
            }));
        }

        // 4. Transport Controls row
        let play_icon = if self.status == "Playing" {
            "󰏤"
        } else {
            "󰐊"
        };

        children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "justify": "center", "align": "center", "gap": 10.0 },
            "children": [
                {
                    "type": "button",
                    "id": "mpris_prev",
                    "text": "󰒮",
                    "style": "row_btn",
                    "on_click": "event:mpris:mpris_prev:click",
                    "layout": { "width": 42.0, "height": 38.0, "justify": "center", "align": "center" }
                },
                {
                    "type": "button",
                    "id": "mpris_play_pause",
                    "text": play_icon,
                    "style": "action_btn",
                    "on_click": "event:mpris:mpris_play_pause:click",
                    "layout": { "width": 54.0, "height": 42.0, "justify": "center", "align": "center" }
                },
                {
                    "type": "button",
                    "id": "mpris_next",
                    "text": "󰒭",
                    "style": "row_btn",
                    "on_click": "event:mpris:mpris_next:click",
                    "layout": { "width": 42.0, "height": 38.0, "justify": "center", "align": "center" }
                },
                {
                    "type": "button",
                    "id": "mpris_stop",
                    "text": "󰓛",
                    "style": "row_btn",
                    "on_click": "event:mpris:mpris_stop:click",
                    "layout": { "width": 38.0, "height": 38.0, "justify": "center", "align": "center" }
                }
            ]
        }));

        let payload = json!({ "children": children });
        send_update("popup:mpris", &payload);
    }
}

fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let mins = s / 60;
    let secs = s % 60;
    format!("{:02}:{:02}", mins, secs)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated.trim_end())
    }
}

export_wasm_module!(MprisModule);
