use crate::app_id::is_valid_app_id;
use crate::manifest::{read_manifest, write_manifest_pretty, AppManifest};
use crate::paths::OpenNtxPaths;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
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
    pub manifest_path: PathBuf,
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
            if !manifest_path.exists() {
                continue;
            }
            let manifest = read_manifest(&manifest_path)?;
            apps.push(RegisteredApp {
                app_id: manifest.app_id,
                name: manifest.name,
                install_mode: manifest.install_mode,
                architecture: manifest.architecture,
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
