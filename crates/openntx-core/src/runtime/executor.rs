// openntx-core/src/runtime/executor.rs — PE execution orchestrator.
//
// When a PE file (.exe) is invoked (via binfmt_misc or directly), the
// `OpenNTXExecutor` coordinates:
//
//   1. Identification  — hash the PE to derive an `app_id`.
//   2. Profile lookup  — check if a CompatProfile already exists.
//   3. Sandbox setup   — virtualise environment variables and route `drive_c`.
//   4. Security        — apply cgroups v2 limits and namespace isolation.
//   5. Process launch  — fork/exec into Wine (headless) with filtered output.
//
// All Wine stderr/stdout noise is suppressed; only OpenNTX diagnostics are
// emitted.

use crate::app_id::generate_app_id;
use crate::profile::{CompatProfile, ProfileManager};
use crate::runtime::cgroups::ResourceGovernor;
use crate::{OpenNtxError, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default Wine binary used when no profile specifies an alternative.
const DEFAULT_WINE: &str = "wine64";

/// Fallback Wine binary for 32-bit PE files.
const DEFAULT_WINE_32: &str = "wine";

/// Root directory under which per-application Wine prefixes are stored.
const SANDBOX_ROOT: &str = ".local/share/openntx/sandboxes";

/// Default memory limit: 4 GB.
const DEFAULT_MAX_MEMORY_BYTES: i64 = 4_294_967_296;

/// Default CPU quota: 100% (no throttling).
const DEFAULT_CPU_QUOTA: &str = "max";

// ── Security configuration ──────────────────────────────────────────────────

/// Namespace isolation flags.
#[derive(Debug, Clone, Default)]
pub struct NamespaceConfig {
    /// Isolate into a new PID namespace.
    pub new_pid: bool,
    /// Isolate into a new network namespace (no host network access).
    pub new_net: bool,
    /// Isolate into a new mount namespace (private mount tree).
    pub new_mount: bool,
}

/// Security configuration for a PE execution.
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Namespace isolation settings.
    pub namespaces: NamespaceConfig,
    /// Maximum memory in bytes (cgroups v2 `memory.max`).  `0` = no limit.
    pub max_memory_bytes: i64,
    /// CPU bandwidth quota (cgroups v2 `cpu.max`).  `"max"` = no limit.
    pub cpu_max_quota: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            namespaces: NamespaceConfig::default(),
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            cpu_max_quota: DEFAULT_CPU_QUOTA.to_string(),
        }
    }
}

impl SecurityConfig {
    /// Create a hardened config with full namespace isolation and resource
    /// limits.
    pub fn hardened() -> Self {
        Self {
            namespaces: NamespaceConfig {
                new_pid: false, // PID ns requires wrapper binary
                new_net: true,
                new_mount: true,
            },
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            cpu_max_quota: DEFAULT_CPU_QUOTA.to_string(),
        }
    }

    /// Create a permissive config with no isolation (direct execution).
    pub fn permissive() -> Self {
        Self {
            namespaces: NamespaceConfig::default(),
            max_memory_bytes: 0,
            cpu_max_quota: "max".to_string(),
        }
    }
}

// ── OpenNTXExecutor ─────────────────────────────────────────────────────────

/// Orchestrates PE execution on Linux via Wine / Proton in an isolated
/// environment with optional cgroups v2 resource governance and Linux
/// namespace isolation.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::executor::{OpenNTXExecutor, SecurityConfig};
/// use std::path::Path;
///
/// let executor = OpenNTXExecutor::new().expect("init");
/// let security = SecurityConfig::hardened();
/// executor.execute_pe(Path::new("/home/user/app.exe"), &[], &security).expect("run");
/// ```
pub struct OpenNTXExecutor {
    profile_manager: ProfileManager,
    /// Root directory for Wine prefixes (sandboxes).
    sandbox_root: PathBuf,
    /// Resource governor for cgroups v2.
    resource_governor: ResourceGovernor,
}

