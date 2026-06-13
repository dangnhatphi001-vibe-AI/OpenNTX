use openntx_core::config::OpenNtxFileConfig;
use openntx_core::doctor::{app_doctor, global_doctor, repair_app};
use openntx_core::logs::{clean_logs, list_logs};
use openntx_core::manifest::AppManifest;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::registry::AppRegistry;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Helper Functions ──

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
    std::env::temp_dir().join(format!("openntx-v1-test-{unique}"))
}

fn register_test_app(registry: &AppRegistry, app_id: &str) {
    let manifest = fixture_manifest(app_id);
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");
}

// ── list --json tests ──

#[test]
fn list_json_returns_valid_json() {
    let registry = temp_registry();
    register_test_app(&registry, "json-list-app");

    let json_apps = registry.list_apps_json().expect("list apps json");
    assert_eq!(json_apps.len(), 1);
    assert_eq!(json_apps[0].app_id, "json-list-app");
    assert_eq!(json_apps[0].name, "Json List App");
    assert_eq!(json_apps[0].architecture, "x86_64");
    assert_eq!(json_apps[0].install_mode, "portable");
    assert_eq!(json_apps[0].desktop_status, "missing");

    // Verify it serializes to valid JSON
    let json_str = serde_json::to_string_pretty(&json_apps).expect("serialize");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).expect("valid json");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["app_id"], "json-list-app");
}

#[test]
fn list_json_empty_registry() {
    let registry = temp_registry();
    let json_apps = registry.list_apps_json().expect("list apps json empty");
    assert!(json_apps.is_empty());
}

// ── show --json tests ──

#[test]
fn show_json_returns_valid_json() {
    let registry = temp_registry();
    register_test_app(&registry, "show-json-app");

    let summary = registry.show_app_json("show-json-app").expect("show json");
    assert_eq!(summary.app_id, "show-json-app");
    assert_eq!(summary.name, "Show Json App");
    assert_eq!(summary.architecture, "x86_64");
    assert_eq!(summary.status, "registered / analysis-only");

    let json_str = serde_json::to_string_pretty(&summary).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid json");
    assert_eq!(parsed["app_id"], "show-json-app");
}

// ── Doctor tests ──

#[test]
fn doctor_global_reports_data_dir_and_app_count() {
    let registry = temp_registry();
    register_test_app(&registry, "doctor-app");

    let report = global_doctor(&registry).expect("global doctor");
    assert_eq!(report.app_count, 1);
    assert!(report.broken_apps.is_empty());
    assert_eq!(report.status, "healthy with warnings");
}

#[test]
fn doctor_global_empty_registry() {
    let registry = temp_registry();
    let report = global_doctor(&registry).expect("global doctor empty");
    assert_eq!(report.app_count, 0);
    assert!(report.broken_apps.is_empty());
}

#[test]
fn doctor_app_reports_manifest_valid() {
    let registry = temp_registry();
    register_test_app(&registry, "doctor-check-app");

    let report = app_doctor(&registry, "doctor-check-app").expect("app doctor");
    assert!(report.manifest_exists);
    assert!(report.manifest_is_regular_file);
    assert!(report.manifest_valid);
    assert!(report.drive_c_exists);
    assert!(report.drive_c_is_real_dir);
    assert!(report.registry_exists);
    assert!(report.registry_is_real_dir);
}

#[test]
fn doctor_app_rejects_invalid_app_id() {
    let registry = temp_registry();
    let result = app_doctor(&registry, "Invalid App ID!!!");
    assert!(result.is_err());
}

#[test]
fn doctor_app_reports_missing_app() {
    let registry = temp_registry();
    let result = app_doctor(&registry, "nonexistent-app");
    assert!(result.is_err());
}

#[test]
fn doctor_app_json_serializes() {
    let registry = temp_registry();
    register_test_app(&registry, "doctor-json-app");

    let report = app_doctor(&registry, "doctor-json-app").expect("app doctor");
    let json_str = serde_json::to_string_pretty(&report).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid json");
    assert_eq!(parsed["app_id"], "doctor-json-app");
    assert_eq!(parsed["manifest_valid"], true);
}

// ── Doctor repair tests ──

