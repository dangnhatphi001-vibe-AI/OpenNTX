use crate::output;
use clap::Subcommand;
use openntx_core::registry::{AppRegistry, DesktopMode};
use openntx_core::{OpenNtxError, Result};

#[derive(Debug, Subcommand)]
pub enum DesktopCommands {
    Create {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    Remove {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(command: DesktopCommands) -> Result<()> {
    match command {
        DesktopCommands::Create {
            app_id,
            dry_run,
            yes,
        } => create(&app_id, dry_run, yes),
        DesktopCommands::Remove {
            app_id,
            dry_run,
            yes,
        } => remove(&app_id, dry_run, yes),
    }
}

fn create(app_id: &str, dry_run: bool, yes: bool) -> Result<()> {
    if dry_run && yes {
        return Err(OpenNtxError::InvalidInput(
            "use either --dry-run or --yes, not both".to_string(),
        ));
    }
    let registry = AppRegistry::from_env()?;
    let mode = if yes {
        DesktopMode::Write
    } else {
        DesktopMode::DryRun
    };
    let plan = registry.create_desktop_entry(app_id, mode, "openntx")?;

    output::title("OpenNTX Desktop Create");
    output::field("App ID", app_id);
    output::field("Desktop entry", plan.desktop_entry_path.display());
    output::field(
        "Mode",
        if yes {
            "write confirmed"
        } else if dry_run {
            "dry-run"
        } else {
            "dry-run (default)"
        },
    );
    output::field(
        "Status",
        if plan.written {
            "written"
        } else {
            "planned / not written"
        },
    );
    if !plan.written {
        output::blank();
        output::note(
            "Use --yes to write the .desktop launcher. Exec will call `openntx run <app-id>`.",
        );
    }
    Ok(())
}

fn remove(app_id: &str, dry_run: bool, yes: bool) -> Result<()> {
    if dry_run && yes {
        return Err(OpenNtxError::InvalidInput(
            "use either --dry-run or --yes, not both".to_string(),
        ));
    }
    let registry = AppRegistry::from_env()?;
    let mode = if yes {
        DesktopMode::Write
    } else {
        DesktopMode::DryRun
    };
    let plan = registry.remove_desktop_entry(app_id, mode)?;

    output::title("OpenNTX Desktop Remove");
    output::field("App ID", app_id);
    output::field("Desktop entry", plan.desktop_entry_path.display());
    output::field(
        "Mode",
        if yes {
            "remove confirmed"
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
    if !plan.removed {
        output::blank();
        output::note("Use --yes to remove the .desktop launcher.");
    }
    Ok(())
}
