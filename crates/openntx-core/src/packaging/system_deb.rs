// openntx-core/src/packaging/system_deb.rs — System-level .deb packaging engine.
//
// Builds a complete OpenNTX platform .deb package containing:
//   - /usr/bin/openntx-core          (backend daemon binary)
//   - /usr/bin/openntx-gui           (Slint UI binary)
//   - /usr/bin/openntx               (CLI binary)
//   - /usr/bin/openntx-runtime       (runtime shim for binfmt_misc)
//   - /etc/openntx/sandbox.toml      (default sandbox configuration)
//   - /var/lib/openntx/sandboxes/    (runtime sandbox directory)
//   - DEBIAN/control                 (package metadata)
//   - DEBIAN/postinst                (post-installation script)
//   - DEBIAN/prerm                   (pre-removal script)
//
// The engine orchestrates: cargo build --release → binary collection →
// staging layout → dpkg-deb --build → .deb artifact.

use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Constants ────────────────────────────────────────────────────────────────

/// Default package version for the system .deb.
const DEFAULT_VERSION: &str = "2.8.0";

/// Debian architecture tag.
const DEBIAN_ARCH: &str = "amd64";

/// Package name in the Debian ecosystem.
const PACKAGE_NAME: &str = "openntx";

/// Maintainer string (Debian §5.6.5 format).
const MAINTAINER: &str = "OpenNTX Team <maintainer@openntx.org>";

/// Homepage URL.
const HOMEPAGE: &str = "https://github.com/openntx/openntx";

/// Runtime dependencies required by the package.
const DEPENDS: &str = "libc6 (>= 2.31), libx11-6, libgcc-s1 (>= 3.0), libstdc++6 (>= 11)";

/// Recommended (optional) dependencies.
const RECOMMENDS: &str = "wine, xdg-utils";

// ── Public Types ─────────────────────────────────────────────────────────────

/// Configuration options for building the system .deb package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDebOptions {
    /// Package version string (e.g. "2.8.0").
    pub version: String,
    /// Path to the workspace root (where Cargo.toml lives).
    pub workspace_root: PathBuf,
    /// Output directory for the final .deb artifact.
    pub output_dir: PathBuf,
    /// If true, generate staging layout without calling dpkg-deb.
    pub dry_run: bool,
    /// If true, skip `cargo build --release` (binaries already built).
    pub skip_build: bool,
    /// Custom maintainer override.
    pub maintainer: Option<String>,
    /// Custom description override.
    pub description: Option<String>,
}

impl Default for SystemDebOptions {
    fn default() -> Self {
        Self {
            version: DEFAULT_VERSION.to_string(),
            workspace_root: PathBuf::from("."),
            output_dir: PathBuf::from("target/debian"),
            dry_run: false,
            skip_build: false,
            maintainer: None,
            description: None,
        }
    }
}

/// Result of a system .deb build operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDebResult {
    /// Path to the generated .deb file.
    pub deb_path: PathBuf,
    /// Package name used.
    pub package_name: String,
    /// Version used.
    pub version: String,
    /// Architecture tag.
    pub architecture: String,
    /// Number of files included in the package.
    pub file_count: usize,
    /// Total uncompressed size in bytes.
    pub total_size_bytes: u64,
    /// Path to the staging directory (kept if keep_staging was set).
    pub staging_dir: PathBuf,
    /// Build status: "built" or "dry-run".
    pub status: String,
}

// ── Binary Metadata ──────────────────────────────────────────────────────────

/// Describes a single binary to include in the .deb package.
struct BinaryEntry {
    /// Cargo binary target name (used for `cargo build --bin <name>`).
    cargo_bin_name: &'static str,
    /// Destination path inside the .deb (relative to staging root).
    /// Example: "usr/bin/openntx-core"
    dest_relative: &'static str,
    /// Human-readable description for logging.
    description: &'static str,
}

/// All binary targets that constitute the OpenNTX platform.
const BINARY_TARGETS: &[BinaryEntry] = &[
    BinaryEntry {
        cargo_bin_name: "openntx",
        dest_relative: "usr/bin/openntx",
        description: "CLI binary",
    },
    BinaryEntry {
        cargo_bin_name: "openntx-gui",
        dest_relative: "usr/bin/openntx-gui",
        description: "Slint GUI binary",
    },
    BinaryEntry {
        cargo_bin_name: "openntx-appportal",
        dest_relative: "usr/bin/openntx-appportal",
        description: "App Portal binary",
    },
];

