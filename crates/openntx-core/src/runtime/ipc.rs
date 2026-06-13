// openntx-core/src/runtime/ipc.rs — Unix Domain Socket IPC for live status.
//
// Provides a `RuntimeIpcServer` that listens on a Unix Domain Socket (UDS)
// for JSON status messages from the runtime's capture process.  Messages are
// forwarded through a `std::sync::mpsc` channel to the caller (typically the
// TUI AppPortal).
//
// Also provides `RuntimeIpcClient` for the runtime entrypoint to send status
// updates during the Auto-Fallback Capture flow.
//
// Protocol:
//   - Each message is a single line of JSON terminated by `\n`.
//   - Message format: `{"app_id":"...","status":"Capturing","files_tracked":42}`
//
// Socket default path: `/tmp/openntx_runtime.sock`

use crate::{OpenNtxError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

/// Default path for the Unix Domain Socket.
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/openntx_runtime.sock";

// ── Status message ───────────────────────────────────────────────────────────

/// A single status update sent from the runtime capture process to the TUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStatusMessage {
    /// Application identifier (from SHA-256 hash of the PE file).
    pub app_id: String,
    /// Current status phase (e.g. `"Capturing"`, `"Executing"`, `"Complete"`).
    pub status: String,
    /// Number of filesystem events tracked so far.
    pub files_tracked: u32,
}

impl CaptureStatusMessage {
    /// Serialize to a JSON line (terminated with `\n`).
    pub fn to_json_line(&self) -> Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(format!("{}\n", json))
    }

    /// Deserialize from a JSON line (with or without trailing newline).
    pub fn from_json_line(line: &str) -> Result<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(OpenNtxError::InvalidInput(
                "empty IPC message".to_string(),
            ));
        }
        let msg: Self = serde_json::from_str(trimmed)?;
        Ok(msg)
    }
}

// ── IPC Server ───────────────────────────────────────────────────────────────

/// Unix Domain Socket server that receives `CaptureStatusMessage`s from the
/// runtime and forwards them through a `mpsc::Receiver`.
///
/// # Lifecycle
///
/// 1. Call [`RuntimeIpcServer::start`] to bind the socket and spawn the
///    listener thread.
/// 2. Receive messages from the returned `Receiver<CaptureStatusMessage>`.
/// 3. Drop the `RuntimeIpcServer` to remove the socket file and signal the
///    listener thread to exit.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::ipc::{RuntimeIpcServer, CaptureStatusMessage};
///
/// let (server, rx) = RuntimeIpcServer::start(None).expect("start IPC server");
///
/// // In another thread / process, a client sends a message.
/// // Here we just receive:
/// if let Ok(msg) = rx.recv() {
///     println!("status: {} — files tracked: {}", msg.status, msg.files_tracked);
/// }
/// ```
pub struct RuntimeIpcServer {
    socket_path: PathBuf,
}

