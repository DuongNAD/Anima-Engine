//! `anima-lab` — run an experiment manifest headlessly and print the result as JSON.
//!
//! Parses arguments, loads a manifest, hands it to [`run_ensemble`] over the selected reference or
//! live model, and serialises the [`EnsembleSummary`]; the science stays in the shared runner.
//! stdout carries JSON and nothing else. Errors go to stderr and exit non-zero — a run that fails
//! *inside* a valid ensemble is reported in the JSON, not on stderr, and still makes the process
//! exit non-zero so automation cannot mistake a partial result for success.
//!
//! `reference` remains the default; `--model live` selects [`LiveExperimentAdapter`].

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anima_engine_lib::core::experiment::{ExperimentManifest, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{run_ensemble, EnsembleSummary};
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use serde::Serialize;

const USAGE: &str = "\
anima-lab — run an experiment manifest headlessly and print JSON to stdout

    anima-lab [--model reference|live] <MANIFEST.json>
    anima-lab --help

Every seed the manifest declares is run in order. The default model is `reference` for compatibility;
`--model live` runs the real Bevy schedule through LiveExperimentAdapter.
Exit 0 when every run completes. Exit 1 with JSON on stdout for preserved runtime failures, or with
a reason on stderr when no report can be produced.
";

/// A manifest is control-plane input, not a data container. Eight MiB leaves ample room for the
/// schema's thousands of bounded seed/observable/intervention entries at normal identifier sizes
/// while preventing an untrusted path from turning JSON parsing into an unbounded allocation.
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Model {
    #[default]
    Reference,
    Live,
}

impl Model {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "reference" => Ok(Self::Reference),
            "live" => Ok(Self::Live),
            other => Err(format!(
                "unknown model '{other}'; expected 'reference' or 'live'"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Live => "live",
        }
    }

    const fn registry_label(self) -> &'static str {
        match self {
            Self::Reference => "core::experiment::ObservableRegistry::reference_default",
            Self::Live => "core::experiment::ObservableRegistry::live_default",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Arguments {
    manifest: PathBuf,
    model: Model,
}

/// Parse `argv` (without the program name). `Ok(None)` means help was requested.
fn parse_args(argv: &[String]) -> Result<Option<Arguments>, String> {
    if argv
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        return Ok(None);
    }
    let mut manifest: Option<PathBuf> = None;
    let mut model = None;
    let mut args = argv.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--model" => {
                if model.is_some() {
                    return Err("--model may be specified only once".into());
                }
                let value = args
                    .next()
                    .ok_or("--model requires 'reference' or 'live'")?;
                model = Some(Model::parse(value)?);
            }
            other if other.starts_with("--model=") => {
                if model.is_some() {
                    return Err("--model may be specified only once".into());
                }
                model = Some(Model::parse(&other["--model=".len()..])?);
            }
            other if other.starts_with('-') => return Err(format!("unknown option '{other}'")),
            other if manifest.is_none() => manifest = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected extra argument '{other}'")),
        }
    }
    manifest
        .map(|manifest| {
            Some(Arguments {
                manifest,
                model: model.unwrap_or_default(),
            })
        })
        .ok_or("missing <MANIFEST.json>".into())
}

/// The stdout envelope: what was run, flattened over the runner's own summary rather than rebuilt
/// from it, so no field can be dropped by going stale here.
#[derive(Serialize)]
struct Report<'a> {
    tool: &'static str,
    manifest: &'a str,
    model: &'static str,
    registry: &'static str,
    #[serde(flatten)]
    summary: &'a EnsembleSummary,
}

#[derive(Debug)]
struct CommandOutput {
    json: String,
    exit_code: ExitCode,
}

fn manifest_too_large(path: &Path, found: u64, limit: usize) -> String {
    format!(
        "manifest is too large {}: at least {found} bytes (limit {limit} bytes)",
        path.display()
    )
}

fn read_manifest_with_limit<R: Read>(
    reader: R,
    path: &Path,
    declared_size: u64,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if declared_size > limit as u64 {
        return Err(manifest_too_large(path, declared_size, limit));
    }

    let requested = declared_size as usize;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(requested).map_err(|e| {
        format!(
            "cannot allocate {requested} bytes for manifest {}: {e}",
            path.display()
        )
    })?;

    reader
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
    if bytes.len() > limit {
        return Err(manifest_too_large(path, bytes.len() as u64, limit));
    }
    Ok(bytes)
}

fn load_manifest_with_limit(path: &Path, limit: usize) -> Result<ExperimentManifest, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
    let size = file
        .metadata()
        .map_err(|e| format!("cannot inspect manifest {}: {e}", path.display()))?
        .len();
    let bytes = read_manifest_with_limit(file, path, size, limit)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot parse manifest {}: {e}", path.display()))
}

fn load_manifest(path: &Path) -> Result<ExperimentManifest, String> {
    load_manifest_with_limit(path, MAX_MANIFEST_BYTES)
}

