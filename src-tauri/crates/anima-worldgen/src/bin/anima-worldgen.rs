#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Deterministic World v2 offline CLI (`anima-worldgen generate | verify`).

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::pipeline::run_worldgen_to_dir;
use anima_worldgen::verify::{verify_dem_file, verify_raw_dir};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateArgs {
    seed: Option<u64>,
    size: Option<u32>,
    out: PathBuf,
    preset: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
struct VerifyArgs {
    raw: Option<PathBuf>,
    dem_input: Option<PathBuf>,
    size: Option<u32>,
    height: Option<u32>,
    dx_m: Option<f32>,
    sea_level_m: f32,
    generator_version: u32,
    out_gates: Option<PathBuf>,
    out_sim: Option<PathBuf>,
    out_index: Option<PathBuf>,
    report_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum CliAction {
    Help,
    Generate(GenerateArgs),
    Verify(VerifyArgs),
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorldV2PresetOverride {
    seed: Option<u64>,
    schema_tag: Option<String>,
    grid_size: Option<u32>,
    extent_m: Option<f32>,
    elev_min_m: Option<f32>,
    elev_max_m: Option<f32>,
    target_ocean_fraction: Option<f32>,
    massif_uplift_amplitude: Option<f32>,
    volcano_relief_m: Option<f32>,
    caprock_min_m: Option<f32>,
    caprock_max_m: Option<f32>,
    max_erodibility_contrast: Option<f32>,
}

fn usage_text() -> &'static str {
    "Usage:\n  anima-worldgen generate [--seed <SEED>] [--size <N>] --out <OUT> [--preset <PATH.toml>]\n  anima-worldgen verify --raw <DIR> [--out-gates <PATH> | --report <PATH>] [--out-sim <PATH>] [--out-index <PATH>] [--generator-version <U32>] [--strict-gates] [--report-only]\n  anima-worldgen verify --dem-input <FILE.f32> --size <N> [--height <H>] --dx <M> [--sea-level <M>] [--out-gates <PATH> | --report <PATH>] [--strict-gates] [--report-only]\n  anima-worldgen --help\n\nNotes:\n  CLI --seed and --size override `seed` / `grid_size` in `--preset` TOML; if omitted, `--preset` must supply them.\n"
}

fn parse_seed(raw: &str) -> Result<u64, String> {
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        if hex.is_empty() {
            return Err(format!("empty hex seed '{raw}'"));
        }
        u64::from_str_radix(hex, 16).map_err(|e| format!("invalid hex seed '{raw}': {e}"))
    } else {
        raw.parse::<u64>()
            .map_err(|e| format!("invalid decimal seed '{raw}': {e}"))
    }
}

fn parse_cli_args<I, S>(args: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut it = args.into_iter().map(|s| s.as_ref().to_owned());
    let _prog = it.next();
    let Some(subcmd) = it.next() else {
        return Err(format!("missing subcommand\n{}", usage_text()));
    };
    match subcmd.as_str() {
        "--help" | "-h" => Ok(CliAction::Help),
        "generate" => parse_generate_args(it),
        "verify" => parse_verify_args(it),
        _ => Err(format!("unknown subcommand '{subcmd}'\n{}", usage_text())),
    }
}

fn next_flag_val(
    it: &mut impl Iterator<Item = String>,
    flag: &str,
    non_empty: bool,
) -> Result<String, String> {
    let val = it
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if non_empty && val.is_empty() {
        return Err(format!("{flag} value must not be empty"));
    }
    Ok(val)
}

fn parse_generate_args<I: Iterator<Item = String>>(mut it: I) -> Result<CliAction, String> {
    let mut seed: Option<u64> = None;
    let mut size: Option<u32> = None;
    let mut out: Option<PathBuf> = None;
    let mut preset: Option<PathBuf> = None;

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(CliAction::Help),
            "--seed" => {
                if seed.is_some() {
                    return Err("duplicate --seed flag".to_owned());
                }
                let val = next_flag_val(&mut it, "--seed", true)?;
                seed = Some(parse_seed(&val)?);
            }
            "--size" => {
                if size.is_some() {
                    return Err("duplicate --size flag".to_owned());
                }
                let val = next_flag_val(&mut it, "--size", true)?;
                let parsed: u32 = val
                    .parse()
                    .map_err(|e| format!("invalid --size '{val}': {e}"))?;
                size = Some(parsed);
            }
            "--out" => {
                if out.is_some() {
                    return Err("duplicate --out flag".to_owned());
                }
                let val = next_flag_val(&mut it, "--out", true)?;
                out = Some(PathBuf::from(val));
            }
            "--preset" => {
                if preset.is_some() {
                    return Err("duplicate --preset flag".to_owned());
                }
                let val = next_flag_val(&mut it, "--preset", true)?;
                preset = Some(PathBuf::from(val));
            }
            other => {
                return Err(format!("unknown flag '{other}'\n{}", usage_text()));
            }
        }
    }

    if preset.is_none() {
        if seed.is_none() {
            return Err(format!("missing required --seed\n{}", usage_text()));
        }
        if size.is_none() {
            return Err(format!("missing required --size\n{}", usage_text()));
        }
    }
    let out = out.ok_or_else(|| format!("missing required --out\n{}", usage_text()))?;

    Ok(CliAction::Generate(GenerateArgs {
        seed,
        size,
        out,
        preset,
    }))
}

