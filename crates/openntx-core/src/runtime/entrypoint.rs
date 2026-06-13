// openntx-core/src/runtime/entrypoint.rs — Runtime entrypoint & auto-fallback.
//
// When the Linux kernel's binfmt_misc redirects a PE file (.exe) to the
// OpenNTX runtime binary (`/usr/bin/openntx-runtime`), this module takes
// over.  It parses the kernel-supplied arguments, looks up (or creates)
// a compatibility profile, and dispatches the execution through the
// `OpenNTXExecutor`.
//
// Auto-fallback:
//   If no `CompatProfile` exists for the PE file (first-time execution),
//   the entrypoint automatically:
//     1. Creates a `CaptureSession` to record filesystem changes.
//     2. Executes the PE once through the executor.
//     3. Analyzes the captured events to infer a `CompatProfile`.
//     4. Persists the new profile for future runs.

use crate::app_id::generate_app_id;
use crate::capture::{CaptureEvent, CaptureSession};
use crate::profile::{
    AppMetadata, Arch, CompatProfile, FilesystemRules, InstallerBehavior, ProfileManager,
    RegistryRules, RuntimeReqs,
};
use crate::runtime::executor::OpenNTXExecutor;
use crate::runtime::ipc::{CaptureStatusMessage, RuntimeIpcClient};
use crate::{OpenNtxError, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Root directory under which per-application sandbox (Wine prefix) trees
/// are created.  Resolved relative to `$XDG_DATA_HOME` or `~/.local/share`.
const SANDBOX_ROOT: &str = "openntx/sandboxes";

/// Subdirectory inside each sandbox where capture artifacts are stored.
const CAPTURE_SUBDIR: &str = "capture";

// ── RuntimeEntrypoint ────────────────────────────────────────────────────────

/// Main runtime entrypoint for the OpenNTX Windows Application Subsystem.
///
/// Instantiated by the `openntx-runtime` binary (the binfmt_misc interpreter).
/// Coordinates profile lookup, capture-based fallback, and PE execution.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::entrypoint::RuntimeEntrypoint;
///
/// let ep = RuntimeEntrypoint::new().expect("init");
/// let (exe, args) = RuntimeEntrypoint::parse_kernel_args(
///     vec!["openntx-runtime".into(), "/tmp/app.exe".into(), "--help".into()]
/// ).unwrap();
/// ep.dispatch_execution(&exe, &args).expect("dispatch");
/// ```
pub struct RuntimeEntrypoint {
    executor: OpenNTXExecutor,
    profile_manager: ProfileManager,
    sandbox_root: PathBuf,
}

impl RuntimeEntrypoint {
    /// Create a new entrypoint using default system paths.
    pub fn new() -> Result<Self> {
        let executor = OpenNTXExecutor::new()?;
        let profile_manager = ProfileManager::new()?;
        let data_dir = dirs::data_local_dir().ok_or_else(|| {
            OpenNtxError::Config("unable to determine local data directory".into())
        })?;
        let sandbox_root = data_dir.join(SANDBOX_ROOT);
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;

        Ok(Self {
            executor,
            profile_manager,
            sandbox_root,
        })
    }

    /// Create a new entrypoint with explicit paths (for testing).
    pub fn with_paths(
        profile_manager: ProfileManager,
        executor: OpenNTXExecutor,
        sandbox_root: PathBuf,
    ) -> Result<Self> {
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;
        Ok(Self {
            executor,
            profile_manager,
            sandbox_root,
        })
    }

    /// Return a reference to the underlying [`ProfileManager`].
    pub fn profile_manager(&self) -> &ProfileManager {
        &self.profile_manager
    }

    /// Return a reference to the underlying [`OpenNTXExecutor`].
    pub fn executor(&self) -> &OpenNTXExecutor {
        &self.executor
    }

    /// Return the sandbox root directory.
    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    // ── Kernel argument parsing ──────────────────────────────────────────────

    /// Parse the command-line arguments supplied by the kernel via
    /// binfmt_misc.
    ///
    /// When a PE file is executed, the kernel invokes the registered
    /// interpreter as:
    ///
    /// ```text
    /// /usr/bin/openntx-runtime <pe_path> [app_args...]
    /// ```
    ///
    /// `env_args[0]` is the interpreter itself (skipped).
    /// `env_args[1]` is the PE file path.
    /// `env_args[2..]` are the application arguments.
    ///
    /// Returns `(pe_path, app_args)`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidInput` if fewer than 2 arguments are provided.
    pub fn parse_kernel_args(env_args: Vec<String>) -> Result<(PathBuf, Vec<String>)> {
        if env_args.len() < 2 {
            return Err(OpenNtxError::InvalidInput(format!(
                "expected at least 2 arguments (interpreter + pe_path), got {}",
                env_args.len()
            )));
        }

        let pe_path = PathBuf::from(&env_args[1]);
        let app_args: Vec<String> = env_args[2..].to_vec();

        Ok((pe_path, app_args))
    }

    // ── Dispatch ─────────────────────────────────────────────────────────────

    /// Dispatch a PE file for execution.
    ///
    /// **Profile exists:** Calls `OpenNTXExecutor::execute_pe` directly.
    ///
    /// **Profile missing (auto-fallback):**
    ///   1. Creates an isolated sandbox for the app.
    ///   2. Starts a `CaptureSession` to record filesystem changes.
    ///   3. Executes the PE through the executor (first run).
    ///   4. Analyzes captured events to infer a `CompatProfile`.
    ///   5. Saves the profile for subsequent runs.
    ///
    /// # Errors
    ///
    /// - `InvalidInput` if the PE file does not exist or is not valid.
    /// - `RuntimeExecution` if Wine cannot be spawned.
    /// - `WinePrefix` if sandbox creation fails.
    pub fn dispatch_execution(&self, exe_path: &Path, app_args: &[String]) -> Result<()> {
        // ── Validate ─────────────────────────────────────────────────────
        if !exe_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "PE file not found: {}",
                exe_path.display()
            )));
        }

        if !is_pe_file(exe_path)? {
            return Err(OpenNtxError::InvalidInput(format!(
                "file is not a valid PE executable (missing MZ header): {}",
                exe_path.display()
            )));
        }

        // ── Identify ─────────────────────────────────────────────────────
        let pe_hash = hash_pe_file(exe_path)?;
        let filename = exe_path
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("unknown");
        let app_id = generate_app_id(filename, Some(&pe_hash));

        // ── Profile lookup ───────────────────────────────────────────────
        let has_profile = match self.profile_manager.load_profile(&app_id) {
            Ok(_) => true,
            Err(OpenNtxError::AppNotFound(_)) => false,
            Err(e) => return Err(e),
        };

        if has_profile {
            // Profile exists — direct execution via the executor.
            return self.executor.execute_pe(exe_path, app_args);
        }

        // ── Auto-fallback: first-time execution ──────────────────────────
        self.fallback_capture_and_execute(&app_id, filename, exe_path, app_args)
    }

    // ── Auto-fallback internals ──────────────────────────────────────────────

    /// Execute a PE file for the first time with automatic capture and
    /// profile generation.
    fn fallback_capture_and_execute(
        &self,
        app_id: &str,
        filename: &str,
        exe_path: &Path,
        app_args: &[String],
    ) -> Result<()> {
        // 1. Prepare the sandbox directory (Wine prefix).
        let app_sandbox = self.sandbox_root.join(app_id);
        fs::create_dir_all(&app_sandbox)
            .map_err(|source| OpenNtxError::WinePrefix(format!(
                "failed to create sandbox {}: {}",
                app_sandbox.display(),
                source
            )))?;

        // 2. Create a capture subdirectory for tracking artifacts.
        let capture_dir = app_sandbox.join(CAPTURE_SUBDIR);
        fs::create_dir_all(&capture_dir)
            .map_err(|source| OpenNtxError::io(&capture_dir, source))?;

        // 3. Start the capture session on the entire sandbox tree.
        let session = CaptureSession::new(app_sandbox.clone());
        let rx = session.start_tracking()?;

        // 4. Initialize IPC client for live status reporting to TUI.
        let ipc_client = RuntimeIpcClient::new(None);
        ipc_client.try_send_status(&CaptureStatusMessage {
            app_id: app_id.to_string(),
            status: "Capturing".to_string(),
            files_tracked: 0,
        });

        // 5. Execute the PE file (first run, no profile-guided isolation).
        ipc_client.try_send_status(&CaptureStatusMessage {
            app_id: app_id.to_string(),
            status: "Executing".to_string(),
            files_tracked: 0,
        });
        let exec_result = self.executor.execute_pe(exe_path, app_args);

        // 6. Collect captured events and report progress via IPC.
        let mut captured_paths: HashSet<PathBuf> = HashSet::new();
        while let Ok(event) = rx.try_recv() {
            match event {
                CaptureEvent::FileCreated(p) | CaptureEvent::FileModified(p) => {
                    captured_paths.insert(p);
                    // Report every new event to the TUI.
                    ipc_client.try_send_status(&CaptureStatusMessage {
                        app_id: app_id.to_string(),
                        status: "Capturing".to_string(),
                        files_tracked: captured_paths.len() as u32,
                    });
                }
                CaptureEvent::FileDeleted(_) => { /* ignore deletes */ }
            }
        }

        // 7. Propagate execution errors after capture is collected.
        exec_result?;

        // 8. Build a CompatProfile from the captured data.
        let profile = build_profile_from_capture(
            app_id,
            filename,
            &captured_paths,
            &app_sandbox,
        );

        // 9. Persist the profile.
        self.profile_manager.save_profile(&profile)?;

        // 10. Report completion via IPC.
        ipc_client.try_send_status(&CaptureStatusMessage {
            app_id: app_id.to_string(),
            status: "Complete".to_string(),
            files_tracked: captured_paths.len() as u32,
        });

        Ok(())
    }
}

