// main.rs — OpenNTX GUI: Slint-based graphical frontend.
//
// Connects to the OpenNTX API Bridge Server (axum, V2.6.5) via HTTP
// and drives the Slint UI with real-time system data.
//
// Architecture:
//   ┌──────────────┐    reqwest HTTP     ┌──────────────────┐
//   │  Slint UI     │ ──────────────────► │  Axum API Server  │
//   │  (this crate) │ ◄────────────────── │  (openntx-core)   │
//   └──────────────┘    JSON responses    └──────────────────┘

use serde::{Deserialize, Serialize};
use slint::SharedString;
use std::sync::{Arc, Mutex};

// Include the Slint UI generated code.
slint::include_modules!();

// ── API response types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MonitorResponse {
    total_pids: i32,
    ram_usage: String,
    cpu_quota: String,
    apps: Vec<AppInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppInfo {
    app_id: String,
    app_name: String,
    exe_path: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecuteResponse {
    status: String,
    app_id: String,
    message: Option<String>,
}

// ── API client ───────────────────────────────────────────────────────────────

/// Base URL for the OpenNTX API Bridge Server.
const API_BASE: &str = "http://127.0.0.1:8420";

/// Fetch system monitor data from the API.
///
/// Returns `None` if the server is unreachable (graceful degradation).
async fn fetch_monitor(client: &reqwest::Client) -> Option<MonitorResponse> {
    let url = format!("{}/api/v1/monitor", API_BASE);
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => resp.json::<MonitorResponse>().await.ok(),
        _ => None,
    }
}

/// Send an execute request to the API.
async fn send_execute(
    client: &reqwest::Client,
    exe_path: &str,
    hardened: bool,
) -> Option<ExecuteResponse> {
    let url = format!("{}/api/v1/execute", API_BASE);
    let body = serde_json::json!({
        "app_id": derive_app_id(exe_path),
        "exe_path": exe_path,
        "hardened_mode": hardened,
    });
    match client.post(&url).json(&body).send().await {
        Ok(resp) => resp.json::<ExecuteResponse>().await.ok(),
        _ => None,
    }
}

/// Derive a simple app_id from the exe path (filename without extension).
fn derive_app_id(exe_path: &str) -> String {
    std::path::Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_lowercase()
        .replace(' ', "-")
}

// ── Log state ────────────────────────────────────────────────────────────────

/// Thread-safe log buffer that accumulates messages and pushes them to Slint.
struct LogState {
    lines: Vec<String>,
}

impl LogState {
    fn new() -> Self {
        Self {
            lines: vec!["[SYSTEM] OpenNTX GUI initialized.".to_string()],
        }
    }

    fn append(&mut self, msg: &str) {
        let timestamp = chrono_free_timestamp();
        self.lines.push(format!("[{}] {}", timestamp, msg));
        // Keep last 500 lines.
        if self.lines.len() > 500 {
            self.lines.remove(0);
        }
    }

    fn to_string(&self) -> String {
        self.lines.join("\n")
    }
}

