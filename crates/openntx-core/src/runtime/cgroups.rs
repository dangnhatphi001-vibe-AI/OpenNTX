// openntx-core/src/runtime/cgroups.rs — Linux cgroups v2 resource governance.
//
// Provides `ResourceGovernor` which creates per-application cgroup subtrees
// under `/sys/fs/cgroup/openntx/<app_id>/` and writes resource limits
// (memory, CPU) before moving the target process into the cgroup.
//
// Cgroups v2 hierarchy:
//
//   /sys/fs/cgroup/
//     openntx/                      ← root cgroup (created once)
//       <app_id>/                   ← per-application cgroup
//         memory.max                ← RAM limit in bytes
//         cpu.max                   ← CPU bandwidth ("$MAX $PERIOD")
//         cgroup.procs              ← PID assignment file
//
// Requires root privileges to write to the cgroup filesystem.

use crate::{OpenNtxError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Base path for the OpenNTX cgroup hierarchy.
const CGROUP_ROOT: &str = "/sys/fs/cgroup/openntx";

/// Default cgroup v2 CPU period in microseconds (100 ms).
const DEFAULT_CPU_PERIOD: u32 = 100_000;

/// `ResourceGovernor` manages cgroups v2 resource limits for sandboxed
/// Windows application processes.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::cgroups::ResourceGovernor;
///
/// let governor = ResourceGovernor::new();
/// // After spawning a process with PID 12345:
/// governor.apply_limits(12345, "notepadpp-a3f2", 4_294_967_296, "50000 100000").unwrap();
/// ```
pub struct ResourceGovernor {
    /// Root path for the OpenNTX cgroup hierarchy.
    cgroup_root: PathBuf,
}

impl ResourceGovernor {
    /// Create a new `ResourceGovernor` using the default cgroup root
    /// (`/sys/fs/cgroup/openntx`).
    pub fn new() -> Self {
        Self {
            cgroup_root: PathBuf::from(CGROUP_ROOT),
        }
    }

    /// Create a new `ResourceGovernor` with a custom cgroup root path.
    ///
    /// Useful for testing with temporary directories.
    pub fn with_root(cgroup_root: PathBuf) -> Self {
        Self { cgroup_root }
    }

    /// Return the cgroup root path.
    pub fn cgroup_root(&self) -> &Path {
        &self.cgroup_root
    }

    /// Return the cgroup directory path for a given `app_id`.
    pub fn cgroup_path(&self, app_id: &str) -> PathBuf {
        self.cgroup_root.join(sanitize_app_id(app_id))
    }

    /// Apply resource limits and move a process into its cgroup.
    ///
    /// # Steps
    ///
    /// 1. Create the root cgroup (`/sys/fs/cgroup/openntx/`) if it does not
    ///    exist.
    /// 2. Create the per-app cgroup (`…/<app_id>/`).
    /// 3. Write `memory.max` with the RAM limit in bytes.
    /// 4. Write `cpu.max` with the CPU bandwidth quota.
    /// 5. Write the PID to `cgroup.procs` to enforce limits immediately.
    ///
    /// # Parameters
    ///
    /// - `pid` — Process ID of the spawned Wine process.
    /// - `app_id` — Application identifier (used as cgroup directory name).
    /// - `max_memory_bytes` — Maximum memory in bytes (e.g. `4_294_967_296`
    ///   for 4 GB). Pass `0` to skip memory limiting.
    /// - `cpu_max_quota` — CPU bandwidth string in cgroups v2 format:
    ///   `"$MAX $PERIOD"` (e.g. `"50000 100000"` for 50% of one core).
    ///   Pass `"max"` to skip CPU limiting.
    ///
    /// # Errors
    ///
    /// - `CgroupCreationFailed` if a cgroup directory cannot be created.
    /// - `CgroupWriteFailed` if a limit file or PID assignment fails.
    pub fn apply_limits(
        &self,
        pid: u32,
        app_id: &str,
        max_memory_bytes: i64,
        cpu_max_quota: &str,
    ) -> Result<()> {
        // 1. Ensure root cgroup exists.
        self.ensure_cgroup_dir(&self.cgroup_root)?;

        // 2. Create per-app cgroup.
        let app_cgroup = self.cgroup_path(app_id);
        self.ensure_cgroup_dir(&app_cgroup)?;

        // 3. Write memory limit.
        if max_memory_bytes > 0 {
            let memory_value = max_memory_bytes.to_string();
            self.write_cgroup_file(
                &app_cgroup.join("memory.max"),
                &memory_value,
                "memory.max",
                app_id,
            )?;
        }

        // 4. Write CPU limit.
        if cpu_max_quota != "max" {
            self.write_cgroup_file(
                &app_cgroup.join("cpu.max"),
                cpu_max_quota,
                "cpu.max",
                app_id,
            )?;
        }

        // 5. Move process into cgroup.
        let pid_str = pid.to_string();
        self.write_cgroup_file(
            &app_cgroup.join("cgroup.procs"),
            &pid_str,
            "cgroup.procs",
            app_id,
        )?;

        Ok(())
    }

    /// Remove the cgroup for an application.
    ///
    /// Attempts to remove the per-app cgroup directory. This will only
    /// succeed if no processes are still running in the cgroup.
    pub fn remove_cgroup(&self, app_id: &str) -> Result<()> {
        let app_cgroup = self.cgroup_path(app_id);
        if app_cgroup.exists() {
            fs::remove_dir(&app_cgroup).map_err(|source| {
                OpenNtxError::CgroupCreationFailed(format!(
                    "failed to remove cgroup {}: {}",
                    app_cgroup.display(),
                    source
                ))
            })?;
        }
        Ok(())
    }

    /// Check whether the cgroup filesystem is available.
    pub fn is_available(&self) -> bool {
        let cgroup_root = Path::new("/sys/fs/cgroup");
        cgroup_root.exists() && cgroup_root.join("cgroup.controllers").exists()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Create a cgroup directory if it does not exist.
    fn ensure_cgroup_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            fs::create_dir_all(path).map_err(|source| {
                OpenNtxError::CgroupCreationFailed(format!(
                    "failed to create cgroup directory {}: {}",
                    path.display(),
                    source
                ))
            })?;
        }
        Ok(())
    }

    /// Write a value to a cgroup control file.
    fn write_cgroup_file(
        &self,
        file_path: &Path,
        value: &str,
        file_name: &str,
        app_id: &str,
    ) -> Result<()> {
        fs::write(file_path, value.as_bytes()).map_err(|source| {
            OpenNtxError::CgroupWriteFailed(format!(
                "failed to write '{}' to {} for app '{}': {}",
                value, file_name, app_id, source
            ))
        })
    }
}

