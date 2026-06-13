use crate::output;
use openntx_core::registry::AppRegistry;
use openntx_core::Result;

pub fn run(app_id: &str, new_name: &str, yes: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;
    let dry_run = !yes;

    let result = registry.rename_app(app_id, new_name, dry_run)?;

    output::title("OpenNTX Rename");
    output::field("App ID", &result.app_id);
    output::field("Old name", &result.old_name);
    output::field("New name", &result.new_name);
    output::field(
        "Manifest updated",
        if result.updated_manifest {
            "yes"
        } else {
            "no (dry-run)"
        },
    );

    if dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to write the rename.");
    }

    Ok(())
}
