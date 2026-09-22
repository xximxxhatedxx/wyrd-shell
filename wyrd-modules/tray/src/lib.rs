//! WASM System Tray Module.
//!
//! D-Bus StatusNotifierWatcher client for system tray items.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, file_exists, log_info, request_surface,
    resolve_icon, send_update, WasmModule,
};

#[derive(Debug, Clone, Default)]
struct TrayItemData {
    id: String,
    service: String,
    path: String,
    menu: String,
    title: String,
    image: Option<String>,
    item_is_menu: bool,
}

#[derive(Debug, Clone)]
struct DBusMenuItem {
    id: i32,
    label: String,
    is_separator: bool,
    enabled: bool,
    is_submenu: bool,
    toggle_type: Option<String>,
    toggle_state: Option<i32>,
    indent_level: usize,
}

#[derive(Default)]
pub struct TrayModule {
    last_item_count: usize,
    items: Vec<TrayItemData>,
    active_menu_service: String,
    active_menu_path: String,
}

impl WasmModule for TrayModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Tray WASM module initialized with native D-Bus integration");

        // Subscribe to StatusNotifierWatcher registration signals
        let _ = dbus_subscribe(
            "session",
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "StatusNotifierItemRegistered",
        );
        let _ = dbus_subscribe(
            "session",
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "StatusNotifierItemUnregistered",
        );
        let _ = dbus_call(
            "session",
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "RegisterStatusNotifierHost",
            &json!(["wyrd"]),
        );

        self.update_tray();
        Ok(())
    }

    fn on_dbus_signal(&mut self, _interface: &str, _member: &str, _payload: &Value) {
        self.update_tray();
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if let Some(rest) = widget_id.strip_prefix("menu_item_") {
            let (target_service, target_menu, id) =
                if let Some((idx_str, id_str)) = rest.split_once('_') {
                    let id = id_str.parse::<i32>().unwrap_or(0);
                    let idx = idx_str.parse::<usize>().ok();
                    let item = idx.and_then(|i| self.items.get(i));
                    let s = item
                        .map(|i| i.service.clone())
                        .unwrap_or_else(|| self.active_menu_service.clone());
                    let m = item
                        .map(|i| i.menu.clone())
                        .unwrap_or_else(|| self.active_menu_path.clone());
                    (s, m, id)
                } else {
                    let id = rest.parse::<i32>().unwrap_or(0);
                    (
                        self.active_menu_service.clone(),
                        self.active_menu_path.clone(),
                        id,
                    )
                };

            if id >= 0 && !target_service.is_empty() && !target_menu.is_empty() {
                log_info(&format!(
                    "Tray activating menu item {} on {} {}",
                    id, target_service, target_menu
                ));
                let _ = dbus_call(
                    "session",
                    &target_service,
                    &target_menu,
                    "com.canonical.dbusmenu",
                    "Event",
                    &json!([id, "clicked", 0, 0]),
                );
            }
            request_surface("close", &json!({ "id": "tray_menu" }));
            return;
        }

        if let Some(payload) = widget_id.strip_prefix("menu_click|") {
            let parts: Vec<&str> = payload.split('|').collect();
            if parts.len() == 3 {
                let service = parts[0];
                let menu_path = parts[1];
                if let Ok(id) = parts[2].parse::<i32>() {
                    log_info(&format!(
                        "Tray activating menu click {} on {} {}",
                        id, service, menu_path
                    ));
                    let _ = dbus_call(
                        "session",
                        service,
                        menu_path,
                        "com.canonical.dbusmenu",
                        "Event",
                        &json!([id, "clicked", 0, 0]),
                    );
                }
            }
            request_surface("close", &json!({ "id": "tray_menu" }));
            return;
        }

        if widget_id.starts_with("tray_") {
            let event_parts: Vec<&str> = event.split(':').collect();
            let evt_type = event_parts.first().copied().unwrap_or(event);
            let gx: i32 = event_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let gy: i32 = event_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

            let item_key = widget_id.strip_prefix("tray_").unwrap_or(widget_id);
            if let Some((idx, item)) = self.items.iter().enumerate().find(|(_, i)| {
                let key = format!(
                    "{}_{}",
                    i.service.replace(['.', ':'], "_"),
                    i.path.replace(['.', '/'], "_")
                );
                key == item_key || i.id.replace(['.', ':'], "_") == item_key
            }) {
                if evt_type == "right_click" || evt_type == "context" {
                    // ALWAYS show Wyrd Shell's context menu in the style of the bar!
                    // NEVER call item.ContextMenu(gx, gy) to spawn app's native window!
                    if !item.menu.is_empty() {
                        self.active_menu_service = item.service.clone();
                        self.active_menu_path = item.menu.clone();

                        let _ = dbus_call(
                            "session",
                            &item.service,
                            &item.menu,
                            "com.canonical.dbusmenu",
                            "AboutToShow",
                            &json!([0]),
                        );

                        if let Some(layout_val) = dbus_call(
                            "session",
                            &item.service,
                            &item.menu,
                            "com.canonical.dbusmenu",
                            "GetLayout",
                            &json!([0, 2, []]),
                        ) {
                            let parsed = parse_dbus_menu_items(&layout_val);
                            if !parsed.is_empty() {
                                let menu_widgets = build_tray_menu_widgets(idx, item, &parsed);
                                send_update("popup:tray_menu", &menu_widgets);
                                let margin_left = (gx - 20).clamp(16, 1920 - 270);
                                request_surface(
                                    "open",
                                    &json!({
                                        "id": "tray_menu",
                                        "margin_left": margin_left,
                                        "margin_top": 58
                                    }),
                                );
                                return;
                            }
                        }
                    }

                    // Fallback to SecondaryActivate only if app has no DBusMenu layout
                    let _ = dbus_call(
                        "session",
                        &item.service,
                        &item.path,
                        "org.kde.StatusNotifierItem",
                        "SecondaryActivate",
                        &json!([gx, gy]),
                    );
                } else if evt_type == "middle_click" {
                    let res = dbus_call(
                        "session",
                        &item.service,
                        &item.path,
                        "org.kde.StatusNotifierItem",
                        "SecondaryActivate",
                        &json!([gx, gy]),
                    );
                    if res.is_none() {
                        let _ = dbus_call(
                            "session",
                            &item.service,
                            &item.path,
                            "org.kde.StatusNotifierItem",
                            "Activate",
                            &json!([gx, gy]),
                        );
                    }
                } else {
                    // Left click:
                    // 1. If declared menu-only item (e.g. nm-applet)
                    if item.item_is_menu {
                        if !item.menu.is_empty() {
                            self.active_menu_service = item.service.clone();
                            self.active_menu_path = item.menu.clone();
                            let _ = dbus_call(
                                "session",
                                &item.service,
                                &item.menu,
                                "com.canonical.dbusmenu",
                                "AboutToShow",
                                &json!([0]),
                            );
                            if let Some(layout_val) = dbus_call(
                                "session",
                                &item.service,
                                &item.menu,
                                "com.canonical.dbusmenu",
                                "GetLayout",
                                &json!([0, 2, []]),
                            ) {
                                let parsed = parse_dbus_menu_items(&layout_val);
                                if !parsed.is_empty() {
                                    let menu_widgets = build_tray_menu_widgets(idx, item, &parsed);
                                    send_update("popup:tray_menu", &menu_widgets);
                                    let margin_left = (gx - 20).clamp(16, 1920 - 270);
                                    request_surface(
                                        "open",
                                        &json!({
                                            "id": "tray_menu",
                                            "margin_left": margin_left,
                                            "margin_top": 58
                                        }),
                                    );
                                }
                            }
                        }
                        let _ = dbus_call(
                            "session",
                            &item.service,
                            &item.path,
                            "org.kde.StatusNotifierItem",
                            "SecondaryActivate",
                            &json!([gx, gy]),
                        );
                        return;
                    }

                    // 2. Normal desktop application: Activate window
                    let res = dbus_call(
                        "session",
                        &item.service,
                        &item.path,
                        "org.kde.StatusNotifierItem",
                        "Activate",
                        &json!([gx, gy]),
                    );
                    if res.is_some() {
                        return;
                    }

                    // 3. SecondaryActivate
                    let res_sec = dbus_call(
                        "session",
                        &item.service,
                        &item.path,
                        "org.kde.StatusNotifierItem",
                        "SecondaryActivate",
                        &json!([gx, gy]),
                    );
                    if res_sec.is_some() {
                        return;
                    }

                    // 4. DBusMenu fallback for menu-only items that didn't set ItemIsMenu (like nm-applet)
                    if !item.menu.is_empty() {
                        self.active_menu_service = item.service.clone();
                        self.active_menu_path = item.menu.clone();
                        let _ = dbus_call(
                            "session",
                            &item.service,
                            &item.menu,
                            "com.canonical.dbusmenu",
                            "AboutToShow",
                            &json!([0]),
                        );
                        if let Some(layout_val) = dbus_call(
                            "session",
                            &item.service,
                            &item.menu,
                            "com.canonical.dbusmenu",
                            "GetLayout",
                            &json!([0, 2, []]),
                        ) {
                            let parsed = parse_dbus_menu_items(&layout_val);
                            if !parsed.is_empty() {
                                let menu_widgets = build_tray_menu_widgets(idx, item, &parsed);
                                send_update("popup:tray_menu", &menu_widgets);
                                let margin_left = (gx - 20).clamp(16, 1920 - 270);
                                request_surface(
                                    "open",
                                    &json!({
                                        "id": "tray_menu",
                                        "margin_left": margin_left,
                                        "margin_top": 58
                                    }),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn tick(&mut self) {
        let Some((items_json, item_records)) = get_tray_items() else {
            return;
        };
        let items_differ = items_json.len() != self.last_item_count
            || item_records.len() != self.items.len()
            || item_records.iter().zip(self.items.iter()).any(|(a, b)| {
                a.id != b.id || a.title != b.title || a.image != b.image || a.service != b.service
            });
        if items_differ {
            self.last_item_count = items_json.len();
            self.items = item_records;
            self.send_tray_update(&items_json);
        }
    }
}

impl TrayModule {
    fn update_tray(&mut self) {
        let Some((items_json, item_records)) = get_tray_items() else {
            return;
        };
        self.last_item_count = items_json.len();
        self.items = item_records;
        for item in &self.items {
            let _ = dbus_subscribe(
                "session",
                &item.service,
                &item.path,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
            );
        }
        self.send_tray_update(&items_json);
    }

    fn send_tray_update(&self, items_json: &[Value]) {
        let mut children = Vec::new();
        for item in items_json {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("item");
            let btn_id = item.get("btn_id").and_then(|v| v.as_str()).unwrap_or(id);
            let image_path = item.get("image").and_then(|v| v.as_str());

            let mut btn_obj = json!({
                "type": "button",
                "id": btn_id,
                "style": "tray_item",
                "on_click": format!("event:tray:{}:click", btn_id),
                "on_right_click": format!("event:tray:{}:right_click", btn_id),
                "layout": { "width": 24.0, "height": 24.0, "justify": "center", "align": "center" }
            });

            if let Some(img) = image_path {
                btn_obj["path"] = json!(img);
            } else if let Some(def_img) = resolve_icon("applications-other") {
                btn_obj["path"] = json!(def_img);
            } else {
                btn_obj["text"] = json!("•");
            }

            children.push(btn_obj);
        }

        send_update(
            "tray",
            &json!({
                "type": "container",
                "id": "tray",
                "layout": { "mode": "flex_row", "align": "center", "gap": 6.0 },
                "children": children,
                "items": items_json,
                "count": self.last_item_count,
            }),
        );
    }
}

fn parse_dbus_menu_items(layout: &Value) -> Vec<DBusMenuItem> {
    let mut items = Vec::new();
    let root_node = if let Some(arr) = layout.as_array() {
        if arr.len() == 2 && arr.get(1).and_then(|v| v.as_array()).is_some() {
            // Standard D-Bus GetLayout reply: [revision, (id, props, children)]
            arr.get(1).and_then(|v| v.as_array())
        } else {
            // Direct (id, props, children) tuple
            Some(arr)
        }
    } else {
        None
    };

    let Some(root_arr) = root_node else {
        return items;
    };

    let children = root_arr.get(2).and_then(|v| v.as_array());
    let Some(child_list) = children else {
        return items;
    };

    parse_dbus_children_recursive(child_list, 0, &mut items);
    items
}

fn parse_dbus_children_recursive(
    child_list: &[Value],
    indent_level: usize,
    items: &mut Vec<DBusMenuItem>,
) {
    for child in child_list {
        let Some(c_arr) = child.as_array() else {
            continue;
        };
        let id = c_arr.first().and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let props = c_arr.get(1).and_then(|v| v.as_object());
        let nested_children = c_arr.get(2).and_then(|v| v.as_array());

        let mut label = String::new();
        let mut is_separator = false;
        let mut enabled = true;
        let mut visible = true;
        let mut toggle_type = None;
        let mut toggle_state = None;
        let mut children_display = String::new();

        if let Some(p) = props {
            if let Some(l) = p.get("label").and_then(|v| v.as_str()) {
                let clean_label = l
                    .replace("__", "\x00")
                    .replace('_', "")
                    .replace('\x00', "_");
                label = clean_label;
            }
            if let Some(t) = p.get("type").and_then(|v| v.as_str()) {
                if t == "separator" {
                    is_separator = true;
                }
            }
            if let Some(en) = p.get("enabled").and_then(|v| v.as_bool()) {
                enabled = en;
            }
            if let Some(vis) = p.get("visible").and_then(|v| v.as_bool()) {
                visible = vis;
            }
            if let Some(tt) = p.get("toggle-type").and_then(|v| v.as_str()) {
                toggle_type = Some(tt.to_string());
            }
            if let Some(ts) = p.get("toggle-state").and_then(|v| v.as_i64()) {
                toggle_state = Some(ts as i32);
            }
            if let Some(cd) = p.get("children-display").and_then(|v| v.as_str()) {
                children_display = cd.to_string();
            }
        }

        if !visible {
            continue;
        }

        let has_children = nested_children.is_some_and(|c| !c.is_empty());
        let is_submenu = has_children || children_display == "submenu";

        items.push(DBusMenuItem {
            id,
            label,
            is_separator,
            enabled,
            is_submenu,
            toggle_type,
            toggle_state,
            indent_level,
        });

        if let Some(nested) = nested_children {
            if !nested.is_empty() {
                parse_dbus_children_recursive(nested, indent_level + 1, items);
            }
        }
    }
}

fn build_tray_menu_widgets(
    item_idx: usize,
    item: &TrayItemData,
    menu_items: &[DBusMenuItem],
) -> Value {
    let mut children = Vec::new();

    let title = if !item.title.is_empty() {
        &item.title
    } else {
        &item.id
    };

    let mut header_row = Vec::new();
    if let Some(img) = &item.image {
        header_row.push(json!({
            "type": if img.ends_with(".svg") { "svg" } else { "image" },
            "path": img,
            "layout": { "width": 16.0, "height": 16.0 }
        }));
    }
    header_row.push(json!({
        "type": "text",
        "text": title,
        "style": "menu_header",
    }));

    children.push(json!({
        "type": "container",
        "layout": { "mode": "flex_row", "align": "center", "gap": 8.0, "padding": [4.0, 6.0, 4.0, 6.0] },
        "children": header_row
    }));
    children.push(json!({
        "type": "container",
        "style": "menu_separator",
        "layout": { "height": 1.0, "width": 234.0 }
    }));

    for mi in menu_items {
        if mi.is_separator {
            children.push(json!({
                "type": "container",
                "style": "menu_separator",
                "layout": { "height": 1.0, "width": 234.0 }
            }));
            continue;
        }

        if mi.is_submenu && mi.indent_level == 0 {
            children.push(json!({
                "type": "text",
                "text": if !mi.label.is_empty() { format!("▾ {}", mi.label) } else { "Submenu".to_string() },
                "style": "menu_section_header",
            }));
            continue;
        }

        let style = if mi.enabled {
            "menu_item"
        } else {
            "menu_item_disabled"
        };
        let on_click = if mi.enabled {
            format!("event:tray:menu_item_{}_{}:click", item_idx, mi.id)
        } else {
            String::new()
        };

        let mut display_text = if !mi.label.is_empty() {
            mi.label.clone()
        } else {
            "Item".to_string()
        };
        if let Some(ref tt) = mi.toggle_type {
            if tt == "checkmark" || tt == "radio" {
                if mi.toggle_state == Some(1) {
                    display_text = format!("󰄲  {}", display_text);
                } else {
                    display_text = format!("󰄱  {}", display_text);
                }
            }
        }
        if mi.indent_level > 0 {
            display_text = format!("    {}", display_text);
        }

        children.push(json!({
            "type": "button",
            "text": display_text,
            "style": style,
            "on_click": on_click,
            "layout": { "align": "start", "justify": "start" }
        }));
    }

    json!({
        "type": "container",
        "style": "tray_menu_popup",
        "layout": {
            "mode": "flex_col",
            "gap": 3.0,
            "align": "stretch"
        },
        "children": children
    })
}

fn get_tray_items() -> Option<(Vec<Value>, Vec<TrayItemData>)> {
    let mut items = Vec::new();
    let mut item_records = Vec::new();

    let reply = dbus_call(
        "session",
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!([
            "org.kde.StatusNotifierWatcher",
            "RegisteredStatusNotifierItems"
        ]),
    )
    .or_else(|| {
        dbus_call(
            "session",
            "org.freedesktop.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.freedesktop.DBus.Properties",
            "Get",
            &json!([
                "org.freedesktop.StatusNotifierWatcher",
                "RegisteredStatusNotifierItems"
            ]),
        )
    });

    let val = reply?;
    let raw_items = val.as_array()?;

    for item_val in raw_items {
        let Some(trimmed) = item_val.as_str() else {
            continue;
        };
        if trimmed.is_empty() {
            continue;
        }

        let (service, path) = match trimmed.split_once('/') {
            Some((s, p)) => (s.to_string(), format!("/{}", p)),
            None => (trimmed.to_string(), "/StatusNotifierItem".to_string()),
        };
        if service.is_empty() {
            continue;
        }

        let mut item_id = service.clone();
        let mut icon_name = String::new();
        let mut icon_theme_path = String::new();
        let mut title = String::new();
        let mut category = String::new();
        let mut tooltip_title = String::new();
        let mut menu_path = String::new();
        let mut item_is_menu = false;

        // 1. Try single roundtrip GetAll
        let mut got_props = false;
        if let Some(props_val) = dbus_call(
            "session",
            &service,
            &path,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &json!(["org.kde.StatusNotifierItem"]),
        ) {
            if let Some(props) = props_val.as_object() {
                if let Some(id) = props.get("Id").and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        item_id = id.to_string();
                        got_props = true;
                    }
                }
                if let Some(t) = props.get("Title").and_then(|v| v.as_str()) {
                    title = t.to_string();
                }
                if let Some(c) = props.get("Category").and_then(|v| v.as_str()) {
                    category = c.to_string();
                }
                if let Some(icon) = props.get("IconName").and_then(|v| v.as_str()) {
                    icon_name = icon.to_string();
                }
                if let Some(theme) = props.get("IconThemePath").and_then(|v| v.as_str()) {
                    icon_theme_path = theme.to_string();
                }
                if let Some(m) = props.get("Menu").and_then(|v| v.as_str()) {
                    menu_path = m.to_string();
                }
                if let Some(iim) = props.get("ItemIsMenu").and_then(|v| v.as_bool()) {
                    item_is_menu = iim;
                }
                if let Some(tt) = props.get("ToolTip").and_then(|v| v.as_array()) {
                    if let Some(tt_str) = tt.get(2).and_then(|v| v.as_str()) {
                        tooltip_title = tt_str.to_string();
                    }
                }
            }
        }

        // 2. Fallback to individual property queries if GetAll failed
        if !got_props {
            if let Some(id_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "Id"]),
            ) {
                if let Some(id) = id_val.as_str() {
                    if !id.is_empty() {
                        item_id = id.to_string();
                        got_props = true;
                    }
                }
            }
            if let Some(title_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "Title"]),
            ) {
                if let Some(t) = title_val.as_str() {
                    if !t.is_empty() {
                        title = t.to_string();
                        if item_id == service {
                            item_id = title.clone();
                        }
                    }
                }
            }
            if let Some(icon_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "IconName"]),
            ) {
                if let Some(icon) = icon_val.as_str() {
                    icon_name = icon.to_string();
                }
            }
            if let Some(cat_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "Category"]),
            ) {
                if let Some(c) = cat_val.as_str() {
                    category = c.to_string();
                }
            }
            if let Some(m_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "Menu"]),
            ) {
                if let Some(m) = m_val.as_str() {
                    menu_path = m.to_string();
                }
            }
            if let Some(iim_val) = dbus_call(
                "session",
                &service,
                &path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.kde.StatusNotifierItem", "ItemIsMenu"]),
            ) {
                if let Some(b) = iim_val.as_bool() {
                    item_is_menu = b;
                }
            }
        }

        // Skip dead, unresponsive, or invalid D-Bus peers
        if !got_props && title.is_empty() && icon_name.is_empty() {
            continue;
        }

        let resolved_image = resolve_icon_file(
            &item_id,
            &icon_name,
            &icon_theme_path,
            &title,
            &tooltip_title,
        )
        .or_else(|| get_category_icon(&category));
        let btn_id = format!(
            "tray_{}_{}",
            service.replace(['.', ':'], "_"),
            path.replace(['.', '/'], "_")
        );

        let mut item_obj = json!({
            "id": item_id,
            "btn_id": btn_id,
            "title": if !title.is_empty() { &title } else { &item_id },
            "service": service,
            "path": path,
        });

        if let Some(ref img) = resolved_image {
            item_obj["image"] = json!(img);
            item_obj["icon"] = json!(img);
        } else {
            item_obj["icon"] = json!("•");
        }

        item_records.push(TrayItemData {
            id: item_id,
            service,
            path,
            menu: menu_path,
            title: if !title.is_empty() {
                title
            } else {
                String::new()
            },
            image: resolved_image,
            item_is_menu,
        });

        items.push(item_obj);
    }

    Some((items, item_records))
}

