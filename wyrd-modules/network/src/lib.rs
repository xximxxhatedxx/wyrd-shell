//! WASM Network Module.
//!
//! Native D-Bus NetworkManager client for status queries & event-driven signals,
//! with process spawning reserved for user-triggered Wi-Fi connections (passwords/secrets).

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, log_info, publish, request_surface,
    resolve_icon, send_update, WasmModule,
};

const SPINNER_ARROWS: &[&str] = &["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];

#[derive(Default)]
pub struct NetworkModule {
    ssid: String,
    signal: u8,
    connected: bool,
    is_scanning: bool,
    scan_ticks: u8,
    scan_frame: usize,
    popup_open: bool,
    auth_target_ssid: Option<String>,
    auth_password: String,
    auth_error: Option<String>,
    auth_popup_open: bool,
}

impl WasmModule for NetworkModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Network WASM module initialized with native D-Bus integration");

        // 1. Subscribe to NetworkManager PropertiesChanged signals on root manager
        let _ = dbus_subscribe(
            "system",
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        );

        // 2. Subscribe to Wireless device signals (AP changes, scans, state changes)
        let _ = dbus_subscribe(
            "system",
            "org.freedesktop.NetworkManager",
            "",
            "org.freedesktop.NetworkManager.Device.Wireless",
            "",
        );

        // 3. Initial network update via D-Bus
        self.update_network();
        Ok(())
    }

    fn on_dbus_signal(&mut self, interface: &str, member: &str, _payload: &Value) {
        log_info(&format!(
            "Network D-Bus signal received: {}::{}",
            interface, member
        ));
        if self.is_scanning
            && self.scan_ticks >= 4
            && (interface.contains("Wireless")
                || member == "PropertiesChanged"
                || member == "AccessPointAdded")
        {
            self.is_scanning = false;
            self.scan_ticks = 0;
            self.scan_frame = 0;
        }
        self.update_network();
        if self.popup_open {
            self.send_network_popup();
        }
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if event == "close" {
            if widget_id == "wifi-auth" || widget_id == "popup:wifi-auth" {
                self.auth_popup_open = false;
                self.auth_password.clear();
                self.auth_error = None;
            } else if widget_id == "network"
                || widget_id == "popup:network"
                || widget_id == "wifi"
                || widget_id == "popup:wifi"
            {
                self.popup_open = false;
            } else {
                self.popup_open = false;
                self.auth_popup_open = false;
                self.auth_password.clear();
                self.auth_error = None;
            }
            return;
        }

        if widget_id == "wifi_toggle" {
            let wifi_on = is_wireless_enabled();
            if wifi_on && self.auth_popup_open {
                self.auth_popup_open = false;
                self.auth_password.clear();
                self.auth_error = None;
                request_surface("close", &json!({ "name": "wifi-auth", "id": "wifi-auth" }));
            }
            let _ = dbus_call(
                "system",
                "org.freedesktop.NetworkManager",
                "/org/freedesktop/NetworkManager",
                "org.freedesktop.DBus.Properties",
                "Set",
                &json!([
                    "org.freedesktop.NetworkManager",
                    "WirelessEnabled",
                    !wifi_on
                ]),
            );
            self.update_network();
            self.send_network_popup();
        } else if widget_id == "wifi_rescan" || widget_id == "popup:wifi-rescan" {
            if !self.is_scanning {
                self.is_scanning = true;
                self.scan_ticks = 0;
                self.scan_frame = 0;
                request_wifi_scan();
                self.send_network_popup();
            }
        } else if let Some(ssid) = widget_id.strip_prefix("connect_") {
            if event == "click" {
                self.handle_connect_request(ssid);
            }
        } else if widget_id == "wifi_auth_input" {
            if event == "submit" || event == "enter" {
                self.submit_auth();
            } else {
                self.auth_password = event.to_string();
                if self.auth_error.is_some() {
                    self.auth_error = None;
                    self.send_auth_popup();
                }
            }
        } else if widget_id == "wifi_auth_connect" {
            if event == "click" {
                self.submit_auth();
            }
        } else if widget_id == "wifi_auth_cancel" {
            if event == "click" {
                self.auth_popup_open = false;
                self.auth_password.clear();
                self.auth_error = None;
                request_surface("close", &json!({ "name": "wifi-auth", "id": "wifi-auth" }));
            }
        } else if (widget_id == "network"
            || widget_id == "wifi"
            || widget_id == "net"
            || widget_id == "popup:network"
            || widget_id == "popup:wifi"
            || widget_id.is_empty())
            && (event == "open" || event == "click")
        {
            self.popup_open = true;
            self.send_network_popup();
        }
    }

    fn tick(&mut self) {
        if !self.is_scanning {
            return;
        }

        self.scan_ticks += 1;
        self.scan_frame = (self.scan_frame + 1) % SPINNER_ARROWS.len();

        // Wi-Fi hardware scan finishes in ~2.1 seconds (14 ticks * 150ms)
        if self.scan_ticks >= 14 {
            self.is_scanning = false;
            self.scan_ticks = 0;
            self.scan_frame = 0;
            self.update_network();
            if self.popup_open {
                self.send_network_popup();
            }
        } else if self.popup_open {
            self.send_network_popup();
        }
    }
}

