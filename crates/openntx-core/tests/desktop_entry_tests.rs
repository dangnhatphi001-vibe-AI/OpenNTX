use openntx_core::desktop::generate_desktop_entry;
use openntx_core::manifest::AppManifest;

#[test]
fn desktop_entry_contains_openntx_run_command() {
    let manifest = AppManifest::minimal("example-app", "Example App", "C:/Example/App.exe");
    let entry = generate_desktop_entry(&manifest, "openntx").expect("desktop entry");

    assert!(entry.contains("[Desktop Entry]"));
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Name=Example App"));
    assert!(entry.contains("Exec=openntx run example-app --notify"));
    assert!(entry.contains("Icon=example-app"));
}

#[test]
fn desktop_entry_replaces_newlines() {
    let mut manifest = AppManifest::minimal("example-app", "Example App", "C:/Example/App.exe");
    manifest.desktop.name = "Example\nApp".to_string();
    let entry = generate_desktop_entry(&manifest, "openntx").expect("desktop entry");
    assert!(entry.contains("Name=Example App"));
}

#[test]
fn desktop_exec_includes_notify_flag() {
    let manifest = AppManifest::minimal("test-notify-app", "Test Notify", "C:/Test/App.exe");
    let entry = generate_desktop_entry(&manifest, "openntx").expect("desktop entry");

    assert!(
        entry.contains("Exec=openntx run test-notify-app --notify"),
        "Exec line should include --notify flag"
    );
}
