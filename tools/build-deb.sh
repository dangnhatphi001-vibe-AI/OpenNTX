#!/usr/bin/env bash
# tools/build-deb.sh — Automated .deb package builder for OpenNTX.
#
# Orchestrates the full packaging pipeline:
#   1. Validates the environment (cargo, dpkg-deb).
#   2. Runs cargo build --release for all workspace binaries.
#   3. Assembles the Debian package layout under target/debian/.
#   4. Generates DEBIAN/control, postinst, prerm.
#   5. Installs default sandbox.toml configuration.
#   6. Invokes dpkg-deb --build to produce the final .deb artifact.
#
# Usage:
#   ./tools/build-deb.sh [--version 2.8.0] [--output target/debian] [--dry-run] [--skip-build]
#
# Requirements:
#   - Rust toolchain (cargo, rustc)
#   - dpkg-dev (provides dpkg-deb)
#   - Standard Unix utilities (chmod, mkdir, cp, tar)

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PACKAGE_NAME="openntx"
VERSION="2.8.0"
ARCH="amd64"
OUTPUT_DIR="${WORKSPACE_ROOT}/target/debian"
DRY_RUN=false
SKIP_BUILD=false
VERBOSE=false

# ── Argument Parsing ─────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build the OpenNTX system .deb package.

Options:
  --version VERSION    Package version (default: ${VERSION})
  --output DIR         Output directory for .deb (default: ${OUTPUT_DIR})
  --dry-run            Generate staging layout without calling dpkg-deb
  --skip-build         Skip cargo build (use existing binaries)
  --verbose            Show detailed output
  -h, --help           Show this help message

Examples:
  ./tools/build-deb.sh
  ./tools/build-deb.sh --version 2.8.0-rc1 --output /tmp/debs
  ./tools/build-deb.sh --dry-run --verbose
  ./tools/build-deb.sh --skip-build
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Error: unknown option '$1'" >&2
            usage >&2
            exit 1
            ;;
    esac
done

DEB_FILENAME="${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
STAGING_DIR="${WORKSPACE_ROOT}/target/openntx-deb-staging-${VERSION}"
RELEASE_DIR="${WORKSPACE_ROOT}/target/release"

# ── Logging ──────────────────────────────────────────────────────────────────

log_info() {
    echo "[INFO]  $*"
}

log_error() {
    echo "[ERROR] $*" >&2
}

log_verbose() {
    if [[ "$VERBOSE" == "true" ]]; then
        echo "[DEBUG] $*"
    fi
}

log_step() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  $*"
    echo "═══════════════════════════════════════════════════════════════"
}

# ── Environment Validation ───────────────────────────────────────────────────

validate_environment() {
    log_step "Step 1/9: Validating build environment"

    # Check cargo
    if ! command -v cargo >/dev/null 2>&1; then
        if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
            export PATH="$HOME/.cargo/bin:$PATH"
        else
            log_error "cargo is not installed. Install Rust from https://rustup.rs/"
            exit 1
        fi
    fi
    log_info "cargo: $(cargo --version)"

    # Check rustc
    if ! command -v rustc >/dev/null 2>&1; then
        log_error "rustc is not installed."
        exit 1
    fi
    log_info "rustc: $(rustc --version)"

    # Check dpkg-deb (only if not dry-run)
    if [[ "$DRY_RUN" == "false" ]]; then
        if ! command -v dpkg-deb >/dev/null 2>&1; then
            log_error "dpkg-deb is not installed. Install dpkg-dev: sudo apt install dpkg-dev"
            exit 1
        fi
        log_info "dpkg-deb: $(dpkg-deb --version 2>&1 | head -1)"
    fi

    # Verify workspace root
    if [[ ! -f "$WORKSPACE_ROOT/Cargo.toml" ]]; then
        log_error "Cargo.toml not found at workspace root: $WORKSPACE_ROOT"
        exit 1
    fi
    log_info "workspace: $WORKSPACE_ROOT"
    log_info "version:   $VERSION"
    log_info "output:    $OUTPUT_DIR"
}

# ── Build Release Binaries ───────────────────────────────────────────────────

