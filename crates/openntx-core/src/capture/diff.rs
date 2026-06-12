use serde::{Deserialize, Serialize};

use crate::capture::snapshot::Snapshot;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDiff {
    pub relative_path: String,
    pub kind: String,
    pub before_size: Option<u64>,
    pub after_size: Option<u64>,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectoryDiff {
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymlinkDiff {
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureDiff {
    pub schema_version: String,
    pub app_id: String,
    pub before_snapshot_at: String,
    pub after_snapshot_at: String,
    pub files_created: Vec<FileDiff>,
    pub files_removed: Vec<FileDiff>,
    pub files_modified: Vec<FileDiff>,
    pub directories_created: Vec<DirectoryDiff>,
    pub directories_removed: Vec<DirectoryDiff>,
    pub symlinks_created: Vec<SymlinkDiff>,
    pub symlinks_removed: Vec<SymlinkDiff>,
    pub registry_files_changed: Vec<FileDiff>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub fn compute_diff(before: &Snapshot, after: &Snapshot) -> crate::Result<CaptureDiff> {
    if before.app_id != after.app_id {
        return Err(crate::OpenNtxError::InvalidInput(format!(
            "snapshot app_id mismatch: before={}, after={}",
            before.app_id, after.app_id
        )));
    }

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    if !before.errors.is_empty() {
        warnings.push(format!(
            "before snapshot had {} error(s)",
            before.errors.len()
        ));
    }
    if !after.errors.is_empty() {
        warnings.push(format!(
            "after snapshot had {} error(s)",
            after.errors.len()
        ));
    }

    // Partition entries by kind
    let before_files: std::collections::HashMap<&str, &crate::capture::snapshot::SnapshotEntry> =
        before
            .entries
            .iter()
            .filter(|e| e.kind == "file")
            .map(|e| (e.relative_path.as_str(), e))
            .collect();

    let after_files: std::collections::HashMap<&str, &crate::capture::snapshot::SnapshotEntry> =
        after
            .entries
            .iter()
            .filter(|e| e.kind == "file")
            .map(|e| (e.relative_path.as_str(), e))
            .collect();

    let before_dirs: std::collections::HashSet<&str> = before
        .entries
        .iter()
        .filter(|e| e.kind == "directory")
        .map(|e| e.relative_path.as_str())
        .collect();

    let after_dirs: std::collections::HashSet<&str> = after
        .entries
        .iter()
        .filter(|e| e.kind == "directory")
        .map(|e| e.relative_path.as_str())
        .collect();

    let before_symlinks: std::collections::HashSet<&str> = before
        .entries
        .iter()
        .filter(|e| e.kind == "symlink")
        .map(|e| e.relative_path.as_str())
        .collect();

    let after_symlinks: std::collections::HashSet<&str> = after
        .entries
        .iter()
        .filter(|e| e.kind == "symlink")
        .map(|e| e.relative_path.as_str())
        .collect();

    // File diffs
    let mut files_created = Vec::new();
    let mut files_removed = Vec::new();
    let mut files_modified = Vec::new();
    let mut registry_files_changed = Vec::new();

    for (path, entry) in &after_files {
        if !before_files.contains_key(path) {
            let diff = FileDiff {
                relative_path: path.to_string(),
                kind: entry.kind.clone(),
                before_size: None,
                after_size: Some(entry.size),
                before_sha256: None,
                after_sha256: entry.sha256.clone(),
            };
            if path.starts_with("registry/") {
                registry_files_changed.push(diff.clone());
            }
            files_created.push(diff);
        }
    }

    for (path, entry) in &before_files {
        if !after_files.contains_key(path) {
            let diff = FileDiff {
                relative_path: path.to_string(),
                kind: entry.kind.clone(),
                before_size: Some(entry.size),
                after_size: None,
                before_sha256: entry.sha256.clone(),
                after_sha256: None,
            };
            if path.starts_with("registry/") {
                registry_files_changed.push(diff.clone());
            }
            files_removed.push(diff);
        }
    }

    for (path, after_entry) in &after_files {
        if let Some(before_entry) = before_files.get(path) {
            let content_changed = before_entry.sha256 != after_entry.sha256;
            let size_changed = before_entry.size != after_entry.size;

            if content_changed || size_changed {
                let diff = FileDiff {
                    relative_path: path.to_string(),
                    kind: after_entry.kind.clone(),
                    before_size: Some(before_entry.size),
                    after_size: Some(after_entry.size),
                    before_sha256: before_entry.sha256.clone(),
                    after_sha256: after_entry.sha256.clone(),
                };
                if path.starts_with("registry/") {
                    registry_files_changed.push(diff.clone());
                }
                files_modified.push(diff);
            }
        }
    }

    // Directory diffs
    let mut directories_created = Vec::new();
    let mut directories_removed = Vec::new();

    for dir in &after_dirs {
        if !before_dirs.contains(dir) {
            directories_created.push(DirectoryDiff {
                relative_path: dir.to_string(),
            });
        }
    }

    for dir in &before_dirs {
        if !after_dirs.contains(dir) {
            directories_removed.push(DirectoryDiff {
                relative_path: dir.to_string(),
            });
        }
    }

    // Symlink diffs — tracked separately and added as warnings
    let mut symlinks_created = Vec::new();
    let mut symlinks_removed = Vec::new();

    for link in &after_symlinks {
        if !before_symlinks.contains(link) {
            symlinks_created.push(SymlinkDiff {
                relative_path: link.to_string(),
            });
            warnings.push(format!("symlink created: {link}"));
        }
    }

    for link in &before_symlinks {
        if !after_symlinks.contains(link) {
            symlinks_removed.push(SymlinkDiff {
                relative_path: link.to_string(),
            });
            warnings.push(format!("symlink removed: {link}"));
        }
    }

    files_created.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files_removed.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files_modified.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    directories_created.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    directories_removed.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    symlinks_created.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    symlinks_removed.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    registry_files_changed.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    errors.sort();
    errors.dedup();
    warnings.sort();
    warnings.dedup();

    Ok(CaptureDiff {
        schema_version: "0.2.0".to_string(),
        app_id: before.app_id.clone(),
        before_snapshot_at: before.created_at.clone(),
        after_snapshot_at: after.created_at.clone(),
        files_created,
        files_removed,
        files_modified,
        directories_created,
        directories_removed,
        symlinks_created,
        symlinks_removed,
        registry_files_changed,
        warnings,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::snapshot::SnapshotEntry;

    fn make_snapshot(app_id: &str, entries: Vec<SnapshotEntry>) -> Snapshot {
        Snapshot {
            schema_version: "0.2.0".to_string(),
            app_id: app_id.to_string(),
            root: "/tmp/test".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            entries,
            errors: Vec::new(),
        }
    }

    fn file_entry(path: &str, sha: &str) -> SnapshotEntry {
        SnapshotEntry {
            relative_path: path.to_string(),
            kind: "file".to_string(),
            size: 100,
            modified_unix: Some(1000),
            sha256: Some(sha.to_string()),
            readonly: Some(false),
        }
    }

    fn dir_entry(path: &str) -> SnapshotEntry {
        SnapshotEntry {
            relative_path: path.to_string(),
            kind: "directory".to_string(),
            size: 0,
            modified_unix: Some(1000),
            sha256: None,
            readonly: Some(false),
        }
    }

    fn symlink_entry(path: &str) -> SnapshotEntry {
        SnapshotEntry {
            relative_path: path.to_string(),
            kind: "symlink".to_string(),
            size: 0,
            modified_unix: Some(1000),
            sha256: None,
            readonly: None,
        }
    }

    #[test]
    fn file_created_detected() {
        let before = make_snapshot("test-app", vec![dir_entry("drive_c")]);
        let after = make_snapshot(
            "test-app",
            vec![dir_entry("drive_c"), file_entry("drive_c/new.txt", "abc")],
        );
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.files_created.len(), 1);
        assert_eq!(diff.files_created[0].relative_path, "drive_c/new.txt");
        assert!(diff.files_removed.is_empty());
        assert!(diff.files_modified.is_empty());
    }

    #[test]
    fn file_removed_detected() {
        let before = make_snapshot(
            "test-app",
            vec![dir_entry("drive_c"), file_entry("drive_c/old.txt", "abc")],
        );
        let after = make_snapshot("test-app", vec![dir_entry("drive_c")]);
        let diff = compute_diff(&before, &after).unwrap();
        assert!(diff.files_created.is_empty());
        assert_eq!(diff.files_removed.len(), 1);
        assert_eq!(diff.files_removed[0].relative_path, "drive_c/old.txt");
    }

    #[test]
    fn file_modified_detected() {
        let before = make_snapshot("test-app", vec![file_entry("drive_c/data.txt", "hash1")]);
        let after = make_snapshot("test-app", vec![file_entry("drive_c/data.txt", "hash2")]);
        let diff = compute_diff(&before, &after).unwrap();
        assert!(diff.files_created.is_empty());
        assert!(diff.files_removed.is_empty());
        assert_eq!(diff.files_modified.len(), 1);
        assert_eq!(diff.files_modified[0].relative_path, "drive_c/data.txt");
        assert_eq!(
            diff.files_modified[0].before_sha256,
            Some("hash1".to_string())
        );
        assert_eq!(
            diff.files_modified[0].after_sha256,
            Some("hash2".to_string())
        );
    }

    #[test]
    fn directory_created_detected() {
        let before = make_snapshot("test-app", vec![]);
        let after = make_snapshot("test-app", vec![dir_entry("drive_c/new_dir")]);
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.directories_created.len(), 1);
        assert_eq!(diff.directories_created[0].relative_path, "drive_c/new_dir");
    }

    #[test]
    fn directory_removed_detected() {
        let before = make_snapshot("test-app", vec![dir_entry("drive_c/old_dir")]);
        let after = make_snapshot("test-app", vec![]);
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.directories_removed.len(), 1);
        assert_eq!(diff.directories_removed[0].relative_path, "drive_c/old_dir");
    }

    #[test]
    fn registry_file_change_tracked() {
        let before = make_snapshot("test-app", vec![]);
        let after = make_snapshot("test-app", vec![file_entry("registry/user.reg", "new")]);
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.registry_files_changed.len(), 1);
        assert_eq!(
            diff.registry_files_changed[0].relative_path,
            "registry/user.reg"
        );
    }

    #[test]
    fn mismatched_app_id_errors() {
        let before = make_snapshot("app-a", vec![]);
        let after = make_snapshot("app-b", vec![]);
        let result = compute_diff(&before, &after);
        assert!(result.is_err());
    }

    #[test]
    fn identical_snapshots_produce_empty_diff() {
        let entries = vec![dir_entry("drive_c"), file_entry("drive_c/f.txt", "x")];
        let before = make_snapshot("test-app", entries.clone());
        let after = make_snapshot("test-app", entries);
        let diff = compute_diff(&before, &after).unwrap();
        assert!(diff.files_created.is_empty());
        assert!(diff.files_removed.is_empty());
        assert!(diff.files_modified.is_empty());
        assert!(diff.directories_created.is_empty());
        assert!(diff.directories_removed.is_empty());
        assert!(diff.symlinks_created.is_empty());
        assert!(diff.symlinks_removed.is_empty());
    }

    #[test]
    fn symlink_created_tracked_and_warned() {
        let before = make_snapshot("test-app", vec![dir_entry("drive_c")]);
        let after = make_snapshot(
            "test-app",
            vec![dir_entry("drive_c"), symlink_entry("drive_c/link")],
        );
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.symlinks_created.len(), 1);
        assert_eq!(diff.symlinks_created[0].relative_path, "drive_c/link");
        assert!(
            diff.warnings.iter().any(|w| w.contains("symlink created")),
            "should warn about created symlink"
        );
    }

    #[test]
    fn symlink_removed_tracked_and_warned() {
        let before = make_snapshot(
            "test-app",
            vec![dir_entry("drive_c"), symlink_entry("drive_c/link")],
        );
        let after = make_snapshot("test-app", vec![dir_entry("drive_c")]);
        let diff = compute_diff(&before, &after).unwrap();
        assert_eq!(diff.symlinks_removed.len(), 1);
        assert_eq!(diff.symlinks_removed[0].relative_path, "drive_c/link");
        assert!(
            diff.warnings.iter().any(|w| w.contains("symlink removed")),
            "should warn about removed symlink"
        );
    }
}
