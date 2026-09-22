#!/usr/bin/env bash
# =============================================================================
# Wyrd Shell - Uninstallation Script
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

# Defaults
DEFAULT_BIN_DIR="$HOME/.local/bin"
DEFAULT_CONFIG_DIR="$HOME/.config/wyrd"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"

BIN_DIR="$DEFAULT_BIN_DIR"
CONFIG_DIR="$DEFAULT_CONFIG_DIR"
NON_INTERACTIVE=false
DRY_RUN=false
PURGE_CONFIG=false

# Known Wyrd binaries to clean up
WYRD_BINARIES=(
    "wyrd-shell"
    "wyrd-audio"
    "wyrd-clipboard"
    "wyrd-notifications"
    "wyrd-tray"
    "wyrd-windows"
    "wyrd-window"
    "wyrd-wallpaper"
    "wyrd-idle"
    "wyrd-screenshot"
    "wyrd-greet"
)

print_usage() {
    echo -e "${BOLD}Wyrd Shell Uninstaller${RESET}

Usage:
  ./uninstall.sh [options]

Options:
      --bin-dir <DIR>       Directory where binaries were installed (default: ~/.local/bin)
      --config-dir <DIR>    Configuration directory (default: ~/.config/wyrd)
      --purge               Also remove configuration files (~/.config/wyrd)
  -y, --yes, --non-interactive  Run non-interactively without confirmation prompts
      --dry-run             Show planned actions without deleting files
  -h, --help                Show this help message
"
}

# Parse CLI arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
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
        --purge)
            PURGE_CONFIG=true
            shift
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

echo -e "${BOLD}=================================================================${RESET}"
echo -e "${BOLD}                   Wyrd Shell Uninstaller                        ${RESET}"
echo -e "${BOLD}=================================================================${RESET}"

if [ "$DRY_RUN" = true ]; then
    echo -e "${YELLOW}[DRY-RUN] No actions will be performed.${RESET}\n"
fi

# Interactive confirmation
if [ "$NON_INTERACTIVE" = false ] && [ "$DRY_RUN" = false ]; then
    echo -e "This will stop all Wyrd services and remove installed binaries from:"
    echo -e "  Binaries:      ${CYAN}$BIN_DIR${RESET}"
    echo -e "  Systemd units: ${CYAN}$SYSTEMD_USER_DIR/wyrd-*.service${RESET}"
    echo -e "  Completions:   ${CYAN}bash, zsh, fish completions${RESET}"
    if [ "$PURGE_CONFIG" = true ]; then
        echo -e "  Config files:  ${RED}$CONFIG_DIR (PURGE)${RESET}"
    else
        echo -e "  Config files:  ${DIM}$CONFIG_DIR (kept by default, use --purge to remove)${RESET}"
    fi
    echo ""
    read -rp "Proceed with uninstallation? [y/N]: " CONFIRM
    if [[ ! "$CONFIRM" =~ ^[Yy] ]]; then
        echo "Uninstallation cancelled."
        exit 0
    fi
fi

# 1. Stop and disable systemd user services
echo -e "\n${CYAN}[1/5] Stopping and disabling systemd user services...${RESET}"
if command -v systemctl &>/dev/null; then
    services=()
    for unit in "$SYSTEMD_USER_DIR"/wyrd-*.service; do
        [ -f "$unit" ] || continue
        services+=("$(basename "$unit")")
    done

    if [ "${#services[@]}" -gt 0 ]; then
        if [ "$DRY_RUN" = false ]; then
            echo -e "  Stopping services: ${services[*]}"
            systemctl --user stop "${services[@]}" 2>/dev/null || true
            systemctl --user disable "${services[@]}" 2>/dev/null || true
            for unit in "$SYSTEMD_USER_DIR"/wyrd-*.service; do
                [ -f "$unit" ] || continue
                rm -f "$unit"
                echo -e "  ${GREEN}✓${RESET} Removed unit: $unit"
            done
            systemctl --user daemon-reload || true
            systemctl --user reset-failed 2>/dev/null || true
        else
            echo "  [dry-run] systemctl --user stop ${services[*]}"
            echo "  [dry-run] systemctl --user disable ${services[*]}"
            echo "  [dry-run] rm $SYSTEMD_USER_DIR/wyrd-*.service"
        fi
    else
        echo -e "  ${DIM}No active Wyrd systemd services found in $SYSTEMD_USER_DIR.${RESET}"
    fi