// ── Public API ───────────────────────────────────────────────────────────────

/// Build a complete OpenNTX system .deb package.
///
/// This is the main entry point that orchestrates the entire packaging pipeline:
///
/// 1. Optionally run `cargo build --release` for all binary targets.
/// 2. Create the staging directory with the full Debian layout.
/// 3. Copy binaries from `target/release/` into `usr/bin/`.
/// 4. Generate `DEBIAN/control` with dependencies and metadata.
/// 5. Generate `DEBIAN/postinst` (sandbox setup, binfmt registration).
/// 6. Generate `DEBIAN/prerm` (process reaping, cleanup).
/// 7. Install default `etc/openntx/sandbox.toml`.
/// 8. Set filesystem permissions (dirs: 0755, files: 0644, bins: 0755).
/// 9. Invoke `dpkg-deb --build` to produce the final `.deb`.
/// 10. Copy the artifact to the output directory and clean up staging.
///
/// # Errors
///
/// Returns `OpenNtxError::ToolNotAvailable` if `dpkg-deb` is not installed.
/// Returns `OpenNtxError::InvalidInput` if binaries are missing after build.
/// Returns `OpenNtxError::SystemDebBuild` if dpkg-deb exits non-zero.
pub fn build_system_deb(options: &SystemDebOptions) -> Result<SystemDebResult> {
    let version = &options.version;
    let workspace_root = fs::canonicalize(&options.workspace_root)
        .map_err(|source| OpenNtxError::io(&options.workspace_root, source))?;
    let release_dir = workspace_root.join("target/release");
    let staging_root = workspace_root
        .join("target")
        .join(format!("openntx-deb-staging-{version}"));

    // ── Step 1: Build release binaries ──────────────────────────────────

    if !options.skip_build {
        cargo_build_release(&workspace_root)?;
    }

    // ── Step 2: Validate binaries exist ─────────────────────────────────

    validate_binaries(&release_dir)?;

    // ── Step 3: Prepare staging directory ───────────────────────────────

    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .map_err(|source| OpenNtxError::io(&staging_root, source))?;
    }

    // Create directory skeleton
    let dirs_to_create = [
        "DEBIAN",
        "usr/bin",
        "etc/openntx",
        "var/lib/openntx/sandboxes",
    ];
    for rel in &dirs_to_create {
        let dir = staging_root.join(rel);
        fs::create_dir_all(&dir).map_err(|source| OpenNtxError::io(&dir, source))?;
    }

    // ── Step 4: Copy binaries ───────────────────────────────────────────

    let mut file_count: usize = 0;
    let mut total_size: u64 = 0;

    for entry in BINARY_TARGETS {
        let src = release_dir.join(entry.cargo_bin_name);
        let dst = staging_root.join(entry.dest_relative);

        if !src.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "binary not found: {} (expected at {})",
                entry.description,
                src.display()
            )));
        }

        let meta = fs::metadata(&src).map_err(|source| OpenNtxError::io(&src, source))?;
        fs::copy(&src, &dst).map_err(|source| OpenNtxError::io(&src, source))?;

        // Binary files get executable permission
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))
                .map_err(|source| OpenNtxError::io(&dst, source))?;
        }

        file_count += 1;
        total_size += meta.len();
    }

    // ── Step 5: Install default sandbox.toml ────────────────────────────

    let sandbox_toml_path = staging_root.join("etc/openntx/sandbox.toml");
    let sandbox_content = generate_sandbox_toml();
    fs::write(&sandbox_toml_path, sandbox_content.as_bytes())
        .map_err(|source| OpenNtxError::io(&sandbox_toml_path, source))?;
    file_count += 1;
    total_size += sandbox_content.len() as u64;

    // ── Step 6: Generate DEBIAN/control ─────────────────────────────────

    let maintainer = options.maintainer.as_deref().unwrap_or(MAINTAINER);
    let description = options.description.as_deref().unwrap_or(
        "OpenNTX - Windows Application Subsystem for Linux\n \
         Provides the core runtime, GUI frontend, CLI tools, and sandbox\n \
         infrastructure for running Windows applications on Linux.",
    );

    let control_content = generate_system_control(version, maintainer, description);
    let control_path = staging_root.join("DEBIAN/control");
    fs::write(&control_path, control_content.as_bytes())
        .map_err(|source| OpenNtxError::io(&control_path, source))?;
    file_count += 1;
    total_size += control_content.len() as u64;

    // ── Step 7: Generate DEBIAN/postinst ────────────────────────────────

    let postinst_content = generate_postinst();
    let postinst_path = staging_root.join("DEBIAN/postinst");
    fs::write(&postinst_path, postinst_content.as_bytes())
        .map_err(|source| OpenNtxError::io(&postinst_path, source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&postinst_path, fs::Permissions::from_mode(0o755))
            .map_err(|source| OpenNtxError::io(&postinst_path, source))?;
    }
    file_count += 1;
    total_size += postinst_content.len() as u64;

    // ── Step 8: Generate DEBIAN/prerm ───────────────────────────────────

    let prerm_content = generate_prerm();
    let prerm_path = staging_root.join("DEBIAN/prerm");
    fs::write(&prerm_path, prerm_content.as_bytes())
        .map_err(|source| OpenNtxError::io(&prerm_path, source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&prerm_path, fs::Permissions::from_mode(0o755))
            .map_err(|source| OpenNtxError::io(&prerm_path, source))?;
    }
    file_count += 1;
    total_size += prerm_content.len() as u64;

    // ── Step 9: Normalize permissions ───────────────────────────────────

    normalize_staging_permissions(&staging_root)?;

    // ── Step 10: Dry-run exit ───────────────────────────────────────────

    if options.dry_run {
        return Ok(SystemDebResult {
            deb_path: PathBuf::new(),
            package_name: PACKAGE_NAME.to_string(),
            version: version.clone(),
            architecture: DEBIAN_ARCH.to_string(),
            file_count,
            total_size_bytes: total_size,
            staging_dir: staging_root,
            status: "dry-run".to_string(),
        });
    }

    // ── Step 11: Invoke dpkg-deb --build ────────────────────────────────

    let deb_filename = format!("{PACKAGE_NAME}_{version}_{DEBIAN_ARCH}.deb");
    let output_dir = fs::canonicalize(&options.output_dir).unwrap_or_else(|_| {
        // If output dir doesn't exist yet, use workspace_root/target/debian
        workspace_root.join("target/debian")
    });

    // Ensure output directory exists
    fs::create_dir_all(&output_dir).map_err(|source| OpenNtxError::io(&output_dir, source))?;

    let output_deb = output_dir.join(&deb_filename);

    let staging_str = staging_root.to_str().ok_or_else(|| {
        OpenNtxError::InvalidInput("staging path contains invalid UTF-8".to_string())
    })?;
    let output_str = output_deb.to_str().ok_or_else(|| {
        OpenNtxError::InvalidInput("output path contains invalid UTF-8".to_string())
    })?;

    let dpkg_status = Command::new("dpkg-deb")
        .args(["--root-owner-group", "--build", staging_str, output_str])
        .status();

    match dpkg_status {
        Ok(status) => {
            if !status.success() {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(OpenNtxError::SystemDebBuild(format!(
                    "dpkg-deb exited with status: {status}"
                )));
            }
        }
        Err(err) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(OpenNtxError::ToolNotAvailable(format!(
                "dpkg-deb is not available: {err}. Install dpkg-dev to build .deb packages."
            )));
        }
    }

    // ── Step 12: Verify output ──────────────────────────────────────────

    let deb_meta =
        fs::metadata(&output_deb).map_err(|source| OpenNtxError::io(&output_deb, source))?;

    // Clean up staging directory
    let _ = fs::remove_dir_all(&staging_root);

    Ok(SystemDebResult {
        deb_path: output_deb,
        package_name: PACKAGE_NAME.to_string(),
        version: version.clone(),
        architecture: DEBIAN_ARCH.to_string(),
        file_count,
        total_size_bytes: total_size + deb_meta.len(),
        staging_dir: staging_root,
        status: "built".to_string(),
    })
}