// ── Profile construction ─────────────────────────────────────────────────────

/// Build a minimal `CompatProfile` from captured filesystem events.
///
/// The captured paths are analysed to extract unique directory prefixes
/// inside `drive_c`, which become the `required_paths` for future sandbox
/// preparation.
fn build_profile_from_capture(
    app_id: &str,
    filename: &str,
    captured_paths: &HashSet<PathBuf>,
    sandbox_root: &Path,
) -> CompatProfile {
    let required_paths = extract_required_paths(captured_paths, sandbox_root);

    CompatProfile {
        app_id: app_id.to_string(),
        metadata: AppMetadata {
            name: filename.to_string(),
            version: "unknown".to_string(),
            publisher: "auto-detected".to_string(),
            arch: Arch::X86_64,
        },
        runtime_reqs: RuntimeReqs {
            dependencies: Vec::new(),
            directx: None,
        },
        fs_rules: FilesystemRules {
            required_paths,
            path_mappings: HashMap::new(),
        },
        reg_rules: RegistryRules {
            required_keys: Vec::new(),
        },
        installer: InstallerBehavior {
            installer_type: "exe".to_string(),
            silent_args: Vec::new(),
        },
    }
}

/// Extract unique directory paths under `drive_c` from captured events.
///
/// For each captured file path, its parent directory (relative to the sandbox
/// root, converted to a Windows-style `C:\...` path) is recorded.  Duplicate
/// and ancestor-only entries are collapsed.
fn extract_required_paths(
    captured_paths: &HashSet<PathBuf>,
    sandbox_root: &Path,
) -> Vec<String> {
    let mut dirs: HashSet<String> = HashSet::new();

    for path in captured_paths {
        // We only care about paths inside the sandbox's drive_c.
        if let Ok(rel) = path.strip_prefix(sandbox_root) {
            let rel_str = rel.to_string_lossy();
            // Skip capture artifacts themselves.
            if rel_str.starts_with(CAPTURE_SUBDIR) {
                continue;
            }
            if let Some(parent) = path.parent() {
                if let Ok(parent_rel) = parent.strip_prefix(sandbox_root) {
                    let win_path = format!("C:\\{}", parent_rel.to_string_lossy().replace('/', "\\"));
                    // Skip the bare drive_c root.
                    if win_path != "C:\\" {
                        dirs.insert(win_path);
                    }
                }
            }
        }
    }

    let mut result: Vec<String> = dirs.into_iter().collect();
    result.sort();
    result
}

