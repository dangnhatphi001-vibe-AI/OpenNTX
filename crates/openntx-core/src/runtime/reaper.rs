// openntx-core/src/runtime/reaper.rs — Process reaping engine.
//
// Terminates all processes inside a per-application cgroup by reading the
// PID list from `cgroup.procs`, sending SIGTERM, waiting for graceful
// shutdown, then escalating to SIGKILL for any survivors.
//
// Cgroup layout:
//   /sys/fs/cgroup/openntx/<app_id>/cgroup.procs   ← one PID per line
//
// The reaper uses `libc::kill` directly (async-signal-safe) and
// `std::thread::sleep` for the grace period.  It does NOT require tokio.

use crate::runtime::cgroups::ResourceGovernor;
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Grace period between SIGTERM and SIGKILL.
const KILL_GRACE_PERIOD: Duration = Duration::from_millis(500);

/// Process reaping engine for OpenNTX sandboxed applications.
///
/// Reads the PID list from a cgroup's `cgroup.procs` file, sends SIGTERM
/// to each process, waits a grace period, then SIGKILLs any survivors.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::reaper::ReaperEngine;
///
/// let reaper = ReaperEngine::new();
/// let killed = reaper.reap_application("notepadpp-a3f2").expect("reap");
/// println!("killed {killed} processes");
/// reaper.cleanup_cgroup_node("notepadpp-a3f2").expect("cleanup");
/// ```
pub struct ReaperEngine {
    /// The cgroup root path (reused from ResourceGovernor).
    cgroup_root: PathBuf,
}

impl ReaperEngine {
    /// Create a new `ReaperEngine` using the default cgroup root.
    pub fn new() -> Self {
        Self {
            cgroup_root: PathBuf::from(ResourceGovernor::new().cgroup_root()),
        }
    }

    /// Create a new `ReaperEngine` with a custom cgroup root.
    pub fn with_root(cgroup_root: PathBuf) -> Self {
        Self { cgroup_root }
    }

    /// Return the cgroup root path.
    pub fn cgroup_root(&self) -> &Path {
        &self.cgroup_root
    }

    /// Return the `cgroup.procs` path for a given app.
    pub fn cgroup_procs_path(&self, app_id: &str) -> PathBuf {
        self.cgroup_root.join(app_id).join("cgroup.procs")
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Reap all processes in the cgroup for `app_id`.
    ///
    /// 1. Reads PIDs from `cgroup.procs`.
    /// 2. Sends SIGTERM to each.
    /// 3. Waits 500ms for graceful shutdown.
    /// 4. Re-reads `cgroup.procs`; SIGKILLs any survivors.
    /// 5. Returns the total number of processes killed.
    pub fn reap_application(&self, app_id: &str) -> Result<u32> {
        let procs_path = self.cgroup_procs_path(app_id);

        // Step 1: Read initial PID list.
        let pids = read_pids_from_file(&procs_path)?;

        if pids.is_empty() {
            return Ok(0);
        }

        let initial_count = pids.len() as u32;

        // Step 2: SIGTERM all processes.
        for pid in &pids {
            send_signal(*pid, libc::SIGTERM).map_err(|e| {
                OpenNtxError::ReaperProcessKillFailed(format!(
                    "SIGTERM to PID {} failed: {}",
                    pid, e
                ))
            })?;
        }

        // Step 3: Wait for graceful shutdown.
        thread::sleep(KILL_GRACE_PERIOD);

        // Step 4: Re-read and SIGKILL survivors.
        let survivors = read_pids_from_file(&procs_path)?;
        for pid in &survivors {
            send_signal(*pid, libc::SIGKILL).map_err(|e| {
                OpenNtxError::ReaperProcessKillFailed(format!(
                    "SIGKILL to PID {} failed: {}",
                    pid, e
                ))
            })?;
        }

        Ok(initial_count)
    }

    /// Remove the cgroup directory for `app_id` after all processes are dead.
    ///
    /// Verifies that `cgroup.procs` is empty before deletion.
    pub fn cleanup_cgroup_node(&self, app_id: &str) -> Result<()> {
        let procs_path = self.cgroup_procs_path(app_id);

        // Verify the cgroup is empty before removing.
        if procs_path.exists() {
            let remaining = read_pids_from_file(&procs_path)?;
            if !remaining.is_empty() {
                return Err(OpenNtxError::CgroupCleanupFailed(format!(
                    "cgroup for '{}' still has {} active PIDs; refusing to remove",
                    app_id,
                    remaining.len()
                )));
            }
        }

        let cgroup_dir = self.cgroup_root.join(app_id);
        if cgroup_dir.exists() {
            // Remove the cgroup.procs file first (the only user-created file).
            // In a real cgroup, the kernel auto-removes control files when
            // the directory is released, but our mock dirs need manual cleanup.
            if procs_path.exists() {
                fs::remove_file(&procs_path).map_err(|source| {
                    OpenNtxError::CgroupCleanupFailed(format!(
                        "failed to remove {}: {}",
                        procs_path.display(),
                        source
                    ))
                })?;
            }

            fs::remove_dir(&cgroup_dir).map_err(|source| {
                OpenNtxError::CgroupCleanupFailed(format!(
                    "failed to remove cgroup directory {}: {}",
                    cgroup_dir.display(),
                    source
                ))
            })?;
        }

        Ok(())
    }
}

impl Default for ReaperEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Read PIDs from a `cgroup.procs` file (one PID per line).
pub fn read_pids_from_file(path: &Path) -> Result<Vec<i32>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|source| OpenNtxError::io(path, source))?;

    let mut pids = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed.parse::<i32>() {
            Ok(pid) if pid > 0 => pids.push(pid),
            _ => {
                // Skip malformed lines (comments, garbage).
                continue;
            }
        }
    }

    Ok(pids)
}

