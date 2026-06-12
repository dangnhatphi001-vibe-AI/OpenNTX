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
    pub timestamp: String,
    pub target: String,
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
    let timestamp = format_utc_timestamp(created_at_unix);

    let mut report = RunPlanReport {
        schema_version: "0.1.0".to_string(),
        created_at_unix,
        timestamp,
        target: app_id.to_string(),
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

fn format_utc_timestamp(unix_secs: u64) -> String {
    let secs_per_day: u64 = 86400;
    let days = unix_secs / secs_per_day;
    let rem = unix_secs % secs_per_day;
    let hours = rem / 3600;
    let minutes = (rem % 3600) / 60;
    let seconds = rem % 60;

    // Days since 1970-01-01 to Y-M-D (civil calendar)
    let (year, month, day) = days_to_civil(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn days_to_civil(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant (public domain)
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