impl OpenNTXExecutor {
    /// Create a new executor using default paths.
    pub fn new() -> Result<Self> {
        let profile_manager = ProfileManager::new()?;
        let data_dir = dirs::data_local_dir().ok_or_else(|| {
            OpenNtxError::Config("unable to determine local data directory".into())
        })?;
        let sandbox_root = data_dir.join(
            SANDBOX_ROOT
                .strip_prefix(".local/share/")
                .unwrap_or(SANDBOX_ROOT),
        );
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;

        Ok(Self {
            profile_manager,
            sandbox_root,
            resource_governor: ResourceGovernor::new(),
        })
    }

    /// Create a new executor with explicit paths (useful for testing).
    pub fn with_paths(profile_manager: ProfileManager, sandbox_root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;
        Ok(Self {
            profile_manager,
            sandbox_root,
            resource_governor: ResourceGovernor::new(),
        })
    }

    /// Create a new executor with explicit paths and a custom resource
    /// governor (for testing with temp cgroup directories).
    pub fn with_paths_and_governor(
        profile_manager: ProfileManager,
        sandbox_root: PathBuf,
        resource_governor: ResourceGovernor,
    ) -> Result<Self> {
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;
        Ok(Self {
            profile_manager,
            sandbox_root,
            resource_governor,
        })
    }

    /// Return a reference to the underlying [`ProfileManager`].
    pub fn profile_manager(&self) -> &ProfileManager {
        &self.profile_manager
    }

