use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturePlan {
    pub capture_id: String,
    pub app_id: String,
    pub input_installer: PathBuf,
    pub temp_root: PathBuf,
    pub sandbox_profile: String,
    pub dry_run: bool,
}

impl CapturePlan {
    pub fn dry_run(
        capture_id: impl Into<String>,
        app_id: impl Into<String>,
        input_installer: impl Into<PathBuf>,
        temp_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            app_id: app_id.into(),
            input_installer: input_installer.into(),
            temp_root: temp_root.into(),
            sandbox_profile: "standard".to_string(),
            dry_run: true,
        }
    }
}
