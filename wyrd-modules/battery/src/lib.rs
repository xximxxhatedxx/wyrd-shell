//! WASM Battery Module.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, log_info, read_dir, read_file, resolve_icon,
    send_update, WasmModule,
};

#[derive(Default)]
pub struct BatteryModule {
    capacity: u8,
    status: String,
    profile: String,
    dirty: bool,
}

impl WasmModule for BatteryModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Battery WASM module initialized");
        let _ = dbus_subscribe(
            "system",
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        );
        self.update_and_render();
        self.dirty = false;
        Ok(())
    }

    fn tick(&mut self) {
        self.update_and_render();
        self.dirty = false;
    }

    fn on_dbus_signal(&mut self, _interface: &str, _member: &str, _payload: &Value) {
        self.dirty = true;
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if event != "click" || !widget_id.starts_with("battery_profile_") {
            return;
        }
        let profile = widget_id.trim_start_matches("battery_profile_");
        let _ = dbus_call(
            "system",
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            "org.freedesktop.DBus.Properties",
            "Set",
            &json!(["net.hadess.PowerProfiles", "ActiveProfile", profile]),
        );
        self.update_and_render();
    }
}

impl BatteryModule {
    fn update_and_render(&mut self) {
        let (cap, stat) = self.read_battery_info();
        self.capacity = cap;
        self.status = stat;
        self.profile = dbus_call(
            "system",
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            "org.freedesktop.DBus.Properties",
            "Get",
            &json!(["net.hadess.PowerProfiles", "ActiveProfile"]),
        )
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();

        let is_charging = matches!(self.status.as_str(), "Charging" | "Full");
        let icon_name = if is_charging {
            "battery-charging"
        } else if self.capacity <= 15 {
            "battery-caution"
        } else if self.capacity <= 35 {
            "battery-low"
        } else if self.capacity <= 75 {
            "battery-good"
        } else {
            "battery-full"
        };
        let icon_path = resolve_icon(icon_name).or_else(|| resolve_icon("battery"));
        let icon_glyph = if is_charging {
            "󰂄"
        } else if self.capacity <= 15 {
            "󰁺"
        } else if self.capacity <= 35 {
            "󰁼"
        } else if self.capacity <= 75 {
            "󰂀"
        } else {
            "󰁹"
        };

        let mut header_content = Vec::new();
        if let Some(ref path) = icon_path {
            header_content.push(json!({
                "type": if path.ends_with(".svg") { "svg" } else { "image" },
                "path": path,
                "layout": { "width": 18.0, "height": 18.0 }
            }));
        }
        header_content.push(json!({
            "type": "text",
            "text": format!("Battery: {}% ({})", self.capacity, self.status),
            "style": "clean_accent",
        }));

        let payload = json!({
            "text": format!("{} {}%", icon_glyph, self.capacity),
            "icon_glyph": icon_glyph,
            "capacity": self.capacity,
            "percent": self.capacity,
            "status": self.status,
            "charging": is_charging,
            "is_charging": is_charging,
            "icon": icon_name,
            "icon_path": icon_path,
            "path": icon_path,
            "power_profile": self.profile,
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "gap": 8.0, "padding": [4.0, 4.0, 4.0, 4.0] },
                    "children": header_content
                },
                { "type": "text", "text": if self.profile.is_empty() { "Power profile unavailable" } else { &self.profile } },
                { "type": "button", "id": "battery_profile_power-saver", "text": "Power Saver", "on_click": "event:battery:battery_profile_power-saver:click" },
                { "type": "button", "id": "battery_profile_balanced", "text": "Balanced", "on_click": "event:battery:battery_profile_balanced:click" },
                { "type": "button", "id": "battery_profile_performance", "text": "Performance", "on_click": "event:battery:battery_profile_performance:click" }
            ],
        });

        send_update("battery", &payload);
        send_update("popup:battery", &payload);
    }

    fn read_battery_info(&self) -> (u8, String) {
        let entries = read_dir("/sys/class/power_supply").unwrap_or_default();
        for entry in &entries {
            let base = format!("/sys/class/power_supply/{}", entry);
            let type_path = format!("{}/type", base);
            let is_battery = read_file(&type_path)
                .map(|t| t.trim().eq_ignore_ascii_case("battery"))
                .unwrap_or_else(|| entry.starts_with("BAT") || entry.starts_with("battery"));

            if is_battery {
                let cap_path = format!("{}/capacity", base);
                let stat_path = format!("{}/status", base);

                if let Some(cap_str) = read_file(&cap_path) {
                    if let Ok(cap) = cap_str.trim().parse::<u8>() {
                        let stat = read_file(&stat_path)
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|| "Discharging".to_string());
                        return (cap, stat);
                    }
                }
            }
        }

        (0, "Unavailable".to_string())
    }
}

export_wasm_module!(BatteryModule);
