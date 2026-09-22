//! System and environment diagnostics for Wyrd Shell.

use std::fs;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

pub struct DiagnosticItem {
    pub category: &'static str,
    pub name: String,
    pub status: DiagnosticStatus,
    pub details: String,
    pub suggestion: Option<String>,
}

impl DiagnosticItem {
    pub fn ok(category: &'static str, name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            status: DiagnosticStatus::Ok,
            details: details.into(),
            suggestion: None,
        }
    }

    pub fn warn(
        category: &'static str,
        name: impl Into<String>,
        details: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            status: DiagnosticStatus::Warning,
            details: details.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn error(
        category: &'static str,
        name: impl Into<String>,
        details: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            category,
            name: name.into(),
            status: DiagnosticStatus::Error,
            details: details.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

pub fn collect_diagnostics() -> Vec<DiagnosticItem> {
    let mut items = Vec::new();

    // 1. Wayland Environment
    check_wayland(&mut items);

    // 2. Compositor Detection & IPC
    check_compositor(&mut items);

    // 3. Companion Daemons
    check_companion_daemons(&mut items);

    // 4. D-Bus Environment
    check_dbus(&mut items);

    // 5. Hardware Subsystems
    check_hardware(&mut items);

    items
}

fn check_wayland(items: &mut Vec<DiagnosticItem>) {
    let category = "Wayland Environment";
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) if !display.is_empty() => {
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
            let socket_path = Path::new(&runtime_dir).join(&display);
            if socket_path.exists() {
                if UnixStream::connect(&socket_path).is_ok() {
                    items.push(DiagnosticItem::ok(
                        category,
                        format!("WAYLAND_DISPLAY ({})", display),
                        format!("Connected successfully to {:?}", socket_path),
                    ));
                } else {
                    items.push(DiagnosticItem::error(
                        category,
                        format!("WAYLAND_DISPLAY ({})", display),
                        format!("Socket exists at {:?} but connection failed", socket_path),
                        "Check compositor permissions or restart your Wayland session",
                    ));
                }
            } else {
                items.push(DiagnosticItem::warn(
                    category,
                    format!("WAYLAND_DISPLAY ({})", display),
                    format!("Socket path {:?} does not exist", socket_path),
                    "Ensure Wayland compositor is running and XDG_RUNTIME_DIR is correct",
                ));
            }
        }
        _ => {
            items.push(DiagnosticItem::error(
                category,
                "WAYLAND_DISPLAY",
                "Environment variable WAYLAND_DISPLAY is not set",
                "Start Wyrd Shell inside an active Wayland session (e.g. Hyprland, Niri, Sway)",
            ));
        }
    }
}

fn check_compositor(items: &mut Vec<DiagnosticItem>) {
    let category = "Compositor IPC";

    if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let sock_path = PathBuf::from(runtime_dir)
            .join("hypr")
            .join(&sig)
            .join(".socket2.sock");
        if sock_path.exists() && UnixStream::connect(&sock_path).is_ok() {
            items.push(DiagnosticItem::ok(
                category,
                "Hyprland IPC",
                format!(
                    "Detected Hyprland session (signature: {}, socket responding)",
                    sig
                ),
            ));
        } else {
            items.push(DiagnosticItem::warn(
                category,
                "Hyprland IPC",
                format!(
                    "Hyprland signature set but socket at {:?} is not reachable",
                    sock_path
                ),
                "Verify Hyprland is active and responsive",
            ));
        }
        return;
    }

    if let Ok(niri_sock) = std::env::var("NIRI_SOCKET") {
        let path = PathBuf::from(&niri_sock);
        if path.exists() && UnixStream::connect(&path).is_ok() {
            items.push(DiagnosticItem::ok(
                category,
                "Niri IPC",
                format!("Detected Niri compositor socket responding at {:?}", path),
            ));
        } else {
            items.push(DiagnosticItem::warn(
                category,
                "Niri IPC",
                format!("NIRI_SOCKET is set to {:?} but connection failed", path),
                "Verify Niri compositor is active",
            ));
        }
        return;
    }

    if let Ok(swaysock) = std::env::var("SWAYSOCK") {
        let path = PathBuf::from(&swaysock);
        if path.exists() && UnixStream::connect(&path).is_ok() {
            items.push(DiagnosticItem::ok(
                category,
                "Sway IPC",
                format!("Detected Sway/i3 IPC socket responding at {:?}", path),
            ));
        } else {
            items.push(DiagnosticItem::warn(
                category,
                "Sway IPC",
                format!("SWAYSOCK is set to {:?} but connection failed", path),
                "Verify Sway compositor is active",
            ));
        }
        return;
    }

    items.push(DiagnosticItem::ok(
        category,
        "Generic Wayland",
        "No specific compositor IPC detected; using Wayland protocol-native integrations",
    ));
}

