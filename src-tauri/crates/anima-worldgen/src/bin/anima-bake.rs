#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Offline World v2 bake CLI (`anima-bake probe-gpu | fields | lighting | pack | verify | assets`).

use anima_worldgen::bake::assets::bake_assets_from_manifest;
use anima_worldgen::bake::fields::{bake_fields_from_raw_dir, merge_field_strips};
use anima_worldgen::bake::lighting_cpu::{bake_lighting_cpu_from_raw_dir, merge_lighting_tiles};
#[cfg(feature = "gpu-bake")]
use anima_worldgen::bake::lighting_gpu::{bake_lighting_wgpu_from_raw_dir, probe_gpu};
use anima_worldgen::bake::pack_ktx2::{pack_world_v2_tiles, verify_tile_pack};
use anima_worldgen::bake::{TileRange, TileStrip};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldsArgs {
    raw: PathBuf,
    out: PathBuf,
    tile_strip: Option<TileStrip>,
    merge_strips: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LightingArgs {
    backend: String,
    raw: PathBuf,
    out: PathBuf,
    tile_range: Option<TileRange>,
    samples: u32,
    merge_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackArgs {
    in_dir: PathBuf,
    raw_dir: Option<PathBuf>,
    sim_path: Option<PathBuf>,
    release_tag: String,
    out_dir: PathBuf,
    verify: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifyArgs {
    index_path: PathBuf,
    sim_path: PathBuf,
    archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetsArgs {
    manifest: PathBuf,
    out: PathBuf,
    atlas_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BakeCliAction {
    Help,
    ProbeGpu,
    Fields(FieldsArgs),
    Lighting(LightingArgs),
    Pack(PackArgs),
    Verify(VerifyArgs),
    Assets(AssetsArgs),
}

fn usage_text() -> &'static str {
    "Usage:
  anima-bake probe-gpu                                (requires --features gpu-bake)
  anima-bake fields --raw <DIR> --out <DIR> [--tile-strip <S>/<K>] [--merge-strips <K>]
  anima-bake lighting [--backend cpu|wgpu] --raw <DIR> --out <DIR> [--tile-range <A>..<B>] [--samples <N>] [--merge-size <N>]
  anima-bake pack --in <DIR> [--raw <DIR>] [--sim <PATH>] --release-tag <TAG> --out <DIR> [--verify]
  anima-bake verify --index <PATH> --sim <PATH> --archive <PATH>
  anima-bake assets --manifest <PATH> --out <DIR> [--atlas-size <N>]
  anima-bake --help\n"
}

fn next_val(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    let val = it
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if val.is_empty() {
        return Err(format!("{flag} value must not be empty"));
    }
    Ok(val)
}

fn parse_bake_args<I, S>(args: I) -> Result<BakeCliAction, String>
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
        "--help" | "-h" => Ok(BakeCliAction::Help),
        "probe-gpu" => {
            if let Some(arg) = it.next() {
                if arg == "--help" || arg == "-h" {
                    return Ok(BakeCliAction::Help);
                }
                return Err(format!("unknown flag '{arg}' for probe-gpu"));
            }
            Ok(BakeCliAction::ProbeGpu)
        }
        "fields" => parse_fields_args(it),
        "lighting" => parse_lighting_args(it),
        "pack" => parse_pack_args(it),
        "verify" => parse_verify_args(it),
        "assets" => parse_assets_args(it),
        other => Err(format!("unknown subcommand '{other}'\n{}", usage_text())),
    }
}

fn parse_fields_args(mut it: impl Iterator<Item = String>) -> Result<BakeCliAction, String> {
    let mut raw: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut tile_strip: Option<TileStrip> = None;
    let mut merge_strips: Option<usize> = None;

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(BakeCliAction::Help),
            "--raw" => raw = Some(PathBuf::from(next_val(&mut it, "--raw")?)),
            "--out" => out = Some(PathBuf::from(next_val(&mut it, "--out")?)),
            "--tile-strip" => {
                let s = next_val(&mut it, "--tile-strip")?;
                tile_strip = Some(TileStrip::parse(&s).map_err(|e| e.to_string())?);
            }
            "--merge-strips" => {
                let s = next_val(&mut it, "--merge-strips")?;
                merge_strips = Some(
                    s.parse()
                        .map_err(|e| format!("invalid --merge-strips: {e}"))?,
                );
            }
            other => return Err(format!("unknown fields flag '{other}'")),
        }
    }

    let out = out.ok_or_else(|| "fields requires --out".to_owned())?;
    if merge_strips.is_none() && raw.is_none() {
        return Err("fields requires --raw <DIR> (unless --merge-strips <K> is passed)".to_owned());
    }
    Ok(BakeCliAction::Fields(FieldsArgs {
        raw: raw.unwrap_or_else(|| out.clone()),
        out,
        tile_strip,
        merge_strips,
    }))
}