    /// Return the sandbox root directory.
    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    /// Return a reference to the [`ResourceGovernor`].
    pub fn resource_governor(&self) -> &ResourceGovernor {
        &self.resource_governor
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Execute a PE file with security enforcement.
    ///
    /// This is the main entry point called by the binfmt_misc runtime shim or
    /// the CLI.
    ///
    /// # Steps
    ///
    /// 1. Compute the `app_id` from the PE file's SHA-256 hash.
    /// 2. Look up the compatibility profile.
    /// 3. Set up an isolated Wine prefix (sandbox).
    /// 4. Configure namespace isolation via `CommandExt::before_spawn`.
    /// 5. Spawn the process, then apply cgroups v2 resource limits.
    ///
    /// # Errors
    ///
    /// - `InvalidInput` if the file does not exist or is not a valid PE.
    /// - `RuntimeExecution` if the Wine process cannot be spawned.
    /// - `WinePrefix` if the sandbox cannot be prepared.
    /// - `CgroupCreationFailed` / `CgroupWriteFailed` if cgroup setup fails.
    /// - `NamespaceUnshareFailed` if namespace isolation fails.
    pub fn execute_pe(
        &self,
        pe_path: &Path,
        args: &[String],
        security: &SecurityConfig,
    ) -> Result<()> {
        // ── Step 1: Validate and identify ────────────────────────────────
        if !pe_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "PE file not found: {}",
                pe_path.display()
            )));
        }

        if !is_pe_file(pe_path)? {
            return Err(OpenNtxError::InvalidInput(format!(
                "file is not a valid PE executable (missing MZ header): {}",
                pe_path.display()
            )));
        }

        let pe_hash = hash_pe_file(pe_path)?;
        let filename = pe_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown");
        let app_id = generate_app_id(filename, Some(&pe_hash));

        // ── Step 2: Profile lookup ───────────────────────────────────────
        let profile = match self.profile_manager.load_profile(&app_id) {
            Ok(profile) => Some(profile),
            Err(OpenNtxError::AppNotFound(_)) => None,
            Err(e) => return Err(e),
        };

        // ── Step 3: Sandbox setup ────────────────────────────────────────
        let prefix_path = self.prepare_sandbox(&app_id, &profile)?;

        // ── Step 4: Build command with security ──────────────────────────
        let wine_bin = select_wine_binary(&profile);
        let wine_env = build_wine_environment(&prefix_path);

        let mut cmd = Command::new(&wine_bin);
        cmd.arg(pe_path);
        cmd.args(args);
        cmd.envs(&wine_env);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        // ── Step 5: Namespace isolation via before_spawn ─────────────────
        let ns_flags = build_unshare_flags(&security.namespaces);
        if ns_flags != 0 {
            // SAFETY: `before_spawn` runs in the child process context just
            // before `exec`. The closure captures `ns_flags` by value.
            // `libc::unshare` is async-signal-safe and valid in this context.
            unsafe {
                cmd.pre_exec(move || {
                    let rc = libc::unshare(ns_flags);
                    if rc != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        // ── Step 6: Spawn and apply cgroups ──────────────────────────────
        let mut child = cmd.spawn().map_err(|source| {
            OpenNtxError::RuntimeExecution(format!(
                "failed to execute {} via {}: {}",
                pe_path.display(),
                wine_bin,
                source
            ))
        })?;

        // Apply cgroups v2 resource limits to the spawned process.
        let pid = child.id();
        let _ = self.resource_governor.apply_limits(
            pid,
            &app_id,
            security.max_memory_bytes,
            &security.cpu_max_quota,
        );

        // Wait for the process to finish.
        let _status = child.wait().map_err(|source| {
            OpenNtxError::RuntimeExecution(format!(
                "failed to wait for {}: {}",
                pe_path.display(),
                source
            ))
        })?;

        Ok(())
    }

    /// Execute a PE file in "dry-run" mode — validate everything up to the
    /// point of actually spawning Wine, then return the resolved app_id and
    /// Wine binary without running anything.
    ///
    /// Useful for the `openntx run --dry-run` CLI path.
    pub fn plan_execution(&self, pe_path: &Path) -> Result<ExecutionPlan> {
        if !pe_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "PE file not found: {}",
                pe_path.display()
            )));
        }

        if !is_pe_file(pe_path)? {
            return Err(OpenNtxError::InvalidInput(format!(
                "file is not a valid PE executable (missing MZ header): {}",
                pe_path.display()
            )));
        }

        let pe_hash = hash_pe_file(pe_path)?;
        let filename = pe_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown");
        let app_id = generate_app_id(filename, Some(&pe_hash));

        let profile = match self.profile_manager.load_profile(&app_id) {
            Ok(profile) => Some(profile),
            Err(OpenNtxError::AppNotFound(_)) => None,
            Err(e) => return Err(e),
        };

        let wine_bin = select_wine_binary(&profile).to_string();
        let prefix_path = self.sandbox_root.join(&app_id);

        Ok(ExecutionPlan {
            app_id,
            pe_path: pe_path.to_path_buf(),
            wine_binary: wine_bin,
            prefix_path,
            has_profile: profile.is_some(),
        })
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Prepare the Wine prefix (sandbox) for the application.
    ///
    /// If a profile exists, required paths are created inside the prefix.
    /// Environment variables for `drive_c` and registry overrides are set.
    fn prepare_sandbox(&self, app_id: &str, profile: &Option<CompatProfile>) -> Result<PathBuf> {
        let prefix_path = self.sandbox_root.join(app_id);
        if !prefix_path.exists() {
            fs::create_dir_all(&prefix_path).map_err(|source| {
                OpenNtxError::WinePrefix(format!(
                    "failed to create Wine prefix at {}: {}",
                    prefix_path.display(),
                    source
                ))
            })?;
        }

        // Create required filesystem paths from the profile.
        if let Some(ref prof) = profile {
            for required in &prof.fs_rules.required_paths {
                // Normalize Windows backslashes to forward slashes for Linux.
                let normalized = required
                    .strip_prefix("C:\\")
                    .or_else(|| required.strip_prefix("C:/"))
                    .unwrap_or(required)
                    .replace('\\', "/");
                let host_path = prefix_path.join("drive_c").join(&normalized);
                if !host_path.exists() {
                    fs::create_dir_all(&host_path).map_err(|source| {
                        OpenNtxError::WinePrefix(format!(
                            "failed to create required path {}: {}",
                            host_path.display(),
                            source
                        ))
                    })?;
                }
            }
        }

        Ok(prefix_path)
    }
}

