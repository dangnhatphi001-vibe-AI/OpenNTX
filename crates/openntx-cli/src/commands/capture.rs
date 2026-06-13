use crate::output;
use openntx_core::capture::CaptureRegistryService;
use openntx_core::Result;

#[derive(Debug, clap::Subcommand)]
pub enum CaptureCommands {
    /// Snapshot current app state (before a capture step)
    SnapshotBefore {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Snapshot current app state (after a capture step)
    SnapshotAfter {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Compute diff between before and after snapshots
    Diff {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
        /// Show compact summary only
        #[arg(long)]
        summary: bool,
    },
    /// Generate a capture report from diff
    Report {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Show capture status for a registered app
    Status {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Clean capture artifacts for an app
    Clean {
        /// The registered app ID
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(command: CaptureCommands) -> Result<()> {
    let service = CaptureRegistryService::from_env()?;
    match command {
        CaptureCommands::SnapshotBefore { app_id, json } => {
            let result = service.snapshot_before(&app_id)?;
            if json {
                let out = serde_json::json!({
                    "status": "ok",
                    "app_id": app_id,
                    "snapshot_path": result.snapshot_path.display().to_string(),
                    "entries": result.snapshot.entries.len(),
                    "errors": result.snapshot.errors.len(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                output::title("OpenNTX Capture Snapshot Before");
                output::field("App ID", &app_id);
                output::field("Snapshot path", result.snapshot_path.display());
                output::field("Entries", result.snapshot.entries.len());
                output::field("Errors", result.snapshot.errors.len());
                output::blank();
                output::note("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
            }
        }
        CaptureCommands::SnapshotAfter { app_id, json } => {
            let result = service.snapshot_after(&app_id)?;
            if json {
                let out = serde_json::json!({
                    "status": "ok",
                    "app_id": app_id,
                    "snapshot_path": result.snapshot_path.display().to_string(),
                    "entries": result.snapshot.entries.len(),
                    "errors": result.snapshot.errors.len(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                output::title("OpenNTX Capture Snapshot After");
                output::field("App ID", &app_id);
                output::field("Snapshot path", result.snapshot_path.display());
                output::field("Entries", result.snapshot.entries.len());
                output::field("Errors", result.snapshot.errors.len());
                output::blank();
                output::note("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
            }
        }
        CaptureCommands::Diff {
            app_id,
            json,
            summary,
        } => {
            let result = service.diff(&app_id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result.diff)?);
            } else if summary {
                output::title("OpenNTX Capture Diff Summary");
                output::field("App ID", &app_id);
                output::field("Files created", result.diff.files_created.len());
                output::field("Files removed", result.diff.files_removed.len());
                output::field("Files modified", result.diff.files_modified.len());
                output::field("Directories created", result.diff.directories_created.len());
                output::field("Directories removed", result.diff.directories_removed.len());
                output::field("Symlinks created", result.diff.symlinks_created.len());
                output::field("Symlinks removed", result.diff.symlinks_removed.len());
                output::field(
                    "Registry files changed",
                    result.diff.registry_files_changed.len(),
                );
            } else {
                output::title("OpenNTX Capture Diff");
                output::field("App ID", &app_id);
                output::field("Diff path", result.diff_path.display());
                output::blank();
                output::field("Files created", result.diff.files_created.len());
                output::field("Files removed", result.diff.files_removed.len());
                output::field("Files modified", result.diff.files_modified.len());
                output::field("Directories created", result.diff.directories_created.len());
                output::field("Directories removed", result.diff.directories_removed.len());
                output::field("Symlinks created", result.diff.symlinks_created.len());
                output::field("Symlinks removed", result.diff.symlinks_removed.len());
                output::field(
                    "Registry files changed",
                    result.diff.registry_files_changed.len(),
                );
                if !result.diff.warnings.is_empty() {
                    output::blank();
                    for w in &result.diff.warnings {
                        output::field("Warning", w);
                    }
                }
                if !result.diff.errors.is_empty() {
                    output::blank();
                    for e in &result.diff.errors {
                        output::field("Error", e);
                    }
                }
            }
        }
        CaptureCommands::Report { app_id, json } => {
            let result = service.report(&app_id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result.report)?);
            } else {
                output::title("OpenNTX Capture Report");
                output::field("App ID", &app_id);
                output::field("App name", &result.report.app_name);
                output::field("Report path", result.report_path.display());
                output::blank();
                output::field("Files created", result.report.files_created.len());
                output::field("Files modified", result.report.files_modified.len());
                output::field("Files removed", result.report.files_removed.len());
                output::field(
                    "Registry files changed",
                    result.diff.registry_files_changed.len(),
                );
                output::field("Status", &result.report.status);
                if !result.report.warnings.is_empty() {
                    output::blank();
                    for w in &result.report.warnings {
                        output::field("Warning", w);
                    }
                }
            }
        }
        CaptureCommands::Status { app_id, json } => {
            let status = service.status(&app_id)?;
            if json {
                let out = serde_json::json!({
                    "app_id": status.app_id,
                    "app_name": status.app_name,
                    "app_dir": status.app_dir.display().to_string(),
                    "capture_dir": status.capture_dir.display().to_string(),
                    "snapshot_before": status.snapshot_before,
                    "snapshot_after": status.snapshot_after,
                    "diff": status.diff,
                    "report": status.report,
                    "status": "analysis-only / no runtime execution",
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                output::title("OpenNTX Capture Status");
                output::field("App ID", &status.app_id);
                output::field("App name", &status.app_name);
                output::field("App directory", status.app_dir.display());
                output::field("Capture directory", status.capture_dir.display());
                output::blank();
                output::field(
                    "Snapshot before",
                    if status.snapshot_before {
                        "present"
                    } else {
                        "missing"
                    },
                );
                output::field(
                    "Snapshot after",
                    if status.snapshot_after {
                        "present"
                    } else {
                        "missing"
                    },
                );
                output::field("Diff", if status.diff { "present" } else { "missing" });
                output::field("Report", if status.report { "present" } else { "missing" });
                output::blank();
                output::note("Capture snapshots only inspect OpenNTX-managed app directories. No installer is executed.");
            }
        }
        CaptureCommands::Clean { app_id, yes } => {
            clean_capture(&app_id, yes)?;
        }
    }
    Ok(())
}

fn clean_capture(app_id: &str, yes: bool) -> Result<()> {
    use openntx_core::registry::AppRegistry;
    use std::fs;

    let registry = AppRegistry::from_env()?;
    let capture_dir = registry.paths().capture_dir(app_id);

    if !capture_dir.exists() {
        output::title("OpenNTX Capture Clean");
        output::field("App ID", app_id);
        output::note("No capture directory found. Nothing to clean.");
        return Ok(());
    }

    let files_to_clean = [
        "snapshot-before.json",
        "snapshot-after.json",
        "capture-diff.json",
        "capture-report.json",
    ];

    let mut found = Vec::new();
    for name in &files_to_clean {
        let path = capture_dir.join(name);
        if path.exists() {
            found.push(path);
        }
    }

    output::title("OpenNTX Capture Clean");
    output::field("App ID", app_id);
    output::field("Files to remove", found.len());
    for path in &found {
        output::field("  ", path.display());
    }

    if !yes {
        output::blank();
        output::note("Dry-run only. Pass --yes to remove capture artifacts.");
        return Ok(());
    }

    for path in &found {
        fs::remove_file(path).map_err(|source| openntx_core::OpenNtxError::io(path, source))?;
    }

    output::blank();
    output::note(&format!("Removed {} capture artifact(s).", found.len()));
    Ok(())
}
