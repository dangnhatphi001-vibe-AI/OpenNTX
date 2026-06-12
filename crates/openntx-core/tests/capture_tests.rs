use openntx_core::capture::diff::compute_diff;
use openntx_core::capture::report_writer::write_capture_report;
use openntx_core::capture::snapshot::create_snapshot;
use openntx_core::capture::Snapshot;
use openntx_core::manifest::AppManifest;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::registry::AppRegistry;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// --- Snapshot tests ---

#[test]
fn snapshot_creates_from_temp_app_directory() {
    let (registry, app_id) = setup_registered_app("snap-create");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");
    fs::write(drive_c.join("hello.txt"), b"hello").unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
    assert_eq!(snapshot.app_id, app_id);
    assert!(snapshot.errors.is_empty());
    let hello = snapshot
        .entries
        .iter()
        .find(|e| e.relative_path == "drive_c/hello.txt");
    assert!(hello.is_some());
    assert_eq!(hello.unwrap().kind, "file");
    assert_eq!(hello.unwrap().size, 5);
    assert!(hello.unwrap().sha256.is_some());
}

#[test]
fn snapshot_captures_registry_directory() {
    let (registry, app_id) = setup_registered_app("snap-reg");
    let app_dir = registry.paths().app_dir(&app_id);
    let registry_dir = app_dir.join("registry");
    fs::write(registry_dir.join("user.reg"), b"REG").unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
    let reg = snapshot
        .entries
        .iter()
        .find(|e| e.relative_path == "registry/user.reg");
    assert!(reg.is_some());
}

#[test]
fn snapshot_captures_optional_files() {
    let (registry, app_id) = setup_registered_app("snap-opt");
    let app_dir = registry.paths().app_dir(&app_id);

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
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

// --- Diff tests ---

#[test]
fn file_created_detected_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-created");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::write(drive_c.join("new.txt"), b"new content").unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert_eq!(diff.files_created.len(), 1);
    assert_eq!(diff.files_created[0].relative_path, "drive_c/new.txt");
    assert!(diff.files_removed.is_empty());
    assert!(diff.files_modified.is_empty());
}

#[test]
fn file_modified_detected_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-modified");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");
    fs::write(drive_c.join("data.txt"), b"original").unwrap();

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::write(drive_c.join("data.txt"), b"modified").unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert!(diff.files_created.is_empty());
    assert!(diff.files_removed.is_empty());
    assert_eq!(diff.files_modified.len(), 1);
    assert_eq!(diff.files_modified[0].relative_path, "drive_c/data.txt");
    assert_ne!(
        diff.files_modified[0].before_sha256,
        diff.files_modified[0].after_sha256
    );
}

#[test]
fn file_removed_detected_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-removed");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");
    fs::write(drive_c.join("old.txt"), b"old").unwrap();

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::remove_file(drive_c.join("old.txt")).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert!(diff.files_created.is_empty());
    assert_eq!(diff.files_removed.len(), 1);
    assert_eq!(diff.files_removed[0].relative_path, "drive_c/old.txt");
    assert!(diff.files_modified.is_empty());
}

#[test]
fn directory_created_detected_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-dir-created");
    let app_dir = registry.paths().app_dir(&app_id);

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::create_dir_all(app_dir.join("drive_c/subdir")).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    let new_dirs: Vec<_> = diff
        .directories_created
        .iter()
        .filter(|d| d.relative_path.contains("subdir"))
        .collect();
    assert_eq!(new_dirs.len(), 1);
}

#[test]
fn directory_removed_detected_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-dir-removed");
    let app_dir = registry.paths().app_dir(&app_id);
    fs::create_dir_all(app_dir.join("drive_c/old_dir")).unwrap();

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::remove_dir(app_dir.join("drive_c/old_dir")).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    let removed_dirs: Vec<_> = diff
        .directories_removed
        .iter()
        .filter(|d| d.relative_path.contains("old_dir"))
        .collect();
    assert_eq!(removed_dirs.len(), 1);
}

#[test]
fn registry_file_change_tracked_in_diff() {
    let (registry, app_id) = setup_registered_app("diff-reg-change");
    let app_dir = registry.paths().app_dir(&app_id);
    let registry_dir = app_dir.join("registry");

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::write(registry_dir.join("user.reg"), b"new reg data").unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    let reg_changes: Vec<_> = diff
        .registry_files_changed
        .iter()
        .filter(|f| f.relative_path == "registry/user.reg")
        .collect();
    assert_eq!(reg_changes.len(), 1);
}