fn check_companion_daemons(items: &mut Vec<DiagnosticItem>) {
    let category = "Companion Daemons";
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());

    let daemons = [
        (
            "wyrd-windows",
            "wyrd-windows.sock",
            "Window titles, workspaces, layout indicator",
        ),
        (
            "wyrd-audio",
            "wyrd-audio.sock",
            "PipeWire/Pulse volume, sinks, mute state",
        ),
        (
            "wyrd-tray",
            "wyrd-tray.sock",
            "StatusNotifierItem system tray icons",
        ),
        (
            "wyrd-notifications",
            "wyrd-notifications.sock",
            "Desktop notifications server",
        ),
        (
            "wyrd-wallpaper",
            "wyrd-wallpaper.sock",
            "Wallpaper manager & color extraction",
        ),
        (
            "wyrd-clipboard",
            "wyrd-clipboard.sock",
            "Clipboard manager & history",
        ),
    ];

    for (name, sock_name, description) in daemons {
        let sock_path = Path::new(&runtime_dir).join(sock_name);
        if sock_path.exists() {
            if UnixStream::connect(&sock_path).is_ok() {
                items.push(DiagnosticItem::ok(
                    category,
                    format!("{} ({})", name, description),
                    format!("Socket responding at {:?}", sock_path),
                ));
            } else {
                items.push(DiagnosticItem::warn(
                    category,
                    format!("{} ({})", name, description),
                    format!(
                        "Socket exists at {:?} but connection failed (stale socket)",
                        sock_path
                    ),
                    format!("Restart service or relaunch '{}'", name),
                ));
            }
        } else {
            items.push(DiagnosticItem::warn(
                category,
                format!("{} ({})", name, description),
                "Daemon is not running (socket not found)",
                format!(
                    "Start service or run '{}' (auto-spawns on shell launch if in PATH)",
                    name
                ),
            ));
        }
    }
}

fn check_dbus(items: &mut Vec<DiagnosticItem>) {
    let category = "D-Bus Subsystem";

    let session_bus_path = if let Ok(addr) = std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        if let Some(path) = addr.strip_prefix("unix:path=") {
            Some(PathBuf::from(path.split(',').next().unwrap_or("")))
        } else {
            None
        }
    } else if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(runtime_dir).join("bus");
        if p.exists() {
            Some(p)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(path) = session_bus_path {
        if path.exists() && UnixStream::connect(&path).is_ok() {
            items.push(DiagnosticItem::ok(
                category,
                "D-Bus Session Bus",
                format!("Responding at {:?}", path),
            ));
        } else {
            items.push(DiagnosticItem::warn(
                category,
                "D-Bus Session Bus",
                format!("Found at {:?} but cannot connect", path),
                "Check session bus daemon status",
            ));
        }
    } else {
        items.push(DiagnosticItem::warn(
            category,
            "D-Bus Session Bus",
            "Session bus address not found in environment",
            "Verify DBUS_SESSION_BUS_ADDRESS is exported",
        ));
    }

    let system_bus_path = Path::new("/run/dbus/system_bus_socket");
    if system_bus_path.exists() && UnixStream::connect(system_bus_path).is_ok() {
        items.push(DiagnosticItem::ok(
            category,
            "D-Bus System Bus",
            "Responding at /run/dbus/system_bus_socket",
        ));
    } else {
        items.push(DiagnosticItem::warn(
            category,
            "D-Bus System Bus",
            "System bus socket not reachable",
            "Ensure system D-Bus daemon is running (e.g. systemctl status dbus or init service)",
        ));
    }
}