impl RuntimeIpcServer {
    /// Start the IPC server.
    ///
    /// Binds to `socket_path` (or `DEFAULT_SOCKET_PATH` if `None`), removes
    /// any stale socket file, and spawns a background OS thread that accepts
    /// connections and reads JSON-line messages.
    ///
    /// Returns `(server_handle, receiver)`.  Dropping the handle cleans up
    /// the socket file.  The receiver yields `CaptureStatusMessage`s.
    pub fn start(
        socket_path: Option<&Path>,
    ) -> Result<(Self, mpsc::Receiver<CaptureStatusMessage>)> {
        let path = socket_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));

        // Remove stale socket file if it exists.
        if path.exists() {
            fs::remove_file(&path).map_err(|source| OpenNtxError::io(&path, source))?;
        }

        let listener =
            UnixListener::bind(&path).map_err(|source| OpenNtxError::io(&path, source))?;

        // Set the listener to non-blocking so the thread can check for
        // shutdown between accept() calls.
        listener
            .set_nonblocking(true)
            .map_err(|source| OpenNtxError::io(&path, source))?;

        let (tx, rx) = mpsc::channel();
        let path_for_thread = path.clone();

        thread::Builder::new()
            .name("openntx-ipc-server".into())
            .spawn(move || {
                Self::accept_loop(listener, tx, &path_for_thread);
            })
            .map_err(|source| OpenNtxError::io(&path, source))?;

        let server = Self { socket_path: path };
        Ok((server, rx))
    }

    /// Return the socket path this server is listening on.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Main accept loop — runs on the background thread.
    ///
    /// Accepts incoming connections, reads JSON-line messages, and forwards
    /// them through `tx`.  Exits when:
    /// - `tx.send()` fails (receiver dropped), or
    /// - The socket file is removed (checked between accepts).
    fn accept_loop(listener: UnixListener, tx: mpsc::Sender<CaptureStatusMessage>, path: &Path) {
        loop {
            // Check if the socket is still alive (cleanup signal).
            if !path.exists() {
                return;
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(e) = Self::handle_connection(stream, &tx) {
                        eprintln!("[openntx-ipc] connection error: {e}");
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connections — sleep before next poll.
                    // Also serves as the shutdown check window.
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(e) => {
                    eprintln!("[openntx-ipc] accept error: {e}");
                    return;
                }
            }
        }
    }

    /// Handle a single client connection.
    ///
    /// Reads JSON-line messages until the client disconnects or an error
    /// occurs.
    fn handle_connection(
        stream: UnixStream,
        tx: &mpsc::Sender<CaptureStatusMessage>,
    ) -> Result<()> {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line.map_err(|source| {
                OpenNtxError::io("ipc-connection", source)
            })?;

            let msg = CaptureStatusMessage::from_json_line(&line)?;
            if tx.send(msg).is_err() {
                return Ok(()); // receiver dropped
            }
        }
        Ok(())
    }
}

impl Drop for RuntimeIpcServer {
    fn drop(&mut self) {
        // Best-effort cleanup — ignore errors.
        let _ = fs::remove_file(&self.socket_path);
    }
}

// ── IPC Client ───────────────────────────────────────────────────────────────

/// Lightweight client that sends `CaptureStatusMessage`s to the IPC server.
///
/// Connects to the Unix Domain Socket for each message send (short-lived
/// connections, consistent with the one-message-per-event pattern).
pub struct RuntimeIpcClient {
    socket_path: PathBuf,
}