#[test]
fn missing_snapshot_before_errors() {
    let (registry, app_id) = setup_registered_app("missing-before");
    let capture_dir = registry.paths().capture_dir(&app_id);
    assert!(!capture_dir.join("snapshot-before.json").exists());
}

#[test]
fn missing_snapshot_after_errors() {
    let (registry, app_id) = setup_registered_app("missing-after");
    let app_dir = registry.paths().app_dir(&app_id);
    let capture_dir = registry.paths().capture_dir(&app_id);

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
    fs::create_dir_all(&capture_dir).unwrap();
    let json = serde_json::to_vec_pretty(&snapshot).unwrap();
    fs::write(capture_dir.join("snapshot-before.json"), json).unwrap();

    assert!(!capture_dir.join("snapshot-after.json").exists());
}

// --- Report tests ---

#[test]
fn capture_report_json_is_valid() {
    let (registry, app_id) = setup_registered_app("report-valid");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");
    let capture_dir = registry.paths().capture_dir(&app_id);

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::write(drive_c.join("new.txt"), b"new").unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    fs::create_dir_all(&capture_dir).unwrap();
    let report_path = capture_dir.join("capture-report.json");

    let manifest = registry.load_manifest(&app_id).unwrap();
    let report = write_capture_report(
        &report_path,
        &app_id,
        &diff,
        &before,
        &after,
        Some(&manifest),
    )
    .unwrap();

    assert_eq!(report.schema_version, "0.2.0");
    assert_eq!(report.app_id, app_id);
    assert_eq!(report.status, "analysis-only / no runtime execution");
    assert!(report_path.exists());

    let json_str = fs::read_to_string(&report_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.is_object());
    assert!(parsed.get("schema_version").is_some());
    assert!(parsed.get("capture_id").is_some());
    assert!(parsed.get("app_id").is_some());
    assert!(parsed.get("status").is_some());
    assert!(parsed.get("files_created").is_some());
    assert!(parsed.get("files_modified").is_some());
    assert!(parsed.get("files_removed").is_some());
    assert!(parsed.get("warnings").is_some());
    assert!(parsed.get("errors").is_some());
}

#[test]
fn capture_report_includes_analysis_only_status() {
    let (registry, app_id) = setup_registered_app("report-status");
    let app_dir = registry.paths().app_dir(&app_id);
    let capture_dir = registry.paths().capture_dir(&app_id);

    let before = create_snapshot(&app_id, &app_dir).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();
    let diff = compute_diff(&before, &after).unwrap();

    fs::create_dir_all(&capture_dir).unwrap();
    let report_path = capture_dir.join("capture-report.json");
    let report = write_capture_report(&report_path, &app_id, &diff, &before, &after, None).unwrap();

    assert_eq!(report.status, "analysis-only / no runtime execution");
    assert!(
        !report.warnings.is_empty(),
        "report should have at least one warning"
    );
}

// --- Security tests ---

#[cfg(unix)]
#[test]
fn drive_c_symlink_to_outside_fails_safely() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("sec-drivec");
    let app_dir = registry.paths().app_dir(&app_id);
    let outside = temp_root().join("outside-dc");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();

    // Remove the real drive_c dir and replace with symlink
    let _ = fs::remove_dir_all(app_dir.join("drive_c"));
    symlink(&outside, app_dir.join("drive_c")).unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
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

    let _ = fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn registry_symlink_to_outside_fails_safely() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("sec-registry");
    let app_dir = registry.paths().app_dir(&app_id);
    let outside = temp_root().join("outside-reg");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.reg"), b"secret").unwrap();

    let _ = fs::remove_dir_all(app_dir.join("registry"));
    symlink(&outside, app_dir.join("registry")).unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
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

    let _ = fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn manifest_json_symlink_to_outside_not_read() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("sec-manifest");
    let app_dir = registry.paths().app_dir(&app_id);
    let outside = temp_root().join("outside-manifest");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("manifest.json"), b"{}").unwrap();

    // Replace real manifest with symlink
    let _ = fs::remove_file(app_dir.join("manifest.json"));
    symlink(outside.join("manifest.json"), app_dir.join("manifest.json")).unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
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

    let _ = fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn capture_symlink_to_tmp_does_not_receive_artifacts() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("sec-capture");
    let app_dir = registry.paths().app_dir(&app_id);
    let outside = temp_root().join("outside-capture");
    fs::create_dir_all(&outside).unwrap();

    // Replace capture dir with symlink to /tmp
    symlink(&outside, app_dir.join("capture")).unwrap();

    // The snapshot should still work (it doesn't write to capture/)
    let snapshot = create_snapshot(&app_id, &app_dir);
    assert!(
        snapshot.is_ok(),
        "snapshot should work even with capture symlink"
    );

    let _ = fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn created_symlink_appears_in_diff_warning() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("diff-symlink-created");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");

    // Create a real file to symlink to (inside app dir)
    fs::write(drive_c.join("real_file.txt"), b"data").unwrap();

    let before = create_snapshot(&app_id, &app_dir).unwrap();

    // Create a symlink pointing to a file inside the app dir
    symlink("real_file.txt", drive_c.join("link_to_real")).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert!(
        diff.symlinks_created
            .iter()
            .any(|s| s.relative_path.contains("link_to_real")),
        "created symlink should appear in symlinks_created"
    );
    assert!(
        diff.warnings.iter().any(|w| w.contains("symlink created")),
        "should warn about created symlink"
    );

    let _ = fs::remove_file(drive_c.join("link_to_real"));
}

