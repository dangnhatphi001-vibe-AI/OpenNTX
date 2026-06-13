// openntx-core/src/runtime/registry.rs — Isolated Windows Registry emulation.
//
// Provides `VirtualRegistry` which emulates the Windows Registry hive structure
// using a flat TOML file stored inside each application's sandbox directory.
//
// File layout:
//   <sandbox_root>/registry.toml
//
// TOML structure mirrors the Windows hive/key/value hierarchy:
//
//   [HKLM]
//   [HKLM.Software.Microsoft]
//   Version = "10.0.19041"
//   ProductName = "Windows 10"
//
//   [HKCU]
//   [HKCU.Software.MyApp]
//   WindowWidth = "1024"
//   WindowHeight = "768"
//
// The hive name (HKLM, HKCU, HKU, etc.) is the top-level table.
// Subkeys are nested TOML tables separated by dots.
// Values are string key-value pairs within the innermost table.

use crate::{OpenNtxError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default registry filename inside the sandbox.
const REGISTRY_FILENAME: &str = "registry.toml";

/// Valid Windows hive prefixes.
const VALID_HIVES: &[&str] = &["HKLM", "HKCU", "HKU", "HKCR", "HKCC"];

/// The entire registry is a nested map: hive -> key_path -> (value_name -> value_data).
type RegistryData = HashMap<String, HashMap<String, HashMap<String, String>>>;

/// Virtual Windows Registry backed by a TOML file in the sandbox.
///
/// Each application gets its own isolated `registry.toml` file, preventing
/// cross-application registry pollution.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::registry::VirtualRegistry;
/// use std::path::Path;
///
/// let mut reg = VirtualRegistry::new(Path::new("/tmp/sandbox/my-app"));
/// reg.set_value("HKLM", "Software\\Microsoft", "Version", "10.0").unwrap();
/// let val = reg.get_value("HKLM", "Software\\Microsoft", "Version").unwrap();
/// assert_eq!(val, Some("10.0".to_string()));
/// ```
pub struct VirtualRegistry {
    /// Path to the `registry.toml` file.
    file_path: PathBuf,
    /// In-memory representation of the registry data.
    data: RegistryData,
}

impl VirtualRegistry {
    /// Create a new `VirtualRegistry` backed by `registry.toml` inside
    /// `sandbox_root`.
    ///
    /// If the file already exists, it is loaded.  Otherwise, an empty
    /// registry is created.
    pub fn new(sandbox_root: &Path) -> Self {
        let file_path = sandbox_root.join(REGISTRY_FILENAME);
        let data = Self::load_from_file(&file_path);

        Self { file_path, data }
    }

    /// Create a new `VirtualRegistry` with an explicit file path (useful
    /// for testing).
    pub fn with_path(file_path: PathBuf) -> Self {
        let data = Self::load_from_file(&file_path);
        Self { file_path, data }
    }

    /// Return the path to the registry file.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Return the number of hives that contain data.
    pub fn hive_count(&self) -> usize {
        self.data.len()
    }

    /// Return the total number of values across all hives and keys.
    pub fn total_value_count(&self) -> usize {
        self.data
            .values()
            .flat_map(|keys| keys.values())
            .map(|vals| vals.len())
            .sum()
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Read a registry value.
    ///
    /// # Parameters
    ///
    /// - `hive` — Registry hive name (e.g. `"HKLM"`, `"HKCU"`).
    /// - `key` — Registry key path using backslash separators
    ///   (e.g. `"Software\\Microsoft"`).
    /// - `value_name` — Value name within the key (e.g. `"Version"`).
    ///
    /// # Returns
    ///
    /// `Ok(Some(data))` if the value exists, `Ok(None)` if not found.
    ///
    /// # Errors
    ///
    /// - `RegistryKeyInvalid` if the hive name is not a valid Windows hive.
    pub fn get_value(
        &self,
        hive: &str,
        key: &str,
        value_name: &str,
    ) -> Result<Option<String>> {
        validate_hive(hive)?;

        let normalized_key = normalize_key(key);

        if let Some(keys) = self.data.get(hive) {
            if let Some(values) = keys.get(&normalized_key) {
                return Ok(values.get(value_name).cloned());
            }
        }

        Ok(None)
    }

    /// Write or update a registry value.
    ///
    /// The data is written to the in-memory representation and immediately
    /// flushed to disk to prevent data loss on process fork/crash.
    ///
    /// # Parameters
    ///
    /// - `hive` — Registry hive name (e.g. `"HKLM"`, `"HKCU"`).
    /// - `key` — Registry key path using backslash separators.
    /// - `value_name` — Value name within the key.
    /// - `value_data` — String data to store.
    ///
    /// # Errors
    ///
    /// - `RegistryKeyInvalid` if the hive name is not a valid Windows hive.
    /// - `RegistryStorageError` if the TOML serialization or file write fails.
    pub fn set_value(
        &mut self,
        hive: &str,
        key: &str,
        value_name: &str,
        value_data: &str,
    ) -> Result<()> {
        validate_hive(hive)?;

        let normalized_key = normalize_key(key);

        self.data
            .entry(hive.to_string())
            .or_default()
            .entry(normalized_key)
            .or_default()
            .insert(value_name.to_string(), value_data.to_string());

        self.flush_to_disk()?;

        Ok(())
    }

    /// Delete a specific value from a key.
    ///
    /// Returns `true` if the value existed and was removed.
    pub fn delete_value(
        &mut self,
        hive: &str,
        key: &str,
        value_name: &str,
    ) -> Result<bool> {
        validate_hive(hive)?;

        let normalized_key = normalize_key(key);

        let existed = if let Some(keys) = self.data.get_mut(hive) {
            if let Some(values) = keys.get_mut(&normalized_key) {
                values.remove(value_name).is_some()
            } else {
                false
            }
        } else {
            false
        };

        if existed {
            self.flush_to_disk()?;
        }

        Ok(existed)
    }

    /// List all value names under a specific key.
    pub fn list_values(
        &self,
        hive: &str,
        key: &str,
    ) -> Result<Vec<String>> {
        validate_hive(hive)?;

        let normalized_key = normalize_key(key);

        if let Some(keys) = self.data.get(hive) {
            if let Some(values) = keys.get(&normalized_key) {
                return Ok(values.keys().cloned().collect());
            }
        }

        Ok(Vec::new())
    }

    /// List all subkeys under a hive/key prefix.
    pub fn list_keys(
        &self,
        hive: &str,
        prefix: &str,
    ) -> Result<Vec<String>> {
        validate_hive(hive)?;

        let normalized_prefix = normalize_key(prefix);

        if let Some(keys) = self.data.get(hive) {
            let matching: Vec<String> = keys
                .keys()
                .filter(|k| {
                    if normalized_prefix.is_empty() {
                        // Top-level: return keys without further nesting.
                        !k.contains('\\')
                    } else {
                        k.starts_with(&normalized_prefix)
                            && k.len() > normalized_prefix.len()
                            && k.as_bytes().get(normalized_prefix.len()) == Some(&b'\\')
                    }
                })
                .cloned()
                .collect();
            return Ok(matching);
        }

        Ok(Vec::new())
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Load registry data from a TOML file.  Returns empty data if the
    /// file does not exist or cannot be parsed.
    fn load_from_file(path: &Path) -> RegistryData {
        if !path.exists() {
            return HashMap::new();
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return HashMap::new(),
        };

        Self::parse_toml(&content)
    }

    /// Parse TOML content into the nested registry data structure.
    fn parse_toml(content: &str) -> RegistryData {
        // Parse as a flat map of dotted keys -> key-value pairs.
        // TOML tables like [HKLM.Software.Microsoft] become a path
        // "HKLM" -> "Software\Microsoft" in our structure.
        let parsed: toml::Table = match toml::from_str(content) {
            Ok(t) => t,
            Err(_) => return HashMap::new(),
        };

        let mut data: RegistryData = HashMap::new();

        Self::walk_toml_table(&parsed, &mut Vec::new(), &mut data);

        data
    }

    /// Recursively walk a TOML table tree and populate the registry data.
    fn walk_toml_table(
        table: &toml::Table,
        path: &mut Vec<String>,
        data: &mut RegistryData,
    ) {
        for (key, value) in table {
            match value {
                toml::Value::Table(sub_table) => {
                    path.push(key.clone());
                    Self::walk_toml_table(sub_table, path, data);
                    path.pop();
                }
                toml::Value::String(s) => {
                    // This is a leaf value.
                    if path.is_empty() {
                        continue;
                    }

                    let hive = &path[0];
                    let key_path = if path.len() > 1 {
                        path[1..].join("\\")
                    } else {
                        String::new()
                    };

                    data.entry(hive.clone())
                        .or_default()
                        .entry(key_path)
                        .or_default()
                        .insert(key.clone(), s.clone());
                }
                _ => {
                    // Non-string values are converted to string representation.
                    if path.is_empty() {
                        continue;
                    }

                    let hive = &path[0];
                    let key_path = if path.len() > 1 {
                        path[1..].join("\\")
                    } else {
                        String::new()
                    };

                    data.entry(hive.clone())
                        .or_default()
                        .entry(key_path)
                        .or_default()
                        .insert(key.clone(), value.to_string());
                }
            }
        }
    }

    /// Serialize the current registry data to TOML and write to disk.
    fn flush_to_disk(&self) -> Result<()> {
        let toml_content = self.serialize_to_toml()?;

        // Ensure parent directory exists.
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                OpenNtxError::RegistryStorageError(format!(
                    "failed to create registry directory {}: {}",
                    parent.display(),
                    source
                ))
            })?;
        }

        fs::write(&self.file_path, toml_content).map_err(|source| {
            OpenNtxError::RegistryStorageError(format!(
                "failed to write registry file {}: {}",
                self.file_path.display(),
                source
            ))
        })
    }

    /// Serialize the in-memory data to a TOML string.
    fn serialize_to_toml(&self) -> Result<String> {
        let mut root = toml::Table::new();

        for (hive, keys) in &self.data {
            let mut hive_table = toml::Table::new();

            for (key_path, values) in keys {
                if key_path.is_empty() {
                    // Values directly under the hive.
                    for (name, data) in values {
                        hive_table.insert(
                            name.clone(),
                            toml::Value::String(data.clone()),
                        );
                    }
                } else {
                    // Build nested table path.
                    let segments: Vec<&str> = key_path.split('\\').collect();
                    insert_nested(&mut hive_table, &segments, values);
                }
            }

            root.insert(hive.clone(), toml::Value::Table(hive_table));
        }

        let output = toml::to_string_pretty(&toml::Value::Table(root))
            .map_err(|e| {
                OpenNtxError::RegistryStorageError(format!(
                    "TOML serialization failed: {}",
                    e
                ))
            })?;

        Ok(output)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Validate that a hive name is one of the recognized Windows hives.
fn validate_hive(hive: &str) -> Result<()> {
    if !VALID_HIVES.contains(&hive) {
        return Err(OpenNtxError::RegistryKeyInvalid(format!(
            "invalid hive '{}': must be one of {:?}",
            hive, VALID_HIVES
        )));
    }
    Ok(())
}

/// Normalize a registry key path: convert forward slashes to backslashes
/// and trim trailing backslashes.
fn normalize_key(key: &str) -> String {
    key.replace('/', "\\")
        .trim_end_matches('\\')
        .to_string()
}

/// Recursively insert values into a nested TOML table structure.
fn insert_nested(
    table: &mut toml::Table,
    segments: &[&str],
    values: &HashMap<String, String>,
) {
    if segments.is_empty() {
        return;
    }

    let segment = segments[0];

    if segments.len() == 1 {
        // Leaf: insert values into this table.
        let entry = table
            .entry(segment.to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));

        if let toml::Value::Table(ref mut t) = entry {
            for (name, data) in values {
                t.insert(name.clone(), toml::Value::String(data.clone()));
            }
        }
    } else {
        // Intermediate: ensure table exists and recurse.
        let entry = table
            .entry(segment.to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));

        if let toml::Value::Table(ref mut t) = entry {
            insert_nested(t, &segments[1..], values);
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    // ── Basic CRUD ───────────────────────────────────────────────────────

    #[test]
    fn write_then_read_basic() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Microsoft", "Version", "10.0.19041")
            .unwrap();

        let val = reg
            .get_value("HKLM", "Software\\Microsoft", "Version")
            .unwrap();
        assert_eq!(val, Some("10.0.19041".to_string()));
    }

    #[test]
    fn write_then_read_multiple_values() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Microsoft", "Version", "10.0")
            .unwrap();
        reg.set_value("HKLM", "Software\\Microsoft", "ProductName", "Windows 10")
            .unwrap();
        reg.set_value("HKLM", "Software\\Microsoft", "Build", "19041")
            .unwrap();

        assert_eq!(
            reg.get_value("HKLM", "Software\\Microsoft", "Version").unwrap(),
            Some("10.0".to_string())
        );
        assert_eq!(
            reg.get_value("HKLM", "Software\\Microsoft", "ProductName").unwrap(),
            Some("Windows 10".to_string())
        );
        assert_eq!(
            reg.get_value("HKLM", "Software\\Microsoft", "Build").unwrap(),
            Some("19041".to_string())
        );
    }

    #[test]
    fn write_then_read_across_hives() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\App", "Global", "yes")
            .unwrap();
        reg.set_value("HKCU", "Software\\App", "User", "no")
            .unwrap();

        assert_eq!(
            reg.get_value("HKLM", "Software\\App", "Global").unwrap(),
            Some("yes".to_string())
        );
        assert_eq!(
            reg.get_value("HKCU", "Software\\App", "User").unwrap(),
            Some("no".to_string())
        );
    }

    #[test]
    fn get_nonexistent_value_returns_none() {
        let dir = temp_dir();
        let reg = VirtualRegistry::new(dir.path());

        let val = reg.get_value("HKLM", "Software\\Missing", "Key").unwrap();
        assert_eq!(val, None);
    }

    #[test]
    fn get_nonexistent_hive_returns_none() {
        let dir = temp_dir();
        let reg = VirtualRegistry::new(dir.path());

        let val = reg.get_value("HKLM", "anything", "value").unwrap();
        assert_eq!(val, None);
    }

    // ── Persistence ──────────────────────────────────────────────────────

    #[test]
    fn data_persists_across_instances() {
        let dir = temp_dir();

        {
            let mut reg = VirtualRegistry::new(dir.path());
            reg.set_value("HKLM", "Software\\Test", "Persist", "yes")
                .unwrap();
        }
        // Drop the first instance.

        {
            let reg = VirtualRegistry::new(dir.path());
            let val = reg
                .get_value("HKLM", "Software\\Test", "Persist")
                .unwrap();
            assert_eq!(val, Some("yes".to_string()));
        }
    }

    #[test]
    fn update_overwrites_existing_value() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Test", "Counter", "1")
            .unwrap();
        reg.set_value("HKLM", "Software\\Test", "Counter", "2")
            .unwrap();

        let val = reg.get_value("HKLM", "Software\\Test", "Counter").unwrap();
        assert_eq!(val, Some("2".to_string()));
    }

    #[test]
    fn toml_file_created_on_write() {
        let dir = temp_dir();
        let file_path = dir.path().join(REGISTRY_FILENAME);
        assert!(!file_path.exists());

        let mut reg = VirtualRegistry::new(dir.path());
        reg.set_value("HKLM", "Software\\Test", "Key", "Value")
            .unwrap();

        assert!(file_path.exists());
    }

    #[test]
    fn toml_file_contains_valid_toml() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Microsoft", "Version", "10.0")
            .unwrap();

        let content = fs::read_to_string(dir.path().join(REGISTRY_FILENAME)).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert!(parsed.is_table());
    }

    // ── Delete ───────────────────────────────────────────────────────────

    #[test]
    fn delete_existing_value() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Test", "Key", "Value")
            .unwrap();
        let deleted = reg.delete_value("HKLM", "Software\\Test", "Key").unwrap();
        assert!(deleted);

        let val = reg.get_value("HKLM", "Software\\Test", "Key").unwrap();
        assert_eq!(val, None);
    }

    #[test]
    fn delete_nonexistent_value_returns_false() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        let deleted = reg.delete_value("HKLM", "Software\\Test", "Missing").unwrap();
        assert!(!deleted);
    }

    // ── List ─────────────────────────────────────────────────────────────

    #[test]
    fn list_values_returns_all_names() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Test", "A", "1").unwrap();
        reg.set_value("HKLM", "Software\\Test", "B", "2").unwrap();
        reg.set_value("HKLM", "Software\\Test", "C", "3").unwrap();

        let mut names = reg.list_values("HKLM", "Software\\Test").unwrap();
        names.sort();
        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn list_values_empty_for_missing_key() {
        let dir = temp_dir();
        let reg = VirtualRegistry::new(dir.path());

        let names = reg.list_values("HKLM", "Software\\Missing").unwrap();
        assert!(names.is_empty());
    }

    // ── Validation ───────────────────────────────────────────────────────

    #[test]
    fn invalid_hive_rejected_on_get() {
        let dir = temp_dir();
        let reg = VirtualRegistry::new(dir.path());

        let result = reg.get_value("INVALID", "Software", "Key");
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::RegistryKeyInvalid(_) => {}
            other => panic!("expected RegistryKeyInvalid, got: {:?}", other),
        }
    }

    #[test]
    fn invalid_hive_rejected_on_set() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        let result = reg.set_value("INVALID", "Software", "Key", "Value");
        assert!(result.is_err());
    }

    #[test]
    fn valid_hives_accepted() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        for hive in VALID_HIVES {
            reg.set_value(hive, "Test", "Key", "Value").unwrap();
        }
    }

    // ── Path normalization ───────────────────────────────────────────────

    #[test]
    fn forward_slash_normalized_to_backslash() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software/Microsoft", "Version", "10.0")
            .unwrap();

        // Should be retrievable with backslash path.
        let val = reg
            .get_value("HKLM", "Software\\Microsoft", "Version")
            .unwrap();
        assert_eq!(val, Some("10.0".to_string()));
    }

    #[test]
    fn trailing_backslash_trimmed() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Test\\", "Key", "Value")
            .unwrap();

        let val = reg.get_value("HKLM", "Software\\Test", "Key").unwrap();
        assert_eq!(val, Some("Value".to_string()));
    }

    // ── Empty values ─────────────────────────────────────────────────────

    #[test]
    fn empty_string_value_stored() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());

        reg.set_value("HKLM", "Software\\Test", "Empty", "").unwrap();

        let val = reg.get_value("HKLM", "Software\\Test", "Empty").unwrap();
        assert_eq!(val, Some("".to_string()));
    }

    // ── Metadata ─────────────────────────────────────────────────────────

    #[test]
    fn hive_count_tracks_writes() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());
        assert_eq!(reg.hive_count(), 0);

        reg.set_value("HKLM", "Software", "K", "V").unwrap();
        assert_eq!(reg.hive_count(), 1);

        reg.set_value("HKCU", "Software", "K", "V").unwrap();
        assert_eq!(reg.hive_count(), 2);
    }

    #[test]
    fn total_value_count_tracks_writes() {
        let dir = temp_dir();
        let mut reg = VirtualRegistry::new(dir.path());
        assert_eq!(reg.total_value_count(), 0);

        reg.set_value("HKLM", "Software", "A", "1").unwrap();
        assert_eq!(reg.total_value_count(), 1);

        reg.set_value("HKLM", "Software", "B", "2").unwrap();
        assert_eq!(reg.total_value_count(), 2);
    }

    #[test]
    fn with_path_constructor() {
        let dir = temp_dir();
        let path = dir.path().join("custom-registry.toml");
        let mut reg = VirtualRegistry::with_path(path.clone());

        reg.set_value("HKLM", "Test", "Key", "Value").unwrap();
        assert!(path.exists());
        assert_eq!(reg.file_path(), path.as_path());
    }

    #[test]
    fn load_existing_toml_file() {
        let dir = temp_dir();
        let toml_content = r#"
[HKLM]
[HKLM.Software]
TestKey = "TestValue"
"#;
        fs::write(dir.path().join(REGISTRY_FILENAME), toml_content).unwrap();

        let reg = VirtualRegistry::new(dir.path());
        let val = reg.get_value("HKLM", "Software", "TestKey").unwrap();
        assert_eq!(val, Some("TestValue".to_string()));
    }

    #[test]
    fn corrupted_toml_file_returns_empty() {
        let dir = temp_dir();
        fs::write(dir.path().join(REGISTRY_FILENAME), "{{{{not valid toml")
            .unwrap();

        let reg = VirtualRegistry::new(dir.path());
        assert_eq!(reg.hive_count(), 0);
    }
}
