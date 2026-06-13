// app.rs — Application state model for the OpenNTX async TUI.
//
// Owns every piece of mutable state the UI layer needs to render.
// The main loop receives `AppEvent`s from the channel and calls
// `App::handle_event()` which mutates state accordingly.
//
// Long-running operations (PE analysis, doctor, packaging, …) are
// spawned as background tokio tasks; their results arrive back via
// `AppEvent::WorkerComplete` / `AppEvent::WorkerError`.

use crate::events::{AppEvent, EventSender, WorkerKind, WorkerPayload, WorkerResult};
use openntx_core::capture::CaptureRegistryService;
use openntx_core::doctor::{app_doctor, global_doctor, repair_app};
use openntx_core::logs::list_logs;
use openntx_core::manifest::{generate_manifest_from_pe, AppManifest, ManifestGenerationInput};
use openntx_core::packaging::{build_deb_package, DebBuildOptions};
use openntx_core::pe::analyze_pe;
use openntx_core::profile::{CompatProfile, ProfileManager};
use openntx_core::registry::{AppRegistry, DesktopMode, RegisteredApp, RemoveMode};
use openntx_core::runtime::{create_registered_run_plan, RunPlanOptions, RunPlanReport};
use openntx_core::{OpenNtxError, Result};
use serde_json::Value as JsonValue;
use std::path::PathBuf;

/// Current version string (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Screen / mode enum ──────────────────────────────────────────────────────

/// Every distinct screen the AppPortal can display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AppMode {
    /// Top-level home / main menu.
    Home,
    /// List of registered apps.
    Library,
    /// Details screen for a single app (selected by index in `library_apps`).
    AppDetails,
    /// PE/EXE analysis wizard.
    Analyze,
    /// Install-plan wizard.
    InstallPlan,
    /// Capture sub-menu (pick an app, then pick snapshot/diff/report/…).
    Capture,
    /// Capture sub-action after an app has been selected.
    CaptureAction,
    /// Package builder sub-menu.
    Package,
    /// Logs viewer.
    Logs,
    /// Doctor sub-menu (global vs per-app).
    Doctor,
    /// Doctor result display.
    DoctorResult,
    /// Settings screen.
    Settings,
}

// ── Feedback bar ─────────────────────────────────────────────────────────────

/// Non-blocking feedback shown in a status bar.
#[derive(Debug, Clone)]
pub enum Feedback {
    None,
    Info(String),
    Success(String),
    Error(String),
}

// ── Main application state ───────────────────────────────────────────────────

/// Central state container.  `ui::draw()` receives an immutable borrow.
#[allow(dead_code)]
pub struct AppState {
    // ── navigation ──
    pub mode: AppMode,
    /// Stack of previous modes so `[B] Back` works.
    pub mode_stack: Vec<AppMode>,

    // ── loading / spinner ──
    pub is_loading: bool,
    pub spinner_frame: u8,

    // ── feedback bar ──
    pub feedback: Feedback,
    pub feedback_ttl: u8, // ticks remaining before auto-clear

    // ── shared indices ──
    pub selected_index: usize,

    // ── cached data ──
    pub registered_app_count: usize,
    pub library_apps: Vec<RegisteredApp>,
    pub logs_list: Vec<openntx_core::logs::LogSummary>,

    // ── detail / action context ──
    pub selected_app_id: Option<String>,
    pub manifest: Option<AppManifest>,
    pub desktop_exists: bool,

    // ── last worker result (consumed by UI on next draw) ──
    pub last_run_plan: Option<RunPlanReport>,
    pub last_doctor_json: Option<JsonValue>,
    pub last_capture_json: Option<JsonValue>,
    pub last_package_json: Option<JsonValue>,
    pub last_error: Option<String>,

    // ── quit flag ──
    pub should_quit: bool,

    // ── capture sub-mode selection (0..=5) ──
    pub capture_action_index: usize,

    // ── doctor sub-mode: 0 = global, 1 = per-app ──
    pub doctor_sub_index: usize,

    // ── compatibility profiles (loaded from disk at startup) ──
    pub profiles: Vec<CompatProfile>,
    pub selected_profile_index: usize,

    // ── live capture status from runtime IPC ──
    pub live_capture_status: Option<String>,
}

