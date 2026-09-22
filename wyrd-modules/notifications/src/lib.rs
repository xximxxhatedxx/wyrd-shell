use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, log_info, notifications_clear,
    notifications_get_status, notifications_set_dnd, notifications_subscribe, now_ms, publish,
    request_surface, resolve_icon, send_update, WasmModule,
};

#[derive(Debug, Clone, Default)]
struct NotificationItem {
    id: u32,
    app_name: String,
    summary: String,
    body: String,
    _icon: String,
    image_path: Option<String>,
    _actions: Vec<String>,
    _timestamp: u64,
}

#[derive(Default)]
pub struct NotificationsModule {
    notifications: Vec<NotificationItem>,
    next_id: u32,
    dnd: bool,
    toast_id: Option<u32>,
    toast_opened_at: Option<u64>,
    toast_duration_ms: u64,
    last_count: Option<u32>,
    last_dnd: Option<bool>,
}

impl WasmModule for NotificationsModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Notifications WASM module initialized with native D-Bus integration");
        let _ = notifications_subscribe();

        let _ = dbus_subscribe(
            "session",
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "ActionInvoked",
        );
        let _ = dbus_subscribe(
            "session",
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "NotificationClosed",
        );
        let _ = dbus_subscribe(
            "session",
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        );

        self.update_count();
        self.sync_daemon_state();
        Ok(())
    }

    fn on_dbus_signal(&mut self, _interface: &str, member: &str, payload: &Value) {
        if member == "NotificationClosed" {
            if let Some(id) = payload.get("id").and_then(Value::as_u64).map(|v| v as u32) {
                if self.toast_id == Some(id) {
                    request_surface("close", &json!({ "name": "toast" }));
                    self.toast_id = None;
                    self.toast_opened_at = None;
                }
                self.notifications
                    .retain(|notification| notification.id != id);
            }
            self.send_notifications_popup();
        }
        self.update_count();
    }

    fn on_topic(&mut self, topic: &str, value: &Value) {
        if topic == "dnd.state" {
            if let Some(enabled) = value.get("enabled").and_then(|v| v.as_bool()) {
                let _ = notifications_set_dnd(enabled);
                if self.dnd != enabled {
                    self.dnd = enabled;
                    if self.dnd && self.toast_id.is_some() {
                        request_surface("close", &json!({ "name": "toast" }));
                        self.toast_id = None;
                        self.toast_opened_at = None;
                    }
                    self.update_count();
                    self.send_notifications_popup();
                }
            }
        } else if topic == "dnd.toggle" {
            self.dnd = !self.dnd;
            let _ = notifications_set_dnd(self.dnd);
            if self.dnd && self.toast_id.is_some() {
                request_surface("close", &json!({ "name": "toast" }));
                self.toast_id = None;
                self.toast_opened_at = None;
            }
            let _ = publish("dnd.state", &json!({ "enabled": self.dnd }));
            self.update_count();
            self.send_notifications_popup();
        } else if topic == "notification.new" {
            let id = value
                .get("id")
                .and_then(Value::as_u64)
                .map(|v| v as u32)
                .unwrap_or_else(|| {
                    self.next_id += 1;
                    self.next_id
                });
            self.next_id = self.next_id.max(id);

            let app = value
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("System")
                .to_string();
            let summary = value
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let body = value
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let raw_icon = value.get("icon").and_then(|v| v.as_str()).unwrap_or("");
            let resolved_icon = if !raw_icon.is_empty() {
                resolve_icon(raw_icon)
            } else {
                None
            }
            .or_else(|| resolve_icon(&app))
            .or_else(|| resolve_icon("preferences-desktop-notification"))
            .or_else(|| resolve_icon("dialog-information"));

            let image_path = value
                .get("image_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or(resolved_icon);
            let actions = value
                .get("actions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let expire_timeout = value
                .get("expire_timeout")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let item = NotificationItem {
                id,
                app_name: app,
                summary,
                body,
                _icon: raw_icon.to_string(),
                image_path,
                _actions: actions,
                _timestamp: now_ms(),
            };

            if let Some(pos) = self.notifications.iter().position(|n| n.id == id) {
                self.notifications[pos] = item.clone();
            } else {
                self.notifications.insert(0, item.clone());
            }

            if self.notifications.len() > 30 {
                self.notifications.truncate(30);
            }

            if !self.dnd {
                let duration = if expire_timeout > 0 {
                    expire_timeout as u64
                } else if expire_timeout == 0 {
                    60_000
                } else {
                    5_000
                };
                self.show_toast(&item, duration);
            }

            self.update_count();
            self.send_notifications_popup();
        } else if topic == "notification.closed" {
            if let Some(id) = value.get("id").and_then(Value::as_u64).map(|v| v as u32) {
                if self.toast_id == Some(id) {
                    request_surface("close", &json!({ "name": "toast" }));
                    self.toast_id = None;
                    self.toast_opened_at = None;
                }
                let reason = value.get("reason").and_then(Value::as_u64).unwrap_or(0);
                if reason == 2 || reason == 3 {
                    self.notifications.retain(|n| n.id != id);
                    self.update_count();
                    self.send_notifications_popup();
                }
            }
        }
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "notification.new" {
            let previous_ids: std::collections::HashSet<u32> =
                self.notifications.iter().map(|n| n.id).collect();
            self.sync_daemon_state();
            if !self.dnd {
                if let Some(notification) = self
                    .notifications
                    .iter()
                    .find(|n| !previous_ids.contains(&n.id))
                    .cloned()
                {
                    self.show_toast(&notification, 60_000);
                }
            }
            self.send_notifications_popup();
            return;
        }
        if widget_id == "toast_close" {
            request_surface("close", &json!({ "name": "toast" }));
            if let Some(id) = self.toast_id.take() {
                let _ = publish("notification.dismiss", &json!({ "id": id, "reason": 2 }));
            }
            self.toast_opened_at = None;
        } else if widget_id.starts_with("toast_action_") {
            let id_str = widget_id.trim_start_matches("toast_action_");
            if let Ok(id) = id_str.parse::<u32>() {
                let _ = publish(
                    "notification.action",
                    &json!({ "id": id, "action": "default" }),
                );
            }
            request_surface("close", &json!({ "name": "toast" }));
            self.toast_id = None;
            self.toast_opened_at = None;
        } else if widget_id.contains("dnd") || widget_id == "toggle_dnd" {
            self.dnd = !self.dnd;
            let _ = notifications_set_dnd(self.dnd);
            if self.dnd && self.toast_id.is_some() {
                request_surface("close", &json!({ "name": "toast" }));
                self.toast_id = None;
                self.toast_opened_at = None;
            }
            let _ = publish("dnd.state", &json!({ "enabled": self.dnd }));
            self.update_count();
            self.send_notifications_popup();
        } else if widget_id == "clear_all_notifs" {
            let _ = notifications_clear();
            for n in &self.notifications {
                let _ = publish("notification.dismiss", &json!({ "id": n.id, "reason": 2 }));
            }
            self.notifications.clear();
            self.update_count();
            self.send_notifications_popup();
        } else if widget_id.starts_with("dismiss_notif_") {
            let id_str = widget_id.trim_start_matches("dismiss_notif_");
            if let Ok(id) = id_str.parse::<u32>() {
                let _ = publish("notification.dismiss", &json!({ "id": id, "reason": 2 }));
                let _ = dbus_call(
                    "session",
                    "org.freedesktop.Notifications",
                    "/org/freedesktop/Notifications",
                    "org.freedesktop.Notifications",
                    "CloseNotification",
                    &json!([id]),
                );
                self.notifications.retain(|n| n.id != id);
                if self.toast_id == Some(id) {
                    request_surface("close", &json!({ "name": "toast" }));
                    self.toast_id = None;
                    self.toast_opened_at = None;
                }
                self.update_count();
                self.send_notifications_popup();
            }
        } else if event == "open" || event == "click" {
            self.send_notifications_popup();
        }
    }

    fn tick(&mut self) {
        if let (Some(opened), Some(_id)) = (self.toast_opened_at, self.toast_id) {
            let elapsed = now_ms().saturating_sub(opened);
            if elapsed >= self.toast_duration_ms {
                request_surface("close", &json!({ "name": "toast" }));
                self.toast_id = None;
                self.toast_opened_at = None;
            }
        }
    }
}

impl NotificationsModule {
    fn sync_daemon_state(&mut self) {
        let Some(status) = notifications_get_status() else {
            return;
        };
        let Some(records) = status.get("notifications").and_then(Value::as_array) else {
            return;
        };
        self.notifications = records
            .iter()
            .filter_map(|record| {
                Some(NotificationItem {
                    id: record.get("id")?.as_u64()? as u32,
                    app_name: record
                        .get("app_name")
                        .and_then(Value::as_str)
                        .unwrap_or("System")
                        .to_string(),
                    summary: record
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    body: record
                        .get("body")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    _icon: record
                        .get("icon")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    image_path: record
                        .get("image_path")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    _actions: Vec::new(),
                    _timestamp: record
                        .get("timestamp_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or_default(),
                })
            })
            .collect();
        self.dnd = status
            .get("dnd")
            .and_then(Value::as_bool)
            .unwrap_or(self.dnd);
        self.update_count();
    }

    fn update_count(&mut self) {
        let count = if self.dnd {
            0
        } else {
            self.notifications.len() as u32
        };
        if self.last_count == Some(count) && self.last_dnd == Some(self.dnd) {
            return;
        }
        self.last_count = Some(count);
        self.last_dnd = Some(self.dnd);

        let icon_name = if self.dnd {
            "notifications-disabled"
        } else {
            "preferences-desktop-notification-bell"
        };
        let icon_path = resolve_icon(icon_name);
        let icon = if self.dnd { "󰂛" } else { "󰂚" };
        let status_text = if self.dnd {
            format!("{} DND", icon)
        } else if count > 0 {
            format!("{} {}", icon, count)
        } else {
            icon.to_string()
        };

        send_update(
            "notifications",
            &json!({
                "text": status_text,
                "icon_glyph": icon,
                "count": if self.dnd { "DND".to_string() } else { count.to_string() },
                "unread": count,
                "total_count": self.notifications.len(),
                "dnd": self.dnd,
                "icon": icon_name,
                "icon_path": icon_path,
            }),
        );
        let _ = publish(
            "notification.count",
            &json!({ "count": count, "dnd": self.dnd }),
        );
    }

    fn send_notifications_popup(&self) {
        let dnd_status = if self.dnd {
            "Active (Silenced)"
        } else {
            "Inactive"
        };
        let mut children = Vec::new();

        let header_icon = resolve_icon("preferences-desktop-notification-bell");
        let mut header_left = Vec::new();
        if let Some(path) = &header_icon {
            header_left.push(json!({
                "type": if path.ends_with(".svg") { "svg" } else { "image" },
                "path": path,
                "layout": { "width": 16.0, "height": 16.0 }
            }));
        }
        header_left.push(json!({
            "type": "text",
            "text": "NOTIFICATIONS",
            "style": "clean_accent",
        }));

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [10.0, 14.0, 10.0, 14.0] },
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "gap": 8.0 },
                    "children": header_left,
                },
                {
                    "type": "button",
                    "id": "toggle_dnd",
                    "text": if self.dnd { "DND ON" } else { "DND OFF" },
                    "style": if self.dnd { "chip_accent" } else { "chip" },
                    "on_click": "event:notifications:toggle_dnd:click",
                    "layout": { "height": 28.0, "justify": "center", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
                }
            ]
        }));

        if self.notifications.is_empty() {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "align": "center", "justify": "center", "padding": [20.0, 14.0, 20.0, 14.0] },
                "children": [
                    {
                        "type": "text",
                        "text": "No new notifications",
                        "style": "muted",
                        "layout": { "padding": [4.0, 0.0, 0.0, 0.0] }
                    }
                ]
            }));
        } else {
            let mut list_children = Vec::new();
            for notif in self.notifications.iter().take(50) {
                let mut card_content = Vec::new();
                if let Some(img) = &notif.image_path {
                    let trimmed = img.trim();
                    if !trimmed.is_empty() {
                        card_content.push(json!({
                            "type": if trimmed.ends_with(".svg") { "svg" } else { "image" },
                            "path": trimmed,
                            "layout": { "width": 36.0, "height": 36.0, "border_radius": 4.0 }
                        }));
                    }
                }

                let app_label = truncate_text(&notif.app_name, 26);
                let summary_label = truncate_text(&notif.summary, 48);
                let body_label = truncate_text(&notif.body, 140);

                card_content.push(json!({
                    "type": "container",
                    "layout": { "mode": "flex_col", "gap": 2.0, "weight": 1.0 },
                    "children": [
                        {
                            "type": "text",
                            "text": summary_label,
                            "style": "card_title",
                        },
                        {
                            "type": "text",
                            "text": body_label,
                            "style": "muted",
                        }
                    ]
                }));

                list_children.push(json!({
                    "type": "container",
                    "style": "card",
                    "layout": { "mode": "flex_col", "gap": 2.0, "padding": [8.0, 10.0, 8.0, 10.0] },
                    "children": [
                        {
                            "type": "container",
                            "layout": { "mode": "flex_row", "justify": "space-between", "align": "center" },
                            "children": [
                                {
                                    "type": "text",
                                    "text": app_label,
                                    "style": "clean_accent",
                                },
                                {
                                    "type": "button",
                                    "id": format!("dismiss_notif_{}", notif.id),
                                    "text": "✕",
                                    "style": "notif_close",
                                    "on_click": format!("event:notifications:dismiss_notif_{}:click", notif.id),
                                    "layout": { "width": 20.0, "height": 20.0, "justify": "center", "align": "center", "padding": [0.0, 0.0, 0.0, 0.0] }
                                }
                            ]
                        },
                        {
                            "type": "container",
                            "layout": { "mode": "flex_row", "gap": 8.0, "align": "center" },
                            "children": card_content
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
                    "max_height": 420.0
                },
                "children": list_children
            }));

            children.push(json!({
                "type": "button",
                "id": "clear_all_notifs",
                "text": "Clear All Notifications",
                "style": "chip",
                "on_click": "event:notifications:clear_all_notifs:click",
                "layout": { "height": 30.0, "justify": "center", "align": "center" }
            }));
        }

        let payload = json!({
            "type": "container",
            "style": "popup",
            "layout": { "mode": "flex_col", "gap": 8.0, "padding": [12.0, 12.0, 12.0, 12.0], "width": 320.0 },
            "children": children,
            "count": self.notifications.len(),
            "dnd": self.dnd,
            "dnd_status": dnd_status,
        });

        send_update("notifications:popup", &payload);
        send_update("popup:notifications", &payload);
    }

    fn show_toast(&mut self, notif: &NotificationItem, duration_ms: u64) {
        self.toast_id = Some(notif.id);
        self.toast_opened_at = Some(now_ms());
        self.toast_duration_ms = duration_ms;

        let toast_app = truncate_text(&notif.app_name, 26);
        let toast_summary = truncate_text(&notif.summary, 48);
        let toast_body = truncate_text(&notif.body, 140);

        let mut body_children = Vec::new();
        body_children.push(json!({
            "type": "text",
            "text": toast_summary,
            "style": "card_title",
        }));
        if !notif.body.is_empty() {
            body_children.push(json!({
                "type": "text",
                "text": toast_body,
                "style": "muted",
            }));
        }

        let mut content_children = Vec::new();
        if let Some(img) = &notif.image_path {
            let trimmed = img.trim();
            if !trimmed.is_empty() {
                content_children.push(json!({
                    "type": if trimmed.ends_with(".svg") { "svg" } else { "image" },
                    "path": trimmed,
                    "layout": { "width": 40.0, "height": 40.0, "border_radius": 6.0 }
                }));
            }
        }
        content_children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_col", "gap": 2.0, "flex_grow": 1.0 },
            "children": body_children,
        }));

        let toast_tree = json!({
            "type": "container",
            "style": "popup",
            "layout": { "mode": "flex_col", "padding": [10.0, 14.0, 10.0, 14.0], "gap": 6.0 },
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "justify": "space-between" },
                    "children": [
                        {
                            "type": "text",
                            "text": toast_app,
                            "style": "clean_accent",
                        },
                        {
                            "type": "button",
                            "id": "toast_close",
                            "text": "✕",
                            "style": "notif_close",
                            "on_click": "event:notifications:toast_close:click",
                            "layout": { "width": 20.0, "height": 20.0, "justify": "center", "align": "center", "padding": [0.0, 0.0, 0.0, 0.0] }
                        }
                    ]
                },
                {
                    "type": "button",
                    "id": format!("toast_action_{}", notif.id),
                    "style": "card",
                    "layout": { "mode": "flex_row", "gap": 10.0, "align": "center", "padding": [4.0, 6.0, 4.0, 6.0] },
                    "children": content_children,
                    "on_click": format!("event:notifications:toast_action_{}:click", notif.id),
                }
            ]
        });

        send_update("popup:toast", &toast_tree);
        request_surface(
            "open",
            &json!({ "name": "toast", "timeout_ms": duration_ms }),
        );
    }
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

export_wasm_module!(NotificationsModule);