fn parse_lighting_args(mut it: impl Iterator<Item = String>) -> Result<BakeCliAction, String> {
    let mut backend = "cpu".to_owned();
    let mut raw: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut tile_range: Option<TileRange> = None;
    let mut samples = 64_u32;
    let mut merge_size: Option<usize> = None;

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(BakeCliAction::Help),
            "--backend" => {
                backend = next_val(&mut it, "--backend")?;
                if backend != "cpu" && backend != "wgpu" {
                    return Err(format!(
                        "invalid --backend '{backend}': expected cpu or wgpu"
                    ));
                }
            }
            "--raw" => raw = Some(PathBuf::from(next_val(&mut it, "--raw")?)),
            "--out" => out = Some(PathBuf::from(next_val(&mut it, "--out")?)),
            "--tile-range" => {
                let s = next_val(&mut it, "--tile-range")?;
                tile_range = Some(TileRange::parse(&s).map_err(|e| e.to_string())?);
            }
            "--samples" => {
                let s = next_val(&mut it, "--samples")?;
                samples = s.parse().map_err(|e| format!("invalid --samples: {e}"))?;
            }
            "--merge-size" => {
                let s = next_val(&mut it, "--merge-size")?;
                merge_size = Some(
                    s.parse()
                        .map_err(|e| format!("invalid --merge-size: {e}"))?,
                );
            }
            other => return Err(format!("unknown lighting flag '{other}'")),
        }
    }

    let out = out.ok_or_else(|| "lighting requires --out".to_owned())?;
    if merge_size.is_none() && raw.is_none() {
        return Err("lighting requires --raw <DIR> (unless --merge-size <N> is passed)".to_owned());
    }
    Ok(BakeCliAction::Lighting(LightingArgs {
        backend,
        raw: raw.unwrap_or_else(|| out.clone()),
        out,
        tile_range,
        samples,
        merge_size,
    }))
}

fn parse_pack_args(mut it: impl Iterator<Item = String>) -> Result<BakeCliAction, String> {
    let mut in_dir: Option<PathBuf> = None;
    let mut raw_dir: Option<PathBuf> = None;
    let mut sim_path: Option<PathBuf> = None;
    let mut release_tag: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut verify = false;

    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(BakeCliAction::Help),
            "--in" => in_dir = Some(PathBuf::from(next_val(&mut it, "--in")?)),
            "--raw" => raw_dir = Some(PathBuf::from(next_val(&mut it, "--raw")?)),
            "--sim" => sim_path = Some(PathBuf::from(next_val(&mut it, "--sim")?)),
            "--release-tag" => release_tag = Some(next_val(&mut it, "--release-tag")?),
            "--out" => out_dir = Some(PathBuf::from(next_val(&mut it, "--out")?)),
            "--verify" => verify = true,
            other => return Err(format!("unknown pack flag '{other}'")),
        }
    }

    let in_dir = in_dir.ok_or_else(|| "pack requires --in <DIR>".to_owned())?;
    let release_tag = release_tag.ok_or_else(|| "pack requires --release-tag <TAG>".to_owned())?;
    let out_dir = out_dir.ok_or_else(|| "pack requires --out <DIR>".to_owned())?;
    Ok(BakeCliAction::Pack(PackArgs {
        in_dir,
        raw_dir,
        sim_path,
        release_tag,
        out_dir,
        verify,
    }))
}

