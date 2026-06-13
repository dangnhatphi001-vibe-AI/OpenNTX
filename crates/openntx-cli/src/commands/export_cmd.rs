use crate::output;
use openntx_core::registry::AppRegistry;
use openntx_core::Result;
use std::path::Path;

pub fn run(app_id: &str, output_path: &Path, yes: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;
    let dry_run = !yes;

    let result = registry.export_bundle(app_id, output_path, dry_run)?;

    output::title("OpenNTX Export");
    output::field("App ID", &result.app_id);
    output::field("Bundle path", result.bundle_path.display());
    output::field("Bundle format", &result.bundle_format);
    output::blank();
    output::title("Files included:");
    for file in &result.files_included {
        output::field("  ", file);
    }

    if dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to create the bundle.");
    }

    Ok(())
}