// ── Cargo Build ──────────────────────────────────────────────────────────────

/// Run `cargo build --release` for all workspace binaries.
fn cargo_build_release(workspace_root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(workspace_root)
        .status();

    match status {
        Ok(s) => {
            if !s.success() {
                return Err(OpenNtxError::SystemDebBuild(format!(
                    "cargo build --release failed with status: {s}"
                )));
            }
            Ok(())
        }
        Err(err) => Err(OpenNtxError::ToolNotAvailable(format!(
            "cargo is not available: {err}"
        ))),
    }
}

/// Verify all expected binaries exist in the release directory.
fn validate_binaries(release_dir: &Path) -> Result<()> {
    for entry in BINARY_TARGETS {
        let bin_path = release_dir.join(entry.cargo_bin_name);
        if !bin_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "required binary '{}' not found at {}. Run 'cargo build --release' first.",
                entry.cargo_bin_name,
                bin_path.display()
            )));
        }
        let meta = fs::metadata(&bin_path).map_err(|source| OpenNtxError::io(&bin_path, source))?;
        if meta.len() == 0 {
            return Err(OpenNtxError::InvalidInput(format!(
                "binary '{}' is empty (0 bytes)",
                entry.cargo_bin_name
            )));
        }
    }
    Ok(())
}