impl AppState {
    /// Construct with sane defaults.  Does **not** touch the registry — call
    /// `refresh_app_count()` after creation.
    pub fn new() -> Self {
        Self {
            mode: AppMode::Home,
            mode_stack: Vec::new(),
            is_loading: false,
            spinner_frame: 0,
            feedback: Feedback::None,
            feedback_ttl: 0,
            selected_index: 0,
            registered_app_count: 0,
            library_apps: Vec::new(),
            logs_list: Vec::new(),
            selected_app_id: None,
            manifest: None,
            desktop_exists: false,
            last_run_plan: None,
            last_doctor_json: None,
            last_capture_json: None,
            last_package_json: None,
            last_error: None,
            should_quit: false,
            capture_action_index: 0,
            doctor_sub_index: 0,
            profiles: Vec::new(),
            selected_profile_index: 0,
            live_capture_status: None,
        }
    }

    // ── navigation helpers ──────────────────────────────────────────────────

    /// Push current mode onto the stack and switch to `new_mode`.
    pub fn push_mode(&mut self, new_mode: AppMode) {
        self.mode_stack.push(self.mode);
        self.mode = new_mode;
        self.selected_index = 0;
        self.clear_feedback();
    }

    /// Pop the previous mode from the stack.  Returns to `Home` if empty.
    pub fn pop_mode(&mut self) {
        self.mode = self.mode_stack.pop().unwrap_or(AppMode::Home);
        self.selected_index = 0;
        self.clear_feedback();
    }

    /// Navigate directly (no stack push).
    #[allow(dead_code)]
    pub fn goto(&mut self, mode: AppMode) {
        self.mode = mode;
        self.selected_index = 0;
        self.clear_feedback();
    }

    // ── selection movement ──────────────────────────────────────────────────

    pub fn select_next(&mut self, max: usize) {
        if max > 0 {
            self.selected_index = (self.selected_index + 1) % max;
        }
    }

