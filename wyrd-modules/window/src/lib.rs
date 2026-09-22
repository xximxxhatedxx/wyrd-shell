//! WASM Active Window Module.
//!
//! Native Hyprland IPC client for active window title and class tracking.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, send_update, unix_socket_request, windows_subscribe, WasmModule,
};

#[derive(Default)]
pub struct WindowModule {
    last_title: String,
}

impl WasmModule for WindowModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Window title WASM module initialized with native Hyprland IPC & Foreign Toplevel fallback");
        let _ = windows_subscribe();
        self.update_window();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, _event: &str) {
        if widget_id == "changed" || widget_id == "wyrd-windows" {
            self.update_window();
        }
    }

    fn tick(&mut self) {
        // Event-driven via windows_subscribe(). Only retry if initial title was empty.
        if self.last_title.is_empty() {
            self.update_window();
        }
    }
}

impl WindowModule {
    fn update_window(&mut self) {
        let mut title = "Desktop".to_string();
        let mut class = "".to_string();

        let active = unix_socket_request("wyrd-windows.sock", r#"{"cmd":"active_window"}"#)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|value| value.get("active").cloned())
            .filter(|value| !value.is_null());
        if let Some(active) = active {
            if let Some(t) = active.get("title").and_then(|s| s.as_str()) {
                if !t.is_empty() {
                    title = t.to_string();
                }
            }
            if let Some(c) = active.get("app_id").and_then(|s| s.as_str()) {
                class = c.to_string();
            }
        }

        if title != self.last_title {
            self.last_title = title.clone();
            send_update(
                "window",
                &json!({
                    "text": title,
                    "title": title,
                    "class": class,
                }),
            );
        }
    }
}

export_wasm_module!(WindowModule);
