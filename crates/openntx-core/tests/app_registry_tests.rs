use openntx_core::manifest::AppManifest;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::registry::{AppRegistry, DesktopMode, RemoveMode};
use openntx_core::runtime::{create_registered_run_plan, RunPlanOptions, RunPlanReport};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn registry_creation_writes_manifest_plan_dirs_and_metadata() {
    let registry = temp_registry();
    let manifest = fixture_manifest("example-app");
    let install_plan = registry.build_install_plan(&manifest, Some("windows-gui".to_string()));

    let result = registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write plan");

    assert!(result.app_dir.is_dir());
    assert!(result.manifest_path.is_file());
    assert!(result.install_plan_path.is_file());
    assert!(result.metadata_path.is_file());
    assert!(result.drive_c_path.is_dir());
    assert!(result.registry_path.is_dir());
    assert!(result.logs_path.is_dir());

    let plan_json = fs::read_to_string(result.install_plan_path).expect("install plan");
    assert!(plan_json.contains("\"app_id\": \"example-app\""));
    assert!(plan_json.contains("\"status\": \"written / analysis-only\""));
}

#[test]
fn registry_lists_and_loads_registered_apps() {
    let registry = temp_registry();
    let manifest = fixture_manifest("list-app");
    let install_plan = registry.build_install_plan(&manifest, Some("windows-console".to_string()));
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let apps = registry.list_apps().expect("list apps");
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].app_id, "list-app");
    assert_eq!(apps[0].install_mode, "portable");
    assert_eq!(apps[0].architecture, "x86_64");
    assert_eq!(apps[0].sandbox_profile, "standard");
    assert_eq!(apps[0].executable_path, "C:/Example/App.exe");
    assert_eq!(apps[0].imported_dll_count, 0);
    assert_eq!(apps[0].status, "registered / analysis-only");
    assert!(!apps[0].desktop_launcher_exists);

    let loaded = registry.load_manifest("list-app").expect("show manifest");
    assert_eq!(loaded.name, "List App");
}

#[test]
fn remove_dry_run_keeps_registered_app() {
    let registry = temp_registry();
    let manifest = fixture_manifest("dry-run-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    let result = registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let remove_plan = registry
        .remove_app("dry-run-app", RemoveMode::DryRun)
        .expect("dry-run remove should plan");

    assert!(!remove_plan.removed);
    assert!(result.app_dir.exists());
}

#[test]
fn remove_yes_deletes_registered_app_directory() {
    let registry = temp_registry();
    let manifest = fixture_manifest("remove-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    let result = registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let remove_plan = registry
        .remove_app("remove-app", RemoveMode::Delete)
        .expect("confirmed remove should delete");

    assert!(remove_plan.removed);
    assert!(!result.app_dir.exists());
}

#[test]
fn desktop_create_and_remove_support_dry_run_and_write() {
    let registry = temp_registry();
    let manifest = fixture_manifest("desktop-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let dry_run = registry
        .create_desktop_entry("desktop-app", DesktopMode::DryRun, "openntx")
        .expect("desktop dry-run should plan");
    assert!(!dry_run.written);
    assert!(dry_run
        .content
        .contains("Exec=openntx run desktop-app --notify"));
    assert!(!dry_run.desktop_entry_path.exists());

    let written = registry
        .create_desktop_entry("desktop-app", DesktopMode::Write, "openntx")
        .expect("desktop write should succeed");
    assert!(written.written);
    assert!(written.desktop_entry_path.is_file());
    let apps = registry.list_apps().expect("list apps after desktop write");
    assert_eq!(apps.len(), 1);
    assert!(apps[0].desktop_launcher_exists);

    let remove_dry_run = registry
        .remove_desktop_entry("desktop-app", DesktopMode::DryRun)
        .expect("desktop remove dry-run should plan");
    assert!(!remove_dry_run.removed);
    assert!(written.desktop_entry_path.exists());

    let removed = registry
        .remove_desktop_entry("desktop-app", DesktopMode::Write)
        .expect("desktop remove should delete");
    assert!(removed.removed);
    assert!(!written.desktop_entry_path.exists());
}

#[test]
fn registered_run_plan_writes_diagnostics_log() {
    let registry = temp_registry();
    let mut manifest = fixture_manifest("run-plan-app");
    manifest.diagnostics.imported_dlls = vec!["KERNEL32.dll".to_string(), "USER32.dll".to_string()];
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");
    registry
        .create_desktop_entry("run-plan-app", DesktopMode::Write, "openntx")
        .expect("desktop write should succeed");

    let report = create_registered_run_plan(
        &registry,
        "run-plan-app",
        &RunPlanOptions { write_log: true },
    )
    .expect("run plan should be created");

    assert_eq!(report.app_id, "run-plan-app");
    assert_eq!(report.name, "Run Plan App");
    assert_eq!(report.executable_path, "C:/Example/App.exe");
    assert_eq!(report.architecture, "x86_64");
    assert_eq!(report.install_mode, "portable");
    assert_eq!(report.sandbox_profile, "standard");
    assert_eq!(report.imported_dll_count, 2);
    assert_eq!(report.desktop_status, "present");
    assert_eq!(report.status, "dry-run / not implemented");
    let log_path = PathBuf::from(report.log_path.as_ref().expect("log path"));
    assert!(log_path.is_file());

    let log_json = fs::read_to_string(&log_path).expect("run plan log");
    let parsed: RunPlanReport = serde_json::from_str(&log_json).expect("valid run plan json");
    assert_eq!(parsed.app_id, "run-plan-app");
    assert_eq!(parsed.log_path, report.log_path);
}

#[test]
fn run_plan_report_serializes_as_valid_json() {
    let registry = temp_registry();
    let manifest = fixture_manifest("json-plan-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let report = create_registered_run_plan(
        &registry,
        "json-plan-app",
        &RunPlanOptions { write_log: false },
    )
    .expect("run plan should be created");

    let json = serde_json::to_string_pretty(&report).expect("report should serialize");
    let parsed: RunPlanReport =
        serde_json::from_str(&json).expect("serialized JSON should round-trip");
    assert_eq!(parsed.app_id, report.app_id);
    assert_eq!(parsed.target, report.target);
    assert_eq!(parsed.timestamp, report.timestamp);
    assert_eq!(parsed.status, "dry-run / not implemented");
}

#[test]
fn run_plan_without_log_does_not_create_file() {
    let registry = temp_registry();
    let manifest = fixture_manifest("nolog-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let report =
        create_registered_run_plan(&registry, "nolog-app", &RunPlanOptions { write_log: false })
            .expect("run plan should be created");

    assert!(report.log_path.is_none(), "no log should be created");
    assert!(
        !registry.paths().logs_root.exists() || {
            let entries = fs::read_dir(&registry.paths().logs_root)
                .expect("read logs dir")
                .count();
            entries == 0
        }
    );
}

#[test]
fn run_plan_includes_target_and_timestamp() {
    let registry = temp_registry();
    let manifest = fixture_manifest("timestamp-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let report = create_registered_run_plan(
        &registry,
        "timestamp-app",
        &RunPlanOptions { write_log: false },
    )
    .expect("run plan should be created");

    assert_eq!(report.target, "timestamp-app");
    assert!(
        report.timestamp.contains('T'),
        "timestamp should be ISO 8601-like"
    );
    assert!(
        report.timestamp.ends_with('Z'),
        "timestamp should end with Z for UTC"
    );
    assert!(report.created_at_unix > 0);
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
    std::env::temp_dir().join(format!("openntx-registry-test-{unique}"))
}
