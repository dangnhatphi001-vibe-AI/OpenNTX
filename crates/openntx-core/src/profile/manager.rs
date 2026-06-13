// profile/manager.rs — CRUD operations for Compatibility Profiles.
//
// Profiles are stored as pretty-printed JSON files at:
//   `~/.local/share/openntx/profiles/<app_id>.json`
//
// The directory is created automatically on first use.

use crate::profile::schema::CompatProfile;
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::PathBuf;

/// Manages on-disk storage of `CompatProfile` JSON files.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::profile::ProfileManager;
///
/// let mgr = ProfileManager::new().expect("init profile manager");
/// let ids = mgr.list_profiles().expect("list");
/// println!("registered profiles: {ids:?}");
/// ```
pub struct ProfileManager {
    /// Root directory that holds `<app_id>.json` files.
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// Initialise the manager, creating the profiles directory if it does not
    /// exist.
    ///
    /// Uses [`dirs::data_local_dir`] (returns `~/.local/share` on Linux) and
    /// appends `openntx/profiles`.
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir().ok_or_else(|| {
            OpenNtxError::Config(
                "unable to determine local data directory (dirs::data_local_dir returned None)"
                    .into(),
            )
        })?;
        let profiles_dir = data_dir.join("openntx").join("profiles");
        Self::with_path(profiles_dir)
    }

    /// Initialise the manager with an explicit directory path.
    ///
    /// The directory (and all ancestors) is created if it does not exist.
    /// Useful for testing or custom installations.
    pub fn with_path(profiles_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&profiles_dir)
            .map_err(|source| OpenNtxError::io(&profiles_dir, source))?;
        Ok(Self { profiles_dir })
    }

    /// Return the directory that stores profile JSON files.
    pub fn profiles_dir(&self) -> &PathBuf {
        &self.profiles_dir
    }

    // ── CRUD ─────────────────────────────────────────────────────────────────

    /// Persist a `CompatProfile` to `<app_id>.json`.
    ///
    /// Overwrites the file if it already exists.  Validates that `app_id`
    /// is non-empty before writing.
    pub fn save_profile(&self, profile: &CompatProfile) -> Result<()> {
        validate_app_id(&profile.app_id)?;

        let path = self.profile_path(&profile.app_id);
        let json = serde_json::to_vec_pretty(profile)?;
        fs::write(&path, json).map_err(|source| OpenNtxError::io(&path, source))?;

        Ok(())
    }

    /// Load a `CompatProfile` from `<app_id>.json`.
    ///
    /// Returns `AppNotFound` if the file does not exist.
    pub fn load_profile(&self, app_id: &str) -> Result<CompatProfile> {
        validate_app_id(app_id)?;

        let path = self.profile_path(app_id);
        if !path.exists() {
            return Err(OpenNtxError::AppNotFound(app_id.to_string()));
        }

        let bytes = fs::read(&path).map_err(|source| OpenNtxError::io(&path, source))?;
        let profile: CompatProfile = serde_json::from_slice(&bytes)?;
        Ok(profile)
    }

    /// List all `app_id`s that have a profile file on disk.
    ///
    /// Returns a sorted `Vec<String>`.  Non-`.json` files and entries whose
    /// file-stem cannot be decoded as UTF-8 are silently skipped.
    pub fn list_profiles(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();

        // If the directory does not exist yet (e.g. no profiles saved), return
        // an empty list instead of an error.
        if !self.profiles_dir.exists() {
            return Ok(ids);
        }

        let entries = fs::read_dir(&self.profiles_dir)
            .map_err(|source| OpenNtxError::io(&self.profiles_dir, source))?;

        for entry in entries {
            let entry = entry.map_err(|source| OpenNtxError::io(&self.profiles_dir, source))?;
            let path = entry.path();

            // Only consider regular `.json` files.
            if !path.is_file() {
                continue;
            }
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {}
                _ => continue,
            }

            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_string());
            }
        }

        ids.sort();
        Ok(ids)
    }

    /// Delete a profile file.  Returns `AppNotFound` if the file does not exist.
    pub fn delete_profile(&self, app_id: &str) -> Result<()> {
        validate_app_id(app_id)?;

        let path = self.profile_path(app_id);
        if !path.exists() {
            return Err(OpenNtxError::AppNotFound(app_id.to_string()));
        }

        fs::remove_file(&path).map_err(|source| OpenNtxError::io(&path, source))?;
        Ok(())
    }

    /// Check whether a profile file exists for the given `app_id`.
    pub fn has_profile(&self, app_id: &str) -> bool {
        validate_app_id(app_id).is_ok() && self.profile_path(app_id).exists()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Resolve the JSON file path for a given `app_id`.
    fn profile_path(&self, app_id: &str) -> PathBuf {
        self.profiles_dir.join(format!("{app_id}.json"))
    }
}

