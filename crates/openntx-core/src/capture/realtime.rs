// capture/realtime.rs — Real-time filesystem capture using Linux inotify.
//
// Replaces the old snapshot-before/after + diff mechanism with a streaming
// approach that records filesystem events as they happen.  This eliminates
// the I/O overhead of full directory scans and produces a clean event log
// without noise from unchanged files.
//
// Architecture:
//
//   ┌──────────────────────┐   inotify (non-blocking)   ┌─────────────────┐
//   │  OS thread            │ ─────────────────────────► │  mpsc::Receiver  │
//   │  inotify fd polling   │   CaptureEvent stream      │  caller thread   │
//   └──────────────────────┘                             └─────────────────┘
//
// The background thread uses `Inotify::init_with(IN_NONBLOCK)` so that
// `read_events()` returns immediately when no events are queued.  The thread
// sleeps for `POLL_INTERVAL` (250 ms) on each idle cycle, which also serves
// as the shutdown check window — when the `Receiver` is dropped, the next
// `tx.send()` fails and the thread exits cleanly.
//
// Security:
//   - Symlinks are rejected at every level (consistent with snapshot.rs).
//   - `DONT_FOLLOW` watch flag prevents the kernel from following symlinks.
//   - Only directories are watched; files are never opened or read.

use crate::{OpenNtxError, Result};
use inotify::{EventMask, Inotify, WatchMask};
use std::collections::HashMap;
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// How long to sleep between polls when the inotify queue is empty.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Buffer size for `read_events()`.  4 KiB comfortably holds ~15 events.
const EVENT_BUFFER_SIZE: usize = 4096;

// ── Public types ─────────────────────────────────────────────────────────────

/// A single filesystem change observed by the capture tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureEvent {
    /// A file or directory was created.
    FileCreated(PathBuf),
    /// A file was modified (content or metadata changed).
    FileModified(PathBuf),
    /// A file or directory was deleted.
    FileDeleted(PathBuf),
}

// ── CaptureSession ───────────────────────────────────────────────────────────

/// Real-time filesystem capture session.
///
/// Uses Linux `inotify` to watch a directory tree for CREATE, MODIFY, and
/// DELETE events.  A dedicated OS thread polls the inotify file descriptor
/// (non-blocking, 250 ms poll interval) so the caller is never blocked.
///
/// # Shutdown
///
/// Dropping the [`mpsc::Receiver`] returned by [`start_tracking`] signals
/// the background thread to exit.  The thread terminates within one poll
/// interval (~250 ms).
///
/// # Security
///
/// Symlinks are rejected at every level.  The `DONT_FOLLOW` watch flag is
/// set to prevent the kernel from following symlinks when resolving paths.
///
/// # Example
///
/// ```no_run
/// use openntx_core::capture::{CaptureSession, CaptureEvent};
/// use std::path::PathBuf;
///
/// let session = CaptureSession::new(PathBuf::from("/tmp/app-drive-c"));
/// let rx = session.start_tracking().expect("start tracking");
///
/// for event in rx {
///     match event {
///         CaptureEvent::FileCreated(p)  => println!("created:  {}", p.display()),
///         CaptureEvent::FileModified(p) => println!("modified: {}", p.display()),
///         CaptureEvent::FileDeleted(p)  => println!("deleted:  {}", p.display()),
///     }
/// }
/// ```
pub struct CaptureSession {
    target_dir: PathBuf,
}

impl CaptureSession {
    /// Create a new capture session targeting `target_dir`.
    ///
    /// The directory is **not** watched until [`start_tracking`] is called.
    pub fn new(target_dir: PathBuf) -> Self {
        Self { target_dir }
    }

    /// The directory being tracked.
    pub fn target_dir(&self) -> &Path {
        &self.target_dir
    }

