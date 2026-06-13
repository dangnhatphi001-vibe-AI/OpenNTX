use crate::output;
use openntx_core::registry::AppRegistry;
use openntx_core::Result;
use std::path::Path;

pub fn run(bundle_path: &Path, rename_as: Option<&str>, yes: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;
    let dry_run = !yes;

    let result = registry.import_bundle(bundle_path, rename_as, dry_run)?;

    output::title("OpenNTX Import");
    output::field("App ID", &result.app_id);
    output::field("App directory", result.app_dir.display());
    output::field("Manifest", result.manifest_path.display());

    if dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to import the bundle.");
    }

    Ok(())
}