    pub fn select_prev(&mut self, max: usize) {
        if max > 0 {
            self.selected_index = if self.selected_index == 0 {
                max - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    // ── feedback ────────────────────────────────────────────────────────────

    pub fn set_info(&mut self, msg: impl Into<String>) {
        self.feedback = Feedback::Info(msg.into());
        self.feedback_ttl = 30; // ~6 seconds at 200ms tick
    }

    pub fn set_success(&mut self, msg: impl Into<String>) {
        self.feedback = Feedback::Success(msg.into());
        self.feedback_ttl = 30;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.feedback = Feedback::Error(msg.into());
        self.feedback_ttl = 60; // errors stay longer
    }

    pub fn clear_feedback(&mut self) {
        self.feedback = Feedback::None;
        self.feedback_ttl = 0;
    }

    /// Called every tick; decrements TTL and clears when expired.
    pub fn tick_feedback(&mut self) {
        if self.feedback_ttl > 0 {
            self.feedback_ttl -= 1;
            if self.feedback_ttl == 0 {
                self.feedback = Feedback::None;
            }
        }
    }

    // ── loading spinner ─────────────────────────────────────────────────────

    pub fn start_loading(&mut self) {
        self.is_loading = true;
        self.spinner_frame = 0;
    }

    pub fn stop_loading(&mut self) {
        self.is_loading = false;
    }

    pub fn advance_spinner(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    /// The classic braille spinner characters.
    pub fn spinner_char(&self) -> char {
        const FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        FRAMES[self.spinner_frame as usize % FRAMES.len()]
    }
}

// ── Worker task helpers ──────────────────────────────────────────────────────
//
// Each `spawn_*` function clones the `EventSender`, moves it into a
// tokio task, runs the blocking core logic on a blocking thread, and
// sends the result back through the channel.

/// Spawn a background task that runs `f` on a blocking thread and
/// sends the result back as `AppEvent::WorkerComplete` or
/// `AppEvent::WorkerError`.
fn spawn_worker<F, T>(tx: &EventSender, kind: WorkerKind, f: F)
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Into<WorkerPayload> + Send + 'static,
{
    let tx = tx.clone();
    tokio::task::spawn_blocking(move || match f() {
        Ok(payload) => {
            let _ = tx.send(AppEvent::WorkerComplete(WorkerResult {
                kind,
                payload: payload.into(),
            }));
        }
        Err(e) => {
            let _ = tx.send(AppEvent::WorkerError(e.to_string()));
        }
    });
}

// ── Convenience conversions ──────────────────────────────────────────────────

impl From<()> for WorkerPayload {
    fn from(_: ()) -> Self {
        WorkerPayload::Unit
    }
}

impl From<String> for WorkerPayload {
    fn from(s: String) -> Self {
        WorkerPayload::Text(s)
    }
}

impl From<JsonValue> for WorkerPayload {
    fn from(v: JsonValue) -> Self {
        WorkerPayload::Json(v)
    }
}

// ── Action dispatchers ───────────────────────────────────────────────────────
//
// These are called from `main.rs` when the user presses a key.
// They clone data out of `AppState`, spawn a worker, and set
// `is_loading = true`.

impl AppState {
    /// Refresh the cached registered-app list (blocking, fast).
    pub fn refresh_apps(&mut self, registry: &AppRegistry) {
        match registry.list_apps() {
            Ok(apps) => {
                self.registered_app_count = apps.len();
                self.library_apps = apps;
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Refresh the cached logs list.
    pub fn refresh_logs(&mut self, registry: &AppRegistry) {
        match list_logs(registry) {
            Ok(logs) => self.logs_list = logs,
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Load manifest for the currently selected app.
    pub fn load_selected_manifest(&mut self, registry: &AppRegistry) {
        if let Some(app_id) = &self.selected_app_id {
            match registry.load_manifest(app_id) {
                Ok(m) => {
                    let desktop_path = registry.paths().desktop_entry_path(app_id);
                    self.desktop_exists = desktop_path.exists();
                    self.manifest = Some(m);
                }
                Err(e) => self.set_error(e.to_string()),
            }
        }
    }

    /// Load all compatibility profiles from disk into `self.profiles`.
    ///
    /// Initialises a `ProfileManager`, lists every stored `app_id`, and
    /// loads each profile.  Individual load failures are logged as warnings
    /// but **do not** abort the entire load — the TUI stays usable even if
    /// some profile files are corrupt or missing.
    pub fn load_profiles(&mut self) {
        let manager = match ProfileManager::new() {
            Ok(m) => m,
            Err(e) => {
                self.set_error(format!("profile manager init failed: {e}"));
                return;
            }
        };

        let ids = match manager.list_profiles() {
            Ok(ids) => ids,
            Err(e) => {
                self.set_error(format!("failed to list profiles: {e}"));
                return;
            }
        };

        self.profiles.clear();
        self.selected_profile_index = 0;

        for id in &ids {
            match manager.load_profile(id) {
                Ok(profile) => self.profiles.push(profile),
                Err(e) => {
                    // Non-fatal: log and continue loading the rest.
                    eprintln!("[openntx] skipping profile '{id}': {e}");
                }
            }
        }
    }

    /// Return the count relevant to the current mode (for selection clamping).
    #[allow(dead_code)]
    pub fn current_list_len(&self) -> usize {
        match self.mode {
            AppMode::Library => self.library_apps.len(),
            AppMode::Logs => self.logs_list.len(),
            AppMode::Capture => self.library_apps.len(),
            AppMode::Package => self.library_apps.len(),
            AppMode::Doctor => 2,        // global, per-app
            AppMode::CaptureAction => 6, // snapshot-before, after, diff, report, status, clean
            _ => 0,
        }
    }
}

// ── Background action launchers ──────────────────────────────────────────────

impl AppState {
    /// Run PE analysis + manifest preview in the background.
    #[allow(dead_code)]
    pub fn spawn_analyze(&mut self, tx: &EventSender, path: PathBuf) {
        self.start_loading();
        spawn_worker(tx, WorkerKind::Analyze, move || {
            let analysis = analyze_pe(&path)?;
            if !analysis.is_pe {
                return Err(OpenNtxError::Unsupported(format!(
                    "input must be a Windows PE/EXE; {} is {}",
                    analysis.file_name, analysis.status
                )));
            }
            let display_name = path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("windows-app");
            let app_id = openntx_core::app_id::generate_app_id(
                display_name,
                Some(&path.display().to_string()),
            );
            let generated = generate_manifest_from_pe(ManifestGenerationInput {
                input_path: &path,
                analysis: &analysis,
                app_id,
            })?;
            let json = serde_json::to_value(&generated.manifest)?;
            Ok(json)
        });
    }

    /// Run global doctor in the background.
    pub fn spawn_doctor_global(&mut self, tx: &EventSender, registry: &AppRegistry) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::DoctorGlobal, move || {
            let report = global_doctor(&registry)?;
            Ok(serde_json::to_value(&report)?)
        });
    }

    /// Run per-app doctor in the background.
    pub fn spawn_doctor_app(&mut self, tx: &EventSender, registry: &AppRegistry, app_id: String) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::DoctorApp, move || {
            let report = app_doctor(&registry, &app_id)?;
            Ok(serde_json::to_value(&report)?)
        });
    }

    /// Run doctor repair (dry-run) in the background.
    #[allow(dead_code)]
    pub fn spawn_doctor_repair(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::DoctorRepair, move || {
            let plan = repair_app(&registry, &app_id, true)?;
            Ok(serde_json::to_value(&plan)?)
        });
    }

    /// Build a .deb package in the background.
    pub fn spawn_package_build(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
        version: String,
    ) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::PackageBuild, move || {
            let mut opts = DebBuildOptions::new(&app_id);
            opts.dry_run = false;
            opts.version = version;
            let plan = build_deb_package(&registry, &opts)?;
            Ok(serde_json::to_value(&plan)?)
        });
    }

    /// Create a desktop launcher in the background.
    pub fn spawn_create_desktop(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::CreateDesktop, move || {
            let result = registry.create_desktop_entry(&app_id, DesktopMode::Write, "openntx")?;
            Ok(serde_json::json!({
                "app_id": result.app_id,
                "path": result.desktop_entry_path.display().to_string(),
                "written": result.written,
            }))
        });
    }

    /// Remove a desktop launcher in the background.
    pub fn spawn_remove_desktop(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::RemoveDesktop, move || {
            let result = registry.remove_desktop_entry(&app_id, DesktopMode::Write)?;
            Ok(serde_json::json!({
                "app_id": result.app_id,
                "removed": result.removed,
            }))
        });
    }

