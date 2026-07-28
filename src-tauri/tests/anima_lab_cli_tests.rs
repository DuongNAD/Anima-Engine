use anima_engine_lib::core::experiment::ExperimentManifest;
use anima_engine_lib::core::experiment_runner::EnsembleSummary;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
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
