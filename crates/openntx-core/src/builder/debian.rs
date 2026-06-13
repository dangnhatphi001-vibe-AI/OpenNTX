// builder/debian.rs — Debian .deb package builder driven by CompatProfile.
//
// Takes a `CompatProfile` (from the V1.2 profile database) and produces a
// standards-compliant `.deb` package that installs the Windows application
// into `/opt/openntx/apps/<app_id>/` and registers a native Linux desktop
// launcher so the app appears in the system application menu.
//
// Build lifecycle:
//   1. `new(profile)`          — store the profile.
//   2. `prepare_workspace()`   — create staging directory tree.
//   3. `generate_control_file()` — write `DEBIAN/control`.
//   4. `generate_desktop_entry()` — write `.desktop` launcher.
//   5. `build_deb()`           — invoke `dpkg-deb --build`, return the `.deb` path.
//
// The caller is responsible for copying application files (drive_c, registry,
// etc.) into the workspace between steps 2 and 5.  The builder only generates
// the Debian metadata and invokes the system toolchain.

use crate::profile::CompatProfile;
use crate::{OpenNtxError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Public API ───────────────────────────────────────────────────────────────

/// Debian .deb package builder.
///
/// # Example
///
/// ```no_run
/// use openntx_core::builder::DebBuilder;
/// use openntx_core::profile::ProfileManager;
///
/// let mgr = ProfileManager::new().unwrap();
/// let profile = mgr.load_profile("notepadpp-v8.6").unwrap();
/// let builder = DebBuilder::new(profile);
///
/// let wspace = builder.prepare_workspace().unwrap();
/// // … copy app files into wspace/opt/openntx/apps/<app_id>/drive_c/ …
/// builder.generate_control_file(&wspace).unwrap();
/// builder.generate_desktop_entry(&wspace).unwrap();
/// let deb_path = builder.build_deb(&wspace).unwrap();
/// println!("built: {}", deb_path.display());
/// ```
pub struct DebBuilder {
    profile: CompatProfile,
    package_name: String,
    version: String,
    output_dir: PathBuf,
}

impl DebBuilder {
    /// Create a new builder for the given compatibility profile.
    ///
    /// The package name is derived as `openntx-<app_id>`.  Version defaults
    /// to the version reported in the profile metadata.
    pub fn new(profile: CompatProfile) -> Self {
        let package_name = format!("openntx-{}", profile.app_id);
        let version = profile.metadata.version.clone();
        Self {
            profile,
            package_name,
            version,
            output_dir: PathBuf::from("dist"),
        }
    }

    /// Override the output directory for the final `.deb` file.
    ///
    /// Defaults to `./dist/` if not called.
    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = dir.into();
        self
    }

    /// Override the package version.
    ///
    /// Defaults to the version from `profile.metadata.version`.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// The package name (e.g. `openntx-notepadpp-v8.6`).
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    /// The final `.deb` filename (e.g. `openntx-notepadpp-v8.6_8.6_amd64.deb`).
    pub fn deb_filename(&self) -> String {
        format!("{}_{}_amd64.deb", self.package_name, self.version)
    }

    /// The profile this builder operates on.
    pub fn profile(&self) -> &CompatProfile {
        &self.profile
    }

    // ── Step 2: prepare workspace ────────────────────────────────────────────

    /// Create a staging workspace with the Debian-standard directory layout.
    ///
    /// Returns the path to the workspace root.  The caller should copy
    /// application files into the returned tree before calling
    /// [`generate_control_file`] and [`build_deb`].
    ///
    /// Layout produced:
    ///
    /// ```text
    /// /tmp/openntx_builder_<app_id>/
    ///   DEBIAN/                              ← control file goes here
    ///   opt/openntx/apps/<app_id>/
    ///     drive_c/                           ← Windows app files go here
    ///   usr/share/applications/              ← .desktop entry goes here
    /// ```
    ///
    /// If the directory already exists it is removed first to guarantee a
    /// clean workspace.
    pub fn prepare_workspace(&self) -> Result<PathBuf> {
        let app_id = &self.profile.app_id;
        let wspace = std::env::temp_dir().join(format!("openntx_builder_{app_id}"));

        // Clean slate — remove stale workspace if present.
        if wspace.exists() {
            fs::remove_dir_all(&wspace)
                .map_err(|source| OpenNtxError::io(&wspace, source))?;
        }

        // DEBIAN/
        let debian_dir = wspace.join("DEBIAN");
        fs::create_dir_all(&debian_dir)
            .map_err(|source| OpenNtxError::io(&debian_dir, source))?;

        // opt/openntx/apps/<app_id>/drive_c/
        let app_dir = wspace.join("opt/openntx/apps").join(app_id).join("drive_c");
        fs::create_dir_all(&app_dir)
            .map_err(|source| OpenNtxError::io(&app_dir, source))?;

        // usr/share/applications/
        let desktop_dir = wspace.join("usr/share/applications");
        fs::create_dir_all(&desktop_dir)
            .map_err(|source| OpenNtxError::io(&desktop_dir, source))?;

        Ok(wspace)
    }

    // ── Step 3: generate control file ────────────────────────────────────────

    /// Write the `DEBIAN/control` file into the workspace.
    ///
    /// Fields generated:
    /// - `Package` — `openntx-<app_id>`
    /// - `Version` — from profile metadata
    /// - `Architecture` — `amd64` (or `i386` if the profile arch is X86)
    /// - `Maintainer` — `OpenNTX <openntx@localhost>`
    /// - `Description` — derived from profile metadata
    /// - `Depends` — `openntx-cli` (the runtime CLI)
    ///
    /// # Errors
    ///
    /// Returns [`OpenNtxError::Io`] if the `DEBIAN/` directory is missing or
    /// the file cannot be written.
    pub fn generate_control_file(&self, wspace: &Path) -> Result<()> {
        let control_path = wspace.join("DEBIAN/control");

        let arch = match self.profile.metadata.arch {
            crate::profile::Arch::X86 => "i386",
            crate::profile::Arch::X86_64 => "amd64",
        };

        let description = format!(
            "{} {} by {} — managed Windows application via OpenNTX",
            self.profile.metadata.name,
            self.profile.metadata.version,
            self.profile.metadata.publisher,
        );

        let content = format!(
            concat!(
                "Package: {package}\n",
                "Version: {version}\n",
                "Section: misc\n",
                "Priority: optional\n",
                "Architecture: {arch}\n",
                "Depends: openntx-cli\n",
                "Maintainer: OpenNTX <openntx@localhost>\n",
                "Description: {desc}\n",
                " This package was built by OpenNTX from a Compatibility Profile.\n",
                " It contains a Windows application installed under\n",
                " /opt/openntx/apps/ and a native .desktop launcher.\n",
                " Run with: openntx run {app_id}\n",
            ),
            package = self.package_name,
            version = self.version,
            arch = arch,
            desc = description,
            app_id = self.profile.app_id,
        );

        fs::write(&control_path, content.as_bytes())
            .map_err(|source| OpenNtxError::io(&control_path, source))?;

        Ok(())
    }

    // ── Step 4: generate desktop entry ───────────────────────────────────────

    /// Write a `.desktop` launcher file into the workspace.
    ///
    /// The launcher is installed to
    /// `usr/share/applications/openntx-<app_id>.desktop` inside the staging
    /// tree.  When the `.deb` is installed on the target system, the Windows
    /// application will appear in the system application menu.
    ///
    /// The `Exec` field points to `openntx run <app_id>` so the OpenNTX CLI
    /// handles the actual launch.
    ///
    /// # Errors
    ///
    /// Returns [`OpenNtxError::Io`] if the file cannot be written.
    pub fn generate_desktop_entry(&self, wspace: &Path) -> Result<()> {
        let app_id = &self.profile.app_id;
        let name = &self.profile.metadata.name;
        let publisher = &self.profile.metadata.publisher;

        let desktop_path = wspace
            .join("usr/share/applications")
            .join(format!("openntx-{app_id}.desktop"));

        let content = format!(
            concat!(
                "[Desktop Entry]\n",
                "Type=Application\n",
                "Name={name}\n",
                "GenericName=Windows Application\n",
                "Comment={name} — managed by OpenNTX\n",
                "Exec=openntx run {app_id}\n",
                "Icon=application-x-executable\n",
                "Terminal=false\n",
                "Categories=X-Windows;\n",
                "X-OpenNTX-AppId={app_id}\n",
                "X-OpenNTX-Publisher={publisher}\n",
            ),
            name = name,
            app_id = app_id,
            publisher = publisher,
        );

        fs::write(&desktop_path, content.as_bytes())
            .map_err(|source| OpenNtxError::io(&desktop_path, source))?;

        Ok(())
    }

    // ── Step 5: build .deb ───────────────────────────────────────────────────

    /// Invoke `dpkg-deb --build` on the workspace and produce the final
    /// `.deb` archive.
    ///
    /// The `.deb` is first built in the system temp directory (so that Unix
    /// permission normalisation works even on non-POSIX filesystems), then
    /// moved to [`output_dir`](Self::with_output_dir).
    ///
    /// On **success** the staging workspace is automatically removed.
    /// On **failure** the workspace is preserved for debugging.
    ///
    /// # Returns
    ///
    /// The absolute path to the built `.deb` file.
    ///
    /// # Errors
    ///
    /// - [`OpenNtxError::InvalidInput`] if `dpkg-deb` is not installed or
    ///   exits with a non-zero status.
    /// - [`OpenNtxError::Io`] for filesystem errors.
    pub fn build_deb(&self, wspace: &Path) -> Result<PathBuf> {
        // Ensure the workspace exists and has a DEBIAN/control file.
        let control_path = wspace.join("DEBIAN/control");
        if !control_path.exists() {
            return Err(OpenNtxError::InvalidInput(format!(
                "workspace is missing DEBIAN/control: {}",
                wspace.display()
            )));
        }

        // Build the .deb in the temp directory first.
        let deb_filename = self.deb_filename();
        let temp_deb = wspace.with_extension("deb");

        let wspace_str = wspace.to_str().ok_or_else(|| {
            OpenNtxError::InvalidInput("workspace path is not valid UTF-8".to_string())
        })?;
        let temp_deb_str = temp_deb.to_str().ok_or_else(|| {
            OpenNtxError::InvalidInput("temp deb path is not valid UTF-8".to_string())
        })?;

        let status = Command::new("dpkg-deb")
            .args(["--root-owner-group", "--build", wspace_str, temp_deb_str])
            .status();

        match status {
            Ok(exit) if exit.success() => { /* ok */ }
            Ok(exit) => {
                // Preserve workspace for debugging.
                return Err(OpenNtxError::InvalidInput(format!(
                    "dpkg-deb exited with status: {exit}"
                )));
            }
            Err(err) => {
                return Err(OpenNtxError::InvalidInput(format!(
                    "dpkg-deb is not available or failed to run: {err}. \
                     Install dpkg-dev to build .deb packages."
                )));
            }
        }

        // Move the .deb to the output directory.
        fs::create_dir_all(&self.output_dir)
            .map_err(|source| OpenNtxError::io(&self.output_dir, source))?;

        let output_deb = self.output_dir.join(&deb_filename);
        fs::rename(&temp_deb, &output_deb).or_else(|_| {
            // Cross-filesystem fallback: copy + remove.
            fs::copy(&temp_deb, &output_deb)
                .map_err(|source| OpenNtxError::io(&output_deb, source))?;
            fs::remove_file(&temp_deb).map_err(|source| OpenNtxError::io(&temp_deb, source))
        })?;

        // Clean up the staging workspace on success.
        let _ = fs::remove_dir_all(wspace);

        Ok(output_deb)
    }

    // ── Convenience: full build in one call ──────────────────────────────────

    /// Run the complete build pipeline in a single call.
    ///
    /// Equivalent to calling `prepare_workspace` → `generate_control_file` →
    /// `generate_desktop_entry` → `build_deb` in sequence.  The workspace is
    /// cleaned up automatically on success; preserved on failure.
    ///
    /// **Note:** This does *not* copy application files (drive_c, registry,
    /// etc.) into the workspace.  The caller must do that between
    /// `prepare_workspace` and `build_deb`, or use the step-by-step API for
    /// full control.
    ///
    /// Returns the path to the built `.deb` file.
    pub fn build(&self) -> Result<PathBuf> {
        let wspace = self.prepare_workspace()?;
        self.generate_control_file(&wspace)?;
        self.generate_desktop_entry(&wspace)?;
        self.build_deb(&wspace)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::*;
    use std::collections::HashMap;

    fn sample_profile(app_id: &str) -> CompatProfile {
        CompatProfile {
            app_id: app_id.to_string(),
            metadata: AppMetadata {
                name: "Test App".to_string(),
                version: "2.1.0".to_string(),
                publisher: "Test Publisher".to_string(),
                arch: Arch::X86_64,
            },
            runtime_reqs: RuntimeReqs {
                dependencies: vec!["vcrun2019".to_string()],
                directx: Some("dx11".to_string()),
            },
            fs_rules: FilesystemRules {
                required_paths: vec!["C:/Program Files/TestApp".to_string()],
                path_mappings: HashMap::new(),
            },
            reg_rules: RegistryRules {
                required_keys: vec!["HKCU\\Software\\TestApp".to_string()],
            },
            installer: InstallerBehavior {
                installer_type: "nsis".to_string(),
                silent_args: vec!["/S".to_string()],
            },
        }
    }

    #[test]
    fn prepare_workspace_creates_structure() {
        let profile = sample_profile("ws-test");
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare_workspace");

        assert!(wspace.join("DEBIAN").is_dir());
        assert!(wspace
            .join("opt/openntx/apps/ws-test/drive_c")
            .is_dir());
        assert!(wspace.join("usr/share/applications").is_dir());

        // Clean up
        let _ = fs::remove_dir_all(&wspace);
    }

    #[test]
    fn prepare_workspace_is_idempotent() {
        let profile = sample_profile("idempotent-test");
        let builder = DebBuilder::new(profile);

        let w1 = builder.prepare_workspace().expect("first call");
        let w2 = builder.prepare_workspace().expect("second call");
        assert_eq!(w1, w2);

        let _ = fs::remove_dir_all(&w1);
    }

    #[test]
    fn generate_control_file_content() {
        let profile = sample_profile("ctrl-test");
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare");

        builder
            .generate_control_file(&wspace)
            .expect("generate_control_file");

        let content =
            fs::read_to_string(wspace.join("DEBIAN/control")).expect("read control");

        assert!(content.contains("Package: openntx-ctrl-test"));
        assert!(content.contains("Version: 2.1.0"));
        assert!(content.contains("Architecture: amd64"));
        assert!(content.contains("Depends: openntx-cli"));
        assert!(content.contains("Maintainer: OpenNTX"));
        assert!(content.contains("Test App"));
        assert!(content.contains("openntx run ctrl-test"));

        let _ = fs::remove_dir_all(&wspace);
    }

    #[test]
    fn generate_control_file_x86_arch() {
        let mut profile = sample_profile("x86-test");
        profile.metadata.arch = Arch::X86;
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare");

        builder
            .generate_control_file(&wspace)
            .expect("generate_control_file");

        let content =
            fs::read_to_string(wspace.join("DEBIAN/control")).expect("read control");

        assert!(content.contains("Architecture: i386"));

        let _ = fs::remove_dir_all(&wspace);
    }

    #[test]
    fn generate_desktop_entry_content() {
        let profile = sample_profile("desktop-test");
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare");

        builder
            .generate_desktop_entry(&wspace)
            .expect("generate_desktop_entry");

        let desktop_path = wspace
            .join("usr/share/applications")
            .join("openntx-desktop-test.desktop");
        assert!(desktop_path.exists());

        let content = fs::read_to_string(&desktop_path).expect("read desktop");

        assert!(content.contains("Type=Application"));
        assert!(content.contains("Name=Test App"));
        assert!(content.contains("Exec=openntx run desktop-test"));
        assert!(content.contains("X-OpenNTX-AppId=desktop-test"));
        assert!(content.contains("X-OpenNTX-Publisher=Test Publisher"));

        let _ = fs::remove_dir_all(&wspace);
    }

    #[test]
    fn build_deb_fails_without_control() {
        let profile = sample_profile("no-ctrl");
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare");

        // Remove the DEBIAN directory to simulate a missing control file.
        let _ = fs::remove_dir_all(wspace.join("DEBIAN"));

        let err = builder.build_deb(&wspace).unwrap_err();
        assert!(matches!(err, OpenNtxError::InvalidInput(_)));
        assert!(format!("{err}").contains("DEBIAN/control"));

        let _ = fs::remove_dir_all(&wspace);
    }

    #[test]
    fn package_name_and_deb_filename() {
        let profile = sample_filename_profile();
        let builder = DebBuilder::new(profile).with_version("1.0.0");

        assert_eq!(builder.package_name(), "openntx-my-app");
        assert_eq!(builder.deb_filename(), "openntx-my-app_1.0.0_amd64.deb");
    }

    fn sample_filename_profile() -> CompatProfile {
        CompatProfile {
            app_id: "my-app".to_string(),
            metadata: AppMetadata {
                name: "My App".to_string(),
                version: "0.1.0".to_string(),
                publisher: "Me".to_string(),
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
                installer_type: "exe".to_string(),
                silent_args: vec![],
            },
        }
    }

    #[test]
    fn with_output_dir_changes_location() {
        let profile = sample_profile("out-dir-test");
        let builder = DebBuilder::new(profile).with_output_dir("/tmp/openntx-test-output");

        assert_eq!(builder.output_dir, PathBuf::from("/tmp/openntx-test-output"));
    }

    #[test]
    fn with_version_overrides_metadata() {
        let profile = sample_profile("ver-test");
        let builder = DebBuilder::new(profile).with_version("99.0.0");

        assert_eq!(builder.version, "99.0.0");
        assert_eq!(builder.deb_filename(), "openntx-ver-test_99.0.0_amd64.deb");
    }

    #[test]
    fn build_deb_requires_dpkg_deb() {
        // This test verifies the error path when dpkg-deb is not available
        // or the workspace has no files to package.  We set up a valid
        // control file so the error comes from dpkg-deb itself, not our
        // validation.
        let profile = sample_profile("dpkg-missing");
        let builder = DebBuilder::new(profile);
        let wspace = builder.prepare_workspace().expect("prepare");

        builder
            .generate_control_file(&wspace)
            .expect("control file");

        // Try to build — dpkg-deb may or may not be installed.
        // Either way, the function should not panic.
        let result = builder.build_deb(&wspace);
        match result {
            Ok(path) => {
                // dpkg-deb is installed and build succeeded.
                assert!(path.exists());
                let _ = fs::remove_file(&path);
            }
            Err(OpenNtxError::InvalidInput(msg)) => {
                // dpkg-deb returned an error or is not installed.
                assert!(
                    msg.contains("dpkg-deb"),
                    "error should mention dpkg-deb: {msg}"
                );
            }
            Err(e) => panic!("unexpected error type: {e}"),
        }

        // Workspace may or may not exist depending on success/failure.
        let _ = fs::remove_dir_all(&wspace);
    }
}
