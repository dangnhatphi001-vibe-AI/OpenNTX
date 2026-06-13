use crate::app_id::is_valid_app_id;
use crate::manifest::validate_manifest;
use crate::registry::AppRegistry;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Global doctor report for the entire OpenNTX installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalDoctorReport {
    pub data_dir_exists: bool,
    pub data_dir_path: String,
    pub app_count: usize,
    pub broken_apps: Vec<String>,
    pub broken_app_count: usize,
    pub logs_dir_exists: bool,
    pub logs_dir_path: String,
    pub logs_dir_writable: bool,
    pub desktop_entries_dir_exists: bool,
    pub dpkg_deb_available: bool,
    pub notify_send_available: bool,
    pub warnings: Vec<String>,
    pub status: String,
}

/// Per-app doctor report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDoctorReport {
    pub app_id: String,
    pub app_name: String,
    pub manifest_exists: bool,
    pub manifest_is_regular_file: bool,
    pub manifest_valid: bool,
    pub manifest_validation_error: Option<String>,
    pub install_plan_exists: bool,
    pub drive_c_exists: bool,
    pub drive_c_is_real_dir: bool,
    pub registry_exists: bool,
    pub registry_is_real_dir: bool,
    pub capture_dir_exists: bool,
    pub desktop_entry_exists: bool,
    pub package_build_possible: bool,
    pub log_dir_writable: bool,
    pub unsafe_symlinks: Vec<String>,
    pub warnings: Vec<String>,
    pub status: String,
}

/// Repair plan for an app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairPlan {
    pub app_id: String,
    pub actions: Vec<RepairAction>,
    pub applied: bool,
    pub unsafe_symlinks_found: Vec<String>,
}

/// Individual repair action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAction {
    pub description: String,
    pub path: String,
    pub action_type: String,
    pub applied: bool,
}

/// Run global doctor checks.
pub fn global_doctor(registry: &AppRegistry) -> Result<GlobalDoctorReport> {
    let paths = registry.paths();
    let data_dir_exists = paths.data_root.exists();
    let apps = registry.list_apps().unwrap_or_default();
    let app_count = apps.len();

    let mut broken_apps = Vec::new();
    for app in &apps {
        let manifest_path = paths.manifest_path(&app.app_id);
        if !manifest_path.exists() {
            broken_apps.push(app.app_id.clone());
            continue;
        }
        match registry.load_manifest(&app.app_id) {
            Ok(m) => {
                if validate_manifest(&m).is_err() {
                    broken_apps.push(app.app_id.clone());
                }
            }
            Err(_) => {
                broken_apps.push(app.app_id.clone());
            }
        }
    }

    let logs_dir_exists = paths.logs_root.exists();
    let logs_dir_writable = if logs_dir_exists {
        check_dir_writable(&paths.logs_root)
    } else {
        false
    };

    let desktop_entries_dir_exists = paths.desktop_entries_dir.exists();
    let dpkg_deb_available = command_available("dpkg-deb");
    let notify_send_available = command_available("notify-send");

    let mut warnings = Vec::new();
    if !data_dir_exists {
        warnings.push(
            "OpenNTX data directory does not exist. Run 'openntx install' to create it."
                .to_string(),
        );
    }
    if !logs_dir_exists {
        warnings
            .push("Logs directory does not exist. It will be created on first run.".to_string());
    }
    if !dpkg_deb_available {
        warnings.push("dpkg-deb is not available. Package building will not work.".to_string());
    }
    if !notify_send_available {
        warnings
            .push("notify-send is not available. Desktop notifications will not work.".to_string());
    }
    // Check filesystem permission preservation
    if let Ok(meta) = fs::metadata(&paths.data_root) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode();
            if mode & 0o700 != 0o700 {
                warnings.push(format!(
                    "Data directory {} may not preserve Unix permissions (mode: {:o}).",
                    paths.data_root.display(),
                    mode
                ));
            }
        }
    }

    let status = if broken_apps.is_empty() && warnings.is_empty() {
        "healthy".to_string()
    } else if !broken_apps.is_empty() {
        "has issues".to_string()
    } else {
        "healthy with warnings".to_string()
    };

    Ok(GlobalDoctorReport {
        data_dir_exists,
        data_dir_path: paths.data_root.display().to_string(),
        app_count,
        broken_app_count: broken_apps.len(),
        broken_apps,
        logs_dir_exists,
        logs_dir_path: paths.logs_root.display().to_string(),
        logs_dir_writable,
        desktop_entries_dir_exists,
        dpkg_deb_available,
        notify_send_available,
        warnings,
        status,
    })
}

