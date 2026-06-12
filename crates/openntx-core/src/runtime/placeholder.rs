use crate::manifest::AppManifest;
use crate::runtime::{RuntimeBackend, RuntimeExecutionPlan};
use crate::{OpenNtxError, Result};

#[derive(Debug, Clone, Copy)]
pub struct NotImplementedBackend;

#[derive(Debug, Clone, Copy)]
pub struct ExternalCompatibilityBackend;

#[derive(Debug, Clone, Copy)]
pub struct FutureNativeBackend;

impl RuntimeBackend for NotImplementedBackend {
    fn name(&self) -> &'static str {
        "not-implemented"
    }

    fn plan_execution(&self, manifest: &AppManifest) -> RuntimeExecutionPlan {
        RuntimeExecutionPlan {
            backend: self.name().to_string(),
            app_id: manifest.app_id.clone(),
            executable: manifest.executable.path.clone(),
            implemented: false,
            message: "Runtime execution is not implemented in V0.6. This command currently validates input and prepares a future execution plan.".to_string(),
        }
    }

    fn execute(&self, _manifest: &AppManifest) -> Result<()> {
        Err(OpenNtxError::NotImplemented(
            "runtime execution backend".to_string(),
        ))
    }
}

impl RuntimeBackend for ExternalCompatibilityBackend {
    fn name(&self) -> &'static str {
        "external-compatibility-placeholder"
    }

    fn plan_execution(&self, manifest: &AppManifest) -> RuntimeExecutionPlan {
        RuntimeExecutionPlan {
            backend: self.name().to_string(),
            app_id: manifest.app_id.clone(),
            executable: manifest.executable.path.clone(),
            implemented: false,
            message: "External compatibility backend is a future integration placeholder."
                .to_string(),
        }
    }

    fn execute(&self, _manifest: &AppManifest) -> Result<()> {
        Err(OpenNtxError::NotImplemented(
            "external compatibility backend".to_string(),
        ))
    }
}

impl RuntimeBackend for FutureNativeBackend {
    fn name(&self) -> &'static str {
        "future-native"
    }

    fn plan_execution(&self, manifest: &AppManifest) -> RuntimeExecutionPlan {
        RuntimeExecutionPlan {
            backend: self.name().to_string(),
            app_id: manifest.app_id.clone(),
            executable: manifest.executable.path.clone(),
            implemented: false,
            message:
                "Future native PE/NT/Win32 backend is a research module, not V0.6 functionality."
                    .to_string(),
        }
    }

    fn execute(&self, _manifest: &AppManifest) -> Result<()> {
        Err(OpenNtxError::NotImplemented(
            "future native PE/NT/Win32 backend".to_string(),
        ))
    }
}
