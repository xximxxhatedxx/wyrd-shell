//! WASM Bluetooth Module.
//!
//! Native D-Bus BlueZ client for Bluetooth status, devices, and control.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    dbus_call, dbus_subscribe, export_wasm_module, log_info, publish, resolve_icon, send_update,
    WasmModule,
};

#[derive(Debug, Clone, Default)]
struct BtDevice {
    path: String,
    address: String,
    name: String,
    connected: bool,
    paired: bool,
    icon: String,
}

fn dev_icon_name_for(icon_name: &str, connected: bool) -> &'static str {
    if !icon_name.is_empty() {
        return match icon_name {
            "audio-headset" | "audio-headphones" => "audio-headphones",
            "audio-card" => "audio-card",
            "input-keyboard" => "input-keyboard",
            "input-mouse" => "input-mouse",
            "input-gaming" => "input-gaming",
            "phone" => "phone",
            "computer" => "computer",
            _ => {
                if connected {
                    "bluetooth-active"
                } else {
                    "bluetooth"
                }
            }
        };
    }
    if connected {
        "bluetooth-active"
    } else {
        "bluetooth"
    }
}

#[derive(Default)]
pub struct BluetoothModule {
    adapter_path: String,
    powered: bool,
    discovering: bool,
    connected_device: Option<String>,
    devices: Vec<BtDevice>,
    scan_ticks: u32,
}

impl WasmModule for BluetoothModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Bluetooth WASM module initialized with native BlueZ D-Bus integration");

        // 1. Subscribe to BlueZ property changes across all adapters & devices
        let _ = dbus_subscribe(
            "system",
            "org.bluez",
            "",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        );

        // 2. Subscribe to ObjectManager additions / removals
        let _ = dbus_subscribe(
            "system",
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "InterfacesAdded",
        );
        let _ = dbus_subscribe(
            "system",
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "InterfacesRemoved",
        );

        self.update_bluetooth();
        Ok(())
    }

    fn on_dbus_signal(&mut self, _interface: &str, _member: &str, _payload: &Value) {
        self.update_bluetooth();
        self.send_bluetooth_popup();
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "bluetooth_power_toggle" {
            let adapter = if self.adapter_path.is_empty() {
                "/org/bluez/hci0".to_string()
            } else {
                self.adapter_path.clone()
            };
            let new_powered = !self.powered;
            let _ = dbus_call(
                "system",
                "org.bluez",
                &adapter,
                "org.freedesktop.DBus.Properties",
                "Set",
                &json!(["org.bluez.Adapter1", "Powered", new_powered]),
            );
            self.update_bluetooth();
            self.send_bluetooth_popup();
        } else if widget_id == "bt_scan_btn" {
            let adapter = if self.adapter_path.is_empty() {
                "/org/bluez/hci0".to_string()
            } else {
                self.adapter_path.clone()
            };
            if !self.powered {
                let _ = dbus_call(
                    "system",
                    "org.bluez",
                    &adapter,
                    "org.freedesktop.DBus.Properties",
                    "Set",
                    &json!(["org.bluez.Adapter1", "Powered", true]),
                );
                self.powered = true;
            }
            let method = if self.discovering {
                "StopDiscovery"
            } else {
                "StartDiscovery"
            };
            let _ = dbus_call(
                "system",
                "org.bluez",
                &adapter,
                "org.bluez.Adapter1",
                method,
                &json!([]),
            );
            self.scan_ticks = 0;
            self.update_bluetooth();
            self.send_bluetooth_popup();
        } else if widget_id.starts_with("bt_dev_") {
            let sanitized_mac = widget_id.trim_start_matches("bt_dev_");
            let mac = sanitized_mac.replace('_', ":");
            if let Some(dev) = self
                .devices
                .iter()
                .find(|d| d.address.eq_ignore_ascii_case(&mac) || d.path.contains(sanitized_mac))
            {
                if dev.connected {
                    let _ = dbus_call(
                        "system",
                        "org.bluez",
                        &dev.path,
                        "org.bluez.Device1",
                        "Disconnect",
                        &json!([]),
                    );
                } else {
                    if !dev.paired {
                        let _ = dbus_call(
                            "system",
                            "org.bluez",
                            &dev.path,
                            "org.bluez.Device1",
                            "Pair",
                            &json!([]),
                        );
                    }
                    let _ = dbus_call(
                        "system",
                        "org.bluez",
                        &dev.path,
                        "org.bluez.Device1",
                        "Connect",
                        &json!([]),
                    );
                }
                self.update_bluetooth();
                self.send_bluetooth_popup();
            }
        } else if (widget_id == "bluetooth"
            || widget_id == "bt"
            || widget_id == "popup:bluetooth"
            || widget_id == "bluetooth:popup"
            || widget_id.is_empty())
            && (event == "open" || event == "click")
        {
            self.update_bluetooth();
            self.send_bluetooth_popup();
        }
    }

    fn tick(&mut self) {
        if self.discovering {
            self.scan_ticks = self.scan_ticks.saturating_add(1);
            // Auto-stop discovery after 30 seconds to preserve battery
            if self.scan_ticks >= 30 {
                let adapter = if self.adapter_path.is_empty() {
                    "/org/bluez/hci0".to_string()
                } else {
                    self.adapter_path.clone()
                };
                let _ = dbus_call(
                    "system",
                    "org.bluez",
                    &adapter,
                    "org.bluez.Adapter1",
                    "StopDiscovery",
                    &json!([]),
                );
                self.discovering = false;
                self.scan_ticks = 0;
                self.update_bluetooth();
                self.send_bluetooth_popup();
            }
        } else {
            self.scan_ticks = 0;
        }
    }
}

