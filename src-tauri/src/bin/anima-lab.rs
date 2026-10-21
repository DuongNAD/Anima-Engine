//! `anima-lab` — run experiments, parameter sweeps, and causal branch trees headlessly.
//!
//! Subcommands:
//! - `anima-lab [ensemble] [--model reference|live] <MANIFEST.json>`
//! - `anima-lab sweep [--model reference|live] [--format json|csv] [--threads N] <SWEEP_MANIFEST.json>`
//! - `anima-lab branch [--model reference|live] <BRANCH_MANIFEST.json>`

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anima_engine_lib::core::branch_tree::{run_branch_tree, BranchTreeManifest};
use anima_engine_lib::core::experiment::{ExperimentManifest, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{run_ensemble, EnsembleSummary};
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sweeps::{run_sweep, SweepManifest};
use serde::Serialize;

const USAGE: &str = "\
anima-lab — headless experimentation, multidimensional parameter sweeps, and causal branching

Usage:
    anima-lab [ensemble] [--model reference|live] <MANIFEST.json>
    anima-lab sweep [--model reference|live] [--format json|csv] [--threads N] <SWEEP_MANIFEST.json>
    anima-lab branch [--model reference|live] <BRANCH_MANIFEST.json>
    anima-lab --help

Options:
    --model <reference|live>   Simulation execution model (default: reference)
    --format <json|csv>        Output format for sweep (default: json)
    --threads <N>              Rayon thread pool size for sweeps (default: auto)
    -h, --help                 Print this help text

Exit Codes:
    0: All runs/points completed successfully
    1: Preserved runtime failures or invalid manifest
";

const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Model {
    #[default]
    Reference,
    Live,
}

impl Model {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "reference" => Ok(Self::Reference),
            "live" => Ok(Self::Live),
            other => Err(format!(
                "unknown model '{other}'; expected 'reference' or 'live'"
            )),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Live => "live",
        }
    }

    pub const fn registry_label(self) -> &'static str {
        match self {
            Self::Reference => "core::experiment::ObservableRegistry::reference_default",
            Self::Live => "core::experiment::ObservableRegistry::live_default",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Json,
    Csv,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => Err(format!(
                "unknown output format '{other}'; expected 'json' or 'csv'"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Ensemble {
        manifest: PathBuf,
        model: Model,
    },
    Sweep {
        manifest: PathBuf,
        model: Model,
        format: OutputFormat,
        threads: Option<usize>,
    },
    Branch {
        manifest: PathBuf,
        model: Model,
    },
}

pub fn parse_args(argv: &[String]) -> Result<Option<Command>, String> {
    if argv.iter().any(|arg| matches!(arg.as_str(), "-h" | "--help")) {
        return Ok(None);
    }
    if argv.is_empty() {
        return Err("missing command or <MANIFEST.json>".into());
    }

    let mut args = argv.iter().peekable();
    let mut subcmd = None;

    if let Some(&first) = args.peek() {
        match first.as_str() {
            "ensemble" => {
                subcmd = Some("ensemble");
                args.next();
            }
            "sweep" => {
                subcmd = Some("sweep");
                args.next();
            }
            "branch" => {
                subcmd = Some("branch");
                args.next();
            }
            _ => {}
        }
    }

    let mut manifest: Option<PathBuf> = None;
    let mut model = None;
    let mut format = None;
    let mut threads = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--model" => {
                if model.is_some() {
                    return Err("--model may be specified only once".into());
                }
                let value = args.next().ok_or("--model requires 'reference' or 'live'")?;
                model = Some(Model::parse(value)?);
            }
            other if other.starts_with("--model=") => {
                if model.is_some() {
                    return Err("--model may be specified only once".into());
                }
                model = Some(Model::parse(&other["--model=".len()..])?);
            }
            "--format" => {
                if format.is_some() {
                    return Err("--format may be specified only once".into());
                }
                let value = args.next().ok_or("--format requires 'json' or 'csv'")?;
                format = Some(OutputFormat::parse(value)?);
            }
            other if other.starts_with("--format=") => {
                if format.is_some() {
                    return Err("--format may be specified only once".into());
                }
                format = Some(OutputFormat::parse(&other["--format=".len()..])?);
            }
            "--threads" => {
                if threads.is_some() {
                    return Err("--threads may be specified only once".into());
                }
                let value = args.next().ok_or("--threads requires an integer")?;
                let n: usize = value.parse().map_err(|_| format!("invalid thread count '{value}'"))?;
                threads = Some(n);
            }
            other if other.starts_with("--threads=") => {
                if threads.is_some() {
                    return Err("--threads may be specified only once".into());
                }
                let val_str = &other["--threads=".len()..];
                let n: usize = val_str.parse().map_err(|_| format!("invalid thread count '{val_str}'"))?;
                threads = Some(n);
            }
            other if other.starts_with('-') => return Err(format!("unknown option '{other}'")),
            other if manifest.is_none() => manifest = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected extra argument '{other}'")),
        }
    }

    let manifest = manifest.ok_or("missing <MANIFEST.json>")?;
    let model = model.unwrap_or_default();

    match subcmd {
        Some("sweep") => Ok(Some(Command::Sweep {
            manifest,
            model,
            format: format.unwrap_or_default(),
            threads,
        })),
        Some("branch") => {
            if format.is_some() {
                return Err("--format is only valid for 'sweep' subcommand".into());
            }
            if threads.is_some() {
                return Err("--threads is only valid for 'sweep' subcommand".into());
            }
            Ok(Some(Command::Branch { manifest, model }))
        }
        _ => {
            if format.is_some() {
                return Err("--format is only valid for 'sweep' subcommand".into());
            }
            if threads.is_some() {
                return Err("--threads is only valid for 'sweep' subcommand".into());
            }
            Ok(Some(Command::Ensemble { manifest, model }))
        }
    }
}

