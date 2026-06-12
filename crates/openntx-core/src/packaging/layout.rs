use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebPackageLayout {
    pub app_id: String,
    pub app_root: PathBuf,
    pub manifest_path: PathBuf,
    pub drive_c_path: PathBuf,
    pub registry_path: PathBuf,
    pub metadata_path: PathBuf,
    pub desktop_entry_path: PathBuf,
    pub icon_path: PathBuf,
    pub runtime_dependency: String,
}

impl DebPackageLayout {
    pub fn new(app_id: impl Into<String>) -> Self {
        let app_id = app_id.into();
        let app_root = PathBuf::from(format!("/opt/openntx/apps/{app_id}"));
        Self {
            manifest_path: app_root.join("manifest.json"),
            drive_c_path: app_root.join("drive_c"),
            registry_path: app_root.join("registry"),
            metadata_path: app_root.join("metadata"),
            desktop_entry_path: PathBuf::from(format!(
                "/usr/share/applications/openntx-{app_id}.desktop"
            )),
            icon_path: PathBuf::from(format!(
                "/usr/share/icons/hicolor/256x256/apps/{app_id}.png"
            )),
            runtime_dependency: "openntx-runtime >= 0".to_string(),
            app_root,
            app_id,
        }
    }
}
