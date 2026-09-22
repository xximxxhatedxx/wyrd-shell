//! WASM Display Brightness & Backlight Module for Wyrd Shell.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, export_wasm_module, log_info, read_dir, read_file, resolve_icon, send_update,
    WasmModule,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklightDevice {
    pub name: String,
    pub device_type: String, // "raw", "platform", "firmware"
    pub current: u64,
    pub max: u64,
}

impl BacklightDevice {
    pub fn percent(&self) -> u8 {
        if self.max == 0 {
            0
        } else {
            ((self.current as f64 / self.max as f64) * 100.0)
                .round()
                .min(100.0) as u8
        }
    }

    pub fn to_raw_value(&self, percent: u8, min_percent: u8) -> u64 {
        let clamped_pct = percent.clamp(min_percent, 100) as u64;
        (clamped_pct * self.max) / 100
    }
}

pub fn rank_device_type(ty: &str) -> u8 {
    match ty.trim().to_lowercase().as_str() {
        "raw" => 3,      // Direct GPU backlight driver (amdgpu, intel, nvidia)
        "platform" => 2, // Platform/OEM drivers (thinkpad_acpi, ideapad, etc.)
        "firmware" => 1, // ACPI firmware fallback
        _ => 0,
    }
}

#[derive(Default)]
pub struct BrightnessModule {
    active_device: Option<BacklightDevice>,
    preferred_device_name: Option<String>,
    current_percent: u8,
    step: u8,
    min_percent: u8,
    dirty: bool,
}

impl WasmModule for BrightnessModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Brightness WASM module initializing");

        if let Some(cfg) = config.as_object() {
            if let Some(dev) = cfg.get("device").and_then(Value::as_str) {
                if !dev.is_empty() && dev != "auto" {
                    self.preferred_device_name = Some(dev.to_string());
                }
            }
            if let Some(step) = cfg.get("step").and_then(Value::as_u64) {
                self.step = (step as u8).clamp(1, 25);
            } else {
                self.step = 5;
            }
            if let Some(min) = cfg.get("min_percent").and_then(Value::as_u64) {
                self.min_percent = (min as u8).clamp(0, 50);
            } else {
                self.min_percent = 1;
            }
        } else {
            self.step = 5;
            self.min_percent = 1;
        }

        let devices = self.discover_devices();
        self.active_device = self.pick_device(&devices);
        self.current_percent = self
            .active_device
            .as_ref()
            .map(|d| d.percent())
            .unwrap_or(100);

        self.update_and_render();
        self.dirty = false;
        Ok(())
    }

    fn tick(&mut self) {
        // Periodic background poll to stay in sync with external hotkeys (Fn+F5/F6)
        if let Some(ref device) = self.active_device {
            let devices = self.discover_devices();
            if let Some(updated) = self.pick_device(&devices) {
                let dev_pct = updated.percent();
                if dev_pct != device.percent() {
                    self.current_percent = dev_pct;
                }
                self.active_device = Some(updated);
            }
        }
        self.update_and_render();
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        let current_pct = self.current_percent;

        let target_pct = if event == "scroll_up" || widget_id == "brightness_inc" {
            Some(current_pct.saturating_add(self.step).min(100))
        } else if event == "scroll_down" || widget_id == "brightness_dec" {
            Some(current_pct.saturating_sub(self.step).max(self.min_percent))
        } else if let Some(preset_str) = widget_id.strip_prefix("brightness_preset_") {
            preset_str.parse::<u8>().ok()
        } else if let Some(val_str) = event.strip_prefix("slider:") {
            val_str.parse::<u8>().ok()
        } else if let Some(val_str) = event.strip_prefix("value:") {
            val_str.parse::<u8>().ok()
        } else if let Some(val_str) = event.strip_prefix("change:") {
            val_str.parse::<u8>().ok()
        } else if let Ok(val) = event.parse::<f64>() {
            Some(val.clamp(0.0, 100.0).round() as u8)
        } else {
            None
        };

        if let Some(pct) = target_pct {
            let clamped = pct.clamp(self.min_percent, 100);
            self.current_percent = clamped;
            if let Some(device) = self.active_device.clone() {
                self.set_brightness_percent(&device, clamped);
            }
            self.update_and_render();
        }
    }
}

impl BrightnessModule {
    fn set_brightness_percent(&mut self, device: &BacklightDevice, percent: u8) {
        let target_raw = device.to_raw_value(percent, self.min_percent);
        // Standard D-Bus call to systemd-logind / elogind Session.SetBrightness (unprivileged, polkit-approved)
        let _ = dbus_call(
            "system",
            "org.freedesktop.login1",
            "/org/freedesktop/login1/session/auto",
            "org.freedesktop.login1.Session",
            "SetBrightness",
            &json!(["backlight", &device.name, target_raw as u32]),
        );

        if let Some(ref mut dev) = self.active_device {
            dev.current = target_raw;
        }
    }