/// Takes the manifest rather than a path so a test can drive the whole pipeline on a shortened one.
fn render(
    manifest: &ExperimentManifest,
    label: &str,
    model: Model,
) -> Result<CommandOutput, String> {
    let summary = match model {
        Model::Reference => run_ensemble::<ReferenceEvolutionWorld>(
            manifest,
            &ObservableRegistry::reference_default(),
        ),
        Model::Live => {
            run_ensemble::<LiveExperimentAdapter>(manifest, &ObservableRegistry::live_default())
        }
    }
    .map_err(|e| format!("ensemble rejected: {e}"))?;

    let json = serde_json::to_string_pretty(&Report {
        tool: "anima-lab",
        manifest: label,
        model: model.label(),
        registry: model.registry_label(),
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
fn run(args: &Arguments) -> Result<CommandOutput, String> {
    render(
        &load_manifest(&args.manifest)?,
        &args.manifest.display().to_string(),
        args.model,
    )
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&argv) {
        Ok(None) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(args)) => match run(&args) {
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

    fn parse(parts: &[&str]) -> Result<Option<Arguments>, String> {
        parse_args(&parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn arguments(path: PathBuf, model: Model) -> Arguments {
        Arguments {
            manifest: path,
            model,
        }
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
        assert_eq!(
            parse(&["m.json"]).unwrap(),
            Some(arguments(PathBuf::from("m.json"), Model::Reference))
        );
        assert_eq!(
            parse(&["--model", "live", "m.json"]).unwrap(),
            Some(arguments(PathBuf::from("m.json"), Model::Live))
        );
        assert_eq!(
            parse(&["m.json", "--model=live"]).unwrap(),
            Some(arguments(PathBuf::from("m.json"), Model::Live))
        );
        for a in [
            vec!["--help"],
            vec!["-h"],
            vec!["m.json", "--help"],
            vec!["--model", "--help"],
        ] {
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
            vec!["--model"],
            vec!["--model", "other", "m.json"],
            vec!["--model", "live", "--model", "reference", "m.json"],
        ] {
            assert!(!parse(&case).unwrap_err().is_empty(), "{case:?}");
        }
    }

    /// Unreadable, unparseable, or refused by the runner — each an error, never an empty report.
    #[test]
    fn a_manifest_that_cannot_be_run_is_an_error() {
        let missing = run(&arguments(PathBuf::from("no/such.json"), Model::Reference)).unwrap_err();
        assert!(missing.contains("cannot read manifest"), "{missing}");
        // The tool's own source is a file that exists and is not a manifest.
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bin/anima-lab.rs");
        assert!(run(&arguments(src, Model::Reference))
            .unwrap_err()
            .contains("cannot parse"));
        // Committed precisely because it is invalid; refused before the first tick.
        let refused = run(&arguments(
            fixture("invalid-negative-source.json"),
            Model::Reference,
        ))
        .unwrap_err();
        assert!(refused.contains("ensemble rejected"), "{refused}");
    }

    #[test]
    fn an_oversized_manifest_is_rejected_before_it_can_be_allocated() {
        let path = std::env::temp_dir().join(format!(
            "anima-lab-oversized-manifest-{}.json",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create sparse manifest");
        file.set_len(MAX_MANIFEST_BYTES as u64 + 1)
            .expect("extend sparse manifest");
        drop(file);

        let error = load_manifest(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert!(error.contains("manifest is too large"), "{error}");
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(
            error.contains(&format!("{} bytes", MAX_MANIFEST_BYTES + 1)),
            "{error}"
        );
        assert!(
            error.contains(&format!("limit {MAX_MANIFEST_BYTES} bytes")),
            "{error}"
        );
    }

    #[test]
    fn a_manifest_that_grows_after_inspection_is_still_bounded() {
        const TEST_LIMIT: usize = 32;
        let path = Path::new("growing.json");
        let error = read_manifest_with_limit(
            std::io::Cursor::new(vec![b' '; TEST_LIMIT + 1]),
            path,
            1,
            TEST_LIMIT,
        )
        .unwrap_err();

        assert!(error.contains("manifest is too large"), "{error}");
        assert!(error.contains("33 bytes"), "{error}");
        assert!(error.contains("limit 32 bytes"), "{error}");
    }

    /// End to end through the real runner, shortened to one seed and one sample window: 600 ticks
    /// establishes the report's shape, and the numbers are the runner's tests to make.
    #[test]
    fn a_committed_fixture_produces_a_machine_readable_report() {
        let mut m = load_manifest(&fixture("baseline-no-exotic.json")).expect("fixture parses");
        m.seeds.truncate(1);
        m.duration_ticks = 600;
        m.sample_period = 600;

        let output = render(&m, "baseline-no-exotic.json", Model::Reference).expect("must run");
        assert_eq!(output.exit_code, ExitCode::SUCCESS);
        let v: serde_json::Value = serde_json::from_str(&output.json).expect("stdout must be JSON");
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
            output.json
        );
        assert!(v["runs"][0]["series"].is_array(), "the series survives");
    }
}
