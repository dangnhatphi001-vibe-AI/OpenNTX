use openntx_core::manifest::{validate_manifest, AppManifest};

#[test]
fn validates_minimal_manifest() {
    let manifest = AppManifest::minimal("example-app", "Example App", "C:/Example/App.exe");
    validate_manifest(&manifest).expect("minimal manifest should be valid");
}

#[test]
fn rejects_invalid_app_id() {
    let mut manifest = AppManifest::minimal("Example App", "Example App", "C:/Example/App.exe");
    let error = validate_manifest(&manifest).expect_err("invalid app id should fail");
    assert!(error.to_string().contains("app_id"));

    manifest.app_id = "example-app".to_string();
    validate_manifest(&manifest).expect("fixed app id should pass");
}

#[test]
fn repository_examples_validate() {
    let manifest_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/manifests");
    for file_name in [
        "portable-app.openntx.json",
        "captured-installer.openntx.json",
    ] {
        let path = manifest_dir.join(file_name);
        let bytes = std::fs::read(&path).expect("example manifest should be readable");
        let manifest: AppManifest =
            serde_json::from_slice(&bytes).expect("example manifest should parse");
        validate_manifest(&manifest).expect("example manifest should validate");
    }
}