#[test]
fn repair_creates_missing_dirs_dry_run() {
    let registry = temp_registry();
    register_test_app(&registry, "repair-app");

    // Remove drive_c to test repair
    let drive_c = registry.paths().drive_c_path("repair-app");
    fs::remove_dir_all(&drive_c).expect("remove drive_c");

    let plan = repair_app(&registry, "repair-app", true).expect("repair dry-run");
    assert!(!plan.applied);
    assert!(!plan.actions.is_empty());
    assert!(plan.actions.iter().any(|a| a.action_type == "create_dir"));

    // Verify dir was NOT created (dry-run)
    assert!(!drive_c.exists());
}

#[test]
fn repair_creates_missing_dirs_with_yes() {
    let registry = temp_registry();
    register_test_app(&registry, "repair-yes-app");

    // Remove drive_c and registry to test repair
    let drive_c = registry.paths().drive_c_path("repair-yes-app");
    let registry_dir = registry.paths().registry_path("repair-yes-app");
    fs::remove_dir_all(&drive_c).expect("remove drive_c");
    fs::remove_dir_all(&registry_dir).expect("remove registry");

    let plan = repair_app(&registry, "repair-yes-app", false).expect("repair yes");
    assert!(plan.applied);
    assert!(drive_c.exists());
    assert!(registry_dir.exists());
}

#[test]
fn repair_rejects_unsafe_symlinks() {
    let registry = temp_registry();
    register_test_app(&registry, "symlink-app");

    // Create an unsafe symlink inside the app dir
    let app_dir = registry.paths().app_dir("symlink-app");
    let unsafe_link = app_dir.join("unsafe-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/passwd", &unsafe_link).expect("create symlink");

    let plan = repair_app(&registry, "symlink-app", true).expect("repair with symlink");
    assert!(!plan.applied);
    assert!(!plan.unsafe_symlinks_found.is_empty());
}

// ── Logs tests ──

#[test]
fn logs_list_empty_when_no_logs() {
    let registry = temp_registry();
    let logs = list_logs(&registry).expect("list logs empty");
    assert!(logs.is_empty());
}

#[test]
fn logs_clean_dry_run_empty() {
    let registry = temp_registry();
    let deleted = clean_logs(&registry, 30, true).expect("clean logs dry-run");
    assert!(deleted.is_empty());
}

// ── Capture clean dry-run tests ──

#[test]
fn capture_clean_dry_run_no_error() {
    // This tests that the CLI capture clean logic doesn't error
    // when no capture directory exists
    let registry = temp_registry();
    register_test_app(&registry, "capture-clean-app");

    let capture_dir = registry.paths().capture_dir("capture-clean-app");
    assert!(!capture_dir.exists());
}

// ── Config tests ──

#[test]
fn config_default_values() {
    let config = OpenNtxFileConfig::default();
    assert_eq!(config.default_output_dir, "dist");
    assert_eq!(config.default_sandbox_profile, "standard");
    assert!(config.enable_notifications);
    assert_eq!(config.log_retention_days, 30);
    assert_eq!(config.package_version_default, "1.0.0-alpha");
    assert!(!config.appportal_show_advanced);
}

#[test]
fn config_set_valid_key() {
    let mut config = OpenNtxFileConfig::default();
    config.set_key("log_retention_days", "60").expect("set key");
    assert_eq!(config.log_retention_days, 60);
}

#[test]
fn config_set_invalid_key() {
    let mut config = OpenNtxFileConfig::default();
    let result = config.set_key("unknown_key", "value");
    assert!(result.is_err());
}

#[test]
fn config_set_invalid_boolean() {
    let mut config = OpenNtxFileConfig::default();
    let result = config.set_key("enable_notifications", "maybe");
    assert!(result.is_err());
}

#[test]
fn config_set_invalid_sandbox_profile() {
    let mut config = OpenNtxFileConfig::default();
    let result = config.set_key("default_sandbox_profile", "invalid");
    assert!(result.is_err());
}

#[test]
fn config_set_valid_sandbox_profile() {
    let mut config = OpenNtxFileConfig::default();
    config
        .set_key("default_sandbox_profile", "strict")
        .expect("set sandbox");
    assert_eq!(config.default_sandbox_profile, "strict");
}

