use crate::{OpenNtxError, Result};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenNtxPaths {
    pub data_root: PathBuf,
    pub apps_root: PathBuf,
    pub logs_root: PathBuf,
    pub cache_root: PathBuf,
    pub desktop_entries_dir: PathBuf,
    pub icons_root: PathBuf,
    pub system_runtime: PathBuf,
}

impl OpenNtxPaths {
    pub fn from_env() -> Result<Self> {
        let home = home_dir()?;
        let data_home = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        let state_home = env_path("XDG_STATE_HOME").unwrap_or_else(|| home.join(".local/state"));
        let cache_home = env_path("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));

        let data_root = data_home.join("openntx");
        Ok(Self {
            apps_root: data_root.join("apps"),
            data_root,
            logs_root: state_home.join("openntx/logs"),
            cache_root: cache_home.join("openntx"),
            desktop_entries_dir: data_home.join("applications"),
            icons_root: data_home.join("icons/hicolor"),
            system_runtime: PathBuf::from("/usr/lib/openntx"),
        })
    }

    pub fn app_dir(&self, app_id: &str) -> PathBuf {
        self.apps_root.join(app_id)
    }

    pub fn manifest_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join("manifest.json")
    }

    pub fn drive_c_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join("drive_c")
    }

    pub fn registry_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join("registry")
    }

    pub fn desktop_entry_path(&self, app_id: &str) -> PathBuf {
        self.desktop_entries_dir
            .join(format!("openntx-{app_id}.desktop"))
    }

    pub fn cache_for_app(&self, app_id: &str) -> PathBuf {
        self.cache_root.join(app_id)
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| OpenNtxError::InvalidInput("HOME is not set".to_string()))
}
