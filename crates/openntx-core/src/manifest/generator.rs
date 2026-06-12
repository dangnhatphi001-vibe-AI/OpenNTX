use crate::manifest::model::{
    AppManifest, AudioConfig, DesktopConfig, DiagnosticsConfig, ExecutableConfig, FilesystemConfig,
    GraphicsConfig, PackagingConfig, RegistryConfig, SourceConfig, WindowsCompatibilityConfig,
};
use crate::manifest::validate_manifest;
use crate::pe::{PeAnalysis, PeArchitecture, PeFormat, PeImageKind, WindowsSubsystem};
use crate::sandbox::SandboxPolicy;
use crate::{OpenNtxError, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestGenerationInput<'a> {
    pub input_path: &'a Path,
    pub analysis: &'a PeAnalysis,
    pub app_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedManifest {
    pub manifest: AppManifest,
    pub install_mode_reason: String,
}

pub fn generate_manifest_from_pe(input: ManifestGenerationInput<'_>) -> Result<GeneratedManifest> {
    if !input.analysis.is_pe {
        return Err(OpenNtxError::Unsupported(format!(
            "manifest generation requires a valid PE file; {} is {}",
            input.analysis.file_name, input.analysis.status
        )));
    }

    let install_mode = install_mode_for(input.analysis);
    let source_type = if install_mode == "captured" {
        "installer"
    } else {
        "portable"
    };
    let sandbox = SandboxPolicy::standard();
    let display_name = display_name(input.input_path);
    let architecture = architecture_string(&input.analysis.architecture).to_string();
    let sha256 = sha256_file(input.input_path)?;

    let (entry_point_rva, image_base) = input
        .analysis
        .optional_header
        .as_ref()
        .map(|optional| {
            (
                Some(format!("0x{:08x}", optional.entry_point_rva)),
                Some(format!("0x{:016x}", optional.image_base)),
            )
        })
        .unwrap_or((None, None));

    let manifest = AppManifest {
        schema_version: "0.1.0".to_string(),
        app_id: input.app_id.clone(),
        name: display_name.clone(),
        version: None,
        source: SourceConfig {
            source_type: source_type.to_string(),
            original_file: input.analysis.file_name.clone(),
            sha256: Some(sha256),
        },
        executable: ExecutableConfig {
            path: input.input_path.display().to_string(),
            arguments: Vec::new(),
            working_directory: input
                .input_path
                .parent()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| ".".to_string()),
        },
        architecture,
        install_mode: install_mode.to_string(),
        windows_compatibility: WindowsCompatibilityConfig {
            version: "windows10".to_string(),
            dpi_mode: "per-monitor-v2".to_string(),
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
        sandbox: (&sandbox).into(),
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
            name: display_name,
            icon: input.app_id,
            categories: categories_for(input.analysis),
        },
        packaging: Some(PackagingConfig {
            deb_package: false,
            package_name: format!("openntx-{}", input.analysis.file_name.to_ascii_lowercase()),
        }),
        diagnostics: DiagnosticsConfig {
            log_level: "info".to_string(),
            crash_reports: true,
            imported_dlls: input.analysis.imported_dlls.clone(),
            entry_point_rva,
            image_base,
        },
    };

    validate_manifest(&manifest)?;

    Ok(GeneratedManifest {
        manifest,
        install_mode_reason: install_mode_reason(input.analysis).to_string(),
    })
}

pub fn install_mode_for(analysis: &PeAnalysis) -> &'static str {
    if analysis.suggested_mode == "capture-install" {
        "captured"
    } else if analysis.subsystem == Some(WindowsSubsystem::WindowsGui)
        && analysis.image_kind == PeImageKind::Executable
    {
        "captured"
    } else {
        "portable"
    }
}

fn install_mode_reason(analysis: &PeAnalysis) -> &'static str {
    if analysis.suggested_mode == "capture-install" {
        "installer-looking filename"
    } else if analysis.subsystem == Some(WindowsSubsystem::WindowsGui)
        && analysis.image_kind == PeImageKind::Executable
    {
        "Windows GUI executable"
    } else {
        "console or portable-looking executable"
    }
}

fn architecture_string(architecture: &PeArchitecture) -> &'static str {
    match architecture {
        PeArchitecture::X86 => "x86",
        PeArchitecture::X86_64 => "x86_64",
        PeArchitecture::Arm64 => "arm64",
        PeArchitecture::Arm | PeArchitecture::Unknown(_) => "unknown",
    }
}

fn categories_for(analysis: &PeAnalysis) -> Vec<String> {
    match (&analysis.subsystem, &analysis.format) {
        (Some(WindowsSubsystem::WindowsCui), _) => vec!["Utility".to_string()],
        (Some(WindowsSubsystem::WindowsGui), PeFormat::Pe32 | PeFormat::Pe32Plus) => {
            vec!["Utility".to_string()]
        }
        _ => vec!["Utility".to_string()],
    }
}

fn display_name(input_path: &Path) -> String {
    input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.replace(['_', '-'], " "))
        .unwrap_or_else(|| "Windows App".to_string())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
