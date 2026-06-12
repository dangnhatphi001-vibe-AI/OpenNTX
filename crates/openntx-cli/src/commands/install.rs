use crate::output;
use openntx_core::app_id::generate_app_id;
use openntx_core::manifest::{generate_manifest_from_pe, ManifestGenerationInput};
use openntx_core::paths::OpenNtxPaths;
use openntx_core::pe::analyze_pe;
use openntx_core::registry::AppRegistry;
use openntx_core::{OpenNtxError, Result};
use std::path::Path;

pub fn run(file: &Path, write_plan: bool) -> Result<()> {
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
    let generated = generate_manifest_from_pe(ManifestGenerationInput {
        input_path: file,
        analysis: &analysis,
        app_id: app_id.clone(),
    })?;
    let paths = OpenNtxPaths::from_env()?;
    let manifest_target = paths.manifest_path(&app_id);
    let registry = AppRegistry::new(paths);
    let install_plan = registry.build_install_plan(
        &generated.manifest,
        analysis.subsystem.as_ref().map(ToString::to_string),
    );

    output::title(if write_plan {
        "OpenNTX Install Plan Writer"
    } else {
        "OpenNTX Install Plan"
    });
    output::field("Input", &analysis.file_name);
    output::field("App ID", &app_id);
    output::field("Manifest target", manifest_target.display());
    output::field("Install mode", &generated.manifest.install_mode);
    output::field("Architecture", &generated.manifest.architecture);
    if let Some(subsystem) = &analysis.subsystem {
        output::field("Subsystem", subsystem);
    }
    output::field(
        "Imported DLL count",
        generated.manifest.diagnostics.imported_dlls.len(),
    );
    output::field("Desktop integration", "planned");
    output::field("Sandbox profile", &generated.manifest.sandbox.profile);
    if write_plan {
        let result = registry.register_plan(&generated.manifest, &install_plan)?;
        output::field("App directory", result.app_dir.display());
        output::field("Install plan", result.install_plan_path.display());
        output::field("Metadata", result.metadata_path.display());
        output::field("drive_c", result.drive_c_path.display());
        output::field("registry", result.registry_path.display());
        output::field("logs", result.logs_path.display());
        output::field("Status", "written / analysis-only");
    } else {
        output::field("Status", "dry-run / not written");
    }
    output::blank();
    output::note(
        "Runtime execution is not implemented. This command validates input and prepares OpenNTX metadata only.",
    );

    Ok(())
}
