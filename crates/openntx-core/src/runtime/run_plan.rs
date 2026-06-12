use crate::registry::AppRegistry;
use crate::runtime::{NotImplementedBackend, RuntimeBackend};
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPlanReport {
    pub schema_version: String,
    pub created_at_unix: u64,
    pub app_id: String,
    pub name: String,
    pub executable_path: String,
    pub architecture: String,
    pub install_mode: String,
    pub sandbox_profile: String,
    pub imported_dll_count: usize,
    pub desktop_status: String,
    pub desktop_entry_path: String,
    pub backend: String,
    pub status: String,
    pub message: String,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlanOptions {
    pub write_log: bool,
}

impl Default for RunPlanOptions {
    fn default() -> Self {
        Self { write_log: true }
    }
}

pub fn create_registered_run_plan(
    registry: &AppRegistry,
    app_id: &str,
    options: &RunPlanOptions,
) -> Result<RunPlanReport> {
    let manifest = registry.load_manifest(app_id)?;
    let desktop_entry_path = registry.paths().desktop_entry_path(app_id);
    let desktop_status = if desktop_entry_path.exists() {
        "present"
    } else {
        "missing"
    };
    let backend = NotImplementedBackend;
    let runtime_plan = backend.plan_execution(&manifest);
    let created_at_unix = unix_now()?;

    let mut report = RunPlanReport {
        schema_version: "0.1.0".to_string(),
        created_at_unix,
        app_id: manifest.app_id,
        name: manifest.name,
        executable_path: manifest.executable.path,
        architecture: manifest.architecture,
        install_mode: manifest.install_mode,
        sandbox_profile: manifest.sandbox.profile,
        imported_dll_count: manifest.diagnostics.imported_dlls.len(),
        desktop_status: desktop_status.to_string(),
        desktop_entry_path: desktop_entry_path.display().to_string(),
        backend: runtime_plan.backend,
        status: "dry-run / not implemented".to_string(),
        message: runtime_plan.message,
        log_path: None,
    };

    if options.write_log {
        let log_path = registry.paths().logs_root.join(format!(
            "run-plan-{}-{}.json",
            report.app_id,
            unix_now_nanos()?
        ));
        report.log_path = Some(log_path.display().to_string());
        write_run_plan_log(&log_path, &report)?;
    }

    Ok(report)
}

pub fn write_run_plan_log(path: &Path, report: &RunPlanReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }
    let json = serde_json::to_vec_pretty(report)?;
    fs::write(path, json).map_err(|source| OpenNtxError::io(path, source))
}

fn unix_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| OpenNtxError::InvalidInput(format!("system clock before epoch: {error}")))?
        .as_secs())
}

fn unix_now_nanos() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| OpenNtxError::InvalidInput(format!("system clock before epoch: {error}")))?
        .as_nanos())
}