impl BluetoothModule {
    fn update_bluetooth(&mut self) {
        let (adapter, powered, discovering, conn_dev, devs) = query_bluez();
        self.adapter_path = adapter;
        self.powered = powered;
        self.discovering = discovering;
        self.connected_device = conn_dev;
        self.devices = devs;

        let icon_name = if !self.powered {
            "bluetooth-disabled"
        } else if self.connected_device.is_some() {
            "bluetooth-active"
        } else {
            "bluetooth"
        };
        let icon_path = resolve_icon(icon_name)
            .or_else(|| resolve_icon("bluetooth-active"))
            .or_else(|| resolve_icon("bluetooth"));

        send_update(
            "bluetooth",
            &json!({
                "text": if let Some(ref d) = self.connected_device { d.clone() } else if self.powered { "On".to_string() } else { "Off".to_string() },
                "powered": self.powered,
                "connected": self.connected_device.is_some(),
                "connected_device": self.connected_device,
                "device": self.connected_device,
                "icon": icon_name,
                "icon_path": icon_path,
                "path": icon_path,
            }),
        );

        let _ = publish(
            "bluetooth.status",
            &json!({ "powered": self.powered, "connected": self.connected_device.is_some() }),
        );
    }

    fn send_bluetooth_popup(&mut self) {
        let mut children = Vec::new();
        let mut devices_data = Vec::new();

        // 1. Header Card
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [10.0, 14.0, 10.0, 14.0] },
            "children": [
                {
                    "type": "text",
                    "text": "BLUETOOTH",
                    "style": "clean_accent",
                },
                {
                    "type": "button",
                    "id": "bluetooth_power_toggle",
                    "text": if self.powered { "ON" } else { "OFF" },
                    "style": if self.powered { "chip_accent" } else { "chip" },
                    "on_click": "event:bluetooth:bluetooth_power_toggle:click",
                    "layout": { "height": 28.0, "justify": "center", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
                }
            ]
        }));

        // 2. Status Card
        if let Some(ref dev_name) = self.connected_device {
            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 12.0, 8.0, 12.0] },
                "children": [
                    {
                        "type": "text",
                        "text": format!("Connected to {}", dev_name),
                        "style": "clean_accent",
                    },
                    {
                        "type": "text",
                        "text": "Active Bluetooth Device",
                        "style": "muted",
                    }
                ]
            }));
        }

        // 3. Paired Devices list Card
        let mut paired_children = Vec::new();
        paired_children.push(json!({
            "type": "text",
            "text": "PAIRED DEVICES",
            "style": "clean_accent",
            "layout": { "padding": [2.0, 4.0, 4.0, 4.0] }
        }));

        let mut paired_count = 0;
        for dev in &self.devices {
            if !dev.paired && !dev.connected {
                continue;
            }
            paired_count += 1;

            let icon_name = dev_icon_name_for(&dev.icon, dev.connected);
            devices_data.push(json!({
                "id": dev.address,
                "mac": dev.address,
                "name": dev.name,
                "connected": dev.connected,
                "paired": dev.paired,
                "icon": icon_name,
                "dev_icon": dev.icon,
            }));

            let status_badge = if dev.connected { "✓ " } else { "  " };
            let btn_id = format!("bt_dev_{}", dev.address.replace(':', "_"));

            paired_children.push(json!({
                "type": "button",
                "id": &btn_id,
                "text": format!("{}{}", status_badge, dev.name),
                "style": if dev.connected { "chip_accent" } else { "chip" },
                "on_click": format!("event:bluetooth:{}:click", btn_id),
                "layout": { "height": 32.0, "justify": "start", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
            }));
        }

        if paired_count == 0 {
            paired_children.push(json!({
                "type": "text",
                "text": if self.powered { "No paired devices" } else { "Bluetooth is powered off" },
                "style": "muted",
                "layout": { "padding": [4.0, 4.0, 4.0, 4.0] }
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 8.0, 8.0, 8.0] },
            "children": paired_children
        }));

        // 4. Available / Discovered Devices Card
        let available_devs: Vec<&BtDevice> = self
            .devices
            .iter()
            .filter(|d| !d.paired && !d.connected)
            .collect();
        if self.powered && (self.discovering || !available_devs.is_empty()) {
            let mut avail_children = Vec::new();
            avail_children.push(json!({
                "type": "text",
                "text": if self.discovering { "AVAILABLE DEVICES [Scanning]" } else { "AVAILABLE DEVICES" },
                "style": "clean_accent",
                "layout": { "padding": [2.0, 4.0, 4.0, 4.0] }
            }));

            for dev in &available_devs {
                let icon_name = dev_icon_name_for(&dev.icon, false);
                devices_data.push(json!({
                    "id": dev.address,
                    "mac": dev.address,
                    "name": dev.name,
                    "connected": dev.connected,
                    "paired": dev.paired,
                    "icon": icon_name,
                    "dev_icon": dev.icon,
                }));

                let btn_id = format!("bt_dev_{}", dev.address.replace(':', "_"));

                avail_children.push(json!({
                    "type": "button",
                    "id": &btn_id,
                    "text": dev.name.clone(),
                    "style": "chip",
                    "on_click": format!("event:bluetooth:{}:click", btn_id),
                    "layout": { "height": 32.0, "justify": "start", "align": "center", "padding": [2.0, 10.0, 2.0, 10.0] }
                }));
            }

            if available_devs.is_empty() && self.discovering {
                avail_children.push(json!({
                    "type": "text",
                    "text": "Scanning for nearby devices...",
                    "style": "muted",
                    "layout": { "padding": [4.0, 4.0, 4.0, 4.0] }
                }));
            }

            children.push(json!({
                "type": "container",
                "style": "card",
                "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 8.0, 8.0, 8.0] },
                "children": avail_children
            }));
        }

        // 5. Bottom Action Buttons
        children.push(json!({
            "type": "container",
            "layout": { "mode": "flex_row", "gap": 6.0 },
            "children": [
                {
                    "type": "button",
                    "id": "bt_scan_btn",
                    "text": if self.discovering { "Scanning... (Stop)" } else { "Scan for Devices" },
                    "style": if self.discovering { "chip_accent" } else { "chip" },
                    "on_click": "event:bluetooth:bt_scan_btn:click",
                    "layout": { "height": 32.0, "justify": "center", "align": "center", "weight": 1.0 }
                },
            ]
        }));

        let popup_payload = json!({
            "type": "container",
            "style": "popup",
            "layout": { "mode": "flex_col", "gap": 8.0, "padding": [12.0, 12.0, 12.0, 12.0], "width": 320.0 },
            "children": children,
            "powered": self.powered,
            "connected": self.connected_device.is_some(),
            "connected_name": self.connected_device,
            "devices": devices_data,
        });

        send_update("popup:bluetooth", &popup_payload);
        send_update("bluetooth:popup", &popup_payload);
    }
}

