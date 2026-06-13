use crate::output;
use openntx_core::registry::AppRegistry;
use openntx_core::Result;

pub fn run(app_id: &str, json: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;

    if json {
        let summary = registry.show_app_json(app_id)?;
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    let manifest = registry.load_manifest(app_id)?;
    let desktop_entry = registry.paths().desktop_entry_path(app_id);

    output::title("OpenNTX App");
    output::field("App ID", &manifest.app_id);
    output::field("Name", &manifest.name);
    output::field("Architecture", &manifest.architecture);
    output::field("Install mode", &manifest.install_mode);
    output::field("Executable", &manifest.executable.path);
    output::field("Sandbox", &manifest.sandbox.profile);
    output::field("Imported DLLs", manifest.diagnostics.imported_dlls.len());
    output::field("Manifest", registry.paths().manifest_path(app_id).display());
    output::field("Desktop entry", desktop_entry.display());
    output::field(
        "Desktop status",
        if desktop_entry.exists() {
            "present"
        } else {
            "not created"
        },
    );
    output::field("Status", "registered / analysis-only");
    Ok(())
}
