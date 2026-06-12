use crate::app_id::is_valid_app_id;
use crate::manifest::model::AppManifest;
use crate::{OpenNtxError, Result};

pub fn validate_manifest(manifest: &AppManifest) -> Result<()> {
    require_eq("schema_version", &manifest.schema_version, "0.1.0")?;
    require_app_id(&manifest.app_id)?;
    require_non_empty("name", &manifest.name)?;
    require_non_empty("source.original_file", &manifest.source.original_file)?;
    require_one_of(
        "source.type",
        &manifest.source.source_type,
        &["installer", "portable", "unknown"],
    )?;
    require_non_empty("executable.path", &manifest.executable.path)?;
    require_one_of(
        "architecture",
        &manifest.architecture,
        &["x86", "x86_64", "arm64", "unknown"],
    )?;
    require_one_of(
        "install_mode",
        &manifest.install_mode,
        &["portable", "captured", "planned", "run-once"],
    )?;
    require_one_of(
        "sandbox.profile",
        &manifest.sandbox.profile,
        &["strict", "standard", "developer"],
    )?;
    for (field, value) in [
        ("sandbox.network", &manifest.sandbox.network),
        ("sandbox.home", &manifest.sandbox.home),
        ("sandbox.documents", &manifest.sandbox.documents),
        ("sandbox.downloads", &manifest.sandbox.downloads),
        (
            "sandbox.removable_drives",
            &manifest.sandbox.removable_drives,
        ),
    ] {
        require_one_of(field, value, &["deny", "ask", "allow"])?;
    }
    require_non_empty("desktop.name", &manifest.desktop.name)?;
    require_non_empty("desktop.icon", &manifest.desktop.icon)?;
    Ok(())
}

fn require_eq(field: &str, value: &str, expected: &str) -> Result<()> {
    if value == expected {
        Ok(())
    } else {
        Err(OpenNtxError::ManifestValidation(format!(
            "{field} must be {expected}"
        )))
    }
}

fn require_app_id(value: &str) -> Result<()> {
    if is_valid_app_id(value) {
        Ok(())
    } else {
        Err(OpenNtxError::ManifestValidation(
            "app_id must be lowercase ASCII with digits and hyphens".to_string(),
        ))
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(OpenNtxError::ManifestValidation(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn require_one_of(field: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(OpenNtxError::ManifestValidation(format!(
            "{field} has unsupported value {value:?}"
        )))
    }
}
