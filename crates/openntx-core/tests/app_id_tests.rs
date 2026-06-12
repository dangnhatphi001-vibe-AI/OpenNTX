use openntx_core::app_id::{generate_app_id, is_valid_app_id};

#[test]
fn app_id_generation_is_stable() {
    let first = generate_app_id("Example Windows App", Some("setup.exe"));
    let second = generate_app_id("Example Windows App", Some("setup.exe"));
    assert_eq!(first, second);
    assert!(is_valid_app_id(&first));
}

#[test]
fn app_id_generation_handles_non_ascii_names() {
    let app_id = generate_app_id("测试应用", Some("setup.exe"));
    assert!(app_id.starts_with("app-"));
    assert!(is_valid_app_id(&app_id));
}
