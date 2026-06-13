// openntx-core/src/runtime/binfmt.rs — binfmt_misc subsystem registration.
//
// Registers the PE (MZ) executable format with the Linux kernel's binfmt_misc
// facility so that running `./something.exe` transparently invokes the
// OpenNTX runtime (Wine / Proton headless).
//
// Registration string:
//   `:OpenNTX:M::MZ::/usr/bin/openntx-runtime:OC`
//
// Unregistration:
//   Write `-1` to `/proc/sys/fs/binfmt_misc/OpenNTX`.
//
// Both operations require root (UID == 0).

use crate::{OpenNtxError, Result};
use std::fs;
use std::path::Path;

/// Well-known name used when registering with binfmt_misc.
pub const BINFMT_NAME: &str = "OpenNTX";

/// Path to the binfmt_misc register pseudo-file.
const BINFMT_REGISTER: &str = "/proc/sys/fs/binfmt_misc/register";

/// Default path to the OpenNTX runtime shim.
const DEFAULT_RUNTIME_PATH: &str = "/usr/bin/openntx-runtime";

/// The magic bytes at the start of every PE file ("MZ").
const PE_MAGIC: &[u8] = b"MZ";

/// `BinfmtManager` handles registration and unregistration of the OpenNTX
/// PE subsystem with the Linux kernel's binfmt_misc interface.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::binfmt::BinfmtManager;
///
/// let mgr = BinfmtManager::new();
/// mgr.register_subsystem().expect("register");
/// // ... later ...
/// mgr.unregister_subsystem().expect("unregister");
/// ```
pub struct BinfmtManager {
    /// Path to the OpenNTX runtime shim that will be invoked by the kernel.
    runtime_path: String,
}

impl BinfmtManager {
    /// Create a new `BinfmtManager` with the default runtime path
    /// (`/usr/bin/openntx-runtime`).
    pub fn new() -> Self {
        Self {
            runtime_path: DEFAULT_RUNTIME_PATH.to_string(),
        }
    }

    /// Create a new `BinfmtManager` with a custom runtime shim path.
    pub fn with_runtime_path(runtime_path: impl Into<String>) -> Self {
        Self {
            runtime_path: runtime_path.into(),
        }
    }

    /// Return the runtime shim path this manager will use.
    pub fn runtime_path(&self) -> &str {
        &self.runtime_path
    }

    /// Build the binfmt_misc registration string for the PE format.
    ///
    /// Format: `:<name>:<type>:<extension>:<magic>:<offset>:<interpreter>:<flags>`
    ///
    /// For OpenNTX PE registration:
    /// ```text
    /// :OpenNTX:M::MZ::/usr/bin/openntx-runtime:OC
    /// ```
    ///
    /// - `M` = match by magic bytes
    /// - `MZ` = the DOS/PE magic header
    /// - `OC` = credentials override (C) + open for read (O)
    pub fn build_registration_string(&self) -> String {
        format!(
            ":{}:M::MZ::{}:OC",
            BINFMT_NAME, self.runtime_path
        )
    }

    /// Register the OpenNTX PE subsystem with binfmt_misc.
    ///
    /// Writes the registration string to `/proc/sys/fs/binfmt_misc/register`.
    /// Requires root privileges (UID == 0).
    ///
    /// # Errors
    ///
    /// - `PermissionDenied` if the current process is not running as root.
    /// - `BinfmtRegistration` if the write to the register file fails.
    pub fn register_subsystem(&self) -> Result<()> {
        require_root()?;

        let reg_string = self.build_registration_string();
        let register_path = Path::new(BINFMT_REGISTER);

        if !register_path.exists() {
            return Err(OpenNtxError::BinfmtRegistration(
                "binfmt_misc filesystem not mounted at /proc/sys/fs/binfmt_misc".to_string(),
            ));
        }

        fs::write(register_path, reg_string.as_bytes()).map_err(|source| {
            OpenNtxError::BinfmtRegistration(format!(
                "failed to write registration string to {}: {}",
                BINFMT_REGISTER, source
            ))
        })?;

        Ok(())
    }

    /// Unregister the OpenNTX PE subsystem from binfmt_misc.
    ///
    /// Writes `-1` to `/proc/sys/fs/binfmt_misc/OpenNTX`.
    /// Requires root privileges (UID == 0).
    ///
    /// # Errors
    ///
    /// - `PermissionDenied` if the current process is not running as root.
    /// - `BinfmtUnregistration` if the unregistration file does not exist or
    ///   the write fails.
    pub fn unregister_subsystem(&self) -> Result<()> {
        require_root()?;

        let unregister_path = format!("/proc/sys/fs/binfmt_misc/{}", BINFMT_NAME);
        let path = Path::new(&unregister_path);

        if !path.exists() {
            return Err(OpenNtxError::BinfmtUnregistration(format!(
                "binfmt entry '{}' not found at {} — nothing to unregister",
                BINFMT_NAME, unregister_path
            )));
        }

        fs::write(path, b"-1").map_err(|source| {
            OpenNtxError::BinfmtUnregistration(format!(
                "failed to write -1 to {}: {}",
                unregister_path, source
            ))
        })?;

        Ok(())
    }

