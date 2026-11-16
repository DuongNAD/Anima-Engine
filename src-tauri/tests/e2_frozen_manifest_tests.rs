//! The E2 manifests in `tests/fixtures/experiments_e2/` are the preregistration, and E2 has run.
//!
//! The run copied every manifest it used beside its results, under
//! `artifacts/experiments/e2-evolved-brain-default/{manifests,smoke/manifests}/`, and
//! `npm run check:e2` (`scripts/verify_e2_artifacts.mjs`) proves those copies are the committed
//! files, byte for byte. That check runs in the frontend CI job only. A backend change that edits a
//! fixture to keep a Rust test green therefore passes `cargo test` and breaks the evidence in a job
//! its author never looks at.
//!
//! Commit `970d963` did exactly that. It added `live.dormant_eu` to the four registered manifests a
//! month after the 2026-07-27 run, and `check:e2` stayed red on every later tree. The owner's
//! decision QD-E2M (2026-11-16) settled it: amend the preregistration with a dated block
//! (`amendment_1_live_dormant_eu`) and re-run the whole ensemble against the amended manifests.
//! The July artifacts were moved to `superseded/` and still carry their own checksums. This file
//! makes the same comparison as `check:e2`, in the place where a Rust change is tested, so the next
//! drift fails here first.
//!
//! # When a test here fails
//!
//! Never edit anything under `artifacts/`: the run's copies, `checksums.sha256` and the results are
//! evidence, and making them agree with an edited fixture would falsify it. The repository has to
//! hold the manifests the run used, so restore the fixture from the run's own copy. If the edit was
//! made to request a new live observable, the observable belongs in
//! `LIVE_EXTENDED_OBSERVABLE_IDS`; `LIVE_OBSERVABLE_IDS` is the frozen list these manifests request.
//! Changing what a registered experiment measures is a new amendment plus a real re-run, and that
//! decision belongs to the owner.

use std::path::{Path, PathBuf};

/// The committed preregistration, relative to the crate root.
const FIXTURE_DIR: &str = "tests/fixtures/experiments_e2";

/// The published E2 artifact directory, relative to the crate root.
const RUN_DIR: &str = "../artifacts/experiments/e2-evolved-brain-default";

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every file the run copied into `<RUN_DIR>/<sub>`, as `(name, path)`, sorted by name.
fn run_copies(sub: &str) -> Vec<(String, PathBuf)> {
    let dir = crate_root().join(RUN_DIR).join(sub);
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            let entry = entry.unwrap_or_else(|e| panic!("entry in {}: {e}", dir.display()));
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            )
        })
        .collect();
    out.sort();
    out
}

/// The first line on which two byte strings differ, for a failure message a person can act on.
fn first_difference(run_copy: &[u8], fixture: &[u8]) -> String {
    let (a, b) = (
        String::from_utf8_lossy(run_copy),
        String::from_utf8_lossy(fixture),
    );
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return format!(
                "line {}: the run used {la:?}, the fixture has {lb:?}",
                i + 1
            );
        }
    }
    format!(
        "the run copy has {} lines and the fixture {}, identical up to the shorter",
        a.lines().count(),
        b.lines().count()
    )
}

fn assert_fixtures_are_the_run_copies(sub: &str, expected: &[&str]) {
    let copies = run_copies(sub);
    let names: Vec<&str> = copies.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names, expected,
        "the run's {sub}/ holds a different set of files than this test pins; nothing may be \
         added to or removed from a published artifact"
    );

    for (name, run_copy) in &copies {
        let fixture = crate_root().join(FIXTURE_DIR).join(name);
        let ran =
            std::fs::read(run_copy).unwrap_or_else(|e| panic!("read {}: {e}", run_copy.display()));
        let committed =
            std::fs::read(&fixture).unwrap_or_else(|e| panic!("read {}: {e}", fixture.display()));
        assert!(
            ran == committed,
            "{FIXTURE_DIR}/{name} is not the manifest the E2 run used ({RUN_DIR}/{sub}/{name}): \
             {}. Restore the fixture from the run's copy; never edit the copy. A new live \
             observable goes in LIVE_EXTENDED_OBSERVABLE_IDS.",
            first_difference(&ran, &committed)
        );
    }
}

#[test]
fn the_ensemble_manifests_are_the_ones_the_e2_ensemble_used() {
    assert_fixtures_are_the_run_copies(
        "manifests",
        &[
            "e2-control-shared-brain.json",
            "e2-preregistration.json",
            "e2-treatment-evolved-brain.json",
        ],
    );
}

#[test]
fn the_smoke_manifests_are_the_ones_the_e2_smoke_calibration_used() {
    assert_fixtures_are_the_run_copies(
        "smoke/manifests",
        &[
            "e2-preregistration.json",
            "e2-smoke-control-shared-brain.json",
            "e2-smoke-treatment-evolved-brain.json",
        ],
    );
}
