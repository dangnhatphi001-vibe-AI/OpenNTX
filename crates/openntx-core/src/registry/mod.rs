use crate::app_id::is_valid_app_id;
use crate::desktop::generate_desktop_entry;
use crate::manifest::{read_manifest, write_manifest_pretty, AppManifest};
use crate::paths::OpenNtxPaths;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct AppRegistry {
    paths: OpenNtxPaths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub schema_version: String,
    pub app_id: String,
    pub name: String,
    pub manifest_path: String,
    pub app_dir: String,
    pub drive_c: String,
    pub registry: String,
    pub logs: String,
    pub install_mode: String,
    pub architecture: String,
    pub subsystem: Option<String>,
    pub imported_dll_count: usize,
    pub desktop_integration: String,
    pub sandbox_profile: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppMetadata {
    pub schema_version: String,
    pub app_id: String,
    pub name: String,
    pub registered_at_unix: u64,
    pub source_file: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredApp {
    pub app_id: String,
    pub name: String,
    pub install_mode: String,
    pub architecture: String,
    pub sandbox_profile: String,
    pub executable_path: String,
    pub imported_dll_count: usize,
    pub status: String,
    pub desktop_launcher_exists: bool,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredAppJson {
    pub app_id: String,
    pub name: String,
    pub architecture: String,
    pub install_mode: String,
    pub desktop_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSummaryJson {
    pub app_id: String,
    pub name: String,
    pub version: Option<String>,
    pub architecture: String,
    pub install_mode: String,
    pub executable_path: String,
    pub sandbox_profile: String,
    pub imported_dll_count: usize,
    pub desktop_status: String,
    pub desktop_name: String,
    pub source_file: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationResult {
    pub app_id: String,
    pub app_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub install_plan_path: PathBuf,
    pub metadata_path: PathBuf,
    pub drive_c_path: PathBuf,
    pub registry_path: PathBuf,
    pub logs_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveMode {
    DryRun,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovePlan {
    pub app_id: String,
    pub app_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub desktop_entry_path: PathBuf,
    pub would_remove: bool,
    pub removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopMode {
    DryRun,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPlan {
    pub app_id: String,
    pub desktop_entry_path: PathBuf,
    pub content: String,
    pub written: bool,
    pub removed: bool,
}

/// Result of a rename operation.
#[derive(Debug, Clone)]
pub struct RenameResult {
    pub app_id: String,
    pub old_name: String,
    pub new_name: String,
    pub updated_manifest: bool,
}

/// Result of a duplicate operation.
#[derive(Debug, Clone)]
pub struct DuplicateResult {
    pub source_app_id: String,
    pub new_app_id: String,
    pub new_app_dir: PathBuf,
    pub new_manifest_path: PathBuf,
}

/// Result of an export operation.
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub app_id: String,
    pub bundle_path: PathBuf,
    pub bundle_format: String,
    pub files_included: Vec<String>,
}

/// Result of an import operation.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub app_id: String,
    pub app_dir: PathBuf,
    pub manifest_path: PathBuf,
}

impl AppRegistry {
    pub fn new(paths: OpenNtxPaths) -> Self {
        Self { paths }
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self::new(OpenNtxPaths::from_env()?))
    }

    pub fn paths(&self) -> &OpenNtxPaths {
        &self.paths
    }

    pub fn build_install_plan(
        &self,
        manifest: &AppManifest,
        subsystem: Option<String>,
    ) -> InstallPlan {
        let app_dir = self.paths.app_dir(&manifest.app_id);
        InstallPlan {
            schema_version: "0.1.0".to_string(),
            app_id: manifest.app_id.clone(),
            name: manifest.name.clone(),
            manifest_path: self
                .paths
                .manifest_path(&manifest.app_id)
                .display()
                .to_string(),
            app_dir: app_dir.display().to_string(),
            drive_c: self
                .paths
                .drive_c_path(&manifest.app_id)
                .display()
                .to_string(),
            registry: self
                .paths
                .registry_path(&manifest.app_id)
                .display()
                .to_string(),
            logs: app_dir.join("logs").display().to_string(),
            install_mode: manifest.install_mode.clone(),
            architecture: manifest.architecture.clone(),
            subsystem,
            imported_dll_count: manifest.diagnostics.imported_dlls.len(),
            desktop_integration: if manifest.desktop.create_launcher {
                "planned".to_string()
            } else {
                "disabled".to_string()
            },
            sandbox_profile: manifest.sandbox.profile.clone(),
            status: "written / analysis-only".to_string(),
        }
    }

    pub fn register_plan(
        &self,
        manifest: &AppManifest,
        install_plan: &InstallPlan,
    ) -> Result<RegistrationResult> {
        validate_app_id(&manifest.app_id)?;

        let app_dir = self.paths.app_dir(&manifest.app_id);
        let manifest_path = self.paths.manifest_path(&manifest.app_id);
        let install_plan_path = app_dir.join("install-plan.json");
        let metadata_path = app_dir.join("metadata.json");
        let drive_c_path = self.paths.drive_c_path(&manifest.app_id);
        let registry_path = self.paths.registry_path(&manifest.app_id);
        let logs_path = app_dir.join("logs");

        fs::create_dir_all(&drive_c_path)
            .map_err(|source| OpenNtxError::io(&drive_c_path, source))?;
        fs::create_dir_all(&registry_path)
            .map_err(|source| OpenNtxError::io(&registry_path, source))?;
        fs::create_dir_all(&logs_path).map_err(|source| OpenNtxError::io(&logs_path, source))?;

        write_manifest_pretty(&manifest_path, manifest)?;
        write_json_pretty(&install_plan_path, install_plan)?;
        write_json_pretty(
            &metadata_path,
            &AppMetadata {
                schema_version: "0.1.0".to_string(),
                app_id: manifest.app_id.clone(),
                name: manifest.name.clone(),
                registered_at_unix: unix_now(),
                source_file: manifest.source.original_file.clone(),
                status: "registered / analysis-only".to_string(),
            },
        )?;

        Ok(RegistrationResult {
            app_id: manifest.app_id.clone(),
            app_dir,
            manifest_path,
            install_plan_path,
            metadata_path,
            drive_c_path,
            registry_path,
            logs_path,
        })
    }

    pub fn list_apps(&self) -> Result<Vec<RegisteredApp>> {
        if !self.paths.apps_root.exists() {
            return Ok(Vec::new());
        }

        let mut apps = Vec::new();
        let entries = fs::read_dir(&self.paths.apps_root)
            .map_err(|source| OpenNtxError::io(&self.paths.apps_root, source))?;

        for entry in entries {
            let entry = entry.map_err(|source| OpenNtxError::io(&self.paths.apps_root, source))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("manifest.json");

            // Use symlink_metadata to reject symlinked manifest.json
            let manifest_meta = match fs::symlink_metadata(&manifest_path) {
                Ok(m) => m,
                Err(_) => continue, // file doesn't exist or inaccessible
            };
            if manifest_meta.file_type().is_symlink() || !manifest_meta.is_file() {
                continue; // skip apps with unsafe manifest
            }

            let manifest = read_manifest(&manifest_path)?;
            let app_id = manifest.app_id;
            let desktop_launcher_exists = self.paths.desktop_entry_path(&app_id).exists();
            apps.push(RegisteredApp {
                app_id,
                name: manifest.name,
                install_mode: manifest.install_mode,
                architecture: manifest.architecture,
                sandbox_profile: manifest.sandbox.profile,
                executable_path: manifest.executable.path,
                imported_dll_count: manifest.diagnostics.imported_dlls.len(),
                status: "registered / analysis-only".to_string(),
                desktop_launcher_exists,
                manifest_path,
            });
        }

        apps.sort_by(|a, b| a.app_id.cmp(&b.app_id));
        Ok(apps)
    }

    pub fn load_manifest(&self, app_id: &str) -> Result<AppManifest> {
        validate_app_id(app_id)?;
        read_manifest(self.paths.manifest_path(app_id))
    }

    pub fn remove_app(&self, app_id: &str, mode: RemoveMode) -> Result<RemovePlan> {
        validate_app_id(app_id)?;
        let app_dir = self.paths.app_dir(app_id);
        let manifest_path = self.paths.manifest_path(app_id);
        let desktop_entry_path = self.paths.desktop_entry_path(app_id);

        if !app_dir.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "registered app does not exist: {app_id}"
            )));
        }

        let mut removed = false;
        if mode == RemoveMode::Delete {
            fs::remove_dir_all(&app_dir).map_err(|source| OpenNtxError::io(&app_dir, source))?;
            if desktop_entry_path.exists() {
                fs::remove_file(&desktop_entry_path)
                    .map_err(|source| OpenNtxError::io(&desktop_entry_path, source))?;
            }
            removed = true;
        }

        Ok(RemovePlan {
            app_id: app_id.to_string(),
            app_dir,
            manifest_path,
            desktop_entry_path,
            would_remove: true,
            removed,
        })
    }

    pub fn create_desktop_entry(
        &self,
        app_id: &str,
        mode: DesktopMode,
        cli_command: &str,
    ) -> Result<DesktopPlan> {
        validate_app_id(app_id)?;
        let manifest = self.load_manifest(app_id)?;
        let content = generate_desktop_entry(&manifest, cli_command)?;
        let desktop_entry_path = self.paths.desktop_entry_path(app_id);
        let mut written = false;

        if mode == DesktopMode::Write {
            if let Some(parent) = desktop_entry_path.parent() {
                fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
            }
            fs::write(&desktop_entry_path, content.as_bytes())
                .map_err(|source| OpenNtxError::io(&desktop_entry_path, source))?;
            written = true;
        }

        Ok(DesktopPlan {
            app_id: app_id.to_string(),
            desktop_entry_path,
            content,
            written,
            removed: false,
        })
    }

    pub fn remove_desktop_entry(&self, app_id: &str, mode: DesktopMode) -> Result<DesktopPlan> {
        validate_app_id(app_id)?;
        let desktop_entry_path = self.paths.desktop_entry_path(app_id);
        let mut removed = false;

        if mode == DesktopMode::Write && desktop_entry_path.exists() {
            fs::remove_file(&desktop_entry_path)
                .map_err(|source| OpenNtxError::io(&desktop_entry_path, source))?;
            removed = true;
        }

        Ok(DesktopPlan {
            app_id: app_id.to_string(),
            desktop_entry_path,
            content: String::new(),
            written: false,
            removed,
        })
    }

    /// Rename an app's display name in the manifest.
    pub fn rename_app(&self, app_id: &str, new_name: &str, dry_run: bool) -> Result<RenameResult> {
        validate_app_id(app_id)?;
        let mut manifest = self.load_manifest(app_id)?;
        let old_name = manifest.name.clone();

        manifest.name = new_name.to_string();
        manifest.desktop.name = new_name.to_string();

        if !dry_run {
            write_manifest_pretty(self.paths.manifest_path(app_id), &manifest)?;
        }

        Ok(RenameResult {
            app_id: app_id.to_string(),
            old_name,
            new_name: new_name.to_string(),
            updated_manifest: !dry_run,
        })
    }

    /// Duplicate an existing app under a new app-id.
    pub fn duplicate_app(
        &self,
        source_app_id: &str,
        new_app_id: &str,
        dry_run: bool,
    ) -> Result<DuplicateResult> {
        validate_app_id(source_app_id)?;
        validate_app_id(new_app_id)?;

        if !self.paths.app_dir(source_app_id).exists() {
            return Err(OpenNtxError::AppNotFound(format!(
                "source app does not exist: {source_app_id}"
            )));
        }

        if self.paths.app_dir(new_app_id).exists() {
            return Err(OpenNtxError::AlreadyExists(format!(
                "target app already exists: {new_app_id}"
            )));
        }

        let source_dir = self.paths.app_dir(source_app_id);
        let canonical_source = fs::canonicalize(&source_dir)
            .map_err(|source| OpenNtxError::io(&source_dir, source))?;

        if dry_run {
            return Ok(DuplicateResult {
                source_app_id: source_app_id.to_string(),
                new_app_id: new_app_id.to_string(),
                new_app_dir: self.paths.app_dir(new_app_id),
                new_manifest_path: self.paths.manifest_path(new_app_id),
            });
        }

        let new_dir = self.paths.app_dir(new_app_id);
        fs::create_dir_all(&new_dir).map_err(|source| OpenNtxError::io(&new_dir, source))?;

        // Copy safe files
        copy_safe_contents(&source_dir, &new_dir, &canonical_source)?;

        // Update manifest app_id
        let new_manifest_path = self.paths.manifest_path(new_app_id);
        let mut manifest = read_manifest(&new_manifest_path)?;
        manifest.app_id = new_app_id.to_string();
        write_manifest_pretty(&new_manifest_path, &manifest)?;

        Ok(DuplicateResult {
            source_app_id: source_app_id.to_string(),
            new_app_id: new_app_id.to_string(),
            new_app_dir: new_dir,
            new_manifest_path,
        })
    }

    /// Export an app as a portable bundle.
    pub fn export_bundle(
        &self,
        app_id: &str,
        output_path: &Path,
        dry_run: bool,
    ) -> Result<ExportResult> {
        validate_app_id(app_id)?;
        let app_dir = self.paths.app_dir(app_id);
        if !app_dir.exists() {
            return Err(OpenNtxError::AppNotFound(format!(
                "app does not exist: {app_id}"
            )));
        }

        let canonical_app =
            fs::canonicalize(&app_dir).map_err(|source| OpenNtxError::io(&app_dir, source))?;

        // Collect files to include
        let mut files_included = Vec::new();
        collect_bundle_files(&app_dir, &canonical_app, &mut files_included)?;

        let bundle_format = if output_path
            .to_string_lossy()
            .ends_with(".openntx-bundle.zip")
        {
            "zip".to_string()
        } else {
            "tar.gz".to_string()
        };

        if dry_run {
            return Ok(ExportResult {
                app_id: app_id.to_string(),
                bundle_path: output_path.to_path_buf(),
                bundle_format,
                files_included,
            });
        }

        // Create tar.gz bundle
        create_tar_gz_bundle(&app_dir, &canonical_app, output_path)?;

        Ok(ExportResult {
            app_id: app_id.to_string(),
            bundle_path: output_path.to_path_buf(),
            bundle_format,
            files_included,
        })
    }

    /// Import an OpenNTX bundle into the local app registry.
    pub fn import_bundle(
        &self,
        bundle_path: &Path,
        rename_as: Option<&str>,
        dry_run: bool,
    ) -> Result<ImportResult> {
        if !bundle_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "bundle file does not exist: {}",
                bundle_path.display()
            )));
        }

        // Read the manifest from the bundle to get app_id
        let app_id_from_bundle = extract_app_id_from_bundle(bundle_path)?;
        let target_app_id = rename_as.unwrap_or(&app_id_from_bundle);
        validate_app_id(target_app_id)?;

        if rename_as.is_none() && self.paths.app_dir(&app_id_from_bundle).exists() {
            return Err(OpenNtxError::AlreadyExists(format!(
                "app already exists: {app_id_from_bundle}. Use --as to import under a different id."
            )));
        }

        if rename_as.is_some() && self.paths.app_dir(target_app_id).exists() {
            return Err(OpenNtxError::AlreadyExists(format!(
                "target app already exists: {target_app_id}"
            )));
        }

        let app_dir = self.paths.app_dir(target_app_id);

        if dry_run {
            return Ok(ImportResult {
                app_id: target_app_id.to_string(),
                app_dir,
                manifest_path: self.paths.manifest_path(target_app_id),
            });
        }

        // Extract bundle
        extract_tar_gz_bundle(bundle_path, &app_dir)?;

        // Update manifest app_id if renamed
        if rename_as.is_some() {
            let manifest_path = self.paths.manifest_path(target_app_id);
            let mut manifest = read_manifest(&manifest_path)?;
            manifest.app_id = target_app_id.to_string();
            write_manifest_pretty(&manifest_path, &manifest)?;
        }

        // Validate manifest
        let manifest_path = self.paths.manifest_path(target_app_id);
        let manifest = read_manifest(&manifest_path)?;
        crate::manifest::validate_manifest(&manifest)?;

        Ok(ImportResult {
            app_id: target_app_id.to_string(),
            app_dir,
            manifest_path,
        })
    }

    /// List apps as JSON.
    pub fn list_apps_json(&self) -> Result<Vec<RegisteredAppJson>> {
        let apps = self.list_apps()?;
        Ok(apps
            .into_iter()
            .map(|app| RegisteredAppJson {
                app_id: app.app_id,
                name: app.name,
                architecture: app.architecture,
                install_mode: app.install_mode,
                desktop_status: if app.desktop_launcher_exists {
                    "present".to_string()
                } else {
                    "missing".to_string()
                },
            })
            .collect())
    }

    /// Show app summary as JSON.
    pub fn show_app_json(&self, app_id: &str) -> Result<AppSummaryJson> {
        validate_app_id(app_id)?;
        let manifest = self.load_manifest(app_id)?;
        let desktop_entry = self.paths.desktop_entry_path(app_id);
        let desktop_status = if desktop_entry.exists() {
            "present".to_string()
        } else {
            "missing".to_string()
        };

        Ok(AppSummaryJson {
            app_id: manifest.app_id,
            name: manifest.name,
            version: manifest.version,
            architecture: manifest.architecture,
            install_mode: manifest.install_mode,
            executable_path: manifest.executable.path,
            sandbox_profile: manifest.sandbox.profile,
            imported_dll_count: manifest.diagnostics.imported_dlls.len(),
            desktop_status,
            desktop_name: manifest.desktop.name,
            source_file: manifest.source.original_file,
            status: "registered / analysis-only".to_string(),
        })
    }
}