// ── Control File Generation ──────────────────────────────────────────────────

/// Generate the `DEBIAN/control` file content.
///
/// Follows Debian Policy Manual §5 (Binary package control files).
/// Every field starts at column 0. Continuation lines are indented with
/// exactly one space character (§5.6.13 for Description).
fn generate_system_control(version: &str, maintainer: &str, description: &str) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str(&format!("Package: {PACKAGE_NAME}\n"));
    out.push_str(&format!("Version: {version}\n"));
    out.push_str("Section: misc\n");
    out.push_str("Priority: optional\n");
    out.push_str(&format!("Architecture: {DEBIAN_ARCH}\n"));
    out.push_str(&format!("Depends: {DEPENDS}\n"));
    out.push_str(&format!("Recommends: {RECOMMENDS}\n"));
    out.push_str("Installed-Size: 40960\n");
    out.push_str(&format!("Maintainer: {maintainer}\n"));
    out.push_str(&format!("Homepage: {HOMEPAGE}\n"));

    // Description field: first line is the short description, continuation
    // lines start with a single space. Empty continuation lines use " ."
    // per Debian policy §5.6.13.
    let lines: Vec<&str> = description.lines().collect();
    if let Some((first, rest)) = lines.split_first() {
        out.push_str(&format!("Description: {first}\n"));
        for line in rest {
            if line.trim().is_empty() {
                out.push_str(" .\n");
            } else {
                out.push_str(&format!(" {line}\n"));
            }
        }
    }

    out
}

// ── Post-Installation Script ─────────────────────────────────────────────────