fn parse_verify_args<I: Iterator<Item = String>>(mut it: I) -> Result<CliAction, String> {
    let mut raw: Option<PathBuf> = None;
    let mut dem_input: Option<PathBuf> = None;
    let mut size: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut dx_m: Option<f32> = None;
    let mut sea_level_m: f32 = 0.0;
    let mut generator_version: u32 = 3;
    let mut out_gates: Option<PathBuf> = None;
    let mut out_sim: Option<PathBuf> = None;
    let mut out_index: Option<PathBuf> = None;
    let mut report_only = false;

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(CliAction::Help),
            "--raw" => {
                if raw.is_some() {
                    return Err("duplicate --raw flag".to_owned());
                }
                raw = Some(PathBuf::from(next_flag_val(&mut it, "--raw", true)?));
            }
            "--dem-input" => {
                if dem_input.is_some() {
                    return Err("duplicate --dem-input flag".to_owned());
                }
                dem_input = Some(PathBuf::from(next_flag_val(&mut it, "--dem-input", true)?));
            }
            "--size" => {
                let val = next_flag_val(&mut it, "--size", true)?;
                size = Some(
                    val.parse::<u32>()
                        .map_err(|e| format!("invalid --size '{val}': {e}"))?,
                );
            }
            "--height" => {
                let val = next_flag_val(&mut it, "--height", true)?;
                height = Some(
                    val.parse::<u32>()
                        .map_err(|e| format!("invalid --height '{val}': {e}"))?,
                );
            }
            "--dx" => {
                let val = next_flag_val(&mut it, "--dx", true)?;
                dx_m = Some(
                    val.parse::<f32>()
                        .map_err(|e| format!("invalid --dx '{val}': {e}"))?,
                );
            }
            "--sea-level" => {
                let val = next_flag_val(&mut it, "--sea-level", true)?;
                sea_level_m = val
                    .parse::<f32>()
                    .map_err(|e| format!("invalid --sea-level '{val}': {e}"))?;
            }
            "--generator-version" => {
                let val = next_flag_val(&mut it, "--generator-version", true)?;
                generator_version = val
                    .parse::<u32>()
                    .map_err(|e| format!("invalid --generator-version '{val}': {e}"))?;
            }
            "--out-gates" | "--report" => {
                if out_gates.is_some() {
                    return Err("duplicate --out-gates/--report flag".to_owned());
                }
                out_gates = Some(PathBuf::from(next_flag_val(&mut it, &flag, true)?));
            }
            "--out-sim" => {
                if out_sim.is_some() {
                    return Err("duplicate --out-sim flag".to_owned());
                }
                out_sim = Some(PathBuf::from(next_flag_val(&mut it, "--out-sim", true)?));
            }
            "--out-index" => {
                if out_index.is_some() {
                    return Err("duplicate --out-index flag".to_owned());
                }
                out_index = Some(PathBuf::from(next_flag_val(&mut it, "--out-index", true)?));
            }
            "--strict-gates" => {}
            "--report-only" => report_only = true,
            other => {
                return Err(format!("unknown flag '{other}'\n{}", usage_text()));
            }
        }
    }

    match (&raw, &dem_input) {
        (Some(_), Some(_)) => {
            return Err("cannot combine --raw and --dem-input".to_owned());
        }
        (None, None) => {
            return Err(format!(
                "verify requires --raw or --dem-input\n{}",
                usage_text()
            ));
        }
        (Some(_), None) => {}
        (None, Some(_)) => {
            if size.is_none() || dx_m.is_none() {
                return Err("--dem-input requires --size and --dx".to_owned());
            }
            if out_sim.is_some() || out_index.is_some() {
                return Err("--out-sim and --out-index require --raw".to_owned());
            }
        }
    }

    Ok(CliAction::Verify(VerifyArgs {
        raw,
        dem_input,
        size,
        height,
        dx_m,
        sea_level_m,
        generator_version,
        out_gates,
        out_sim,
        out_index,
        report_only,
    }))
}

