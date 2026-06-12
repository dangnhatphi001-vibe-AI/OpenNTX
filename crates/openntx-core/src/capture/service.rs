use std::fs;
use std::path::{Path, PathBuf};

use crate::capture::diff::{compute_diff, CaptureDiff};
use crate::capture::report_writer::{write_capture_report, CaptureReport};
use crate::capture::snapshot::{create_snapshot, Snapshot};
use crate::registry::AppRegistry;
use crate::{OpenNtxError, Result};

/// Centralized capture orchestration service.
///
/// CLI and AppPortal must call this service rather than duplicating
/// snapshot/diff/report write logic.
pub struct CaptureRegistryService {
    registry: AppRegistry,
}

#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub snapshot: Snapshot,
    pub snapshot_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub diff: CaptureDiff,
    pub diff_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ReportResult {
    pub report: CaptureReport,
    pub diff: CaptureDiff,
    pub diff_path: PathBuf,
    pub report_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CaptureStatus {
    pub app_id: String,
    pub app_name: String,
    pub app_dir: PathBuf,
    pub capture_dir: PathBuf,
    pub snapshot_before: bool,
    pub snapshot_after: bool,
    pub diff: bool,
    pub report: bool,
}

impl CaptureRegistryService {
    pub fn new(registry: AppRegistry) -> Self {
        Self { registry }
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self::new(AppRegistry::from_env()?))
    }

    pub fn registry(&self) -> &AppRegistry {
        &self.registry
    }

    /// Take a snapshot of the current app directory state.
    /// Validates app_id, creates capture dir securely, writes snapshot.
    pub fn snapshot_before(&self, app_id: &str) -> Result<SnapshotResult> {
        self.registry.load_manifest(app_id)?;
        let app_dir = self.registry.paths().app_dir(app_id);
        let capture_dir = self.ensure_capture_dir(app_id)?;
        let snapshot_path = capture_dir.join("snapshot-before.json");

        let snapshot = create_snapshot(app_id, &app_dir)?;
        self.secure_write(&snapshot_path, &snapshot)?;

        Ok(SnapshotResult {
            snapshot,
            snapshot_path,
        })
    }

    /// Take a snapshot after a capture step. Requires snapshot-before to exist.
    pub fn snapshot_after(&self, app_id: &str) -> Result<SnapshotResult> {
        self.registry.load_manifest(app_id)?;
        let app_dir = self.registry.paths().app_dir(app_id);
        let capture_dir = self.ensure_capture_dir(app_id)?;
        let before_path = capture_dir.join("snapshot-before.json");

        if !before_path.exists() {
            return Err(OpenNtxError::InvalidInput(
                "snapshot-before.json not found. Run 'openntx capture snapshot-before' first."
                    .to_string(),
            ));
        }

        let snapshot = create_snapshot(app_id, &app_dir)?;
        let after_path = capture_dir.join("snapshot-after.json");
        self.secure_write(&after_path, &snapshot)?;

        Ok(SnapshotResult {
            snapshot,
            snapshot_path: after_path,
        })
    }

    /// Compute diff between before and after snapshots.
    pub fn diff(&self, app_id: &str) -> Result<DiffResult> {
        self.registry.load_manifest(app_id)?;
        let capture_dir = self.ensure_capture_dir(app_id)?;
        let before_path = capture_dir.join("snapshot-before.json");
        let after_path = capture_dir.join("snapshot-after.json");

        if !before_path.exists() {
            return Err(OpenNtxError::InvalidInput(
                "snapshot-before.json not found. Run 'openntx capture snapshot-before' first."
                    .to_string(),
            ));
        }
        if !after_path.exists() {
            return Err(OpenNtxError::InvalidInput(
                "snapshot-after.json not found. Run 'openntx capture snapshot-after' first."
                    .to_string(),
            ));
        }

        let before = self.read_snapshot(&before_path)?;
        let after = self.read_snapshot(&after_path)?;
        let diff = compute_diff(&before, &after)?;

        let diff_path = capture_dir.join("capture-diff.json");
        self.secure_write(&diff_path, &diff)?;

        Ok(DiffResult { diff, diff_path })
    }

    /// Generate a capture report. Computes diff if not already present.
    pub fn report(&self, app_id: &str) -> Result<ReportResult> {
        let manifest = self.registry.load_manifest(app_id)?;
        let capture_dir = self.ensure_capture_dir(app_id)?;
        let before_path = capture_dir.join("snapshot-before.json");
        let after_path = capture_dir.join("snapshot-after.json");

        if !before_path.exists() {
            return Err(OpenNtxError::InvalidInput(
                "snapshot-before.json not found. Run 'openntx capture snapshot-before' first."
                    .to_string(),
            ));
        }
        if !after_path.exists() {
            return Err(OpenNtxError::InvalidInput(
                "snapshot-after.json not found. Run 'openntx capture snapshot-after' first."
                    .to_string(),
            ));
        }

        let before = self.read_snapshot(&before_path)?;
        let after = self.read_snapshot(&after_path)?;
        let diff = compute_diff(&before, &after)?;

        let diff_path = capture_dir.join("capture-diff.json");
        self.secure_write(&diff_path, &diff)?;

        let report_path = capture_dir.join("capture-report.json");
        let report = write_capture_report(
            &report_path,
            app_id,
            &diff,
            &before,
            &after,
            Some(&manifest),
        )?;

        Ok(ReportResult {
            report,
            diff,
            diff_path,
            report_path,
        })
    }

    /// Show capture status for a registered app.
    pub fn status(&self, app_id: &str) -> Result<CaptureStatus> {
        let manifest = self.registry.load_manifest(app_id)?;
        let app_dir = self.registry.paths().app_dir(app_id);
        let capture_dir = self.registry.paths().capture_dir(app_id);

        Ok(CaptureStatus {
            app_id: app_id.to_string(),
            app_name: manifest.name,
            app_dir,
            capture_dir: capture_dir.clone(),
            snapshot_before: capture_dir.join("snapshot-before.json").exists(),
            snapshot_after: capture_dir.join("snapshot-after.json").exists(),
            diff: capture_dir.join("capture-diff.json").exists(),
            report: capture_dir.join("capture-report.json").exists(),
        })
    }

    /// Ensure capture directory exists and is a real directory (not a symlink).
    fn ensure_capture_dir(&self, app_id: &str) -> Result<PathBuf> {
        let app_dir = self.registry.paths().app_dir(app_id);
        let capture_dir = self.registry.paths().capture_dir(app_id);

        // Verify app_dir is canonical
        let canonical_app =
            fs::canonicalize(&app_dir).map_err(|source| OpenNtxError::io(&app_dir, source))?;

        if capture_dir.exists() {
            // If it exists, verify it's not a symlink
            let meta = fs::symlink_metadata(&capture_dir)
                .map_err(|source| OpenNtxError::io(&capture_dir, source))?;
            if meta.file_type().is_symlink() {
                return Err(OpenNtxError::InvalidInput(format!(
                    "capture directory is a symlink; refusing to write artifacts: {}",
                    capture_dir.display()
                )));
            }

            // Verify canonical path stays inside app dir
            let canonical_capture = fs::canonicalize(&capture_dir)
                .map_err(|source| OpenNtxError::io(&capture_dir, source))?;
            if !canonical_capture.starts_with(&canonical_app) {
                return Err(OpenNtxError::InvalidInput(format!(
                    "capture directory resolves outside app directory: {}",
                    capture_dir.display()
                )));
            }
        } else {
            // Create it — parent must be real app dir
            fs::create_dir_all(&capture_dir)
                .map_err(|source| OpenNtxError::io(&capture_dir, source))?;

            // Verify the created directory is inside app dir
            let canonical_capture = fs::canonicalize(&capture_dir)
                .map_err(|source| OpenNtxError::io(&capture_dir, source))?;
            if !canonical_capture.starts_with(&canonical_app) {
                return Err(OpenNtxError::InvalidInput(format!(
                    "created capture directory resolves outside app directory: {}",
                    capture_dir.display()
                )));
            }
        }

        Ok(capture_dir)
    }

    /// Securely write a JSON artifact, validating the target path.
    fn secure_write<T: serde::Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        // Validate parent is inside the capture directory
        if let Some(parent) = path.parent() {
            let canonical_parent =
                fs::canonicalize(parent).map_err(|source| OpenNtxError::io(parent, source))?;
            let app_id_from_path = extract_app_id_from_capture_path(&canonical_parent);
            if let Some(app_id) = app_id_from_path {
                let canonical_app = self.registry.paths().app_dir(&app_id);
                let canonical_app = fs::canonicalize(&canonical_app)
                    .map_err(|source| OpenNtxError::io(&canonical_app, source))?;
                if !canonical_parent.starts_with(&canonical_app) {
                    return Err(OpenNtxError::InvalidInput(format!(
                        "refusing to write artifact outside app directory: {}",
                        path.display()
                    )));
                }
            }
        }

        let json = serde_json::to_vec_pretty(value)?;
        fs::write(path, json).map_err(|source| OpenNtxError::io(path, source))
    }

    fn read_snapshot(&self, path: &Path) -> Result<Snapshot> {
        let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
        let snapshot: Snapshot = serde_json::from_slice(&bytes)?;
        Ok(snapshot)
    }
}

/// Try to extract an app-id from a capture directory path.
/// Looks for .../apps/<app-id>/capture in the path.
fn extract_app_id_from_capture_path(path: &Path) -> Option<String> {
    let components: Vec<_> = path.components().collect();
    for i in 0..components.len() {
        if let Some(name) = components[i].as_os_str().to_str() {
            if name == "apps" && i + 2 < components.len() {
                if let Some(capture_name) = components[i + 2].as_os_str().to_str() {
                    if capture_name == "capture" {
                        return components[i + 1].as_os_str().to_str().map(String::from);
                    }
                }
            }
        }
    }
    None
}
