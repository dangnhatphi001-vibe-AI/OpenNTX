use crate::paths::OpenNtxPaths;
use crate::Result;

#[derive(Debug, Clone)]
pub struct OpenNtxConfig {
    pub paths: OpenNtxPaths,
    pub default_sandbox_profile: String,
    pub default_runtime_backend: String,
}

impl OpenNtxConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            paths: OpenNtxPaths::from_env()?,
            default_sandbox_profile: "standard".to_string(),
            default_runtime_backend: "not-implemented".to_string(),
        })
    }
}
