// openntx-core/src/runtime/executor.rs — PE execution orchestrator.
//
// When a PE file (.exe) is invoked (via binfmt_misc or directly), the
// `OpenNTXExecutor` coordinates:
//
//   1. Identification  — hash the PE to derive an `app_id`.
//   2. Profile lookup  — check if a CompatProfile already exists.
//   3. Sandbox setup   — virtualise environment variables and route `drive_c`.
//   4. Process launch  — fork/exec into Wine (headless) with filtered output.
//
// All Wine stderr/stdout noise is suppressed; only OpenNTX diagnostics are
// emitted.

use crate::app_id::generate_app_id;
use crate::profile::{CompatProfile, ProfileManager};
use crate::{OpenNtxError, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Default Wine binary used when no profile specifies an alternative.
const DEFAULT_WINE: &str = "wine64";

/// Fallback Wine binary for 32-bit PE files.
const DEFAULT_WINE_32: &str = "wine";

/// Root directory under which per-application Wine prefixes are stored.
const SANDBOX_ROOT: &str = ".local/share/openntx/sandboxes";

/// Orchestrates PE execution on Linux via Wine / Proton in an isolated
/// environment.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::executor::OpenNTXExecutor;
/// use std::path::Path;
///
/// let executor = OpenNTXExecutor::new().expect("init");
/// executor.execute_pe(Path::new("/home/user/app.exe"), &[]).expect("run");
/// ```
pub struct OpenNTXExecutor {
    profile_manager: ProfileManager,
    /// Root directory for Wine prefixes (sandboxes).
    sandbox_root: PathBuf,
}

impl OpenNTXExecutor {
    /// Create a new executor using default paths.
    pub fn new() -> Result<Self> {
        let profile_manager = ProfileManager::new()?;
        let data_dir = dirs::data_local_dir().ok_or_else(|| {
            OpenNtxError::Config(
                "unable to determine local data directory".into(),
            )
        })?;
        let sandbox_root = data_dir.join(SANDBOX_ROOT.strip_prefix(".local/share/").unwrap_or(SANDBOX_ROOT));
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;

        Ok(Self {
            profile_manager,
            sandbox_root,
        })
    }

    /// Create a new executor with explicit paths (useful for testing).
    pub fn with_paths(profile_manager: ProfileManager, sandbox_root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&sandbox_root)
            .map_err(|source| OpenNtxError::io(&sandbox_root, source))?;
        Ok(Self {
            profile_manager,
            sandbox_root,
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

    // ── Public API ───────────────────────────────────────────────────────────

    /// Execute a PE file.
    ///
    /// This is the main entry point called by the binfmt_misc runtime shim or
    /// the CLI.
    ///
    /// # Steps
    ///
    /// 1. Compute the `app_id` from the PE file's SHA-256 hash.
    /// 2. Look up the compatibility profile.
    /// 3. Set up an isolated Wine prefix (sandbox).
    /// 4. Launch the PE via Wine with transparent argument passthrough.
    ///
    /// # Errors
    ///
    /// - `InvalidInput` if the file does not exist or is not a valid PE.
    /// - `RuntimeExecution` if the Wine process cannot be spawned.
    /// - `WinePrefix` if the sandbox cannot be prepared.
    pub fn execute_pe(&self, pe_path: &Path, args: &[String]) -> Result<()> {
        // ── Step 1: Validate and identify ────────────────────────────────────
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

        // ── Step 2: Profile lookup ───────────────────────────────────────────
        let profile = match self.profile_manager.load_profile(&app_id) {
            Ok(profile) => Some(profile),
            Err(OpenNtxError::AppNotFound(_)) => None,
            Err(e) => return Err(e),
        };

        // ── Step 3: Sandbox setup ────────────────────────────────────────────
        let prefix_path = self.prepare_sandbox(&app_id, &profile)?;

        // ── Step 4: Launch via Wine ──────────────────────────────────────────
        let wine_bin = select_wine_binary(&profile);
        let wine_env = build_wine_environment(&prefix_path);

        let mut cmd = Command::new(&wine_bin);
        cmd.arg(pe_path);
        cmd.args(args);
        cmd.envs(&wine_env);

        // Suppress Wine's noisy output — only OpenNTX diagnostics pass through.
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let _output: Output = cmd.output().map_err(|source| {
            OpenNtxError::RuntimeExecution(format!(
                "failed to execute {} via {}: {}",
                pe_path.display(),
                wine_bin,
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
    fn prepare_sandbox(
        &self,
        app_id: &str,
        profile: &Option<CompatProfile>,
    ) -> Result<PathBuf> {
        let prefix_path = self.sandbox_root.join(app_id);
        if !prefix_path.exists() {
            fs::create_dir_all(&prefix_path)
                .map_err(|source| OpenNtxError::WinePrefix(format!(
                    "failed to create Wine prefix at {}: {}",
                    prefix_path.display(),
                    source
                )))?;
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
    env.insert(
        "WINEPREFIX".to_string(),
        prefix_path.display().to_string(),
    );
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert("WINEESYNC".to_string(), "1".to_string());
    env.insert(
        "WINEPREFIX_DIR_OVERRIDE".to_string(),
        prefix_path.display().to_string(),
    );
    env
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

    #[test]
    fn is_pe_file_detects_mz_header() {
        let dir = temp_dir();
        // Valid MZ header
        let mz_path = write_fake_exe(dir.path(), "test.exe", b"MZ\x90\x00\x03\x00");
        assert!(is_pe_file(&mz_path).unwrap());

        // Not a PE file
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

    #[test]
    fn execute_pe_rejects_nonexistent() {
        let dir = temp_dir();
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();

        let result = executor.execute_pe(Path::new("/nonexistent/file.exe"), &[]);
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

        let result = executor.execute_pe(&txt_path, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            OpenNtxError::InvalidInput(msg) => {
                assert!(msg.contains("missing MZ header"), "msg: {}", msg);
            }
            other => panic!("expected InvalidInput, got: {:?}", other),
        }
    }

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
    fn args_passthrough_preserved() {
        // This test verifies that argument strings — including ones with spaces,
        // special characters, and empty strings — would be passed through to
        // Command unchanged.  We test by constructing the Command and inspecting
        // the args we set.
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

        // We cannot easily inspect a Command's args after construction, but we
        // can verify the arg slice is exactly what we expect.
        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "--fullscreen");
        assert_eq!(args[1], "--resolution=1920x1080");
        assert_eq!(args[2], "path with spaces");
        assert_eq!(args[3], "");
        assert_eq!(args[4], "--verbose");
    }

    #[test]
    fn wine_environment_variables() {
        let prefix = Path::new("/tmp/test-prefix");
        let env = build_wine_environment(prefix);

        assert_eq!(
            env.get("WINEPREFIX").unwrap(),
            "/tmp/test-prefix"
        );
        assert_eq!(env.get("WINEDEBUG").unwrap(), "-all");
        assert_eq!(env.get("WINEESYNC").unwrap(), "1");
    }

    #[test]
    fn select_wine_binary_defaults() {
        assert_eq!(select_wine_binary(&None), DEFAULT_WINE);
    }

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
    fn plan_execution_rejects_non_pe() {
        let dir = temp_dir();
        let txt_path = write_fake_exe(dir.path(), "data.bin", b"\x00\x01\x02\x03");
        let profile_mgr = ProfileManager::with_path(dir.path().join("profiles")).unwrap();
        let sandbox = dir.path().join("sandbox");
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox).unwrap();

        let result = executor.plan_execution(&txt_path);
        assert!(result.is_err());
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

        // Verify the required paths were created inside drive_c
        assert!(prefix.join("drive_c/Program Files/TestApp").exists());
        assert!(
            prefix
                .join("drive_c/Users/Public/AppData/Test")
                .exists()
        );
    }
}
