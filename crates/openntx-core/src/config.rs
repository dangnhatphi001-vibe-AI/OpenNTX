use crate::paths::OpenNtxPaths;
use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persistent configuration for OpenNTX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenNtxFileConfig {
    pub default_output_dir: String,
    pub default_sandbox_profile: String,
    pub enable_notifications: bool,
    pub log_retention_days: u64,
    pub package_version_default: String,
    pub appportal_show_advanced: bool,
}

impl Default for OpenNtxFileConfig {
    fn default() -> Self {
        Self {
            default_output_dir: "dist".to_string(),
            default_sandbox_profile: "standard".to_string(),
            enable_notifications: true,
            log_retention_days: 30,
            package_version_default: "1.0.0-alpha".to_string(),
            appportal_show_advanced: false,
        }
    }
}

/// List of valid configuration keys.
const VALID_KEYS: &[&str] = &[
    "default_output_dir",
    "default_sandbox_profile",
    "enable_notifications",
    "log_retention_days",
    "package_version_default",
    "appportal_show_advanced",
];

impl OpenNtxFileConfig {
    /// Get the path to the config file.
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| OpenNtxError::InvalidInput("HOME is not set".to_string()))?;
        Ok(home.join(".config/openntx/config.json"))
    }

    /// Load config from file, or return defaults if file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let bytes = fs::read(&path).map_err(|source| OpenNtxError::io(&path, source))?;
        let config: Self = serde_json::from_slice(&bytes)?;
        Ok(config)
    }

    /// Save config to file.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        fs::write(&path, json).map_err(|source| OpenNtxError::io(&path, source))
    }

    /// Initialize config file with defaults (fail if already exists).
    pub fn init() -> Result<PathBuf> {
        let path = Self::config_path()?;
        if path.exists() {
            return Err(OpenNtxError::AlreadyExists(format!(
                "config file already exists: {}",
                path.display()
            )));
        }
        let config = Self::default();
        config.save()?;
        Ok(path)
    }

    /// Reset config to defaults (overwrite existing).
    pub fn reset() -> Result<()> {
        let config = Self::default();
        config.save()
    }

    /// Set a config key to a value.
    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        if !VALID_KEYS.contains(&key) {
            return Err(OpenNtxError::Config(format!(
                "unknown config key: {key}. Valid keys: {}",
                VALID_KEYS.join(", ")
            )));
        }

        match key {
            "default_output_dir" => self.default_output_dir = value.to_string(),
            "default_sandbox_profile" => {
                if !["strict", "standard", "developer"].contains(&value) {
                    return Err(OpenNtxError::Config(format!(
                        "invalid sandbox profile: {value}. Valid: strict, standard, developer"
                    )));
                }
                self.default_sandbox_profile = value.to_string();
            }
            "enable_notifications" => {
                self.enable_notifications = parse_bool(key, value)?;
            }
            "log_retention_days" => {
                self.log_retention_days = value.parse().map_err(|_| {
                    OpenNtxError::Config(format!("invalid number for {key}: {value}"))
                })?;
            }
            "package_version_default" => self.package_version_default = value.to_string(),
            "appportal_show_advanced" => {
                self.appportal_show_advanced = parse_bool(key, value)?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    /// Get a config value by key.
    pub fn get_value(&self, key: &str) -> Result<String> {
        match key {
            "default_output_dir" => Ok(self.default_output_dir.clone()),
            "default_sandbox_profile" => Ok(self.default_sandbox_profile.clone()),
            "enable_notifications" => Ok(self.enable_notifications.to_string()),
            "log_retention_days" => Ok(self.log_retention_days.to_string()),
            "package_version_default" => Ok(self.package_version_default.clone()),
            "appportal_show_advanced" => Ok(self.appportal_show_advanced.to_string()),
            _ => Err(OpenNtxError::Config(format!(
                "unknown config key: {key}. Valid keys: {}",
                VALID_KEYS.join(", ")
            ))),
        }
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(OpenNtxError::Config(format!(
            "invalid boolean for {key}: {value}. Use true/false"
        ))),
    }
}

/// Runtime config combining paths and file config.
#[derive(Debug, Clone)]
pub struct OpenNtxConfig {
    pub paths: OpenNtxPaths,
    pub file_config: OpenNtxFileConfig,
    pub default_sandbox_profile: String,
    pub default_runtime_backend: String,
}

impl OpenNtxConfig {
    pub fn from_env() -> Result<Self> {
        let file_config = OpenNtxFileConfig::load().unwrap_or_default();
        let default_sandbox_profile = file_config.default_sandbox_profile.clone();
        Ok(Self {
            paths: OpenNtxPaths::from_env()?,
            file_config,
            default_sandbox_profile,
            default_runtime_backend: "not-implemented".to_string(),
        })
    }
}