/// Generate the `DEBIAN/postinst` maintainer script.
///
/// Responsibilities:
/// - Create the system sandbox directory `/var/lib/openntx/sandboxes`.
/// - Create the configuration directory `/etc/openntx`.
/// - Set ownership and permissions on sandbox directories.
/// - Register the PE (MZ) format with binfmt_misc so `.exe` files invoke
///   the OpenNTX runtime transparently.
/// - Create the openntx system user/group if not present.
/// - Reload systemd if applicable.
fn generate_postinst() -> String {
    r#"#!/bin/bash
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

# ── Helper ──────────────────────────────────────────────────────────────────

log_action() {
    echo "openntx.postinst: $1"
}

# ── Create system user and group ───────────────────────────────────────────

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

# ── Create directory hierarchy ─────────────────────────────────────────────

create_directories() {
    # Data root
    mkdir -p "$DATA_DIR"
    mkdir -p "$SANDBOX_ROOT"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$LOG_DIR"

    # Per-app runtime directories will be created dynamically
    # but the base structure must exist at install time.

    log_action "directory hierarchy created"
}

# ── Set ownership and permissions ──────────────────────────────────────────

set_permissions() {
    # Data root: owned by openntx user, group-readable
    chown -R "$OPENNTX_USER:$OPENNTX_GROUP" "$DATA_DIR"
    chmod 755 "$DATA_DIR"
    chmod 755 "$SANDBOX_ROOT"
    chmod 755 "$LOG_DIR"

    # Config directory: readable by all, writable by root only
    chown root:root "$CONFIG_DIR"
    chmod 755 "$CONFIG_DIR"

    # Sandbox directory: openntx user manages per-app subtrees
    chown "$OPENNTX_USER:$OPENNTX_GROUP" "$SANDBOX_ROOT"
    chmod 770 "$SANDBOX_ROOT"

    log_action "permissions configured"
}

# ── Configure binfmt_misc for PE executables ───────────────────────────────

setup_binfmt() {
    # Check if binfmt_misc filesystem is mounted
    if [ ! -d /proc/sys/fs/binfmt_misc ]; then
        # Try to mount it
        if modprobe binfmt_misc 2>/dev/null; then
            mount -t binfmt_misc binfmt_misc /proc/sys/fs/binfmt_misc 2>/dev/null || true
        fi
    fi

    if [ ! -f "$BINFMT_REGISTER" ]; then
        log_action "binfmt_misc not available — skipping PE registration"
        return 0
    fi

    # Unregister existing entry if present (idempotent upgrade)
    local unregister_path="/proc/sys/fs/binfmt_misc/$BINFMT_NAME"
    if [ -f "$unregister_path" ]; then
        echo -1 > "$unregister_path" 2>/dev/null || true
        log_action "removed previous binfmt entry"
    fi

    # Register: M = match by magic bytes, MZ = PE header magic
    # O = open the file for reading, C = credentials of the binary
    local reg_string=":${BINFMT_NAME}:M::MZ::${RUNTIME_BIN}:OC"

    if echo "$reg_string" > "$BINFMT_REGISTER" 2>/dev/null; then
        log_action "registered PE (.exe) binfmt_misc handler -> $RUNTIME_BIN"
    else
        log_action "WARNING: binfmt_misc registration failed (non-fatal, manual setup needed)"
    fi
}

# ── Create cgroup hierarchy ────────────────────────────────────────────────

setup_cgroup_hierarchy() {
    local cgroup_root="/sys/fs/cgroup/openntx"

    if [ -d /sys/fs/cgroup ]; then
        mkdir -p "$cgroup_root" 2>/dev/null || true

        # Set up cgroup.subtree_control to enable CPU and memory controllers
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

# ── Main ───────────────────────────────────────────────────────────────────

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
        # No action needed for these cases
        ;;
    *)
        echo "openntx.postinst: unknown argument '$1'" >&2
        exit 1
        ;;
esac

exit 0
"#
    .to_string()
}

// ── Pre-Removal Script ───────────────────────────────────────────────────────