/// Run per-app doctor checks.
pub fn app_doctor(registry: &AppRegistry, app_id: &str) -> Result<AppDoctorReport> {
    if !is_valid_app_id(app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )));
    }

    let paths = registry.paths();
    let app_dir = paths.app_dir(app_id);
    if !app_dir.exists() {
        return Err(OpenNtxError::AppNotFound(format!(
            "registered app does not exist: {app_id}"
        )));
    }

    let manifest_path = paths.manifest_path(app_id);
    let manifest_exists = manifest_path.exists();
    let manifest_is_regular_file = if manifest_exists {
        check_is_regular_file(&manifest_path)
    } else {
        false
    };

    let (manifest_valid, manifest_validation_error) = if manifest_exists && manifest_is_regular_file
    {
        match registry.load_manifest(app_id) {
            Ok(m) => match validate_manifest(&m) {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            },
            Err(e) => (false, Some(e.to_string())),
        }
    } else {
        (
            false,
            Some("manifest does not exist or is not a regular file".to_string()),
        )
    };

    let app_name = if manifest_valid {
        registry
            .load_manifest(app_id)
            .map(|m| m.name)
            .unwrap_or_else(|_| "unknown".to_string())
    } else {
        "unknown".to_string()
    };

    let install_plan_path = app_dir.join("install-plan.json");
    let install_plan_exists = install_plan_path.exists();

    let drive_c_path = paths.drive_c_path(app_id);
    let drive_c_exists = drive_c_path.exists();
    let drive_c_is_real_dir = if drive_c_exists {
        check_is_real_dir(&drive_c_path)
    } else {
        false
    };

    let registry_path = paths.registry_path(app_id);
    let registry_exists = registry_path.exists();
    let registry_is_real_dir = if registry_exists {
        check_is_real_dir(&registry_path)
    } else {
        false
    };

    let capture_dir = paths.capture_dir(app_id);
    let capture_dir_exists = capture_dir.exists();

    let desktop_entry_path = paths.desktop_entry_path(app_id);
    let desktop_entry_exists = desktop_entry_path.exists();

    let package_build_possible =
        manifest_valid && dpkg_deb_available() && drive_c_is_real_dir && registry_is_real_dir;

    let logs_dir = app_dir.join("logs");
    let log_dir_writable = if logs_dir.exists() {
        check_dir_writable(&logs_dir)
    } else {
        false
    };

    // Check for unsafe symlinks
    let mut unsafe_symlinks = Vec::new();
    check_unsafe_symlinks(&app_dir, &mut unsafe_symlinks);

    let mut warnings = Vec::new();
    if !manifest_exists {
        warnings.push("manifest.json is missing".to_string());
    } else if !manifest_is_regular_file {
        warnings.push("manifest.json is not a regular file (possible symlink)".to_string());
    } else if !manifest_valid {
        warnings.push(format!(
            "manifest validation failed: {}",
            manifest_validation_error.as_deref().unwrap_or("unknown")
        ));
    }
    if !install_plan_exists {
        warnings.push("install-plan.json is missing".to_string());
    }
    if !drive_c_exists {
        warnings.push("drive_c/ directory is missing".to_string());
    } else if !drive_c_is_real_dir {
        warnings.push("drive_c/ is not a real directory (possible symlink)".to_string());
    }
    if !registry_exists {
        warnings.push("registry/ directory is missing".to_string());
    } else if !registry_is_real_dir {
        warnings.push("registry/ is not a real directory (possible symlink)".to_string());
    }
    if !capture_dir_exists {
        warnings.push("capture/ directory is missing".to_string());
    }
    if !desktop_entry_exists {
        warnings.push("desktop entry is missing".to_string());
    }
    if !log_dir_writable {
        warnings.push("logs directory is not writable or missing".to_string());
    }
    if !unsafe_symlinks.is_empty() {
        warnings.push(format!("found {} unsafe symlink(s)", unsafe_symlinks.len()));
    }

    let status = if warnings.is_empty() && unsafe_symlinks.is_empty() {
        "healthy".to_string()
    } else if !unsafe_symlinks.is_empty() {
        "unsafe symlinks found".to_string()
    } else {
        "has warnings".to_string()
    };

    Ok(AppDoctorReport {
        app_id: app_id.to_string(),
        app_name,
        manifest_exists,
        manifest_is_regular_file,
        manifest_valid,
        manifest_validation_error,
        install_plan_exists,
        drive_c_exists,
        drive_c_is_real_dir,
        registry_exists,
        registry_is_real_dir,
        capture_dir_exists,
        desktop_entry_exists,
        package_build_possible,
        log_dir_writable,
        unsafe_symlinks,
        warnings,
        status,
    })
}

