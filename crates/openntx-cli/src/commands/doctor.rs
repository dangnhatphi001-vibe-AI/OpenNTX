use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::pe::analyze_pe;
use openntx_core::{OpenNtxError, Result};
use std::path::Path;

pub fn run(target: &str) -> Result<()> {
    let path = Path::new(target);
    output::title("OpenNTX Doctor");

    if path.exists() {
        let analysis = analyze_pe(path)?;
        output::field("Target", target);
        output::field("Kind", "file");
        output::field("PE", analysis.is_pe);
        output::field("Status", &analysis.status);
        for warning in &analysis.warnings {
            output::field("Warning", warning);
        }
    } else if is_valid_app_id(target) {
        output::field("Target", target);
        output::field("Kind", "app-id");
        output::field(
            "Manifest",
            "expected under ~/.local/share/openntx/apps/<app-id>/manifest.json",
        );
        output::field("Runtime", "not implemented in V0.2");
    } else {
        return Err(OpenNtxError::InvalidInput(format!(
            "target is not an existing file or valid app id: {target}"
        )));
    }

    output::blank();
    output::note("Doctor currently performs static checks only.");
    Ok(())
}