/// Generate the `DEBIAN/prerm` maintainer script.
///
/// Responsibilities:
/// - Terminate all Windows processes managed by OpenNTX via the Reaper Engine.
/// - Kill all processes inside each per-app cgroup.
/// - Clean up cgroup subtrees.
/// - Unregister the binfmt_misc PE handler.
/// - Stop any running OpenNTX daemon services.
fn generate_prerm() -> String {
    r#"#!/bin/bash
# openntx prerm — Pre-removal script for the openntx Debian package.
# Generated by openntx-core packaging engine. Do not edit manually.
set -e

OPENNTX_USER="openntx"
SANDBOX_ROOT="/var/lib/openntx/sandboxes"
CGROUP_ROOT="/sys/fs/cgroup/openntx"
BINFMT_NAME="OpenNTX"

# ── Helper ──────────────────────────────────────────────────────────────────

log_action() {
    echo "openntx.prerm: $1"
}

# ── Terminate all managed Windows processes (Reaper Engine) ─────────────────

reap_all_processes() {
    log_action "invoking Reaper Engine to terminate managed processes"

    if [ ! -d "$CGROUP_ROOT" ]; then
        log_action "no cgroup hierarchy found — nothing to reap"
        return 0
    fi

    local killed_total=0

    # Iterate over per-application cgroup directories
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

        # Phase 1: SIGTERM for graceful shutdown
        for pid in $pids; do
            if [ "$pid" -gt 1 ] 2>/dev/null; then
                kill -TERM "$pid" 2>/dev/null || true
                killed_total=$((killed_total + 1))
            fi
        done

        # Grace period: 500ms
        sleep 0.5

        # Phase 2: SIGKILL for any survivors
        pids=$(cat "$procs_file" 2>/dev/null || true)
        for pid in $pids; do
            if [ "$pid" -gt 1 ] 2>/dev/null; then
                kill -KILL "$pid" 2>/dev/null || true
                log_action "force-killed survivor PID $pid in $app_name"
            fi
        done

        # Clean up the cgroup directory
        rmdir "$app_cgroup" 2>/dev/null || true
    done

    log_action "Reaper Engine complete: terminated $killed_total process(es)"
}

# ── Stop OpenNTX daemon services ────────────────────────────────────────────

stop_daemons() {
    # Stop systemd service if present
    if command -v systemctl >/dev/null 2>&1; then
        if systemctl is-active --quiet openntx-core.service 2>/dev/null; then
            systemctl stop openntx-core.service 2>/dev/null || true
            log_action "stopped openntx-core.service"
        fi
    fi

    # Fallback: kill by process name
    local pids
    pids=$(pgrep -u "$OPENNTX_USER" -f "openntx-core" 2>/dev/null || true)
    if [ -n "$pids" ]; then
        echo "$pids" | xargs kill -TERM 2>/dev/null || true
        sleep 0.5
        echo "$pids" | xargs kill -KILL 2>/dev/null || true
        log_action "killed residual openntx-core processes"
    fi
}

# ── Unregister binfmt_misc handler ─────────────────────────────────────────

unregister_binfmt() {
    local unregister_path="/proc/sys/fs/binfmt_misc/$BINFMT_NAME"

    if [ -f "$unregister_path" ]; then
        echo -1 > "$unregister_path" 2>/dev/null || true
        log_action "unregistered binfmt_misc handler: $BINFMT_NAME"
    fi
}

# ── Clean up cgroup hierarchy ──────────────────────────────────────────────

cleanup_cgroups() {
    if [ -d "$CGROUP_ROOT" ]; then
        # Remove empty subdirectories
        find "$CGROUP_ROOT" -mindepth 1 -type d -empty -delete 2>/dev/null || true
        # Remove root if empty
        rmdir "$CGROUP_ROOT" 2>/dev/null || true
        log_action "cgroup hierarchy cleaned"
    fi
}

# ── Main ───────────────────────────────────────────────────────────────────

case "$1" in
    remove|upgrade|deconfigure)
        reap_all_processes
        stop_daemons
        unregister_binfmt
        cleanup_cgroups
        log_action "pre-removal cleanup complete"
        ;;
    failed-upgrade)
        # No action needed
        ;;
    *)
        echo "openntx.prerm: unknown argument '$1'" >&2
        exit 1
        ;;
esac

exit 0
"#
    .to_string()
}

// ── Sandbox Configuration ────────────────────────────────────────────────────