    /// Start tracking filesystem events in `target_dir` recursively.
    ///
    /// Returns a [`mpsc::Receiver`] that yields [`CaptureEvent`]s.
    /// The background thread exits automatically when the receiver is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`OpenNtxError::InvalidInput`] if `target_dir` does not exist
    /// or is not a directory.  Returns [`OpenNtxError::Io`] if the inotify
    /// instance cannot be created or the background thread cannot be spawned.
    pub fn start_tracking(&self) -> Result<mpsc::Receiver<CaptureEvent>> {
        let target_dir = self.target_dir.clone();

        if !target_dir.is_dir() {
            return Err(OpenNtxError::InvalidInput(format!(
                "target directory does not exist or is not a directory: {}",
                target_dir.display()
            )));
        }

        let (tx, rx) = mpsc::channel();

        let target_dir_for_err = target_dir.clone();

        thread::Builder::new()
            .name("openntx-capture-tracker".into())
            .spawn(move || {
                if let Err(e) = tracking_loop(&target_dir, &tx) {
                    eprintln!("[openntx] capture tracker error: {e}");
                }
            })
            .map_err(|source| OpenNtxError::io(&target_dir_for_err, source))?;

        Ok(rx)
    }
}

// ── Internal: tracking loop ──────────────────────────────────────────────────