impl Default for ResourceGovernor {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize an `app_id` for use as a cgroup directory name.
///
/// Replaces characters that are not valid in cgroup paths (slashes,
/// whitespace, null bytes) with underscores.
fn sanitize_app_id(app_id: &str) -> String {
    app_id
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ' ' | '\t' | '\n' | '\r' => '_',
            other => other,
        })
        .collect()
}

/// Build the `cpu.max` string for a given percentage.
///
/// Converts a percentage (0–100) to the cgroups v2 `"$MAX $PERIOD"` format.
/// The period is fixed at 100,000 µs (100 ms).
pub fn cpu_max_from_percent(percent: u32) -> String {
    let percent = percent.min(100);
    let max = (DEFAULT_CPU_PERIOD as u64 * percent as u64) / 100;
    format!("{} {}", max, DEFAULT_CPU_PERIOD)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_governor_has_correct_root() {
        let gov = ResourceGovernor::new();
        assert_eq!(gov.cgroup_root(), Path::new(CGROUP_ROOT));
    }

    #[test]
    fn custom_root() {
        let gov = ResourceGovernor::with_root(PathBuf::from("/tmp/test-cgroup"));
        assert_eq!(gov.cgroup_root(), Path::new("/tmp/test-cgroup"));
    }

    #[test]
    fn cgroup_path_for_app() {
        let gov = ResourceGovernor::with_root(PathBuf::from("/sys/fs/cgroup/openntx"));
        let path = gov.cgroup_path("notepadpp-v8.6");
        assert_eq!(
            path,
            PathBuf::from("/sys/fs/cgroup/openntx/notepadpp-v8.6")
        );
    }

    #[test]
    fn sanitize_app_id_replaces_slashes() {
        assert_eq!(sanitize_app_id("app/id"), "app_id");
        assert_eq!(sanitize_app_id("app\\id"), "app_id");
        assert_eq!(sanitize_app_id("app id"), "app_id");
    }

    #[test]
    fn sanitize_app_id_preserves_valid_chars() {
        assert_eq!(sanitize_app_id("notepadpp-v8.6"), "notepadpp-v8.6");
        assert_eq!(sanitize_app_id("test-app-1234"), "test-app-1234");
    }

    #[test]
    fn sanitize_app_id_handles_empty() {
        assert_eq!(sanitize_app_id(""), "");
    }

    #[test]
    fn cpu_max_from_percent_50() {
        let result = cpu_max_from_percent(50);
        assert_eq!(result, "50000 100000");
    }

    #[test]
    fn cpu_max_from_percent_100() {
        let result = cpu_max_from_percent(100);
        assert_eq!(result, "100000 100000");
    }

    #[test]
    fn cpu_max_from_percent_0() {
        let result = cpu_max_from_percent(0);
        assert_eq!(result, "0 100000");
    }

    #[test]
    fn cpu_max_from_percent_clamped_over_100() {
        let result = cpu_max_from_percent(150);
        assert_eq!(result, "100000 100000");
    }

    #[test]
    fn cpu_max_from_percent_25() {
        let result = cpu_max_from_percent(25);
        assert_eq!(result, "25000 100000");
    }

    #[test]
    fn apply_limits_creates_cgroup_structure() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        // apply_limits will fail on cgroup.procs (PID 0 is invalid),
        // but should successfully create directories and write limits.
        let result = gov.apply_limits(0, "test-app", 4_294_967_296, "50000 100000");

        // The cgroup directories should have been created.
        assert!(dir.path().join("test-app").exists());

        // memory.max should have been written.
        let memory_content =
            fs::read_to_string(dir.path().join("test-app/memory.max")).unwrap();
        assert_eq!(memory_content, "4294967296");

        // cpu.max should have been written.
        let cpu_content = fs::read_to_string(dir.path().join("test-app/cpu.max")).unwrap();
        assert_eq!(cpu_content, "50000 100000");

        // cgroup.procs write will fail because PID 0 is not valid,
        // but the preceding writes should succeed.
        // Depending on the kernel, writing "0" to cgroup.procs may succeed
        // or fail. Either way, the structure test above is valid.
        let _ = result; // Don't assert on the overall result.
    }

    #[test]
    fn apply_limits_skips_memory_when_zero() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        let _ = gov.apply_limits(0, "test-app", 0, "50000 100000");

        // memory.max should NOT have been written.
        assert!(!dir.path().join("test-app/memory.max").exists());

        // cpu.max should have been written.
        let cpu_content = fs::read_to_string(dir.path().join("test-app/cpu.max")).unwrap();
        assert_eq!(cpu_content, "50000 100000");
    }

    #[test]
    fn apply_limits_skips_cpu_when_max() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        let _ = gov.apply_limits(0, "test-app", 4_294_967_296, "max");

        // memory.max should have been written.
        let memory_content =
            fs::read_to_string(dir.path().join("test-app/memory.max")).unwrap();
        assert_eq!(memory_content, "4294967296");

        // cpu.max should NOT have been written.
        assert!(!dir.path().join("test-app/cpu.max").exists());
    }

    #[test]
    fn remove_cgroup_cleans_up() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        // Create the cgroup directory.
        let app_cgroup = gov.cgroup_path("my-app");
        fs::create_dir_all(&app_cgroup).unwrap();
        assert!(app_cgroup.exists());

        // Remove it.
        gov.remove_cgroup("my-app").unwrap();
        assert!(!app_cgroup.exists());
    }

    #[test]
    fn remove_cgroup_noop_when_missing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        // Should succeed even if the directory doesn't exist.
        gov.remove_cgroup("nonexistent").unwrap();
    }

    #[test]
    fn default_trait_works() {
        let gov = ResourceGovernor::default();
        assert_eq!(gov.cgroup_root(), Path::new(CGROUP_ROOT));
    }

    #[test]
    fn cgroup_path_with_sanitization() {
        let gov = ResourceGovernor::with_root(PathBuf::from("/sys/fs/cgroup/openntx"));
        let path = gov.cgroup_path("app/id with spaces");
        assert_eq!(
            path,
            PathBuf::from("/sys/fs/cgroup/openntx/app_id_with_spaces")
        );
    }

    #[test]
    fn apply_limits_memory_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        let _ = gov.apply_limits(0, "mem-only", 2_147_483_648, "max");

        let memory_content =
            fs::read_to_string(dir.path().join("mem-only/memory.max")).unwrap();
        assert_eq!(memory_content, "2147483648");
        assert!(!dir.path().join("mem-only/cpu.max").exists());
    }

    #[test]
    fn apply_limits_cpu_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let gov = ResourceGovernor::with_root(dir.path().to_path_buf());

        let _ = gov.apply_limits(0, "cpu-only", 0, "25000 100000");

        assert!(!dir.path().join("cpu-only/memory.max").exists());
        let cpu_content =
            fs::read_to_string(dir.path().join("cpu-only/cpu.max")).unwrap();
        assert_eq!(cpu_content, "25000 100000");
    }
}
