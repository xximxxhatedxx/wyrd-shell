//! WASM Power Menu Module.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, export_wasm_module, log_info, publish, request_surface, resolve_icon, send_update,
    unix_socket_request, WasmModule,
};

#[derive(Default)]
pub struct PowerMenuModule {
    config: Value,
}

impl WasmModule for PowerMenuModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Power Menu WASM module initialized");
        self.config = config;
        let power_icon_path =
            resolve_icon("system-shutdown").or_else(|| resolve_icon("system-log-out"));
        let bar_payload = json!({
            "text": "⏻",
            "icon": "system-shutdown",
            "icon_path": power_icon_path,
        });
        send_update("power", &bar_payload);
        send_update("power-menu", &bar_payload);
        send_update("power_btn", &bar_payload);
        self.send_power_popup();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        log_info(&format!("Power action: {} on {}", event, widget_id));
        let is_power = widget_id == "power"
            || widget_id == "power-menu"
            || widget_id == "powermenu"
            || widget_id.contains("power");
        if (event == "open" || event == "click" || event.starts_with("click")) && is_power {
            self.send_power_popup();
            return;
        }

        match widget_id {
            "lock" => {
                let _ = dbus_call(
                    "session",
                    "org.freedesktop.ScreenSaver",
                    "/org/freedesktop/ScreenSaver",
                    "org.freedesktop.ScreenSaver",
                    "Lock",
                    &json!([]),
                );
                let _ = login1_call("LockSessions", json!([]));
                let _ = publish("power.lock_requested", &json!({}));
                let _ = publish("power.action", &json!({ "action": "lock" }));
            }
            "suspend" => {
                let _ = login1_call("Suspend", json!([true]));
                let _ = publish("power.action", &json!({ "action": "suspend" }));
            }
            "reboot" => {
                let _ = login1_call("Reboot", json!([true]));
                let _ = publish("power.action", &json!({ "action": "reboot" }));
            }
            "poweroff" | "shutdown" => {
                let _ = login1_call("PowerOff", json!([true]));
                let _ = publish("power.action", &json!({ "action": "poweroff" }));
            }
            "logout" => {
                // Terminate compositor cleanly via wyrd-windows.sock IPC
                let _ = unix_socket_request("wyrd-windows.sock", r#"{"cmd":"exit"}"#);
                let _ = login1_call("TerminateSession", json!(["self"]));
                let _ = publish("power.action", &json!({ "action": "logout" }));
            }
            _ => return,
        }
        request_surface("close", &json!({ "name": "power" }));
    }
}

impl PowerMenuModule {
    fn send_power_popup(&self) {
        let mut children = Vec::new();

        // 1. Header Card with dynamic system icon
        let header_icon = resolve_icon("system-shutdown");
        let mut header_content = Vec::new();
        if let Some(path) = &header_icon {
            header_content.push(json!({
                "type": if path.ends_with(".svg") { "svg" } else { "image" },
                "path": path,
                "layout": { "width": 18.0, "height": 18.0 }
            }));
        }
        header_content.push(json!({
            "type": "text",
            "text": "POWER & SESSION",
            "style": "clean_accent",
        }));

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "center", "gap": 8.0, "padding": [10.0, 14.0, 10.0, 14.0] },
            "children": header_content
        }));

        // 2. Buttons card with FreeDesktop standard action icons
        let actions = [
            ("lock", "Lock Screen", "system-lock-screen", false),
            ("suspend", "Suspend / Sleep", "system-suspend", false),
            ("logout", "Log Out Session", "system-log-out", false),
            ("reboot", "Reboot System", "system-reboot", true),
            ("shutdown", "Power Off", "system-shutdown", true),
        ];

        let mut btn_children = Vec::new();
        for (id, label, icon_name, is_accent) in actions {
            let style_name = if is_accent { "chip_accent" } else { "chip" };
            let icon_path = resolve_icon(icon_name);
            let mut row_content = Vec::new();

            if let Some(path) = &icon_path {
                row_content.push(json!({
                    "type": if path.ends_with(".svg") { "svg" } else { "image" },
                    "path": path,
                    "layout": { "width": 16.0, "height": 16.0 }
                }));
            }
            row_content.push(json!({
                "type": "text",
                "text": label,
            }));

            btn_children.push(json!({
                "type": "button",
                "id": id,
                "style": style_name,
                "on_click": format!("event:power-menu:{}:click", id),
                "layout": { "mode": "flex_row", "gap": 8.0, "height": 34.0, "justify": "center", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] },
                "children": row_content
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 6.0, "padding": [8.0, 8.0, 8.0, 8.0] },
            "children": btn_children
        }));

        let payload = json!({ "children": children });
        send_update("popup:power", &payload);
        send_update("popup:power-menu", &payload);
    }
}

fn login1_call(member: &str, args: Value) -> bool {
    dbus_call(
        "system",
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        member,
        &args,
    )
    .is_some()
}

export_wasm_module!(PowerMenuModule);
