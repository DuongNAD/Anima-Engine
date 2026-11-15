use anima_engine_lib::core::experiment::ExperimentManifest;
use anima_engine_lib::core::experiment_runner::EnsembleSummary;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempManifest(PathBuf);

impl Drop for TempManifest {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn runtime_failure_manifest() -> TempManifest {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments/baseline-no-exotic.json");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture is readable"))
            .expect("fixture is a manifest");

    manifest.seeds = vec![1];
    manifest.duration_ticks = 60;
    manifest.sample_period = 60;
    assert!(
        manifest
            .initial_conditions
            .values
            .iter()
            .all(|(name, _)| name != "npp"),
        "the fixture should exercise the default NPP path before this test overrides it"
    );
    manifest
        .initial_conditions
        .values
        .push(("npp".to_string(), f64::MAX));

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "anima-lab-runtime-failure-{}-{nonce}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&manifest).expect("manifest serialises"),
    )
    .expect("temporary manifest is writable");
    TempManifest(path)
}

fn live_success_manifest() -> TempManifest {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture is readable"))
            .expect("fixture is a manifest");
    manifest.duration_ticks = 2;
    manifest.sample_period = 1;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "anima-lab-live-success-{}-{nonce}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&manifest).expect("manifest serialises"),
    )
    .expect("temporary manifest is writable");
    TempManifest(path)
}

/// `baseline-no-exotic.json` cut down to one seed, as the pre-2026-10-22 unit tests did.
fn shortened_fixture_manifest(label: &str, duration_ticks: u64) -> TempManifest {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments/baseline-no-exotic.json");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(fixture).expect("fixture is readable"))
            .expect("fixture is a manifest");
    manifest.seeds.truncate(1);
    manifest.duration_ticks = duration_ticks;
    manifest.sample_period = duration_ticks;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "anima-lab-{label}-{}-{nonce}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&manifest).expect("manifest serialises"),
    )
    .expect("temporary manifest is writable");
    TempManifest(path)
}

#[test]
fn a_preserved_runtime_failure_is_json_and_a_nonzero_process_exit() {
    let manifest = runtime_failure_manifest();
    let output = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg(&manifest.0)
        .output()
        .expect("anima-lab starts");

    let report: Value =
        serde_json::from_slice(&output.stdout).expect("stdout remains a machine-readable report");
    let typed = serde_json::from_slice::<EnsembleSummary>(&output.stdout)
        .expect("the emitted report must round-trip through its public Rust schema");
    assert!(
        typed.runs[0]
            .final_observables
            .iter()
            .any(|(_, value)| !value.is_finite()),
        "the value that caused the runtime failure must remain explicit"
    );
    assert_eq!(report["completed_runs"], 0);
    assert_eq!(report["failed"].as_array().map(Vec::len), Some(1));
    assert!(
        !output.status.success(),
        "a failed run must fail the shell command; report={report}"
    );
    assert!(
        output.stderr.is_empty(),
        "a preserved runtime result belongs in JSON, not stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_live_manifest_runs_through_the_headless_cli() {
    let manifest = live_success_manifest();

    let wrong_model = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg(&manifest.0)
        .output()
        .expect("anima-lab starts");
    assert!(
        !wrong_model.status.success(),
        "a live manifest must not silently run against the default reference model"
    );
    assert!(wrong_model.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&wrong_model.stderr).contains("ensemble rejected"),
        "{}",
        String::from_utf8_lossy(&wrong_model.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["--model", "live"])
        .arg(&manifest.0)
        .output()
        .expect("anima-lab starts");

    assert!(
        output.status.success(),
        "live run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be a machine-readable report");
    assert_eq!(report["model"], "live");
    assert_eq!(
        report["registry"],
        "core::experiment::ObservableRegistry::live_default"
    );
    assert_eq!(report["completed_runs"], 1);
    assert!(report["failed"].as_array().is_some_and(Vec::is_empty));
    assert!(
        report["runs"][0]["final_observables"]
            .as_array()
            .and_then(|values| { values.iter().find(|entry| entry[0] == "live.agent_count") })
            .and_then(|entry| entry[1].as_f64())
            .is_some_and(|agents| agents > 0.0),
        "LiveExperimentAdapter must produce a non-empty population: {report}"
    );
}

/// Process-level form of `a_committed_fixture_produces_a_machine_readable_report`, one of the four
/// `anima-lab` unit tests deleted by 4e618b9. The original called `render_to_writer` inside the
/// binary; an integration test cannot reach a binary's private functions, so this runs the binary.
/// 600 ticks establishes the report's shape; the numbers are the runner's tests to make.
#[test]
fn a_committed_fixture_produces_a_machine_readable_report() {
    let manifest = shortened_fixture_manifest("fixture-report", 600);
    let output = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg(&manifest.0)
        .output()
        .expect("anima-lab starts");

    assert!(
        output.status.success(),
        "the fixture run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: Value = serde_json::from_slice(&output.stdout).expect("stdout must be JSON");
    assert_eq!(v["tool"], "anima-lab");
    assert_eq!(v["model"], "reference");
    assert_eq!(
        v["registry"],
        "core::experiment::ObservableRegistry::reference_default"
    );
    assert_eq!(v["experiment_id"], "exp-1");
    assert_eq!(v["completed_runs"], 1);
    assert_eq!(v["runs"].as_array().map(Vec::len), Some(1));
    assert!(
        v["failed"].as_array().unwrap().is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(v["runs"][0]["series"].is_array(), "the series survives");
}

/// Process-level form of `a_closed_stdout_is_a_reported_error_not_a_panic`, also deleted by
/// 4e618b9. The original handed `render_to_writer` a writer that always fails. Here the read end of
/// stdout is closed before the child can finish, so its write fails with a broken pipe: the child
/// must report that and exit 1, not panic (101) and not die of a signal (no exit code).
#[test]
fn a_closed_stdout_is_a_reported_error_not_a_panic() {
    let manifest = shortened_fixture_manifest("closed-stdout", 1);
    let mut child = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg(&manifest.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("anima-lab starts");
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("anima-lab exits");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "a closed stdout must not panic: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "a closed stdout is a failed command, not a signal or a panic: {stderr}"
    );
    assert!(stderr.contains("anima-lab:"), "{stderr}");
    assert!(
        stderr.contains("Broken pipe"),
        "the reason must be the closed pipe: {stderr}"
    );
}