pub fn get_category_icon_name(category: &str) -> &'static str {
    match category {
        "Communications" => "applications-chat",
        "SystemServices" => "applications-system",
        "Hardware" => "preferences-desktop-peripherals",
        "ApplicationStatus" => "applications-other",
        _ => "application-x-executable",
    }
}

fn get_category_icon(category: &str) -> Option<String> {
    resolve_icon(get_category_icon_name(category))
}

fn get_icon_candidates(
    icon_name: &str,
    item_id: &str,
    title: &str,
    tooltip_title: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add = |name: &str| {
        let trimmed = name.trim();
        if !trimmed.is_empty() && !trimmed.starts_with(':') && seen.insert(trimmed.to_string()) {
            candidates.push(trimmed.to_string());
        }
    };

    // 1. Explicit IconName from SNI
    if !icon_name.is_empty() {
        add(icon_name);
        if let Some(base) = icon_name.strip_suffix("-symbolic") {
            add(base);
        } else {
            add(&format!("{}-symbolic", icon_name));
        }
    }

    // Generic status icon ID normalization (e.g. "discord_status_icon_1" -> "discord")
    if !item_id.is_empty() {
        add(item_id);
        add(&item_id.to_lowercase());

        if let Some((prefix, _)) = item_id.split_once("_status_icon") {
            add(prefix);
            add(&prefix.to_lowercase());
        }

        if item_id.contains('.') {
            if let Some(last) = item_id.split('.').next_back() {
                add(last);
                add(&last.to_lowercase());
            }
        }
    }

    // 3. Tooltip / Title
    if !tooltip_title.is_empty() {
        add(tooltip_title);
        add(&tooltip_title.to_lowercase());
    }
    if !title.is_empty() {
        add(title);
        add(&title.to_lowercase());
    }

    candidates
}