impl RuntimeIpcClient {
    /// Create a new client targeting the given socket path.
    pub fn new(socket_path: Option<&Path>) -> Self {
        let path = socket_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));
        Self { socket_path: path }
    }

    /// Send a single status message to the IPC server.
    ///
    /// Opens a new connection, writes the JSON line, and closes.
    /// Non-blocking: if the server is not running, the error is returned
    /// but does **not** panic.
    pub fn send_status(&self, message: &CaptureStatusMessage) -> Result<()> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|source| {
            OpenNtxError::io(&self.socket_path, source)
        })?;

        let json_line = message.to_json_line()?;
        stream
            .write_all(json_line.as_bytes())
            .map_err(|source| OpenNtxError::io(&self.socket_path, source))?;

        stream
            .flush()
            .map_err(|source| OpenNtxError::io(&self.socket_path, source))?;

        Ok(())
    }

    /// Try to send a status message, silently ignoring errors.
    ///
    /// Useful for fire-and-forget status updates where a missing server
    /// should not abort the capture flow.
    pub fn try_send_status(&self, message: &CaptureStatusMessage) {
        let _ = self.send_status(message);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_socket_path() -> PathBuf {
        let dir = tempfile::tempdir().expect("temp dir");
        // Leak the dir so the socket file persists for the test.
        let path = dir.path().join("test-openntx.sock");
        // Keep dir alive by leaking — we clean up the socket manually.
        std::mem::forget(dir);
        path
    }

    #[test]
    fn capture_status_message_round_trip() {
        let msg = CaptureStatusMessage {
            app_id: "test-app-1234".to_string(),
            status: "Capturing".to_string(),
            files_tracked: 42,
        };

        let json_line = msg.to_json_line().unwrap();
        assert!(json_line.ends_with('\n'), "must be newline-terminated");

        let parsed = CaptureStatusMessage::from_json_line(&json_line).unwrap();
        assert_eq!(parsed.app_id, "test-app-1234");
        assert_eq!(parsed.status, "Capturing");
        assert_eq!(parsed.files_tracked, 42);
    }

    #[test]
    fn capture_status_message_from_json_without_newline() {
        let json = r#"{"app_id":"abc","status":"Executing","files_tracked":0}"#;
        let msg = CaptureStatusMessage::from_json_line(json).unwrap();
        assert_eq!(msg.app_id, "abc");
        assert_eq!(msg.status, "Executing");
        assert_eq!(msg.files_tracked, 0);
    }

    #[test]
    fn capture_status_message_rejects_empty() {
        let result = CaptureStatusMessage::from_json_line("");
        assert!(result.is_err());
    }

    #[test]
    fn capture_status_message_rejects_invalid_json() {
        let result = CaptureStatusMessage::from_json_line("not json");
        assert!(result.is_err());
    }

    #[test]
    fn ipc_server_and_client_communicate() {
        let socket_path = temp_socket_path();
        let (server, rx) = RuntimeIpcServer::start(Some(&socket_path)).unwrap();

        // Give the server thread a moment to start listening.
        thread::sleep(std::time::Duration::from_millis(50));

        let client = RuntimeIpcClient::new(Some(&socket_path));

        let msg1 = CaptureStatusMessage {
            app_id: "app-1".to_string(),
            status: "Capturing".to_string(),
            files_tracked: 10,
        };
        client.send_status(&msg1).unwrap();

        let msg2 = CaptureStatusMessage {
            app_id: "app-1".to_string(),
            status: "Complete".to_string(),
            files_tracked: 25,
        };
        client.send_status(&msg2).unwrap();

        // Receive both messages.
        let received1 = rx.recv().unwrap();
        assert_eq!(received1.app_id, "app-1");
        assert_eq!(received1.status, "Capturing");
        assert_eq!(received1.files_tracked, 10);

        let received2 = rx.recv().unwrap();
        assert_eq!(received2.status, "Complete");
        assert_eq!(received2.files_tracked, 25);

        // Cleanup.
        drop(server);
        let _ = fs::remove_file(&socket_path);
    }

    #[test]
    fn ipc_client_try_send_ignores_missing_server() {
        let client = RuntimeIpcClient::new(Some(Path::new("/tmp/nonexistent-openntx-test.sock")));
        let msg = CaptureStatusMessage {
            app_id: "test".to_string(),
            status: "Capturing".to_string(),
            files_tracked: 0,
        };
        // Should not panic — errors are silently ignored.
        client.try_send_status(&msg);
    }

    #[test]
    fn ipc_server_cleans_up_socket_on_drop() {
        let socket_path = temp_socket_path();
        let (server, _rx) = RuntimeIpcServer::start(Some(&socket_path)).unwrap();
        assert!(socket_path.exists(), "socket should exist after start");

        drop(server);
        // Give the OS a moment to clean up.
        thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            !socket_path.exists(),
            "socket should be removed after server drop"
        );
    }

    #[test]
    fn default_socket_path_is_set() {
        assert_eq!(DEFAULT_SOCKET_PATH, "/tmp/openntx_runtime.sock");
    }

    #[test]
    fn capture_status_message_serialization() {
        let msg = CaptureStatusMessage {
            app_id: "my-app".to_string(),
            status: "Executing".to_string(),
            files_tracked: 7,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"app_id\":\"my-app\""));
        assert!(json.contains("\"status\":\"Executing\""));
        assert!(json.contains("\"files_tracked\":7"));
    }
}
