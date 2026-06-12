use crate::output;
use clap::Subcommand;
use openntx_core::app_id::generate_app_id;
use openntx_core::manifest::{
    generate_manifest_from_pe, write_manifest_pretty, ManifestGenerationInput,
};
use openntx_core::pe::analyze_pe;
use openntx_core::{OpenNtxError, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum ManifestCommands {
    Generate {
        #[arg(value_name = "file.exe")]
        file: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, value_name = "path")]
        output: Option<PathBuf>,
    },
}

pub fn execute(command: ManifestCommands) -> Result<()> {
    match command {
        ManifestCommands::Generate { file, json, output } => {
            generate(&file, json, output.as_deref())
        }
    }
}

pub fn generate(file: &Path, json: bool, output_path: Option<&Path>) -> Result<()> {
    let analysis = analyze_pe(file)?;
    if !analysis.is_pe {
        return Err(OpenNtxError::Unsupported(format!(
            "manifest generation expects a Windows PE/EXE input; {} is {}",
            analysis.file_name, analysis.status
        )));
    }

    let display_name = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("windows-app");
    let app_id = generate_app_id(display_name, Some(&file.display().to_string()));
    let generated = generate_manifest_from_pe(ManifestGenerationInput {
        input_path: file,
        analysis: &analysis,
        app_id,
    })?;

    if let Some(path) = output_path {
        write_manifest_pretty(path, &generated.manifest)?;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&generated.manifest)?);
        return Ok(());
    }

    output::title("OpenNTX Manifest Generate");
    output::field("Input", &analysis.file_name);
    output::field("App ID", &generated.manifest.app_id);
    output::field("Architecture", &generated.manifest.architecture);
    if let Some(subsystem) = &analysis.subsystem {
        output::field("Subsystem", subsystem);
    }
    output::field("Install mode", &generated.manifest.install_mode);
    output::field("Install mode reason", &generated.install_mode_reason);
    output::field(
        "Imported DLLs",
        generated.manifest.diagnostics.imported_dlls.len(),
    );
    output::field("Sandbox", &generated.manifest.sandbox.profile);
    output::field(
        "Desktop launcher",
        if generated.manifest.desktop.create_launcher {
            "planned"
        } else {
            "disabled"
        },
    );
    if let Some(path) = output_path {
        output::field("Written", path.display());
    }
    output::field("Status", "generated / analysis-only");
    output::blank();
    output::note(
        "Runtime execution is not implemented. This command only generates OpenNTX metadata for future install flows.",
    );

    Ok(())
}
