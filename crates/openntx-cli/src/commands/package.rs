use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::packaging::DebPackagePlan;
use openntx_core::{OpenNtxError, Result};

pub fn run(app_id: &str) -> Result<()> {
    if !is_valid_app_id(app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )));
    }

    let plan = DebPackagePlan::dry_run(app_id);
    output::title("OpenNTX Package Plan");
    output::field("App ID", app_id);
    output::field("App root", plan.layout.app_root.display());
    output::field("Manifest", plan.layout.manifest_path.display());
    output::field("Desktop entry", plan.layout.desktop_entry_path.display());
    output::field("Icon", plan.layout.icon_path.display());
    output::field("Runtime dependency", &plan.layout.runtime_dependency);
    output::field("Status", &plan.status);
    Ok(())
}
