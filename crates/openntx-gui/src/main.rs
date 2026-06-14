// main.rs — OpenNTX GUI: Slint-based graphical frontend.
//
// V3.0.0: Consumer Edition — 1-Click .deb Package.
//
// Connects to the OpenNTX API Bridge Server (axum) via HTTP
// and drives the Slint UI with real-time system data.
//
// File Input:
//   • Native file dialog via rfd (XDG Desktop Portal / GTK fallback)
//   • Drag-and-drop via Slint WinitWindowAccessor::on_winit_window_event
//
// Auto-Fallback Launcher:
//   • On startup, checks if the API server is already live.
//   • If not, spawns /usr/bin/openntx-appportal as a detached background
//     process so users never have to start the daemon manually.

use serde::{Deserialize, Serialize};
use slint::SharedString;
use std::sync::{Arc, Mutex};

// Include the Slint UI generated code.
slint::include_modules!();

// winit integration for drag-and-drop file handling.
use slint::winit_030::{EventResult, WinitWindowAccessor};

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

const API_BASE: &str = "http://127.0.0.1:8080";
const APPORTAL_BIN: &str = "/usr/bin/openntx-appportal";

/// Check if the API server is already running by sending a quick GET to /api/v1/monitor.
async fn is_api_server_live() -> bool {
    let url = format!("{}/api/v1/monitor", API_BASE);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();
    matches!(client.get(&url).send().await, Ok(resp) if resp.status().is_success())
}

/// Spawn the OpenNTX AppPortal daemon as a detached background process.
///
/// This function uses a double-fork technique to fully detach the child
/// from the GUI process so it survives after the GUI exits.
fn spawn_appportal_detached() {
    use std::os::unix::process::CommandExt;

    // Check if the binary exists before trying to spawn
    if !std::path::Path::new(APPORTAL_BIN).exists() {
        eprintln!(
            "openntx-gui: warning: {} not found — daemon auto-start skipped",
            APPORTAL_BIN
        );
        return;
    }

    match std::process::Command::new(APPORTAL_BIN)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0) // Create a new process group to detach
        .spawn()
    {
        Ok(child) => {
            eprintln!(
                "openntx-gui: auto-started openntx-appportal (PID {})",
                child.id()
            );
        }
        Err(err) => {
            eprintln!(
                "openntx-gui: warning: failed to auto-start {}: {}",
                APPORTAL_BIN, err
            );
        }
    }
}

async fn fetch_monitor(client: &reqwest::Client) -> Option<MonitorResponse> {
    let url = format!("{}/api/v1/monitor", API_BASE);
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => resp.json::<MonitorResponse>().await.ok(),
        _ => None,
    }
}

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

fn derive_app_id(exe_path: &str) -> String {
    std::path::Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_lowercase()
        .replace(' ', "-")
}

// ── Log state ────────────────────────────────────────────────────────────────

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
        if self.lines.len() > 500 {
            self.lines.remove(0);
        }
    }

    fn to_string(&self) -> String {
        self.lines.join("\n")
    }
}

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

// ── File drop helpers ────────────────────────────────────────────────────────

