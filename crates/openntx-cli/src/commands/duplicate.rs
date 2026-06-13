use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::registry::AppRegistry;
use openntx_core::{OpenNtxError, Result};

pub fn run(app_id: &str, new_app_id: &str, yes: bool) -> Result<()> {
    if !is_valid_app_id(new_app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid new app id: {new_app_id}"
        )));
    }

    let registry = AppRegistry::from_env()?;
    let dry_run = !yes;

    let result = registry.duplicate_app(app_id, new_app_id, dry_run)?;

    output::title("OpenNTX Duplicate");
    output::field("Source app ID", &result.source_app_id);
    output::field("New app ID", &result.new_app_id);
    output::field("New app directory", result.new_app_dir.display());
    output::field("New manifest", result.new_manifest_path.display());

    if dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to create the duplicate.");
    }

    Ok(())
}
