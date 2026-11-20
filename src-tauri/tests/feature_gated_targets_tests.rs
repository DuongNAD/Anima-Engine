//! The Cargo target metadata and the crate-level feature gates say the same thing.
//!
//! Eight integration targets cannot exist without an optional subsystem, and that fact is written
//! down twice: once as `#![cfg(feature = "...")]` at the top of the file, and once as
//! `required-features` in `Cargo.toml`. Neither alone is enough.
//!
//! - **Without `required-features`**, Cargo still builds the target under `--no-default-features`.
//!   The `#![cfg]` empties it, so the binary compiles, reports `running 0 tests`, and exits 0. Eight
//!   green targets over 2,040 lines that never ran — the exact failure `check_test_targets.mjs`
//!   exists to catch, and the reason it was written.
//! - **Without the `#![cfg]`**, a `required-features` entry that someone deletes or mistypes turns
//!   the file into a compile error under the wrong feature set rather than an empty pass. Louder,
//!   but still a break.
//!
//! Two statements of one fact drift. This test is the thing that makes them agree, and it runs in
//! both feature configurations because it reads source files rather than linking anything.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Workspace crates that own feature-gated `tests/*.rs` targets (`src-tauri` + `anima-worldgen`).
fn gated_workspace_roots() -> [(&'static str, PathBuf); 2] {
    let root = crate_root();
    [
        ("networking", root.clone()),
        ("generate", root.join("crates/anima-worldgen")),
    ]
}

/// Every `tests/*.rs` file that opens with a crate-level `#![cfg(feature = "X")]`, mapped to `X`.
///
/// Only the crate-level form counts. A `#[cfg(feature = ...)]` on an individual test is a different
/// thing — it removes one test from a target that still exists and still runs — and it is not what
/// `required-features` describes.
fn feature_gated_test_files(root: &Path) -> BTreeMap<String, String> {
    let dir = root.join("tests");
    let mut found = BTreeMap::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // `#![cfg(feature = "networking")]` — inner attribute, so it must be at the top of the file
        // before any item. Scanning the first handful of lines keeps this from matching a string
        // literal buried in a test body.
        for line in text.lines().take(12) {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("#![cfg(feature = \"") else {
                continue;
            };
            let Some(feature) = rest.strip_suffix("\")]") else {
                continue;
            };
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("utf-8 file stem")
                .to_string();
            found.insert(name, feature.to_string());
            break;
        }
    }
    found
}

/// Every `[[test]]` block in `Cargo.toml` that declares `required-features`, mapped to the single
/// feature it requires.
///
/// A deliberately small hand-rolled reader rather than a TOML dependency: this crate has no TOML
/// parser in its dependency tree, and adding one so a test can read one table would be a heavier
/// change than the thing it verifies. The shape it accepts is the shape the file is written in, and
/// `every_required_features_entry_is_well_formed` fails if that stops being true.
fn cargo_required_features(root: &Path) -> BTreeMap<String, Vec<String>> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest_path.display()));
    let mut found = BTreeMap::new();
    let mut name: Option<String> = None;
    let mut in_test_block = false;
    let mut reading_features = false;
    let mut feature_buf = String::new();

    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') && !reading_features {
            in_test_block = line == "[[test]]";
            name = None;
            continue;
        }
        if !in_test_block {
            continue;
        }
        if let Some(v) = line.strip_prefix("name = \"") {
            name = v.strip_suffix('"').map(str::to_string);
        }
        if let Some(v) = line.strip_prefix("required-features = [") {
            reading_features = true;
            feature_buf.clear();
            feature_buf.push_str(v);
        } else if reading_features {
            feature_buf.push(' ');
            feature_buf.push_str(line);
        }
        if reading_features && feature_buf.contains(']') {
            let inside = feature_buf.split(']').next().unwrap_or("");
            let features: Vec<String> = inside
                .split(',')
                .map(|f| f.trim().trim_matches('"').to_string())
                .filter(|f| !f.is_empty())
                .collect();
            let target = name
                .clone()
                .expect("a [[test]] block must name its target before requiring features");
            found.insert(target, features);
            reading_features = false;
        }
    }
    found
}