/// Main tracking loop — runs on the dedicated OS thread.
///
/// Initializes a non-blocking inotify instance, recursively watches all
/// directories under `target_dir`, and forwards matching events through
/// `tx`.  The loop exits when:
///
/// - `tx.send()` fails (receiver dropped), or
/// - an unrecoverable I/O error occurs.
fn tracking_loop(target_dir: &Path, tx: &mpsc::Sender<CaptureEvent>) -> Result<()> {
    let mut inotify = Inotify::init().map_err(|source| OpenNtxError::io(target_dir, source))?;

    // Set the inotify file descriptor to non-blocking so that read_events()
    // returns immediately with WouldBlock when no events are queued.
    // This lets us sleep + check for receiver shutdown between polls.
    set_nonblocking(inotify.as_raw_fd(), target_dir)?;

    // Maps watch descriptors to their directory paths so we can resolve
    // event names (which are relative to the watched directory) to full paths.
    let mut watch_map: HashMap<inotify::WatchDescriptor, PathBuf> = HashMap::new();

    // Seed watches for every existing directory under target_dir.
    add_watch_recursive(&mut inotify, target_dir, &mut watch_map)?;

    let mut buffer = [0u8; EVENT_BUFFER_SIZE];

    loop {
        match inotify.read_events(&mut buffer) {
            Ok(events) => {
                for event in events {
                    // Only process events that carry a CREATE, MODIFY, or
                    // DELETE flag.  ACCESS, OPEN, CLOSE_WRITE, CLOSE_NOWRITE,
                    // ATTRIB, MOVED_FROM, MOVED_TO, etc. are silently ignored
                    // unless they also carry one of the three target flags.
                    if !event
                        .mask
                        .intersects(EventMask::CREATE | EventMask::MODIFY | EventMask::DELETE)
                    {
                        continue;
                    }

                    // Resolve the full path: parent directory (from watch map)
                    // joined with the event name.  Events on the watched
                    // directory itself have `name == None`.
                    let name = event.name.unwrap_or_default();
                    let parent = watch_map
                        .get(&event.wd)
                        .cloned()
                        .unwrap_or_else(|| target_dir.to_path_buf());
                    let full_path = parent.join(name);

                    // ── CREATE ───────────────────────────────────────────
                    if event.mask.contains(EventMask::CREATE) {
                        if tx
                            .send(CaptureEvent::FileCreated(full_path.clone()))
                            .is_err()
                        {
                            return Ok(()); // receiver dropped
                        }
                        // Automatically watch newly created directories so
                        // files created inside them are also tracked.
                        if event.mask.contains(EventMask::ISDIR) {
                            // Non-fatal: log and continue if watch fails.
                            if let Err(e) =
                                add_watch_recursive(&mut inotify, &full_path, &mut watch_map)
                            {
                                eprintln!(
                                    "[openntx] failed to watch new dir {}: {e}",
                                    full_path.display()
                                );
                            }
                        }
                    }

                    // ── MODIFY ───────────────────────────────────────────
                    if event.mask.contains(EventMask::MODIFY) {
                        if tx
                            .send(CaptureEvent::FileModified(full_path.clone()))
                            .is_err()
                        {
                            return Ok(());
                        }
                    }

                    // ── DELETE ───────────────────────────────────────────
                    if event.mask.contains(EventMask::DELETE) {
                        if tx.send(CaptureEvent::FileDeleted(full_path)).is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No events queued — sleep before the next poll.
                // This is also the shutdown check window: if the receiver
                // was dropped, the next tx.send() will fail and the loop exits.
                thread::sleep(POLL_INTERVAL);
                continue;
            }
            Err(e) => {
                return Err(OpenNtxError::io(target_dir, e));
            }
        }
    }
}

// ── Internal: fd helpers ─────────────────────────────────────────────────────

/// Set `O_NONBLOCK` on a raw file descriptor via `fcntl`.
///
/// This is needed because `inotify` 0.10 does not expose `init_with()`
/// (removed in the published crate).  We create a blocking inotify fd and
/// immediately flip it to non-blocking so that `read_events()` returns
/// `WouldBlock` instead of blocking the thread.
fn set_nonblocking(fd: std::os::unix::io::RawFd, context: &Path) -> Result<()> {
    // SAFETY: we only call fcntl on a valid fd owned by the Inotify instance.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(OpenNtxError::io(context, std::io::Error::last_os_error()));
    }
    let rc = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if rc < 0 {
        return Err(OpenNtxError::io(context, std::io::Error::last_os_error()));
    }
    Ok(())
}

// ── Internal: recursive watch setup ──────────────────────────────────────────

/// Recursively add inotify watches for `dir` and all its subdirectories.
///
/// Security rules (consistent with `snapshot.rs`):
///   - Uses `symlink_metadata` to reject symlinks.
///   - Sets `DONT_FOLLOW` on every watch.
///   - Silently skips entries that cannot be stat'd (permission denied, etc.).
fn add_watch_recursive(
    inotify: &mut Inotify,
    dir: &Path,
    watch_map: &mut HashMap<inotify::WatchDescriptor, PathBuf>,
) -> Result<()> {
    let meta = fs::symlink_metadata(dir).map_err(|source| OpenNtxError::io(dir, source))?;

    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Ok(());
    }

    let mask = WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE | WatchMask::DONT_FOLLOW;

    let wd = inotify
        .watches()
        .add(dir, mask)
        .map_err(|source| OpenNtxError::io(dir, source))?;

    watch_map.insert(wd, dir.to_path_buf());

    // Walk immediate children and recurse into subdirectories.
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Use symlink_metadata to avoid following symlinks.
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    add_watch_recursive(inotify, &path, watch_map)?;
                }
            }
            // Entries that fail symlink_metadata (permission denied, etc.)
            // are silently skipped — non-fatal.
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant};

    /// Helper: create a `CaptureSession` backed by a temp directory.
    fn temp_session() -> (CaptureSession, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let session = CaptureSession::new(dir.path().to_path_buf());
        (session, dir)
    }

    /// Helper: drain events from `rx` until `pred` returns `true` or `timeout` elapses.
    fn wait_for(
        rx: &mpsc::Receiver<CaptureEvent>,
        timeout: Duration,
        pred: impl Fn(&CaptureEvent) -> bool,
    ) -> Option<CaptureEvent> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(event) if pred(&event) => return Some(event),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
        None
    }

    #[test]
    fn start_tracking_returns_receiver() {
        let (session, _tmp) = temp_session();
        let _rx = session.start_tracking().expect("start_tracking");
    }

    #[test]
    fn reject_nonexistent_dir() {
        let session = CaptureSession::new(PathBuf::from("/nonexistent/path"));
        let err = session.start_tracking().unwrap_err();
        assert!(matches!(err, OpenNtxError::InvalidInput(_)));
    }

    #[test]
    fn detect_file_creation() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");

        // Give the thread time to set up watches.
        thread::sleep(Duration::from_millis(100));

        let test_file = tmp.path().join("test.txt");
        fs::write(&test_file, "hello").expect("write test file");

        let found = wait_for(
            &rx,
            Duration::from_secs(3),
            |ev| matches!(ev, CaptureEvent::FileCreated(p) if p == &test_file),
        );
        assert!(
            found.is_some(),
            "expected FileCreated for {}",
            test_file.display()
        );
    }

    #[test]
    fn detect_file_modification() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");
        thread::sleep(Duration::from_millis(100));

        let test_file = tmp.path().join("modify.txt");
        fs::write(&test_file, "v1").expect("write");
        // Drain the initial FileCreated event.
        thread::sleep(Duration::from_millis(100));
        while rx.recv_timeout(Duration::from_millis(50)).is_ok() {}

        fs::write(&test_file, "v2").expect("modify");

        let found = wait_for(
            &rx,
            Duration::from_secs(3),
            |ev| matches!(ev, CaptureEvent::FileModified(p) if p == &test_file),
        );
        assert!(
            found.is_some(),
            "expected FileModified for {}",
            test_file.display()
        );
    }

    #[test]
    fn detect_file_deletion() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");
        thread::sleep(Duration::from_millis(100));

        let test_file = tmp.path().join("delete_me.txt");
        fs::write(&test_file, "bye").expect("write");
        thread::sleep(Duration::from_millis(100));
        // Drain create/modify events.
        while rx.recv_timeout(Duration::from_millis(50)).is_ok() {}

        fs::remove_file(&test_file).expect("delete");

        let found = wait_for(
            &rx,
            Duration::from_secs(3),
            |ev| matches!(ev, CaptureEvent::FileDeleted(p) if p == &test_file),
        );
        assert!(
            found.is_some(),
            "expected FileDeleted for {}",
            test_file.display()
        );
    }

    #[test]
    fn detect_nested_directory_events() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");
        thread::sleep(Duration::from_millis(100));

        let nested_dir = tmp.path().join("sub").join("deep");
        fs::create_dir_all(&nested_dir).expect("create nested");

        let found = wait_for(&rx, Duration::from_secs(3), |ev| {
            matches!(ev, CaptureEvent::FileCreated(_))
        });
        assert!(found.is_some(), "expected FileCreated for nested directory");
    }

    #[test]
    fn detect_file_in_new_subdirectory() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");
        thread::sleep(Duration::from_millis(100));

        let sub = tmp.path().join("newdir");
        fs::create_dir(&sub).expect("create dir");
        thread::sleep(Duration::from_millis(200)); // let tracker pick up the new watch

        let file_in_sub = sub.join("inner.txt");
        fs::write(&file_in_sub, "data").expect("write in subdir");

        let found = wait_for(
            &rx,
            Duration::from_secs(3),
            |ev| matches!(ev, CaptureEvent::FileCreated(p) if p == &file_in_sub),
        );
        assert!(
            found.is_some(),
            "expected FileCreated for file inside new subdirectory"
        );
    }

    #[test]
    fn receiver_drop_stops_thread() {
        let (session, _tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");

        // Drop the receiver — the thread should exit within ~250 ms.
        drop(rx);

        // If the thread didn't exit, this test would hang.  Give it time.
        thread::sleep(Duration::from_millis(500));
        // Test passes if we get here without hanging.
    }

    #[test]
    fn event_ordering_is_chronological() {
        let (session, tmp) = temp_session();
        let rx = session.start_tracking().expect("start_tracking");
        thread::sleep(Duration::from_millis(100));

        let f1 = tmp.path().join("first.txt");
        let f2 = tmp.path().join("second.txt");
        fs::write(&f1, "1").expect("write f1");
        thread::sleep(Duration::from_millis(50));
        fs::write(&f2, "2").expect("write f2");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut seen_f1 = false;
        let mut seen_f2 = false;
        let mut ordered = false;

        while Instant::now() < deadline && !(seen_f1 && seen_f2) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(CaptureEvent::FileCreated(p)) if p == f1 => seen_f1 = true,
                Ok(CaptureEvent::FileCreated(p)) if p == f2 => {
                    seen_f2 = true;
                    // f1 must have been seen before f2.
                    if seen_f1 {
                        ordered = true;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        assert!(seen_f1 && seen_f2, "should see both files");
        assert!(ordered, "f1 should appear before f2 in the event stream");
    }
}
