use crate::{OpenNtxError, Result};

#[derive(Debug, Default)]
pub struct CaptureTracker;

impl CaptureTracker {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self) -> Result<()> {
        Err(OpenNtxError::NotImplemented(
            "installer capture tracker is a future module".to_string(),
        ))
    }
}
