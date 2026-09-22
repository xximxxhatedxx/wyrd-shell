//! WASM Keyboard Layout Indicator & Switcher Module for Wyrd Shell.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, send_update, unix_socket_request, windows_subscribe, WasmModule,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct KeyboardInfo {
    pub layout: String,
    pub short_name: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub layouts: Vec<String>,
    #[serde(default)]
    pub short_layouts: Vec<String>,
    #[serde(default)]
    pub device_name: String,
}

pub fn format_layout(info: &KeyboardInfo, format: &str, icon: &str) -> String {
    let short = if info.short_name.is_empty() {
        "EN"
    } else {
        &info.short_name
    };
    let full = if info.layout.is_empty() {
        "English (US)"
    } else {
        &info.layout
    };

    match format {
        "full" => full.to_string(),
        "icon_short" => format!("{} {}", icon, short),
        "icon_full" => format!("{} {}", icon, full),
        "code" => short.to_string(),
        _ => short.to_string(), // "short" or default
    }
}

pub struct KeyboardModule {
    current: KeyboardInfo,
    format: String,
    icon: String,
}

impl Default for KeyboardModule {
    fn default() -> Self {
        Self {
            current: KeyboardInfo {
                layout: "English (US)".to_string(),
                short_name: "US".to_string(),
                variant: String::new(),
                index: 0,
                layouts: vec!["English (US)".to_string()],
                short_layouts: vec!["US".to_string()],
                device_name: String::new(),
            },
            format: "short".to_string(),
            icon: "󰌌".to_string(),
        }
    }
}

impl WasmModule for KeyboardModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Keyboard layout WASM module initialized");

        if let Some(cfg) = config.as_object() {
            if let Some(fmt) = cfg.get("format").and_then(Value::as_str) {
                self.format = fmt.to_string();
            }
            if let Some(ic) = cfg.get("icon").and_then(Value::as_str) {
                self.icon = ic.to_string();
            }
        }

        let _ = windows_subscribe();
        self.update_keyboard();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "changed" || widget_id == "wyrd-windows" {
            self.update_keyboard();
            return;
        }

        if event == "click" || event == "action" {
            if widget_id == "keyboard" || widget_id == "keyboard_toggle" {
                // Cycle to next layout
                let _ = unix_socket_request(
                    "wyrd-windows.sock",
                    r#"{"cmd":"switch_layout","id":"next"}"#,
                );
                self.update_keyboard();
            } else if let Some(idx_str) = widget_id.strip_prefix("keyboard_select_") {
                // Switch directly to selected layout index
                let req = json!({
                    "cmd": "switch_layout",
                    "id": idx_str
                })
                .to_string();
                let _ = unix_socket_request("wyrd-windows.sock", &req);
                self.update_keyboard();
            }
        }
    }

    fn tick(&mut self) {
        self.update_keyboard();
    }
}

impl KeyboardModule {
    fn update_keyboard(&mut self) {
        if let Ok(raw) = unix_socket_request("wyrd-windows.sock", r#"{"cmd":"keyboard"}"#) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
                if let Some(kb_val) = parsed.get("keyboard") {
                    if !kb_val.is_null() {
                        if let Ok(kb) = serde_json::from_value::<KeyboardInfo>(kb_val.clone()) {
                            self.current = kb;
                        }
                    }
                }
            }
        }

        let text = format_layout(&self.current, &self.format, &self.icon);

        // Build popup layout items
        let mut items = Vec::new();
        items.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "align": "center", "gap": 8.0, "padding": [4.0, 4.0, 4.0, 4.0] },
            "children": [
                { "type": "text", "text": &self.icon, "style": "clean_accent" },
                {
                    "type": "text",
                    "text": if self.current.device_name.is_empty() {
                        "Keyboard Layout".to_string()
                    } else {
                        format!("Keyboard ({})", self.current.device_name)
                    },
                    "style": "clean_accent"
                }
            ]
        }));

        let active_idx = self.current.index as usize;
        let num_layouts = self
            .current
            .layouts
            .len()
            .max(self.current.short_layouts.len());

        for i in 0..num_layouts {
            let full_name = self.current.layouts.get(i).cloned().unwrap_or_default();
            let short = self
                .current
                .short_layouts
                .get(i)
                .cloned()
                .unwrap_or_default();
            let label = if !full_name.is_empty() && !short.is_empty() {
                format!("{} ({})", full_name, short)
            } else if !full_name.is_empty() {
                full_name
            } else {
                short
            };

            let is_active = i == active_idx;
            let display_text = format!("{} {}", if is_active { "●" } else { "○" }, label);

            items.push(json!({
                "type": "button",
                "id": format!("keyboard_select_{}", i),
                "text": display_text,
                "style": if is_active { "chip_active" } else { "chip" },
                "on_click": format!("event:keyboard:keyboard_select_{}:click", i)
            }));
        }

        let payload = json!({
            "text": text,
            "layout": self.current.layout,
            "short_name": self.current.short_name,
            "index": self.current.index,
            "layouts": self.current.layouts,
            "short_layouts": self.current.short_layouts,
            "device": self.current.device_name,
            "icon": self.icon,
            "children": items
        });

        send_update("keyboard", &payload);
        send_update("popup:keyboard", &payload);
    }
}

export_wasm_module!(KeyboardModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_layout() {
        let kb = KeyboardInfo {
            layout: "Russian".to_string(),
            short_name: "RU".to_string(),
            variant: String::new(),
            index: 1,
            layouts: vec!["English (US)".to_string(), "Russian".to_string()],
            short_layouts: vec!["US".to_string(), "RU".to_string()],
            device_name: "epomaker".to_string(),
        };

        assert_eq!(format_layout(&kb, "short", "󰌌"), "RU");
        assert_eq!(format_layout(&kb, "full", "󰌌"), "Russian");
        assert_eq!(format_layout(&kb, "icon_short", "󰌌"), "󰌌 RU");
        assert_eq!(format_layout(&kb, "icon_full", "󰌌"), "󰌌 Russian");
        assert_eq!(format_layout(&kb, "code", "󰌌"), "RU");
    }

    #[test]
    fn test_default_keyboard_info() {
        let kb = KeyboardInfo::default();
        assert_eq!(format_layout(&kb, "short", "󰌌"), "EN");
    }
}
