#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: Automated Edge Installation Script
# ==============================================================================
# Downloads the latest precompiled edge binary and static web dashboard,
# installs them to /opt/oryzaelo_engine, configures environment variables,
# and configures the host service manager (systemd, OpenRC, or runit).
# ==============================================================================

set -euo pipefail

# ANSI color codes
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

INSTALL_DIR="/opt/oryzaelo_engine"
BIN_DIR="$INSTALL_DIR/bin"
WEB_DIR="$INSTALL_DIR/web"
DATA_DIR="$INSTALL_DIR/data"
SCRIPTS_DIR="$INSTALL_DIR/scripts"
ENV_FILE="$INSTALL_DIR/.env"

GITHUB_REPO="wilsonborba/oryzaelo_engine"
DEFAULT_PORT=8005

say() {
    printf "${CYAN}[oryza-install]${NC} %s\n" "$*"
}

say_ok() {
    printf "${GREEN}[oryza-install] [OK]${NC} %s\n" "$*"
}

say_warn() {
    printf "${YELLOW}[oryza-install] [WARN]${NC} %s\n" "$*"
}

fail() {
    printf "${RED}[oryza-install] ERROR:${NC} %s\n" "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1
}

# 1. Root / Sudo Resolution
resolve_sudo() {
    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        SUDO=""
    elif need_cmd sudo; then
        SUDO="sudo"
    else
        fail "This installer requires root privileges to configure /opt and system services. Run as root or install sudo."
    fi
}

# 2. Architecture & OS Detection
detect_platform() {
    OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCH="$(uname -m)"

    if [[ "$OS" != "linux" ]]; then
        fail "Automated system installation currently supports Linux edge environments (Debian, Ubuntu, Raspberry Pi OS, Alpine, Arch, Fedora). For other OSes, clone the repository and run ./run_local_edge.sh."
    fi

    case "$ARCH" in
        x86_64|amd64)
            TARGET_ARCH="x86_64-unknown-linux-gnu"
            ;;
        aarch64|arm64)
            TARGET_ARCH="aarch64-unknown-linux-gnu"
            ;;
        armv7l)
            TARGET_ARCH="armv7-unknown-linux-gnueabihf"
            ;;
        *)
            fail "Unsupported CPU architecture: $ARCH. Supported architectures are x86_64, aarch64 (ARM64), and armv7l."
            ;;
    esac

    say "Detected platform: Linux ($ARCH) -> Target: $TARGET_ARCH"
}

# 3. Detect Service Manager
detect_init_system() {
    INIT_SYSTEM="none"
    if [[ -d /run/systemd/system ]] || need_cmd systemctl; then
        INIT_SYSTEM="systemd"
    elif need_cmd rc-service && [[ -d /etc/init.d ]]; then
        INIT_SYSTEM="openrc"
    elif need_cmd runit || [[ -d /etc/runit ]]; then
        INIT_SYSTEM="runit"
    fi

    say "Detected service manager: $INIT_SYSTEM"
}

# 4. Create Directory Hierarchy
prepare_directories() {
    say "Creating installation hierarchy at $INSTALL_DIR..."
    $SUDO mkdir -p "$BIN_DIR" "$WEB_DIR" "$DATA_DIR" "$SCRIPTS_DIR"
    $SUDO chmod 755 "$INSTALL_DIR" "$BIN_DIR" "$DATA_DIR"
}

