//! One compile-time identity for every artifact emitted by this binary.
//!
//! `build.rs` binds this value to the package version, source state and compilation configuration,
//! or to a validated release/CI override. Keeping the value here prevents snapshots, measurements
//! and experiment results from quietly defining incompatible notions of "the same build".

/// Exact identity of the binary's source and compilation inputs.
pub const BUILD_ID: &str = env!("ANIMA_BUILD_ID");

/// Owned form retained for the public experiment-runner API and serialized provenance fields.
pub fn build_id() -> String {
    BUILD_ID.to_owned()
}