build_release() {
    log_step "Step 2/9: Building release binaries"

    if [[ "$SKIP_BUILD" == "true" ]]; then
        log_info "Skipping cargo build (--skip-build)"
        validate_binaries
        return
    fi

    log_info "Running: cargo build --release"
    cd "$WORKSPACE_ROOT"

    if [[ "$VERBOSE" == "true" ]]; then
        cargo build --release
    else
        cargo build --release 2>&1 | tail -5
    fi

    if [[ "${PIPESTATUS[0]:-0}" -ne 0 ]] && [[ "$VERBOSE" != "true" ]]; then
        log_error "cargo build --release failed. Run with --verbose for details."
        exit 1
    fi

    log_info "Build complete."
    validate_binaries
}

# ── Validate Binaries ────────────────────────────────────────────────────────

validate_binaries() {
    log_step "Step 3/9: Validating release binaries"

    local bins=("openntx" "openntx-gui" "openntx-appportal")

    for bin in "${bins[@]}"; do
        local bin_path="$RELEASE_DIR/$bin"
        if [[ ! -f "$bin_path" ]]; then
            log_error "Binary not found: $bin_path"
            log_error "Run without --skip-build or run: cargo build --release"
            exit 1
        fi
        local size
        size=$(stat -c%s "$bin_path" 2>/dev/null || stat -f%z "$bin_path" 2>/dev/null || echo 0)
        if [[ "$size" -eq 0 ]]; then
            log_error "Binary is empty (0 bytes): $bin_path"
            exit 1
        fi
        log_verbose "$bin: $(numfmt --to=iec "$size" 2>/dev/null || echo "${size} bytes")"
    done

    log_info "All binaries validated."
}

# ── Prepare Staging Directory ────────────────────────────────────────────────

prepare_staging() {
    log_step "Step 4/9: Preparing staging directory"

    # Clean previous staging
    if [[ -d "$STAGING_DIR" ]]; then
        rm -rf "$STAGING_DIR"
        log_verbose "Removed previous staging: $STAGING_DIR"
    fi

    # Create directory skeleton
    mkdir -p "$STAGING_DIR/DEBIAN"
    mkdir -p "$STAGING_DIR/usr/bin"
    mkdir -p "$STAGING_DIR/etc/openntx"
    mkdir -p "$STAGING_DIR/var/lib/openntx/sandboxes"

    log_info "Staging directory created: $STAGING_DIR"
}

# ── Copy Binaries ────────────────────────────────────────────────────────────

copy_binaries() {
    log_step "Step 5/9: Copying binaries to staging"

    local bins=("openntx" "openntx-gui" "openntx-appportal")

    for bin in "${bins[@]}"; do
        local src="$RELEASE_DIR/$bin"
        local dst="$STAGING_DIR/usr/bin/$bin"

        cp "$src" "$dst"
        chmod 755 "$dst"

        log_verbose "Installed: usr/bin/$bin"
    done

    log_info "Binaries installed."
}

# ── Generate DEBIAN/control ──────────────────────────────────────────────────

generate_control() {
    log_step "Step 6/9: Generating DEBIAN/control"

    local control_file="$STAGING_DIR/DEBIAN/control"

    cat > "$control_file" <<EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: misc
Priority: optional
Architecture: ${ARCH}
Depends: libc6 (>= 2.31), libx11-6, libgcc-s1 (>= 3.0), libstdc++6 (>= 11)
Recommends: wine, xdg-utils
Installed-Size: 40960
Maintainer: OpenNTX Team <maintainer@openntx.org>
Homepage: https://github.com/openntx/openntx
Description: OpenNTX - Windows Application Subsystem for Linux
 Provides the core runtime, GUI frontend, CLI tools, and sandbox
 infrastructure for running Windows applications on Linux.
 .
 OpenNTX enables seamless execution of Windows .exe applications
 on Linux through cgroups v2 sandboxing, binfmt_misc integration,
 and a managed Wine/Proton runtime layer.
EOF

    log_info "DEBIAN/control generated."
    log_verbose "Content:"
    log_verbose "$(cat "$control_file")"
}

# ── Generate DEBIAN/postinst ─────────────────────────────────────────────────

