//! WASM Do Not Disturb (DND) Module.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, publish, resolve_icon, send_update, unix_socket_request,
    WasmModule,
};

#[derive(Default)]
pub struct DndModule {
    enabled: bool,
}

impl WasmModule for DndModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("DND WASM module initialized");
        // Sync with notifications daemon if running
        if let Ok(raw) = unix_socket_request("wyrd-notifications.sock", r#"{"cmd":"status"}"#) {
            if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                if let Some(dnd) = val.get("dnd").and_then(Value::as_bool) {
                    self.enabled = dnd;
                }
            }
        }
        self.render();
        let _ = publish("dnd.state", &json!({ "enabled": self.enabled }));
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if event == "click" && (widget_id == "dnd" || widget_id.contains("dnd")) {
            self.toggle();
        }
    }

    fn on_topic(&mut self, topic: &str, payload: &Value) {
        if topic == "dnd.toggle" {
            self.toggle();
        } else if topic == "dnd.state" {
            if let Some(enabled) = payload.get("enabled").and_then(|v| v.as_bool()) {
                if self.enabled != enabled {
                    self.enabled = enabled;
                    let _ = unix_socket_request(
                        "wyrd-notifications.sock",
                        &json!({ "cmd": "dnd", "dnd": self.enabled }).to_string(),
                    );
                    self.render();
                }
            }
        }
    }
}

impl DndModule {
    fn toggle(&mut self) {
        self.enabled = !self.enabled;
        let _ = unix_socket_request(
            "wyrd-notifications.sock",
            &json!({ "cmd": "dnd", "dnd": self.enabled }).to_string(),
        );
        self.render();
        let _ = publish("dnd.state", &json!({ "enabled": self.enabled }));
    }
    fn render(&self) {
        let icon_name = if self.enabled {
            "notifications-disabled"
        } else {
            "preferences-desktop-notification-bell"
        };
        let icon_path = resolve_icon(icon_name)
            .or_else(|| {
                resolve_icon(if self.enabled {
                    "notifications-disabled"
                } else {
                    "notification-bell"
                })
            })
            .or_else(|| resolve_icon("notification"));

        let style = if self.enabled { "chip_accent" } else { "chip" };
        let payload = json!({
            "text": if icon_path.is_some() { "" } else if self.enabled { "DND" } else { "Bell" },
            "icon": icon_name,
            "icon_path": icon_path,
            "path": icon_path,
            "enabled": self.enabled,
            "dnd": self.enabled,
            "style": style,
            "tooltip": if self.enabled { "Do Not Disturb: ON" } else { "Do Not Disturb: OFF" },
            "on_click": "event:dnd:dnd:click"
        });
        send_update("dnd", &payload);
    }
}

export_wasm_module!(DndModule);