fn resolve_icon_file(
    item_id: &str,
    icon_name: &str,
    theme_path: &str,
    title: &str,
    tooltip_title: &str,
) -> Option<String> {
    if icon_name.starts_with('/')
        && (icon_name.ends_with(".png") || icon_name.ends_with(".svg"))
        && file_exists(icon_name)
    {
        return Some(icon_name.to_string());
    }

    let candidates = get_icon_candidates(icon_name, item_id, title, tooltip_title);
    if candidates.is_empty() {
        return None;
    }

    if !theme_path.is_empty() {
        for cand in &candidates {
            for ext in &["svg", "png"] {
                let candidate_path = format!("{}/{}.{}", theme_path, cand, ext);
                if file_exists(&candidate_path) {
                    return Some(candidate_path);
                }
            }
        }
    }

    for cand in &candidates {
        if let Some(path) = resolve_icon(cand) {
            return Some(path);
        }
    }

    None
}

export_wasm_module!(TrayModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_electron_status_icon_candidate_extraction() {
        let discord_cands = get_icon_candidates("", "discord_status_icon_1", "", "Discord");
        assert!(discord_cands.contains(&"discord".to_string()));
        assert!(discord_cands.contains(&"Discord".to_string()));

        let slack_cands = get_icon_candidates("", "Slack_status_icon_1", "", "Slack");
        assert!(slack_cands.contains(&"slack".to_string()));
        assert!(slack_cands.contains(&"Slack".to_string()));
    }

    #[test]
    fn test_symbolic_icon_candidate_extraction() {
        let telegram_cands = get_icon_candidates(
            "org.telegram.desktop-attention-symbolic",
            "TelegramDesktop",
            "",
            "",
        );
        assert!(telegram_cands.contains(&"org.telegram.desktop-attention-symbolic".to_string()));
        assert!(telegram_cands.contains(&"org.telegram.desktop-attention".to_string()));
    }

    #[test]
    fn test_generic_category_icons() {
        assert_eq!(
            get_category_icon_name("Communications"),
            "applications-chat"
        );
        assert_eq!(
            get_category_icon_name("SystemServices"),
            "applications-system"
        );
        assert_eq!(
            get_category_icon_name("Hardware"),
            "preferences-desktop-peripherals"
        );
        assert_eq!(
            get_category_icon_name("ApplicationStatus"),
            "applications-other"
        );
    }

    #[test]
    fn test_parse_dbus_menu_items() {
        // 1. Direct root node tuple
        let layout_direct = json!([
            0,
            { "children-display": "submenu" },
            [
                [ 10, { "label": "_Open Discord" }, [] ],
                [ 11, { "type": "separator" }, [] ],
                [ 12, { "label": "Mute", "visible": false }, [] ],
                [ 13, { "label": "_Quit", "enabled": true }, [] ]
            ]
        ]);

        let items = parse_dbus_menu_items(&layout_direct);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].label, "Open Discord");
        assert!(!items[0].is_separator);
        assert!(items[1].is_separator);
        assert_eq!(items[2].label, "Quit");

        // 2. Real D-Bus reply: [revision, (id, props, children)]
        let layout_dbus = json!([
            2,
            [
                0,
                { "children-display": "submenu" },
                [
                    [ 10, { "label": "_Open Discord" }, [] ],
                    [ 11, { "type": "separator" }, [] ],
                    [ 12, { "label": "Mute", "visible": false }, [] ],
                    [ 13, { "label": "_Quit", "enabled": true }, [] ]
                ]
            ]
        ]);

        let items_dbus = parse_dbus_menu_items(&layout_dbus);
        assert_eq!(items_dbus.len(), 3);
        assert_eq!(items_dbus[0].label, "Open Discord");
        assert!(!items_dbus[0].is_separator);
        assert!(items_dbus[1].is_separator);
        assert_eq!(items_dbus[2].label, "Quit");
    }

    #[test]
    fn test_parse_dbus_menu_items_with_submenus_and_toggles() {
        let layout = json!([
            1,
            [
                0,
                { "children-display": "submenu" },
                [
                    [
                        50,
                        { "children-display": "submenu", "label": "_Available Networks" },
                        [
                            [ 51, { "label": "Home__WiFi" }, [] ],
                            [ 52, { "label": "Guest_WiFi" }, [] ]
                        ]
                    ],
                    [ 53, { "type": "separator" }, [] ],
                    [ 54, { "label": "Enable _Networking", "toggle-type": "checkmark", "toggle-state": 1 }, [] ],
                    [ 55, { "label": "Enable _Bluetooth", "toggle-type": "checkmark", "toggle-state": 0 }, [] ]
                ]
            ]
        ]);

        let items = parse_dbus_menu_items(&layout);
        assert_eq!(items.len(), 6);
        assert_eq!(items[0].label, "Available Networks");
        assert!(items[0].is_submenu);
        assert_eq!(items[0].indent_level, 0);

        assert_eq!(items[1].label, "Home_WiFi");
        assert_eq!(items[1].indent_level, 1);

        assert_eq!(items[2].label, "GuestWiFi");
        assert_eq!(items[2].indent_level, 1);

        assert!(items[3].is_separator);

        assert_eq!(items[4].label, "Enable Networking");
        assert_eq!(items[4].toggle_type.as_deref(), Some("checkmark"));
        assert_eq!(items[4].toggle_state, Some(1));

        assert_eq!(items[5].label, "Enable Bluetooth");
        assert_eq!(items[5].toggle_type.as_deref(), Some("checkmark"));
        assert_eq!(items[5].toggle_state, Some(0));
    }
}
