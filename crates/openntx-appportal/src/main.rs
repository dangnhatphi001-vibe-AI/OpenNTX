// main.rs — Async entry point for the OpenNTX AppPortal TUI.
//
// Responsibilities:
//   1. Install a panic hook that restores the terminal before printing the
//      stack trace, so a crash never leaves the shell in raw-mode / alt-screen.
//   2. Initialise crossterm terminal (raw mode, alternate screen, mouse capture).
//   3. Create an `EventHandler` which spawns a dedicated OS thread to poll
//      crossterm input (no tokio blocking).
//   4. Run the main loop via `run_app()`:
//        a. Draw the current UI frame via `ui::draw()`.
//        b. Receive the next `AppEvent` from the channel (async, non-blocking).
//        c. Update `AppState` (navigate, spawn workers, consume results).
//   5. On exit (normal or panic): restore terminal state (disable raw mode,
//      leave alternate screen, disable mouse capture, show cursor).

mod app;
mod events;
mod ui;

use anyhow::{Context, Result};
use app::{AppMode, AppState};
use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{AppEvent, EventHandler, EventSender};
use openntx_core::logs::show_log;
use openntx_core::registry::AppRegistry;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::panic;

// ── Terminal teardown helper ─────────────────────────────────────────────────
//
// Called from both the normal exit path (via `Drop` guard) and from the
// custom panic hook.  Idempotent — safe to call more than once.

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
}

/// RAII guard that restores the terminal on drop (normal exit **or** panic).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // ── 1. Panic hook ────────────────────────────────────────────────────────
    //
    // If *any* thread panics we must restore the terminal **before** the
    // default hook prints the backtrace.  Otherwise the backtrace is written
    // while the terminal is still in raw / alt-screen mode, which garbles the
    // output and may leave the shell unusable ("frozen terminal").
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restore — ignore I/O errors.
        restore_terminal();
        // Delegate to the default hook so the backtrace still prints.
        original_hook(panic_info);
    }));

    // ── 2. Terminal setup ────────────────────────────────────────────────────
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("failed to enter alternate screen / enable mouse capture")?;

    // Guard ensures teardown on *any* exit path (return, ?, or panic).
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
    terminal.clear().context("failed to clear terminal")?;

    // ── 3. Application state ─────────────────────────────────────────────────
    let registry = AppRegistry::from_env().context("failed to initialise app registry")?;
    let mut app = AppState::new();
    app.refresh_apps(&registry);
    app.load_profiles();

    // ── 4. Event system (dedicated OS thread, NOT tokio::spawn) ───────────────
    let events = EventHandler::new();

    // ── 5. Run the async application loop ────────────────────────────────────
    let run_result = run_app(&mut terminal, &mut app, &registry, events).await;

    // ── 6. Cleanup ───────────────────────────────────────────────────────────
    // `_guard` drops here → `restore_terminal()` is called.
    // `events` (and its sender clones) drop → OS reader thread exits.

    run_result
}

