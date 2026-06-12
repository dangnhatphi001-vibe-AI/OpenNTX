use crate::output;
use openntx_core::registry::{AppRegistry, RemoveMode};
use openntx_core::{OpenNtxError, Result};

pub fn run(app_id: &str, dry_run: bool, yes: bool) -> Result<()> {
    if dry_run && yes {
        return Err(OpenNtxError::InvalidInput(
            "use either --dry-run or --yes, not both".to_string(),
        ));
    }

    let registry = AppRegistry::from_env()?;
    let mode = if yes {
        RemoveMode::Delete
    } else {
        RemoveMode::DryRun
    };
    let plan = registry.remove_app(app_id, mode)?;

    output::title("OpenNTX Remove Plan");
    output::field("App ID", app_id);
    output::field("App data", plan.app_dir.display());
    output::field("Manifest", plan.manifest_path.display());
    output::field("Desktop entry", plan.desktop_entry_path.display());
    output::field(
        "Mode",
        if yes {
            "delete confirmed"
        } else if dry_run {
            "dry-run"
        } else {
            "dry-run (default)"
        },
    );
    output::field(
        "Status",
        if plan.removed {
            "removed"
        } else {
            "planned / not removed"
        },
    );
    output::blank();
    if plan.removed {
        output::note("App registry entry was removed. Runtime execution was not involved.");
    } else {
        output::note(
            "Use --yes to remove the registered app directory. Runtime execution is not involved.",
        );
    }
    Ok(())
}
