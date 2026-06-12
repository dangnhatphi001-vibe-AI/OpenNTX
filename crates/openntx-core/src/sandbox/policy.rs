use crate::manifest::SandboxConfig;
use crate::sandbox::permissions::PermissionDecision;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub profile: String,
    pub network: PermissionDecision,
    pub home: PermissionDecision,
    pub documents: PermissionDecision,
    pub downloads: PermissionDecision,
    pub removable_drives: PermissionDecision,
}

impl SandboxPolicy {
    pub fn standard() -> Self {
        Self {
            profile: "standard".to_string(),
            network: PermissionDecision::Ask,
            home: PermissionDecision::Deny,
            documents: PermissionDecision::Ask,
            downloads: PermissionDecision::Ask,
            removable_drives: PermissionDecision::Deny,
        }
    }
}

impl From<&SandboxPolicy> for SandboxConfig {
    fn from(value: &SandboxPolicy) -> Self {
        Self {
            profile: value.profile.clone(),
            network: value.network.as_str().to_string(),
            home: value.home.as_str().to_string(),
            documents: value.documents.as_str().to_string(),
            downloads: value.downloads.as_str().to_string(),
            removable_drives: value.removable_drives.as_str().to_string(),
        }
    }
}
