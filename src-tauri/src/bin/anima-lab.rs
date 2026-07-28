//! `anima-lab` — run an experiment manifest headlessly and print the result as JSON.
//!
//! Parses arguments, loads a manifest, hands it to [`run_ensemble`] over [`ReferenceEvolutionWorld`]
//! and serialises the [`EnsembleSummary`] it returns; the science stays in `core::experiment_runner`.
//! stdout carries JSON and nothing else. Errors go to stderr and exit non-zero — a run that fails
//! *inside* a valid ensemble is reported in the JSON, not on stderr, and still makes the process
//! exit non-zero so automation cannot mistake a partial result for success.
//!
//! Reference manifests only (`tests/fixtures/experiments/`). The live-Bevy manifests in
//! `experiments_e2/` need `LiveExperimentAdapter` and `ObservableRegistry::live_default`, which is a
//! `--model` flag and two more match arms whenever someone needs to run them from a shell.

use std::path::PathBuf;
use std::process::ExitCode;

use anima_engine_lib::core::experiment::{ExperimentManifest, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{run_ensemble, EnsembleSummary};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use serde::Serialize;

const USAGE: &str = "\
anima-lab — run an experiment manifest headlessly and print JSON to stdout

    anima-lab <MANIFEST.json>
    anima-lab --help

Every seed the manifest declares is run, in the declared order, against ReferenceEvolutionWorld.
Exit 0 when every run completes. Exit 1 with JSON on stdout for preserved runtime failures, or with
a reason on stderr when no report can be produced.
";

/// Parse `argv` (without the program name). `Ok(None)` means help was requested.
fn parse_args(argv: &[String]) -> Result<Option<PathBuf>, String> {
    let mut manifest: Option<PathBuf> = None;
    for arg in argv {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            other if other.starts_with('-') => return Err(format!("unknown option '{other}'")),
            other if manifest.is_none() => manifest = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected extra argument '{other}'")),
        }
    }
    manifest.map(Some).ok_or("missing <MANIFEST.json>".into())
}

/// The stdout envelope: what was run, flattened over the runner's own summary rather than rebuilt
/// from it, so no field can be dropped by going stale here.
#[derive(Serialize)]
struct Report<'a> {
    tool: &'static str,
    manifest: &'a str,
    model: &'static str,
    #[serde(flatten)]
    summary: &'a EnsembleSummary,
}

#[derive(Debug)]
struct CommandOutput {
    json: String,
    exit_code: ExitCode,
}

fn load_manifest(path: &PathBuf) -> Result<ExperimentManifest, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("cannot parse manifest {}: {e}", path.display()))
}

/// Takes the manifest rather than a path so a test can drive the whole pipeline on a shortened one.
fn render(manifest: &ExperimentManifest, label: &str) -> Result<CommandOutput, String> {
    let registry = ObservableRegistry::reference_default();
    let summary = run_ensemble::<ReferenceEvolutionWorld>(manifest, &registry)
        .map_err(|e| format!("ensemble rejected: {e}"))?;

    let json = serde_json::to_string_pretty(&Report {
        tool: "anima-lab",
        manifest: label,
        model: "reference",
        summary: &summary,
    })
    .map_err(|e| format!("cannot serialise report: {e}"))?;

    Ok(CommandOutput {
        json,
        exit_code: if summary.failed.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        },
    })
}

/// Split from `main` so the error path is a value, not a process exit.
fn run(path: &PathBuf) -> Result<CommandOutput, String> {
    render(&load_manifest(path)?, &path.display().to_string())
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&argv) {
        Ok(None) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(path)) => match run(&path) {
            Ok(output) => {
                println!("{}", output.json);
                output.exit_code
            }
            Err(why) => fail(&why),
        },
        Err(why) => fail(&format!("{why}\n\n{USAGE}")),
    }
}

fn fail(why: &str) -> ExitCode {
    eprintln!("anima-lab: {why}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(parts: &[&str]) -> Result<Option<PathBuf>, String> {
        parse_args(&parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/experiments")
            .join(name)
    }

    /// Adding a second binary makes both Cargo's implicit `run` target and Tauri's package target
    /// ambiguous. Keep the desktop executable explicit so the headless tool cannot make the app
    /// unbuildable again.
    #[test]
    fn adding_the_lab_binary_keeps_the_desktop_binary_unambiguous() {
        let manifest =
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
                .expect("Cargo.toml is readable");
        let package = manifest
            .split("\n[lib]")
            .next()
            .expect("Cargo.toml starts with [package]");

        assert!(
            package
                .lines()
                .any(|line| line.trim() == r#"default-run = "anima-engine""#),
            "Cargo.toml must set package.default-run to the desktop binary when anima-lab exists"
        );
    }

    /// Help wins even alongside a path, so `--help` never accidentally runs an experiment.
    #[test]
    fn a_path_is_the_only_argument_and_help_beats_it() {
        assert_eq!(parse(&["m.json"]).unwrap(), Some(PathBuf::from("m.json")));
        for a in [vec!["--help"], vec!["-h"], vec!["m.json", "--help"]] {
            assert!(parse(&a).unwrap().is_none(), "{a:?}");
        }
    }

    /// Each must be a message, not a default: silently picking a manifest the caller never named is
    /// how a run gets attributed to the wrong thing.
    #[test]
    fn malformed_invocations_are_rejected_with_a_reason() {
        for case in [
            vec![],
            vec!["--verbose", "m.json"],
            vec!["a.json", "b.json"],
        ] {
            assert!(!parse(&case).unwrap_err().is_empty(), "{case:?}");
        }
    }

    /// Unreadable, unparseable, or refused by the runner — each an error, never an empty report.
    #[test]
    fn a_manifest_that_cannot_be_run_is_an_error() {
        let missing = run(&PathBuf::from("no/such.json")).unwrap_err();
        assert!(missing.contains("cannot read manifest"), "{missing}");
        // The tool's own source is a file that exists and is not a manifest.
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bin/anima-lab.rs");
        assert!(run(&src).unwrap_err().contains("cannot parse"));
        // Committed precisely because it is invalid; refused before the first tick.
        let refused = run(&fixture("invalid-negative-source.json")).unwrap_err();
        assert!(refused.contains("ensemble rejected"), "{refused}");
    }

    /// End to end through the real runner, shortened to one seed and one sample window: 600 ticks
    /// establishes the report's shape, and the numbers are the runner's tests to make.
    #[test]
    fn a_committed_fixture_produces_a_machine_readable_report() {
        let mut m = load_manifest(&fixture("baseline-no-exotic.json")).expect("fixture parses");
        m.seeds.truncate(1);
        m.duration_ticks = 600;
        m.sample_period = 600;

        let output = render(&m, "baseline-no-exotic.json").expect("must run");
        assert_eq!(output.exit_code, ExitCode::SUCCESS);
        let v: serde_json::Value = serde_json::from_str(&output.json).expect("stdout must be JSON");
        assert_eq!(v["tool"], "anima-lab");
        assert_eq!(v["model"], "reference");
        assert_eq!(v["experiment_id"], "exp-1");
        assert_eq!(v["completed_runs"], 1);
        assert_eq!(v["runs"].as_array().map(Vec::len), Some(1));
        assert!(
            v["failed"].as_array().unwrap().is_empty(),
            "{}",
            output.json
        );
        assert!(v["runs"][0]["series"].is_array(), "the series survives");
    }
}