impl NetworkModule {
    fn update_network(&mut self) {
        let (conn, ssid, sig) = query_wifi_dbus();
        self.connected = conn;
        self.ssid = ssid;
        self.signal = sig;

        let icon_name = if !conn {
            "network-wireless-offline"
        } else if sig < 30 {
            "network-wireless-signal-weak"
        } else if sig < 65 {
            "network-wireless-signal-ok"
        } else {
            "network-wireless-signal-excellent"
        };
        let icon_path = resolve_icon(icon_name)
            .or_else(|| resolve_icon("network-wireless-connected"))
            .or_else(|| resolve_icon("network-wireless"))
            .or_else(|| resolve_icon("network-workgroup"));

        let icon_glyph = if !conn {
            "󰤮"
        } else if sig < 30 {
            "󰤟"
        } else if sig < 65 {
            "󰤥"
        } else {
            "󰤨"
        };
        let text = if conn {
            if self.ssid.is_empty() {
                format!("{} Connected", icon_glyph)
            } else {
                format!("{} {}", icon_glyph, self.ssid)
            }
        } else {
            "󰤮 Offline".to_string()
        };

        send_update(
            "network",
            &json!({
                "text": text,
                "icon_glyph": icon_glyph,
                "ssid": self.ssid,
                "connected_ssid": self.ssid,
                "signal": self.signal,
                "signal_strength": self.signal,
                "connected": self.connected,
                "is_connected": self.connected,
                "icon": icon_name,
                "icon_path": icon_path,
                "path": icon_path,
            }),
        );

        let _ = publish(
            "network.status",
            &json!({ "connected": self.connected, "ssid": self.ssid }),
        );
    }

