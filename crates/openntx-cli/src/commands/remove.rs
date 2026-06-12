use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::{OpenNtxError, Result};

pub fn run(app_id: &str) -> Result<()> {
    if !is_valid_app_id(app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )));
    }

    let paths = OpenNtxPaths::from_env()?;
    output::title("OpenNTX Remove Plan");
    output::field("App ID", app_id);
    output::field("App data", paths.app_dir(app_id).display());
    output::field("Desktop entry", paths.desktop_entry_path(app_id).display());
    output::field("Status", "planned / not implemented");
    output::blank();
    output::note(
        "V0.2 does not remove app data. Future versions will require explicit confirmation.",
    );
    Ok(())
}