fn parse_verify_args(mut it: impl Iterator<Item = String>) -> Result<BakeCliAction, String> {
    let mut index_path: Option<PathBuf> = None;
    let mut sim_path: Option<PathBuf> = None;
    let mut archive_path: Option<PathBuf> = None;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(BakeCliAction::Help),
            "--index" => index_path = Some(PathBuf::from(next_val(&mut it, "--index")?)),
            "--sim" => sim_path = Some(PathBuf::from(next_val(&mut it, "--sim")?)),
            "--archive" => archive_path = Some(PathBuf::from(next_val(&mut it, "--archive")?)),
            other => return Err(format!("unknown verify flag '{other}'")),
        }
    }
    Ok(BakeCliAction::Verify(VerifyArgs {
        index_path: index_path.ok_or_else(|| "verify requires --index <PATH>".to_owned())?,
        sim_path: sim_path.ok_or_else(|| "verify requires --sim <PATH>".to_owned())?,
        archive_path: archive_path.ok_or_else(|| "verify requires --archive <PATH>".to_owned())?,
    }))
}

fn parse_assets_args(mut it: impl Iterator<Item = String>) -> Result<BakeCliAction, String> {
    let mut manifest: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut atlas_size = 512_usize;
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--help" | "-h" => return Ok(BakeCliAction::Help),
            "--manifest" => manifest = Some(PathBuf::from(next_val(&mut it, "--manifest")?)),
            "--out" => out = Some(PathBuf::from(next_val(&mut it, "--out")?)),
            "--atlas-size" => {
                let s = next_val(&mut it, "--atlas-size")?;
                atlas_size = s
                    .parse()
                    .map_err(|e| format!("invalid --atlas-size: {e}"))?;
            }
            other => return Err(format!("unknown assets flag '{other}'")),
        }
    }
    Ok(BakeCliAction::Assets(AssetsArgs {
        manifest: manifest.ok_or_else(|| "assets requires --manifest <PATH>".to_owned())?,
        out: out.ok_or_else(|| "assets requires --out <DIR>".to_owned())?,
        atlas_size,
    }))
}

