use crate::output;
use openntx_core::pe::analyze_pe;
use openntx_core::Result;
use std::path::Path;

pub fn run(file: &Path) -> Result<()> {
    let analysis = analyze_pe(file)?;

    output::title("OpenNTX Analyze");
    output::field("File", &analysis.file_name);
    output::field("Format", &analysis.format);
    output::field("Architecture", &analysis.architecture);
    output::field("Type", &analysis.image_kind);
    if let Some(subsystem) = &analysis.subsystem {
        output::field("Subsystem", subsystem);
    }
    output::field("Suggested mode", &analysis.suggested_mode);
    output::field("Status", &analysis.status);

    for warning in &analysis.warnings {
        output::field("Warning", warning);
    }

    output::blank();
    output::note(
        "Runtime execution is not implemented in V0.1. This analysis prepares metadata for future OpenNTX install flows.",
    );
    Ok(())
}