/// Generate the default `etc/openntx/sandbox.toml` configuration.
///
/// This file defines the default cgroups v2 resource limits and network
/// jail configurations for sandboxed Windows applications.
fn generate_sandbox_toml() -> String {
    r#"# openntx sandbox.toml — Default sandbox configuration.
# Generated by openntx-core packaging engine.
#
# This file defines the default resource limits and isolation policies
# applied to sandboxed Windows applications running under OpenNTX.
#
# Per-application overrides can be placed in:
#   /var/lib/openntx/sandboxes/<app_id>/sandbox.toml

[sandbox]
# Enable or disable sandboxing globally.
# When false, applications run without cgroup or namespace restrictions.
enabled = true

# Default sandbox profile: "strict", "standard", or "permissive".
# Individual app manifests can override this.
default_profile = "standard"

# ── Cgroups v2 Resource Limits ──────────────────────────────────────────────

[sandbox.cgroups]
# Root path for the OpenNTX cgroup v2 hierarchy.
# Created automatically by the postinst script.
root_path = "/sys/fs/cgroup/openntx"

# Default maximum memory per application (in bytes).
# 2 GiB — applications can override via their manifest.
default_memory_max = 2147483648

# Default CPU bandwidth quota.
# Format: "$MAX $PERIOD" in microseconds.
# "50000 100000" = 50% of one CPU core.
# "max" = no CPU limit.
default_cpu_max = "50000 100000"

# CPU period in microseconds (100ms = standard Linux default).
cpu_period = 100000

# Maximum number of PIDs per application cgroup.
# Prevents fork bombs inside the sandbox.
default_pids_max = 256

# ── Network Jail ────────────────────────────────────────────────────────────

[sandbox.network]
# Network isolation mode: "none", "host", "bridge", "isolated".
# "none"    = no network access (default for untrusted apps).
# "host"    = share the host network namespace.
# "bridge"  = connect to the openntx bridge (10.200.0.0/24).
# "isolated" = loopback only (127.0.0.1).
default_mode = "none"

# Enable loopback interface inside the sandbox even when mode = "none".
# Some Windows applications expect localhost to be available.
enable_loopback = true

# DNS configuration for bridge mode.
# The sandbox runs its own DNS resolver at this address.
bridge_dns = "10.200.0.1"

# ── Filesystem Isolation ────────────────────────────────────────────────────

[sandbox.filesystem]
# Mount namespace strategy: "private", "shared", or "chroot".
# "private" = MS_PRIVATE mount propagation (default).
# "shared"  = inherit host mounts.
# "chroot"  = chroot into the app directory only.
default_mount_strategy = "private"

# Bind-mount the host /tmp into the sandbox.
# Set to false for strict isolation.
mount_tmp = false

# Bind-mount the host /dev/null, /dev/zero, /dev/urandom.
mount_dev = true

# Bind-mount the host fonts directory (/usr/share/fonts).
mount_fonts = true

# ── Process Isolation ───────────────────────────────────────────────────────

[sandbox.process]
# Drop all Linux capabilities except the minimum required.
drop_capabilities = true

# Use a dedicated user namespace for the sandboxed process.
# Requires kernel.unprivileged_userns_clone = 1.
use_user_namespace = false

# Set the nice value for sandboxed processes (higher = lower priority).
# Range: -20 (highest) to 19 (lowest).
nice_value = 10

# ── Reaper Engine ───────────────────────────────────────────────────────────

[sandbox.reaper]
# Grace period (in milliseconds) between SIGTERM and SIGKILL.
kill_grace_period_ms = 500

# Interval (in seconds) for the reaper to check for zombie processes.
reap_interval_secs = 5

# Maximum number of processes to reap in a single pass.
max_reap_batch = 64

# ── Logging ─────────────────────────────────────────────────────────────────

[sandbox.logging]
# Log sandbox events to syslog.
use_syslog = true

# Log level: "error", "warn", "info", "debug".
log_level = "info"

# Log file path for sandbox-specific events.
log_file = "/var/log/openntx/sandbox.log"
"#
    .to_string()
}

// ── Permission Normalization ─────────────────────────────────────────────────

/// Recursively normalize permissions in the staging directory.
/// - Directories: 0755
/// - Regular files: 0644 (binaries already set to 0755 in step 4)
#[cfg(unix)]
fn normalize_staging_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let meta = fs::symlink_metadata(path).map_err(|source| OpenNtxError::io(path, source))?;

    if meta.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|source| OpenNtxError::io(path, source))?;

        let entries = fs::read_dir(path).map_err(|source| OpenNtxError::io(path, source))?;
        for entry in entries {
            let entry = entry.map_err(|source| OpenNtxError::io(path, source))?;
            normalize_staging_permissions(&entry.path())?;
        }
    } else if meta.is_file() {
        // Preserve executable bits for binaries, otherwise 0644
        let current_mode = meta.permissions().mode();
        let is_executable = current_mode & 0o111 != 0;
        let target_mode = if is_executable { 0o755 } else { 0o644 };
        fs::set_permissions(path, fs::Permissions::from_mode(target_mode))
            .map_err(|source| OpenNtxError::io(path, source))?;
    }

    Ok(())
}

