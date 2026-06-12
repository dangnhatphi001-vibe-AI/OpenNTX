mod commands;
mod output;

use clap::Parser;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "openntx")]
#[command(version)]
#[command(about = "OpenNTX V0.1 CLI foundation")]
struct Cli {
    #[command(subcommand)]
    command: commands::Commands,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match commands::execute(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("OpenNTX error: {error}");
            ExitCode::from(1)
        }
    }
}
