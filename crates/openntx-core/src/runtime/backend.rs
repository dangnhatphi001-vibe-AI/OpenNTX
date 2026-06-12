use crate::manifest::AppManifest;
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutionPlan {
    pub backend: String,
    pub app_id: String,
    pub executable: String,
    pub implemented: bool,
    pub message: String,
}

pub trait RuntimeBackend {
    fn name(&self) -> &'static str;
    fn plan_execution(&self, manifest: &AppManifest) -> RuntimeExecutionPlan;
    fn execute(&self, manifest: &AppManifest) -> Result<()>;
}