    /// Check whether the OpenNTX binfmt entry is currently registered.
    ///
    /// Returns `true` if `/proc/sys/fs/binfmt_misc/OpenNTX` exists.
    /// Does **not** require root privileges.
    pub fn is_registered(&self) -> bool {
        let path = format!("/proc/sys/fs/binfmt_misc/{}", BINFMT_NAME);
        Path::new(&path).exists()
    }
}

impl Default for BinfmtManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify the current process is running as root (UID == 0).
///
/// # Errors
///
/// Returns `PermissionDenied` if the effective UID is not 0.
fn require_root() -> Result<()> {
    let uid = unsafe { libc::geteuid() };
    if uid != 0 {
        return Err(OpenNtxError::PermissionDenied(format!(
            "binfmt_misc operations require root privileges (current euid: {})",
            uid
        )));
    }
    Ok(())
}

/// Return the raw PE magic bytes used for binfmt registration.
pub fn pe_magic_bytes() -> &'static [u8] {
    PE_MAGIC
}

/// Return the full registration string for a given runtime path.
///
/// This is a standalone helper that mirrors [`BinfmtManager::build_registration_string`]
/// but does not require instantiating a manager.
pub fn build_registration_string(runtime_path: &str) -> String {
    format!(":{}:M::MZ::{}:OC", BINFMT_NAME, runtime_path)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_string_format() {
        let mgr = BinfmtManager::new();
        let reg = mgr.build_registration_string();
        assert_eq!(reg, ":OpenNTX:M::MZ::/usr/bin/openntx-runtime:OC");
    }

    #[test]
    fn registration_string_custom_runtime() {
        let mgr = BinfmtManager::with_runtime_path("/opt/openntx/bin/runtime");
        let reg = mgr.build_registration_string();
        assert_eq!(reg, ":OpenNTX:M::MZ::/opt/openntx/bin/runtime:OC");
    }

    #[test]
    fn registration_string_has_mz_magic() {
        let mgr = BinfmtManager::new();
        let reg = mgr.build_registration_string();
        assert!(
            reg.contains("::MZ::"),
            "registration string must contain MZ magic: {}",
            reg
        );
    }

    #[test]
    fn registration_string_has_correct_flags() {
        let mgr = BinfmtManager::new();
        let reg = mgr.build_registration_string();
        assert!(
            reg.ends_with(":OC"),
            "registration string must end with OC flags: {}",
            reg
        );
    }

    #[test]
    fn pe_magic_is_mz() {
        assert_eq!(pe_magic_bytes(), b"MZ");
        assert_eq!(pe_magic_bytes().len(), 2);
    }

    #[test]
    fn build_standalone_registration_string() {
        let reg = build_registration_string("/usr/bin/openntx-runtime");
        assert_eq!(reg, ":OpenNTX:M::MZ::/usr/bin/openntx-runtime:OC");
    }

    #[test]
    fn binfmt_name_is_openntx() {
        assert_eq!(BINFMT_NAME, "OpenNTX");
    }

    #[test]
    fn default_manager_has_standard_runtime_path() {
        let mgr = BinfmtManager::new();
        assert_eq!(mgr.runtime_path(), "/usr/bin/openntx-runtime");
    }

    #[test]
    fn default_trait_works() {
        let mgr = BinfmtManager::default();
        assert_eq!(mgr.runtime_path(), "/usr/bin/openntx-runtime");
    }

    #[test]
    fn register_fails_when_not_root() {
        // This test verifies the root-check logic.  In CI we are almost
        // certainly not root, so the call must fail with PermissionDenied.
        // If the test happens to run as root (e.g. in a container), we skip
        // the assertion.
        let uid = unsafe { libc::geteuid() };
        if uid == 0 {
            // Running as root — skip this particular check; the actual
            // binfmt_misc filesystem may or may not be available.
            return;
        }

        let mgr = BinfmtManager::new();
        let result = mgr.register_subsystem();
        assert!(result.is_err(), "should fail when not root");
        match result.unwrap_err() {
            OpenNtxError::PermissionDenied(_) => {} // expected
            other => panic!("expected PermissionDenied, got: {:?}", other),
        }
    }

    #[test]
    fn unregister_fails_when_not_root() {
        let uid = unsafe { libc::geteuid() };
        if uid == 0 {
            return;
        }

        let mgr = BinfmtManager::new();
        let result = mgr.unregister_subsystem();
        assert!(result.is_err(), "should fail when not root");
        match result.unwrap_err() {
            OpenNtxError::PermissionDenied(_) => {} // expected
            other => panic!("expected PermissionDenied, got: {:?}", other),
        }
    }

    #[test]
    fn registration_string_field_count() {
        // The binfmt registration format is colon-separated:
        //   :name:type:extension:magic:offset:interpreter:flags
        // = 8 fields (7 colons)
        let mgr = BinfmtManager::new();
        let reg = mgr.build_registration_string();
        let colon_count = reg.matches(':').count();
        assert_eq!(
            colon_count, 7,
            "expected 7 colons in registration string, got {}: {}",
            colon_count, reg
        );
    }
}
