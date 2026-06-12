use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureReport {
    pub schema_version: String,
    pub capture_id: String,
    pub input_installer: String,
    pub started_at: String,
    pub finished_at: String,
    pub installer_exit_code: i32,
    pub files_created: Vec<String>,
    pub files_modified: Vec<String>,
    pub registry_keys_created: Vec<String>,
    pub registry_values_changed: Vec<String>,
    pub shortcuts_detected: Vec<ShortcutDetected>,
    pub executable_candidates: Vec<ExecutableCandidate>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutDetected {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutableCandidate {
    pub path: String,
    pub score: f64,
    pub reason: String,
}