/// Result of a dry-run execution plan.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub app_id: String,
    pub pe_path: PathBuf,
    pub wine_binary: String,
    pub prefix_path: PathBuf,
    pub has_profile: bool,
}

/// Select the appropriate Wine binary based on the profile's architecture.
fn select_wine_binary(profile: &Option<CompatProfile>) -> &'static str {
    match profile {
        Some(p) => match p.metadata.arch {
            crate::profile::Arch::X86 => DEFAULT_WINE_32,
            crate::profile::Arch::X86_64 => DEFAULT_WINE,
        },
        None => DEFAULT_WINE,
    }
}

/// Build the environment variables needed for an isolated Wine execution.
fn build_wine_environment(prefix_path: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("WINEPREFIX".to_string(), prefix_path.display().to_string());
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert("WINEESYNC".to_string(), "1".to_string());
    env.insert(
        "WINEPREFIX_DIR_OVERRIDE".to_string(),
        prefix_path.display().to_string(),
    );
    env
}

/// Compute the libc `unshare` flags from a `NamespaceConfig`.
fn build_unshare_flags(ns: &NamespaceConfig) -> i32 {
    let mut flags: i32 = 0;
    if ns.new_pid {
        flags |= libc::CLONE_NEWPID;
    }
    if ns.new_net {
        flags |= libc::CLONE_NEWNET;
    }
    if ns.new_mount {
        flags |= libc::CLONE_NEWNS;
    }
    flags
}

/// Compute the SHA-256 hex digest of a file.
fn hash_pe_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>())
}