generate_postinst() {
    log_step "Step 7/9: Generating DEBIAN/postinst"

    local postinst_file="$STAGING_DIR/DEBIAN/postinst"

    cat > "$postinst_file" <<'POSTINST'
#!/bin/bash
# openntx postinst — Post-installation script for the openntx Debian package.
# Generated by openntx-core packaging engine. Do not edit manually.
set -e

OPENNTX_USER="openntx"
OPENNTX_GROUP="openntx"
SANDBOX_ROOT="/var/lib/openntx/sandboxes"
CONFIG_DIR="/etc/openntx"
DATA_DIR="/var/lib/openntx"
LOG_DIR="/var/log/openntx"
BINFMT_NAME="OpenNTX"
BINFMT_REGISTER="/proc/sys/fs/binfmt_misc/register"
RUNTIME_BIN="/usr/bin/openntx"

log_action() {
    echo "openntx.postinst: $1"
}

create_system_user() {
    if ! getent group "$OPENNTX_GROUP" >/dev/null 2>&1; then
        groupadd --system "$OPENNTX_GROUP"
        log_action "created system group: $OPENNTX_GROUP"
    fi

    if ! getent passwd "$OPENNTX_USER" >/dev/null 2>&1; then
        useradd --system \
            --gid "$OPENNTX_GROUP" \
            --home-dir "$DATA_DIR" \
            --shell /usr/sbin/nologin \
            --comment "OpenNTX daemon user" \
            "$OPENNTX_USER"
        log_action "created system user: $OPENNTX_USER"
    fi
}

create_directories() {
    mkdir -p "$DATA_DIR"
    mkdir -p "$SANDBOX_ROOT"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$LOG_DIR"
    log_action "directory hierarchy created"
}

set_permissions() {
    chown -R "$OPENNTX_USER:$OPENNTX_GROUP" "$DATA_DIR"
    chmod 755 "$DATA_DIR"
    chmod 755 "$SANDBOX_ROOT"
    chmod 755 "$LOG_DIR"
    chown root:root "$CONFIG_DIR"
    chmod 755 "$CONFIG_DIR"
    chown "$OPENNTX_USER:$OPENNTX_GROUP" "$SANDBOX_ROOT"
    chmod 770 "$SANDBOX_ROOT"
    log_action "permissions configured"
}

setup_binfmt() {
    if [ ! -d /proc/sys/fs/binfmt_misc ]; then
        if modprobe binfmt_misc 2>/dev/null; then
            mount -t binfmt_misc binfmt_misc /proc/sys/fs/binfmt_misc 2>/dev/null || true
        fi
    fi

    if [ ! -f "$BINFMT_REGISTER" ]; then
        log_action "binfmt_misc not available — skipping PE registration"
        return 0
    fi

    local unregister_path="/proc/sys/fs/binfmt_misc/$BINFMT_NAME"
    if [ -f "$unregister_path" ]; then
        echo -1 > "$unregister_path" 2>/dev/null || true
        log_action "removed previous binfmt entry"
    fi

    local reg_string=":${BINFMT_NAME}:M::MZ::${RUNTIME_BIN}:OC"

    if echo "$reg_string" > "$BINFMT_REGISTER" 2>/dev/null; then
        log_action "registered PE (.exe) binfmt_misc handler -> $RUNTIME_BIN"
    else
        log_action "WARNING: binfmt_misc registration failed (non-fatal)"
    fi
}

setup_cgroup_hierarchy() {
    local cgroup_root="/sys/fs/cgroup/openntx"

    if [ -d /sys/fs/cgroup ]; then
        mkdir -p "$cgroup_root" 2>/dev/null || true
        if [ -f /sys/fs/cgroup/cgroup.subtree_control ]; then
            echo "+cpu +memory" > /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null || true
        fi
        chown "$OPENNTX_USER:$OPENNTX_GROUP" "$cgroup_root" 2>/dev/null || true
        chmod 770 "$cgroup_root" 2>/dev/null || true
        log_action "cgroup v2 hierarchy initialized at $cgroup_root"
    else
        log_action "cgroup filesystem not found — skipping hierarchy setup"
    fi
}

case "$1" in
    configure)
        create_system_user
        create_directories
        set_permissions
        setup_binfmt
        setup_cgroup_hierarchy
        log_action "installation complete"
        ;;
    abort-upgrade|abort-remove|abort-deconfigure)
        ;;
    *)
        echo "openntx.postinst: unknown argument '$1'" >&2
        exit 1
        ;;
