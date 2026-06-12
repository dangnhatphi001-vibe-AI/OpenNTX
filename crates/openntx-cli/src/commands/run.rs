use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::manifest::AppManifest;
use openntx_core::pe::analyze_pe;
use openntx_core::runtime::{NotImplementedBackend, RuntimeBackend};
use openntx_core::{OpenNtxError, Result};
use std::path::Path;

pub fn run(target: &str) -> Result<()> {
    let path = Path::new(target);
    let manifest = if path.exists() {
        let analysis = analyze_pe(path)?;
        if !analysis.is_pe {
            return Err(OpenNtxError::Unsupported(format!(
                "run expects an app id or Windows PE/EXE; {} is {}",
                analysis.file_name, analysis.status
            )));
        }
        AppManifest::minimal("run-once", &analysis.file_name, &path.display().to_string())
    } else {
        if !is_valid_app_id(target) {
            return Err(OpenNtxError::InvalidInput(format!(
                "target is not an existing file or valid app id: {target}"
            )));
        }
        AppManifest::minimal(target, target, "manifest executable path")
    };

    let backend = NotImplementedBackend;
    let plan = backend.plan_execution(&manifest);

    output::title("OpenNTX Run Plan");
    output::field("Target", target);
    output::field("Backend", plan.backend);
    output::field("Executable", plan.executable);
    output::field("Status", "dry-run / not implemented");
    output::blank();
    output::note(&plan.message);
    Ok(())
}