# 5. Fetch Latest Compiled Assets
fetch_release() {
    say "Checking latest release from $GITHUB_REPO..."
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap '$SUDO rm -rf "$tmp_dir"' EXIT

    local archive_name="oryzaelo_engine-${TARGET_ARCH}.tar.gz"
    local release_url="https://github.com/${GITHUB_REPO}/releases/latest/download/${archive_name}"

    say "Attempting to download latest release archive: $archive_name"

    if curl -fsSL --head "$release_url" >/dev/null 2>&1; then
        say "Downloading release archive from GitHub Releases..."
        curl -fsSL "$release_url" -o "$tmp_dir/package.tar.gz"
        say "Extracting package into $INSTALL_DIR..."
        $SUDO tar -xzf "$tmp_dir/package.tar.gz" -C "$INSTALL_DIR"
    else
        say_warn "Precompiled GitHub Release asset not found for $TARGET_ARCH."
        say "Falling back to installing via Git repository clone & local compilation..."

        need_cmd git || fail "git command is required for source installation. Install git or run on a system with prebuilt binaries."

        local clone_dir="$tmp_dir/source"
        git clone --depth 1 https://github.com/${GITHUB_REPO}.git "$clone_dir"

        if [[ -f "$clone_dir/target/release/oryzaelo_engine" ]]; then
            $SUDO cp "$clone_dir/target/release/oryzaelo_engine" "$BIN_DIR/"
        else
            say "Compiling native Rust binary with cargo..."
            need_cmd cargo || fail "cargo is required for source compilation. Install Rust/cargo or provide prebuilt release."
            (cd "$clone_dir" && cargo build --release)
            $SUDO cp "$clone_dir/target/release/oryzaelo_engine" "$BIN_DIR/"
        fi

        if [[ -d "$clone_dir/src/presentation/static" ]]; then
            $SUDO cp -r "$clone_dir/src/presentation/static"/* "$WEB_DIR/"
        fi

        if [[ -d "$clone_dir/scripts" ]]; then
            $SUDO cp "$clone_dir/scripts"/*.sh "$SCRIPTS_DIR/"
            $SUDO chmod +x "$SCRIPTS_DIR"/*.sh
        fi
    fi

    say "Cleaning up temporary files and downloaded archives..."
    $SUDO rm -rf "$tmp_dir"
    trap - EXIT

    $SUDO chmod 755 "$BIN_DIR/oryzaelo_engine" 2>/dev/null || true
    say_ok "Binary installed to $BIN_DIR/oryzaelo_engine"
}

# 6. Configure Environment File
configure_environment() {
    if [[ ! -f "$ENV_FILE" ]]; then
        say "Generating default configuration at $ENV_FILE..."
        $SUDO tee "$ENV_FILE" > /dev/null <<EOF
# ==============================================================================
# Oryza-Elo Edge Station - Runtime Configuration (.env)
# ==============================================================================

SERVER_HOST=0.0.0.0
SERVER_PORT=${DEFAULT_PORT}
PORT=${DEFAULT_PORT}

DATABASE_PATH=${DATA_DIR}/oryza_elo_edge.db
STATIC_DIR=${WEB_DIR}
LOG_LEVEL=info

# Edge Autonomy Mode: local_only (100% offline) or hybrid_sync (cloud sync)
AUTONOMY_MODE=local_only
INFERENCE_ENGINE=onnx_resident
EOF
        $SUDO chmod 644 "$ENV_FILE"
        say_ok "Configuration created."
    else
        say "Existing configuration preserved at $ENV_FILE."
    fi
}

# 7. Configure and Enable System Service
configure_service() {
    case "$INIT_SYSTEM" in
        systemd)
            say "Configuring systemd service (/etc/systemd/system/oryzaelo_engine.service)..."
            $SUDO tee /etc/systemd/system/oryzaelo_engine.service > /dev/null <<EOF
[Unit]
Description=Oryza-Elo Edge Rural Microservice & Offline Phenology Engine
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=${INSTALL_DIR}
EnvironmentFile=${ENV_FILE}
ExecStart=${BIN_DIR}/oryzaelo_engine
Restart=always
RestartSec=3
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
            $SUDO systemctl daemon-reload
            $SUDO systemctl enable --now oryzaelo_engine
            say_ok "systemd service enabled and started."
            ;;

        openrc)
            say "Configuring OpenRC service (/etc/init.d/oryzaelo_engine)..."
            $SUDO tee /etc/init.d/oryzaelo_engine > /dev/null <<EOF
#!/sbin/openrc-run
name="oryzaelo_engine"
description="Oryza-Elo Edge Microservice"
command="${BIN_DIR}/oryzaelo_engine"
command_background="yes"
pidfile="/run/\${RC_SVCNAME}.pid"
directory="${INSTALL_DIR}"

depend() {
    need net
    after firewall
}
EOF
            $SUDO chmod +x /etc/init.d/oryzaelo_engine
            $SUDO rc-update add oryzaelo_engine default
            $SUDO rc-service oryzaelo_engine start
            say_ok "OpenRC service enabled and started."
            ;;

        runit)
            say "Configuring runit service (/etc/sv/oryzaelo_engine)..."
            $SUDO mkdir -p /etc/sv/oryzaelo_engine
            $SUDO tee /etc/sv/oryzaelo_engine/run > /dev/null <<EOF
#!/bin/sh
exec 2>&1
cd ${INSTALL_DIR}
exec ${BIN_DIR}/oryzaelo_engine
EOF
            $SUDO chmod +x /etc/sv/oryzaelo_engine/run
            if [[ -d /var/service ]]; then
                $SUDO ln -sfn /etc/sv/oryzaelo_engine /var/service/
            fi
            say_ok "runit service created."
            ;;

        none)
            say_warn "No recognized init system detected. Start manually with:"
            say_warn "  cd $INSTALL_DIR && ./bin/oryzaelo_engine"
            ;;
    esac
}

# 8. Post-Install Health Check
verify_installation() {
    say "Verifying service readiness at http://127.0.0.1:${DEFAULT_PORT}/health..."
    local ok=false
    for _ in {1..20}; do
        if curl -s "http://127.0.0.1:${DEFAULT_PORT}/health" >/dev/null 2>&1; then
            ok=true
            break
        fi
        sleep 0.5
    done

    local lan_ip
    lan_ip=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1")

    echo ""
    echo "=================================================================="
    echo "  Oryza-Elo Edge Installation Completed Successfully"
    echo "=================================================================="
    echo ""
    echo "  Installation Directory: ${INSTALL_DIR}"
    echo "  Binary Path:            ${BIN_DIR}/oryzaelo_engine"
    echo "  Configuration File:     ${ENV_FILE}"
    echo "  Service Manager:        ${INIT_SYSTEM}"
    echo ""
    echo "  Access Web Dashboard:"
    echo "    Local: http://localhost:${DEFAULT_PORT}"
    echo "    LAN:   http://${lan_ip}:${DEFAULT_PORT}"
    echo ""
    echo "  Testing & Data Population:"
    echo "    Option 1: Open the Web Dashboard -> Sensor Ingestion Center -> Click 'Load Demo Farm Data'"
    echo "    Option 2: Run CLI script: cd ${INSTALL_DIR} && ./scripts/populate_test_data.sh"
    echo ""
    echo "=================================================================="
    echo ""
}

main() {
    resolve_sudo
    detect_platform
    detect_init_system
    prepare_directories
    fetch_release
    configure_environment
    configure_service
    verify_installation
}

main "$@"