#[derive(Serialize)]
struct EnsembleReport<'a> {
    tool: &'static str,
    manifest: &'a str,
    model: &'static str,
    registry: &'static str,
    #[serde(flatten)]
    summary: &'a EnsembleSummary,
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

fn load_manifest_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
    let size = file
        .metadata()
        .map_err(|e| format!("cannot inspect manifest {}: {e}", path.display()))?
        .len();
    read_manifest_with_limit(file, path, size, MAX_MANIFEST_BYTES)
}

fn load_experiment_manifest(path: &Path) -> Result<ExperimentManifest, String> {
    let bytes = load_manifest_bytes(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot parse manifest {}: {e}", path.display()))
}

fn load_sweep_manifest(path: &Path) -> Result<SweepManifest, String> {
    let bytes = load_manifest_bytes(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot parse sweep manifest {}: {e}", path.display()))
}

fn load_branch_manifest(path: &Path) -> Result<BranchTreeManifest, String> {
    let bytes = load_manifest_bytes(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot parse branch manifest {}: {e}", path.display()))
}

pub fn render_ensemble<W: Write>(
    manifest: &ExperimentManifest,
    label: &str,
    model: Model,
    writer: &mut W,
) -> Result<ExitCode, String> {
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

    serde_json::to_writer_pretty(
        &mut *writer,
        &EnsembleReport {
            tool: "anima-lab",
            manifest: label,
            model: model.label(),
            registry: model.registry_label(),
            summary: &summary,
        },
    )
    .map_err(|e| format!("cannot serialise report: {e}"))?;
    writer
        .write_all(b"\n")
        .map_err(|e| format!("cannot write report terminator: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("cannot flush report: {e}"))?;

    Ok(if summary.failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub fn render_sweep<W: Write>(
    mut manifest: SweepManifest,
    model: Model,
    format: OutputFormat,
    threads: Option<usize>,
    writer: &mut W,
) -> Result<ExitCode, String> {
    if threads.is_some() {
        manifest.max_threads = threads;
    }
    let registry = match model {
        Model::Reference => ObservableRegistry::reference_default(),
        Model::Live => ObservableRegistry::live_default(),
    };

    let result = match model {
        Model::Reference => run_sweep::<ReferenceEvolutionWorld>(&manifest, &registry),
        Model::Live => run_sweep::<LiveExperimentAdapter>(&manifest, &registry),
    }
    .map_err(|e| format!("sweep rejected: {e}"))?;

    match format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &result)
                .map_err(|e| format!("cannot serialise sweep report: {e}"))?;
            writer.write_all(b"\n").map_err(|e| format!("cannot write newline: {e}"))?;
        }
        OutputFormat::Csv => {
            writer
                .write_all(result.to_csv().as_bytes())
                .map_err(|e| format!("cannot write csv: {e}"))?;
        }
    }
    writer.flush().map_err(|e| format!("cannot flush report: {e}"))?;

    Ok(if result.failed_runs == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub fn render_branch<W: Write>(
    manifest: &BranchTreeManifest,
    model: Model,
    writer: &mut W,
) -> Result<ExitCode, String> {
    let registry = match model {
        Model::Reference => ObservableRegistry::reference_default(),
        Model::Live => ObservableRegistry::live_default(),
    };

    let result = match model {
        Model::Reference => run_branch_tree::<ReferenceEvolutionWorld>(manifest, &registry),
        Model::Live => run_branch_tree::<LiveExperimentAdapter>(manifest, &registry),
    }
    .map_err(|e| format!("branch tree rejected: {e}"))?;

    serde_json::to_writer_pretty(&mut *writer, &result)
        .map_err(|e| format!("cannot serialise branch tree report: {e}"))?;
    writer.write_all(b"\n").map_err(|e| format!("cannot write newline: {e}"))?;
    writer.flush().map_err(|e| format!("cannot flush report: {e}"))?;

    Ok(ExitCode::SUCCESS)
}

pub fn run<W: Write>(cmd: &Command, writer: &mut W) -> Result<ExitCode, String> {
    match cmd {
        Command::Ensemble { manifest, model } => {
            let m = load_experiment_manifest(manifest)?;
            render_ensemble(&m, &manifest.display().to_string(), *model, writer)
        }
        Command::Sweep {
            manifest,
            model,
            format,
            threads,
        } => {
            let m = load_sweep_manifest(manifest)?;
            render_sweep(m, *model, *format, *threads, writer)
        }
        Command::Branch { manifest, model } => {
            let m = load_branch_manifest(manifest)?;
            render_branch(&m, *model, writer)
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&argv) {
        Ok(None) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(cmd)) => {
            let stdout = std::io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            match run(&cmd, &mut writer) {
                Ok(exit_code) => exit_code,
                Err(why) => fail(&why),
            }
        }
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

    fn parse(parts: &[&str]) -> Result<Option<Command>, String> {
        parse_args(&parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/experiments")
            .join(name)
    }

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

    #[test]
    fn a_path_is_the_only_argument_and_help_beats_it() {
        assert_eq!(
            parse(&["m.json"]).unwrap(),
            Some(Command::Ensemble {
                manifest: PathBuf::from("m.json"),
                model: Model::Reference,
            })
        );
        assert_eq!(
            parse(&["ensemble", "--model", "live", "m.json"]).unwrap(),
            Some(Command::Ensemble {
                manifest: PathBuf::from("m.json"),
                model: Model::Live,
            })
        );
        assert_eq!(
            parse(&["sweep", "--format", "csv", "--threads", "4", "sweep.json"]).unwrap(),
            Some(Command::Sweep {
                manifest: PathBuf::from("sweep.json"),
                model: Model::Reference,
                format: OutputFormat::Csv,
                threads: Some(4),
            })
        );
        assert_eq!(
            parse(&["branch", "--model=live", "branch.json"]).unwrap(),
            Some(Command::Branch {
                manifest: PathBuf::from("branch.json"),
                model: Model::Live,
            })
        );
        for a in [
            vec!["--help"],
            vec!["-h"],
            vec!["m.json", "--help"],
            vec!["sweep", "--help"],
            vec!["branch", "--help"],
        ] {
            assert!(parse(&a).unwrap().is_none(), "{a:?}");
        }
    }

    #[test]
    fn malformed_invocations_are_rejected_with_a_reason() {
        for case in [
            vec![],
            vec!["--verbose", "m.json"],
            vec!["a.json", "b.json"],
            vec!["--model"],
            vec!["--model", "other", "m.json"],
            vec!["--model", "live", "--model", "reference", "m.json"],
            vec!["branch", "--format", "csv", "b.json"],
        ] {
            assert!(!parse(&case).unwrap_err().is_empty(), "{case:?}");
        }
    }

    #[test]
    fn a_manifest_that_cannot_be_run_is_an_error() {
        let missing = run(
            &Command::Ensemble {
                manifest: PathBuf::from("no/such.json"),
                model: Model::Reference,
            },
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(missing.contains("cannot read manifest"), "{missing}");

        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bin/anima-lab.rs");
        assert!(run(
            &Command::Ensemble {
                manifest: src,
                model: Model::Reference,
            },
            &mut Vec::new(),
        )
        .unwrap_err()
        .contains("cannot parse"));

        let refused = run(
            &Command::Ensemble {
                manifest: fixture("invalid-negative-source.json"),
                model: Model::Reference,
            },
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(refused.contains("ensemble rejected"), "{refused}");
    }
}
