//! High-level SDK for WASM guest modules for Wyrd Shell.

pub mod wasm;

pub use wasm::{
    audio_get_status, audio_set_default, audio_set_mute, audio_set_volume, audio_subscribe,
    audio_toggle_mute, dbus_call, dbus_subscribe, exec_command, exec_process, file_exists,
    get_application_dirs, get_env, list_dir, local_offset_seconds, log_error, log_info, log_warn,
    notifications_clear, notifications_get_status, notifications_set_dnd, notifications_subscribe,
    now_ms, publish, read_dir, read_file, request_capability, request_focus, request_surface,
    resolve_icon, send_update, spawn_process, unix_socket_request, unix_socket_subscribe,
    windows_subscribe, WasmModule,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Message from shell core → module.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CoreMessage {
    Init { version: u32, config: Value },
    Event { widget_id: String, event: String },
    PopupEvent { popup_id: String, event: String },
    TopicEvent { topic: String, value: Value },
    Shutdown,
}

/// Message from module → shell core.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModuleMessage {
    Ready,
    Update {
        widget_id: String,
        payload: Value,
    },
    Surface {
        action: String,
        params: Value,
    },
    Publish {
        topic: String,
        value: Value,
    },
    RequestCapability {
        capability: String,
        action: String,
        params: Value,
    },
    FocusRequest {
        popup_id: String,
    },
    Ping,
}