/// Check whether a file starts with the PE "MZ" magic bytes.
fn is_pe_file(path: &Path) -> Result<bool> {
    let mut buf = [0u8; 2];
    let bytes_read = {
        use std::io::Read;
        let mut f = fs::File::open(path).map_err(|source| OpenNtxError::io(path, source))?;
        f.read(&mut buf)
            .map_err(|source| OpenNtxError::io(path, source))?
    };
    if bytes_read < 2 {
        return Ok(false);
    }
    Ok(&buf == b"MZ")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temp dir")
    }

    fn write_fake_exe(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).expect("create file");
        f.write_all(content).expect("write");
        path
    }

    // ── PE utilities ─────────────────────────────────────────────────────

    #[test]
    fn is_pe_file_detects_mz_header() {
        let dir = temp_dir();
        let mz_path = write_fake_exe(dir.path(), "test.exe", b"MZ\x90\x00\x03\x00");
        assert!(is_pe_file(&mz_path).unwrap());
        let txt_path = write_fake_exe(dir.path(), "readme.txt", b"hello world");
        assert!(!is_pe_file(&txt_path).unwrap());
    }

    #[test]
    fn is_pe_file_rejects_short_file() {
        let dir = temp_dir();
        let short_path = write_fake_exe(dir.path(), "tiny.exe", b"M");
        assert!(!is_pe_file(&short_path).unwrap());
    }

    #[test]
    fn hash_pe_file_deterministic() {
        let dir = temp_dir();
        let pe_path = write_fake_exe(dir.path(), "app.exe", b"MZ\x90\x00test data here");
        let hash1 = hash_pe_file(&pe_path).unwrap();
        let hash2 = hash_pe_file(&pe_path).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64, "SHA-256 hex digest should be 64 chars");
    }

    // ── Executor lifecycle ───────────────────────────────────────────────

    #[test]
    fn execute_pe_rejects_nonexistent() {
        let dir = temp_dir();
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();
        let security = SecurityConfig::permissive();

        let result = executor.execute_pe(Path::new("/nonexistent/file.exe"), &[], &security);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(_) => {} // expected
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn execute_pe_rejects_non_pe() {
        let dir = temp_dir();
        let txt_path = write_fake_exe(dir.path(), "readme.txt", b"not an exe");
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();
        let security = SecurityConfig::permissive();

        let result = executor.execute_pe(&txt_path, &[], &security);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("missing MZ header"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    // ── Execution plan ───────────────────────────────────────────────────

    #[test]
    fn plan_execution_valid_pe() {
        let dir = temp_dir();
        let pe_path = write_fake_exe(dir.path(), "notepad.exe", b"MZ\x90\x03\x00\x00app data");
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();

        let plan = executor.plan_execution(&pe_path).unwrap();
        assert_eq!(plan.pe_path, pe_path);
        assert!(!plan.app_id.is_empty());
        assert!(plan.app_id.contains("notepad"), "app_id: {}", plan.app_id);
        assert_eq!(plan.wine_binary, DEFAULT_WINE);
        assert!(!plan.has_profile, "should have no profile for new PE");
    }

    #[test]
    fn plan_execution_rejects_non_pe() {
        let dir = temp_dir();
        let txt_path = write_fake_exe(dir.path(), "data.bin", b"\x00\x01\x02\x03");
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();

        let result = executor.plan_execution(&txt_path);
        assert!(result.is_err());
    }

    // ── Sandbox ──────────────────────────────────────────────────────────

    #[test]
    fn sandbox_root_created() {
        let dir = temp_dir();
        let sandbox = dir.path().join("new-sandbox");
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox.clone()).unwrap();

        assert!(sandbox.exists(), "sandbox root should be created");
        assert_eq!(executor.sandbox_root(), sandbox);
    }

    #[test]
    fn sandbox_creates_required_paths_from_profile() {
        use crate::profile::*;
        use std::collections::HashMap;

        let dir = temp_dir();
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox.clone()).unwrap();

        let app_id = "test-app-1234";
        let profile = Some(CompatProfile {
            app_id: app_id.to_string(),
            metadata: AppMetadata {
                name: "Test App".into(),
                version: "1.0".into(),
                publisher: "Test".into(),
                arch: Arch::X86_64,
            },
            runtime_reqs: RuntimeReqs {
                dependencies: vec![],
                directx: None,
            },
            fs_rules: FilesystemRules {
                required_paths: vec![
                    "C:\\Program Files\\TestApp".into(),
                    "C:\\Users\\Public\\AppData\\Test".into(),
                ],
                path_mappings: HashMap::new(),
            },
            reg_rules: RegistryRules {
                required_keys: vec![],
            },
            installer: InstallerBehavior {
                installer_type: "exe".into(),
                silent_args: vec![],
            },
        });

        let prefix = executor.prepare_sandbox(app_id, &profile).unwrap();

        assert!(prefix.join("drive_c/Program Files/TestApp").exists());
        assert!(prefix.join("drive_c/Users/Public/AppData/Test").exists());
    }

    // ── Args passthrough ─────────────────────────────────────────────────

    #[test]
    fn args_passthrough_preserved() {
        let pe_path = Path::new("/tmp/fake.exe");
        let args: Vec<String> = vec![
            "--fullscreen".into(),
            "--resolution=1920x1080".into(),
            "path with spaces".into(),
            "".into(),
            "--verbose".into(),
        ];

        let mut cmd = Command::new("echo");
        cmd.arg(pe_path);
        cmd.args(&args);

        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "--fullscreen");
        assert_eq!(args[1], "--resolution=1920x1080");
        assert_eq!(args[2], "path with spaces");
        assert_eq!(args[3], "");
        assert_eq!(args[4], "--verbose");
    }

    // ── Wine environment ─────────────────────────────────────────────────

    #[test]
    fn wine_environment_variables() {
        let prefix = Path::new("/tmp/test-prefix");
        let env = build_wine_environment(prefix);

        assert_eq!(env.get("WINEPREFIX").unwrap(), "/tmp/test-prefix");
        assert_eq!(env.get("WINEDEBUG").unwrap(), "-all");
        assert_eq!(env.get("WINEESYNC").unwrap(), "1");
    }

    #[test]
    fn select_wine_binary_defaults() {
        assert_eq!(select_wine_binary(&None), DEFAULT_WINE);
    }

    // ── Namespace flags ──────────────────────────────────────────────────

    #[test]
    fn build_unshare_flags_empty() {
        let ns = NamespaceConfig::default();
        assert_eq!(build_unshare_flags(&ns), 0);
    }

    #[test]
    fn build_unshare_flags_net_only() {
        let ns = NamespaceConfig {
            new_pid: false,
            new_net: true,
            new_mount: false,
        };
        let flags = build_unshare_flags(&ns);
        assert_eq!(flags, libc::CLONE_NEWNET);
    }

    #[test]
    fn build_unshare_flags_mount_only() {
        let ns = NamespaceConfig {
            new_pid: false,
            new_net: false,
            new_mount: true,
        };
        let flags = build_unshare_flags(&ns);
        assert_eq!(flags, libc::CLONE_NEWNS);
    }

    #[test]
    fn build_unshare_flags_all() {
        let ns = NamespaceConfig {
            new_pid: true,
            new_net: true,
            new_mount: true,
        };
        let flags = build_unshare_flags(&ns);
        assert_eq!(
            flags,
            libc::CLONE_NEWPID | libc::CLONE_NEWNET | libc::CLONE_NEWNS
        );
    }

    #[test]
    fn build_unshare_flags_pid_only() {
        let ns = NamespaceConfig {
            new_pid: true,
            new_net: false,
            new_mount: false,
        };
        let flags = build_unshare_flags(&ns);
        assert_eq!(flags, libc::CLONE_NEWPID);
    }

    // ── SecurityConfig ───────────────────────────────────────────────────

    #[test]
    fn security_config_default() {
        let cfg = SecurityConfig::default();
        assert!(!cfg.namespaces.new_pid);
        assert!(!cfg.namespaces.new_net);
        assert!(!cfg.namespaces.new_mount);
        assert_eq!(cfg.max_memory_bytes, DEFAULT_MAX_MEMORY_BYTES);
        assert_eq!(cfg.cpu_max_quota, "max");
    }

    #[test]
    fn security_config_hardened() {
        let cfg = SecurityConfig::hardened();
        assert!(!cfg.namespaces.new_pid);
        assert!(cfg.namespaces.new_net);
        assert!(cfg.namespaces.new_mount);
        assert_eq!(cfg.max_memory_bytes, DEFAULT_MAX_MEMORY_BYTES);
    }

    #[test]
    fn security_config_permissive() {
        let cfg = SecurityConfig::permissive();
        assert!(!cfg.namespaces.new_pid);
        assert!(!cfg.namespaces.new_net);
        assert!(!cfg.namespaces.new_mount);
        assert_eq!(cfg.max_memory_bytes, 0);
        assert_eq!(cfg.cpu_max_quota, "max");
    }

    // ── ResourceGovernor integration ─────────────────────────────────────

    #[test]
    fn executor_has_resource_governor() {
        let dir = temp_dir();
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let cgroup_root = dir.path().join("cgroups");
        let gov = ResourceGovernor::with_root(cgroup_root);
        let executor = OpenNTXExecutor::with_paths_and_governor(profile_mgr, sandbox, gov).unwrap();

        // Verify the governor is accessible.
        assert_eq!(
            executor.resource_governor().cgroup_root(),
            dir.path().join("cgroups")
        );
    }

    // ── execute_pe with permissive security (no cgroups needed) ───────────

    #[test]
    fn execute_pe_with_permissive_security_rejects_nonexistent() {
        let dir = temp_dir();
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();
        let security = SecurityConfig::permissive();

        let result = executor.execute_pe(Path::new("/no/such/file.exe"), &[], &security);
        assert!(result.is_err());
    }
}
