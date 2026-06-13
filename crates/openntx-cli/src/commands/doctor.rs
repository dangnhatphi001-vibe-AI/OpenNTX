use crate::output;
use openntx_core::app_id::is_valid_app_id;
use openntx_core::doctor::{app_doctor, global_doctor, repair_app};
use openntx_core::registry::AppRegistry;
use openntx_core::{OpenNtxError, Result};

pub fn run(target: Option<&str>, json: bool, repair: bool, yes: bool) -> Result<()> {
    let registry = AppRegistry::from_env()?;

    match target {
        None => global(&registry, json),
        Some(app_id) => {
            if is_valid_app_id(app_id) {
                if repair {
                    app_repair(&registry, app_id, yes)
                } else {
                    app_check(&registry, app_id, json)
                }
            } else {
                Err(OpenNtxError::InvalidInput(format!(
                    "target is not a valid app id: {app_id}"
                )))
            }
        }
    }
}

fn global(registry: &AppRegistry, json: bool) -> Result<()> {
    let report = global_doctor(registry)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    output::title("OpenNTX Doctor - Global");
    output::blank();
    output::field(
        "Data directory",
        format!(
            "{} ({})",
            report.data_dir_path,
            if report.data_dir_exists {
                "exists"
            } else {
                "missing"
            }
        ),
    );
    output::field("Registered apps", report.app_count);
    output::field("Broken apps", report.broken_app_count);
    if !report.broken_apps.is_empty() {
        for app_id in &report.broken_apps {
            output::field("  Broken", app_id);
        }
    }
    output::field(
        "Logs directory",
        format!(
            "{} ({}, {})",
            report.logs_dir_path,
            if report.logs_dir_exists {
                "exists"
            } else {
                "missing"
            },
            if report.logs_dir_writable {
                "writable"
            } else {
                "not writable"
            }
        ),
    );
    output::field(
        "Desktop entries directory",
        if report.desktop_entries_dir_exists {
            "exists"
        } else {
            "missing"
        },
    );
    output::field(
        "dpkg-deb",
        if report.dpkg_deb_available {
            "available"
        } else {
            "not available"
        },
    );
    output::field(
        "notify-send",
        if report.notify_send_available {
            "available"
        } else {
            "not available"
        },
    );

    if !report.warnings.is_empty() {
        output::blank();
        output::title("Warnings:");
        for warning in &report.warnings {
            output::field("  !", warning);
        }
    }

    output::blank();
    output::field("Status", &report.status);
    Ok(())
}

fn app_check(registry: &AppRegistry, app_id: &str, json: bool) -> Result<()> {
    let report = app_doctor(registry, app_id)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    output::title(&format!("OpenNTX Doctor - App: {app_id}"));
    output::blank();
    output::field("App name", &report.app_name);
    output::field(
        "Manifest",
        format!(
            "{} ({})",
            if report.manifest_exists {
                "exists"
            } else {
                "missing"
            },
            if report.manifest_is_regular_file {
                "regular file"
            } else {
                "not regular"
            }
        ),
    );
    output::field(
        "Manifest valid",
        if report.manifest_valid { "yes" } else { "no" },
    );
    if let Some(err) = &report.manifest_validation_error {
        if !report.manifest_valid {
            output::field("Validation error", err);
        }
    }
    output::field(
        "Install plan",
        if report.install_plan_exists {
            "exists"
        } else {
            "missing"
        },
    );
    output::field(
        "drive_c",
        format!(
            "{} ({})",
            if report.drive_c_exists {
                "exists"
            } else {
                "missing"
            },
            if report.drive_c_is_real_dir {
                "real dir"
            } else {
                "not real dir"
            }
        ),
    );
    output::field(
        "Registry",
        format!(
            "{} ({})",
            if report.registry_exists {
                "exists"
            } else {
                "missing"
            },
            if report.registry_is_real_dir {
                "real dir"
            } else {
                "not real dir"
            }
        ),
    );
    output::field(
        "Capture directory",
        if report.capture_dir_exists {
            "exists"
        } else {
            "missing"
        },
    );
    output::field(
        "Desktop entry",
        if report.desktop_entry_exists {
            "present"
        } else {
            "missing"
        },
    );
    output::field(
        "Package build possible",
        if report.package_build_possible {
            "yes"
        } else {
            "no"
        },
    );
    output::field(
        "Log directory writable",
        if report.log_dir_writable { "yes" } else { "no" },
    );

    if !report.unsafe_symlinks.is_empty() {
        output::blank();
        output::title("Unsafe symlinks:");
        for link in &report.unsafe_symlinks {
            output::field("  !!", link);
        }
    }

    if !report.warnings.is_empty() {
        output::blank();
        output::title("Warnings:");
        for warning in &report.warnings {
            output::field("  !", warning);
        }
    }

    output::blank();
    output::field("Status", &report.status);
    Ok(())
}

fn app_repair(registry: &AppRegistry, app_id: &str, yes: bool) -> Result<()> {
    let dry_run = !yes;
    let plan = repair_app(registry, app_id, dry_run)?;

    output::title(&format!("OpenNTX Doctor Repair - App: {app_id}"));
    output::blank();

    if !plan.unsafe_symlinks_found.is_empty() {
        output::title("UNSAFE SYMLINKS FOUND - REPAIR REFUSED:");
        for link in &plan.unsafe_symlinks_found {
            output::field("  !!", link);
        }
        output::blank();
        output::note("Remove unsafe symlinks manually before repairing.");
        return Ok(());
    }

    if plan.actions.is_empty() {
        output::note("No repairs needed. App is healthy.");
        return Ok(());
    }

    output::title("Repair actions:");
    for action in &plan.actions {
        output::field(
            "  ",
            format!(
                "[{}] {} -> {}",
                if action.applied { "done" } else { "planned" },
                action.description,
                action.path
            ),
        );
    }

    output::blank();
    if dry_run {
        output::note("Dry-run only. Pass --yes to apply repairs.");
    } else {
        output::note(&format!("Applied {} repair action(s).", plan.actions.len()));
    }

    Ok(())
}
