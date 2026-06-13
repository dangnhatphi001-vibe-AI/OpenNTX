use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::packaging::{build_deb_package, DebBuildOptions};
use openntx_core::registry::AppRegistry;
use openntx_core::{OpenNtxError, Result};
use std::path::PathBuf;
use std::process::Command;

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
        #[arg(long, default_value = "1.0.0-alpha")]
        version: String,
        /// Show package plan without building
        #[arg(long)]
        dry_run: bool,
        /// Actually build the .deb package
        #[arg(long)]
        yes: bool,
        /// Keep staging directory in dist after building
        #[arg(long)]
        keep_staging: bool,
    },
    /// Inspect a .deb package
    Inspect {
        /// Path to the .deb file
        #[arg(value_name = "deb-file")]
        deb_file: PathBuf,
    },
    /// Clean package build artifacts
    Clean {
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
            keep_staging,
        } => build(&app_id, output, version, dry_run, yes, keep_staging),
        PackageCommands::Inspect { deb_file } => inspect(&deb_file),
        PackageCommands::Clean { yes } => clean(yes),
    }
}

fn build(
    app_id: &str,
    output: PathBuf,
    version: String,
    dry_run: bool,
    yes: bool,
    keep_staging: bool,
) -> Result<()> {
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
        output_dir: output.clone(),
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
    } else if keep_staging && plan.staging_dir.exists() {
        let staging_copy = output.join(format!("staging-{}", app_id));
        if staging_copy.exists() {
            std::fs::remove_dir_all(&staging_copy)
                .map_err(|source| OpenNtxError::io(&staging_copy, source))?;
        }
        // Copy staging to dist for inspection
        copy_dir_recursive(&plan.staging_dir, &staging_copy)?;
        output::blank();
        output::field("Staging preserved at", staging_copy.display());
    }

    Ok(())
}

fn inspect(deb_file: &PathBuf) -> Result<()> {
    if !deb_file.exists() {
        return Err(OpenNtxError::InvalidInput(format!(
            "deb file does not exist: {}",
            deb_file.display()
        )));
    }

    output::title("OpenNTX Package Inspect");
    output::field("Package", deb_file.display());

    // Check dpkg-deb availability
    let dpkg_available = Command::new("which")
        .arg("dpkg-deb")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !dpkg_available {
        output::blank();
        output::note("dpkg-deb is not available. Install dpkg-dev to inspect .deb packages.");
        return Err(OpenNtxError::ToolNotAvailable(
            "dpkg-deb is not available".to_string(),
        ));
    }

    // Run dpkg-deb -I
    output::blank();
    output::title("Package info (dpkg-deb -I):");
    match Command::new("dpkg-deb")
        .args(["-I", &deb_file.display().to_string()])
        .output()
    {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            for line in stdout.lines() {
                println!("  {line}");
            }
        }
        Err(e) => {
            output::field("Error", format!("Failed to run dpkg-deb -I: {e}"));
        }
    }

    // Run dpkg-deb -c
    output::blank();
    output::title("Package contents (dpkg-deb -c):");
    match Command::new("dpkg-deb")
        .args(["-c", &deb_file.display().to_string()])
        .output()
    {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            for line in stdout.lines() {
                println!("  {line}");
            }
        }
        Err(e) => {
            output::field("Error", format!("Failed to run dpkg-deb -c: {e}"));
        }
    }

    Ok(())
}

fn clean(yes: bool) -> Result<()> {
    use std::fs;

    let dist_dir = PathBuf::from("dist");
    let mut artifacts = Vec::new();

    if dist_dir.exists() {
        if let Ok(entries) = fs::read_dir(&dist_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.starts_with("openntx-") && name.ends_with(".deb") {
                    artifacts.push(path.clone());
                }
                if name.starts_with("staging-") {
                    artifacts.push(path);
                }
            }
        }
    }

    // Also check temp dir for staging artifacts
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.starts_with("openntx-package-build-") {
                artifacts.push(path);
            }
        }
    }

    output::title("OpenNTX Package Clean");
    output::field("Artifacts found", artifacts.len());
    for path in &artifacts {
        output::field("  ", path.display());
    }

    if !yes {
        output::blank();
        output::note("Dry-run only. Pass --yes to remove package artifacts.");
        return Ok(());
    }

    for path in &artifacts {
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|source| OpenNtxError::io(path, source))?;
        } else {
            fs::remove_file(path).map_err(|source| OpenNtxError::io(path, source))?;
        }
    }

    output::blank();
    output::note(&format!("Removed {} artifact(s).", artifacts.len()));
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    use std::fs;
    fs::create_dir_all(dst).map_err(|source| OpenNtxError::io(dst, source))?;
    let entries = fs::read_dir(src).map_err(|source| OpenNtxError::io(src, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| OpenNtxError::io(src, source))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|source| OpenNtxError::io(&dst_path, source))?;
        }
    }
    Ok(())
}
