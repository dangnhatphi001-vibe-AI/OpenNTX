// profile/mod.rs — Compatibility Profile subsystem for OpenNTX V1.2.
//
// Stores per-application compatibility configuration (runtime requirements,
// filesystem / registry rules, installer behaviour) as JSON files under
// `~/.local/share/openntx/profiles/`.

pub mod manager;
pub mod schema;

pub use manager::ProfileManager;
pub use schema::{
    AppMetadata, Arch, CompatProfile, FilesystemRules, InstallerBehavior, RegistryRules,
    RuntimeReqs,
};