#[cfg(not(unix))]
fn normalize_staging_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_control_format() {
        let control = generate_system_control(
            "2.8.0",
            "Test <test@test.org>",
            "Short desc\n Long desc line.\n .\n Extra info.",
        );
        assert!(control.contains("Package: openntx"));
        assert!(control.contains("Version: 2.8.0"));
        assert!(control.contains("Architecture: amd64"));
        assert!(control.contains("Depends: libc6"));
        assert!(control.contains("Maintainer: Test <test@test.org>"));
        assert!(control.contains("Description: Short desc"));
        assert!(control.contains(" Long desc line."));
        assert!(control.contains(" ."));
        assert!(control.contains(" Extra info."));
    }

    #[test]
    fn test_postinst_contains_required_sections() {
        let script = generate_postinst();
        assert!(script.contains("#!/bin/bash"));
        assert!(script.contains("OPENNTX_USER=\"openntx\""));
        assert!(script.contains("SANDBOX_ROOT=\"/var/lib/openntx/sandboxes\""));
        assert!(script.contains("create_system_user"));
        assert!(script.contains("create_directories"));
        assert!(script.contains("set_permissions"));
        assert!(script.contains("setup_binfmt"));
        assert!(script.contains("setup_cgroup_hierarchy"));
        assert!(script.contains("binfmt_misc"));
        assert!(script.contains("groupadd"));
        assert!(script.contains("useradd"));
        assert!(script.contains(":OpenNTX:M::MZ::") || script.contains(":${BINFMT_NAME}:M::MZ::"));
        assert!(script.contains("/sys/fs/cgroup/openntx"));
        assert!(script.contains("case \"$1\""));
        assert!(script.contains("configure"));
    }

    #[test]
    fn test_prerm_contains_reaper_engine() {
        let script = generate_prerm();
        assert!(script.contains("#!/bin/bash"));
        assert!(script.contains("reap_all_processes"));
        assert!(script.contains("SIGTERM"));
        assert!(script.contains("SIGKILL"));
        assert!(script.contains("stop_daemons"));
        assert!(script.contains("unregister_binfmt"));
        assert!(script.contains("cleanup_cgroups"));
        assert!(script.contains("cgroup.procs"));
        assert!(script.contains("/sys/fs/cgroup/openntx"));
        assert!(script.contains("echo -1 >"));
        assert!(script.contains("case \"$1\""));
        assert!(script.contains("remove"));
    }

    #[test]
    fn test_sandbox_toml_structure() {
        let toml_content = generate_sandbox_toml();
        assert!(toml_content.contains("[sandbox]"));
        assert!(toml_content.contains("[sandbox.cgroups]"));
        assert!(toml_content.contains("[sandbox.network]"));
        assert!(toml_content.contains("[sandbox.filesystem]"));
        assert!(toml_content.contains("[sandbox.process]"));
        assert!(toml_content.contains("[sandbox.reaper]"));
        assert!(toml_content.contains("[sandbox.logging]"));
        assert!(toml_content.contains("root_path = \"/sys/fs/cgroup/openntx\""));
        assert!(toml_content.contains("default_memory_max = 2147483648"));
        assert!(toml_content.contains("default_cpu_max = \"50000 100000\""));
        assert!(toml_content.contains("kill_grace_period_ms = 500"));
    }

    #[test]
    fn test_sandbox_toml_is_valid_toml() {
        let toml_content = generate_sandbox_toml();
        let parsed: toml::Value =
            toml::from_str(&toml_content).expect("sandbox.toml should be valid TOML");

        let sandbox = parsed.get("sandbox").expect("missing [sandbox] section");
        assert!(sandbox.get("enabled").is_some());
        assert!(sandbox.get("default_profile").is_some());

        let cgroups = sandbox.get("cgroups").expect("missing [sandbox.cgroups]");
        assert_eq!(
            cgroups.get("root_path").and_then(|v| v.as_str()),
            Some("/sys/fs/cgroup/openntx")
        );
    }

    #[test]
    fn test_default_options() {
        let opts = SystemDebOptions::default();
        assert_eq!(opts.version, "2.8.0");
        assert_eq!(opts.output_dir, PathBuf::from("target/debian"));
        assert!(!opts.dry_run);
        assert!(!opts.skip_build);
    }

    #[test]
    fn test_binary_targets_completeness() {
        // Verify all expected binaries are listed
        let names: Vec<&str> = BINARY_TARGETS.iter().map(|e| e.cargo_bin_name).collect();
        assert!(names.contains(&"openntx"));
        assert!(names.contains(&"openntx-gui"));
        assert!(names.contains(&"openntx-appportal"));
    }

    #[test]
    fn test_control_depends_contains_critical_libs() {
        let control = generate_system_control("2.8.0", "M <m@m.org>", "Desc");
        assert!(control.contains("libc6"));
        assert!(control.contains("libx11-6"));
        assert!(control.contains("libgcc-s1"));
        assert!(control.contains("libstdc++6"));
    }
}