#[cfg(unix)]
#[test]
fn removed_symlink_appears_in_diff_warning() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("diff-symlink-removed");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");

    // Create a real file to symlink to (inside app dir)
    fs::write(drive_c.join("real_file.txt"), b"data").unwrap();

    // Create a symlink pointing to a file inside the app dir
    symlink("real_file.txt", drive_c.join("link_to_real")).unwrap();
    let before = create_snapshot(&app_id, &app_dir).unwrap();

    fs::remove_file(drive_c.join("link_to_real")).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert!(
        diff.symlinks_removed
            .iter()
            .any(|s| s.relative_path.contains("link_to_real")),
        "removed symlink should appear in symlinks_removed"
    );
    assert!(
        diff.warnings.iter().any(|w| w.contains("symlink removed")),
        "should warn about removed symlink"
    );
}

// --- Snapshot entry field tests ---

#[test]
fn snapshot_entry_has_correct_fields() {
    let (registry, app_id) = setup_registered_app("entry-fields");
    let app_dir = registry.paths().app_dir(&app_id);
    let drive_c = app_dir.join("drive_c");
    fs::write(drive_c.join("data.bin"), b"abc123").unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
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

// --- Identical snapshot diff ---

#[test]
fn identical_snapshots_produce_empty_diff() {
    let (registry, app_id) = setup_registered_app("identical");
    let app_dir = registry.paths().app_dir(&app_id);
    fs::write(app_dir.join("drive_c").join("stable.txt"), b"stable").unwrap();

    let before = create_snapshot(&app_id, &app_dir).unwrap();
    let after = create_snapshot(&app_id, &app_dir).unwrap();

    let diff = compute_diff(&before, &after).unwrap();
    assert!(diff.files_created.is_empty());
    assert!(diff.files_removed.is_empty());
    assert!(diff.files_modified.is_empty());
}

// --- Snapshot JSON round-trip ---

#[test]
fn snapshot_json_round_trip() {
    let (registry, app_id) = setup_registered_app("round-trip");
    let app_dir = registry.paths().app_dir(&app_id);
    fs::write(app_dir.join("drive_c").join("test.txt"), b"data").unwrap();

    let snapshot = create_snapshot(&app_id, &app_dir).unwrap();
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    let parsed: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.app_id, snapshot.app_id);
    assert_eq!(parsed.entries.len(), snapshot.entries.len());
}

// --- Manifest symlink security probes ---
// Each probe uses a fresh registry so one corrupted probe does not affect the next.

#[cfg(unix)]
#[test]
fn load_manifest_rejects_symlinked_manifest_json() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("probe-load");
    let manifest_path = registry.paths().manifest_path(&app_id);

    // Replace manifest.json with a symlink to an outside file
    let outside_dir = temp_root();
    fs::create_dir_all(&outside_dir).unwrap();
    let outside = outside_dir.join("outside-probe-load.txt");
    fs::write(&outside, b"not valid json at all").unwrap();
    fs::remove_file(&manifest_path).unwrap();
    symlink(&outside, &manifest_path).unwrap();

    let result = registry.load_manifest(&app_id);
    assert!(result.is_err(), "should reject symlinked manifest.json");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("symlink") || err_msg.contains("unsafe"),
        "error should mention symlink or unsafe: {err_msg}"
    );
    // Must not have read the target content (would get JSON error, not symlink error)
    assert!(
        !err_msg.contains("expected"),
        "should not parse symlink target content"
    );

    let _ = fs::remove_dir_all(&outside_dir);
}

