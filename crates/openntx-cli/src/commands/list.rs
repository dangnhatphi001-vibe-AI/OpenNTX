use crate::output;
use openntx_core::registry::AppRegistry;
use openntx_core::Result;

pub fn run(json: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;

    if json {
        let apps = registry.list_apps_json()?;
        println!("{}", serde_json::to_string_pretty(&apps)?);
        return Ok(());
    }

    let apps = registry.list_apps()?;

    output::title("OpenNTX Registered Apps");
    output::field("Count", apps.len());
    if apps.is_empty() {
        output::field("Status", "no registered apps");
        return Ok(());
    }

    for app in apps {
        output::field(
            "App",
            format!(
                "{} | {} | {} | {} | {}",
                app.app_id,
                app.name,
                app.install_mode,
                app.architecture,
                if app.desktop_launcher_exists {
                    "desktop"
                } else {
                    "no-desktop"
                }
            ),
        );
    }

    Ok(())
}