fn validate_app_id(app_id: &str) -> Result<()> {
    if is_valid_app_id(app_id) {
        Ok(())
    } else {
        Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )))
    }
}

fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }
    let json = serde_json::to_vec_pretty(value)?;
    fs::write(path, json).map_err(|source| OpenNtxError::io(path, source))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Copy safe contents from source to dest, rejecting symlinks that escape.
fn copy_safe_contents(source: &Path, dest: &Path, canonical_source: &Path) -> Result<()> {
    let entries =
        fs::read_dir(source).map_err(|source_err| OpenNtxError::io(source, source_err))?;

    for entry in entries {
        let entry = entry.map_err(|source_err| OpenNtxError::io(source, source_err))?;
        let src_path = entry.path();
        let meta = fs::symlink_metadata(&src_path)
            .map_err(|source_err| OpenNtxError::io(&src_path, source_err))?;

        if meta.file_type().is_symlink() {
            let canonical = fs::canonicalize(&src_path)
                .map_err(|source_err| OpenNtxError::io(&src_path, source_err))?;
            if !canonical.starts_with(canonical_source) {
                return Err(OpenNtxError::UnsafePath(format!(
                    "symlink {} escapes app directory",
                    src_path.display()
                )));
            }
        }

        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        if meta.is_dir() && !meta.file_type().is_symlink() {
            fs::create_dir_all(&dest_path)
                .map_err(|source_err| OpenNtxError::io(&dest_path, source_err))?;
            copy_safe_contents(&src_path, &dest_path, canonical_source)?;
        } else if meta.is_file() && !meta.file_type().is_symlink() {
            fs::copy(&src_path, &dest_path)
                .map_err(|source_err| OpenNtxError::io(&dest_path, source_err))?;
        }
    }
    Ok(())
}

