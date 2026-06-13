// profile/schema.rs — Compatibility Profile data model for OpenNTX V1.2.
//
// Each Windows application registered with OpenNTX can have an associated
// `CompatProfile` that captures runtime requirements, filesystem / registry
// rules, and installer behaviour.  Profiles are stored as JSON files under
// `~/.local/share/openntx/profiles/<app_id>.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Top-level profile ────────────────────────────────────────────────────────

/// Compatibility profile for a single Windows application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompatProfile {
    /// Unique application identifier (e.g. `"notepadpp-v8.6"`).
    pub app_id: String,

    /// Human-readable metadata about the application.
    pub metadata: AppMetadata,

    /// Runtime dependencies and DirectX requirements.
    pub runtime_reqs: RuntimeReqs,

    /// Filesystem paths the application needs and host ↔ guest mappings.
    pub fs_rules: FilesystemRules,

    /// Windows registry keys the application expects.
    pub reg_rules: RegistryRules,

    /// How the original installer should be handled.
    pub installer: InstallerBehavior,
}

// ── Metadata ─────────────────────────────────────────────────────────────────

/// Human-readable information about the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppMetadata {
    /// Display name (e.g. `"Notepad++"`).
    pub name: String,

    /// Version string as reported by the application.
    pub version: String,

    /// Publisher / vendor name.
    pub publisher: String,

    /// Target CPU architecture.
    pub arch: Arch,
}

/// CPU architecture the application was compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    /// 32-bit x86.
    X86,
    /// 64-bit x86-64.
    X86_64,
}

// ── Runtime requirements ─────────────────────────────────────────────────────

/// Runtime dependencies the application needs at execution time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReqs {
    /// List of runtime components (e.g. `["vcrun2019", "dotnet48"]`).
    pub dependencies: Vec<String>,

    /// Required DirectX version, if any (e.g. `"dx11"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directx: Option<String>,
}

// ── Filesystem rules ─────────────────────────────────────────────────────────

/// Filesystem paths and mappings required by the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemRules {
    /// Paths that must exist inside the Wine prefix before launch
    /// (e.g. `["C:/Program Files/App", "C:/Users/Public/AppData"]`).
    pub required_paths: Vec<String>,

    /// Host-path → guest-path mappings for bind mounts or symlinks.
    /// Keys are host paths, values are Wine-prefix paths.
    pub path_mappings: HashMap<String, String>,
}

// ── Registry rules ───────────────────────────────────────────────────────────

/// Windows registry keys the application expects to find.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRules {
    /// Registry keys that must be present (e.g.
    /// `["HKCU\\Software\\Notepad++", "HKLM\\Software\\ODBC"]`).
    pub required_keys: Vec<String>,
}

// ── Installer behaviour ──────────────────────────────────────────────────────

/// How the original Windows installer should be executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallerBehavior {
    /// Installer technology (e.g. `"nsis"`, `"inno"`, `"msi"`, `"exe"`).
    pub installer_type: String,

    /// Arguments to pass for silent / unattended installation
    /// (e.g. `["/S", "/D=C:\\Program Files\\App"]`).
    pub silent_args: Vec<String>,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal profile for round-trip testing.
    fn sample_profile() -> CompatProfile {
        CompatProfile {
            app_id: "notepadpp-v8.6".to_string(),
            metadata: AppMetadata {
                name: "Notepad++".to_string(),
                version: "8.6".to_string(),
                publisher: "Don Ho".to_string(),
                arch: Arch::X86_64,
            },
            runtime_reqs: RuntimeReqs {
                dependencies: vec!["vcrun2019".to_string()],
                directx: None,
            },
            fs_rules: FilesystemRules {
                required_paths: vec!["C:/Program Files/Notepad++".to_string()],
                path_mappings: HashMap::new(),
            },
            reg_rules: RegistryRules {
                required_keys: vec!["HKCU\\Software\\Notepad++".to_string()],
            },
            installer: InstallerBehavior {
                installer_type: "nsis".to_string(),
                silent_args: vec!["/S".to_string()],
            },
        }
    }

    #[test]
    fn round_trip_json() {
        let profile = sample_profile();
        let json = serde_json::to_string_pretty(&profile).unwrap();
        let parsed: CompatProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, parsed);
    }

    #[test]
    fn arch_serde() {
        assert_eq!(serde_json::to_string(&Arch::X86).unwrap(), "\"x86\"");
        assert_eq!(
            serde_json::to_string(&Arch::X86_64).unwrap(),
            "\"x86_64\""
        );
        assert_eq!(serde_json::from_str::<Arch>("\"x86\"").unwrap(), Arch::X86);
        assert_eq!(
            serde_json::from_str::<Arch>("\"x86_64\"").unwrap(),
            Arch::X86_64
        );
    }

    #[test]
    fn optional_directx_absent() {
        let profile = sample_profile();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(
            !json.contains("\"directx\""),
            "directx should be omitted when None"
        );
    }

    #[test]
    fn optional_directx_present() {
        let mut profile = sample_profile();
        profile.runtime_reqs.directx = Some("dx11".to_string());
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"directx\""));
        let parsed: CompatProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.runtime_reqs.directx, Some("dx11".to_string()));
    }

    #[test]
    fn empty_path_mappings() {
        let profile = sample_profile();
        let json = serde_json::to_string(&profile).unwrap();
        // path_mappings should still be serialized (not skipped).
        assert!(json.contains("\"path_mappings\""));
    }
}