    fn send_network_popup(&mut self) {
        let (conn, active_ssid, sig) = query_wifi_dbus();
        let wifi_on = is_wireless_enabled();
        let ip_addr = query_active_ip();

        let mut children = Vec::new();

        // 1. Header Card: Title + Scan button + Wi-Fi Toggle
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [10.0, 14.0, 10.0, 14.0] },
            "children": [
                {
                    "type": "text",
                    "text": "WI-FI & NETWORKS",
                    "style": "clean_accent",
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "gap": 6.0 },
                    "children": [
                        {
                            "type": "button",
                            "id": "wifi_rescan",
                            "text": if self.is_scanning { SPINNER_ARROWS[self.scan_frame] } else { "Scan" },
                            "style": if self.is_scanning { "chip_accent" } else { "chip" },
                            "on_click": "event:network:wifi_rescan:click",
                            "layout": { "width": 62.0, "height": 28.0, "justify": "center", "align": "center", "padding": [2.0, 6.0, 2.0, 6.0] }
                        },
                        {
                            "type": "button",
                            "id": "wifi_toggle",
                            "text": if wifi_on { "ON" } else { "OFF" },
                            "style": if wifi_on { "chip_accent" } else { "chip" },
                            "on_click": "event:network:wifi_toggle:click",
                            "layout": { "height": 28.0, "justify": "center", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
                        }
                    ]
                }
            ]
        }));

        // 2. Status Card
        if conn && !active_ssid.is_empty() {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 12.0, 8.0, 12.0] },
                "children": [
                    {
                        "type": "text",
                        "text": format!("Connected to {}", active_ssid),
                        "style": "clean_accent",
                    },
                    {
                        "type": "text",
                        "text": if let Some(ip) = &ip_addr {
                            format!("Signal: {}%   •   IP: {}", sig, ip)
                        } else {
                            format!("Signal: {}%   •   Connected", sig)
                        },
                        "style": "muted",
                    }
                ]
            }));
        } else {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 12.0, 8.0, 12.0] },
                "children": [
                    {
                        "type": "text",
                        "text": if wifi_on { "Not connected to any network" } else { "Wi-Fi is turned off" },
                        "style": "muted",
                    }
                ]
            }));
        }

        // 3. Available Wi-Fi Networks Card (via D-Bus)
        let mut scan_children = Vec::new();
        let available_networks_data = query_available_networks_dbus();

        scan_children.push(json!({
            "type": "text",
            "text": "AVAILABLE ACCESS POINTS",
            "style": "clean_accent",
            "layout": { "padding": [2.0, 4.0, 4.0, 4.0] }
        }));

        for net in available_networks_data.iter().take(6) {
            let ssid = net.get("ssid").and_then(|v| v.as_str()).unwrap_or("");
            let sig_num = net.get("signal").and_then(|v| v.as_u64()).unwrap_or(50) as u8;
            let secured = net
                .get("secured")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_active = net
                .get("connected")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let lock_badge = if secured { " [locked]" } else { "" };
            let style_name = if is_active { "chip_accent" } else { "chip" };

            let display_ssid = if ssid.chars().count() > 22 {
                let short: String = ssid.chars().take(20).collect();
                format!("{}..", short)
            } else {
                ssid.to_string()
            };

            let status_badge = if is_active { "✓ " } else { "  " };

            scan_children.push(json!({
                "type": "button",
                "id": format!("connect_{}", ssid),
                "text": format!("{}{}{} ({}%)", status_badge, display_ssid, lock_badge, sig_num),
                "style": style_name,
                "on_click": format!("event:network:connect_{}:click", ssid),
                "layout": { "height": 32.0, "justify": "start", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
            }));
        }

        if scan_children.len() <= 1 {
            scan_children.push(json!({
                "type": "text",
                "text": "No available Wi-Fi networks found",
                "style": "muted",
                "layout": { "padding": [8.0, 4.0, 8.0, 4.0] }
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 5.0, "padding": [8.0, 8.0, 8.0, 8.0] },
            "children": scan_children
        }));

        // 4. Action button at bottom
        let payload = json!({
            "children": children,
            "connected_ssid": active_ssid,
            "ssid": active_ssid,
            "signal_strength": sig,
            "signal": sig,
            "connected": conn,
            "is_connected": conn,
            "ip": ip_addr,
            "ip_address": ip_addr,
            "wifi_enabled": wifi_on,
            "available_networks": available_networks_data,
        });

        send_update("popup:network", &payload);
        send_update("popup:wifi", &payload);
        send_update("popup:net", &payload);
    }

    fn handle_connect_request(&mut self, ssid: &str) {
        log_info(&format!("Connect requested for SSID: {}", ssid));

        // 1. Check if a connection with valid credentials already exists in NetworkManager Settings
        if let Some(saved) = find_saved_connection(ssid) {
            if saved.has_credentials {
                log_info(&format!(
                    "Saved connection with valid credentials found for {}, activating directly",
                    ssid
                ));
                let _ = activate_saved_connection(&saved.path, ssid);
                self.update_network();
                self.send_network_popup();
                return;
            }
        }

        // 2. Check if the access point is open/unsecured
        let ap_details = find_access_point_details(ssid);
        if !ap_details.secured {
            log_info(&format!(
                "Network {} is unsecured, connecting directly",
                ssid
            ));
            let _ = connect_open_network(ssid);
            self.update_network();
            self.send_network_popup();
            return;
        }

        // 3. Network is secured and has no valid credentials saved -> show centered password modal
        log_info(&format!(
            "Network {} requires password, opening centered wifi-auth modal",
            ssid
        ));
        self.auth_target_ssid = Some(ssid.to_string());
        self.auth_password.clear();
        self.auth_error = None;
        self.auth_popup_open = true;

        // Send popup update before opening so initial tree has widgets
        self.send_auth_popup();
        request_surface("open", &json!({ "name": "wifi-auth", "id": "wifi-auth" }));
    }

    fn submit_auth(&mut self) {
        let Some(ssid) = self.auth_target_ssid.clone() else {
            return;
        };

        if self.auth_password.len() < 8 {
            self.auth_error = Some("Password must be at least 8 characters".to_string());
            self.send_auth_popup();
            return;
        }

        let ap_details = find_access_point_details(&ssid);
        let success =
            connect_to_network_with_password(&ssid, &self.auth_password, ap_details.is_sae);
        if success {
            self.auth_popup_open = false;
            self.auth_password.clear();
            self.auth_error = None;
            request_surface("close", &json!({ "name": "wifi-auth", "id": "wifi-auth" }));
            self.update_network();
            self.send_network_popup();
        } else {
            self.auth_error = Some("Connection failed. Please check password.".to_string());
            self.send_auth_popup();
        }
    }

    fn send_auth_popup(&self) {
        let target_ssid = self.auth_target_ssid.as_deref().unwrap_or("Wi-Fi Network");
        let mut children = Vec::new();

        // 1. Header Card: Wi-Fi Icon + Title + Subtitle
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 4.0, "padding": [12.0, 14.0, 12.0, 14.0] },
            "children": [
                {
                    "type": "text",
                    "text": "WI-FI AUTHENTICATION",
                    "style": "clean_accent",
                },
                {
                    "type": "text",
                    "text": format!("Enter password for \"{}\"", target_ssid),
                    "style": "muted",
                }
            ]
        }));

        // 2. Password Input Card
        children.push(json!({
            "type": "container",
            "style": "launcher_search",
            "layout": { "mode": "flex_row", "gap": 8.0, "align": "center", "padding": [8.0, 12.0, 8.0, 12.0] },
            "children": [
                {
                    "type": "text",
                    "text": "Key:",
                    "style": "launcher_input_icon",
                    "layout": { "fixed_width": 32.0 }
                },
                {
                    "type": "text_input",
                    "id": "wifi_auth_input",
                    "text": self.auth_password,
                    "placeholder": "Enter Wi-Fi password...",
                    "focused": true,
                    "style": "launcher_input",
                    "layout": { "height": 30.0, "weight": 1.0 }
                }
            ]
        }));

        // 3. Error Feedback
        if let Some(ref err) = self.auth_error {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_row", "align": "center", "gap": 6.0, "padding": [6.0, 10.0, 6.0, 10.0] },
                "children": [
                    {
                        "type": "text",
                        "text": format!("Error: {}", err),
                        "style": "clean_accent",
                    }
                ]
            }));
        }

        // 4. Action Buttons: Cancel and Connect
        children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "justify": "end", "align": "center", "gap": 8.0, "padding": [4.0, 0.0, 4.0, 0.0] },
            "children": [
                {
                    "type": "button",
                    "id": "wifi_auth_cancel",
                    "text": "Cancel",
                    "style": "chip",
                    "on_click": "event:network:wifi_auth_cancel:click",
                    "layout": { "height": 32.0, "justify": "center", "align": "center", "padding": [2.0, 14.0, 2.0, 14.0] }
                },
                {
                    "type": "button",
                    "id": "wifi_auth_connect",
                    "text": "Connect",
                    "style": "chip_accent",
                    "on_click": "event:network:wifi_auth_connect:click",
                    "layout": { "height": 32.0, "justify": "center", "align": "center", "padding": [2.0, 16.0, 2.0, 16.0] }
                }
            ]
        }));

        let payload = json!({
            "children": children,
            "target_ssid": target_ssid,
        });

        send_update("popup:wifi-auth", &payload);
    }
}

