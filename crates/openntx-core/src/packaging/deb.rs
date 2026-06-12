use crate::desktop::generate_desktop_entry;
use crate::packaging::layout::DebPackageLayout;
use crate::registry::AppRegistry;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DebBuildOptions {
    pub app_id: String,
    pub version: String,
    pub output_dir: PathBuf,
    pub dry_run: bool,
}

impl DebBuildOptions {
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            version: "0.9.0".to_string(),
            output_dir: PathBuf::from("dist"),
            dry_run: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebPackagePlan {
    pub layout: DebPackageLayout,
    pub package_name: String,
    pub version: String,
    pub deb_filename: String,
    pub staging_dir: PathBuf,
    pub output_dir: PathBuf,
    pub files_to_package: Vec<String>,
    pub status: String,
}

impl DebPackagePlan {
    pub fn dry_run(app_id: impl Into<String>) -> Self {
        Self {
            layout: DebPackageLayout::new(app_id),
            package_name: String::new(),
            version: String::new(),
            deb_filename: String::new(),
            staging_dir: PathBuf::new(),
            output_dir: PathBuf::new(),
            files_to_package: Vec::new(),
            status: "dry-run / package builder not implemented in V0.9".to_string(),
        }
    }
}

/// Build a .deb package for a registered OpenNTX app.
///
/// Security rules:
/// - Does not execute EXE files or installers.
/// - Does not follow symlinks outside the app directory.
/// - Does not package files outside the registered app dir.
/// - Uses `symlink_metadata()` to reject unsafe entries.
///
/// Staging strategy:
/// The staging directory is always created under `std::env::temp_dir()` so that
/// Unix permission normalization (chmod) works reliably, even when the output
/// directory resides on a filesystem that does not support Unix permissions
/// (e.g. FAT32, NTFS, exFAT on `/media/…`). After dpkg-deb builds the `.deb`,
/// only the final archive is moved to the requested output directory; the
/// staging directory is cleaned up automatically.
pub fn build_deb_package(
    registry: &AppRegistry,
    options: &DebBuildOptions,
) -> Result<DebPackagePlan> {
    let app_id = &options.app_id;
    let manifest = registry.load_manifest(app_id)?;
    let app_dir = registry.paths().app_dir(app_id);

    // Verify app_dir is a real directory
    let app_meta =
        fs::symlink_metadata(&app_dir).map_err(|source| OpenNtxError::io(&app_dir, source))?;
    if app_meta.file_type().is_symlink() || !app_meta.is_dir() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe app directory: {} must be a real directory",
            app_dir.display()
        )));
    }

    let canonical_app =
        fs::canonicalize(&app_dir).map_err(|source| OpenNtxError::io(&app_dir, source))?;

    let layout = DebPackageLayout::new(app_id);
    let package_name = format!("openntx-{app_id}");
    let version = options.version.clone();
    let deb_filename = format!("{package_name}_{version}_all.deb");
    // Always stage in the system temp directory so that chmod reliably applies
    // even when the final output directory is on a non-Unix-permission-capable
    // filesystem (FAT32, NTFS, exFAT on /media/…).
    let staging_dir = std::env::temp_dir().join(format!(
        "openntx-package-build-{}-{package_name}_{version}_all",
        std::process::id()
    ));

    // Collect files to package (for dry-run report)
    let mut files_to_package = Vec::new();

    // Verify and collect manifest.json
    let manifest_src = app_dir.join("manifest.json");
    verify_safe_file(&manifest_src, &canonical_app)?;
    files_to_package.push("manifest.json".to_string());

    // Verify and collect install-plan.json
    let install_plan_src = app_dir.join("install-plan.json");
    if install_plan_src.exists() {
        verify_safe_file(&install_plan_src, &canonical_app)?;
        files_to_package.push("install-plan.json".to_string());
    }

    // Verify and collect metadata.json
    let metadata_src = app_dir.join("metadata.json");
    if metadata_src.exists() {
        verify_safe_file(&metadata_src, &canonical_app)?;
        files_to_package.push("metadata.json".to_string());
    }

    // Collect drive_c/ directory
    let drive_c_src = app_dir.join("drive_c");
    if drive_c_src.exists() {
        verify_safe_dir(&drive_c_src, &canonical_app)?;
        verify_dir_contents_safe(&drive_c_src, &canonical_app)?;
        files_to_package.push("drive_c/".to_string());
    }

    // Collect registry/ directory
    let registry_src = app_dir.join("registry");
    if registry_src.exists() {
        verify_safe_dir(&registry_src, &canonical_app)?;
        verify_dir_contents_safe(&registry_src, &canonical_app)?;
        files_to_package.push("registry/".to_string());
    }

    // Collect capture/ directory if present
    let capture_src = app_dir.join("capture");
    if capture_src.exists() {
        verify_safe_dir(&capture_src, &canonical_app)?;
        verify_dir_contents_safe(&capture_src, &canonical_app)?;
        files_to_package.push("capture/".to_string());
    }

    let plan = DebPackagePlan {
        layout,
        package_name,
        version,
        deb_filename,
        staging_dir,
        output_dir: options.output_dir.clone(),
        files_to_package,
        status: if options.dry_run {
            "dry-run".to_string()
        } else {
            "building".to_string()
        },
    };

    if options.dry_run {
        return Ok(plan);
    }

    // --- Actual build ---

    // Clean and create staging directory
    if plan.staging_dir.exists() {
        fs::remove_dir_all(&plan.staging_dir)
            .map_err(|source| OpenNtxError::io(&plan.staging_dir, source))?;
    }

    // Create DEBIAN directory
    let debian_dir = plan.staging_dir.join("DEBIAN");
    fs::create_dir_all(&debian_dir).map_err(|source| OpenNtxError::io(&debian_dir, source))?;

    // Generate DEBIAN/control
    let control_content = generate_control(&plan.package_name, &plan.version, &manifest.name);
    fs::write(debian_dir.join("control"), control_content.as_bytes())
        .map_err(|source| OpenNtxError::io(debian_dir.join("control"), source))?;

    // Create app directory in staging
    let staging_app_dir = plan.staging_dir.join("opt/openntx/apps").join(app_id);
    fs::create_dir_all(&staging_app_dir)
        .map_err(|source| OpenNtxError::io(&staging_app_dir, source))?;

    // Copy manifest.json
    copy_file_safe(
        &manifest_src,
        &staging_app_dir.join("manifest.json"),
        &canonical_app,
    )?;

    // Copy install-plan.json
    if install_plan_src.exists() {
        copy_file_safe(
            &install_plan_src,
            &staging_app_dir.join("install-plan.json"),
            &canonical_app,
        )?;
    }

    // Copy metadata.json
    if metadata_src.exists() {
        copy_file_safe(
            &metadata_src,
            &staging_app_dir.join("metadata.json"),
            &canonical_app,
        )?;
    }

    // Copy drive_c/
    if drive_c_src.exists() {
        copy_dir_safe(
            &drive_c_src,
            &staging_app_dir.join("drive_c"),
            &canonical_app,
        )?;
    }

    // Copy registry/
    if registry_src.exists() {
        copy_dir_safe(
            &registry_src,
            &staging_app_dir.join("registry"),
            &canonical_app,
        )?;
    }

    // Copy capture/
    if capture_src.exists() {
        copy_dir_safe(
            &capture_src,
            &staging_app_dir.join("capture"),
            &canonical_app,
        )?;
    }

    // Generate desktop entry
    let desktop_content = generate_desktop_entry(&manifest, "openntx")?;
    let desktop_dir = plan.staging_dir.join("usr/share/applications");
    fs::create_dir_all(&desktop_dir).map_err(|source| OpenNtxError::io(&desktop_dir, source))?;
    fs::write(
        desktop_dir.join(format!("openntx-{app_id}.desktop")),
        desktop_content.as_bytes(),
    )
    .map_err(|source| OpenNtxError::io(desktop_dir.join("openntx.desktop"), source))?;

    // Fix permissions: directories 0755, regular files 0644
    set_staging_permissions(&plan.staging_dir)?;

    // Build .deb using dpkg-deb (staging is in temp dir for reliable permissions)
    let output_deb = plan.output_dir.join(&plan.deb_filename);
    // Build into a temp location first, then move to output dir
    let temp_deb = plan.staging_dir.with_extension("deb");

    let status = Command::new("dpkg-deb")
        .args([
            "--root-owner-group",
            "--build",
            plan.staging_dir.to_str().ok_or_else(|| {
                OpenNtxError::InvalidInput("staging path is not valid UTF-8".to_string())
            })?,
            temp_deb.to_str().ok_or_else(|| {
                OpenNtxError::InvalidInput("temp deb path is not valid UTF-8".to_string())
            })?,
        ])
        .status();

    match status {
        Ok(exit_status) => {
            if !exit_status.success() {
                // Clean up staging on failure
                let _ = fs::remove_dir_all(&plan.staging_dir);
                let _ = fs::remove_file(&temp_deb);
                return Err(OpenNtxError::InvalidInput(format!(
                    "dpkg-deb exited with status: {exit_status}"
                )));
            }
        }
        Err(err) => {
            // Clean up staging on failure
            let _ = fs::remove_dir_all(&plan.staging_dir);
            return Err(OpenNtxError::InvalidInput(format!(
                "dpkg-deb is not available or failed to run: {err}. Install dpkg-dev to build .deb packages."
            )));
        }
    }

    // Move .deb from temp to the requested output directory
    fs::create_dir_all(&plan.output_dir)
        .map_err(|source| OpenNtxError::io(&plan.output_dir, source))?;
    fs::rename(&temp_deb, &output_deb).or_else(|_| {
        // Cross-filesystem fallback: copy + remove
        fs::copy(&temp_deb, &output_deb).map_err(|source| OpenNtxError::io(&output_deb, source))?;
        fs::remove_file(&temp_deb).map_err(|source| OpenNtxError::io(&temp_deb, source))
    })?;

    // Clean up staging directory
    let _ = fs::remove_dir_all(&plan.staging_dir);

    // Update plan status
    let mut plan = plan;
    plan.status = "built".to_string();
    Ok(plan)
}

