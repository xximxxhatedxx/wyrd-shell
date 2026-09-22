//! WASM Clipboard History Module — event-driven via unix_socket_subscribe.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, log_warn, request_surface, resolve_icon, send_update,
    unix_socket_request, unix_socket_subscribe, WasmModule,
};

const SOCKET_NAME: &str = "wyrd-clipboard.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: u64,
    pub text_preview: String,
    pub mime: String,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    #[serde(default)]
    entries: Vec<ClipboardItem>,
}

#[derive(Default)]
pub struct ClipboardModule {
    items: Vec<ClipboardItem>,
    daemon_available: bool,
}

impl WasmModule for ClipboardModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Clipboard WASM module initialized with wyrd-clipboard socket integration");
        // Subscribe for push notifications — no polling needed.
        match unix_socket_subscribe(SOCKET_NAME) {
            Ok(()) => log_info("Clipboard subscribed for push events"),
            Err(code) => log_warn(&format!(
                "Clipboard subscribe failed (code {}), falling back to lazy fetch",
                code
            )),
        }
        self.refresh_items();
        self.render_bar();
        self.render_popup();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        // Push notification from daemon via unix_socket_subscribe
        if widget_id == SOCKET_NAME && event == "changed" {
            self.refresh_items();
            self.render_bar();
            self.render_popup();
            return;
        }

        // Popup opened — refresh to show latest items
        if event == "open" {
            self.refresh_items();
            self.render_bar();
            self.render_popup();
            return;
        }

        if event == "click" {
            if widget_id == "clipboard" {
                request_surface("toggle", &json!({ "name": "clipboard" }));
            } else if let Some(id_str) = widget_id.strip_prefix("copy_entry_") {
                if let Ok(id) = id_str.parse::<u64>() {
                    let cmd = format!("{{\"cmd\":\"select\",\"id\":{}}}", id);
                    let _ = unix_socket_request(SOCKET_NAME, &cmd);
                    request_surface("close", &json!({ "name": "clipboard" }));
                }
            } else if widget_id == "clipboard_clear" {
                let _ = unix_socket_request(SOCKET_NAME, "{\"cmd\":\"clear\"}");
                self.items.clear();
                self.daemon_available = true;
                self.render_bar();
                self.render_popup();
            }
        }
    }

    // No tick() — all updates are event-driven via unix_socket_subscribe.
}

impl ClipboardModule {
    fn refresh_items(&mut self) {
        match unix_socket_request(SOCKET_NAME, "{\"cmd\":\"list\"}") {
            Ok(resp_str) => {
                self.daemon_available = true;
                if let Ok(resp) = serde_json::from_str::<ListResponse>(&resp_str) {
                    self.items = resp.entries;
                }
            }
            Err(_) => {
                if self.daemon_available || !self.items.is_empty() {
                    self.daemon_available = false;
                    self.items.clear();
                }
            }
        }
    }

    fn render_bar(&self) {
        let clip_icon = resolve_icon("edit-copy").or_else(|| resolve_icon("edit-paste"));
        let tooltip = if self.daemon_available {
            format!("Clipboard ({} items)", self.items.len())
        } else {
            "Clipboard (daemon inactive)".to_string()
        };

        send_update(
            "clipboard",
            &json!({
                "text": if clip_icon.is_some() { "" } else { "Clip" },
                "icon": "edit-copy",
                "icon_path": clip_icon,
                "path": clip_icon,
                "style": "chip",
                "tooltip": tooltip,
                "on_click": "surface:toggle:clipboard"
            }),
        );
    }

    fn render_popup(&self) {
        let mut children = Vec::new();

        // Header
        children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "justify": "space-between", "align": "center", "padding": [4.0, 4.0, 6.0, 4.0] },
            "children": [
                {
                    "type": "text",
                    "text": if self.daemon_available {
                        format!("Clipboard ({})", self.items.len())
                    } else {
                        "Clipboard".to_string()
                    },
                    "style": "card_title",
                },
                {
                    "type": "button",
                    "id": "clipboard_clear",
                    "text": "Clear",
                    "style": "chip",
                    "on_click": "event:clipboard:clipboard_clear:click",
                    "layout": { "padding": [2.0, 6.0, 2.0, 6.0] }
                }
            ]
        }));

        // Items or Empty State
        if !self.daemon_available {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "align": "center", "justify": "center", "padding": [24.0, 14.0, 24.0, 14.0] },
                "children": [
                    {
                        "type": "text",
                        "text": "Clipboard daemon inactive",
                        "style": "muted",
                        "layout": { "padding": [6.0, 0.0, 0.0, 0.0] }
                    }
                ]
            }));
        } else if self.items.is_empty() {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "align": "center", "justify": "center", "padding": [24.0, 14.0, 24.0, 14.0] },
                "children": [
                    {
                        "type": "text",
                        "text": "Clipboard is empty",
                        "style": "muted",
                        "layout": { "padding": [6.0, 0.0, 0.0, 0.0] }
                    }
                ]
            }));
        } else {
            let mut list_children = Vec::new();
            for item in self.items.iter().take(50) {
                let clean_preview = item.text_preview.replace('\n', " ").replace('\r', "");

                list_children.push(json!({
                    "type": "button",
                    "id": format!("copy_entry_{}", item.id),
                    "style": "card",
                    "on_click": format!("event:clipboard:copy_entry_{}:click", item.id),
                    "layout": { "mode": "flex_col", "gap": 2.0, "padding": [6.0, 8.0, 6.0, 8.0] },
                    "children": [
                        {
                            "type": "text",
                            "text": clean_preview,
                            "style": "default",
                        }
                    ]
                }));
            }

            children.push(json!({
                "type": "container",
                "layout": {
                    "mode": "flex_col",
                    "gap": 6.0,
                    "scroll_y": true,
                    "max_height": 380.0
                },
                "children": list_children
            }));
        }

        let popup_tree = json!({
            "type": "container",
            "style": "popup",
            "layout": { "mode": "flex_col", "gap": 8.0, "padding": [12.0, 12.0, 12.0, 12.0] },
            "children": children
        });

        send_update("popup:clipboard", &popup_tree);
        send_update("clipboard:popup", &popup_tree);
        send_update("popup_root", &popup_tree);
    }
}

export_wasm_module!(ClipboardModule);
