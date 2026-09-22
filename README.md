# Wyrd Shell

[![CI](https://github.com/xximxxhatedxx/wyrd-shell/actions/workflows/ci.yml/badge.svg)](https://github.com/xximxxhatedxx/wyrd-shell/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Wayland](https://img.shields.io/badge/Wayland-wlr--layer--shell-orange.svg)](https://wayland.freedesktop.org/)

**Wyrd Shell** is a fast, highly modular Wayland desktop shell written in Rust. It provides a declarative Lua configuration engine, high-performance 2D rendering powered by tiny-skia, and a sandboxed WebAssembly (WASM) plugin architecture.

Rather than being a rigid status bar, Wyrd is a flexible desktop surface environment supporting bars, docks, widgets, floating popups, application launchers, clipboard history, notification centers, and full compositor integration (Hyprland, Sway, Niri, River).

---

## Architecture & Ecosystem

Wyrd is organized into dedicated, modular components:

- **[`wyrd-engine`](https://github.com/xximxxhatedxx/wyrd-engine)**: Core rendering engine (`tiny-skia`), Wayland layer-shell protocol bindings, Flexbox-inspired layout engine, icon lookup, and compositor IPC integrations.
- **`wyrd-shell`** *(this repository)*: Main shell runtime, Lua configuration interpreter, surface coordinator, companion daemons, and 18 WASM ecosystem modules.
- **[`wyrd-wallpaper`](https://github.com/xximxxhatedxx/wyrd-wallpaper)**: Dynamic Wayland wallpaper daemon with Material You / M3 palette extraction, standalone wallpaper picker, and multi-monitor rendering.

---

## Features

- **🚀 Performance & Low Memory**: Pure Rust codebase using `tiny-skia` CPU rasterization for subpixel-accurate rendering without GPU overhead or compositor glitches.
- **🎨 Declarative Lua Configuration**: Customize themes, styles, panels, popups, and keybindings with hot-reloading (`~/.config/wyrd/init.lua`).
- **🧩 Sandboxed WASM Modules**: Extend functionality safely using compiled WebAssembly modules with the provided `wyrd-module-sdk`.
- **🔔 Built-in Companion Daemons**:
  - `wyrd-notifications`: FreeDesktop notification server (`org.freedesktop.Notifications`) with action support and notification history.
  - `wyrd-clipboard`: Wayland clipboard manager tracking text and image history.
  - `wyrd-audio`: Native PipeWire and PulseAudio volume and device management.
  - `wyrd-tray`: StatusNotifierItem (SNI) system tray host.
  - `wyrd-windows`: Window titles, active workspaces, and keyboard layout tracking.
- **⚙️ Compositor Integration**: Native IPC support for Hyprland, Sway, and Niri with automatic keybinding synchronization; standard `wlr-layer-shell-v1` compatibility for River, Wayfire, and all wlroots-compliant compositors.
- **📦 Zero Dependency Headaches**: Provided with systemd user units, cross-glibc portable packaging, and shell autocompletion.

---

## Included Modules

Wyrd Shell ships with 18 first-party WASM modules located in `wyrd-modules/`:

| Module | Description |
| :--- | :--- |
| `workspaces` | Interactive workspace pager with multi-monitor support (Hyprland, Sway, Niri) |
| `clock` | Live date/time clock with calendar popup |
| `tray` | System tray hosting StatusNotifierItem icons |
| `mpris` | Media player controller (playback, track info, seek bar, album art) |
| `audio` | Volume slider, mute toggle, and audio device picker |
| `network` | Network connectivity indicator and Wi-Fi manager |
| `bluetooth` | Bluetooth device status, pairing, and connection toggle |
| `battery` | Battery charge level, charging state, and power profile switcher |
| `brightness` | Screen brightness control via backlight / DDC |
| `system` | Real-time CPU, memory, disk, and temperature monitors |
| `keyboard` | Active keyboard layout indicator and switcher |
| `notifications` | Notification center toggle and unread count badge |
| `launcher` | Fuzzy-search application launcher |
| `clipboard` | Clipboard manager popup with history search |
| `power-menu` | Session management (Lock, Suspend, Reboot, Shutdown) |
| `window` | Active window title and desktop icon |
| `dnd` | Do Not Disturb notification mode toggle |
| `dynamic-color` | Material You dynamic theme extraction and color synchronizer |

---

## Quick Start

### Dependencies

- Rust (MSRV: 1.85+)
- `wasm32-unknown-unknown` Rust target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- Wayland development headers: `libwayland-dev`, `libxkbcommon-dev`, `libpulse-dev`, `libfontconfig-dev`, `libfreetype-dev` (or distro equivalents).

### Build

```bash
# Clone the repository
git clone https://github.com/xximxxhatedxx/wyrd-shell.git
cd wyrd-shell

# Build workspace binaries and daemons
cargo build --release --workspace

# Build WASM modules
./package.sh
```

### Install

Run the interactive installer:

```bash
./install.sh
```

The installer configures default directories, copies binaries to `~/.local/bin`, installs modules to `~/.local/share/wyrd/modules`, sets up themes, and enables user systemd services.

To uninstall:
```bash
./uninstall.sh
# To also remove user configuration (~/.config/wyrd):
./uninstall.sh --purge
```

---

## Configuration

Configuration is located in `~/.config/wyrd/init.lua`:

```lua
-- Define custom style
wyrd.style("bar", {
    background = "rgba(12, 14, 22, 0.88)",
    foreground = "#E2E8F0",
    accent     = "#7DD3FC",
    outline    = "1px solid rgba(125, 211, 252, 0.08)",
    radius     = 16,
    padding    = { horizontal = 18, vertical = 5 },
})

-- Create top bar
wyrd.create({
    type = "bar",
    name = "main_bar",
    layer = "top",
    height = 38,
    anchor = { "top", "left", "right" },
    margin = { top = 8, left = 16, right = 16 },
    exclusive_zone = 46,
    style = "bar",
    widgets = {
        -- Left: Workspaces & Window title
        { type = "module", name = "workspaces" },
        { type = "module", name = "window" },

        -- Center: Clock
        { type = "module", name = "clock" },

        -- Right: System, Audio, Battery, Tray
        { type = "module", name = "audio" },
        { type = "module", name = "battery" },
        { type = "module", name = "tray" },
    }
})

-- Bind global shortcuts
wyrd.keybind("SUPER", "Space", "popup:launcher")
wyrd.keybind("SUPER", "V", "popup:clipboard")
wyrd.keybind("SUPER", "N", "popup:notifications")
```

---

## Autostart

### Systemd (Recommended)
```bash
systemctl --user enable --now wyrd-shell
```

### Compositor Config
- **Hyprland** (`hyprland.conf`):
  ```ini
  exec-once = wyrd-shell
  ```
- **Sway** (`config`):
  ```ini
  exec wyrd-shell
  ```
- **Niri** (`config.kdl`):
  ```kdl
  spawn-at-startup "wyrd-shell"
  ```

---

## License

Dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
