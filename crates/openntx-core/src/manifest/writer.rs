use crate::manifest::model::AppManifest;
use crate::manifest::validate::validate_manifest;
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::Path;

/// Read a JSON file safely: reject symlinks, non-regular files, and paths
/// outside the expected location. Does not follow symlinks.
pub fn read_json_safe<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let meta = fs::symlink_metadata(path).map_err(|source| OpenNtxError::io(path, source))?;

    if meta.file_type().is_symlink() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe file: {} must be a regular file, not a symlink",
            path.display()
        )));
    }

    if !meta.is_file() {
        return Err(OpenNtxError::InvalidInput(format!(
            "unsafe file: {} must be a regular file",
            path.display()
        )));
    }

    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let value: T = serde_json::from_slice(&bytes)?;
    Ok(value)
}

pub fn read_manifest(path: impl AsRef<Path>) -> Result<AppManifest> {
    let path = path.as_ref();
    let manifest: AppManifest = read_json_safe(path)?;
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
