use crate::output;
use openntx_core::app_id::generate_app_id;
use openntx_core::paths::OpenNtxPaths;
use openntx_core::pe::analyze_pe;
use openntx_core::{OpenNtxError, Result};
use std::path::Path;

pub fn run(file: &Path) -> Result<()> {
    let analysis = analyze_pe(file)?;
    if !analysis.is_pe {
        return Err(OpenNtxError::Unsupported(format!(
            "install expects a Windows PE/EXE input; {} is {}",
            analysis.file_name, analysis.status
        )));
    }

    let display_name = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("windows-app");
    let app_id = generate_app_id(display_name, Some(&file.display().to_string()));
    let paths = OpenNtxPaths::from_env()?;
    let manifest_target = paths.manifest_path(&app_id);

    output::title("OpenNTX Install Plan");
    output::field("Input", &analysis.file_name);
    output::field("App ID", &app_id);
    output::field("Mode", &analysis.suggested_mode);
    output::field("Sandbox", "standard");
    output::field("Manifest target", manifest_target.display());
    output::field("Desktop integration", "planned");
    output::field("Status", "dry-run / not implemented");
    output::blank();
    output::note(
        "Runtime execution is not implemented. This command currently validates input and prepares a future execution plan.",
    );

    Ok(())
}
