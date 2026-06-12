use crate::manifest::model::AppManifest;
use crate::manifest::validate::validate_manifest;
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::Path;

pub fn read_manifest(path: impl AsRef<Path>) -> Result<AppManifest> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let manifest: AppManifest = serde_json::from_slice(&bytes)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn write_manifest_pretty(path: impl AsRef<Path>, manifest: &AppManifest) -> Result<()> {
    validate_manifest(manifest)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| OpenNtxError::io(parent, source))?;
    }
    let json = serde_json::to_vec_pretty(manifest)?;
    fs::write(path, json).map_err(|source| OpenNtxError::io(path, source))
}