    /// Run capture snapshot-before in the background.
    pub fn spawn_snapshot_before(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let service = CaptureRegistryService::new(AppRegistry::new(registry.paths().clone()));
        spawn_worker(tx, WorkerKind::SnapshotBefore, move || {
            let result = service.snapshot_before(&app_id)?;
            Ok(serde_json::json!({
                "snapshot_path": result.snapshot_path.display().to_string(),
                "entries": result.snapshot.entries.len(),
            }))
        });
    }

    /// Run capture snapshot-after in the background.
    pub fn spawn_snapshot_after(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let service = CaptureRegistryService::new(AppRegistry::new(registry.paths().clone()));
        spawn_worker(tx, WorkerKind::SnapshotAfter, move || {
            let result = service.snapshot_after(&app_id)?;
            Ok(serde_json::json!({
                "snapshot_path": result.snapshot_path.display().to_string(),
                "entries": result.snapshot.entries.len(),
            }))
        });
    }

    /// Run capture diff in the background.
    pub fn spawn_diff(&mut self, tx: &EventSender, registry: &AppRegistry, app_id: String) {
        self.start_loading();
        let service = CaptureRegistryService::new(AppRegistry::new(registry.paths().clone()));
        spawn_worker(tx, WorkerKind::Diff, move || {
            let result = service.diff(&app_id)?;
            Ok(serde_json::to_value(&result.diff)?)
        });
    }

    /// Run capture report in the background.
    pub fn spawn_report(&mut self, tx: &EventSender, registry: &AppRegistry, app_id: String) {
        self.start_loading();
        let service = CaptureRegistryService::new(AppRegistry::new(registry.paths().clone()));
        spawn_worker(tx, WorkerKind::Report, move || {
            let result = service.report(&app_id)?;
            Ok(serde_json::to_value(&result.report)?)
        });
    }

    /// Run capture status in the background.
    pub fn spawn_capture_status(
        &mut self,
        tx: &EventSender,
        registry: &AppRegistry,
        app_id: String,
    ) {
        self.start_loading();
        let service = CaptureRegistryService::new(AppRegistry::new(registry.paths().clone()));
        spawn_worker(tx, WorkerKind::CaptureStatus, move || {
            let status = service.status(&app_id)?;
            Ok(serde_json::json!({
                "app_id": status.app_id,
                "app_name": status.app_name,
                "snapshot_before": status.snapshot_before,
                "snapshot_after": status.snapshot_after,
                "diff": status.diff,
                "report": status.report,
            }))
        });
    }

