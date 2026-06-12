use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::{OpenNtxError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub relative_path: String,
    pub kind: String,
    pub size: u64,
    pub modified_unix: Option<u64>,
    pub sha256: Option<String>,
    pub readonly: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: String,
    pub app_id: String,
    pub root: String,
    pub created_at: String,
    pub entries: Vec<SnapshotEntry>,
    pub errors: Vec<String>,
}

/// Create a filesystem snapshot of the OpenNTX app directory.
///
/// Security rules:
/// - Only scans `drive_c/`, `registry/`, and optional metadata files.
/// - Uses `symlink_metadata()` everywhere — never follows symlinks.
/// - Rejects top-level `drive_c`, `registry`, `capture` if they are symlinks.
/// - Rejects optional metadata files if they are symlinks.
/// - For regular files, re-checks file type immediately before hashing.
/// - Validates all scanned paths stay inside the canonical app directory.
pub fn create_snapshot(app_id: &str, app_dir: &Path) -> Result<Snapshot> {
    let canonical_app_dir =
        fs::canonicalize(app_dir).map_err(|source| OpenNtxError::io(app_dir, source))?;

    // Verify app_dir itself is a real directory (not a symlink)
    let app_meta =
        fs::symlink_metadata(app_dir).map_err(|source| OpenNtxError::io(app_dir, source))?;
    if app_meta.file_type().is_symlink() {
        return Err(OpenNtxError::InvalidInput(format!(
            "app directory is a symlink: {}",
            app_dir.display()
        )));
    }
    if !app_meta.is_dir() {
        return Err(OpenNtxError::InvalidInput(format!(
            "app directory does not exist or is not a directory: {}",
            app_dir.display()
        )));
    }

    let mut entries = Vec::new();
    let mut errors = Vec::new();

    // Scan drive_c and registry — reject if they are symlinks
    let snapshot_roots = ["drive_c", "registry"];
    for sub_name in &snapshot_roots {
        let sub_path = app_dir.join(sub_name);
        match fs::symlink_metadata(&sub_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    errors.push(format!("{sub_name} is a symlink; skipped for security"));
                    continue;
                }
                if meta.is_dir() {
                    let canonical_sub = fs::canonicalize(&sub_path)
                        .map_err(|source| OpenNtxError::io(&sub_path, source));
                    match canonical_sub {
                        Ok(canon) => walk_directory(
                            &sub_path,
                            sub_name,
                            &canon,
                            &canonical_app_dir,
                            &mut entries,
                            &mut errors,
                        ),
                        Err(err) => {
                            errors
                                .push(format!("failed to canonicalize {sub_name}: {err}; skipped"));
                        }
                    }
                }
            }
            Err(_) => {
                // Directory does not exist — not an error, just absent
            }
        }
    }

    // Scan optional metadata files — reject if they are symlinks
    let optional_files = ["metadata.json", "manifest.json", "install-plan.json"];
    for file_name in &optional_files {
        let file_path = app_dir.join(file_name);
        match fs::symlink_metadata(&file_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    errors.push(format!("{file_name} is a symlink; skipped for security"));
                    continue;
                }
                if meta.is_file() {
                    collect_file_entry_inner(
                        &file_path,
                        file_name,
                        &meta,
                        &canonical_app_dir,
                        &mut entries,
                        &mut errors,
                    );
                }
            }
            Err(_) => {
                // File does not exist — not an error
            }
        }
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(Snapshot {
        schema_version: "0.2.0".to_string(),
        app_id: app_id.to_string(),
        root: app_dir.display().to_string(),
        created_at: crate::runtime::run_plan::utc_now_iso8601(),
        entries,
        errors,
    })
}

