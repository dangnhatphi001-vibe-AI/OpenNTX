pub mod analyze;
pub mod desktop;
pub mod doctor;
pub mod install;
pub mod list;
pub mod manifest;
pub mod package;
pub mod remove;
pub mod run;
pub mod show;

use clap::Subcommand;
use openntx_core::Result;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Commands {
    Analyze {
        #[arg(value_name = "file.exe")]
        file: PathBuf,
    },
    Run {
        #[arg(value_name = "app-or-file")]
        target: String,
    },
    Install {
        #[arg(value_name = "file.exe")]
        file: PathBuf,
        #[arg(long)]
        write_plan: bool,
        #[arg(long)]
        desktop: bool,
    },
    Desktop {
        #[command(subcommand)]
        command: desktop::DesktopCommands,
    },
    Manifest {
        #[command(subcommand)]
        command: manifest::ManifestCommands,
    },
    Package {
        #[arg(value_name = "app-id")]
        app_id: String,
    },
    Remove {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    List,
    Show {
        #[arg(value_name = "app-id")]
        app_id: String,
    },
    Doctor {
        #[arg(value_name = "app-id-or-file")]
        target: String,
    },
}

pub fn execute(command: Commands) -> Result<()> {
    match command {
        Commands::Analyze { file } => analyze::run(&file),
        Commands::Run { target } => run::run(&target),
        Commands::Install {
            file,
            write_plan,
            desktop,
        } => install::run(&file, write_plan, desktop),
        Commands::Desktop { command } => desktop::execute(command),
        Commands::Manifest { command } => manifest::execute(command),
        Commands::Package { app_id } => package::run(&app_id),
        Commands::Remove {
            app_id,
            dry_run,
            yes,
        } => remove::run(&app_id, dry_run, yes),
        Commands::List => list::run(),
        Commands::Show { app_id } => show::run(&app_id),
        Commands::Doctor { target } => doctor::run(&target),
    }
}
