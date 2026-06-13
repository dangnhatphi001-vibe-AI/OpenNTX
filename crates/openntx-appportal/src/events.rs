// events.rs — Event system for the OpenNTX AppPortal TUI.
//
// Uses a **dedicated OS thread** (`std::thread::spawn`) to poll crossterm for
// terminal input.  This avoids blocking the tokio async executor, which was
// the root cause of the "frozen terminal" deadlock when `tokio::spawn` was
// used with crossterm's async `EventStream`.
//
// Architecture:
//   ┌──────────────────┐   unbounded_channel   ┌──────────────────┐
//   │  OS thread        │ ──────────────────────► │  tokio main loop │
//   │  crossterm::poll  │   AppEvent::Input/Tick  │  events.next()   │
//   └──────────────────┘                          └──────────────────┘
//
// Worker tasks (spawned via `tokio::task::spawn_blocking`) also send events
// through the same unbounded channel via the cloned `EventSender`.

use crossterm::event::{self, Event, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

/// Poll interval — doubles as the tick rate for spinner / loading animation.
const TICK_RATE: Duration = Duration::from_millis(250);

// ── Public event type ────────────────────────────────────────────────────────

/// Everything the main loop can react to.
#[derive(Debug)]
pub enum AppEvent {
    /// A key-press from the terminal.
    Input(KeyEvent),

    /// Periodic heartbeat.  Used for spinner animation and polling
    /// background tasks.
    Tick,

    /// A long-running worker task finished.
    WorkerComplete(WorkerResult),

    /// A worker task returned an error.
    WorkerError(String),

    /// Live capture status update from the runtime IPC server.
    IpcCaptureStatus(String),

    /// Request a clean shutdown.
    Quit,
}

/// Opaque payload returned by a completed background task.
#[derive(Debug)]
pub struct WorkerResult {
    pub kind: WorkerKind,
    pub payload: WorkerPayload,
}

/// Identifies which background task completed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WorkerKind {
    Analyze,
    InstallPlan,
    SnapshotBefore,
    SnapshotAfter,
    Diff,
    Report,
    CaptureStatus,
    DoctorGlobal,
    DoctorApp,
    DoctorRepair,
    PackageBuild,
    PackageInspect,
    PackageClean,
    LogsList,
    LogShow,
    LogsClean,
    Rename,
    Duplicate,
    Export,
    Import,
    RemoveApp,
    CreateDesktop,
    RemoveDesktop,
    RunPlan,
}

/// The data a worker can send back.
#[derive(Debug)]
#[allow(dead_code)]
pub enum WorkerPayload {
    /// No meaningful data (fire-and-forget action).
    Unit,
    /// A JSON-serializable value.
    Json(serde_json::Value),
    /// A plain-text message.
    Text(String),
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Sender half of the event channel.
///
/// Used by worker tasks (via `clone()`) to deliver results back to the
/// main loop.  Also held internally by `EventHandler` for the reader thread.
pub type EventSender = mpsc::UnboundedSender<AppEvent>;

/// Owns the receiver half of the event channel and manages the dedicated
/// OS thread that reads crossterm input.
///
/// # Usage
/// ```ignore
/// let mut events = EventHandler::new();
/// let worker_tx = events.sender();   // clone for background tasks
///
/// loop {
///     terminal.draw(|f| ui::draw(f, &app))?;
///     match events.next().await {
///         Some(AppEvent::Input(key)) => { /* … */ }
///         Some(AppEvent::Tick)       => { /* … */ }
///         None                       => break,
///     }
/// }
/// ```
pub struct EventHandler {
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    sender: EventSender,
}

impl EventHandler {
    /// Create a new `EventHandler`.
    ///
    /// Spawns a dedicated OS thread that polls `crossterm::event::poll`
    /// every 250 ms.  Key events are forwarded as `AppEvent::Input(key)`;
    /// poll timeouts emit `AppEvent::Tick`.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<AppEvent>();
        let tx = sender.clone();

        std::thread::Builder::new()
            .name("openntx-crossterm-reader".into())
            .spawn(move || {
                Self::reader_loop(tx);
            })
            .expect("failed to spawn crossterm reader thread");

        Self { receiver, sender }
    }

    /// Async wait for the next event from the reader thread or a worker.
    ///
    /// Returns `None` only if all senders have been dropped (i.e. the
    /// reader thread exited **and** every worker clone was dropped).
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }

    /// Clone the sender so worker tasks can deliver `WorkerComplete` /
    /// `WorkerError` events back to the main loop.
    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

    // ── Private ──────────────────────────────────────────────────────────────

    /// Blocking reader loop — runs on the dedicated OS thread.
    ///
    /// Uses `crossterm::event::poll` (which internally calls `select`/
    /// `epoll`/`ReadConsoleInput` depending on the platform) so the
    /// thread sleeps efficiently between events and **never** holds a
    /// tokio runtime permit.
    fn reader_loop(tx: EventSender) {
        loop {
            // `poll` returns `Ok(true)` if an event is ready within the
            // timeout, `Ok(false)` on timeout, `Err` on I/O failure.
            match event::poll(TICK_RATE) {
                Ok(true) => {
                    // An event is ready — read it.
                    match event::read() {
                        Ok(Event::Key(key)) => {
                            // Forward every key event; the main loop
                            // decides what to do with it.
                            if tx.send(AppEvent::Input(key)).is_err() {
                                return; // receiver dropped
                            }
                        }
                        Ok(_) => {
                            // Non-key event (mouse, resize, focus, …).
                            // ratatui handles resize automatically on
                            // the next draw, and we ignore the rest.
                            // Send a Tick to trigger a prompt redraw
                            // (e.g. after terminal resize).
                            if tx.send(AppEvent::Tick).is_err() {
                                return;
                            }
                        }
                        Err(_) => {
                            // crossterm I/O error — request shutdown.
                            let _ = tx.send(AppEvent::Quit);
                            return;
                        }
                    }
                }
                Ok(false) => {
                    // Timeout — no input.  Emit a Tick for spinner /
                    // loading-state animation.
                    if tx.send(AppEvent::Tick).is_err() {
                        return; // receiver dropped
                    }
                }
                Err(_) => {
                    // `poll` itself failed — request shutdown.
                    let _ = tx.send(AppEvent::Quit);
                    return;
                }
            }
        }
    }
}