// ── Async main loop ──────────────────────────────────────────────────────────
//
// Extracted into its own async fn so `main()` can own the terminal setup /
// teardown lifecycle cleanly.  Returns `Ok(())` on graceful exit or an error
// if the terminal draw / event channel fails.

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    registry: &AppRegistry,
    mut events: EventHandler,
) -> Result<()> {
    // Clone the sender so `handle_key_event` can spawn background tasks
    // that deliver `WorkerComplete` / `WorkerError` back through the
    // same channel.
    let worker_tx: EventSender = events.sender();

    loop {
        // ── Draw ─────────────────────────────────────────────────────────────
        terminal.draw(|frame| ui::draw(frame, app))?;

        // ── Receive next event (async — yields while waiting) ─────────────────
        let Some(event) = events.next().await else {
            // All senders dropped (reader thread exited + workers done).
            break;
        };

        // ── Dispatch ─────────────────────────────────────────────────────────
        match event {
            AppEvent::Input(key) => {
                handle_key_event(app, registry, &worker_tx, key);
            }

            AppEvent::Tick => {
                app.tick_feedback();
                if app.is_loading {
                    app.advance_spinner();
                }
            }

            AppEvent::WorkerComplete(result) => {
                app.handle_worker_result(result);
            }

            AppEvent::WorkerError(msg) => {
                app.handle_worker_error(msg);
            }

            AppEvent::Quit => {
                app.should_quit = true;
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

// ── Key dispatch ─────────────────────────────────────────────────────────────

fn handle_key_event(app: &mut AppState, registry: &AppRegistry, tx: &EventSender, key: KeyEvent) {
    // Ctrl-C always quits.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Global: 'q' from Home quits.
    if key.code == KeyCode::Char('q') && app.mode == AppMode::Home {
        app.should_quit = true;
        return;
    }

    // Navigation: Escape / Backspace goes back.
    if matches!(key.code, KeyCode::Esc | KeyCode::Backspace) {
        app.pop_mode();
        app.refresh_apps(registry);
        return;
    }

    match app.mode {
        AppMode::Home => handle_home_key(app, registry, tx, key),
        AppMode::Library => handle_library_key(app, registry, tx, key),
        AppMode::AppDetails => handle_details_key(app, registry, tx, key),
        AppMode::Capture => handle_capture_key(app, registry, tx, key),
        AppMode::CaptureAction => handle_capture_action_key(app, registry, tx, key),
        AppMode::Logs => handle_logs_key(app, registry, tx, key),
        AppMode::Doctor => handle_doctor_key(app, registry, tx, key),
        AppMode::DoctorResult => {
            /* any key goes back */
            app.pop_mode();
        }
        AppMode::Analyze => {
            app.pop_mode();
        }
        AppMode::InstallPlan => {
            app.pop_mode();
        }
        AppMode::Package => handle_package_key(app, registry, tx, key),
        AppMode::Settings => {
            app.pop_mode();
        }
    }
}

fn handle_home_key(app: &mut AppState, registry: &AppRegistry, _tx: &EventSender, key: KeyEvent) {
    match key.code {
        KeyCode::Char('1') | KeyCode::Char('l') => {
            app.refresh_apps(registry);
            app.push_mode(AppMode::Library);
        }
        KeyCode::Char('2') | KeyCode::Char('a') => {
            app.set_info("Enter a PE/EXE path in the prompt below (TODO: input mode).");
        }
        KeyCode::Char('3') | KeyCode::Char('i') => {
            app.set_info("Enter a PE/EXE path to generate an install plan (TODO: input mode).");
        }
        KeyCode::Char('4') | KeyCode::Char('c') => {
            app.refresh_apps(registry);
            app.push_mode(AppMode::Capture);
        }
        KeyCode::Char('5') | KeyCode::Char('p') => {
            app.refresh_apps(registry);
            app.push_mode(AppMode::Package);
        }
        KeyCode::Char('6') => {
            app.refresh_logs(registry);
            app.push_mode(AppMode::Logs);
        }
        KeyCode::Char('7') | KeyCode::Char('d') => {
            app.push_mode(AppMode::Doctor);
        }
        KeyCode::Char('8') | KeyCode::Char('s') => {
            app.push_mode(AppMode::Settings);
        }
        _ => {}
    }
}

fn handle_library_key(
    app: &mut AppState,
    registry: &AppRegistry,
    _tx: &EventSender,
    key: KeyEvent,
) {
    let max = app.library_apps.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(max),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(max),
        KeyCode::Enter => {
            if let Some(entry) = app.library_apps.get(app.selected_index) {
                app.selected_app_id = Some(entry.app_id.clone());
                app.load_selected_manifest(registry);
                app.push_mode(AppMode::AppDetails);
            }
        }
        _ => {}
    }
}

fn handle_details_key(app: &mut AppState, registry: &AppRegistry, tx: &EventSender, key: KeyEvent) {
    let Some(app_id) = app.selected_app_id.clone() else {
        app.pop_mode();
        return;
    };
    match key.code {
        KeyCode::Char('r') => {
            app.spawn_run_plan(tx, registry, app_id);
        }
        KeyCode::Char('c') => {
            app.spawn_create_desktop(tx, registry, app_id);
        }
        KeyCode::Char('x') => {
            app.spawn_remove_desktop(tx, registry, app_id);
        }
        KeyCode::Char('1') => {
            app.spawn_snapshot_before(tx, registry, app_id);
        }
        KeyCode::Char('2') => {
            app.spawn_snapshot_after(tx, registry, app_id);
        }
        KeyCode::Char('3') => {
            app.spawn_diff(tx, registry, app_id);
        }
        KeyCode::Char('4') => {
            app.spawn_report(tx, registry, app_id);
        }
        KeyCode::Char('5') => {
            app.spawn_capture_status(tx, registry, app_id);
        }
        KeyCode::Char('p') => {
            let version = "1.0.0-alpha".to_string();
            app.spawn_package_build(tx, registry, app_id, version);
        }
        KeyCode::Char('l') => {
            // Show logs for this app (navigate to logs filtered).
            app.refresh_logs(registry);
            app.push_mode(AppMode::Logs);
        }
        KeyCode::Char('g') => {
            app.spawn_doctor_app(tx, registry, app_id);
        }
        _ => {}
    }
}

fn handle_capture_key(
    app: &mut AppState,
    _registry: &AppRegistry,
    _tx: &EventSender,
    key: KeyEvent,
) {
    let max = app.library_apps.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(max),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(max),
        KeyCode::Enter => {
            if let Some(entry) = app.library_apps.get(app.selected_index) {
                app.selected_app_id = Some(entry.app_id.clone());
                app.capture_action_index = 0;
                app.push_mode(AppMode::CaptureAction);
            }
        }
        _ => {}
    }
}

fn handle_capture_action_key(
    app: &mut AppState,
    registry: &AppRegistry,
    tx: &EventSender,
    key: KeyEvent,
) {
    let max = 6usize;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(max),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(max),
        KeyCode::Enter => {
            let Some(app_id) = app.selected_app_id.clone() else {
                return;
            };
            match app.selected_index {
                0 => app.spawn_snapshot_before(tx, registry, app_id),
                1 => app.spawn_snapshot_after(tx, registry, app_id),
                2 => app.spawn_diff(tx, registry, app_id),
                3 => app.spawn_report(tx, registry, app_id),
                4 => app.spawn_capture_status(tx, registry, app_id),
                5 => {
                    // Clean — pop back, user confirmed
                    app.pop_mode();
                    app.set_info("Capture clean requested.");
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_logs_key(app: &mut AppState, registry: &AppRegistry, tx: &EventSender, key: KeyEvent) {
    let max = app.logs_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(max),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(max),
        KeyCode::Enter => {
            if let Some(log) = app.logs_list.get(app.selected_index) {
                let path = log.file_path.clone();
                let registry = AppRegistry::new(registry.paths().clone());
                let tx2 = tx.clone();
                app.start_loading();
                tokio::task::spawn_blocking(move || {
                    let result = show_log(&registry, &path);
                    match result {
                        Ok(report) => {
                            let json = serde_json::to_value(&report).unwrap_or_default();
                            let _ = tx2.send(AppEvent::WorkerComplete(events::WorkerResult {
                                kind: events::WorkerKind::LogShow,
                                payload: events::WorkerPayload::Json(json),
                            }));
                        }
                        Err(e) => {
                            let _ = tx2.send(AppEvent::WorkerError(e.to_string()));
                        }
                    }
                });
            }
        }
        _ => {}
    }
}

fn handle_doctor_key(app: &mut AppState, registry: &AppRegistry, tx: &EventSender, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(2),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(2),
        KeyCode::Enter => {
            match app.selected_index {
                0 => app.spawn_doctor_global(tx, registry),
                1 => {
                    // Per-app doctor — need to select an app.
                    // For now, if we have a selected app, use it.
                    if let Some(app_id) = app.selected_app_id.clone() {
                        app.spawn_doctor_app(tx, registry, app_id);
                    } else if let Some(first) = app.library_apps.first() {
                        let app_id = first.app_id.clone();
                        app.selected_app_id = Some(app_id.clone());
                        app.spawn_doctor_app(tx, registry, app_id);
                    } else {
                        app.set_error("No registered apps.");
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_package_key(app: &mut AppState, registry: &AppRegistry, tx: &EventSender, key: KeyEvent) {
    let max = app.library_apps.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(max),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(max),
        KeyCode::Enter => {
            if let Some(entry) = app.library_apps.get(app.selected_index) {
                let app_id = entry.app_id.clone();
                let version = "1.0.0-alpha".to_string();
                app.spawn_package_build(tx, registry, app_id, version);
            }
        }
        _ => {}
    }
}