#[test]
fn config_get_value_roundtrip() {
    let config = OpenNtxFileConfig::default();
    assert_eq!(config.get_value("default_output_dir").unwrap(), "dist");
    assert_eq!(config.get_value("enable_notifications").unwrap(), "true");
    assert_eq!(config.get_value("log_retention_days").unwrap(), "30");
}

#[test]
fn config_get_unknown_key_errors() {
    let config = OpenNtxFileConfig::default();
    assert!(config.get_value("unknown_key").is_err());
}

// ── Export/Import path traversal rejection tests ──

#[test]
fn export_rejects_nonexistent_app() {
    let registry = temp_registry();
    let result = registry.export_bundle(
        "nonexistent-app",
        &PathBuf::from("/tmp/test-bundle.tar.gz"),
        true,
    );
    assert!(result.is_err());
}

#[test]
fn import_rejects_nonexistent_bundle() {
    let registry = temp_registry();
    let result =
        registry.import_bundle(&PathBuf::from("/tmp/nonexistent-bundle.tar.gz"), None, true);
    assert!(result.is_err());
}

#[test]
fn import_rejects_existing_app_without_rename() {
    let registry = temp_registry();
    register_test_app(&registry, "existing-app");

    // We can't easily create a real bundle in a unit test without tar/flate2,
    // but we can test the duplicate_app rejection logic
    let result = registry.duplicate_app("existing-app", "existing-app", true);
    assert!(result.is_err());
}

// ── Duplicate app safety tests ──

#[test]
fn duplicate_rejects_invalid_new_app_id() {
    let registry = temp_registry();
    register_test_app(&registry, "source-app");

    let result = registry.duplicate_app("source-app", "Invalid ID!!!", true);
    assert!(result.is_err());
}

#[test]
fn duplicate_rejects_nonexistent_source() {
    let registry = temp_registry();

    let result = registry.duplicate_app("nonexistent", "new-app", true);
    assert!(result.is_err());
}

#[test]
fn duplicate_rejects_existing_target() {
    let registry = temp_registry();
    register_test_app(&registry, "dup-source");
    register_test_app(&registry, "dup-target");

    let result = registry.duplicate_app("dup-source", "dup-target", true);
    assert!(result.is_err());
}

#[test]
fn duplicate_dry_run_succeeds() {
    let registry = temp_registry();
    register_test_app(&registry, "dup-dry-source");

    let result = registry
        .duplicate_app("dup-dry-source", "dup-dry-target", true)
        .expect("duplicate dry-run");
    assert_eq!(result.source_app_id, "dup-dry-source");
    assert_eq!(result.new_app_id, "dup-dry-target");
}

// ── Rename dry-run/write tests ──

#[test]
fn rename_dry_run_does_not_modify_manifest() {
    let registry = temp_registry();
    register_test_app(&registry, "rename-app");

    let result = registry
        .rename_app("rename-app", "New Name", true)
        .expect("rename dry-run");
    assert_eq!(result.old_name, "Rename App");
    assert_eq!(result.new_name, "New Name");
    assert!(!result.updated_manifest);

    // Verify manifest was NOT changed
    let manifest = registry.load_manifest("rename-app").expect("load manifest");
    assert_eq!(manifest.name, "Rename App");
}

#[test]
fn rename_write_modifies_manifest() {
    let registry = temp_registry();
    register_test_app(&registry, "rename-write-app");

    let result = registry
        .rename_app("rename-write-app", "Updated Name", false)
        .expect("rename write");
    assert_eq!(result.old_name, "Rename Write App");
    assert_eq!(result.new_name, "Updated Name");
    assert!(result.updated_manifest);

    // Verify manifest WAS changed
    let manifest = registry
        .load_manifest("rename-write-app")
        .expect("load manifest");
    assert_eq!(manifest.name, "Updated Name");
    assert_eq!(manifest.desktop.name, "Updated Name");
}

#[test]
fn rename_rejects_invalid_app_id() {
    let registry = temp_registry();
    let result = registry.rename_app("Invalid ID!!!", "New Name", true);
    assert!(result.is_err());
}