/// Simple timestamp without external crate dependency.
fn chrono_free_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (now / 3600) % 24;
    let minutes = (now / 60) % 60;
    let seconds = now % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client");

    let client = Arc::new(client);
    let log_state = Arc::new(Mutex::new(LogState::new()));

    // ── Deploy callback ──────────────────────────────────────────────────
    {
        let client = client.clone();
        let log_state = log_state.clone();
        let window_weak = window.as_weak();

        window.on_deploy_clicked(move |exe_path: SharedString| {
            let exe_path = exe_path.to_string();
            if exe_path.is_empty() {
                if let Some(log) = log_state.lock().ok() {
                    let mut log = log;
                    log.append("[ERROR] No EXE path provided.");
                    if let Some(win) = window_weak.upgrade() {
                        win.set_log_content(SharedString::from(log.to_string().as_str()));
                    }
                }
                return;
            }

            let client = client.clone();
            let log_state = log_state.clone();
            let window_weak = window_weak.clone();
            let exe = exe_path.clone();

            // Update UI state immediately.
            if let Some(win) = window_weak.upgrade() {
                win.set_is_deploying(true);
                win.set_deploy_status(SharedString::from("DEPLOYING"));
                if let Ok(mut log) = log_state.lock() {
                    log.append(&format!("[DEPLOY] Sending execute request for: {}", exe));
                    win.set_log_content(SharedString::from(log.to_string().as_str()));
                }
            }

            // Spawn async task to call the API.
            tokio::spawn(async move {
                let result = send_execute(&client, &exe, false).await;

                let (success, message) = match result {
                    Some(resp) if resp.status == "success" => (
                        true,
                        format!("[OK] Execution started for: {}", resp.app_id),
                    ),
                    Some(resp) => (
                        false,
                        format!(
                            "[FAIL] {}: {}",
                            resp.status,
                            resp.message.unwrap_or_default()
                        ),
                    ),
                    None => (
                        false,
                        "[ERROR] API server unreachable. Is the daemon running?".to_string(),
                    ),
                };

                if let Ok(mut log) = log_state.lock() {
                    log.append(&message);
                    if let Some(win) = window_weak.upgrade() {
                        win.set_log_content(SharedString::from(log.to_string().as_str()));
                        win.set_is_deploying(false);
                        win.set_deploy_status(SharedString::from(if success {
                            "DEPLOYED"
                        } else {
                            "FAILED"
                        }));
                    }
                }
            });
        });
    }

    // ── Refresh callback ─────────────────────────────────────────────────
    {
        let window_weak = window.as_weak();
        window.on_refresh_clicked(move || {
            if let Some(win) = window_weak.upgrade() {
                win.set_deploy_status(SharedString::from("REFRESHING"));
            }
        });
    }

    // ── Monitor polling loop ─────────────────────────────────────────────
    {
        let client = client.clone();
        let window_weak = window.as_weak();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                interval.tick().await;

                let monitor = fetch_monitor(&client).await;

                if let Some(win) = window_weak.upgrade() {
                    if let Some(data) = monitor {
                        win.set_total_pids(data.total_pids);
                        win.set_ram_usage(SharedString::from(data.ram_usage.as_str()));
                        win.set_cpu_quota(SharedString::from(data.cpu_quota.as_str()));

                        // Update app list.
                        let entries: Vec<AppEntry> = data
                            .apps
                            .iter()
                            .map(|a| AppEntry {
                                app_id: SharedString::from(a.app_id.as_str()),
                                app_name: SharedString::from(a.app_name.as_str()),
                                exe_path: SharedString::from(a.exe_path.as_str()),
                                status: SharedString::from(a.status.as_str()),
                            })
                            .collect();
                        win.set_app_list(std::rc::Rc::new(slint::VecModel::from(entries)).into());
                    } else {
                        // Server unreachable — show placeholder data.
                        win.set_total_pids(0);
                        win.set_ram_usage(SharedString::from("-- MB"));
                        win.set_cpu_quota(SharedString::from("offline"));
                    }
                } else {
                    // Window closed — exit polling loop.
                    break;
                }
            }
        });
    }

    // ── Initial log message ──────────────────────────────────────────────
    {
        if let Ok(mut log) = log_state.lock() {
            log.append("[SYSTEM] Connecting to API server at 127.0.0.1:8420...");
            window.set_log_content(SharedString::from(log.to_string().as_str()));
        }
    }

    window.run()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_app_id_basic() {
        assert_eq!(derive_app_id("/home/user/notepad.exe"), "notepad");
        assert_eq!(derive_app_id("/path/to/My App.exe"), "my-app");
        assert_eq!(derive_app_id("game.exe"), "game");
    }

    #[test]
    fn derive_app_id_unknown_fallback() {
        assert_eq!(derive_app_id(""), "unknown");
        assert_eq!(derive_app_id("/no/extension"), "extension");
    }

    #[test]
    fn log_state_append_and_to_string() {
        let mut log = LogState::new();
        assert!(log.to_string().contains("initialized"));

        log.append("test message");
        assert!(log.to_string().contains("test message"));
    }

    #[test]
    fn log_state_truncates_at_500() {
        let mut log = LogState::new();
        for i in 0..600 {
            log.append(&format!("line {}", i));
        }
        // Should have at most 500 lines.
        let content = log.to_string();
        let line_count = content.lines().count();
        assert!(line_count <= 500, "expected <= 500 lines, got {}", line_count);
        // The earliest lines should be gone.
        assert!(!content.contains("line 0"));
        // The latest lines should be present.
        assert!(content.contains("line 599"));
    }

    #[test]
    fn monitor_response_deserialize() {
        let json = r#"{
            "total_pids": 42,
            "ram_usage": "2.1 GB",
            "cpu_quota": "50000 100000",
            "apps": [
                {
                    "app_id": "notepad",
                    "app_name": "Notepad++",
                    "exe_path": "/usr/bin/notepad.exe",
                    "status": "running"
                }
            ]
        }"#;

        let resp: MonitorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total_pids, 42);
        assert_eq!(resp.ram_usage, "2.1 GB");
        assert_eq!(resp.apps.len(), 1);
        assert_eq!(resp.apps[0].app_id, "notepad");
    }

    #[test]
    fn execute_response_deserialize() {
        let json = r#"{"status": "success", "app_id": "test-app"}"#;
        let resp: ExecuteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.app_id, "test-app");
        assert!(resp.message.is_none());
    }

    #[test]
    fn execute_response_with_message() {
        let json = r#"{"status": "error", "app_id": "", "message": "not found"}"#;
        let resp: ExecuteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "error");
        assert_eq!(resp.message, Some("not found".to_string()));
    }

    #[test]
    fn app_entry_model_fields() {
        // Verify AppEntry struct can be constructed with expected fields.
        let entry = AppEntry {
            app_id: SharedString::from("test-id"),
            app_name: SharedString::from("Test App"),
            exe_path: SharedString::from("/path/to/app.exe"),
            status: SharedString::from("running"),
        };
        assert_eq!(entry.app_id.as_str(), "test-id");
        assert_eq!(entry.app_name.as_str(), "Test App");
        assert_eq!(entry.status.as_str(), "running");
    }

    #[test]
    fn chrono_free_timestamp_format() {
        let ts = chrono_free_timestamp();
        // Should be HH:MM:SS format.
        assert_eq!(ts.len(), 8);
        assert_eq!(ts.as_bytes()[2], b':');
        assert_eq!(ts.as_bytes()[5], b':');
    }
}