/// Collect files for bundle export, rejecting unsafe symlinks.
fn collect_bundle_files(dir: &Path, canonical_base: &Path, files: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|source| OpenNtxError::io(dir, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(dir, source))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|source| OpenNtxError::io(&path, source))?;

        if meta.file_type().is_symlink() {
            let canonical =
                fs::canonicalize(&path).map_err(|source| OpenNtxError::io(&path, source))?;
            if !canonical.starts_with(canonical_base) {
                return Err(OpenNtxError::UnsafePath(format!(
                    "symlink {} escapes app directory",
                    path.display()
                )));
            }
            continue; // Don't include symlinks in bundle
        }

        if meta.is_dir() {
            collect_bundle_files(&path, canonical_base, files)?;
        } else if meta.is_file() {
            if let Ok(relative) = path.strip_prefix(canonical_base) {
                files.push(relative.display().to_string());
            }
        }
    }
    Ok(())
}

/// Create a tar.gz bundle from the app directory.
fn create_tar_gz_bundle(app_dir: &Path, _canonical_base: &Path, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }

    let output_file =
        fs::File::create(output_path).map_err(|source| OpenNtxError::io(output_path, source))?;
    let enc = flate2::write::GzEncoder::new(output_file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_dir_all(".", app_dir)
        .map_err(|source| OpenNtxError::io(app_dir, source))?;

    let enc = tar
        .into_inner()
        .map_err(|source| OpenNtxError::io(output_path, source))?;
    enc.finish()
        .map_err(|source| OpenNtxError::io(output_path, source))?;

    Ok(())
}

