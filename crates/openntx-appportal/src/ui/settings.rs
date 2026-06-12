use openntx_core::paths::OpenNtxPaths;

pub fn render(paths: &OpenNtxPaths) -> String {
    vec![
        "OpenNTX Settings".to_string(),
        "----------------".to_string(),
        "Default sandbox profile: standard".to_string(),
        "Runtime backend: not-implemented".to_string(),
        "Manifest generation: enabled".to_string(),
        "Compatibility database: local profiles planned".to_string(),
        format!("App registry: {}", paths.apps_root.display()),
        format!("Desktop entries: {}", paths.desktop_entries_dir.display()),
        format!("Diagnostics/logs: {}", paths.logs_root.display()),
        String::new(),
        "Runtime execution is not implemented in V0.7.".to_string(),
    ]
    .join("\n")
}