/// Reject empty or whitespace-only `app_id` values.
fn validate_app_id(app_id: &str) -> Result<()> {
    if app_id.trim().is_empty() {
        return Err(OpenNtxError::InvalidInput(
            "app_id must not be empty or whitespace-only".into(),
        ));
    }
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::schema::*;
    use std::collections::HashMap;

    /// Build a `ProfileManager` backed by a temporary directory.
    fn temp_manager() -> (ProfileManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let mgr = ProfileManager::with_path(dir.path().to_path_buf()).expect("with_path");
        (mgr, dir)
    }

    fn sample_profile(app_id: &str) -> CompatProfile {
        CompatProfile {
            app_id: app_id.to_string(),
            metadata: AppMetadata {
                name: "Test App".to_string(),
                version: "1.0.0".to_string(),
                publisher: "Test Publisher".to_string(),
                arch: Arch::X86_64,
            },
            runtime_reqs: RuntimeReqs {
                dependencies: vec!["vcrun2019".to_string()],
                directx: None,
            },
            fs_rules: FilesystemRules {
                required_paths: vec!["C:/Test".to_string()],
                path_mappings: HashMap::new(),
            },
            reg_rules: RegistryRules {
                required_keys: vec![],
            },
            installer: InstallerBehavior {
                installer_type: "nsis".to_string(),
                silent_args: vec!["/S".to_string()],
            },
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let (mgr, _tmp) = temp_manager();
        let profile = sample_profile("test-app-v1");

        mgr.save_profile(&profile).expect("save");
        let loaded = mgr.load_profile("test-app-v1").expect("load");
        assert_eq!(profile, loaded);
    }

    #[test]
    fn list_profiles_empty() {
        let (mgr, _tmp) = temp_manager();
        assert!(mgr.list_profiles().expect("list").is_empty());
    }

    #[test]
    fn list_profiles_after_save() {
        let (mgr, _tmp) = temp_manager();
        mgr.save_profile(&sample_profile("alpha")).expect("save a");
        mgr.save_profile(&sample_profile("beta")).expect("save b");
        mgr.save_profile(&sample_profile("gamma")).expect("save g");

        let mut ids = mgr.list_profiles().expect("list");
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn load_nonexistent_returns_not_found() {
        let (mgr, _tmp) = temp_manager();
        let err = mgr.load_profile("nope").unwrap_err();
        assert!(matches!(err, OpenNtxError::AppNotFound(_)));
    }

    #[test]
    fn delete_profile() {
        let (mgr, _tmp) = temp_manager();
        mgr.save_profile(&sample_profile("del-me")).expect("save");
        assert!(mgr.has_profile("del-me"));

        mgr.delete_profile("del-me").expect("delete");
        assert!(!mgr.has_profile("del-me"));
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let (mgr, _tmp) = temp_manager();
        let err = mgr.delete_profile("ghost").unwrap_err();
        assert!(matches!(err, OpenNtxError::AppNotFound(_)));
    }

    #[test]
    fn has_profile_false_before_save() {
        let (mgr, _tmp) = temp_manager();
        assert!(!mgr.has_profile("missing"));
    }

    #[test]
    fn save_overwrites_existing() {
        let (mgr, _tmp) = temp_manager();
        let mut profile = sample_profile("overwrite-me");
        mgr.save_profile(&profile).expect("first save");

        profile.metadata.version = "2.0.0".to_string();
        mgr.save_profile(&profile).expect("second save");

        let loaded = mgr.load_profile("overwrite-me").expect("load");
        assert_eq!(loaded.metadata.version, "2.0.0");
    }

    #[test]
    fn reject_empty_app_id() {
        let (mgr, _tmp) = temp_manager();
        let profile = sample_profile("");
        let err = mgr.save_profile(&profile).unwrap_err();
        assert!(matches!(err, OpenNtxError::InvalidInput(_)));
    }

    #[test]
    fn non_json_files_ignored_in_list() {
        let (mgr, tmp) = temp_manager();
        // Create a non-json file in the profiles directory.
        let junk = tmp.path().join("readme.txt");
        fs::write(&junk, "not a profile").expect("write junk");

        mgr.save_profile(&sample_profile("real")).expect("save");
        let ids = mgr.list_profiles().expect("list");
        assert_eq!(ids, vec!["real"]);
    }
}