fn main() -> ExitCode {
    let action = match parse_bake_args(std::env::args()) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(2);
        }
    };

    match action {
        BakeCliAction::Help => {
            print!("{}", usage_text());
            ExitCode::SUCCESS
        }
        BakeCliAction::ProbeGpu => {
            #[cfg(feature = "gpu-bake")]
            {
                match probe_gpu() {
                    Ok(rep) => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&rep).unwrap_or_else(|_| "{}".to_owned())
                        );
                        ExitCode::SUCCESS
                    }
                    Err(err) => {
                        eprintln!("probe-gpu failed: {err}");
                        ExitCode::from(1)
                    }
                }
            }
            #[cfg(not(feature = "gpu-bake"))]
            {
                eprintln!("probe-gpu requires compiling anima-bake with `--features gpu-bake`");
                ExitCode::from(2)
            }
        }
        BakeCliAction::Fields(args) => {
            if let Some(k) = args.merge_strips {
                if let Err(err) = merge_field_strips(&args.out, k) {
                    eprintln!("fields --merge-strips failed: {err}");
                    return ExitCode::from(1);
                }
                println!("merged {k} strips into {}", args.out.display());
                return ExitCode::SUCCESS;
            }
            match bake_fields_from_raw_dir(&args.raw, &args.out, args.tile_strip) {
                Ok(rep) => {
                    println!(
                        "baked fields {}x{} rows={}..{} normal_sha={} hydro_sha={}",
                        rep.width,
                        rep.height,
                        rep.row_start,
                        rep.row_end,
                        &rep.normal_xy_sha256[..12],
                        &rep.hydro_flow_sha256[..12]
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("fields failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        BakeCliAction::Lighting(args) => {
            if let Some(sz) = args.merge_size {
                if let Err(err) = merge_lighting_tiles(&args.out, sz, sz) {
                    eprintln!("lighting --merge-size failed: {err}");
                    return ExitCode::from(1);
                }
                println!(
                    "merged lighting tiles for {sz}x{sz} into {}",
                    args.out.display()
                );
                return ExitCode::SUCCESS;
            }
            let res = if args.backend == "wgpu" {
                #[cfg(feature = "gpu-bake")]
                {
                    bake_lighting_wgpu_from_raw_dir(
                        &args.raw,
                        &args.out,
                        args.tile_range,
                        args.samples,
                    )
                }
                #[cfg(not(feature = "gpu-bake"))]
                {
                    eprintln!("--backend wgpu requires building with `--features gpu-bake`");
                    return ExitCode::from(2);
                }
            } else {
                bake_lighting_cpu_from_raw_dir(&args.raw, &args.out, args.tile_range)
            };
            match res {
                Ok(rep) => {
                    println!(
                        "baked lighting ({}) {}x{} light_sha={} hor_sha={}",
                        rep.backend,
                        rep.width,
                        rep.height,
                        &rep.lighting_sha256[..12],
                        &rep.horizon_sha256[..12]
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("lighting failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        BakeCliAction::Pack(args) => {
            let raw_dir = args.raw_dir.unwrap_or_else(|| args.in_dir.join("raw"));
            let sim_path = args
                .sim_path
                .unwrap_or_else(|| args.in_dir.join("sim_256.anmw3"));
            match pack_world_v2_tiles(
                &args.in_dir,
                &raw_dir,
                &sim_path,
                &args.release_tag,
                &args.out_dir,
            ) {
                Ok(summary) => {
                    if args.verify {
                        let idx_p = args.out_dir.join("index.json");
                        let sim_p = args.out_dir.join("sim_256.anmw3");
                        let arch_p = args.out_dir.join(&summary.archive_filename);
                        if let Err(err) = verify_tile_pack(&idx_p, &sim_p, &arch_p) {
                            eprintln!("pack --verify failed: {err}");
                            return ExitCode::from(1);
                        }
                    }
                    println!(
                        "packed {}x{} archive={} ({} bytes, sha256={}) bundle_id={} sim_id={} bc5_psnr={:.2}dB ssim={:.5}",
                        summary.width,
                        summary.height,
                        summary.archive_filename,
                        summary.archive_byte_len,
                        &summary.release_archive_sha256[..12],
                        &summary.bundle_identity[..12],
                        &summary.sim_world_identity[..12],
                        summary.normal_xy_bc5_block_psnr_db,
                        summary.normal_xy_bc5_ssim
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("pack failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        BakeCliAction::Verify(args) => {
            match verify_tile_pack(&args.index_path, &args.sim_path, &args.archive_path) {
                Ok(rep) => {
                    println!(
                        "verified bundle_id={} entries={} ktx2={} archive_sha={}",
                        &rep.bundle_identity[..12],
                        rep.archive_entries_verified,
                        rep.ktx2_headers_verified,
                        &rep.release_archive_sha256[..12]
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("verify failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        BakeCliAction::Assets(args) => {
            match bake_assets_from_manifest(&args.manifest, &args.out, args.atlas_size) {
                Ok(rep) => {
                    println!(
                        "baked {} species into {} (atlas={}x{})",
                        rep.species_count,
                        args.out.display(),
                        rep.atlas_size,
                        rep.atlas_size
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("assets failed: {err}");
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
    fn cli_parses_all_bake_subcommands_and_rejects_invalid_flags() {
        assert_eq!(
            parse_bake_args(["anima-bake", "--help"]).unwrap(),
            BakeCliAction::Help
        );
        assert_eq!(
            parse_bake_args(["anima-bake", "probe-gpu"]).unwrap(),
            BakeCliAction::ProbeGpu
        );
        assert_eq!(
            parse_bake_args([
                "anima-bake",
                "fields",
                "--raw",
                "/tmp/raw",
                "--out",
                "/tmp/fields",
                "--tile-strip",
                "1/4"
            ])
            .unwrap(),
            BakeCliAction::Fields(FieldsArgs {
                raw: PathBuf::from("/tmp/raw"),
                out: PathBuf::from("/tmp/fields"),
                tile_strip: Some(TileStrip { index: 1, count: 4 }),
                merge_strips: None,
            })
        );
        assert_eq!(
            parse_bake_args([
                "anima-bake",
                "lighting",
                "--backend",
                "wgpu",
                "--raw",
                "/tmp/raw",
                "--out",
                "/tmp/light",
                "--tile-range",
                "0..4",
                "--samples",
                "64"
            ])
            .unwrap(),
            BakeCliAction::Lighting(LightingArgs {
                backend: "wgpu".to_owned(),
                raw: PathBuf::from("/tmp/raw"),
                out: PathBuf::from("/tmp/light"),
                tile_range: Some(TileRange { start: 0, end: 4 }),
                samples: 64,
                merge_size: None,
            })
        );
        assert!(parse_bake_args(["anima-bake"]).is_err());
        assert!(parse_bake_args(["anima-bake", "lighting", "--backend", "cuda"]).is_err());
        assert!(parse_bake_args([
            "anima-bake",
            "fields",
            "--raw",
            "/tmp/r",
            "--out",
            "/tmp/o",
            "--tile-strip",
            "4/4"
        ])
        .is_err());
    }
}