fn query_bluez() -> (String, bool, bool, Option<String>, Vec<BtDevice>) {
    let mut adapter_path = "/org/bluez/hci0".to_string();
    let mut powered = false;
    let mut discovering = false;
    let mut connected_name: Option<String> = None;
    let mut devices = Vec::new();

    let reply = dbus_call(
        "system",
        "org.bluez",
        "/",
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
        &json!([]),
    );

    let Some(val) = reply else {
        return (adapter_path, powered, discovering, connected_name, devices);
    };

    let Some(objects) = val.as_object() else {
        return (adapter_path, powered, discovering, connected_name, devices);
    };

    for (path, ifaces_val) in objects {
        let Some(ifaces) = ifaces_val.as_object() else {
            continue;
        };

        // 1. Adapter
        if let Some(adapter_obj) = ifaces.get("org.bluez.Adapter1") {
            let is_p = adapter_obj
                .get("Powered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_d = adapter_obj
                .get("Discovering")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if adapter_path.is_empty() || adapter_path == "/org/bluez/hci0" || is_p {
                adapter_path = path.clone();
                powered = is_p;
                discovering = is_d;
            }
        }

        // 2. Device
        if let Some(dev_obj) = ifaces.get("org.bluez.Device1") {
            let addr = dev_obj
                .get("Address")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if addr.is_empty() {
                continue;
            }
            let name_prop = dev_obj.get("Name").and_then(|v| v.as_str());
            let alias_prop = dev_obj.get("Alias").and_then(|v| v.as_str());
            let alias = match (alias_prop, name_prop) {
                (Some(a), _) if !a.is_empty() => a.to_string(),
                (_, Some(n)) if !n.is_empty() => n.to_string(),
                _ => addr.clone(),
            };
            let is_conn = dev_obj
                .get("Connected")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_paired = dev_obj
                .get("Paired")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let icon = dev_obj
                .get("Icon")
                .and_then(|v| v.as_str())
                .unwrap_or("bluetooth")
                .to_string();

            if is_conn && connected_name.is_none() {
                connected_name = Some(alias.clone());
            }

            devices.push(BtDevice {
                path: path.clone(),
                address: addr,
                name: alias,
                connected: is_conn,
                paired: is_paired,
                icon,
            });
        }
    }

    // Sort devices:
    // 1. Connected first
    // 2. Paired second
    // 3. Named devices before unnamed (MAC-only) devices
    // 4. Alphabetically
    devices.sort_by(|a, b| {
        let a_has_name = !a.name.eq_ignore_ascii_case(&a.address)
            && !a.name.replace('-', ":").eq_ignore_ascii_case(&a.address);
        let b_has_name = !b.name.eq_ignore_ascii_case(&b.address)
            && !b.name.replace('-', ":").eq_ignore_ascii_case(&b.address);

        b.connected
            .cmp(&a.connected)
            .then_with(|| b.paired.cmp(&a.paired))
            .then_with(|| b_has_name.cmp(&a_has_name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    (adapter_path, powered, discovering, connected_name, devices)
}

export_wasm_module!(BluetoothModule);
