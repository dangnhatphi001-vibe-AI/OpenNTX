use openntx_core::registry::RegisteredApp;

pub fn render(apps: &[RegisteredApp]) -> String {
    let mut lines = vec![
        "OpenNTX App Library".to_string(),
        "-------------------".to_string(),
        "Select a registered app by number to view details.".to_string(),
        "".to_string(),
    ];

    if apps.is_empty() {
        lines.push("No registered apps found.".to_string());
        lines.push(
            "Use Install Plan to register an analyzed PE/EXE without executing it.".to_string(),
        );
    } else {
        for (index, app) in apps.iter().enumerate() {
            lines.push(format!(
                "[{}] {} | {} | {} | {} | sandbox={} | dlls={} | {} | {}",
                index + 1,
                app.name,
                app.app_id,
                app.architecture,
                app.install_mode,
                app.sandbox_profile,
                app.imported_dll_count,
                if app.desktop_launcher_exists {
                    "desktop=present"
                } else {
                    "desktop=missing"
                },
                app.status
            ));
        }
    }

    lines.join("\n")
}
