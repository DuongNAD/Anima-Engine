//! Regenerate the committed AE-210 manifest fixtures.
//!
//! ```text
//! cargo run --example gen_ae_fixtures
//! ```
//!
//! This is a tool, not a test, and that distinction is the point. It used to be a `#[ignore]`d test
//! that wrote into the repository: it guarded nothing on an ordinary run, and it could only help
//! when somebody remembered to type `--ignored`. Meanwhile the fixtures had already gone stale —
//! they predated ADR-0004's `observer` field, and every other AE-210 test still passed because a
//! `#[serde(default)]` field makes a stale fixture parse perfectly.
//!
//! The guard is now `core::experiment::tests::ae210_fixtures_match_the_serializer`, which fails the
//! moment the committed bytes stop matching the serializer. This is what you run to make them match
//! again after an *intentional* schema change. Nothing in the ordinary test suite writes here.
//!
//! LF line endings on every platform, because the test compares bytes.

use anima_engine_lib::core::experiment::ae210_reference_manifests;

/// Where the fixtures live, relative to the crate root. Duplicated in the test rather than exported
/// from the library: a manifest does not know where it is filed.
const FIXTURE_DIR: &str = "tests/fixtures/experiments";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR);
    std::fs::create_dir_all(&dir)?;

    for (name, manifest) in ae210_reference_manifests() {
        let path = dir.join(name);
        // Pretty JSON plus the trailing newline every text file in this repository ends with.
        let text = serde_json::to_string_pretty(&manifest)? + "\n";
        std::fs::write(&path, text.as_bytes())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