fn load_config(args: &GenerateArgs) -> Result<WorldV2Config, String> {
    let preset_ov = if let Some(ref preset_path) = args.preset {
        let raw = fs::read_to_string(preset_path)
            .map_err(|e| format!("failed to read preset '{}': {e}", preset_path.display()))?;
        let ov: WorldV2PresetOverride = toml::from_str(&raw)
            .map_err(|e| format!("invalid preset TOML '{}': {e}", preset_path.display()))?;
        Some(ov)
    } else {
        None
    };

    let ov_seed = preset_ov.as_ref().and_then(|o| o.seed);
    let ov_size = preset_ov.as_ref().and_then(|o| o.grid_size);
    let seed = args
        .seed
        .or(ov_seed)
        .ok_or_else(|| "missing --seed and preset TOML omits `seed`".to_owned())?;
    let size = args
        .size
        .or(ov_size)
        .ok_or_else(|| "missing --size and preset TOML omits `grid_size`".to_owned())?;

    let mut cfg = WorldV2Config::for_size(seed, size);
    if let Some(ov) = preset_ov {
        if let Some(v) = ov.schema_tag {
            cfg.schema_tag = v;
        }
        if let Some(v) = ov.extent_m {
            cfg.extent_m = v;
        }
        if let Some(v) = ov.elev_min_m {
            cfg.elev_min_m = v;
        }
        if let Some(v) = ov.elev_max_m {
            cfg.elev_max_m = v;
        }
        if let Some(v) = ov.target_ocean_fraction {
            cfg.target_ocean_fraction = v;
        }
        if let Some(v) = ov.massif_uplift_amplitude {
            cfg.massif_uplift_amplitude = v;
        }
        if let Some(v) = ov.volcano_relief_m {
            cfg.volcano_relief_m = v;
        }
        if let Some(v) = ov.caprock_min_m {
            cfg.caprock_min_m = v;
        }
        if let Some(v) = ov.caprock_max_m {
            cfg.caprock_max_m = v;
        }
        if let Some(v) = ov.max_erodibility_contrast {
            cfg.max_erodibility_contrast = v;
        }
    }
    cfg.validate()
        .map_err(|e| format!("configuration validation failed: {e}"))?;
    Ok(cfg)
}

