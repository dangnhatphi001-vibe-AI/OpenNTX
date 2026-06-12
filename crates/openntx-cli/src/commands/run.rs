use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::registry::AppRegistry;
use openntx_core::runtime::{create_registered_run_plan, RunPlanOptions, RunPlanReport};
use openntx_core::{OpenNtxError, Result};
use std::process::Command;

pub fn run(target: &str, json: bool, _log: bool, no_log: bool, notify: bool) -> Result<()> {
    if !is_valid_app_id(target) {
        return Err(OpenNtxError::InvalidInput(format!(
            "run expects a registered app id: {target}"
        )));
    }

    let registry = AppRegistry::from_env()?;
    let report =
        create_registered_run_plan(&registry, target, &RunPlanOptions { write_log: !no_log })?;

    if notify {
        notify_run_plan(&report, json);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human_report(&report);
    }

    Ok(())
}

fn print_human_report(report: &RunPlanReport) {
    output::title("OpenNTX Run Plan");
    output::field("App ID", &report.app_id);
    output::field("Name", &report.name);
    output::field("Executable", &report.executable_path);
    output::field("Architecture", &report.architecture);
    output::field("Install mode", &report.install_mode);
    output::field("Sandbox profile", &report.sandbox_profile);
    output::field("Imported DLL count", report.imported_dll_count);
    output::field("Desktop status", &report.desktop_status);
    output::field("Desktop entry", &report.desktop_entry_path);
    output::field("Backend", &report.backend);
    output::field("Status", &report.status);
    if let Some(log_path) = &report.log_path {
        output::field("Run-plan log", log_path);
    }
    output::blank();
    output::note(&report.message);
}

fn notify_run_plan(report: &RunPlanReport, json_mode: bool) {
    let summary = format!("OpenNTX: {}", report.name);
    let body = format!(
        "Runtime execution is not implemented. Prepared run plan for {}.",
        report.app_id
    );

    let notification_sent = Command::new("notify-send")
        .arg(&summary)
        .arg(&body)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !notification_sent && !json_mode {
        println!("Notification: {summary} - {body}");
    }
}
