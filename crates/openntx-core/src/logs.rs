use crate::registry::AppRegistry;
use crate::runtime::RunPlanReport;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A summary of a run-plan log file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummary {
    pub file_name: String,
    pub file_path: String,
    pub app_id: String,
    pub app_name: String,
    pub timestamp: String,
    pub created_at_unix: u64,
    pub status: String,
}

/// List all run-plan logs from the logs directory.
pub fn list_logs(registry: &AppRegistry) -> Result<Vec<LogSummary>> {
    let logs_root = &registry.paths().logs_root;
    if !logs_root.exists() {
        return Ok(Vec::new());
    }

    let mut logs = Vec::new();
    let entries = fs::read_dir(logs_root).map_err(|source| OpenNtxError::io(logs_root, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(logs_root, source))?;
        let path = entry.path();

        // Only process regular files with .json extension
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if !file_name.ends_with(".json") || !file_name.starts_with("run-plan-") {
            continue;
        }

        match read_log_summary(&path) {
            Ok(summary) => logs.push(summary),
            Err(_) => continue,
        }
    }

    logs.sort_by(|a, b| b.created_at_unix.cmp(&a.created_at_unix));
    Ok(logs)
}

/// Show a specific log by path or by app_id (latest log for that app).
pub fn show_log(registry: &AppRegistry, target: &str) -> Result<RunPlanReport> {
    let path = Path::new(target);
    if path.exists() {
        // Direct path
        let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
        let report: RunPlanReport = serde_json::from_slice(&bytes)?;
        return Ok(report);
    }

    // Try as app_id - find latest log
    let logs = list_logs(registry)?;
    let app_log = logs
        .iter()
        .find(|l| l.app_id == target)
        .ok_or_else(|| OpenNtxError::AppNotFound(format!("no log found for app: {target}")))?;

    let path = PathBuf::from(&app_log.file_path);
    let bytes = fs::read(&path).map_err(|source| OpenNtxError::io(&path, source))?;
    let report: RunPlanReport = serde_json::from_slice(&bytes)?;
    Ok(report)
}

/// Clean logs older than a given number of days.
/// Returns the list of files that were (or would be) deleted.
pub fn clean_logs(
    registry: &AppRegistry,
    older_than_days: u64,
    dry_run: bool,
) -> Result<Vec<String>> {
    let logs_root = &registry.paths().logs_root;
    if !logs_root.exists() {
        return Ok(Vec::new());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cutoff = now.saturating_sub(older_than_days * 86400);
    let mut deleted = Vec::new();

    let entries = fs::read_dir(logs_root).map_err(|source| OpenNtxError::io(logs_root, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(logs_root, source))?;
        let path = entry.path();

        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if !file_name.ends_with(".json") || !file_name.starts_with("run-plan-") {
            continue;
        }

        // Check if the log is old enough
        if let Ok(summary) = read_log_summary(&path) {
            if summary.created_at_unix < cutoff {
                // Verify the file is inside the logs directory (safety check)
                if let Ok(canonical_path) = fs::canonicalize(&path) {
                    if let Ok(canonical_root) = fs::canonicalize(logs_root) {
                        if canonical_path.starts_with(&canonical_root) {
                            if !dry_run {
                                let _ = fs::remove_file(&path);
                            }
                            deleted.push(file_name);
                        }
                    }
                }
            }
        }
    }

    Ok(deleted)
}

fn read_log_summary(path: &Path) -> Result<LogSummary> {
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let report: RunPlanReport = serde_json::from_slice(&bytes)?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    Ok(LogSummary {
        file_name,
        file_path: path.display().to_string(),
        app_id: report.app_id,
        app_name: report.name,
        timestamp: report.timestamp,
        created_at_unix: report.created_at_unix,
        status: report.status,
    })
}
