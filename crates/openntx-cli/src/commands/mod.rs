pub mod analyze;
pub mod capture;
pub mod completions;
pub mod config_cmd;
pub mod desktop;
pub mod doctor;
pub mod duplicate;
pub mod export_cmd;
pub mod import_cmd;
pub mod install;
pub mod list;
pub mod logs_cmd;
pub mod manifest;
pub mod package;
pub mod remove;
pub mod rename;
pub mod run;
pub mod show;
pub mod system_package;

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
        #[arg(long)]
        json: bool,
        #[arg(long)]
        log: bool,
        #[arg(long)]
        no_log: bool,
        #[arg(long)]
        notify: bool,
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
    Capture {
        #[command(subcommand)]
        command: capture::CaptureCommands,
    },
    Package {
        #[command(subcommand)]
        command: package::PackageCommands,
    },
    Remove {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Show {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(value_name = "app-id-or-file")]
        target: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        repair: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Rename an app's display name
    Rename {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(value_name = "new-name")]
        new_name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Duplicate a registered app under a new app-id
    Duplicate {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long, value_name = "new-app-id")]
        r#as: String,
        #[arg(long)]
        yes: bool,
    },
    /// Export an app as a portable bundle
    Export {
        #[arg(value_name = "app-id")]
        app_id: String,
        #[arg(long, value_name = "path")]
        output: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Import an OpenNTX bundle
    Import {
        #[arg(value_name = "bundle-path")]
        bundle_path: PathBuf,
        #[arg(long, value_name = "new-app-id")]
        r#as: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Manage run-plan logs
    Logs {
        #[command(subcommand)]
        command: logs_cmd::LogsCommands,
    },
    /// Manage OpenNTX configuration
    Config {
        #[command(subcommand)]
        command: config_cmd::ConfigCommands,
    },
    /// Build the complete OpenNTX system .deb package
    SystemPackage {
        #[command(subcommand)]
        command: system_package::SystemPackageCommands,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_name = "shell")]
        shell: String,
    },
}

pub fn execute(command: Commands) -> Result<()> {
    match command {
        Commands::Analyze { file } => analyze::run(&file),
        Commands::Run {
            target,
            json,
            log,
            no_log,
            notify,
        } => run::run(&target, json, log, no_log, notify),
        Commands::Install {
            file,
            write_plan,
            desktop,
        } => install::run(&file, write_plan, desktop),
        Commands::Desktop { command } => desktop::execute(command),
        Commands::Manifest { command } => manifest::execute(command),
        Commands::Capture { command } => capture::execute(command),
        Commands::Package { command } => package::execute(command),
        Commands::Remove {
            app_id,
            dry_run,
            yes,
        } => remove::run(&app_id, dry_run, yes),
        Commands::List { json } => list::run(json),
        Commands::Show { app_id, json } => show::run(&app_id, json),
        Commands::Doctor {
            target,
            json,
            repair,
            yes,
        } => doctor::run(target.as_deref(), json, repair, yes),
        Commands::Rename {
            app_id,
            new_name,
            yes,
        } => rename::run(&app_id, &new_name, yes),
        Commands::Duplicate { app_id, r#as, yes } => duplicate::run(&app_id, &r#as, yes),
        Commands::Export {
            app_id,
            output,
            yes,
        } => export_cmd::run(&app_id, &output, yes),
        Commands::Import {
            bundle_path,
            r#as,
            yes,
        } => import_cmd::run(&bundle_path, r#as.as_deref(), yes),
        Commands::Logs { command } => logs_cmd::execute(command),
        Commands::Config { command } => config_cmd::execute(command),
        Commands::SystemPackage { command } => system_package::execute(command),
        Commands::Completions { shell } => completions::run(&shell),
    }
}