// ----------------------------------------------------------------------------
// D-Bus Query Helpers
// ----------------------------------------------------------------------------

fn is_wireless_enabled() -> bool {
    if let Some(val) = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!(["org.freedesktop.NetworkManager", "WirelessEnabled"]),
    ) {
        if let Some(b) = val.as_bool() {
            return b;
        }
    }
    true
}

fn request_wifi_scan() {
    if let Some(devices) = get_devices_dbus() {
        for dev_path in devices {
            let dev_type = get_device_type(&dev_path);
            if dev_type == 2 {
                let _ = dbus_call(
                    "system",
                    "org.freedesktop.NetworkManager",
                    &dev_path,
                    "org.freedesktop.NetworkManager.Device.Wireless",
                    "RequestScan",
                    &json!([{}]),
                );
            }
        }
    }
}

fn get_devices_dbus() -> Option<Vec<String>> {
    let res = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "GetDevices",
        &json!([]),
    )?;

    if let Value::Array(arr) = res {
        Some(
            arr.into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
        )
    } else {
        None
    }
}

fn get_device_type(dev_path: &str) -> u64 {
    let val = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        dev_path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!(["org.freedesktop.NetworkManager.Device", "DeviceType"]),
    );
    val.and_then(|v| v.as_u64()).unwrap_or(0)
}

