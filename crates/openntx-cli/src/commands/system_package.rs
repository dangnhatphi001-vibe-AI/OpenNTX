use crate::output;
use openntx_core::packaging::{build_system_deb, SystemDebOptions};
use openntx_core::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Subcommand)]
pub enum SystemPackageCommands {
    /// Build the complete OpenNTX system .deb package
    Build {
        /// Package version
        #[arg(long, default_value = "2.8.0")]
        version: String,
        /// Output directory for the .deb file
        #[arg(long, default_value = "target/debian")]
        output: PathBuf,
        /// Workspace root directory
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        /// Show package plan without building
        #[arg(long)]
        dry_run: bool,
        /// Skip cargo build (use existing release binaries)
        #[arg(long)]
        skip_build: bool,
        /// Custom maintainer string
        #[arg(long)]
        maintainer: Option<String>,
    },
    /// Show what files would be included in the system .deb
    Plan {
        /// Package version
        #[arg(long, default_value = "2.8.0")]
        version: String,
        /// Workspace root directory
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
}

pub fn execute(command: SystemPackageCommands) -> Result<()> {
    match command {
        SystemPackageCommands::Build {
            version,
            output,
            workspace,
            dry_run,
            skip_build,
            maintainer,
        } => build(version, output, workspace, dry_run, skip_build, maintainer),
        SystemPackageCommands::Plan { version, workspace } => plan(version, workspace),
    }
}

fn build(
    version: String,
    output: PathBuf,
    workspace: PathBuf,
    dry_run: bool,
    skip_build: bool,
    maintainer: Option<String>,
) -> Result<()> {
    let options = SystemDebOptions {
        version: version.clone(),
        workspace_root: workspace,
        output_dir: output.clone(),
        dry_run,
        skip_build,
        maintainer,
        description: None,
    };

    output::title("OpenNTX System Package Build");
    output::field("Package", "openntx");
    output::field("Version", &version);
    output::field("Architecture", "amd64");
    output::field("Output", output.display());
    output::field("Dry run", if dry_run { "yes" } else { "no" });
    output::field("Skip build", if skip_build { "yes" } else { "no" });
    output::blank();

    let result = build_system_deb(&options)?;

    output::blank();
    output::title("Build Result");
    output::field("Status", &result.status);
    output::field("Package name", &result.package_name);
    output::field("Version", &result.version);
    output::field("Architecture", &result.architecture);
    output::field("Files included", &result.file_count.to_string());
    output::field("Total size", &format!("{} bytes", result.total_size_bytes));

    if result.status == "built" {
        output::blank();
        output::field("Deb file", result.deb_path.display());
        output::success(&format!(
            "System .deb package '{}' v{} built successfully!",
            result.package_name, result.version
        ));
    } else {
        output::blank();
        output::info("Dry run complete. No .deb was produced.");
        output::field("Staging dir", result.staging_dir.display());
    }

    Ok(())
}

fn plan(version: String, workspace: PathBuf) -> Result<()> {
    let options = SystemDebOptions {
        version: version.clone(),
        workspace_root: workspace,
        dry_run: true,
        skip_build: true,
        ..SystemDebOptions::default()
    };

    let result = build_system_deb(&options)?;

    output::title("OpenNTX System Package Plan");
    output::field("Package", &result.package_name);
    output::field("Version", &result.version);
    output::field("Architecture", &result.architecture);
    output::blank();
    output::title("Package Layout");
    output::field("  ", "DEBIAN/control");
    output::field("  ", "DEBIAN/postinst");
    output::field("  ", "DEBIAN/prerm");
    output::field("  ", "usr/bin/openntx");
    output::field("  ", "usr/bin/openntx-gui");
    output::field("  ", "usr/bin/openntx-appportal");
    output::field("  ", "etc/openntx/sandbox.toml");
    output::field("  ", "var/lib/openntx/sandboxes/");

    Ok(())
}
