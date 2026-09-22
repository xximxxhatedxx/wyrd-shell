#!/usr/bin/env bash
# =============================================================================
# Wyrd Shell - Modular Wayland Desktop Shell Installer
# =============================================================================
set -e

# ANSI Color codes
if [ -t 1 ]; then
    BOLD="\033[1m"
    GREEN="\033[1;32m"
    CYAN="\033[1;36m"
    YELLOW="\033[1;33m"
    RED="\033[1;31m"
    DIM="\033[2m"
    RESET="\033[0m"
else
    BOLD=""
    GREEN=""
    CYAN=""
    YELLOW=""
    RED=""
    DIM=""
    RESET=""
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Defaults
DEFAULT_BIN_DIR="$HOME/.local/bin"
DEFAULT_CONFIG_DIR="$HOME/.config/wyrd"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"

BIN_DIR="$DEFAULT_BIN_DIR"
CONFIG_DIR="$DEFAULT_CONFIG_DIR"
NON_INTERACTIVE=false
FORCE_BUILD=false
NO_SYSTEMD=false
DRY_RUN=false
PRESET=""
CUSTOM_MODULES_ARG=""

discover_modules() {
    local dir
    local -a modules=()
    for dir in "$SCRIPT_DIR"/wyrd-modules/* "$SCRIPT_DIR"/modules/* "$HOME/.config/wyrd/modules"/*; do
        [ -d "$dir" ] || continue
        if [ -f "$dir/manifest.toml" ] || [ -f "$dir/Cargo.toml" ]; then
            modules+=("$(basename "$dir")")
        fi
    done
    printf '%s\n' "${modules[@]}" | sort -u
}

discover_daemons() {
    local -a pkgs=()
    if [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
        local dir
        for dir in "$SCRIPT_DIR"/wyrd-*; do
            if [ -d "$dir" ] && [ -f "$dir/src/main.rs" ] && [ -f "$dir/Cargo.toml" ]; then
                local pkg_name
                pkg_name=$(grep -m1 '^name =' "$dir/Cargo.toml" | awk -F '"' '{print $2}')
                if [ -n "$pkg_name" ] && [ "$pkg_name" != "wyrd-shell" ]; then
                    pkgs+=("$pkg_name")
                fi
            fi
        done
    elif [ -d "$SCRIPT_DIR/bin" ]; then
        for f in "$SCRIPT_DIR"/bin/wyrd-*; do
            [ -f "$f" ] || continue
            local b
            b=$(basename "$f")
            if [ "$b" != "wyrd-shell" ]; then
                pkgs+=("$b")
            fi
        done
    fi
    printf '%s\n' "${pkgs[@]}" | sort -u
}

ALL_MODULES=($(discover_modules))
DAEMONS=($(discover_daemons))

# Descriptions (English)
declare -A MOD_DESC=(
    ["workspaces"]="Workspace switcher and window indicator"
    ["clock"]="Clock, date and interactive calendar popup"
    ["tray"]="System tray (SNI) with Freedesktop icon resolution"
    ["audio"]="Volume control, microphone and audio device selector"
    ["network"]="Wi-Fi / Ethernet state and network selector popup"
    ["bluetooth"]="BlueZ Bluetooth device discovery and pairing"
    ["battery"]="Battery status, percentage and power levels"
    ["system"]="System monitor (CPU, RAM, disk, temperature graphs)"
    ["notifications"]="Notification center and toast popups"
    ["launcher"]="Application launcher with fuzzy search"
    ["power-menu"]="Power management (Lock, Sleep, Reboot, Shutdown)"
    ["dnd"]="Do Not Disturb toggle"
    ["mpris"]="Media player status, transport controls and playback console popup"
    ["clipboard"]="Clipboard history daemon and manager"
    ["dynamic-color"]="Material You dynamic palette extraction and theme generator"
    ["window"]="Active window title and app icon indicator"
    ["brightness"]="Display brightness and backlight control"
    ["keyboard"]="Keyboard layout indicator and switcher"
)

# Usage help
print_usage() {
    echo -e "${BOLD}Wyrd Shell Installer${RESET}

Usage:
  ./install.sh [options]

Options:
  -p, --preset <NAME>       Preset configuration: all, laptop, desktop, minimal
  -m, --modules <LIST>      Comma-separated list of modules (e.g. clock,workspaces,audio)
      --bin-dir <DIR>       Destination directory for wyrd-shell binary (default: ~/.local/bin)
      --config-dir <DIR>    Configuration directory (default: ~/.config/wyrd)
  -b, --build               Force rebuild from source using cargo
      --no-systemd          Skip installing systemd user service
      --uninstall           Uninstall Wyrd Shell and remove services/binaries
  -y, --yes, --non-interactive  Run non-interactively with defaults
      --dry-run             Show planned actions without modifying any files
  -h, --help                Show this help message

Presets:
  all      All 19 ecosystem modules (full functionality)
  laptop   Optimized for laptops (battery, Wi-Fi, Bluetooth, audio, tray, clock, workspaces)
  desktop  Optimized for desktops (no battery: system monitor, audio, network, tray, launcher, clock, workspaces)
  minimal  Minimal lightweight bar (workspaces, clock, audio, system tray)
"
}

# Parse CLI arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -p|--preset)
            PRESET="$2"
            shift 2
            ;;
        --preset=*)
            PRESET="${1#*=}"
            shift
            ;;
        -m|--modules)
            CUSTOM_MODULES_ARG="$2"
            shift 2
            ;;
        --modules=*)
            CUSTOM_MODULES_ARG="${1#*=}"
            shift
            ;;
        --bin-dir)
            BIN_DIR="$2"
            shift 2
            ;;
        --bin-dir=*)
            BIN_DIR="${1#*=}"
            shift
            ;;
        --config-dir)
            CONFIG_DIR="$2"
            shift 2
            ;;
        --config-dir=*)
            CONFIG_DIR="${1#*=}"
            shift
            ;;
        -b|--build)
            FORCE_BUILD=true
            shift
            ;;
        --no-systemd)
            NO_SYSTEMD=true
            shift
            ;;
        --uninstall)
            shift
            exec "$SCRIPT_DIR/uninstall.sh" "$@"
            ;;
        -y|--yes|--non-interactive)
            NON_INTERACTIVE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option:${RESET} $1"
            print_usage
            exit 1
            ;;
    esac
done

echo -e "${CYAN}${BOLD}"
echo "================================================================="
echo "                    Wyrd Shell Installer"
echo "================================================================="
echo -e "${RESET}"

# Selected modules array
SELECTED_MODULES=()

resolve_preset() {
    local p="$1"
    local -a available=()
    available=("${ALL_MODULES[@]}")
    SELECTED_MODULES=()

    case "$p" in
        all)
            SELECTED_MODULES=("${available[@]}")
            ;;
        laptop)
            for mod in workspaces clock tray audio brightness keyboard network bluetooth battery notifications launcher power-menu; do
                if printf '%s\n' "${available[@]}" | grep -qx "$mod"; then
                    SELECTED_MODULES+=("$mod")
                fi
            done
            ;;
        desktop)
            for mod in workspaces clock tray audio keyboard network bluetooth system notifications launcher power-menu; do
                if printf '%s\n' "${available[@]}" | grep -qx "$mod"; then
                    SELECTED_MODULES+=("$mod")
                fi
            done
            ;;
        minimal)
            for mod in workspaces clock tray audio; do
                if printf '%s\n' "${available[@]}" | grep -qx "$mod"; then
                    SELECTED_MODULES+=("$mod")
                fi
            done
            ;;
        *)
            echo -e "${RED}Unknown preset:${RESET} $p"
            echo "Available presets: all, laptop, desktop, minimal"
            exit 1
            ;;
    esac
}

# 1. Selection mode
if [ -n "$CUSTOM_MODULES_ARG" ]; then
    IFS=',' read -ra ADDR <<< "$CUSTOM_MODULES_ARG"
    for m in "${ADDR[@]}"; do
        clean_m=$(echo "$m" | tr -d '[:space:]')
        if [[ " ${ALL_MODULES[*]} " =~ " ${clean_m} " ]]; then
            SELECTED_MODULES+=("$clean_m")
        else
            echo -e "${YELLOW}Warning:${RESET} skipping unknown module '${clean_m}'."
        fi
    done
elif [ -n "$PRESET" ]; then
    resolve_preset "$PRESET"
elif [ "$NON_INTERACTIVE" = true ]; then
    resolve_preset "all"
else
    # Interactive menu
    echo -e "${BOLD}Select module collection to install:${RESET}"
    echo "  [1] 🌟 All Recommended (All ${#ALL_MODULES[@]} modules) [Default]"
    echo "  [2] 💻 Laptop (Clock, workspaces, battery, audio, network, bluetooth, tray, notifications, power-menu)"
    echo "  [3] 🖥️  Desktop (No battery: clock, workspaces, system, audio, network, bluetooth, tray, launcher, power-menu)"
    echo "  [4] ⚡ Minimal (Workspaces, clock, audio, tray)"
    echo "  [5] 🛠️  Custom selection (Select modules individually)"
    echo ""
    read -rp "Your choice [1-5, Enter=1]: " CHOICE

    case "$CHOICE" in
        2) resolve_preset "laptop" ;;
        3) resolve_preset "desktop" ;;
        4) resolve_preset "minimal" ;;
        5)
            echo ""
            echo -e "${BOLD}Interactive module selection:${RESET}"
            declare -A MOD_STATE
            for m in "${ALL_MODULES[@]}"; do
                MOD_STATE["$m"]=1
            done

            while true; do
                echo ""
                echo -e "${CYAN}Current selection (enter item number to toggle, or 'd'/Enter to finish):${RESET}"
                idx=1
                for m in "${ALL_MODULES[@]}"; do
                    if [ "${MOD_STATE[$m]}" -eq 1 ]; then
                        chk="${GREEN}[X]${RESET}"
                    else
                        chk="${DIM}[ ]${RESET}"
                    fi
                    printf "  [%2d] %b %-14s %s\n" "$idx" "$chk" "$m" "${MOD_DESC[$m]}"
                    ((idx++))
                done
                echo ""
                read -rp "Toggle item (1-${#ALL_MODULES[@]}) or 'd' to confirm: " TOGGLE
                if [[ -z "$TOGGLE" || "$TOGGLE" == "d" || "$TOGGLE" == "D" || "$TOGGLE" == "done" ]]; then
                    break
                fi
                if [[ "$TOGGLE" =~ ^[0-9]+$ ]] && [ "$TOGGLE" -ge 1 ] && [ "$TOGGLE" -le "${#ALL_MODULES[@]}" ]; then
                    target_mod="${ALL_MODULES[$((TOGGLE-1))]}"
                    if [ "${MOD_STATE[$target_mod]}" -eq 1 ]; then
                        MOD_STATE["$target_mod"]=0
                    else
                        MOD_STATE["$target_mod"]=1
                    fi
                else
                    echo -e "${RED}Invalid input, please enter a number between 1 and ${#ALL_MODULES[@]}.${RESET}"
                fi
            done

            SELECTED_MODULES=()
            for m in "${ALL_MODULES[@]}"; do
                if [ "${MOD_STATE[$m]}" -eq 1 ]; then
                    SELECTED_MODULES+=("$m")
                fi
            done
            ;;
        *) resolve_preset "all" ;;
    esac
fi

if [ "${#SELECTED_MODULES[@]}" -eq 0 ]; then
    echo -e "${YELLOW}Warning: no modules selected. Falling back to core modules (workspaces, clock, audio, tray).${RESET}"
    SELECTED_MODULES=("workspaces" "clock" "audio" "tray")
fi

echo ""
echo -e "${BOLD}Selected modules (${#SELECTED_MODULES[@]}):${RESET}"
for m in "${SELECTED_MODULES[@]}"; do
    echo -e "  ${GREEN}✓${RESET} ${BOLD}$m${RESET} (${MOD_DESC[$m]})"
done
echo ""

# 2. Check binary wyrd-shell and standalone daemons
find_binary() {
    # 1. Local bin in package
    if [ -f "$SCRIPT_DIR/bin/wyrd-shell" ] && [ "$FORCE_BUILD" = false ]; then
        echo "$SCRIPT_DIR/bin/wyrd-shell"
        return
    fi
    # 2. Cross-glibc / zigbuild target release
    local arch="$(uname -m)"
    if [ -f "$SCRIPT_DIR/target/${arch}-unknown-linux-gnu/release/wyrd-shell" ] && [ "$FORCE_BUILD" = false ]; then
        echo "$SCRIPT_DIR/target/${arch}-unknown-linux-gnu/release/wyrd-shell"
        return
    fi
    # 3. Standard target release
    if [ -f "$SCRIPT_DIR/target/release/wyrd-shell" ] && [ "$FORCE_BUILD" = false ]; then
        echo "$SCRIPT_DIR/target/release/wyrd-shell"
        return
    fi
    # 4. Already installed in system
    if command -v wyrd-shell &>/dev/null && [ "$FORCE_BUILD" = false ]; then
        command -v wyrd-shell
        return
    fi
    echo ""
}

SHELL_BIN=$(find_binary)

find_daemon_binary() {
    local daemon="$1"
    local arch="$(uname -m)"
    local candidates=(
        "$SCRIPT_DIR/bin/$daemon"
        "$SCRIPT_DIR/target/${arch}-unknown-linux-gnu/release/$daemon"
        "$SCRIPT_DIR/target/release/$daemon"
    )
    for candidate in "${candidates[@]}"; do
        if [ -f "$candidate" ] && [ "$FORCE_BUILD" = false ]; then
            echo "$candidate"
            return
        fi
    done
    if command -v "$daemon" &>/dev/null && [ "$FORCE_BUILD" = false ]; then
        command -v "$daemon"
    fi
}

# Compile helper that prefers cargo-zigbuild if available
compile_pkg() {
    local pkg="$1"
    local arch="$(uname -m)"
    if command -v cargo-zigbuild &>/dev/null && command -v zig &>/dev/null; then
        RUSTFLAGS="-C link-arg=-L/usr/lib" cargo zigbuild --release --target "${arch}-unknown-linux-gnu.2.31" -p "$pkg"
    else
        cargo build --release -p "$pkg"
    fi
}

check_build_deps() {
    local missing=()
    if ! command -v pkg-config &>/dev/null; then
        missing+=("pkg-config")
    else
        pkg-config --exists wayland-client 2>/dev/null || missing+=("wayland-client (libwayland-dev / wayland)")
        pkg-config --exists xkbcommon 2>/dev/null || missing+=("xkbcommon (libxkbcommon-dev / libxkbcommon)")
        pkg-config --exists libpulse 2>/dev/null || missing+=("libpulse (libpulse-dev / libpulse)")
    fi
    if [ ${#missing[@]} -gt 0 ]; then
        echo -e "${YELLOW}Warning: Missing build dependencies detected:${RESET}"
        for dep in "${missing[@]}"; do
            echo -e "  - $dep"
        done
        echo "Building from source might fail. Install the corresponding packages using your package manager:"
        echo "  Arch Linux:      sudo pacman -S pkgconf wayland libxkbcommon libpulse"
        echo "  Debian / Ubuntu: sudo apt install pkg-config libwayland-dev libxkbcommon-dev libpulse-dev"
        echo "  Fedora:          sudo dnf install pkgconf wayland-devel libxkbcommon-devel pulseaudio-libs-devel"
        echo ""
    fi
}

if [ -z "$SHELL_BIN" ] || [ "$FORCE_BUILD" = true ]; then
    echo -e "${YELLOW}Pre-built wyrd-shell binary not found (or --build was specified).${RESET}"
    if ! command -v cargo &>/dev/null; then
        echo -e "${RED}Error: Rust/Cargo is not installed on this system.${RESET}"
        echo "Please download an official pre-built release package of Wyrd Shell or install Rust via https://rustup.rs"
        exit 1
    fi

    check_build_deps

    echo -e "${CYAN}Compiling wyrd-shell in release mode...${RESET}"
    if [ "$DRY_RUN" = false ]; then
        compile_pkg "wyrd-shell"
        SHELL_BIN=$(find_binary)
        if [ -z "$SHELL_BIN" ]; then
            SHELL_BIN="$SCRIPT_DIR/target/release/wyrd-shell"
        fi
    fi
fi

DAEMON_BINS=()
for daemon in "${DAEMONS[@]}"; do
    daemon_bin=$(find_daemon_binary "$daemon")
    if [ -z "$daemon_bin" ] || [ "$FORCE_BUILD" = true ]; then
        if ! command -v cargo &>/dev/null; then
            echo -e "${RED}Error: missing $daemon and Rust/Cargo is not installed.${RESET}"
            exit 1
        fi
        echo -e "${CYAN}Compiling $daemon in release mode...${RESET}"
        if [ "$DRY_RUN" = false ]; then
            compile_pkg "$daemon"
            daemon_bin=$(find_daemon_binary "$daemon")
            if [ -z "$daemon_bin" ]; then
                daemon_bin="$SCRIPT_DIR/target/release/$daemon"
            fi
        fi
    fi
    DAEMON_BINS+=("$daemon_bin")
done

# 3. Check / Build WASM modules
find_wasm() {
    local mod_name="$1"
    local candidates=(
        "$SCRIPT_DIR/target/wasm32-unknown-unknown/release/wyrd_module_${mod_name//-/_}.wasm"
        "$SCRIPT_DIR/target/wasm32-unknown-unknown/release/${mod_name//-/_}.wasm"
        "$SCRIPT_DIR/target/wasm32-unknown-unknown/release/${mod_name}.wasm"
        "$SCRIPT_DIR/modules/${mod_name}/${mod_name}.wasm"
        "$SCRIPT_DIR/modules/${mod_name}/wyrd_module_${mod_name//-/_}.wasm"
        "$HOME/.config/wyrd/modules/${mod_name}/wyrd_module_${mod_name//-/_}.wasm"
        "$HOME/.config/wyrd/modules/${mod_name}/${mod_name}.wasm"
    )
    for c in "${candidates[@]}"; do
        if [ -f "$c" ]; then
            echo "$c"
            return
        fi
    done
    echo ""
}

# 4. Confirmation (if interactive)
if [ "$NON_INTERACTIVE" = false ] && [ "$DRY_RUN" = false ]; then
    echo -e "${BOLD}Installation parameters:${RESET}"
    echo "  Binary path:     $BIN_DIR/wyrd-shell"
    echo "  Config directory: $CONFIG_DIR"
    echo "  Systemd service:  $([ "$NO_SYSTEMD" = true ] && echo 'Skipped' || echo "$SYSTEMD_USER_DIR/wyrd-shell.service")"
    echo ""
    read -rp "Proceed with installation? [Y/n]: " CONFIRM
    if [[ "$CONFIRM" =~ ^[Nn] ]]; then
        echo "Installation aborted."
        exit 0
    fi
fi

# 5. Create directories
if [ "$DRY_RUN" = true ]; then
    echo -e "\n${YELLOW}[DRY-RUN] Files will not be modified.${RESET}"
fi

echo -e "\n${CYAN}[1/7] Installing shell and standalone daemon binaries...${RESET}"
if [ "$DRY_RUN" = false ]; then
    mkdir -p "$BIN_DIR"
    cp -f "$SHELL_BIN" "$BIN_DIR/wyrd-shell"
    chmod +x "$BIN_DIR/wyrd-shell"
    echo -e "  ${GREEN}✓${RESET} Installed: $BIN_DIR/wyrd-shell"
    for index in "${!DAEMONS[@]}"; do
        daemon="${DAEMONS[$index]}"
        cp -f "${DAEMON_BINS[$index]}" "$BIN_DIR/$daemon"
        chmod +x "$BIN_DIR/$daemon"
        echo -e "  ${GREEN}✓${RESET} Installed: $BIN_DIR/$daemon"
    done
else
    echo "  [dry-run] cp $SHELL_BIN -> $BIN_DIR/wyrd-shell"
    for daemon in "${DAEMONS[@]}"; do
        echo "  [dry-run] install $daemon -> $BIN_DIR/$daemon"
    done
fi

# 6. Backup existing config if present
if [ -d "$CONFIG_DIR" ] && [ "$(ls -A "$CONFIG_DIR" 2>/dev/null)" ]; then
    BACKUP_DIR="${CONFIG_DIR}.bak.$(date +%Y%m%d_%H%M%S)"
    echo -e "\n${YELLOW}[2/7] Existing configuration directory found at $CONFIG_DIR.${RESET}"
    if [ "$DRY_RUN" = false ]; then
        cp -r "$CONFIG_DIR" "$BACKUP_DIR"
        echo -e "  ${GREEN}✓${RESET} Backup created: $BACKUP_DIR"
    else
        echo "  [dry-run] backup $CONFIG_DIR -> $BACKUP_DIR"
    fi
else
    echo -e "\n${CYAN}[2/7] Preparing configuration directory...${RESET}"
fi

# 7. Install Themes (Aetheria & Catppuccin)
echo -e "\n${CYAN}[3/7] Installing themes...${RESET}"
if [ "$DRY_RUN" = false ]; then
    mkdir -p "$CONFIG_DIR/themes"
    themes_src="$SCRIPT_DIR/themes"
    if [ ! -d "$themes_src" ]; then
        themes_src="$SCRIPT_DIR/examples/config/themes"
    fi
    cp -rf "$themes_src/"* "$CONFIG_DIR/themes/" 2>/dev/null || true
    echo -e "  ${GREEN}✓${RESET} Themes installed in $CONFIG_DIR/themes"
else
    echo "  [dry-run] copy themes -> $CONFIG_DIR/themes"
fi

# 8. Install Selected Modules
echo -e "\n${CYAN}[4/7] Installing selected WASM modules (${#SELECTED_MODULES[@]})...${RESET}"
if [ "$DRY_RUN" = false ] && command -v rustup &>/dev/null; then
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
fi
for mod in "${SELECTED_MODULES[@]}"; do
    wasm_path=$(find_wasm "$mod")
    if [ -z "$wasm_path" ] || [ "$FORCE_BUILD" = true ]; then
        if command -v cargo &>/dev/null; then
            echo -e "  ${CYAN}Building module ${mod}...${RESET}"
            if [ "$DRY_RUN" = false ]; then
                cargo build -p "wyrd-module-${mod}" --target wasm32-unknown-unknown --release 2>/dev/null || true
                wasm_path=$(find_wasm "$mod")
            fi
        fi
    fi

    if [ "$DRY_RUN" = false ]; then
        mod_dir="$CONFIG_DIR/modules/$mod"
        mkdir -p "$mod_dir"

        # Copy manifest
        manifest_src="$SCRIPT_DIR/modules/$mod/manifest.toml"
        if [ ! -f "$manifest_src" ]; then
            manifest_src="$SCRIPT_DIR/wyrd-modules/$mod/manifest.toml"
        fi
        if [ -f "$manifest_src" ]; then
            cp -f "$manifest_src" "$mod_dir/manifest.toml"
        fi

        # Copy wasm
        if [ -n "$wasm_path" ] && [ -f "$wasm_path" ]; then
            cp -f "$wasm_path" "$mod_dir/wyrd_module_${mod//-/_}.wasm"
            cp -f "$wasm_path" "$mod_dir/${mod}.wasm"
            cp -f "$wasm_path" "$CONFIG_DIR/modules/wyrd_module_${mod//-/_}.wasm"
            echo -e "  ${GREEN}✓${RESET} Module $mod installed"
        else
            echo -e "  ${YELLOW}⚠${RESET} WASM binary for $mod not found"
        fi
    else
        echo "  [dry-run] install module $mod -> $CONFIG_DIR/modules/$mod"
    fi
done

# 9. Generate init.lua and surfaces/bar.lua based on selected modules
echo -e "\n${CYAN}[5/7] Generating tailored init.lua and bar surface layout...${RESET}"

has_module() {
    local target="$1"
    for m in "${SELECTED_MODULES[@]}"; do
        if [ "$m" == "$target" ]; then
            return 0
        fi
    done
    return 1
}

if [ "$DRY_RUN" = false ]; then
    mkdir -p "$CONFIG_DIR/surfaces"

    # Copy popups.lua
    popups_src="$SCRIPT_DIR/surfaces/popups.lua"
    if [ ! -f "$popups_src" ]; then
        popups_src="$SCRIPT_DIR/examples/config/surfaces/popups.lua"
    fi
    if [ -f "$popups_src" ]; then
        cp -f "$popups_src" "$CONFIG_DIR/surfaces/popups.lua"
    fi

    # Copy bar_left.lua
    bar_left_src="$SCRIPT_DIR/surfaces/bar_left.lua"
    if [ ! -f "$bar_left_src" ]; then
        bar_left_src="$SCRIPT_DIR/examples/config/surfaces/bar_left.lua"
    fi
    if [ -f "$bar_left_src" ]; then
        cp -f "$bar_left_src" "$CONFIG_DIR/surfaces/bar_left.lua"
    fi

    # Copy settings.lua
    settings_lua_src="$SCRIPT_DIR/settings.lua"
    if [ ! -f "$settings_lua_src" ]; then
        settings_lua_src="$SCRIPT_DIR/examples/config/settings.lua"
    fi
    if [ -f "$settings_lua_src" ]; then
        cp -f "$settings_lua_src" "$CONFIG_DIR/settings.lua"
    fi

    # Copy settings.toml if not already present
    if [ ! -f "$CONFIG_DIR/settings.toml" ]; then
        settings_toml_src="$SCRIPT_DIR/settings.toml"
        if [ ! -f "$settings_toml_src" ]; then
            settings_toml_src="$SCRIPT_DIR/examples/config/settings.toml"
        fi
        if [ -f "$settings_toml_src" ]; then
            cp -f "$settings_toml_src" "$CONFIG_DIR/settings.toml"
        fi
    fi

    # Generate init.lua
    cat > "$CONFIG_DIR/init.lua" <<EOF
-- =============================================================================
-- WYRD SHELL: MAIN CONFIGURATION
-- Generated by Wyrd Shell Installer on $(date -u +"%Y-%m-%dT%H:%M:%SZ")
-- =============================================================================

-- 1. Load User Settings
local settings = require("settings").load()

-- 2. Apply Theme Styles
local theme_name = settings.theme or "catppuccin"
local ok, theme_mod = pcall(require, "themes." .. theme_name)
if ok and theme_mod and type(theme_mod.apply) == "function" then
    theme_mod.apply()
elseif settings.theme == "aetheria" then
    require("themes.aetheria").apply()
else
    require("themes.catppuccin").apply()
end

-- 3. Load Selected Modules
EOF

    for mod in "${SELECTED_MODULES[@]}"; do
        echo "Module.load(\"$mod\")" >> "$CONFIG_DIR/init.lua"
    done

    cat >> "$CONFIG_DIR/init.lua" <<EOF
-- 4. Create Declarative Surfaces
if settings.layout == "left" then
    require("surfaces.bar_left")
else
    require("surfaces.bar")
end

require("surfaces.popups")
EOF

    # Generate surfaces/bar.lua containing only selected widgets
    cat > "$CONFIG_DIR/surfaces/bar.lua" <<EOF
-- =============================================================================
-- MAIN BAR SURFACE
-- Generated by Wyrd Shell Installer
-- =============================================================================

wyrd.create({
    type = "bar",
    name = "main_bar",
    layer = "top",
    height = 38,
    anchor = { "top", "left", "right" },
    margin = { top = 10, left = 20, right = 20 },
    exclusive_zone = 48,
    
    style = "bar",
    
    widgets = {
        -- ========== LEFT SECTION ==========
        {
            type = "container",
            layout = { mode = "flex_row", gap = 8, align = "center", weight = 1, justify = "start" },
            children = {
EOF

    # Left widgets
    if has_module "launcher"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "launcher_btn",
                    text = "󰀻",
                    style = "launcher_round",
                    action = "popup:launcher",
                    layout = { width = 32, height = 28, justify = "center", align = "center" },
                },
EOF
    fi

    if has_module "mpris"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "mpris",
                    text = "󰎆 Media",
                    tooltip = "Media Player",
                    action = "popup:mpris",
                    on_right_click = "event:mpris:mpris_play_pause:click",
                    style = "chip",
                },
EOF
    fi

    if has_module "network"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "network",
                    text = "󰤨 {network.ssid}",
                    action = "popup:network",
                    style = "chip",
                },
EOF
    fi

    if has_module "bluetooth"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "bluetooth",
                    text = "󰂯",
                    action = "popup:bluetooth",
                    style = "chip",
                },
EOF
    fi

    if has_module "tray"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "tray",
                    id = "tray",
                    style = "clean_item",
                    layout = { mode = "flex_row", align = "center", gap = 6 },
                },
EOF
    fi

    cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
            }
        },
        
        -- ========== CENTER SECTION ==========
EOF

    if has_module "workspaces"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
        {
            type = "workspaces",
            id = "workspaces",
            layout = { mode = "flex_row", gap = 4, align = "center", justify = "center" },
        },
EOF
    else
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
        {
            type = "container",
            layout = { mode = "flex_row", align = "center", justify = "center" },
            children = {},
        },
EOF
    fi

    cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
        
        -- ========== RIGHT SECTION ==========
        {
            type = "container",
            layout = { mode = "flex_row", gap = 6, align = "center", weight = 1, justify = "end" },
            children = {
EOF

    # Right widgets
    if has_module "system"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                -- Performance / System Monitor (Uncomment to display on bar)
                -- {
                --     type = "button",
                --     id = "system",
                --     text = "󰍛 {system.cpu}% • 󰚌 {system.mem}%",
                --     action = "popup:system",
                --     style = "chip",
                -- },
EOF
    fi

    if has_module "audio"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "audio",
                    text = "󰕾 {audio.volume}%",
                    action = "popup:audio",
                    style = "chip",
                },
                {
                    type = "button",
                    id = "microphone",
                    text = "󰍬 {microphone.volume}%",
                    action = "popup:microphone",
                    style = "chip",
                },
EOF
    fi

    if has_module "battery"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "battery",
                    text = "󰁹 {battery.percent}%",
                    action = "popup:battery",
                    style = "chip",
                },
EOF
    fi

    if has_module "brightness"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                -- Display Brightness (Uncomment to display on bar; available in Quick Settings)
                -- {
                --     type = "button",
                --     id = "brightness",
                --     text = "󰃟 {brightness.percent}%",
                --     action = "popup:brightness",
                --     style = "chip",
                -- },
EOF
    fi

    if has_module "keyboard"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "keyboard",
                    text = "󰌌 {keyboard.short_name}",
                    action = "popup:keyboard",
                    style = "chip",
                },
EOF
    fi

    if has_module "notifications"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "notifications",
                    text = "󰂚 {notifications.count}",
                    action = "popup:notifications",
                    style = "chip",
                },
EOF
    fi

    if has_module "clock"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "clock",
                    text = "󰥔 {clock.time} • {clock.date}",
                    action = "popup:calendar",
                    style = "chip",
                },
EOF
    fi

    # Quick Settings / Control Center
    cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                {
                    type = "button",
                    id = "quicksettings_btn",
                    text = "󰒓",
                    action = "popup:quicksettings",
                    tooltip = "Quick Settings",
                    style = "chip",
                },
EOF

    if has_module "power-menu"; then
        cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
                -- Standalone Power Button (Available in Quick Settings)
                -- {
                --     type = "button",
                --     id = "power_btn",
                --     text = "⏻",
                --     action = "popup:power",
                --     style = "power_round",
                --     layout = { width = 32, height = 32, justify = "center", align = "center" },
                -- },
EOF
    fi

    cat >> "$CONFIG_DIR/surfaces/bar.lua" <<EOF
            }
        }
    }
})
EOF

    echo -e "  ${GREEN}✓${RESET} Configuration generated matching selected modules"
fi

# 10. Install systemd user services
if [ "$NO_SYSTEMD" = false ]; then
    echo -e "\n${CYAN}[6/7] Installing systemd user services...${RESET}"
    if [ "$DRY_RUN" = false ]; then
        mkdir -p "$SYSTEMD_USER_DIR"

        # Determine companion daemons based on selected modules
        selected_daemon_services=()
        has_module "audio" && selected_daemon_services+=("wyrd-audio.service")
        has_module "tray" && selected_daemon_services+=("wyrd-tray.service")
        if has_module "workspaces" || has_module "window" || has_module "keyboard"; then
            selected_daemon_services+=("wyrd-windows.service")
        fi
        has_module "notifications" && selected_daemon_services+=("wyrd-notifications.service")
        has_module "clipboard" && selected_daemon_services+=("wyrd-clipboard.service")
        if [ -f "$SCRIPT_DIR/systemd/wyrd-idle.service" ]; then
            selected_daemon_services+=("wyrd-idle.service")
        fi

        # Install selected companion service units
        valid_companion_services=()
        for svc in "${selected_daemon_services[@]}"; do
            src="$SCRIPT_DIR/systemd/$svc"
            if [ -f "$src" ]; then
                valid_companion_services+=("$svc")
                cp -f "$src" "$SYSTEMD_USER_DIR/$svc"
                echo -e "  ${GREEN}✓${RESET} Bound companion service: $SYSTEMD_USER_DIR/$svc"
            fi
        done

        # Build dynamic wyrd-shell.service linking all selected companion daemons
        wants_directive=""
        after_directive="After=graphical-session.target"
        if [ "${#valid_companion_services[@]}" -gt 0 ]; then
            wants_directive="Wants=${valid_companion_services[*]}"
            after_directive="After=graphical-session.target ${valid_companion_services[*]}"
        fi

        cat > "$SYSTEMD_USER_DIR/wyrd-shell.service" <<EOF
[Unit]
Description=Wyrd Wayland Shell
PartOf=graphical-session.target
ConditionEnvironment=WAYLAND_DISPLAY
$after_directive
$wants_directive

[Service]
Type=simple
Environment="MIMALLOC_PURGE_DELAY=0"
Environment="MIMALLOC_ARENA_PURGE_MULT=1"
Environment="MIMALLOC_ALLOW_LARGE_OS_PAGES=0"
Environment="MIMALLOC_RESERVE_HUGE_OS_PAGES=0"
Environment="MIMALLOC_ALLOW_THP=0"
Environment="MIMALLOC_ARENA_EAGER_COMMIT=0"
Environment="MALLOC_ARENA_MAX=2"
ExecStart=$BIN_DIR/wyrd-shell
Restart=on-failure
RestartSec=2

[Install]
WantedBy=graphical-session.target default.target
EOF
        echo -e "  ${GREEN}✓${RESET} Master shell service created: $SYSTEMD_USER_DIR/wyrd-shell.service"

        if command -v systemctl &>/dev/null; then
            systemctl --user daemon-reload || true
            all_services=("wyrd-shell.service" "${valid_companion_services[@]}")
            if systemctl --user enable --now "${all_services[@]}" 2>/dev/null; then
                echo -e "  ${GREEN}✓${RESET} Systemd user services bound & active: ${all_services[*]}"
            else
                systemctl --user enable "${all_services[@]}" 2>/dev/null || true
                echo -e "  ${GREEN}✓${RESET} Systemd user services enabled for auto-start with session: ${all_services[*]}"
            fi
        fi
    else
        echo "  [dry-run] generate $SYSTEMD_USER_DIR/wyrd-shell.service bound to selected daemons"
        echo "  [dry-run] enable and start systemd user services"
    fi
else
    echo -e "\n${YELLOW}[6/7] Skipping systemd user services (--no-systemd specified).${RESET}"
fi

# 11. Install shell completions
echo -e "\n${CYAN}[7/7] Installing shell completions...${RESET}"
completions_dir="$SCRIPT_DIR/completions"
if [ "$DRY_RUN" = false ]; then
    if [ -d "$completions_dir" ]; then
        # Bash
        mkdir -p "$HOME/.local/share/bash-completion/completions"
        if [ -f "$completions_dir/wyrd-shell.bash" ]; then
            cp -f "$completions_dir/wyrd-shell.bash" "$HOME/.local/share/bash-completion/completions/wyrd-shell" 2>/dev/null || true
            echo -e "  ${GREEN}✓${RESET} Bash completion installed"
        fi

        # Zsh
        mkdir -p "$HOME/.local/share/zsh/site-functions"
        if [ -f "$completions_dir/_wyrd-shell" ]; then
            cp -f "$completions_dir/_wyrd-shell" "$HOME/.local/share/zsh/site-functions/_wyrd-shell" 2>/dev/null || true
            echo -e "  ${GREEN}✓${RESET} Zsh completion installed"
        fi

        # Fish
        mkdir -p "$HOME/.config/fish/completions"
        if [ -f "$completions_dir/wyrd-shell.fish" ]; then
            cp -f "$completions_dir/wyrd-shell.fish" "$HOME/.config/fish/completions/wyrd-shell.fish" 2>/dev/null || true
            echo -e "  ${GREEN}✓${RESET} Fish completion installed"
        fi
    fi
else
    echo "  [dry-run] install completions -> ~/.local/share/bash-completion, ~/.local/share/zsh/site-functions, ~/.config/fish/completions"
fi

# 12. Final instructions
echo ""
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
echo -e "${GREEN}${BOLD}       Wyrd Shell successfully installed and configured!${RESET}"
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
echo ""
echo -e "Binary:  ${CYAN}$BIN_DIR/wyrd-shell${RESET}"
echo -e "Config:  ${CYAN}$CONFIG_DIR/init.lua${RESET}"
echo ""

# Check PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo -e "${YELLOW}Note:${RESET} $BIN_DIR is not currently in your \$PATH."
    echo "Add the following line to your ~/.bashrc or ~/.zshrc:"
    echo -e "  ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}"
    echo ""
fi

echo -e "${BOLD}To start:${RESET}"
echo "  1) Automatically with your Wayland session (already enabled via systemd user services)"
echo "  2) Hyprland autostart (~/.config/hypr/hyprland.conf):"
echo "       exec-once = wyrd-shell"
echo "  3) Manually via systemd:"
echo "       systemctl --user restart wyrd-shell"
echo ""
echo -e "  ${GREEN}✓${RESET} Companion daemons are automatically bound to wyrd-shell and will start automatically on boot."
echo ""