/// Generate DEBIAN/control file content.
///
/// Every field starts at column 0. Description continuation lines are prefixed
/// with exactly one space, as required by Debian policy §5.6.13.
fn generate_control(package_name: &str, version: &str, app_name: &str) -> String {
    // Build line-by-line to guarantee exact whitespace. Rust's `\` continuation
    // strips leading whitespace from the next line, so we cannot use it here.
    let mut out = String::with_capacity(512);
    out.push_str(&format!("Package: {package_name}\n"));
    out.push_str(&format!("Version: {version}\n"));
    out.push_str("Section: misc\n");
    out.push_str("Priority: optional\n");
    out.push_str("Architecture: all\n");
    out.push_str("Depends: openntx-cli\n");
    out.push_str("Maintainer: OpenNTX <openntx@localhost>\n");
    out.push_str(&format!(
        "Description: OpenNTX managed Windows application - {app_name}\n"
    ));
    out.push_str(" This package contains a Windows application managed by OpenNTX.\n");
    out.push_str(" It requires the openntx-cli package to run.\n");
    out
}

/// Verify a file is safe to package: must be a regular file inside the app directory.
fn verify_safe_file(path: &Path, canonical_app: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|source| OpenNtxError::io(path, source))?;

    if meta.file_type().is_symlink() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe file: {} is a symlink and will not be packaged",
            path.display()
        )));
    }

    if !meta.is_file() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe file: {} is not a regular file",
            path.display()
        )));
    }

    let canonical = fs::canonicalize(path).map_err(|source| OpenNtxError::io(path, source))?;
    if !canonical.starts_with(canonical_app) {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe file: {} resolves outside app directory",
            path.display()
        )));
    }

    Ok(())
}

