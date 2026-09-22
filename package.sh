#!/usr/bin/env bash
# =============================================================================
# Wyrd Shell - Release Packaging Script
# =============================================================================
set -e

# ANSI Color codes
if [ -t 1 ]; then
    BOLD="\033[1m"
    GREEN="\033[1;32m"
    CYAN="\033[1;36m"
    YELLOW="\033[1;33m"
    RED="\033[1;31m"
    RESET="\033[0m"
else
    BOLD=""
    GREEN=""
    CYAN=""
    YELLOW=""
    RED=""
    RESET=""
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Extract version from Cargo.toml if not specified
DEFAULT_VERSION=$(grep -m1 '^version' Cargo.toml | awk -F '"' '{print $2}')
VERSION="${VERSION:-$DEFAULT_VERSION}"
OS="linux"
ARCH="$(uname -m)"
GLIBC="${GLIBC:-2.31}"
TARGET_TRIPLE="${ARCH}-unknown-linux-gnu"
USE_ZIGBUILD=true

discover_modules() {
    local dir
    local -a modules=()
    for dir in "$SCRIPT_DIR"/wyrd-modules/*; do
        [ -d "$dir" ] || continue
        if [ -f "$dir/manifest.toml" ] || [ -f "$dir/Cargo.toml" ]; then
            modules+=("$(basename "$dir")")
        fi
    done
    printf '%s\n' "${modules[@]}" | sort -u
}

discover_daemon_packages() {
    local dir
    local -a pkgs=()
    for dir in "$SCRIPT_DIR"/wyrd-*; do
        [ -d "$dir" ] || continue
        [ -f "$dir/Cargo.toml" ] || continue
        [ -f "$dir/src/main.rs" ] || continue
        local pkg_name
        pkg_name=$(grep -m1 '^name =' "$dir/Cargo.toml" | awk -F '"' '{print $2}')
        if [ -n "$pkg_name" ] && [ "$pkg_name" != "wyrd-shell" ]; then
            pkgs+=("$pkg_name")
        fi
    done
    printf '%s\n' "${pkgs[@]}" | sort -u
}

MODULES=($(discover_modules))
DAEMONS=($(discover_daemon_packages))

# Parse CLI options
SKIP_BUILD=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--version)
            VERSION="$2"
            shift 2
            ;;
        --version=*)
            VERSION="${1#*=}"
            shift
            ;;
        --glibc)
            GLIBC="$2"
            shift 2
            ;;
        --glibc=*)
            GLIBC="${1#*=}"
            shift
            ;;
        -a|--arch)
            ARCH="$2"
            shift 2
            ;;
        --arch=*)
            ARCH="${1#*=}"
            shift
            ;;
        --no-zigbuild)
            USE_ZIGBUILD=false
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        -h|--help)
            echo "Usage: ./package.sh [options]"
            echo "Options:"
            echo "  -v, --version <VER>   Set release version (default: $DEFAULT_VERSION)"
            echo "  -a, --arch <ARCH>     Set architecture (x86_64, aarch64, default: $(uname -m))"
            echo "      --glibc <VER>     Set glibc compatibility target (default: 2.31)"
            echo "      --no-zigbuild     Disable cargo-zigbuild; use host cargo build"
            echo "      --skip-build      Skip compiling binaries and reuse existing release artifacts"
            echo "  -h, --help            Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option:${RESET} $1"
            exit 1
            ;;
    esac
done

TARGET_TRIPLE="${ARCH}-unknown-linux-gnu"
PACKAGE_NAME="wyrd-shell-v${VERSION}-${OS}-${ARCH}"
DIST_DIR="$SCRIPT_DIR/dist"
STAGE_DIR="$DIST_DIR/$PACKAGE_NAME"
ARCHIVE_FILE="$DIST_DIR/${PACKAGE_NAME}.tar.gz"

echo -e "${CYAN}${BOLD}"
echo "================================================================="
echo "               Wyrd Shell Release Packager"
echo "================================================================="
echo -e "${RESET}"
echo -e "Version:       ${BOLD}v${VERSION}${RESET}"
echo -e "Target:        ${BOLD}${OS}-${ARCH}${RESET}"
echo -e "Target Glibc:  ${BOLD}${GLIBC}${RESET}"
echo -e "Package Name:  ${BOLD}${PACKAGE_NAME}${RESET}"
echo ""

# 1. Build release binaries
if [ "$SKIP_BUILD" = false ]; then
    # Verify cargo-zigbuild and zig availability
    if [ "$USE_ZIGBUILD" = true ]; then
        if ! command -v cargo-zigbuild &>/dev/null; then
            echo -e "${YELLOW}cargo-zigbuild not found in PATH.${RESET}"
            if command -v cargo &>/dev/null; then
                echo "Attempting to install cargo-zigbuild via cargo install..."
                cargo install cargo-zigbuild || {
                    echo -e "${YELLOW}Failed to install cargo-zigbuild. Falling back to standard cargo build.${RESET}"
                    USE_ZIGBUILD=false
                }
            else
                USE_ZIGBUILD=false
            fi
        fi
        if [ "$USE_ZIGBUILD" = true ] && ! command -v zig &>/dev/null; then
            echo -e "${YELLOW}zig compiler not found in PATH. Falling back to standard cargo build.${RESET}"
            echo -e "${DIM}Tip: Install zig from https://ziglang.org/download/ to enable cross-glibc targeting.${RESET}"
            USE_ZIGBUILD=false
        fi
    fi

    if [ "$USE_ZIGBUILD" = true ]; then
        BUILD_TARGET="${TARGET_TRIPLE}.${GLIBC}"
        echo -e "${CYAN}[1/5] Building shell and standalone daemons with cargo-zigbuild (${BUILD_TARGET})...${RESET}"
        if ! rustup target list --installed | grep -q "^${TARGET_TRIPLE}$"; then
            rustup target add "${TARGET_TRIPLE}" 2>/dev/null || true
        fi
        # Discover multiarch library search paths for zig linker
        ZIG_LINK_FLAGS=""
        for d in \
            "/usr/lib/$(uname -m)-linux-gnu" \
            "/usr/lib64" \
            "/usr/lib" \
            "/lib/$(uname -m)-linux-gnu" \
            "/lib64" \
            "/lib"; do
            if [ -d "$d" ]; then
                ZIG_LINK_FLAGS="$ZIG_LINK_FLAGS -C link-arg=-L$d"
            fi
        done
        if command -v pkg-config &>/dev/null; then
            for pc_lib in $(pkg-config --libs-only-L xkbcommon libpulse wayland-client fontconfig 2>/dev/null | tr ' ' '\n' | sed 's/^-L//'); do
                if [ -d "$pc_lib" ]; then
                    ZIG_LINK_FLAGS="$ZIG_LINK_FLAGS -C link-arg=-L$pc_lib"
                fi
            done
        fi
        RUSTFLAGS="$ZIG_LINK_FLAGS" cargo zigbuild --release --target "${BUILD_TARGET}" -p wyrd-shell
        for daemon in "${DAEMONS[@]}"; do
            printf "  Building %-20s ... " "$daemon"
            RUSTFLAGS="$ZIG_LINK_FLAGS" cargo zigbuild --release --target "${BUILD_TARGET}" -p "$daemon" --quiet
            echo -e "${GREEN}OK${RESET}"
        done
        BIN_SRC_DIR="$SCRIPT_DIR/target/${TARGET_TRIPLE}/release"
    else
        echo -e "${CYAN}[1/5] Building shell and standalone daemons with standard cargo build...${RESET}"
        cargo build --release -p wyrd-shell
        for daemon in "${DAEMONS[@]}"; do
            printf "  Building %-20s ... " "$daemon"
            cargo build --release -p "$daemon" --quiet
            echo -e "${GREEN}OK${RESET}"
        done
        BIN_SRC_DIR="$SCRIPT_DIR/target/release"
    fi

    echo -e "\n${CYAN}[2/5] Building ${#MODULES[@]} WASM modules (release mode)...${RESET}"
    if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
        echo "Adding wasm32-unknown-unknown target..."
        rustup target add wasm32-unknown-unknown
    fi

    for mod in "${MODULES[@]}"; do
        printf "  Building %-20s ... " "$mod"
        cargo build -p "wyrd-module-${mod}" --target wasm32-unknown-unknown --release --quiet
        echo -e "${GREEN}OK${RESET}"
    done
else
    echo -e "${YELLOW}[1/5 & 2/5] Skipping build (--skip-build specified).${RESET}"
    if [ -d "$SCRIPT_DIR/target/${TARGET_TRIPLE}/release" ]; then
        BIN_SRC_DIR="$SCRIPT_DIR/target/${TARGET_TRIPLE}/release"
    else
        BIN_SRC_DIR="$SCRIPT_DIR/target/release"
    fi
fi

# 2. Prepare staging directory
echo -e "\n${CYAN}[3/5] Assembling release directory structure...${RESET}"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/bin"
mkdir -p "$STAGE_DIR/modules"
mkdir -p "$STAGE_DIR/themes/aetheria"
mkdir -p "$STAGE_DIR/surfaces"
mkdir -p "$STAGE_DIR/systemd"

# Copy and strip shell plus discovered standalone daemons.
BINARIES=("wyrd-shell" "${DAEMONS[@]}")
for binary in "${BINARIES[@]}"; do
    source_binary="$BIN_SRC_DIR/$binary"
    if [ ! -f "$source_binary" ] && [ -f "$SCRIPT_DIR/target/release/$binary" ]; then
        source_binary="$SCRIPT_DIR/target/release/$binary"
    fi
    if [ ! -f "$source_binary" ]; then
        echo -e "${YELLOW}Warning: release binary not found: $source_binary${RESET}"
        continue
    fi
    if command -v strip &>/dev/null; then
        strip -s "$source_binary" -o "$STAGE_DIR/bin/$binary" 2>/dev/null || cp -f "$source_binary" "$STAGE_DIR/bin/$binary"
    else
        cp -f "$source_binary" "$STAGE_DIR/bin/$binary"
    fi
    chmod +x "$STAGE_DIR/bin/$binary"
done

# Copy WASM modules and manifests
for mod in "${MODULES[@]}"; do
    mod_dir="$STAGE_DIR/modules/$mod"
    mkdir -p "$mod_dir"

    # Manifest
    if [ -f "$SCRIPT_DIR/wyrd-modules/$mod/manifest.toml" ]; then
        cp -f "$SCRIPT_DIR/wyrd-modules/$mod/manifest.toml" "$mod_dir/manifest.toml"
    fi

    # WASM binary
    wasm_src="$SCRIPT_DIR/target/wasm32-unknown-unknown/release/wyrd_module_${mod//-/_}.wasm"
    if [ -f "$wasm_src" ]; then
        cp -f "$wasm_src" "$mod_dir/wyrd_module_${mod//-/_}.wasm"
        cp -f "$wasm_src" "$mod_dir/${mod}.wasm"
        cp -f "$wasm_src" "$STAGE_DIR/modules/wyrd_module_${mod//-/_}.wasm"
    else
        echo -e "${YELLOW}Warning: wasm binary for $mod not found at $wasm_src${RESET}"
    fi

    # Precompiled AOT cwasm binary
    cwasm_src="$SCRIPT_DIR/target/wasm32-unknown-unknown/release/wyrd_module_${mod//-/_}.cwasm"
    if [ -f "$cwasm_src" ]; then
        cp -f "$cwasm_src" "$mod_dir/wyrd_module_${mod//-/_}.cwasm"
        cp -f "$cwasm_src" "$mod_dir/${mod}.cwasm"
    fi
done

# Copy themes
mkdir -p "$STAGE_DIR/themes"
cp -rf "$SCRIPT_DIR/examples/config/themes/"* "$STAGE_DIR/themes/"

# Copy surfaces
mkdir -p "$STAGE_DIR/surfaces"
cp -rf "$SCRIPT_DIR/examples/config/surfaces/"* "$STAGE_DIR/surfaces/"

# Copy settings and init templates
cp -f "$SCRIPT_DIR/examples/config/init.lua" "$STAGE_DIR/init.lua"
cp -f "$SCRIPT_DIR/examples/config/settings.lua" "$STAGE_DIR/settings.lua"
cp -f "$SCRIPT_DIR/examples/config/settings.toml" "$STAGE_DIR/settings.toml"

# Copy shell completions
mkdir -p "$STAGE_DIR/completions"
cp -rf "$SCRIPT_DIR/completions/"* "$STAGE_DIR/completions/"

# Copy systemd services from the project directory without hardcoding the service list.
for service in "$SCRIPT_DIR"/systemd/*.service; do
    [ -f "$service" ] || continue
    cp -f "$service" "$STAGE_DIR/systemd/"
done

# Copy installer script and docs
cp -f "$SCRIPT_DIR/install.sh" "$STAGE_DIR/install.sh"
chmod +x "$STAGE_DIR/install.sh"
if [ -f "$SCRIPT_DIR/uninstall.sh" ]; then
    cp -f "$SCRIPT_DIR/uninstall.sh" "$STAGE_DIR/uninstall.sh"
    chmod +x "$STAGE_DIR/uninstall.sh"
fi

if [ -f "$SCRIPT_DIR/README.md" ]; then
    cp -f "$SCRIPT_DIR/README.md" "$STAGE_DIR/README.md"
fi
if [ -f "$SCRIPT_DIR/LICENSE" ]; then
    cp -f "$SCRIPT_DIR/LICENSE" "$STAGE_DIR/LICENSE"
fi
if [ -f "$SCRIPT_DIR/CHANGELOG.md" ]; then
    cp -f "$SCRIPT_DIR/CHANGELOG.md" "$STAGE_DIR/CHANGELOG.md"
fi

# 3. Create tarball
echo -e "\n${CYAN}[4/5] Creating compressed release archive (.tar.gz)...${RESET}"
rm -f "$ARCHIVE_FILE"
tar -czf "$ARCHIVE_FILE" -C "$DIST_DIR" "$PACKAGE_NAME"

# 4. Generate SHA256 checksum
echo -e "\n${CYAN}[5/5] Generating SHA256 checksum...${RESET}"
cd "$DIST_DIR"
sha256sum "${PACKAGE_NAME}.tar.gz" > "${PACKAGE_NAME}.tar.gz.sha256"

ARCHIVE_SIZE=$(du -h "${PACKAGE_NAME}.tar.gz" | cut -f1)
CHECKSUM=$(awk '{print $1}' "${PACKAGE_NAME}.tar.gz.sha256")

if [ -f "$SCRIPT_DIR/packaging/aur/PKGBUILD" ] && [ "$ARCH" = "x86_64" ]; then
    sed -i "s|sha256sums=('.*')|sha256sums=('${CHECKSUM}')|g" "$SCRIPT_DIR/packaging/aur/PKGBUILD"
fi

echo ""
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
echo -e "${GREEN}${BOLD}         Release Archive Created Successfully!${RESET}"
echo -e "${GREEN}${BOLD}=================================================================${RESET}"
MAX_GLIBC=$(objdump -p "$STAGE_DIR/bin/wyrd-shell" 2>/dev/null | grep -A 25 "required from libc.so.6:" | grep "GLIBC_" | awk '{print $NF}' | sort -V | tail -n 1)
echo -e "Archive:       ${BOLD}${DIST_DIR}/${PACKAGE_NAME}.tar.gz${RESET} (${CYAN}${ARCHIVE_SIZE}${RESET})"
echo -e "Checksum:      ${DIM}${CHECKSUM}${RESET}"
echo -e "SHA File:      ${BOLD}${DIST_DIR}/${PACKAGE_NAME}.tar.gz.sha256${RESET}"
echo -e "Compatibility: ${GREEN}glibc >= ${MAX_GLIBC:-2.31} (Ubuntu 20.04+, Debian 11+, Fedora, Arch)${RESET}"
echo ""
echo "Contents:"
tar -tf "${PACKAGE_NAME}.tar.gz" | head -n 25
echo "  ... [and more]"
echo ""
echo -e "${BOLD}Ready for upload to GitHub Releases:${RESET}"
echo "  gh release create v${VERSION} dist/${PACKAGE_NAME}.tar.gz dist/${PACKAGE_NAME}.tar.gz.sha256 --title \"Wyrd Shell v${VERSION}\""
echo ""