fn walk_directory(
    current: &Path,
    relative_prefix: &str,
    strip_root: &Path,
    security_root: &Path,
    entries: &mut Vec<SnapshotEntry>,
    errors: &mut Vec<String>,
) {
    let read_dir = match fs::read_dir(current) {
        Ok(rd) => rd,
        Err(err) => {
            errors.push(format!(
                "failed to read directory {}: {err}",
                current.display()
            ));
            return;
        }
    };

    for entry_result in read_dir {
        let entry = match entry_result {
            Ok(e) => e,
            Err(err) => {
                errors.push(format!(
                    "failed to read entry in {}: {err}",
                    current.display()
                ));
                continue;
            }
        };

        let path = entry.path();

        let relative = match path.strip_prefix(strip_root) {
            Ok(rel) => {
                format!("{relative_prefix}/{}", rel.display())
            }
            Err(_) => {
                let file_name = entry.file_name().to_string_lossy().to_string();
                format!("{relative_prefix}/{file_name}")
            }
        };

        // Always use symlink_metadata — never follow symlinks
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                errors.push(format!(
                    "failed to read metadata for {}: {err}",
                    path.display()
                ));
                continue;
            }
        };

        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            // Validate symlink target stays inside app directory
            if let Ok(target) = fs::read_link(&path) {
                let resolved = if target.is_relative() {
                    path.parent()
                        .map(|p| p.join(&target))
                        .unwrap_or(target.clone())
                } else {
                    target.clone()
                };
                match fs::canonicalize(&resolved) {
                    Ok(canonical_target) => {
                        if !canonical_target.starts_with(security_root) {
                            errors.push(format!(
                                "symlink {relative} points outside app directory; skipped"
                            ));
                            continue;
                        }
                    }
                    Err(_) => {
                        // Target doesn't resolve — record as symlink but warn
                        errors.push(format!(
                            "symlink {relative} target cannot be resolved; recorded without dereferencing"
                        ));
                    }
                }
            }
            entries.push(SnapshotEntry {
                relative_path: relative,
                kind: "symlink".to_string(),
                size: 0,
                modified_unix: modified_as_unix(&metadata),
                sha256: None,
                readonly: None,
            });
        } else if file_type.is_dir() {
            // Validate directory stays inside app root
            match fs::canonicalize(&path) {
                Ok(canonical) => {
                    if !canonical.starts_with(security_root) {
                        errors.push(format!(
                            "directory {relative} resolves outside app directory; skipped"
                        ));
                        continue;
                    }
                }
                Err(err) => {
                    errors.push(format!(
                        "failed to canonicalize directory {}: {err}; skipped",
                        path.display()
                    ));
                    continue;
                }
            }
            entries.push(SnapshotEntry {
                relative_path: relative.clone(),
                kind: "directory".to_string(),
                size: 0,
                modified_unix: modified_as_unix(&metadata),
                sha256: None,
                readonly: Some(metadata.permissions().readonly()),
            });
            walk_directory(&path, &relative, strip_root, security_root, entries, errors);
        } else if file_type.is_file() {
            collect_file_entry_inner(&path, &relative, &metadata, security_root, entries, errors);
        } else {
            entries.push(SnapshotEntry {
                relative_path: relative,
                kind: "other".to_string(),
                size: 0,
                modified_unix: None,
                sha256: None,
                readonly: None,
            });
        }
    }
}

fn collect_file_entry_inner(
    path: &Path,
    relative: &str,
    _metadata: &fs::Metadata,
    canonical_app_root: &Path,
    entries: &mut Vec<SnapshotEntry>,
    errors: &mut Vec<String>,
) {
    // TOCTOU mitigation: re-check with symlink_metadata right before hashing
    let fresh_meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(err) => {
            errors.push(format!(
                "file {relative} vanished or became inaccessible before hashing: {err}"
            ));
            return;
        }
    };

    if fresh_meta.file_type().is_symlink() {
        // File became a symlink between first check and now — skip
        errors.push(format!(
            "file {relative} became a symlink before hashing; skipped for security"
        ));
        return;
    }

    if !fresh_meta.is_file() {
        errors.push(format!(
            "file {relative} is no longer a regular file; skipped"
        ));
        return;
    }

    // Validate path stays inside app root
    match fs::canonicalize(path) {
        Ok(canonical) => {
            if !canonical.starts_with(canonical_app_root) {
                errors.push(format!(
                    "file {relative} resolves outside app directory; skipped"
                ));
                return;
            }
        }
        Err(err) => {
            errors.push(format!("failed to canonicalize {relative}: {err}; skipped"));
            return;
        }
    }

    let sha256 = match compute_sha256(path) {
        Ok(hash) => Some(hash),
        Err(err) => {
            errors.push(format!("failed to compute sha256 for {relative}: {err}"));
            None
        }
    };

    entries.push(SnapshotEntry {
        relative_path: relative.to_string(),
        kind: "file".to_string(),
        size: fresh_meta.len(),
        modified_unix: modified_as_unix(&fresh_meta),
        sha256,
        readonly: Some(fresh_meta.permissions().readonly()),
    });
}

fn compute_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