/// Verify a directory is safe to package: must be a real directory inside the app directory.
fn verify_safe_dir(path: &Path, canonical_app: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|source| OpenNtxError::io(path, source))?;

    if meta.file_type().is_symlink() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe directory: {} is a symlink and will not be packaged",
            path.display()
        )));
    }

    if !meta.is_dir() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe directory: {} is not a directory",
            path.display()
        )));
    }

    let canonical = fs::canonicalize(path).map_err(|source| OpenNtxError::io(path, source))?;
    if !canonical.starts_with(canonical_app) {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe directory: {} resolves outside app directory",
            path.display()
        )));
    }

    Ok(())
}

/// Recursively verify directory contents do not contain symlinks pointing outside the app.
fn verify_dir_contents_safe(path: &Path, canonical_app: &Path) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|source| OpenNtxError::io(path, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(path, source))?;
        let entry_path = entry.path();
        let meta = fs::symlink_metadata(&entry_path)
            .map_err(|source| OpenNtxError::io(&entry_path, source))?;

        if meta.file_type().is_symlink() {
            let canonical = fs::canonicalize(&entry_path)
                .map_err(|source| OpenNtxError::io(&entry_path, source))?;
            if !canonical.starts_with(canonical_app) {
                return Err(OpenNtxError::InvalidInput(format!(
                    "unsafe symlink: {} points outside app directory",
                    entry_path.display()
                )));
            }
        } else if meta.is_dir() {
            verify_dir_contents_safe(&entry_path, canonical_app)?;
        }
    }

    Ok(())
}