fn get_device_state(dev_path: &str) -> u64 {
    let val = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        dev_path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!(["org.freedesktop.NetworkManager.Device", "State"]),
    );
    val.and_then(|v| v.as_u64()).unwrap_or(0)
}

fn get_device_interface(dev_path: &str) -> String {
    let val = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        dev_path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!(["org.freedesktop.NetworkManager.Device", "Interface"]),
    );
    val.and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn query_wifi_dbus() -> (bool, String, u8) {
    if let Some(devices) = get_devices_dbus() {
        for dev_path in devices {
            let dev_type = get_device_type(&dev_path);
            if dev_type == 2 {
                // DeviceType 2 = Wireless
                let state = get_device_state(&dev_path);
                let is_activated = state == 100; // NM_DEVICE_STATE_ACTIVATED

                if is_activated {
                    if let Some(ap_val) = dbus_call(
                        "system",
                        "org.freedesktop.NetworkManager",
                        &dev_path,
                        "org.freedesktop.DBus.Properties",
                        "Get",
                        &json!([
                            "org.freedesktop.NetworkManager.Device.Wireless",
                            "ActiveAccessPoint"
                        ]),
                    ) {
                        if let Some(ap_path) = ap_val.as_str() {
                            if ap_path != "/" && !ap_path.is_empty() {
                                let (ssid, strength, _) = query_ap_dbus(ap_path);
                                return (true, ssid, strength);
                            }
                        }
                    }
                }
            } else if dev_type == 1 {
                // DeviceType 1 = Ethernet
                let state = get_device_state(&dev_path);
                if state == 100 {
                    return (true, "Wired Ethernet".to_string(), 100);
                }
            }
        }
    }

    (false, String::new(), 0)
}

#[derive(Default, Clone)]
struct ApDetails {
    ssid: String,
    strength: u8,
    secured: bool,
    is_sae: bool,
}