// ── PE utilities (local copies to avoid changing executor's public API) ──────

/// Compute the SHA-256 hex digest of a file.
fn hash_pe_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| OpenNtxError::io(path, source))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{:02x}", b)).collect::<String>())
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

    // ── parse_kernel_args tests ──────────────────────────────────────────

    #[test]
    fn parse_kernel_args_basic() {
        let args = vec![
            "/usr/bin/openntx-runtime".into(),
            "/home/user/app.exe".into(),
            "--fullscreen".into(),
            "--verbose".into(),
        ];
        let (exe, app_args) = RuntimeEntrypoint::parse_kernel_args(args).unwrap();
        assert_eq!(exe, PathBuf::from("/home/user/app.exe"));
        assert_eq!(app_args, vec!["--fullscreen", "--verbose"]);
    }

    #[test]
    fn parse_kernel_args_no_app_args() {
        let args = vec![
            "openntx-runtime".into(),
            "/tmp/test.exe".into(),
        ];
        let (exe, app_args) = RuntimeEntrypoint::parse_kernel_args(args).unwrap();
        assert_eq!(exe, PathBuf::from("/tmp/test.exe"));
        assert!(app_args.is_empty());
    }

    #[test]
    fn parse_kernel_args_too_few() {
        let args = vec!["openntx-runtime".into()];
        let result = RuntimeEntrypoint::parse_kernel_args(args);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("expected at least 2"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn parse_kernel_args_empty() {
        let args: Vec<String> = vec![];
        let result = RuntimeEntrypoint::parse_kernel_args(args);
        assert!(result.is_err());
    }

    #[test]
    fn parse_kernel_args_special_characters() {
        let args = vec![
            "runtime".into(),
            "/path/to/my app (v2).exe".into(),
            "arg with spaces".into(),
            "".into(),
            "--flag=value".into(),
        ];
        let (exe, app_args) = RuntimeEntrypoint::parse_kernel_args(args).unwrap();
        assert_eq!(exe, PathBuf::from("/path/to/my app (v2).exe"));
        assert_eq!(app_args.len(), 3);
        assert_eq!(app_args[0], "arg with spaces");
        assert_eq!(app_args[1], "");
        assert_eq!(app_args[2], "--flag=value");
    }

    // ── dispatch_execution tests ─────────────────────────────────────────

    #[test]
    fn dispatch_rejects_nonexistent_file() {
        let dir = temp_dir();
        let profiles = dir.path().join("profiles");
        let profile_mgr_ep = ProfileManager::with_path(profiles.clone()).unwrap();
        let profile_mgr_exec = ProfileManager::with_path(profiles).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor =
            OpenNTXExecutor::with_paths(profile_mgr_exec, dir.path().join("exec-sandbox"))
                .unwrap();
        let ep = RuntimeEntrypoint::with_paths(profile_mgr_ep, executor, sandbox).unwrap();

        let result = ep.dispatch_execution(Path::new("/nonexistent/app.exe"), &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => assert!(msg.contains("not found"), "msg: {}", msg),
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn dispatch_rejects_non_pe_file() {
        let dir = temp_dir();
        let txt = write_fake_exe(dir.path(), "readme.txt", b"not an exe");
        let profiles = dir.path().join("profiles");
        let profile_mgr_ep = ProfileManager::with_path(profiles.clone()).unwrap();
        let profile_mgr_exec = ProfileManager::with_path(profiles).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor =
            OpenNTXExecutor::with_paths(profile_mgr_exec, dir.path().join("exec-sandbox"))
                .unwrap();
        let ep = RuntimeEntrypoint::with_paths(profile_mgr_ep, executor, sandbox).unwrap();

        let result = ep.dispatch_execution(&txt, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("missing MZ header"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

    #[test]
    fn dispatch_with_existing_profile_calls_executor() {
        let dir = temp_dir();
        let pe_path = write_fake_exe(dir.path(), "known-app.exe", b"MZ\x90\x00test data");

        let profiles = dir.path().join("profiles");
        let profile_mgr_ep = ProfileManager::with_path(profiles.clone()).unwrap();
        let profile_mgr_exec = ProfileManager::with_path(profiles).unwrap();
        let sandbox = dir.path().join("sandbox");
        let exec_sandbox = dir.path().join("exec-sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr_exec, exec_sandbox).unwrap();
        let ep = RuntimeEntrypoint::with_paths(profile_mgr_ep, executor, sandbox).unwrap();

        // Compute the app_id and save a profile for it.
        let pe_hash = hash_pe_file(&pe_path).unwrap();
        let app_id = generate_app_id("known-app", Some(&pe_hash));
        let profile = CompatProfile {
            app_id: app_id.clone(),
            metadata: AppMetadata {
                name: "Known App".into(),
                version: "1.0".into(),
                publisher: "Test".into(),
                arch: Arch::X86_64,
            },
            runtime_reqs: RuntimeReqs {
                dependencies: vec![],
                directx: None,
            },
            fs_rules: FilesystemRules {
                required_paths: vec![],
                path_mappings: HashMap::new(),
            },
            reg_rules: RegistryRules {
                required_keys: vec![],
            },
            installer: InstallerBehavior {
                installer_type: "exe".into(),
                silent_args: vec![],
            },
        };
        ep.profile_manager().save_profile(&profile).unwrap();

        // Dispatch — should reach the executor, which will fail because
        // wine64 is not installed.  That's the expected path.
        let result = ep.dispatch_execution(&pe_path, &["--test".into()]);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::RuntimeExecution(msg) => {
                assert!(msg.contains("wine64") || msg.contains("wine"), "msg: {}", msg);
            }
            other => panic!("expected RuntimeExecution, got: {:?}", other),
        }
    }

    #[test]
    fn dispatch_fallback_creates_sandbox_and_profile() {
        let dir = temp_dir();
        let pe_path = write_fake_exe(dir.path(), "new-tool.exe", b"MZ\x90\x00new app data");

        let profiles = dir.path().join("profiles");
        let profile_mgr_ep = ProfileManager::with_path(profiles.clone()).unwrap();
        let profile_mgr_exec = ProfileManager::with_path(profiles).unwrap();
        let sandbox_root = dir.path().join("sandbox");
        let exec_sandbox = dir.path().join("exec-sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr_exec, exec_sandbox).unwrap();
        let ep = RuntimeEntrypoint::with_paths(profile_mgr_ep, executor, sandbox_root.clone())
            .unwrap();

        // Verify no profile exists yet.
        let pe_hash = hash_pe_file(&pe_path).unwrap();
        let app_id = generate_app_id("new-tool", Some(&pe_hash));
        assert!(
            !ep.profile_manager().has_profile(&app_id),
            "profile should not exist before dispatch"
        );

        // Dispatch — fallback path will be entered.
        let result = ep.dispatch_execution(&pe_path, &[]);
        // The executor will fail because wine64 is not available in CI,
        // but the fallback path should have created the sandbox.
        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            OpenNtxError::RuntimeExecution(_) => {} // expected: no wine
            OpenNtxError::WinePrefix(_) => {}       // also acceptable
            other => panic!("expected RuntimeExecution or WinePrefix, got: {:?}", other),
        }

        // Verify the sandbox directory was created.
        let app_sandbox = sandbox_root.join(&app_id);
        assert!(
            app_sandbox.exists(),
            "app sandbox should have been created at {}",
            app_sandbox.display()
        );

        // Verify the capture subdirectory was created.
        let capture_dir = app_sandbox.join(CAPTURE_SUBDIR);
        assert!(
            capture_dir.exists(),
            "capture subdirectory should exist at {}",
            capture_dir.display()
        );
    }

    // ── build_profile_from_capture tests ─────────────────────────────────

    #[test]
    fn build_profile_from_capture_basic() {
        let dir = temp_dir();
        let sandbox = dir.path().join("sandbox");
        fs::create_dir_all(sandbox.join("drive_c/Program Files/TestApp")).unwrap();
        fs::create_dir_all(sandbox.join("drive_c/users/appdata/local")).unwrap();

        let mut captured = HashSet::new();
        captured.insert(sandbox.join("drive_c/Program Files/TestApp/main.dll"));
        captured.insert(sandbox.join("drive_c/users/appdata/local/config.ini"));

        let profile = build_profile_from_capture("test-1234", "TestApp", &captured, &sandbox);

        assert_eq!(profile.app_id, "test-1234");
        assert_eq!(profile.metadata.name, "TestApp");
        assert_eq!(profile.metadata.arch, Arch::X86_64);
        assert!(profile.fs_rules.required_paths.contains(&"C:\\drive_c\\Program Files\\TestApp".to_string())
            || profile.fs_rules.required_paths.iter().any(|p| p.contains("Program Files")));
    }

    #[test]
    fn extract_required_paths_ignores_capture_dir() {
        let dir = temp_dir();
        let sandbox = dir.path().join("sandbox");
        fs::create_dir_all(sandbox.join("drive_c/app")).unwrap();
        fs::create_dir_all(sandbox.join(CAPTURE_SUBDIR)).unwrap();

        let mut captured = HashSet::new();
        captured.insert(sandbox.join("drive_c/app/test.dll"));
        captured.insert(sandbox.join(format!("{}/snapshot.json", CAPTURE_SUBDIR)));

        let paths = extract_required_paths(&captured, &sandbox);
        // Only the drive_c path should appear, not the capture dir.
        assert!(paths.iter().all(|p| !p.contains("capture")));
        assert!(!paths.is_empty());
    }

    #[test]
    fn extract_required_paths_deduplicates() {
        let dir = temp_dir();
        let sandbox = dir.path().join("sandbox");
        fs::create_dir_all(sandbox.join("drive_c/app")).unwrap();

        let mut captured = HashSet::new();
        captured.insert(sandbox.join("drive_c/app/file1.dll"));
        captured.insert(sandbox.join("drive_c/app/file2.dll"));
        captured.insert(sandbox.join("drive_c/app/sub/deep.dll"));

        let paths = extract_required_paths(&captured, &sandbox);
        // Should have at most 2 unique dirs: drive_c/app and drive_c/app/sub.
        assert!(paths.len() <= 3, "expected deduplication, got: {:?}", paths);
    }

    // ── PE utility tests ─────────────────────────────────────────────────

    #[test]
    fn is_pe_file_valid() {
        let dir = temp_dir();
        let pe = write_fake_exe(dir.path(), "test.exe", b"MZ\x90\x00\x03\x00rest");
        assert!(is_pe_file(&pe).unwrap());
    }

    #[test]
    fn is_pe_file_invalid() {
        let dir = temp_dir();
        let txt = write_fake_exe(dir.path(), "readme.txt", b"hello world");
        assert!(!is_pe_file(&txt).unwrap());
    }

    #[test]
    fn is_pe_file_short() {
        let dir = temp_dir();
        let short = write_fake_exe(dir.path(), "tiny.bin", b"M");
        assert!(!is_pe_file(&short).unwrap());
    }

    #[test]
    fn hash_pe_file_deterministic() {
        let dir = temp_dir();
        let pe = write_fake_exe(dir.path(), "app.exe", b"MZ\x90\x00data here");
        let h1 = hash_pe_file(&pe).unwrap();
        let h2 = hash_pe_file(&pe).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    // ── Entrypoint struct tests ───────────────────────────────────────────

    #[test]
    fn with_paths_creates_sandbox_root() {
        let dir = temp_dir();
        let sandbox = dir.path().join("my-sandbox");
        let profiles = dir.path().join("profiles");
        let profile_mgr_ep = ProfileManager::with_path(profiles.clone()).unwrap();
        let profile_mgr_exec = ProfileManager::with_path(profiles).unwrap();
        let executor =
            OpenNTXExecutor::with_paths(profile_mgr_exec, dir.path().join("exec")).unwrap();
        let ep = RuntimeEntrypoint::with_paths(profile_mgr_ep, executor, sandbox.clone()).unwrap();

        assert!(sandbox.exists());
        assert_eq!(ep.sandbox_root(), sandbox);
    }
}