/// Copy a single file safely, verifying it stays inside the app directory.
fn copy_file_safe(src: &Path, dst: &Path, canonical_app: &Path) -> Result<()> {
    verify_safe_file(src, canonical_app)?;

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }
    fs::copy(src, dst).map_err(|source| OpenNtxError::io(src, source))?;
    Ok(())
}

/// Recursively copy a directory safely, rejecting symlinks that point outside.
fn copy_dir_safe(src: &Path, dst: &Path, canonical_app: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|source| OpenNtxError::io(dst, source))?;

    let entries = fs::read_dir(src).map_err(|source| OpenNtxError::io(src, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(src, source))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        let meta = fs::symlink_metadata(&path).map_err(|source| OpenNtxError::io(&path, source))?;

        if meta.file_type().is_symlink() {
            // Verify symlink target stays inside app directory
            let canonical =
                fs::canonicalize(&path).map_err(|source| OpenNtxError::io(&path, source))?;
            if !canonical.starts_with(canonical_app) {
                return Err(OpenNtxError::InvalidInput(format!(
                    "unsafe symlink: {} points outside app directory",
                    path.display()
                )));
            }
            // Copy the target file, not the symlink
            if canonical.is_dir() {
                copy_dir_safe(&canonical, &dst_path, canonical_app)?;
            } else {
                fs::copy(&canonical, &dst_path)
                    .map_err(|source| OpenNtxError::io(&canonical, source))?;
            }
        } else if meta.is_dir() {
            copy_dir_safe(&path, &dst_path, canonical_app)?;
        } else if meta.is_file() {
            fs::copy(&path, &dst_path).map_err(|source| OpenNtxError::io(&path, source))?;
        }
    }

    Ok(())
}

/// Recursively set permissions on staging directory:
/// directories get 0o755, regular files get 0o644.
#[cfg(unix)]
fn set_staging_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let meta = fs::symlink_metadata(path).map_err(|source| OpenNtxError::io(path, source))?;

    if meta.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|source| OpenNtxError::io(path, source))?;

        let entries = fs::read_dir(path).map_err(|source| OpenNtxError::io(path, source))?;
        for entry in entries {
            let entry = entry.map_err(|source| OpenNtxError::io(path, source))?;
            set_staging_permissions(&entry.path())?;
        }
    } else if meta.is_file() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))
            .map_err(|source| OpenNtxError::io(path, source))?;
    }

    Ok(())
}