fi

# 2. Terminate any running Wyrd processes
echo -e "\n${CYAN}[2/5] Stopping any remaining Wyrd processes...${RESET}"
if [ "$DRY_RUN" = false ]; then
    for bin in "${WYRD_BINARIES[@]}"; do
        pkill -u "$UID" -x "$bin" 2>/dev/null || true
    done
    echo -e "  ${GREEN}✓${RESET} All Wyrd processes stopped"
else
    echo "  [dry-run] pkill processes: ${WYRD_BINARIES[*]}"
fi

# 3. Remove installed binaries
echo -e "\n${CYAN}[3/5] Removing installed binaries from $BIN_DIR...${RESET}"
removed_any=false
for bin in "${WYRD_BINARIES[@]}"; do
    target="$BIN_DIR/$bin"
    if [ -f "$target" ]; then
        if [ "$DRY_RUN" = false ]; then
            rm -f "$target"
            echo -e "  ${GREEN}✓${RESET} Removed: $target"
        else
            echo "  [dry-run] rm $target"
        fi
        removed_any=true
    fi
done

if [ "$removed_any" = false ]; then
    echo -e "  ${DIM}No Wyrd binaries found in $BIN_DIR.${RESET}"
fi

# 4. Remove shell completions
echo -e "\n${CYAN}[4/5] Removing shell completions...${RESET}"
COMPLETION_FILES=(
    "$HOME/.local/share/bash-completion/completions/wyrd-shell"
    "$HOME/.local/share/zsh/site-functions/_wyrd-shell"
    "$HOME/.config/fish/completions/wyrd-shell.fish"
)
removed_completion=false
for comp in "${COMPLETION_FILES[@]}"; do
    if [ -f "$comp" ]; then
        if [ "$DRY_RUN" = false ]; then
            rm -f "$comp"
            echo -e "  ${GREEN}✓${RESET} Removed: $comp"
        else
            echo "  [dry-run] rm $comp"
        fi
        removed_completion=true
    fi
done
if [ "$removed_completion" = false ]; then
    echo -e "  ${DIM}No Wyrd shell completions found.${RESET}"
fi

# 5. Handle configuration directory
echo -e "\n${CYAN}[5/5] Configuration files ($CONFIG_DIR)...${RESET}"
if [ -d "$CONFIG_DIR" ]; then
    if [ "$PURGE_CONFIG" = true ]; then
        if [ "$DRY_RUN" = false ]; then
            rm -rf "$CONFIG_DIR"
            echo -e "  ${GREEN}✓${RESET} Removed configuration directory: $CONFIG_DIR"
        else
            echo "  [dry-run] rm -rf $CONFIG_DIR"
        fi
    else
        echo -e "  ${YELLOW}Notice:${RESET} Configuration kept at $CONFIG_DIR."
        echo -e "  To remove it manually: ${DIM}rm -rf $CONFIG_DIR${RESET}"
        echo -e "  Or rerun uninstaller with: ${DIM}./uninstall.sh --purge${RESET}"
    fi
else
    echo -e "  ${DIM}No configuration directory found at $CONFIG_DIR.${RESET}"
fi

# Clean up socket if lingering
if [ -n "$XDG_RUNTIME_DIR" ] && [ -S "$XDG_RUNTIME_DIR/wyrd-shell.sock" ]; then
    rm -f "$XDG_RUNTIME_DIR/wyrd-shell.sock" 2>/dev/null || true
fi

echo ""
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
echo -e "${GREEN}${BOLD}             Wyrd Shell uninstalled successfully!                ${RESET}"
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