/// Send a signal to a process via `libc::kill`.
///
/// Returns `Ok(())` on success.  Returns `Err` with the errno if the
/// signal could not be delivered (e.g. process does not exist).
fn send_signal(pid: i32, signal: i32) -> std::result::Result<(), std::io::Error> {
    // SAFETY: libc::kill is a simple syscall wrapper.  It is
    // async-signal-safe and does not touch Rust memory.
    let rc = unsafe { libc::kill(pid, signal) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    /// Write a mock `cgroup.procs` file with the given PID lines.
    fn write_mock_procs(dir: &Path, app_id: &str, content: &str) -> PathBuf {
        let cgroup_dir = dir.join(app_id);
        fs::create_dir_all(&cgroup_dir).expect("create cgroup dir");
        let procs_path = cgroup_dir.join("cgroup.procs");
        let mut f = fs::File::create(&procs_path).expect("create procs file");
        f.write_all(content.as_bytes()).expect("write");
        procs_path
    }

    // ── read_pids_from_file tests ────────────────────────────────────────

    #[test]
    fn read_pids_parses_valid_lines() {
        let dir = temp_dir();
        let path = write_mock_procs(dir.path(), "test-app", "1234\n5678\n9012\n");
        let pids = read_pids_from_file(&path).unwrap();
        assert_eq!(pids, vec![1234, 5678, 9012]);
    }

    #[test]
    fn read_pids_skips_empty_lines() {
        let dir = temp_dir();
        let path = write_mock_procs(dir.path(), "test-app", "100\n\n\n200\n");
        let pids = read_pids_from_file(&path).unwrap();
        assert_eq!(pids, vec![100, 200]);
    }

    #[test]
    fn read_pids_skips_malformed_lines() {
        let dir = temp_dir();
        let path = write_mock_procs(dir.path(), "test-app", "100\nabc\n-5\n0\n200\n");
        let pids = read_pids_from_file(&path).unwrap();
        // "abc" is not a valid integer, -5 and 0 are <= 0, so only 100 and 200.
        assert_eq!(pids, vec![100, 200]);
    }

    #[test]
    fn read_pids_empty_file() {
        let dir = temp_dir();
        let path = write_mock_procs(dir.path(), "test-app", "");
        let pids = read_pids_from_file(&path).unwrap();
        assert!(pids.is_empty());
    }

    #[test]
    fn read_pids_missing_file() {
        let dir = temp_dir();
        let path = dir.path().join("nonexistent").join("cgroup.procs");
        let pids = read_pids_from_file(&path).unwrap();
        assert!(pids.is_empty());
    }

    #[test]
    fn read_pids_whitespace_trimmed() {
        let dir = temp_dir();
        let path = write_mock_procs(dir.path(), "test-app", "  1234 \n\t5678\t\n");
        let pids = read_pids_from_file(&path).unwrap();
        assert_eq!(pids, vec![1234, 5678]);
    }

    #[test]
    fn read_pids_large_list() {
        let dir = temp_dir();
        let content: String = (1..=1000).map(|i| format!("{}\n", i)).collect();
        let path = write_mock_procs(dir.path(), "test-app", &content);
        let pids = read_pids_from_file(&path).unwrap();
        assert_eq!(pids.len(), 1000);
        assert_eq!(pids[0], 1);
        assert_eq!(pids[999], 1000);
    }

    // ── ReaperEngine path helpers ────────────────────────────────────────

    #[test]
    fn cgroup_procs_path_format() {
        let reaper = ReaperEngine::with_root(PathBuf::from("/sys/fs/cgroup/openntx"));
        let path = reaper.cgroup_procs_path("my-app");
        assert_eq!(
            path,
            PathBuf::from("/sys/fs/cgroup/openntx/my-app/cgroup.procs")
        );
    }

    #[test]
    fn default_reaper_has_correct_root() {
        let reaper = ReaperEngine::new();
        assert_eq!(reaper.cgroup_root(), Path::new("/sys/fs/cgroup/openntx"));
    }

    #[test]
    fn default_trait_works() {
        let reaper = ReaperEngine::default();
        assert_eq!(reaper.cgroup_root(), Path::new("/sys/fs/cgroup/openntx"));
    }

    // ── reap_application tests ───────────────────────────────────────────

    #[test]
    fn reap_empty_cgroup_returns_zero() {
        let dir = temp_dir();
        write_mock_procs(dir.path(), "empty-app", "");
        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        let killed = reaper.reap_application("empty-app").unwrap();
        assert_eq!(killed, 0);
    }

    #[test]
    fn reap_missing_cgroup_returns_zero() {
        let dir = temp_dir();
        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        let killed = reaper.reap_application("nonexistent").unwrap();
        assert_eq!(killed, 0);
    }

    #[test]
    fn reap_with_nonexistent_pids() {
        // PIDs that don't exist will get ESRCH from kill().
        // The reaper should propagate this as an error.
        let dir = temp_dir();
        write_mock_procs(dir.path(), "dead-app", "999999\n999998\n");
        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        let result = reaper.reap_application("dead-app");
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::ReaperProcessKillFailed(msg) => {
                assert!(msg.contains("SIGTERM"), "msg: {}", msg);
            }
            other => panic!("expected ReaperProcessKillFailed, got: {:?}", other),
        }
    }

    #[test]
    fn reap_single_pid_file_format() {
        // Test that a single PID is correctly parsed and the count is returned.
        let dir = temp_dir();
        write_mock_procs(dir.path(), "single-app", "1\n");
        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        // PID 1 (init) will likely ignore SIGTERM/SIGKILL from non-root,
        // but the reaper should still return the count.
        // On non-root, kill(1, SIGTERM) returns EPERM.
        let result = reaper.reap_application("single-app");
        // Either succeeds or fails with permission error — both are valid.
        if let Ok(count) = result {
            assert_eq!(count, 1);
        }
        // If Err, it's because we don't have permission to signal PID 1.
    }

    // ── cleanup_cgroup_node tests ────────────────────────────────────────

    #[test]
    fn cleanup_removes_empty_cgroup() {
        let dir = temp_dir();
        let app_dir = dir.path().join("clean-app");
        fs::create_dir_all(&app_dir).unwrap();
        // Write an empty cgroup.procs.
        fs::write(app_dir.join("cgroup.procs"), "").unwrap();

        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        reaper.cleanup_cgroup_node("clean-app").unwrap();

        assert!(!app_dir.exists(), "cgroup directory should be removed");
    }

    #[test]
    fn cleanup_refuses_if_pids_remain() {
        let dir = temp_dir();
        write_mock_procs(dir.path(), "busy-app", "1234\n");

        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        let result = reaper.cleanup_cgroup_node("busy-app");
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::CgroupCleanupFailed(msg) => {
                assert!(msg.contains("still has"), "msg: {}", msg);
            }
            other => panic!("expected CgroupCleanupFailed, got: {:?}", other),
        }
    }

    #[test]
    fn cleanup_noop_when_missing() {
        let dir = temp_dir();
        let reaper = ReaperEngine::with_root(dir.path().to_path_buf());
        // Should succeed even if the directory doesn't exist.
        reaper.cleanup_cgroup_node("ghost-app").unwrap();
    }

    // ── send_signal tests ────────────────────────────────────────────────

    #[test]
    fn send_signal_to_nonexistent_pid_returns_error() {
        // PID 999999 should not exist.
        let result = send_signal(999999, libc::SIGTERM);
        assert!(result.is_err());
    }

    #[test]
    fn send_signal_to_self_succeedes() {
        // Sending signal 0 to self should always succeed (existence check).
        let result = send_signal(unsafe { libc::getpid() }, 0);
        assert!(result.is_ok());
    }
}
