// builder/mod.rs — Package builder subsystem for OpenNTX V1.4.
//
// Provides builders that take a `CompatProfile` (V1.2) and produce
// installable Linux packages (currently .deb for Debian/Ubuntu).

pub mod debian;

pub use debian::DebBuilder;
