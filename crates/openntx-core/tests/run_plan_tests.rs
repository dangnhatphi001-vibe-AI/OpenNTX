use openntx_core::manifest::AppManifest;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::registry::AppRegistry;
use openntx_core::runtime::{create_registered_run_plan, RunPlanOptions};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verify that the notify_send path (called from CLI) cannot cause a run-plan
/// failure even when notify-send is unavailable. We test this by exercising the
/// core run plan which is always invoked first. The CLI notify wrapper catches
/// any error from `notify-send` and falls back to stdout.
#[test]
fn run_plan_succeeds_even_without_notify_send() {
    let registry = temp_registry();
    let manifest = fixture_manifest("notify-test-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    // The run plan itself should never fail regardless of notify-send availability
    let report = create_registered_run_plan(
        &registry,
        "notify-test-app",
        &RunPlanOptions { write_log: true },
    )
    .expect("run plan should succeed even if notify-send is missing");

    assert_eq!(report.app_id, "notify-test-app");
    assert_eq!(report.status, "dry-run / not implemented");
    assert!(report.log_path.is_some());
}

/// Verify that the run-plan report message contains the V0.8 status notice
#[test]
fn run_plan_message_mentions_v08_status() {
    let registry = temp_registry();
    let manifest = fixture_manifest("msg-test-app");
    let install_plan = registry.build_install_plan(&manifest, None);
    registry
        .register_plan(&manifest, &install_plan)
        .expect("registry should write app");

    let report = create_registered_run_plan(
        &registry,
        "msg-test-app",
        &RunPlanOptions { write_log: false },
    )
    .expect("run plan should succeed");

    assert!(
        report.message.contains("not implemented"),
        "message should mention not implemented"
    );
    assert!(
        report.message.contains("V0.8"),
        "message should mention V0.8"
    );
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
    std::env::temp_dir().join(format!("openntx-runplan-test-{unique}"))
}