fn main() -> ExitCode {
    let action = match parse_cli_args(std::env::args()) {
        Ok(act) => act,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(2);
        }
    };

    match action {
        CliAction::Help => {
            print!("{}", usage_text());
            ExitCode::SUCCESS
        }
        CliAction::Generate(args) => {
            let cfg = match load_config(&args) {
                Ok(c) => c,
                Err(msg) => {
                    eprintln!("error: {msg}");
                    return ExitCode::from(2);
                }
            };
            match run_worldgen_to_dir(&cfg, &args.out) {
                Ok(meta) => {
                    println!(
                        "generated {}x{} world (seed={:#018x}, sha256={}, waterfalls={}, caves={}, lakes={:.4})",
                        meta.width,
                        meta.height,
                        meta.seed,
                        meta.world_sha256,
                        meta.stats.waterfall_count,
                        meta.stats.cave_entrance_count,
                        meta.stats.lake_surface_fraction
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("generation failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        CliAction::Verify(args) => {
            let res = if let Some(ref raw_dir) = args.raw {
                verify_raw_dir(
                    raw_dir,
                    args.generator_version,
                    args.out_sim.as_deref(),
                    args.out_index.as_deref(),
                )
            } else {
                let dem = args.dem_input.as_deref().expect("checked dem_input");
                let w = args.size.expect("checked size") as usize;
                let h = args.height.map_or(w, |v| v as usize);
                let dx = args.dx_m.expect("checked dx");
                verify_dem_file(dem, w, h, dx, args.sea_level_m)
            };

            let report = match res {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("verification failed: {err}");
                    return ExitCode::from(1);
                }
            };

            if let Some(ref gates_path) = args.out_gates {
                if let Some(parent) = gates_path.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        eprintln!("failed to create gates dir: {err}");
                        return ExitCode::from(1);
                    }
                }
                let mut json = match serde_json::to_string_pretty(&report) {
                    Ok(j) => j,
                    Err(err) => {
                        eprintln!("failed to serialize gates.json: {err}");
                        return ExitCode::from(1);
                    }
                };
                json.push('\n');
                if let Err(err) = fs::write(gates_path, json) {
                    eprintln!("failed to write gates.json: {err}");
                    return ExitCode::from(1);
                }
            }

            let g_comp_tag =
                report
                    .g_comp_v2
                    .as_ref()
                    .map_or("N/A", |c| if c.passed { "PASS" } else { "FAIL" });
            let g_sim_tag =
                report
                    .g_sim_raw
                    .as_ref()
                    .map_or("N/A", |s| if s.passed { "PASS" } else { "FAIL" });
            println!(
                "verified {}x{} ({}) overall={} G-SHAPE={} G-COMP={} G-SIM={} failed={:?}",
                report.width,
                report.height,
                report.mode,
                if report.overall_pass { "PASS" } else { "FAIL" },
                if report.g_shape.passed {
                    "PASS"
                } else {
                    "FAIL"
                },
                g_comp_tag,
                g_sim_tag,
                report.failed_gates
            );

            if report.overall_pass || args.report_only {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_help_and_decimal_or_hex_generate_and_verify_args() {
        assert_eq!(
            parse_cli_args(["anima-worldgen", "--help"]).unwrap(),
            CliAction::Help
        );
        assert_eq!(
            parse_cli_args(["anima-worldgen", "generate", "-h"]).unwrap(),
            CliAction::Help
        );
        assert_eq!(
            parse_cli_args(["anima-worldgen", "verify", "-h"]).unwrap(),
            CliAction::Help
        );

        let parsed = parse_cli_args([
            "anima-worldgen",
            "generate",
            "--seed",
            "0x57563201",
            "--size",
            "512",
            "--out",
            "/tmp/out_raw",
            "--preset",
            "/tmp/preset.toml",
        ])
        .unwrap();
        assert_eq!(
            parsed,
            CliAction::Generate(GenerateArgs {
                seed: Some(0x5756_3201),
                size: Some(512),
                out: PathBuf::from("/tmp/out_raw"),
                preset: Some(PathBuf::from("/tmp/preset.toml")),
            })
        );

        let dec = parse_cli_args([
            "anima-worldgen",
            "generate",
            "--seed",
            "42",
            "--size",
            "256",
            "--out",
            "/tmp/out_dec",
        ])
        .unwrap();
        assert_eq!(
            dec,
            CliAction::Generate(GenerateArgs {
                seed: Some(42),
                size: Some(256),
                out: PathBuf::from("/tmp/out_dec"),
                preset: None,
            })
        );

        let preset_only = parse_cli_args([
            "anima-worldgen",
            "generate",
            "--preset",
            "/tmp/preset.toml",
            "--out",
            "/tmp/out_preset",
        ])
        .unwrap();
        assert_eq!(
            preset_only,
            CliAction::Generate(GenerateArgs {
                seed: None,
                size: None,
                out: PathBuf::from("/tmp/out_preset"),
                preset: Some(PathBuf::from("/tmp/preset.toml")),
            })
        );

        let ver = parse_cli_args([
            "anima-worldgen",
            "verify",
            "--raw",
            "/tmp/raw",
            "--strict-gates",
            "--report",
            "/tmp/gates.json",
            "--out-sim",
            "/tmp/sim_256.anmw3",
            "--out-index",
            "/tmp/index.json",
        ])
        .unwrap();
        assert_eq!(
            ver,
            CliAction::Verify(VerifyArgs {
                raw: Some(PathBuf::from("/tmp/raw")),
                dem_input: None,
                size: None,
                height: None,
                dx_m: None,
                sea_level_m: 0.0,
                generator_version: 3,
                out_gates: Some(PathBuf::from("/tmp/gates.json")),
                out_sim: Some(PathBuf::from("/tmp/sim_256.anmw3")),
                out_index: Some(PathBuf::from("/tmp/index.json")),
                report_only: false,
            })
        );
    }

    #[test]
    fn cli_rejects_missing_or_unknown_flags_and_invalid_presets() {
        assert!(parse_cli_args(["anima-worldgen"]).is_err());
        assert!(parse_cli_args(["anima-worldgen", "bogus"]).is_err());
        assert!(parse_cli_args([
            "anima-worldgen",
            "generate",
            "--seed",
            "42",
            "--size",
            "256"
        ])
        .is_err());
        assert!(parse_cli_args([
            "anima-worldgen",
            "generate",
            "--out",
            "/tmp/no_seed_no_preset"
        ])
        .is_err());
        assert!(parse_cli_args([
            "anima-worldgen",
            "generate",
            "--seed",
            "42",
            "--size",
            "256",
            "--out",
            "/tmp/x",
            "--bogus",
            "1"
        ])
        .is_err());
        assert!(parse_cli_args([
            "anima-worldgen",
            "verify",
            "--raw",
            "/tmp/a",
            "--dem-input",
            "/tmp/b.f32"
        ])
        .is_err());
        assert!(parse_cli_args([
            "anima-worldgen",
            "verify",
            "--dem-input",
            "/tmp/b.f32",
            "--size",
            "128",
            "--dx",
            "10",
            "--out-sim",
            "/tmp/s"
        ])
        .is_err());

        let parsed_bad_size = parse_cli_args([
            "anima-worldgen",
            "generate",
            "--seed",
            "42",
            "--size",
            "129",
            "--out",
            "/tmp/x",
        ])
        .unwrap();
        if let CliAction::Generate(g) = parsed_bad_size {
            assert!(load_config(&g).is_err());
        } else {
            panic!("expected Generate");
        }
    }
}
