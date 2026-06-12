pub mod model;
pub mod validate;
pub mod writer;

pub use model::{
    AppManifest, AudioConfig, DesktopConfig, DiagnosticsConfig, ExecutableConfig, FilesystemConfig,
    GraphicsConfig, PackagingConfig, RegistryConfig, SandboxConfig, SourceConfig,
    WindowsCompatibilityConfig,
};
pub use validate::validate_manifest;
pub use writer::{read_manifest, write_manifest_pretty};
