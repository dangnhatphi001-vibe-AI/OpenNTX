use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppManifest {
    pub schema_version: String,
    pub app_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub source: SourceConfig,
    pub executable: ExecutableConfig,
    pub architecture: String,
    pub install_mode: String,
    pub windows_compatibility: WindowsCompatibilityConfig,
    pub filesystem: FilesystemConfig,
    pub registry: RegistryConfig,
    pub sandbox: SandboxConfig,
    pub graphics: GraphicsConfig,
    pub audio: AudioConfig,
    pub desktop: DesktopConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packaging: Option<PackagingConfig>,
    pub diagnostics: DiagnosticsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: String,
    pub original_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableConfig {
    pub path: String,
    pub arguments: Vec<String>,
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsCompatibilityConfig {
    pub version: String,
    pub dpi_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemConfig {
    pub drive_c: String,
    pub home_mapping: String,
    pub documents_access: String,
    pub downloads_access: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub mode: String,
    pub user_hive: String,
    pub machine_hive: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub profile: String,
    pub network: String,
    pub home: String,
    pub documents: String,
    pub downloads: String,
    pub removable_drives: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphicsConfig {
    pub preferred_backend: String,
    pub d3d_translation: String,
    pub wayland: bool,
    pub x11: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub backend: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopConfig {
    pub create_launcher: bool,
    pub name: String,
    pub icon: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackagingConfig {
    pub deb_package: bool,
    pub package_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    pub log_level: String,
    pub crash_reports: bool,
}

impl AppManifest {
    pub fn minimal(app_id: &str, name: &str, executable_path: &str) -> Self {
        Self {
            schema_version: "0.1.0".to_string(),
            app_id: app_id.to_string(),
            name: name.to_string(),
            version: None,
            source: SourceConfig {
                source_type: "unknown".to_string(),
                original_file: executable_path.to_string(),
                sha256: None,
            },
            executable: ExecutableConfig {
                path: executable_path.to_string(),
                arguments: Vec::new(),
                working_directory: "C:/".to_string(),
            },
            architecture: "unknown".to_string(),
            install_mode: "planned".to_string(),
            windows_compatibility: WindowsCompatibilityConfig {
                version: "auto".to_string(),
                dpi_mode: "auto".to_string(),
            },
            filesystem: FilesystemConfig {
                drive_c: "drive_c".to_string(),
                home_mapping: "limited".to_string(),
                documents_access: "ask".to_string(),
                downloads_access: "ask".to_string(),
            },
            registry: RegistryConfig {
                mode: "overlay".to_string(),
                user_hive: "registry/user.json".to_string(),
                machine_hive: "registry/machine.json".to_string(),
            },
            sandbox: SandboxConfig {
                profile: "standard".to_string(),
                network: "ask".to_string(),
                home: "deny".to_string(),
                documents: "ask".to_string(),
                downloads: "ask".to_string(),
                removable_drives: "deny".to_string(),
            },
            graphics: GraphicsConfig {
                preferred_backend: "auto".to_string(),
                d3d_translation: "future".to_string(),
                wayland: true,
                x11: true,
            },
            audio: AudioConfig {
                backend: "pipewire".to_string(),
            },
            desktop: DesktopConfig {
                create_launcher: true,
                name: name.to_string(),
                icon: app_id.to_string(),
                categories: vec!["Utility".to_string()],
            },
            packaging: None,
            diagnostics: DiagnosticsConfig {
                log_level: "info".to_string(),
                crash_reports: true,
            },
        }
    }
}
