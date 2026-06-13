use crate::output;
use clap::Subcommand;
use openntx_core::logs::{clean_logs, list_logs, show_log};
use openntx_core::registry::AppRegistry;
use openntx_core::Result;

#[derive(Debug, Subcommand)]
pub enum LogsCommands {
    /// List run-plan logs
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show a specific log
    Show {
        /// Log file path or app-id
        #[arg(value_name = "path-or-app-id")]
        target: String,
        #[arg(long)]
        json: bool,
    },
    /// Clean old logs
    Clean {
        /// Delete logs older than this many days
        #[arg(long, value_name = "days")]
        older_than_days: u64,
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(command: LogsCommands) -> Result<()> {
    let registry = AppRegistry::from_env()?;

    match command {
        LogsCommands::List { json } => list(&registry, json),
        LogsCommands::Show { target, json } => show(&registry, &target, json),
        LogsCommands::Clean {
            older_than_days,
            yes,
        } => clean(&registry, older_than_days, yes),
    }
}

fn list(registry: &AppRegistry, json: bool) -> Result<()> {
    let logs = list_logs(registry)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&logs)?);
        return Ok(());
    }

    output::title("OpenNTX Run-Plan Logs");
    output::field("Count", logs.len());
    if logs.is_empty() {
        output::field("Status", "no logs found");
        return Ok(());
    }

    output::blank();
    for log in &logs {
        output::field(
            "Log",
            format!(
                "{} | {} | {} | {}",
                log.timestamp, log.app_id, log.app_name, log.status
            ),
        );
    }
    Ok(())
}

fn show(registry: &AppRegistry, target: &str, json: bool) -> Result<()> {
    let report = show_log(registry, target)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    output::title("OpenNTX Run-Plan Log");
    output::field("App ID", &report.app_id);
    output::field("Name", &report.name);
    output::field("Target", &report.target);
    output::field("Timestamp", &report.timestamp);
    output::field("Executable", &report.executable_path);
    output::field("Architecture", &report.architecture);
    output::field("Install mode", &report.install_mode);
    output::field("Sandbox profile", &report.sandbox_profile);
    output::field("Backend", &report.backend);
    output::field("Status", &report.status);
    if let Some(log_path) = &report.log_path {
        output::field("Log path", log_path);
    }
    output::blank();
    output::note(&report.message);
    Ok(())
}

fn clean(registry: &AppRegistry, older_than_days: u64, yes: bool) -> Result<()> {
    let dry_run = !yes;
    let deleted = clean_logs(registry, older_than_days, dry_run)?;

    output::title("OpenNTX Logs Clean");
    output::field("Older than (days)", older_than_days);
    output::field("Logs to remove", deleted.len());
    if !deleted.is_empty() {
        for name in &deleted {
            output::field("  ", name);
        }
    }

    if dry_run {
        output::blank();
        output::note("Dry-run only. Pass --yes to delete old logs.");
    } else {
        output::blank();
        output::note(&format!("Deleted {} log file(s).", deleted.len()));
    }

    Ok(())
}
