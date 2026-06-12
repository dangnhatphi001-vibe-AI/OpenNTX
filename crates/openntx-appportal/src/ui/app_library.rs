use openntx_core::registry::RegisteredApp;

pub fn render(apps: &[RegisteredApp]) -> String {
    let mut lines = vec![
        "App Library".to_string(),
        "- List installed OpenNTX apps".to_string(),
        "- Run".to_string(),
        "- Settings".to_string(),
        "- Repair".to_string(),
        "- Package".to_string(),
        "- Remove".to_string(),
        "".to_string(),
        "V0.4 status: reads registered apps from the local OpenNTX app registry.".to_string(),
    ];

    if apps.is_empty() {
        lines.push("- Registered apps: none".to_string());
    } else {
        lines.push("- Registered apps:".to_string());
        for app in apps {
            lines.push(format!(
                "  - {} | {} | {} | {}",
                app.app_id, app.name, app.install_mode, app.architecture
            ));
        }
    }

    lines.join("\n")
}
