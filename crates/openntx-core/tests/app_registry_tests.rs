use openntx_core::manifest::AppManifest;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::registry::{AppRegistry, RemoveMode};
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