fn check_hardware(items: &mut Vec<DiagnosticItem>) {
    let category = "Hardware Subsystems";

    // Backlight
    let backlight_dir = Path::new("/sys/class/backlight");
    if backlight_dir.exists() {
        if let Ok(entries) = fs::read_dir(backlight_dir) {
            let valid_controllers: Vec<_> = entries.filter_map(Result::ok).collect();
            if valid_controllers.is_empty() {
                items.push(DiagnosticItem::warn(
                    category,
                    "Display Backlight",
                    "No controllers found in /sys/class/backlight (typical for desktop monitors)",
                    "Display backlight control uses /sys/class/backlight or compositor brightness protocol",
                ));
            } else {
                for entry in valid_controllers {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let b_file = entry.path().join("brightness");
                    if b_file.exists() {
                        let can_write = fs::OpenOptions::new().write(true).open(&b_file).is_ok();
                        if can_write {
                            items.push(DiagnosticItem::ok(
                                category,
                                format!("Backlight ({})", name),
                                "Controller found with read/write access",
                            ));
                        } else {
                            items.push(DiagnosticItem::warn(
                                category,
                                format!("Backlight ({})", name),
                                "Controller found, but write permission denied",
                                "Add user to 'video' or 'input' group, or set up udev rule for /sys/class/backlight",
                            ));
                        }
                    }
                }
            }
        }
    }

    // Battery / Power Supply
    let power_dir = Path::new("/sys/class/power_supply");
    if power_dir.exists() {
        if let Ok(entries) = fs::read_dir(power_dir) {
            let mut found_battery = false;
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("BAT") {
                    found_battery = true;
                    let capacity = fs::read_to_string(entry.path().join("capacity"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "unknown".to_string());
                    let status = fs::read_to_string(entry.path().join("status"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "unknown".to_string());
                    items.push(DiagnosticItem::ok(
                        category,
                        format!("Battery ({})", name),
                        format!("Level: {}%, Status: {}", capacity, status),
                    ));
                }
            }
            if !found_battery {
                items.push(DiagnosticItem::ok(
                    category,
                    "Power Supply",
                    "Desktop AC power (no battery devices detected)",
                ));
            }
        }
    }

    // Audio server socket
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let pw_socket = Path::new(&runtime_dir).join("pipewire-0");
    let pulse_socket = Path::new(&runtime_dir).join("pulse").join("native");
    if pw_socket.exists() {
        items.push(DiagnosticItem::ok(
            category,
            "Audio Server",
            format!("PipeWire socket found at {:?}", pw_socket),
        ));
    } else if pulse_socket.exists() {
        items.push(DiagnosticItem::ok(
            category,
            "Audio Server",
            format!("PulseAudio socket found at {:?}", pulse_socket),
        ));
    } else {
        items.push(DiagnosticItem::warn(
            category,
            "Audio Server",
            "Neither PipeWire nor PulseAudio socket found in XDG_RUNTIME_DIR",
            "Verify PipeWire or PulseAudio sound server daemon is running",
        ));
    }
}

pub fn print_diagnostics(items: &[DiagnosticItem]) -> bool {
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const RED: &str = "\x1b[31m";
    const CYAN: &str = "\x1b[36m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const RESET: &str = "\x1b[0m";

    println!();
    println!(
        "{}================================================================={}",
        CYAN, RESET
    );
    println!("{}                 Wyrd Shell System Doctor{}", BOLD, RESET);
    println!(
        "{}================================================================={}",
        CYAN, RESET
    );
    println!();

    let mut current_category = "";
    let mut ok_count = 0;
    let mut warn_count = 0;
    let mut error_count = 0;

    for item in items {
        if item.category != current_category {
            current_category = item.category;
            println!("{}{}{}:{}", BOLD, CYAN, current_category, RESET);
        }

        match item.status {
            DiagnosticStatus::Ok => {
                ok_count += 1;
                println!(
                    "  {}✓{} {}: {}{}{}",
                    GREEN, RESET, item.name, DIM, item.details, RESET
                );
            }
            DiagnosticStatus::Warning => {
                warn_count += 1;
                println!("  {}⚠{} {}: {}", YELLOW, RESET, item.name, item.details);
                if let Some(ref sug) = item.suggestion {
                    println!("    {}Tip: {}{}", YELLOW, sug, RESET);
                }
            }
            DiagnosticStatus::Error => {
                error_count += 1;
                println!(
                    "  {}✗{} {}: {}{}{}",
                    RED, RESET, item.name, BOLD, item.details, RESET
                );
                if let Some(ref sug) = item.suggestion {
                    println!("    {}Action: {}{}", RED, sug, RESET);
                }
            }
        }
    }

    println!();
    println!(
        "{}-----------------------------------------------------------------{}",
        DIM, RESET
    );
    print!("{}Summary:{} ", BOLD, RESET);
    print!("{}{} passed{}, ", GREEN, ok_count, RESET);
    print!("{}{} warnings{}, ", YELLOW, warn_count, RESET);
    println!(
        "{}{} errors{}",
        if error_count > 0 { RED } else { GREEN },
        error_count,
        RESET
    );
    println!();

    if error_count == 0 {
        println!(
            "{}All core system requirements are satisfied!{}",
            GREEN, RESET
        );
    } else {
        println!(
            "{}Some critical requirements are missing. Please address the errors above.{}",
            RED, RESET
        );
    }
    println!();

    error_count == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_diagnostics_produces_items() {
        let items = collect_diagnostics();
        assert!(
            !items.is_empty(),
            "Doctor should always produce diagnostic items"
        );
        assert!(items.iter().any(|i| i.category == "Wayland Environment"));
        assert!(items.iter().any(|i| i.category == "Compositor IPC"));
        assert!(items.iter().any(|i| i.category == "Companion Daemons"));
    }
}