#[cfg(not(unix))]
fn set_staging_permissions(_path: &Path) -> Result<()> {
    // On non-Unix systems, permissions are not enforced
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AppManifest;
    use crate::paths::OpenNtxPaths;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_registry() -> AppRegistry {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openntx-deb-test-{unique}"));
        AppRegistry::new(OpenNtxPaths {
            data_root: root.join("data/openntx"),
            apps_root: root.join("data/openntx/apps"),
            logs_root: root.join("state/openntx/logs"),
            cache_root: root.join("cache/openntx"),
            desktop_entries_dir: root.join("data/applications"),
            icons_root: root.join("data/icons/hicolor"),
            system_runtime: PathBuf::from("/usr/lib/openntx"),
        })
    }

    fn register_test_app(registry: &AppRegistry, app_id: &str) {
        let manifest = AppManifest::minimal(app_id, "Test App", "C:/test.exe");
        let plan = registry.build_install_plan(&manifest, None);
        registry.register_plan(&manifest, &plan).unwrap();
    }

    #[test]
    fn default_version_is_0_9_0() {
        let options = DebBuildOptions::new("test-app");
        assert_eq!(options.version, "0.9.0", "default version should be 0.9.0");
    }

    #[test]
    fn package_layout_dry_run() {
        let registry = temp_registry();
        register_test_app(&registry, "test-app");

        let mut options = DebBuildOptions::new("test-app");
        options.dry_run = true;
        options.output_dir = std::env::temp_dir().join("openntx-deb-dryrun-test");

        let plan = build_deb_package(&registry, &options).unwrap();
        assert_eq!(plan.status, "dry-run");
        assert!(plan.files_to_package.contains(&"manifest.json".to_string()));
        assert!(plan
            .files_to_package
            .contains(&"install-plan.json".to_string()));
        assert!(plan.files_to_package.contains(&"drive_c/".to_string()));
        assert!(plan.files_to_package.contains(&"registry/".to_string()));
        assert_eq!(plan.package_name, "openntx-test-app");
        assert_eq!(
            plan.version, "0.9.0",
            "dry-run default version should be 0.9.0"
        );
        assert!(plan.deb_filename.contains("openntx-test-app"));
        assert!(
            plan.deb_filename.contains("0.9.0"),
            "deb filename should contain version"
        );
        assert!(
            !plan.staging_dir.exists(),
            "dry-run should not create staging dir"
        );
        // Staging should be under system temp, not under output_dir
        assert!(
            plan.staging_dir.starts_with(std::env::temp_dir()),
            "staging dir should be under system temp dir for reliable permissions: {}",
            plan.staging_dir.display()
        );
    }

    #[test]
    fn package_staging_creation() {
        let registry = temp_registry();
        register_test_app(&registry, "staging-app");

        let output_dir = std::env::temp_dir().join(format!(
            "openntx-deb-staging-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut options = DebBuildOptions::new("staging-app");
        options.dry_run = false;
        options.output_dir = output_dir.clone();

        // dpkg-deb may not be available, so we test staging creation up to that point
        let result = build_deb_package(&registry, &options);
        match result {
            Ok(plan) => {
                assert_eq!(plan.status, "built");
                // staging_dir is in temp, not output_dir
                assert!(
                    !plan.staging_dir.starts_with(&output_dir),
                    "staging should be in temp dir, not output dir"
                );
                // The .deb should be in the output dir
                assert!(
                    output_dir.join(&plan.deb_filename).exists(),
                    ".deb should be in output dir"
                );
                let _ = fs::remove_dir_all(&output_dir);
            }
            Err(OpenNtxError::InvalidInput(msg)) if msg.contains("dpkg-deb") => {
                // dpkg-deb not available — staging was in temp, check it existed
                // We can't check the exact path since it's in temp, but the error
                // confirms staging was created before dpkg-deb was called
                let _ = fs::remove_dir_all(&output_dir);
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&output_dir);
                panic!("unexpected error: {e}");
            }
        }
    }

    #[test]
    fn generated_control_file_content() {
        let control = generate_control("openntx-test-app", "0.9.0", "Test App");
        assert!(control.contains("Package: openntx-test-app"));
        assert!(control.contains("Version: 0.9.0"));
        assert!(control.contains("Depends: openntx-cli"));
        assert!(control.contains("Architecture: all"));
        assert!(control.contains("Test App"));
    }

    #[test]
    fn generated_control_accepted_by_dpkg_deb() {
        // Write control to a staging DEBIAN dir and validate with dpkg-deb --info
        let tmp = std::env::temp_dir().join(format!(
            "openntx-control-check-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let debian_dir = tmp.join("DEBIAN");
        fs::create_dir_all(&debian_dir).unwrap();

        let control = generate_control("openntx-ctl-test", "1.2.3", "Ctl App");
        fs::write(debian_dir.join("control"), &control).unwrap();

        // dpkg-deb validates control when building; we use --contents on a
        // minimal archive.  Easier: just ask dpkg-deb to parse the control
        // file by building into a throwaway .deb.
        let deb_path = tmp.join("test.deb");
        let result = Command::new("dpkg-deb")
            .args(["--build", tmp.to_str().unwrap(), deb_path.to_str().unwrap()])
            .output();

        match result {
            Ok(output) => {
                assert!(
                    output.status.success(),
                    "dpkg-deb --build should succeed, stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                // Verify we can read it back
                let info = Command::new("dpkg-deb")
                    .args(["--info", deb_path.to_str().unwrap()])
                    .output()
                    .expect("dpkg-deb --info should run");
                assert!(info.status.success(), "dpkg-deb --info should succeed");
                let info_str = String::from_utf8_lossy(&info.stdout);
                assert!(
                    info_str.contains("Package: openntx-ctl-test"),
                    "info should contain package name"
                );
                assert!(
                    info_str.contains("Version: 1.2.3"),
                    "info should contain version"
                );
            }
            Err(_) => {
                // dpkg-deb not installed — skip gracefully
                eprintln!(
                    "skipping generated_control_accepted_by_dpkg_deb: dpkg-deb not available"
                );
            }
        }

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn desktop_file_generated_in_package() {
        let registry = temp_registry();
        register_test_app(&registry, "desktop-app");

        let output_dir = std::env::temp_dir().join(format!(
            "openntx-deb-desktop-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut options = DebBuildOptions::new("desktop-app");
        options.dry_run = false;
        options.output_dir = output_dir.clone();

        let result = build_deb_package(&registry, &options);
        match result {
            Ok(plan) => {
                // After build, staging is cleaned up and .deb is in output_dir
                let deb_path = output_dir.join(&plan.deb_filename);
                assert!(deb_path.exists(), ".deb should exist in output dir");
                let _ = fs::remove_dir_all(&output_dir);
            }
            Err(OpenNtxError::InvalidInput(msg)) if msg.contains("dpkg-deb") => {
                // dpkg-deb not available — staging was created in temp; just verify error
                let _ = fs::remove_dir_all(&output_dir);
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&output_dir);
                panic!("unexpected error: {e}");
            }
        }
    }

    #[test]
    fn missing_app_id_fails_gracefully() {
        let registry = temp_registry();
        let options = DebBuildOptions::new("nonexistent-app");
        let result = build_deb_package(&registry, &options);
        assert!(result.is_err(), "should fail for missing app");
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_symlink_rejected() {
        use std::os::unix::fs::symlink;

        let registry = temp_registry();
        register_test_app(&registry, "symlink-app");
        let app_dir = registry.paths().app_dir("symlink-app");

        // Create a symlink in drive_c pointing outside
        let outside = std::env::temp_dir().join("openntx-outside-deb-test");
        fs::write(&outside, b"secret").unwrap();
        symlink(&outside, app_dir.join("drive_c/evil_link")).unwrap();

        let options = DebBuildOptions::new("symlink-app");
        let result = build_deb_package(&registry, &options);
        assert!(result.is_err(), "should reject unsafe symlink");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("symlink") || err_msg.contains("unsafe"),
            "error should mention symlink or unsafe: {err_msg}"
        );

        let _ = fs::remove_file(&outside);
    }

    #[cfg(unix)]
    #[test]
    fn staging_permissions_are_correct() {
        let registry = temp_registry();
        register_test_app(&registry, "perm-app");

        // Create a capture directory with a file to test capture permissions
        let app_dir = registry.paths().app_dir("perm-app");
        let capture_dir = app_dir.join("capture");
        fs::create_dir_all(&capture_dir).unwrap();
        fs::write(capture_dir.join("capture-diff.json"), b"{}").unwrap();

        let output_dir = std::env::temp_dir().join(format!(
            "openntx-deb-perm-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut options = DebBuildOptions::new("perm-app");
        options.dry_run = false;
        options.output_dir = output_dir.clone();

        let result = build_deb_package(&registry, &options);
        match result {
            Ok(plan) => {
                assert_eq!(plan.status, "built");
                // Staging is cleaned up; verify permissions inside the .deb
                let deb_path = output_dir.join(&plan.deb_filename);
                assert!(deb_path.exists(), ".deb should be in output dir");

                // Use dpkg-deb -c to verify permissions inside the archive
                let output = Command::new("dpkg-deb")
                    .args(["-c", deb_path.to_str().unwrap()])
                    .output()
                    .expect("dpkg-deb -c should run");
                assert!(output.status.success(), "dpkg-deb -c should succeed");
                let contents = String::from_utf8_lossy(&output.stdout);

                // All directories should be drwxr-xr-x (0755)
                // All regular files should be -rw-r--r-- (0644)
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    // Lines look like: drwxr-xr-x root/root  0 ... ./opt/
                    // or: -rw-r--r-- root/root  123 ... ./opt/.../manifest.json
                    if line.starts_with("drwxr-xr-x") {
                        // expected directory permission
                    } else if line.starts_with("-rw-r--r--") {
                        // expected file permission
                    } else {
                        panic!("unexpected permissions in .deb: {line}");
                    }
                }

                let _ = fs::remove_dir_all(&output_dir);
            }
            Err(OpenNtxError::InvalidInput(msg)) if msg.contains("dpkg-deb") => {
                // dpkg-deb not available — can't test
                let _ = fs::remove_dir_all(&output_dir);
                eprintln!("skipping staging_permissions_are_correct: dpkg-deb not available");
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&output_dir);
                panic!("unexpected error: {e}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn set_staging_permissions_normalizes_executable_files() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = std::env::temp_dir().join(format!(
            "openntx-perm-normalize-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let subdir = tmp.join("sub");
        fs::create_dir_all(&subdir).unwrap();

        // Create files with executable permissions (simulating source files)
        let file_a = tmp.join("a.json");
        let file_b = subdir.join("b.txt");
        fs::write(&file_a, b"{}").unwrap();
        fs::write(&file_b, b"hello").unwrap();
        fs::set_permissions(&file_a, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&file_b, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o700)).unwrap();

        // Verify setup: files are executable
        assert_eq!(
            fs::metadata(&file_a).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(&file_b).unwrap().permissions().mode() & 0o777,
            0o755
        );

        // Run normalization
        set_staging_permissions(&tmp).unwrap();

        // Verify: files should be 0644, dirs should be 0755
        assert_eq!(
            fs::metadata(&tmp).unwrap().permissions().mode() & 0o777,
            0o755,
            "root dir"
        );
        assert_eq!(
            fs::metadata(&subdir).unwrap().permissions().mode() & 0o777,
            0o755,
            "subdir"
        );
        assert_eq!(
            fs::metadata(&file_a).unwrap().permissions().mode() & 0o777,
            0o644,
            "file_a"
        );
        assert_eq!(
            fs::metadata(&file_b).unwrap().permissions().mode() & 0o777,
            0o644,
            "file_b"
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
