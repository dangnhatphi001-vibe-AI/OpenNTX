use crate::output;
use clap::Subcommand;
use openntx_core::config::OpenNtxFileConfig;
use openntx_core::Result;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Initialize config file with defaults
    Init,
    /// Set a configuration key
    Set {
        #[arg(value_name = "key")]
        key: String,
        #[arg(value_name = "value")]
        value: String,
    },
    /// Reset configuration to defaults
    Reset {
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => show(),
        ConfigCommands::Init => init(),
        ConfigCommands::Set { key, value } => set(&key, &value),
        ConfigCommands::Reset { yes } => reset(yes),
    }
}

fn show() -> Result<()> {
    let config = OpenNtxFileConfig::load()?;
    let path = OpenNtxFileConfig::config_path()?;

    output::title("OpenNTX Configuration");
    output::field("Config file", path.display());
    output::blank();
    output::field("default_output_dir", &config.default_output_dir);
    output::field("default_sandbox_profile", &config.default_sandbox_profile);
    output::field("enable_notifications", config.enable_notifications);
    output::field("log_retention_days", config.log_retention_days);
    output::field("package_version_default", &config.package_version_default);
    output::field("appportal_show_advanced", config.appportal_show_advanced);
    Ok(())
}

fn init() -> Result<()> {
    let path = OpenNtxFileConfig::init()?;
    output::title("OpenNTX Config Init");
    output::field("Config file created", path.display());
    output::blank();
    output::note("Configuration initialized with default values.");
    Ok(())
}

fn set(key: &str, value: &str) -> Result<()> {
    let mut config = OpenNtxFileConfig::load()?;
    let old_value = config.get_value(key)?;
    config.set_key(key, value)?;
    config.save()?;

    output::title("OpenNTX Config Set");
    output::field("Key", key);
    output::field("Old value", &old_value);
    output::field("New value", value);
    Ok(())
}

fn reset(yes: bool) -> Result<()> {
    if !yes {
        output::title("OpenNTX Config Reset");
        output::note("This will reset all configuration to defaults. Pass --yes to confirm.");
        return Ok(());
    }

    OpenNtxFileConfig::reset()?;
    output::title("OpenNTX Config Reset");
    output::note("Configuration reset to defaults.");
    Ok(())
}