    /// Run plan in the background.
    pub fn spawn_run_plan(&mut self, tx: &EventSender, registry: &AppRegistry, app_id: String) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::RunPlan, move || {
            let report =
                create_registered_run_plan(&registry, &app_id, &RunPlanOptions::default())?;
            Ok(serde_json::to_value(&report)?)
        });
    }

    /// Remove an app in the background (with confirmation already handled by UI).
    #[allow(dead_code)]
    pub fn spawn_remove_app(&mut self, tx: &EventSender, registry: &AppRegistry, app_id: String) {
        self.start_loading();
        let registry = AppRegistry::new(registry.paths().clone());
        spawn_worker(tx, WorkerKind::RemoveApp, move || {
            let plan = registry.remove_app(&app_id, RemoveMode::Delete)?;
            Ok(serde_json::json!({
                "app_id": plan.app_id,
                "removed": plan.removed,
                "app_dir": plan.app_dir.display().to_string(),
            }))
        });
    }
}

// ── Worker result handler ────────────────────────────────────────────────────

impl AppState {
    /// Called from `main.rs` when `AppEvent::WorkerComplete` arrives.
    pub fn handle_worker_result(&mut self, result: WorkerResult) {
        self.stop_loading();
        match result.kind {
            WorkerKind::Analyze => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_doctor_json = Some(v);
                    self.push_mode(AppMode::Analyze);
                    self.set_success("Analysis complete.");
                }
                _ => self.set_error("Unexpected analysis result."),
            },
            WorkerKind::DoctorGlobal => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_doctor_json = Some(v);
                    self.push_mode(AppMode::DoctorResult);
                }
                _ => self.set_error("Unexpected doctor result."),
            },
            WorkerKind::DoctorApp => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_doctor_json = Some(v);
                    self.push_mode(AppMode::DoctorResult);
                }
                _ => self.set_error("Unexpected doctor result."),
            },
            WorkerKind::DoctorRepair => {
                self.set_success("Repair dry-run complete.");
            }
            WorkerKind::PackageBuild => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_package_json = Some(v);
                    self.set_success("Package built successfully.");
                }
                _ => self.set_error("Unexpected package result."),
            },
            WorkerKind::CreateDesktop => {
                self.set_success("Desktop launcher created.");
            }
            WorkerKind::RemoveDesktop => {
                self.set_success("Desktop launcher removed.");
            }
            WorkerKind::SnapshotBefore
            | WorkerKind::SnapshotAfter
            | WorkerKind::Diff
            | WorkerKind::Report
            | WorkerKind::CaptureStatus => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_capture_json = Some(v);
                    self.set_success(format!("{:?} complete.", result.kind));
                }
                _ => self.set_error("Unexpected capture result."),
            },
            WorkerKind::RunPlan => match result.payload {
                WorkerPayload::Json(v) => {
                    self.last_run_plan = serde_json::from_value(v).ok();
                    self.set_success("Run plan generated.");
                }
                _ => self.set_error("Unexpected run-plan result."),
            },
            WorkerKind::RemoveApp => {
                self.set_success("App removed.");
            }
            _ => {
                self.set_info("Action completed.");
            }
        }
    }

    /// Called from `main.rs` when `AppEvent::WorkerError` arrives.
    pub fn handle_worker_error(&mut self, msg: String) {
        self.stop_loading();
        self.set_error(msg);
    }

    /// Called from `main.rs` when `AppEvent::IpcCaptureStatus` arrives.
    ///
    /// Updates the live capture status string.  If the status is `"Complete"`,
    /// the status is cleared after a short delay (on the next tick cycle).
    pub fn handle_ipc_capture_status(&mut self, status_json: String) {
        // Parse to extract human-readable info.
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&status_json) {
            let app_id = msg
                .get("app_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = msg
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let files = msg
                .get("files_tracked")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if status == "Complete" {
                self.live_capture_status = Some(format!(
                    "CAPTURE COMPLETE: {} ({} files tracked)",
                    app_id, files
                ));
                // Auto-clear after a few ticks.
                self.feedback_ttl = 20;
            } else {
                self.live_capture_status = Some(format!(
                    "{}: {} — {} files tracked",
                    status, app_id, files
                ));
            }
        } else {
            // Fallback: store raw string.
            self.live_capture_status = Some(status_json);
        }
    }

    /// Called every tick; clears live_capture_status when the TTL expires.
    pub fn tick_live_capture(&mut self) {
        // If status contains "COMPLETE", count down and clear.
        if let Some(ref s) = self.live_capture_status {
            if s.contains("COMPLETE") && self.feedback_ttl > 0 {
                self.feedback_ttl -= 1;
                if self.feedback_ttl == 0 {
                    self.live_capture_status = None;
                }
            }
        }
    }
}
