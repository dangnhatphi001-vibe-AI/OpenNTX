pub mod analyze;
pub mod doctor;
pub mod install;
pub mod package;
pub mod remove;
pub mod run;

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
    },
    Package {
        #[arg(value_name = "app-id")]
        app_id: String,
    },
    Remove {
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
        Commands::Install { file } => install::run(&file),
        Commands::Package { app_id } => package::run(&app_id),
        Commands::Remove { app_id } => remove::run(&app_id),
        Commands::Doctor { target } => doctor::run(&target),
    }
}
