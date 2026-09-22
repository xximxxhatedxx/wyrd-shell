use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, resolve_icon, send_update, unix_socket_request,
    windows_subscribe, WasmModule,
};

#[derive(Default)]
pub struct WorkspacesModule {
    last_payload: Option<Value>,
}

impl WasmModule for WorkspacesModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Workspaces module initialized with wyrd-windows socket");
        let _ = windows_subscribe();
        self.update_workspaces();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, _event: &str) {
        if widget_id == "changed" || widget_id == "wyrd-windows" {
            self.update_workspaces();
            return;
        }
        let id = widget_id
            .strip_prefix("workspace_")
            .or_else(|| widget_id.strip_prefix("ws-"))
            .or_else(|| widget_id.strip_prefix("ws_"));
        if let Some(id) = id {
            let request = json!({"cmd": "switch_workspace", "id": id});
            let _ = unix_socket_request("wyrd-windows.sock", &request.to_string());
            self.update_workspaces();
        }
    }
}

impl WorkspacesModule {
    fn update_workspaces(&mut self) {
        let response = unix_socket_request("wyrd-windows.sock", r#"{"cmd":"workspaces"}"#)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
        let workspaces = response
            .as_ref()
            .and_then(|value| value.get("workspaces").or_else(|| value.get("list")))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut children = Vec::new();
        let mut chips = Vec::new();
        let mut active_workspace = Value::Null;
        for workspace in workspaces {
            let id = workspace.get("id").cloned().unwrap_or(Value::Null);
            let id_text = id
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| id.to_string());
            let name = workspace
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let active = workspace
                .get("active")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let monitor = workspace
                .get("monitor")
                .and_then(Value::as_str)
                .unwrap_or("");
            if active {
                active_workspace = id.clone();
            }
            let label = if name.is_empty() {
                id_text.clone()
            } else {
                name
            };
            let app_icons = workspace
                .get("apps")
                .and_then(Value::as_array)
                .map(|apps| {
                    apps.iter()
                        .filter_map(|app| {
                            resolve_icon(app.get("app_id").and_then(Value::as_str).unwrap_or(""))
                                .or_else(|| resolve_icon("applications-other"))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            chips.push(json!({"id": id, "name": label, "label": label, "active": active, "focused": active, "monitor": monitor, "apps": app_icons}));
            let mut button_children = vec![json!({"type": "text", "text": label})];
            for icon in &app_icons {
                button_children.push(json!({"type": if icon.ends_with(".svg") { "svg" } else { "image" }, "path": icon, "layout": {"width": 16.0, "height": 16.0}}));
            }
            children.push(json!({
                "type": "button",
                "id": format!("ws-{}", id_text),
                "monitor": monitor,
                "children": button_children,
                "style": if active { "chip_accent" } else { "chip" },
                "on_click": format!("event:workspaces:workspace_{}:click", id_text),
                "layout": {"mode": "flex_row", "gap": 8.0, "height": 28.0, "justify": "center", "align": "center", "padding": [2.0, 12.0, 2.0, 12.0]}
            }));
        }

        let payload = json!({"children": children, "workspaces": chips, "active_workspace": active_workspace});
        if self.last_payload.as_ref() != Some(&payload) {
            self.last_payload = Some(payload.clone());
            send_update("workspaces", &payload);
        }
    }
}

export_wasm_module!(WorkspacesModule);