fn is_exe_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // ── Auto-Fallback Launcher ───────────────────────────────────────────
    //
    // Before drawing the GUI, check if the API server is already live.
    // If not, spawn the AppPortal daemon as a detached background process.
    // This ensures the consumer experience: install .deb → launch GUI → everything works.
    if !is_api_server_live().await {
        eprintln!("openntx-gui: API server not detected — auto-starting daemon...");
        spawn_appportal_detached();
        // Give the daemon a moment to bind the port
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    }

    let window = AppWindow::new()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client");

    let client = Arc::new(client);
    let log_state = Arc::new(Mutex::new(LogState::new()));

    // ── Native File Dialog (rfd) ─────────────────────────────────────────
    {
        let log_state = log_state.clone();
        let window_weak = window.as_weak();

        window.on_open_file_clicked(move || {
            let dialog = rfd::FileDialog::new()
                .set_title("Select Windows Executable")
                .add_filter("Windows Executable", &["exe"]);

            if let Some(path) = dialog.pick_file() {
                let path_str = path.to_string_lossy().to_string();

                if is_exe_file(&path) {
                    let path_for_ui = path_str.clone();
                    let log_for_ui = log_state.clone();
                    let weak_for_ui = window_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(win) = weak_for_ui.upgrade() {
                            win.set_current_exe_path(SharedString::from(path_for_ui.as_str()));
                            win.set_deploy_status(SharedString::from("READY"));
                        }
                        if let Ok(mut log) = log_for_ui.lock() {
                            log.append(&format!("[FILE] Selected: {}", path_for_ui));
                            if let Some(win) = weak_for_ui.upgrade() {
                                win.set_log_content(SharedString::from(log.to_string().as_str()));
                            }
                        }
                    })
                    .expect("failed to invoke on Slint event loop");
                } else {
                    let log_ref = log_state.clone();
                    let weak_ref = window_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Ok(mut log) = log_ref.lock() {
                            log.append(&format!("[REJECTED] Not a .exe file: {}", path_str));
                            if let Some(win) = weak_ref.upgrade() {
                                win.set_log_content(SharedString::from(log.to_string().as_str()));
                            }
                        }
                    })
                    .expect("failed to invoke on Slint event loop");
                }
            }
        });
    }

    // ── Deploy callback ──────────────────────────────────────────────────
    {
        let client = client.clone();
        let log_state = log_state.clone();
        let window_weak = window.as_weak();

        window.on_deploy_clicked(move |exe_path: SharedString| {
            let exe_path = exe_path.to_string();
            if exe_path.is_empty() {
                if let Ok(mut log) = log_state.lock() {
                    log.append("[ERROR] No EXE path provided. Drag & drop or Browse first.");
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

            if let Some(win) = window_weak.upgrade() {
                win.set_is_deploying(true);
                win.set_deploy_status(SharedString::from("DEPLOYING"));
                if let Ok(mut log) = log_state.lock() {
                    log.append(&format!("[DEPLOY] Sending execute request for: {}", exe));
                    win.set_log_content(SharedString::from(log.to_string().as_str()));
                }
            }

            tokio::spawn(async move {
                let result = send_execute(&client, &exe, false).await;

                let (success, message) = match result {
                    Some(resp) if resp.status == "success" => {
                        (true, format!("[OK] Execution started for: {}", resp.app_id))
                    }
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

    // ── Drag-and-Drop via WinitWindowAccessor::on_winit_window_event ────
    //
    // Registers a native winit window event filter on the Slint window.
    // When a file is dragged from the file manager and dropped onto the
    // OpenNTX window, winit fires WindowEvent::DroppedFile.
    // Only .exe files are accepted; other extensions are logged as rejected.
    // Requires the `unstable-winit-030` feature on the slint crate.
    {
        let log_state = log_state.clone();
        let window_weak = window.as_weak();

        window
            .window()
            .on_winit_window_event(move |_slint_window, event| {
                use winit::event::WindowEvent;

                if let WindowEvent::DroppedFile(path) = event {
                    let path_str = path.to_string_lossy().to_string();

                    if is_exe_file(path) {
                        let path_clone = path_str.clone();
                        let log_ref = log_state.clone();
                        let weak_ref = window_weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(win) = weak_ref.upgrade() {
                                win.set_current_exe_path(SharedString::from(path_clone.as_str()));
                                win.set_deploy_status(SharedString::from("READY"));
                            }
                            if let Ok(mut log) = log_ref.lock() {
                                log.append(&format!("[DROP] File accepted: {}", path_clone));
                                if let Some(win) = weak_ref.upgrade() {
                                    win.set_log_content(SharedString::from(
                                        log.to_string().as_str(),
                                    ));
                                }
                            }
                        })
                        .expect("failed to invoke on Slint event loop");
                    } else {
                        let log_ref = log_state.clone();
                        let weak_ref = window_weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Ok(mut log) = log_ref.lock() {
                                log.append(&format!("[REJECTED] Not a .exe file: {}", path_str));
                                if let Some(win) = weak_ref.upgrade() {
                                    win.set_log_content(SharedString::from(
                                        log.to_string().as_str(),
                                    ));
                                }
                            }
                        })
                        .expect("failed to invoke on Slint event loop");
                    }
                }

                // Let Slint handle all other events normally.
                EventResult::Propagate
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
                        win.set_total_pids(0);
                        win.set_ram_usage(SharedString::from("-- MB"));
                        win.set_cpu_quota(SharedString::from("offline"));
                    }
                } else {
                    break;
                }
            }
        });
    }

    // ── Initial log message ──────────────────────────────────────────────
    {
        if let Ok(mut log) = log_state.lock() {
            log.append("[SYSTEM] Connecting to API server at 127.0.0.1:8080...");
            log.append("[SYSTEM] Drop zone active - drag a .exe file onto the window.");
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
        let content = log.to_string();
        let line_count = content.lines().count();
        assert!(
            line_count <= 500,
            "expected <= 500 lines, got {}",
            line_count
        );
        assert!(!content.contains("line 0"));
        assert!(content.contains("line 599"));
    }

    #[test]
    fn monitor_response_deserialize() {
        let json = r#"{
            "total_pids": 42,
            "ram_usage": "2.1 GB",
            "cpu_quota": "50000 100000",
            "apps": [{"app_id": "notepad", "app_name": "Notepad++", "exe_path": "/usr/bin/notepad.exe", "status": "running"}]
        }"#;
        let resp: MonitorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total_pids, 42);
        assert_eq!(resp.apps.len(), 1);
    }

    #[test]
    fn execute_response_deserialize() {
        let json = r#"{"status": "success", "app_id": "test-app"}"#;
        let resp: ExecuteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
    }

    #[test]
    fn execute_response_with_message() {
        let json = r#"{"status": "error", "app_id": "", "message": "not found"}"#;
        let resp: ExecuteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message, Some("not found".to_string()));
    }

    #[test]
    fn app_entry_model_fields() {
        let entry = AppEntry {
            app_id: SharedString::from("test-id"),
            app_name: SharedString::from("Test App"),
            exe_path: SharedString::from("/path/to/app.exe"),
            status: SharedString::from("running"),
        };
        assert_eq!(entry.app_id.as_str(), "test-id");
    }

    #[test]
    fn chrono_free_timestamp_format() {
        let ts = chrono_free_timestamp();
        assert_eq!(ts.len(), 8);
        assert_eq!(ts.as_bytes()[2], b':');
    }

    #[test]
    fn is_exe_file_valid() {
        assert!(is_exe_file(std::path::Path::new("/path/to/app.exe")));
        assert!(is_exe_file(std::path::Path::new("C:\\Games\\game.EXE")));
        assert!(is_exe_file(std::path::Path::new("test.Exe")));
    }

    #[test]
    fn is_exe_file_invalid() {
        assert!(!is_exe_file(std::path::Path::new("/path/to/app.txt")));
        assert!(!is_exe_file(std::path::Path::new("/path/to/app")));
        assert!(!is_exe_file(std::path::Path::new("/path/to/app.msi")));
        assert!(!is_exe_file(std::path::Path::new("")));
    }
}