fn modified_as_unix(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn snapshot_from_temp_dir() {
        let root = temp_app_dir("snap-basic");
        let drive_c = root.join("drive_c");
        fs::create_dir_all(&drive_c).unwrap();
        fs::write(drive_c.join("test.txt"), b"hello world").unwrap();

        let snapshot = create_snapshot("test-app", &root).unwrap();
        assert_eq!(snapshot.app_id, "test-app");
        assert!(snapshot.errors.is_empty());

        let test_entry = snapshot
            .entries
            .iter()
            .find(|e| e.relative_path == "drive_c/test.txt");
        assert!(test_entry.is_some());
        let entry = test_entry.unwrap();
        assert_eq!(entry.kind, "file");
        assert_eq!(entry.size, 11);
        assert!(entry.sha256.is_some());
    }

    #[test]
    fn snapshot_captures_registry_dir() {
        let root = temp_app_dir("snap-registry");
        let registry = root.join("registry");
        fs::create_dir_all(&registry).unwrap();
        fs::write(registry.join("user.reg"), b"REG DATA").unwrap();

        let snapshot = create_snapshot("reg-app", &root).unwrap();
        let reg_entry = snapshot
            .entries
            .iter()
            .find(|e| e.relative_path == "registry/user.reg");
        assert!(reg_entry.is_some());
    }

    #[test]
    fn snapshot_captures_optional_files() {
        let root = temp_app_dir("snap-optional");
        fs::write(root.join("manifest.json"), b"{}").unwrap();
        fs::write(root.join("metadata.json"), b"{}").unwrap();
        fs::write(root.join("install-plan.json"), b"{}").unwrap();

        let snapshot = create_snapshot("opt-app", &root).unwrap();
        let has_manifest = snapshot
            .entries
            .iter()
            .any(|e| e.relative_path == "manifest.json");
        let has_metadata = snapshot
            .entries
            .iter()
            .any(|e| e.relative_path == "metadata.json");
        let has_plan = snapshot
            .entries
            .iter()
            .any(|e| e.relative_path == "install-plan.json");
        assert!(has_manifest);
        assert!(has_metadata);
        assert!(has_plan);
    }

    #[test]
    fn snapshot_errors_on_missing_dir() {
        let root = PathBuf::from("/nonexistent/path");
        let result = create_snapshot("missing", &root);
        assert!(result.is_err());
    }

    #[test]
    fn snapshot_entry_fields_are_correct() {
        let root = temp_app_dir("snap-fields");
        let drive_c = root.join("drive_c");
        fs::create_dir_all(&drive_c).unwrap();
        fs::write(drive_c.join("data.bin"), b"abc123").unwrap();

        let snapshot = create_snapshot("fields-app", &root).unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|e| e.relative_path == "drive_c/data.bin")
            .unwrap();
        assert_eq!(entry.kind, "file");
        assert_eq!(entry.size, 6);
        assert!(entry.sha256.is_some());
        assert!(entry.modified_unix.is_some());
    }

    #[test]
    fn snapshot_rejects_drive_c_symlink() {
        let root = temp_app_dir("snap-drivec-symlink");
        let outside = temp_app_dir("snap-outside-dc");
        fs::write(outside.join("secret.txt"), b"secret").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&outside, root.join("drive_c")).unwrap();
        }

        let snapshot = create_snapshot("test-app", &root).unwrap();
        assert!(
            snapshot
                .errors
                .iter()
                .any(|e| e.contains("drive_c") && e.contains("symlink")),
            "should reject drive_c symlink"
        );
        assert!(
            !snapshot
                .entries
                .iter()
                .any(|e| e.relative_path.contains("secret")),
            "should not read through drive_c symlink"
        );
    }

    #[test]
    fn snapshot_rejects_registry_symlink() {
        let root = temp_app_dir("snap-reg-symlink");
        let outside = temp_app_dir("snap-outside-reg");
        fs::write(outside.join("secret.reg"), b"secret").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&outside, root.join("registry")).unwrap();
        }

        let snapshot = create_snapshot("test-app", &root).unwrap();
        assert!(
            snapshot
                .errors
                .iter()
                .any(|e| e.contains("registry") && e.contains("symlink")),
            "should reject registry symlink"
        );
        assert!(
            !snapshot
                .entries
                .iter()
                .any(|e| e.relative_path.contains("secret")),
            "should not read through registry symlink"
        );
    }

    #[test]
    fn snapshot_rejects_manifest_symlink() {
        let root = temp_app_dir("snap-manifest-symlink");
        let outside = temp_app_dir("snap-outside-manifest");
        fs::write(outside.join("manifest.json"), b"{}").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(outside.join("manifest.json"), root.join("manifest.json")).unwrap();
        }

        let snapshot = create_snapshot("test-app", &root).unwrap();
        assert!(
            snapshot
                .errors
                .iter()
                .any(|e| e.contains("manifest.json") && e.contains("symlink")),
            "should reject manifest.json symlink"
        );
        assert!(
            !snapshot
                .entries
                .iter()
                .any(|e| e.relative_path == "manifest.json"),
            "should not read manifest.json symlink target"
        );
    }

    fn temp_app_dir(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openntx-snapshot-test-{name}-{unique}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