#[test]
fn every_feature_gated_test_file_is_declared_with_required_features() {
    for (_, root) in gated_workspace_roots() {
        let files = feature_gated_test_files(&root);
        let declared = cargo_required_features(&root);

        assert!(
            !files.is_empty(),
            "no crate-level `#![cfg(feature = ...)]` test file was found in {} — either the scan \
             broke or the gated targets were deleted. Both need a human.",
            root.display()
        );

        for (target, feature) in &files {
            let required = declared.get(target).unwrap_or_else(|| {
                panic!(
                    "{}/tests/{target}.rs is gated on feature `{feature}` but Cargo.toml has no \
                     `[[test]] name = \"{target}\"` with `required-features`. Without it, \
                     `--no-default-features` builds the target into an empty binary that passes.",
                    root.display()
                )
            });
            assert_eq!(
                required,
                &vec![feature.clone()],
                "{}/tests/{target}.rs is gated on `{feature}` but Cargo.toml requires {required:?}",
                root.display()
            );
        }
    }
}

#[test]
fn every_required_features_entry_has_a_matching_gated_file() {
    for (_, root) in gated_workspace_roots() {
        let files = feature_gated_test_files(&root);
        let declared = cargo_required_features(&root);

        assert!(
            !declared.is_empty(),
            "{}/Cargo.toml declares no `required-features` test target — the hand-rolled reader \
             in this file probably stopped matching the manifest's formatting",
            root.display()
        );

        for (target, required) in &declared {
            let path = root.join("tests").join(format!("{target}.rs"));
            assert!(
                path.exists(),
                "{}/Cargo.toml declares `[[test]] name = \"{target}\"` requiring {required:?}, \
                 but {} does not exist",
                root.display(),
                path.display()
            );
            let feature = files.get(target).unwrap_or_else(|| {
                panic!(
                    "{}/Cargo.toml requires {required:?} for `{target}`, but tests/{target}.rs \
                     has no crate-level `#![cfg(feature = ...)]`. The two statements have drifted.",
                    root.display()
                )
            });
            assert_eq!(
                required,
                &vec![feature.clone()],
                "{}: `{target}` requires {required:?} in Cargo.toml and `{feature}` in its source",
                root.display()
            );
        }
    }
}

#[test]
fn every_required_feature_is_one_this_crate_actually_declares() {
    // A typo in a feature name is silent in the worst possible way: Cargo simply never schedules
    // the target, in any configuration, and the tests vanish without a message.
    for (sentinel_feature, root) in gated_workspace_roots() {
        let manifest_path = root.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", manifest_path.display()));
        let mut declared_features: Vec<String> = Vec::new();
        let mut in_features = false;
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_features = trimmed == "[features]";
                continue;
            }
            if !in_features || trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some((name, _)) = trimmed.split_once('=') {
                declared_features.push(name.trim().to_string());
            }
        }
        assert!(
            declared_features.contains(&sentinel_feature.to_string()),
            "{}/Cargo.toml [features] reader found {declared_features:?}, missing `{sentinel_feature}`",
            root.display()
        );

        for (target, required) in cargo_required_features(&root) {
            for feature in required {
                assert!(
                    declared_features.contains(&feature),
                    "{}: test target `{target}` requires feature `{feature}`, which [features] \
                     does not declare. Cargo would silently never build this target.",
                    root.display()
                );
            }
        }
    }
}

#[test]
fn the_gated_targets_are_the_ones_the_policy_gate_knows_about() {
    // `scripts/test_target_policy.mjs` asserts these are absent when their feature is off and
    // present under their owning profile. If a new gated target appears here and not there, the
    // policy gate would let it silently vanish from a default run.
    let policy = crate_root()
        .parent()
        .expect("repo root")
        .join("scripts/test_target_policy.mjs");
    let text = std::fs::read_to_string(&policy)
        .unwrap_or_else(|e| panic!("read {}: {e}", policy.display()));
    for (_, root) in gated_workspace_roots() {
        for (target, feature) in feature_gated_test_files(&root) {
            assert!(
                text.contains(&format!("tests/{target}.rs")),
                "{}/tests/{target}.rs is gated on `{feature}` but {} does not list it",
                root.display(),
                policy.display()
            );
        }
    }
}