fn query_ap_details(ap_path: &str) -> ApDetails {
    if let Some(props) = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        ap_path,
        "org.freedesktop.DBus.Properties",
        "GetAll",
        &json!(["org.freedesktop.NetworkManager.AccessPoint"]),
    ) {
        let ssid = props.get("Ssid").map(parse_ssid_bytes).unwrap_or_default();
        let strength = props.get("Strength").and_then(|v| v.as_u64()).unwrap_or(50) as u8;
        let rsn = props.get("RsnFlags").and_then(|v| v.as_u64()).unwrap_or(0);
        let wpa = props.get("WpaFlags").and_then(|v| v.as_u64()).unwrap_or(0);
        let flags = props.get("Flags").and_then(|v| v.as_u64()).unwrap_or(0);
        let secured = rsn > 0 || wpa > 0 || (flags & 1 != 0);
        let is_sae = (rsn & 1024 != 0) && (rsn & 512 == 0);
        return ApDetails {
            ssid,
            strength,
            secured,
            is_sae,
        };
    }
    ApDetails::default()
}

fn query_ap_dbus(ap_path: &str) -> (String, u8, bool) {
    let details = query_ap_details(ap_path);
    (details.ssid, details.strength, details.secured)
}

fn query_available_networks_dbus() -> Vec<Value> {
    let mut networks = Vec::new();
    let mut seen_ssids = HashSet::new();
    let (conn, active_ssid, _) = query_wifi_dbus();

    if let Some(devices) = get_devices_dbus() {
        for dev_path in devices {
            if get_device_type(&dev_path) == 2 {
                if let Some(Value::Array(aps)) = dbus_call(
                    "system",
                    "org.freedesktop.NetworkManager",
                    &dev_path,
                    "org.freedesktop.DBus.Properties",
                    "Get",
                    &json!([
                        "org.freedesktop.NetworkManager.Device.Wireless",
                        "AccessPoints"
                    ]),
                ) {
                    for ap_item in aps {
                        if let Some(ap_path) = ap_item.as_str() {
                            let (ssid, strength, secured) = query_ap_dbus(ap_path);
                            if !ssid.is_empty() && seen_ssids.insert(ssid.clone()) {
                                let is_active = conn && ssid == active_ssid;
                                networks.push(json!({
                                    "ssid": ssid,
                                    "signal": strength,
                                    "secured": secured,
                                    "connected": is_active,
                                    "bssid": ssid,
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    networks.sort_by(|a, b| {
        let sig_b = b.get("signal").and_then(|v| v.as_u64()).unwrap_or(0);
        let sig_a = a.get("signal").and_then(|v| v.as_u64()).unwrap_or(0);
        sig_b.cmp(&sig_a)
    });

    networks
}

#[allow(dead_code)]
struct SavedConnectionInfo {
    path: String,
    has_security: bool,
    has_credentials: bool,
}

fn find_saved_connection(target_ssid: &str) -> Option<SavedConnectionInfo> {
    let connections_val = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
        "ListConnections",
        &json!([]),
    )?;

    let conn_paths = connections_val.as_array()?;
    for p in conn_paths {
        let Some(path_str) = p.as_str() else {
            continue;
        };
        if let Some(settings) = dbus_call(
            "system",
            "org.freedesktop.NetworkManager",
            path_str,
            "org.freedesktop.NetworkManager.Settings.Connection",
            "GetSettings",
            &json!([]),
        ) {
            let conn_id = settings
                .get("connection")
                .and_then(|c| c.get("id"))
                .and_then(|id| id.as_str())
                .unwrap_or("");

            let conn_type = settings
                .get("connection")
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            if conn_type != "802-11-wireless" {
                continue;
            }

            let ssid_val = settings.get("802-11-wireless").and_then(|w| w.get("ssid"));
            let parsed_ssid = ssid_val.map(parse_ssid_bytes).unwrap_or_default();

            if conn_id == target_ssid || parsed_ssid == target_ssid {
                let has_security = settings.get("802-11-wireless-security").is_some();
                let mut has_credentials = false;

                if has_security {
                    if let Some(secrets) = dbus_call(
                        "system",
                        "org.freedesktop.NetworkManager",
                        path_str,
                        "org.freedesktop.NetworkManager.Settings.Connection",
                        "GetSecrets",
                        &json!(["802-11-wireless-security"]),
                    ) {
                        if let Some(sec) = secrets.get("802-11-wireless-security") {
                            let psk = sec.get("psk").and_then(|v| v.as_str()).unwrap_or("");
                            let wep = sec.get("wep-key0").and_then(|v| v.as_str()).unwrap_or("");
                            if !psk.is_empty() || !wep.is_empty() {
                                has_credentials = true;
                            }
                        }
                    }
                    if let Some(sec) = settings.get("802-11-wireless-security") {
                        let psk = sec.get("psk").and_then(|v| v.as_str()).unwrap_or("");
                        if !psk.is_empty() {
                            has_credentials = true;
                        }
                    }
                } else {
                    has_credentials = true;
                }

                return Some(SavedConnectionInfo {
                    path: path_str.to_string(),
                    has_security,
                    has_credentials,
                });
            }
        }
    }

    None
}

fn activate_saved_connection(conn_path: &str, ssid: &str) -> bool {
    let Some(device) = get_devices_dbus()
        .and_then(|devices| devices.into_iter().find(|path| get_device_type(path) == 2))
    else {
        return false;
    };
    let ap = find_access_point(&device, ssid).unwrap_or_else(|| "/".to_string());

    dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "ActivateConnection",
        &json!([conn_path, device, ap]),
    )
    .is_some()
}

fn find_access_point_details(wanted_ssid: &str) -> ApDetails {
    if let Some(device) = get_devices_dbus()
        .and_then(|devices| devices.into_iter().find(|path| get_device_type(path) == 2))
    {
        if let Some(ap_path) = find_access_point(&device, wanted_ssid) {
            return query_ap_details(&ap_path);
        }
    }
    ApDetails {
        ssid: wanted_ssid.to_string(),
        strength: 50,
        secured: true,
        is_sae: false,
    }
}

fn connect_open_network(ssid: &str) -> bool {
    let Some(device) = get_devices_dbus()
        .and_then(|devices| devices.into_iter().find(|path| get_device_type(path) == 2))
    else {
        return false;
    };
    let Some(ap) = find_access_point(&device, ssid) else {
        return false;
    };
    let ssid_bytes: Vec<Value> = ssid.as_bytes().iter().map(|byte| json!(*byte)).collect();
    let settings = json!({
        "connection": { "id": ssid, "type": "802-11-wireless" },
        "802-11-wireless": { "ssid": ssid_bytes },
        "ipv4": { "method": "auto" },
        "ipv6": { "method": "auto" }
    });
    dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "AddAndActivateConnection",
        &json!([settings, device, ap]),
    )
    .is_some()
}

fn connect_to_network_with_password(ssid: &str, password: &str, is_sae: bool) -> bool {
    let Some(device) = get_devices_dbus()
        .and_then(|devices| devices.into_iter().find(|path| get_device_type(path) == 2))
    else {
        return false;
    };
    let Some(ap) = find_access_point(&device, ssid) else {
        return false;
    };
    let key_mgmt = if is_sae { "sae" } else { "wpa-psk" };
    let ssid_bytes: Vec<Value> = ssid.as_bytes().iter().map(|byte| json!(*byte)).collect();
    let settings = json!({
        "connection": { "id": ssid, "type": "802-11-wireless" },
        "802-11-wireless": { "ssid": ssid_bytes },
        "802-11-wireless-security": {
            "key-mgmt": key_mgmt,
            "psk": password
        },
        "ipv4": { "method": "auto" },
        "ipv6": { "method": "auto" }
    });
    dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "AddAndActivateConnection",
        &json!([settings, device, ap]),
    )
    .is_some()
}

fn find_access_point(device: &str, wanted_ssid: &str) -> Option<String> {
    let aps = dbus_call(
        "system",
        "org.freedesktop.NetworkManager",
        device,
        "org.freedesktop.DBus.Properties",
        "Get",
        &json!([
            "org.freedesktop.NetworkManager.Device.Wireless",
            "AccessPoints"
        ]),
    )?;
    aps.as_array()?.iter().find_map(|ap| {
        let path = ap.as_str()?;
        (query_ap_dbus(path).0 == wanted_ssid).then(|| path.to_string())
    })
}

fn query_active_ip() -> Option<String> {
    let devices = get_devices_dbus()?;

    // Prioritize Wi-Fi (2), then Ethernet (1), then other non-loopback devices
    let mut sorted_devices: Vec<(String, u64)> = devices
        .into_iter()
        .map(|path| {
            let ty = get_device_type(&path);
            (path, ty)
        })
        .filter(|(path, ty)| *ty != 32 && get_device_interface(path) != "lo")
        .collect();

    sorted_devices.sort_by_key(|(_, ty)| match *ty {
        2 => 0, // Wi-Fi first
        1 => 1, // Ethernet second
        _ => 2,
    });

    for (dev_path, _) in sorted_devices {
        if get_device_state(&dev_path) == 100 {
            if let Some(ip4_val) = dbus_call(
                "system",
                "org.freedesktop.NetworkManager",
                &dev_path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &json!(["org.freedesktop.NetworkManager.Device", "Ip4Config"]),
            ) {
                if let Some(ip4_path) = ip4_val.as_str() {
                    if ip4_path != "/" && !ip4_path.is_empty() {
                        if let Some(props) = dbus_call(
                            "system",
                            "org.freedesktop.NetworkManager",
                            ip4_path,
                            "org.freedesktop.DBus.Properties",
                            "GetAll",
                            &json!(["org.freedesktop.NetworkManager.IP4Config"]),
                        ) {
                            if let Some(Value::Array(addrs)) = props.get("AddressData") {
                                for addr_item in addrs {
                                    if let Some(addr) =
                                        addr_item.get("address").and_then(|v| v.as_str())
                                    {
                                        if !addr.is_empty() && addr != "127.0.0.1" {
                                            return Some(addr.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn parse_ssid_bytes(val: &Value) -> String {
    if let Value::Array(bytes) = val {
        let u8_vec: Vec<u8> = bytes
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .collect();
        String::from_utf8(u8_vec).unwrap_or_default()
    } else if let Value::String(s) = val {
        s.clone()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssid_bytes() {
        let bytes_val = json!([77, 121, 87, 105, 70, 105]); // "MyWiFi"
        assert_eq!(parse_ssid_bytes(&bytes_val), "MyWiFi");

        let str_val = json!("DirectString");
        assert_eq!(parse_ssid_bytes(&str_val), "DirectString");

        let null_val = Value::Null;
        assert_eq!(parse_ssid_bytes(&null_val), "");
    }

    #[test]
    fn test_network_auth_state_initialization() {
        let module = NetworkModule::default();
        assert!(module.auth_target_ssid.is_none());
        assert!(module.auth_password.is_empty());
        assert!(!module.auth_popup_open);
        assert!(module.auth_error.is_none());
    }

    #[test]
    fn test_network_auth_input_event() {
        let mut module = NetworkModule {
            auth_error: Some("Old error".to_string()),
            ..Default::default()
        };
        module.on_event("wifi_auth_input", "mypassword123");
        assert_eq!(module.auth_password, "mypassword123");
        assert!(module.auth_error.is_none());
    }

    #[test]
    fn test_network_auth_cancel_event() {
        let mut module = NetworkModule {
            auth_target_ssid: Some("TestSSID".to_string()),
            auth_password: "temp".to_string(),
            auth_popup_open: true,
            auth_error: Some("Error".to_string()),
            ..Default::default()
        };
        module.on_event("wifi_auth_cancel", "click");
        assert!(!module.auth_popup_open);
        assert!(module.auth_password.is_empty());
        assert!(module.auth_error.is_none());
    }

    #[test]
    fn test_network_auth_close_event() {
        let mut module = NetworkModule {
            auth_target_ssid: Some("TestSSID".to_string()),
            auth_password: "temp".to_string(),
            auth_popup_open: true,
            auth_error: Some("Error".to_string()),
            ..Default::default()
        };
        module.on_event("wifi-auth", "close");
        assert!(!module.auth_popup_open);
        assert!(module.auth_password.is_empty());
        assert!(module.auth_error.is_none());
    }
}

export_wasm_module!(NetworkModule);
