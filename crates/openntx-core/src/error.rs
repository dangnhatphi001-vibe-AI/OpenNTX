use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum OpenNtxError {
    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("manifest validation failed: {0}")]
    ManifestValidation(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("not implemented in OpenNTX V1.0-alpha: {0}")]
    NotImplemented(String),

    #[error("unsafe path: {0}")]
    UnsafePath(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("tool not available: {0}")]
    ToolNotAvailable(String),

    #[error("diagnostic warning: {0}")]
    Diagnostic(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("binfmt registration failed: {0}")]
    BinfmtRegistration(String),

    #[error("binfmt unregistration failed: {0}")]
    BinfmtUnregistration(String),

    #[error("runtime execution failed: {0}")]
    RuntimeExecution(String),

    #[error("wine prefix error: {0}")]
    WinePrefix(String),

    #[error("cgroup creation failed: {0}")]
    CgroupCreationFailed(String),

    #[error("cgroup write failed: {0}")]
    CgroupWriteFailed(String),

    #[error("namespace unshare failed: {0}")]
    NamespaceUnshareFailed(String),

    #[error("reaper process kill failed: {0}")]
    ReaperProcessKillFailed(String),

    #[error("cgroup cleanup failed: {0}")]    CgroupCleanupFailed(String),

    #[error("graphics context creation failed: {0}")]
    GraphicsContextCreationFailed(String),

    #[error("registry storage error: {0}")]
    RegistryStorageError(String),

    #[error("registry key invalid: {0}")]
    RegistryKeyInvalid(String),
}

pub type Result<T> = std::result::Result<T, OpenNtxError>;

impl OpenNtxError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