/// Attempt safe repairs on an app.
pub fn repair_app(registry: &AppRegistry, app_id: &str, dry_run: bool) -> Result<RepairPlan> {
    if !is_valid_app_id(app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )));
    }

    let paths = registry.paths();
    let app_dir = paths.app_dir(app_id);
    if !app_dir.exists() {
        return Err(OpenNtxError::AppNotFound(format!(
            "registered app does not exist: {app_id}"
        )));
    }

    // First check for unsafe symlinks
    let mut unsafe_symlinks = Vec::new();
    check_unsafe_symlinks(&app_dir, &mut unsafe_symlinks);

    if !unsafe_symlinks.is_empty() {
        return Ok(RepairPlan {
            app_id: app_id.to_string(),
            actions: Vec::new(),
            applied: false,
            unsafe_symlinks_found: unsafe_symlinks,
        });
    }

    let mut actions = Vec::new();

    // Check and create drive_c
    let drive_c_path = paths.drive_c_path(app_id);
    if !drive_c_path.exists() {
        actions.push(RepairAction {
            description: "Create missing drive_c/ directory".to_string(),
            path: drive_c_path.display().to_string(),
            action_type: "create_dir".to_string(),
            applied: !dry_run,
        });
        if !dry_run {
            fs::create_dir_all(&drive_c_path)
                .map_err(|source| OpenNtxError::io(&drive_c_path, source))?;
        }
    }

    // Check and create registry
    let registry_path = paths.registry_path(app_id);
    if !registry_path.exists() {
        actions.push(RepairAction {
            description: "Create missing registry/ directory".to_string(),
            path: registry_path.display().to_string(),
            action_type: "create_dir".to_string(),
            applied: !dry_run,
        });
        if !dry_run {
            fs::create_dir_all(&registry_path)
                .map_err(|source| OpenNtxError::io(&registry_path, source))?;
        }
    }

    // Check and create logs
    let logs_path = app_dir.join("logs");
    if !logs_path.exists() {
        actions.push(RepairAction {
            description: "Create missing logs/ directory".to_string(),
            path: logs_path.display().to_string(),
            action_type: "create_dir".to_string(),
            applied: !dry_run,
        });
        if !dry_run {
            fs::create_dir_all(&logs_path)
                .map_err(|source| OpenNtxError::io(&logs_path, source))?;
        }
    }

    // Check and create capture
    let capture_path = paths.capture_dir(app_id);
    if !capture_path.exists() {
        actions.push(RepairAction {
            description: "Create missing capture/ directory".to_string(),
            path: capture_path.display().to_string(),
            action_type: "create_dir".to_string(),
            applied: !dry_run,
        });
        if !dry_run {
            fs::create_dir_all(&capture_path)
                .map_err(|source| OpenNtxError::io(&capture_path, source))?;
        }
    }

    // Regenerate desktop launcher if manifest is valid
    let manifest_path = paths.manifest_path(app_id);
    let desktop_entry_path = paths.desktop_entry_path(app_id);
    if manifest_path.exists()
        && check_is_regular_file(&manifest_path)
        && !desktop_entry_path.exists()
    {
        if let Ok(manifest) = registry.load_manifest(app_id) {
            if validate_manifest(&manifest).is_ok() {
                actions.push(RepairAction {
                    description: "Regenerate desktop launcher".to_string(),
                    path: desktop_entry_path.display().to_string(),
                    action_type: "regenerate_desktop".to_string(),
                    applied: !dry_run,
                });
                if !dry_run {
                    use crate::desktop::generate_desktop_entry;
                    let content = generate_desktop_entry(&manifest, "openntx")?;
                    if let Some(parent) = desktop_entry_path.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|source| OpenNtxError::io(parent, source))?;
                    }
                    fs::write(&desktop_entry_path, content.as_bytes())
                        .map_err(|source| OpenNtxError::io(&desktop_entry_path, source))?;
                }
            }
        }
    }

    Ok(RepairPlan {
        app_id: app_id.to_string(),
        actions,
        applied: !dry_run,
        unsafe_symlinks_found: Vec::new(),
    })
}

fn check_is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn check_is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_dir() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn check_dir_writable(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let test_file = path.join(".openntx-write-test");
    match fs::write(&test_file, b"test") {
        Ok(()) => {
            let _ = fs::remove_file(&test_file);
            true
        }
        Err(_) => false,
    }
}

fn command_available(command: &str) -> bool {
    std::process::Command::new("which")
        .arg(command)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn dpkg_deb_available() -> bool {
    command_available("dpkg-deb")
}

/// Recursively check for unsafe symlinks in a directory.
fn check_unsafe_symlinks(dir: &Path, unsafe_links: &mut Vec<String>) {
    let canonical_dir = match fs::canonicalize(dir) {
        Ok(p) => p,
        Err(_) => return,
    };

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            match fs::canonicalize(&path) {
                Ok(canonical) => {
                    if !canonical.starts_with(&canonical_dir) {
                        unsafe_links.push(format!("{} -> {}", path.display(), canonical.display()));
                    }
                }
                Err(_) => {
                    unsafe_links.push(format!("{} (unresolvable)", path.display()));
                }
            }
        } else if meta.is_dir() {
            check_unsafe_symlinks(&path, unsafe_links);
        }
    }
}
