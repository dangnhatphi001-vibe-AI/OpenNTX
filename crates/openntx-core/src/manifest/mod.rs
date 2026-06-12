pub mod generator;
pub mod model;
pub mod validate;
pub mod writer;

pub use generator::{
    generate_manifest_from_pe, install_mode_for, GeneratedManifest, ManifestGenerationInput,
};
pub use model::{
    AppManifest, AudioConfig, DesktopConfig, DiagnosticsConfig, ExecutableConfig, FilesystemConfig,
    GraphicsConfig, PackagingConfig, RegistryConfig, SandboxConfig, SourceConfig,
    WindowsCompatibilityConfig,
};
pub use validate::validate_manifest;
pub use writer::{read_json_safe, read_manifest, write_manifest_pretty};