/// Extract app_id from a bundle by reading the manifest inside.
fn extract_app_id_from_bundle(bundle_path: &Path) -> Result<String> {
    let file =
        fs::File::open(bundle_path).map_err(|source| OpenNtxError::io(bundle_path, source))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    for entry in archive
        .entries()
        .map_err(|source| OpenNtxError::io(bundle_path, source))?
    {
        let entry = entry.map_err(|source| OpenNtxError::io(bundle_path, source))?;
        let path = entry
            .path()
            .map_err(|source| OpenNtxError::io(bundle_path, source))?;

        // Check for unsafe paths
        if path.to_string_lossy().contains("..") {
            return Err(OpenNtxError::UnsafePath(format!(
                "bundle contains unsafe path: {}",
                path.display()
            )));
        }

        if path.to_string_lossy().ends_with("manifest.json") {
            let bytes = entry
                .bytes()
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|source| OpenNtxError::io(bundle_path, source))?;
            let manifest: crate::manifest::AppManifest = serde_json::from_slice(&bytes)?;
            return Ok(manifest.app_id);
        }
    }

    Err(OpenNtxError::InvalidInput(
        "bundle does not contain a manifest.json".to_string(),
    ))
}

/// Extract a tar.gz bundle into the app directory.
fn extract_tar_gz_bundle(bundle_path: &Path, dest: &Path) -> Result<()> {
    let file =
        fs::File::open(bundle_path).map_err(|source| OpenNtxError::io(bundle_path, source))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    // First pass: validate all paths
    for entry in archive
        .entries()
        .map_err(|source| OpenNtxError::io(bundle_path, source))?
    {
        let entry = entry.map_err(|source| OpenNtxError::io(bundle_path, source))?;
        let path = entry
            .path()
            .map_err(|source| OpenNtxError::io(bundle_path, source))?;

        if path.to_string_lossy().contains("..") {
            return Err(OpenNtxError::UnsafePath(format!(
                "bundle contains unsafe path traversal: {}",
                path.display()
            )));
        }
    }

    // Second pass: extract
    let file =
        fs::File::open(bundle_path).map_err(|source| OpenNtxError::io(bundle_path, source))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    fs::create_dir_all(dest).map_err(|source| OpenNtxError::io(dest, source))?;

    for entry in archive
        .entries()
        .map_err(|source| OpenNtxError::io(bundle_path, source))?
    {
        let mut entry = entry.map_err(|source| OpenNtxError::io(bundle_path, source))?;
        let path = entry
            .path()
            .map_err(|source| OpenNtxError::io(bundle_path, source))?;

        // Check for symlinks in the archive
        if entry.header().entry_type().is_symlink() {
            return Err(OpenNtxError::UnsafePath(format!(
                "bundle contains symlink: {}",
                path.display()
            )));
        }

        entry
            .unpack_in(dest)
            .map_err(|source| OpenNtxError::io(dest, source))?;
    }

    Ok(())
}
