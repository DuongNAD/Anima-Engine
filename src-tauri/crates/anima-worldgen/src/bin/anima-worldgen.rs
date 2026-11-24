#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Deterministic World v2 offline CLI (`anima-worldgen generate`).

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::pipeline::run_worldgen_to_dir;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateArgs {
    seed: u64,
    size: u32,
    out: PathBuf,
    preset: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    Help,
    Generate(GenerateArgs),
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
    "Usage:\n  anima-worldgen generate --seed <SEED> --size <N> --out <OUT> [--preset <PATH.toml>]\n  anima-worldgen --help\n\nNotes:\n  CLI --seed and --size always override any `seed` or `grid_size` fields in `--preset` TOML.\n"
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
    if subcmd == "--help" || subcmd == "-h" {
        return Ok(CliAction::Help);
    }
    if subcmd != "generate" {
        return Err(format!("unknown subcommand '{subcmd}'\n{}", usage_text()));
    }

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
                let val = it
                    .next()
                    .ok_or_else(|| "--seed requires a value".to_owned())?;
                seed = Some(parse_seed(&val)?);
            }
            "--size" => {
                if size.is_some() {
                    return Err("duplicate --size flag".to_owned());
                }
                let val = it
                    .next()
                    .ok_or_else(|| "--size requires a value".to_owned())?;
                let parsed: u32 = val
                    .parse()
                    .map_err(|e| format!("invalid --size '{val}': {e}"))?;
                size = Some(parsed);
            }
            "--out" => {
                if out.is_some() {
                    return Err("duplicate --out flag".to_owned());
                }
                let val = it
                    .next()
                    .ok_or_else(|| "--out requires a path".to_owned())?;
                if val.is_empty() {
                    return Err("--out path must not be empty".to_owned());
                }
                out = Some(PathBuf::from(val));
            }
            "--preset" => {
                if preset.is_some() {
                    return Err("duplicate --preset flag".to_owned());
                }
                let val = it
                    .next()
                    .ok_or_else(|| "--preset requires a path".to_owned())?;
                if val.is_empty() {
                    return Err("--preset path must not be empty".to_owned());
                }
                preset = Some(PathBuf::from(val));
            }
            other => {
                return Err(format!("unknown flag '{other}'\n{}", usage_text()));
            }
        }
    }

    let seed = seed.ok_or_else(|| format!("missing required --seed\n{}", usage_text()))?;
    let size = size.ok_or_else(|| format!("missing required --size\n{}", usage_text()))?;
    let out = out.ok_or_else(|| format!("missing required --out\n{}", usage_text()))?;

    Ok(CliAction::Generate(GenerateArgs {
        seed,
        size,
        out,
        preset,
    }))
}

fn load_config(args: &GenerateArgs) -> Result<WorldV2Config, String> {
    let mut cfg = WorldV2Config::for_size(args.seed, args.size);
    if let Some(ref preset_path) = args.preset {
        let raw = std::fs::read_to_string(preset_path)
            .map_err(|e| format!("failed to read preset '{}': {e}", preset_path.display()))?;
        let ov: WorldV2PresetOverride = toml::from_str(&raw)
            .map_err(|e| format!("invalid preset TOML '{}': {e}", preset_path.display()))?;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_help_and_decimal_or_hex_generate_args() {
        assert_eq!(
            parse_cli_args(["anima-worldgen", "--help"]).unwrap(),
            CliAction::Help
        );
        assert_eq!(
            parse_cli_args(["anima-worldgen", "generate", "-h"]).unwrap(),
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
                seed: 0x5756_3201,
                size: 512,
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
                seed: 42,
                size: 256,
                out: PathBuf::from("/tmp/out_dec"),
                preset: None,
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