esac

exit 0
POSTINST

    chmod 755 "$postinst_file"
    log_info "DEBIAN/postinst generated."
}

# ── Generate DEBIAN/prerm ────────────────────────────────────────────────────

generate_prerm() {
    log_step "Step 8/9: Generating DEBIAN/prerm"

    local prerm_file="$STAGING_DIR/DEBIAN/prerm"

    cat > "$prerm_file" <<'PRERM'
#!/bin/bash
# openntx prerm — Pre-removal script for the openntx Debian package.
# Generated by openntx-core packaging engine. Do not edit manually.
set -e

OPENNTX_USER="openntx"
SANDBOX_ROOT="/var/lib/openntx/sandboxes"
CGROUP_ROOT="/sys/fs/cgroup/openntx"
BINFMT_NAME="OpenNTX"

log_action() {
    echo "openntx.prerm: $1"
}

reap_all_processes() {
    log_action "invoking Reaper Engine to terminate managed processes"

    if [ ! -d "$CGROUP_ROOT" ]; then
        log_action "no cgroup hierarchy found — nothing to reap"
        return 0
    fi

    local killed_total=0

    for app_cgroup in "$CGROUP_ROOT"/*/; do
        [ -d "$app_cgroup" ] || continue

        local procs_file="${app_cgroup}cgroup.procs"
        if [ ! -f "$procs_file" ]; then
            continue
        fi

        local app_name
        app_name=$(basename "$app_cgroup")
        local pids
        pids=$(cat "$procs_file" 2>/dev/null || true)

        if [ -z "$pids" ]; then
            continue
        fi

        log_action "reaping app: $app_name"

        for pid in $pids; do
            if [ "$pid" -gt 1 ] 2>/dev/null; then
                kill -TERM "$pid" 2>/dev/null || true
                killed_total=$((killed_total + 1))
            fi
        done

        sleep 0.5

        pids=$(cat "$procs_file" 2>/dev/null || true)
        for pid in $pids; do
            if [ "$pid" -gt 1 ] 2>/dev/null; then
                kill -KILL "$pid" 2>/dev/null || true
                log_action "force-killed survivor PID $pid in $app_name"
            fi
        done

        rmdir "$app_cgroup" 2>/dev/null || true
    done

    log_action "Reaper Engine complete: terminated $killed_total process(es)"
}

stop_daemons() {
    if command -v systemctl >/dev/null 2>&1; then
        if systemctl is-active --quiet openntx-core.service 2>/dev/null; then
            systemctl stop openntx-core.service 2>/dev/null || true
            log_action "stopped openntx-core.service"
        fi
    fi

    local pids
    pids=$(pgrep -u "$OPENNTX_USER" -f "openntx-core" 2>/dev/null || true)
    if [ -n "$pids" ]; then
        echo "$pids" | xargs kill -TERM 2>/dev/null || true
        sleep 0.5
        echo "$pids" | xargs kill -KILL 2>/dev/null || true
        log_action "killed residual openntx-core processes"
    fi
}

unregister_binfmt() {
    local unregister_path="/proc/sys/fs/binfmt_misc/$BINFMT_NAME"

    if [ -f "$unregister_path" ]; then
        echo -1 > "$unregister_path" 2>/dev/null || true
        log_action "unregistered binfmt_misc handler: $BINFMT_NAME"
    fi
}

cleanup_cgroups() {
    if [ -d "$CGROUP_ROOT" ]; then
        find "$CGROUP_ROOT" -mindepth 1 -type d -empty -delete 2>/dev/null || true
        rmdir "$CGROUP_ROOT" 2>/dev/null || true
        log_action "cgroup hierarchy cleaned"
    fi
}

case "$1" in
    remove|upgrade|deconfigure)
        reap_all_processes
        stop_daemons
        unregister_binfmt
        cleanup_cgroups
        log_action "pre-removal cleanup complete"
        ;;
    failed-upgrade)
        ;;
    *)
        echo "openntx.prerm: unknown argument '$1'" >&2
        exit 1
        ;;
esac

exit 0
PRERM

    chmod 755 "$prerm_file"
    log_info "DEBIAN/prerm generated."
}

# ── Install Default Configuration ────────────────────────────────────────────

install_config() {
    log_info "Installing default sandbox.toml"

    local config_src="$WORKSPACE_ROOT/etc/openntx/sandbox.toml"
    local config_dst="$STAGING_DIR/etc/openntx/sandbox.toml"

    if [[ -f "$config_src" ]]; then
        cp "$config_src" "$config_dst"
    else
        log_error "Default sandbox.toml not found at: $config_src"
        exit 1
    fi

    chmod 644 "$config_dst"
    log_info "Configuration installed."
}

# ── Normalize Permissions ────────────────────────────────────────────────────

normalize_permissions() {
    log_info "Normalizing filesystem permissions"

    # Directories: 0755
    find "$STAGING_DIR" -type d -exec chmod 755 {} +

    # Regular files: 0644 (binaries already set to 0755)
    find "$STAGING_DIR" -type f ! -perm /111 -exec chmod 644 {} +

    # Preserve executable for binaries and scripts
    chmod 755 "$STAGING_DIR/usr/bin/"*
    chmod 755 "$STAGING_DIR/DEBIAN/postinst"
    chmod 755 "$STAGING_DIR/DEBIAN/prerm"

    log_info "Permissions normalized."
}

# ── Build .deb ───────────────────────────────────────────────────────────────

build_deb() {
    log_step "Step 9/9: Building .deb package"

    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "DRY RUN — Staging layout:"
        if command -v tree >/dev/null 2>&1; then
            tree "$STAGING_DIR"
        else
            find "$STAGING_DIR" -type f | sort | sed "s|$STAGING_DIR/||"
        fi
        log_info "Dry run complete. No .deb was built."
        return
    fi

    mkdir -p "$OUTPUT_DIR"

    local output_deb="$OUTPUT_DIR/$DEB_FILENAME"

    log_info "Running: dpkg-deb --root-owner-group --build $STAGING_DIR $output_deb"

    dpkg-deb --root-owner-group --build "$STAGING_DIR" "$output_deb"

    if [[ ! -f "$output_deb" ]]; then
        log_error "dpkg-deb did not produce output file: $output_deb"
        exit 1
    fi

    local deb_size
    deb_size=$(stat -c%s "$output_deb" 2>/dev/null || stat -f%z "$output_deb" 2>/dev/null || echo 0)

    log_info "Package built successfully!"
    log_info "  File:     $output_deb"
    log_info "  Size:     $(numfmt --to=iec "$deb_size" 2>/dev/null || echo "${deb_size} bytes")"
    log_info "  Package:  $PACKAGE_NAME"
    log_info "  Version:  $VERSION"
    log_info "  Arch:     $ARCH"

    # Clean up staging
    rm -rf "$STAGING_DIR"
    log_verbose "Staging directory cleaned up."
}

# ── Verify .deb ──────────────────────────────────────────────────────────────

verify_deb() {
    if [[ "$DRY_RUN" == "true" ]]; then
        return
    fi

    local output_deb="$OUTPUT_DIR/$DEB_FILENAME"

    log_info "Verifying .deb contents:"
    dpkg-deb --contents "$output_deb" 2>/dev/null || true

    echo ""
    log_info "Control information:"
    dpkg-deb --info "$output_deb" 2>/dev/null || true
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
    echo ""
    echo "┌─────────────────────────────────────────────────────────────┐"
    echo "│  OpenNTX .deb Package Builder v${VERSION}                   │"
    echo "│  Automated Debian Package Generation Engine                 │"
    echo "└─────────────────────────────────────────────────────────────┘"
    echo ""

    validate_environment
    build_release
    prepare_staging
    copy_binaries
    generate_control
    generate_postinst
    generate_prerm
    install_config
    normalize_permissions
    build_deb
    verify_deb

    echo ""
    log_info "Done."
}

main "$@"
