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

    #[error("not implemented in OpenNTX V0.2: {0}")]
    NotImplemented(String),
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
