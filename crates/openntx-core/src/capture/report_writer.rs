use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::capture::diff::CaptureDiff;
use crate::capture::snapshot::Snapshot;
use crate::manifest::AppManifest;
use crate::{OpenNtxError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureReport {
    pub schema_version: String,
    pub capture_id: String,
    pub app_id: String,
    pub app_name: String,
    pub input_installer: String,
    pub started_at: String,
    pub finished_at: String,
    pub files_created: Vec<String>,
    pub files_modified: Vec<String>,
    pub files_removed: Vec<String>,
    pub registry_keys_created: Vec<String>,
    pub registry_values_changed: Vec<String>,
    pub shortcuts_detected: Vec<ShortcutEntry>,
    pub executable_candidates: Vec<ExecutableCandidate>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directories_created: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directories_removed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry_files_changed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutEntry {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutableCandidate {
    pub path: String,
    pub score: f64,
    pub reason: String,
}

pub fn write_capture_report(
    path: &Path,
    app_id: &str,
    diff: &CaptureDiff,
    before: &Snapshot,
    after: &Snapshot,
    manifest: Option<&AppManifest>,
) -> Result<CaptureReport> {
    let mut warnings = diff.warnings.clone();
    let errors = diff.errors.clone();

    let app_name = manifest
        .map(|m| m.name.clone())
        .unwrap_or_else(|| app_id.to_string());

    let input_installer = manifest
        .map(|m| m.source.original_file.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let registry_keys_created: Vec<String> = diff
        .registry_files_changed
        .iter()
        .filter(|f| {
            diff.files_created
                .iter()
                .any(|c| c.relative_path == f.relative_path)
        })
        .map(|f| f.relative_path.clone())
        .collect();

    let registry_values_changed: Vec<String> = diff
        .registry_files_changed
        .iter()
        .filter(|f| {
            diff.files_modified
                .iter()
                .any(|m| m.relative_path == f.relative_path)
        })
        .map(|f| f.relative_path.clone())
        .collect();

    warnings.push(
        "Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed."
            .to_string(),
    );

    let directories_created: Vec<String> = diff
        .directories_created
        .iter()
        .map(|d| d.relative_path.clone())
        .collect();
    let directories_removed: Vec<String> = diff
        .directories_removed
        .iter()
        .map(|d| d.relative_path.clone())
        .collect();
    let registry_files_changed: Vec<String> = diff
        .registry_files_changed
        .iter()
        .map(|f| f.relative_path.clone())
        .collect();

    let report = CaptureReport {
        schema_version: "0.2.0".to_string(),
        capture_id: format!("capture-{app_id}"),
        app_id: app_id.to_string(),
        app_name,
        input_installer,
        started_at: before.created_at.clone(),
        finished_at: after.created_at.clone(),
        files_created: diff
            .files_created
            .iter()
            .map(|f| f.relative_path.clone())
            .collect(),
        files_modified: diff
            .files_modified
            .iter()
            .map(|f| f.relative_path.clone())
            .collect(),
        files_removed: diff
            .files_removed
            .iter()
            .map(|f| f.relative_path.clone())
            .collect(),
        registry_keys_created,
        registry_values_changed,
        shortcuts_detected: Vec::new(),
        executable_candidates: Vec::new(),
        warnings,
        errors,
        status: "analysis-only / no runtime execution".to_string(),
        directories_created,
        directories_removed,
        registry_files_changed,
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }
    let json = serde_json::to_vec_pretty(&report)?;
    fs::write(path, json).map_err(|source| OpenNtxError::io(path, source))?;

    Ok(report)
}