    fn discover_devices(&self) -> Vec<BacklightDevice> {
        let mut devices = Vec::new();
        let entries = read_dir("/sys/class/backlight").unwrap_or_default();

        for name in entries {
            let base = format!("/sys/class/backlight/{}", name);
            let max_path = format!("{}/max_brightness", base);
            let cur_path = format!("{}/brightness", base);
            let alt_cur_path = format!("{}/actual_brightness", base);
            let type_path = format!("{}/type", base);

            let max: u64 = read_file(&max_path)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            if max == 0 {
                continue;
            }

            let current: u64 = read_file(&cur_path)
                .or_else(|| read_file(&alt_cur_path))
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(max);

            let device_type = read_file(&type_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "raw".to_string());

            devices.push(BacklightDevice {
                name,
                device_type,
                current,
                max,
            });
        }

        devices.sort_by(|a, b| {
            rank_device_type(&b.device_type).cmp(&rank_device_type(&a.device_type))
        });

        devices
    }

    fn pick_device(&self, devices: &[BacklightDevice]) -> Option<BacklightDevice> {
        if let Some(ref preferred) = self.preferred_device_name {
            if let Some(dev) = devices.iter().find(|d| d.name == *preferred) {
                return Some(dev.clone());
            }
        }
        devices.first().cloned()
    }

    fn update_and_render(&mut self) {
        let devices = self.discover_devices();
        let device = self.pick_device(&devices);
        self.active_device = device.clone();

        let percent = self.current_percent;
        let dev_name = device
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "monitor".to_string());

        let icon_glyph = if percent <= 25 {
            "󰃞"
        } else if percent <= 70 {
            "󰃟"
        } else {
            "󰃠"
        };

        let icon_path = resolve_icon("display-brightness-symbolic")
            .or_else(|| resolve_icon("display-brightness"))
            .or_else(|| resolve_icon("brightness"));

        let payload = json!({
            "text": format!("{} {}%", icon_glyph, percent),
            "percent": percent,
            "value": percent,
            "capacity": percent,
            "icon_glyph": icon_glyph,
            "device": dev_name,
            "icon_path": icon_path,
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [4.0, 4.0, 4.0, 4.0] },
                    "children": [
                        { "type": "text", "text": format!("Display Brightness ({}): {}%", dev_name, percent), "style": "clean_accent" },
                        { "type": "text", "text": icon_glyph, "style": "clean_accent" }
                    ]
                },
                {
                    "type": "slider",
                    "id": "slider_brightness",
                    "value": percent,
                    "min": self.min_percent,
                    "max": 100,
                    "on_change": "event:brightness:slider_brightness:change"
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "gap": 6.0 },
                    "children": [
                        { "type": "button", "id": "brightness_dec", "text": format!("-{}%", self.step), "on_click": "event:brightness:brightness_dec:click" },
                        { "type": "button", "id": "brightness_preset_25", "text": "25%", "on_click": "event:brightness:brightness_preset_25:click" },
                        { "type": "button", "id": "brightness_preset_50", "text": "50%", "on_click": "event:brightness:brightness_preset_50:click" },
                        { "type": "button", "id": "brightness_preset_75", "text": "75%", "on_click": "event:brightness:brightness_preset_75:click" },
                        { "type": "button", "id": "brightness_preset_100", "text": "100%", "on_click": "event:brightness:brightness_preset_100:click" },
                        { "type": "button", "id": "brightness_inc", "text": format!("+{}%", self.step), "on_click": "event:brightness:brightness_inc:click" }
                    ]
                }
            ]
        });

        send_update("brightness", &payload);
        send_update("popup:brightness", &payload);
        let _ = wyrd_module_sdk::publish("brightness.percent", &json!(percent));
        let _ = wyrd_module_sdk::publish("brightness.value", &json!(percent));
    }
}

export_wasm_module!(BrightnessModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_percent_calculation() {
        let dev = BacklightDevice {
            name: "amdgpu_bl2".to_string(),
            device_type: "raw".to_string(),
            current: 32768,
            max: 65535,
        };
        assert_eq!(dev.percent(), 50);

        let dev_zero = BacklightDevice {
            name: "amdgpu_bl2".to_string(),
            device_type: "raw".to_string(),
            current: 0,
            max: 65535,
        };
        assert_eq!(dev_zero.percent(), 0);

        let dev_full = BacklightDevice {
            name: "amdgpu_bl2".to_string(),
            device_type: "raw".to_string(),
            current: 65535,
            max: 65535,
        };
        assert_eq!(dev_full.percent(), 100);
    }

    #[test]
    fn test_to_raw_value_and_clamping() {
        let dev = BacklightDevice {
            name: "intel_backlight".to_string(),
            device_type: "raw".to_string(),
            current: 500,
            max: 1000,
        };
        assert_eq!(dev.to_raw_value(75, 1), 750);
        // Clamping to min_percent
        assert_eq!(dev.to_raw_value(0, 5), 50);
        assert_eq!(dev.to_raw_value(120, 1), 1000);
    }

    #[test]
    fn test_rank_device_type() {
        assert!(rank_device_type("raw") > rank_device_type("platform"));
        assert!(rank_device_type("platform") > rank_device_type("firmware"));
        assert!(rank_device_type("firmware") > rank_device_type("unknown"));
    }
}
