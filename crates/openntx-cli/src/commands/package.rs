use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::packaging::{build_deb_package, DebBuildOptions};
use openntx_core::registry::AppRegistry;
use openntx_core::{OpenNtxError, Result};
use std::path::PathBuf;

#[derive(Debug, clap::Subcommand)]
pub enum PackageCommands {
    /// Build a .deb package for a registered app
    Build {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        /// Output directory for the .deb file
        #[arg(long, default_value = "dist")]
        output: PathBuf,
        /// Package version
        #[arg(long, default_value = "0.9.0")]
        version: String,
        /// Show package plan without building
        #[arg(long)]
        dry_run: bool,
        /// Actually build the .deb package
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(command: PackageCommands) -> Result<()> {
    match command {
        PackageCommands::Build {
            app_id,
            output,
            version,
            dry_run,
            yes,
        } => build(&app_id, output, version, dry_run, yes),
    }
}

fn build(app_id: &str, output: PathBuf, version: String, dry_run: bool, yes: bool) -> Result<()> {
    if !is_valid_app_id(app_id) {
        return Err(OpenNtxError::InvalidInput(format!(
            "invalid app id: {app_id}"
        )));
    }

    let registry = AppRegistry::from_env()?;
    let manifest = registry.load_manifest(app_id)?;

    let is_dry_run = dry_run || !yes;

    let options = DebBuildOptions {
        app_id: app_id.to_string(),
        version,
        output_dir: output,
        dry_run: is_dry_run,
    };

    let plan = build_deb_package(&registry, &options)?;

    output::title("OpenNTX Package Build");
    output::field("App ID", app_id);
    output::field("App name", &manifest.name);
    output::field("Package name", &plan.package_name);
    output::field("Version", &plan.version);
    output::field("Deb filename", &plan.deb_filename);
    output::field("Staging dir", plan.staging_dir.display());
    output::field("Output dir", plan.output_dir.display());
    output::blank();
    output::title("Files to package:");
    for file in &plan.files_to_package {
        output::field("  ", file);
    }
    output::blank();
    output::field("Layout app root", plan.layout.app_root.display());
    output::field("Layout manifest", plan.layout.manifest_path.display());
    output::field(
        "Layout desktop entry",
        plan.layout.desktop_entry_path.display(),
    );
    output::field("Runtime dependency", &plan.layout.runtime_dependency);
    output::blank();
    output::field("Status", &plan.status);

    if is_dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to actually build the .deb package.");
    }

    Ok(())
}