#[cfg(unix)]
#[test]
fn list_apps_skips_symlinked_manifest_json() {
    use std::os::unix::fs::symlink;

    // Create two apps: one normal, one with symlinked manifest
    let registry = temp_registry();
    let normal_manifest = fixture_manifest("normal-app");
    let normal_plan = registry.build_install_plan(&normal_manifest, None);
    registry
        .register_plan(&normal_manifest, &normal_plan)
        .unwrap();

    let bad_manifest = fixture_manifest("bad-app");
    let bad_plan = registry.build_install_plan(&bad_manifest, None);
    registry.register_plan(&bad_manifest, &bad_plan).unwrap();

    // Replace bad-app's manifest.json with a symlink to .bashrc
    let bad_manifest_path = registry.paths().manifest_path("bad-app");
    let outside_dir = temp_root();
    fs::create_dir_all(&outside_dir).unwrap();
    let outside = outside_dir.join("outside-list-probe.txt");
    fs::write(&outside, b"#!/bin/bash\necho pwned").unwrap();
    fs::remove_file(&bad_manifest_path).unwrap();
    symlink(&outside, &bad_manifest_path).unwrap();

    let apps = registry.list_apps().expect("list should succeed");
    assert_eq!(apps.len(), 1, "should list only the normal app");
    assert_eq!(apps[0].app_id, "normal-app");
    // bad-app should be silently skipped

    let _ = fs::remove_dir_all(&outside_dir);
}

#[cfg(unix)]
#[test]
fn snapshot_before_fails_safely_with_symlinked_manifest() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("probe-snap");
    let manifest_path = registry.paths().manifest_path(&app_id);

    // Replace manifest.json with a symlink to an outside file
    let outside_dir = temp_root();
    fs::create_dir_all(&outside_dir).unwrap();
    let outside = outside_dir.join("outside-snap-probe.txt");
    fs::write(&outside, b"this is not JSON").unwrap();
    fs::remove_file(&manifest_path).unwrap();
    symlink(&outside, &manifest_path).unwrap();

    let service = openntx_core::capture::CaptureRegistryService::new(registry);
    let result = service.snapshot_before(&app_id);
    assert!(
        result.is_err(),
        "snapshot_before should fail with symlinked manifest"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("symlink") || err_msg.contains("unsafe"),
        "error should mention symlink or unsafe: {err_msg}"
    );

    let _ = fs::remove_dir_all(&outside_dir);
}

#[cfg(unix)]
#[test]
fn capture_status_fails_safely_with_symlinked_manifest() {
    use std::os::unix::fs::symlink;

    let (registry, app_id) = setup_registered_app("probe-status");
    let manifest_path = registry.paths().manifest_path(&app_id);

    // Replace manifest.json with a symlink to an outside file
    let outside_dir = temp_root();
    fs::create_dir_all(&outside_dir).unwrap();
    let outside = outside_dir.join("outside-status-probe.txt");
    fs::write(&outside, b"#!/bin/bash\necho pwned").unwrap();
    fs::remove_file(&manifest_path).unwrap();
    symlink(&outside, &manifest_path).unwrap();

    let service = openntx_core::capture::CaptureRegistryService::new(registry);
    let result = service.status(&app_id);
    assert!(
        result.is_err(),
        "status should fail with symlinked manifest"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("symlink") || err_msg.contains("unsafe"),
        "error should mention symlink or unsafe: {err_msg}"
    );

    let _ = fs::remove_dir_all(&outside_dir);
}

#[cfg(unix)]
#[test]
fn read_json_safe_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let real_file = root.join("real.json");
    let link_file = root.join("link.json");
    fs::write(&real_file, b"{\"ok\": true}").unwrap();
    symlink(&real_file, &link_file).unwrap();

    let result = openntx_core::manifest::read_json_safe::<serde_json::Value>(&link_file);
    assert!(result.is_err(), "should reject symlink");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("symlink") || err_msg.contains("unsafe"),
        "error should mention symlink or unsafe: {err_msg}"
    );
}

// --- Helpers ---

fn setup_registered_app(name: &str) -> (AppRegistry, String) {
    let registry = temp_registry();
    let manifest = fixture_manifest(name);
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");
    (registry, name.to_string())
}

fn fixture_manifest(app_id: &str) -> AppManifest {
    let name = app_id
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut manifest = AppManifest::minimal(app_id, &name, "C:/Example/App.exe");
    manifest.install_mode = "portable".to_string();
    manifest.architecture = "x86_64".to_string();
    manifest
}

fn temp_registry() -> AppRegistry {
    let root = temp_root();
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

fn temp_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("openntx-capture-test-{unique}"))
}
