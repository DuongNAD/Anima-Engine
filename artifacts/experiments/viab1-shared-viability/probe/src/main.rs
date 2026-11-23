use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use serde::Serialize;

use anima_engine_lib::core::experiment::{ExperimentManifest, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    run_manifest_seed, ExperimentModel, RunResult,
};
use anima_engine_lib::core::live_experiment::{
    run_live_drift_pair, DriftPairReport, LiveExperimentAdapter,
};
use anima_engine_lib::core::world_artifact::WorldIdentity;

const E3_BLIND_OBSERVABLES: [&str; 12] = [
    "plants",
    "detritus",
    "live.closed_eu_total",
    "live.agent_count",
    "live.herbivore_count",
    "live.predator_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

const SEL2P_BLIND_NON_DRIFT_OBSERVABLES: [&str; 8] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

const SEL2P_BLIND_DRIFT_OBSERVABLES: [&str; 6] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
];

const DEFOREST_CAUSE_ID: u32 = 1;
const ADD_NUTRIENT_CAUSE_ID: u32 = 2;
const CAUSE_PARTITION_TOLERANCE: f64 = 1e-6;
const E3_WP2_B1_MIN_REL_SHIFT: f64 = 0.10;
const E3_WP2_B2_MAX_REL_MISMATCH: f64 = 0.10;

const SEL2P_C1_MIN_AGENTS: f64 = 5.0;
const SEL2P_C2_G_LOW: f64 = 0.4;
const SEL2P_C2_G_HIGH: f64 = 0.6;
const SEL2P_C3_MIN_GENERATIONS: f64 = 30.0;
const SEL2P_C4_MIN_STARVATION_SHARE: f64 = 0.50;
const SEL2P_MIN_SURVIVING_BASELINE: usize = 3;

struct Args {
    manifest_root: PathBuf,
    out_dir: PathBuf,
    trunk_sha: String,
    suite: String,
    cells: Option<Vec<String>>,
    seeds: Vec<u64>,
    threads: usize,
}

fn parse_seeds(spec: &str) -> Result<Vec<u64>, String> {
    if let Some((start_s, end_s)) = spec.split_once("..=") {
        let start: u64 = start_s.parse().map_err(|e| format!("bad seed start: {e}"))?;
        let end: u64 = end_s.parse().map_err(|e| format!("bad seed end: {e}"))?;
        if start > end {
            return Err(format!("empty seed range {start}..={end}"));
        }
        Ok((start..=end).collect())
    } else {
        let mut out = Vec::new();
        for tok in spec.split(',') {
            let s: u64 = tok.trim().parse().map_err(|e| format!("bad seed {tok}: {e}"))?;
            out.push(s);
        }
        Ok(out)
    }
}

fn parse_args() -> Result<Args, String> {
    let mut manifest_root: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut trunk_sha: Option<String> = None;
    let mut suite: Option<String> = None;
    let mut cells: Option<Vec<String>> = None;
    let mut seeds: Vec<u64> = (880001..=880012).collect();
    let mut threads: usize = 12;

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        let flag = raw[i].as_str();
        let next = raw
            .get(i + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?
            .clone();
        match flag {
            "--manifest-root" => manifest_root = Some(PathBuf::from(next)),
            "--out" => out_dir = Some(PathBuf::from(next)),
            "--trunk-sha" => trunk_sha = Some(next),
            "--suite" => suite = Some(next),
            "--cells" => {
                if next != "all" {
                    cells = Some(
                        next.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    );
                }
            }
            "--seeds" => seeds = parse_seeds(&next)?,
            "--threads" => threads = next.parse().map_err(|e| format!("bad --threads: {e}"))?,
            other => return Err(format!("unknown arg: {other}")),
        }
        i += 2;
    }

    if seeds.is_empty() {
        return Err("no seeds specified".into());
    }
    for &s in &seeds {
        if !(880001..=880012).contains(&s) {
            return Err(format!(
                "refusing non-VIAB-1 seed {s} (allowed exploratory seeds: 880001..=880012)"
            ));
        }
    }

    Ok(Args {
        manifest_root: manifest_root.ok_or("--manifest-root required")?,
        out_dir: out_dir.ok_or("--out required")?,
        trunk_sha: trunk_sha.ok_or("--trunk-sha required")?,
        suite: suite.ok_or("--suite required (e3|sel2p)")?,
        cells,
        seeds,
        threads,
    })
}

fn verify_pre_run_host() -> Result<(), String> {
    if let Some(val) = std::env::var_os("ANIMA_WORLD_ARTIFACT") {
        return Err(format!(
            "ANIMA_WORLD_ARTIFACT must be unset, found {:?}",
            val
        ));
    }
    let tmp_artifact = std::env::temp_dir().join("anima_world_artifact.anmw");
    if tmp_artifact.exists() {
        return Err(format!(
            "desktop temp world artifact exists at {}",
            tmp_artifact.display()
        ));
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<ExperimentManifest, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str::<ExperimentManifest>(&raw)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

fn assert_blind_observables(obs: &[&str], label: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(obs.len());
    for &id in obs {
        if id.to_ascii_lowercase().contains("trait_mass") {
            return Err(format!("BLINDNESS VIOLATION in {label}: {id}"));
        }
        out.push(id.to_string());
    }
    Ok(out)
}

fn set_ic(m: &mut ExperimentManifest, key: &str, val: f64) {
    if let Some(entry) = m.initial_conditions.values.iter_mut().find(|(k, _)| k == key) {
        entry.1 = val;
    } else {
        m.initial_conditions.values.push((key.to_string(), val));
    }
}

fn get_ic(m: &ExperimentManifest, key: &str) -> Option<f64> {
    m.initial_conditions
        .values
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| *v)
}

fn measure_base_crop_cap_eu(scale1_manifest: &ExperimentManifest, seed: u64) -> Result<f64, String> {
    let model = LiveExperimentAdapter::from_manifest(
        &scale1_manifest.laws,
        &scale1_manifest.initial_conditions,
        &[],
        seed,
        (0, 0),
        1,
    )
    .map_err(|e| format!("base_crop_cap build failed: {e}"))?;
    let obs = model.observables();
    obs.iter()
        .find(|(k, _)| k == "live.standing_crop")
        .map(|(_, v)| *v)
        .ok_or_else(|| "missing live.standing_crop at tick 0".to_string())
}

fn sample_obs(sample: &[(String, f64)], key: &str) -> Result<f64, String> {
    sample
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("missing observable {key}"))
}

fn write_blind_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("json serialize failed: {e}"))?;
    if json.to_ascii_lowercase().contains("trait_mass") {
        return Err(format!(
            "BLINDNESS VIOLATION: output {} contains 'trait_mass'",
            path.display()
        ));
    }
    fs::write(path, json).map_err(|e| format!("write {} failed: {e}", path.display()))
}

// ============================================================================
// E3 suite
// ============================================================================

#[derive(Clone, Debug)]
struct E3CellDef {
    id: &'static str,
    description: &'static str,
    overrides: Vec<(&'static str, f64)>,
}

#[derive(Serialize)]
struct E3ArmMetrics {
    arm: String,
    seed: u64,
    completed: bool,
    survived: bool,
    extinct_tick: Option<u64>,
    final_checksum: u32,
    terrain_source: Option<String>,
    world_identity: Option<WorldIdentity>,
    min_n: f64,
    mean_n: f64,
    max_n: f64,
    final_n: f64,
    herbivore_final: f64,
    predator_final: f64,
    n_at_ticks: BTreeMap<u64, f64>,
    total_births: f64,
    total_deaths: f64,
    total_starvation: f64,
    total_dehydration: f64,
    starvation_share: Option<f64>,
    cause_partition_ok: bool,
    crop_post_warmup: f64,
    crop_final: f64,
    pasture_fill_post_warmup: f64,
    detritus_pos_before_3000: Option<(u64, f64)>,
    detritus_at_3000: Option<f64>,
    deforest_ledger_count: usize,
    add_nutrient_ledger_count: usize,
    max_abs_eu_drift: f64,
    closed_eu_spread: f64,
}

#[derive(Serialize)]
struct E3SeedGateRow {
    seed: u64,
    u: E3ArmMetrics,
    s: E3ArmMetrics,
    t: E3ArmMetrics,
    all_survived: bool,
    min_n_all_arms: f64,
    b1_rel_shift_t_vs_u: Option<f64>,
    b1_passed: bool,
    b2_rel_mismatch_vs_t: Option<f64>,
    b2_rel_mismatch_vs_s: Option<f64>,
    b2_passed: bool,
    c_passed: bool,
    wp2_all_passed: bool,
}

#[derive(Serialize)]
struct E3CellReport {
    suite: &'static str,
    trunk_sha: String,
    cell_id: String,
    description: String,
    overrides: Vec<(String, f64)>,
    base_crop_cap_eu: f64,
    elapsed_secs: f64,
    seeds_evaluated: Vec<u64>,
    world_identities_observed: Vec<WorldIdentity>,
    all_terrain_generated: bool,
    per_seed: Vec<E3SeedGateRow>,
}

fn summarize_e3_arm(
    arm: &str,
    seed: u64,
    scale: f64,
    base_crop_cap_eu: f64,
    run: &RunResult,
) -> Result<E3ArmMetrics, String> {
    let completed = run.status.is_completed();
    let final_n = sample_obs(&run.final_observables, "live.agent_count")?;
    let herbivore_final = sample_obs(&run.final_observables, "live.herbivore_count")?;
    let predator_final = sample_obs(&run.final_observables, "live.predator_count")?;
    let crop_final = sample_obs(&run.final_observables, "live.standing_crop")?;
    let eu_final = sample_obs(&run.final_observables, "live.closed_eu_total")?;

    let target_ticks: [u64; 6] = [600, 1800, 3000, 6000, 12000, 18000];
    let mut n_at_ticks = BTreeMap::new();
    let mut min_n = f64::INFINITY;
    let mut max_n = f64::NEG_INFINITY;
    let mut sum_n = 0.0;
    let mut count_n = 0usize;
    let mut extinct_tick: Option<u64> = None;

    let mut total_births = 0.0;
    let mut total_deaths = 0.0;
    let mut total_starvation = 0.0;
    let mut total_dehydration = 0.0;
    let mut per_sample_part_ok = true;

    let mut crop_pw_sum = 0.0;
    let mut crop_pw_n = 0usize;

    let mut detritus_pos_before_3000 = None;
    let mut detritus_at_3000 = None;

    let mut eu_vals = Vec::with_capacity(run.series.len() + 1);

    for s in &run.series {
        let n = sample_obs(&s.observables, "live.agent_count")?;
        let sc = sample_obs(&s.observables, "live.standing_crop")?;
        let det = sample_obs(&s.observables, "detritus")?;
        let eu = sample_obs(&s.observables, "live.closed_eu_total")?;
        let b = sample_obs(&s.observables, "live.births")?;
        let d = sample_obs(&s.observables, "live.deaths")?;
        let ds = sample_obs(&s.observables, "live.deaths_starvation")?;
        let dh = sample_obs(&s.observables, "live.deaths_dehydration")?;

        min_n = min_n.min(n);
        max_n = max_n.max(n);
        sum_n += n;
        count_n += 1;
        if n == 0.0 && extinct_tick.is_none() {
            extinct_tick = Some(s.tick);
        }
        if target_ticks.contains(&s.tick) {
            n_at_ticks.insert(s.tick, n);
        }
        if s.tick > 3_000 {
            crop_pw_sum += sc;
            crop_pw_n += 1;
        }
        if s.tick < 3_000 && det > 0.0 && detritus_pos_before_3000.is_none() {
            detritus_pos_before_3000 = Some((s.tick, det));
        }
        if s.tick == 3_000 {
            detritus_at_3000 = Some(det);
        }

        total_births += b;
        total_deaths += d;
        total_starvation += ds;
        total_dehydration += dh;
        if ((ds + dh) - d).abs() > CAUSE_PARTITION_TOLERANCE {
            per_sample_part_ok = false;
        }
        eu_vals.push(eu);
    }
    eu_vals.push(eu_final);

    if extinct_tick.is_none() && final_n == 0.0 {
        extinct_tick = Some(18_000);
    }
    let survived = completed && extinct_tick.is_none() && final_n > 0.0;
    let cause_partition_ok = per_sample_part_ok
        && ((total_starvation + total_dehydration) - total_deaths).abs()
            <= CAUSE_PARTITION_TOLERANCE;
    let starvation_share = if cause_partition_ok && total_deaths > 0.0 {
        Some(total_starvation / total_deaths)
    } else {
        None
    };

    let mean_n = if count_n > 0 {
        sum_n / count_n as f64
    } else {
        0.0
    };
    let crop_post_warmup = if crop_pw_n > 0 {
        crop_pw_sum / crop_pw_n as f64
    } else {
        0.0
    };
    let expected_cap = base_crop_cap_eu * scale;
    let pasture_fill_post_warmup = if expected_cap > 0.0 {
        crop_post_warmup / expected_cap
    } else {
        0.0
    };

    let eu_first = eu_vals.first().copied().unwrap_or(0.0);
    let eu_min = eu_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let eu_max = eu_vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let max_abs_eu_drift = eu_vals
        .iter()
        .map(|v| (*v - eu_first).abs())
        .fold(0.0, f64::max);
    let closed_eu_spread = (eu_max - eu_min).max(0.0);

    let mut deforest_ledger_count = 0usize;
    let mut add_nutrient_ledger_count = 0usize;
    for rec in run.ledger.all() {
        if rec.cause_id == DEFOREST_CAUSE_ID && rec.target == "plants" {
            deforest_ledger_count += 1;
        } else if rec.cause_id == ADD_NUTRIENT_CAUSE_ID && rec.target == "plants" {
            add_nutrient_ledger_count += 1;
        }
    }

    Ok(E3ArmMetrics {
        arm: arm.to_string(),
        seed,
        completed,
        survived,
        extinct_tick,
        final_checksum: run.final_checksum,
        terrain_source: run.provenance.terrain_source.clone(),
        world_identity: run.provenance.world_identity,
        min_n: min_n.min(final_n),
        mean_n,
        max_n: max_n.max(final_n),
        final_n,
        herbivore_final,
        predator_final,
        n_at_ticks,
        total_births,
        total_deaths,
        total_starvation,
        total_dehydration,
        starvation_share,
        cause_partition_ok,
        crop_post_warmup,
        crop_final,
        pasture_fill_post_warmup,
        detritus_pos_before_3000,
        detritus_at_3000,
        deforest_ledger_count,
        add_nutrient_ledger_count,
        max_abs_eu_drift,
        closed_eu_spread,
    })
}

fn e3_cell_defs() -> Vec<E3CellDef> {
    vec![
        E3CellDef {
            id: "e3_a_reg",
            description: "Registered E3 baseline (f=120, pf=0.3, lakes=2, layout=0, dehyd=120, cap=80, strict=0, grass=0)",
            overrides: vec![],
        },
        E3CellDef {
            id: "e3_b_strict",
            description: "E3 + live.strict_prey_cap=1",
            overrides: vec![("live.strict_prey_cap", 1.0)],
        },
        E3CellDef {
            id: "e3_c_grass",
            description: "E3 + live.grass_steering=1",
            overrides: vec![("live.grass_steering", 1.0)],
        },
        E3CellDef {
            id: "e3_d_both",
            description: "E3 + strict_prey_cap=1 + grass_steering=1",
            overrides: vec![("live.strict_prey_cap", 1.0), ("live.grass_steering", 1.0)],
        },
        E3CellDef {
            id: "e3_e1_sym100",
            description: "E3 + both + founders=100, prey_cap=100 (all 3 arms)",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
            ],
        },
        E3CellDef {
            id: "e3_e2_sym160",
            description: "E3 + both + founders=160, prey_cap=160 (all 3 arms)",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
            ],
        },
        E3CellDef {
            id: "e3_f1_lakes16",
            description: "E3 + both + lakes=16, lake_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_f2_dehyd960",
            description: "E3 + both + dehydration_ticks=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_f3_lakes16_dehyd960",
            description: "E3 + both + lakes=16, lake_layout=1, dehydration_ticks=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_f4_sym100_water",
            description: "E3 + both + sym100 (f=100, cap=100) + lakes=16, lake_layout=1, dehydration_ticks=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_f5_sym160_water",
            description: "E3 + both + sym160 (f=160, cap=160) + lakes=16, lake_layout=1, dehydration_ticks=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_f6_herb_sym100_water",
            description: "E3 + both + sym100 + lakes=16, lake_layout=1, dehyd=960 + predator_fraction=0",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.predator_fraction", 0.0),
            ],
        },
        E3CellDef {
            id: "e3_g1_strict_lakes16",
            description: "E3 + strict_prey_cap=1, grass=0 + lakes=16, lake_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_g2_strict_lakes16_dehyd960",
            description: "E3 + strict_prey_cap=1, grass=0 + lakes=16, lake_layout=1, dehydration_ticks=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_g3_strict_sym100_water",
            description: "E3 + strict=1, grass=0 + sym100 (f=100, cap=100) + lakes=16, lake_layout=1, dehyd=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_g4_strict_sym160_water",
            description: "E3 + strict=1, grass=0 + sym160 (f=160, cap=160) + lakes=16, lake_layout=1, dehyd=960",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
            ],
        },
        E3CellDef {
            id: "e3_g5_strict_herb_sym100_water",
            description: "E3 + strict=1, grass=0 + sym100 + lakes=16, lake_layout=1, dehyd=960 + predator_fraction=0",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.predator_fraction", 0.0),
            ],
        },
        E3CellDef {
            id: "e3_g6_strict_herb_sym160_water",
            description: "E3 + strict=1, grass=0 + sym160 + lakes=16, lake_layout=1, dehyd=960 + predator_fraction=0",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.predator_fraction", 0.0),
            ],
        },
        E3CellDef {
            id: "e3_b1_reg_fl1",
            description: "Phase B: Registered E3 + live.founder_layout=1 (walkable grid)",
            overrides: vec![("live.founder_layout", 1.0)],
        },
        E3CellDef {
            id: "e3_b2_reg_fl2",
            description: "Phase B: Registered E3 + live.founder_layout=2 (walkable scatter)",
            overrides: vec![("live.founder_layout", 2.0)],
        },
        E3CellDef {
            id: "e3_b1_strict_fl1",
            description: "Phase B: E3 + strict_prey_cap=1 + founder_layout=1",
            overrides: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
        },
        E3CellDef {
            id: "e3_b2_strict_fl2",
            description: "Phase B: E3 + strict_prey_cap=1 + founder_layout=2",
            overrides: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
        },
        E3CellDef {
            id: "e3_b1_g2_fl1",
            description: "Phase B: E3 g2 (strict=1, lakes=16, ll=1, dehyd=960) + founder_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_b2_g2_fl2",
            description: "Phase B: E3 g2 (strict=1, lakes=16, ll=1, dehyd=960) + founder_layout=2",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 2.0),
            ],
        },
        E3CellDef {
            id: "e3_b1_g3_fl1",
            description: "Phase B: E3 g3 (strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960) + founder_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_b2_g3_fl2",
            description: "Phase B: E3 g3 (strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960) + founder_layout=2",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 2.0),
            ],
        },
        E3CellDef {
            id: "e3_b1_g4_fl1",
            description: "Phase B: E3 g4 (strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960) + founder_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_b2_g4_fl2",
            description: "Phase B: E3 g4 (strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960) + founder_layout=2",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.founder_layout", 2.0),
            ],
        },
        E3CellDef {
            id: "e3_b1_g5_fl1",
            description: "Phase B: E3 g5 herbivore (strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0) + founder_layout=1",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.predator_fraction", 0.0),
                ("live.founder_layout", 1.0),
            ],
        },
        E3CellDef {
            id: "e3_b2_g5_fl2",
            description: "Phase B: E3 g5 herbivore (strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0) + founder_layout=2",
            overrides: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.lakes", 16.0),
                ("live.lake_layout", 1.0),
                ("live.dehydration_ticks", 960.0),
                ("live.predator_fraction", 0.0),
                ("live.founder_layout", 2.0),
            ],
        },
    ]
}

fn run_e3_suite(args: &Args, registry: &ObservableRegistry) -> Result<(), String> {
    let e3_dir = args.manifest_root.join("experiments_e3");
    let mut base_u = load_manifest(&e3_dir.join("e3-control-uniform.json"))?;
    let mut base_s = load_manifest(&e3_dir.join("e3-control-uniform-scarcity.json"))?;
    let mut base_t = load_manifest(&e3_dir.join("e3-treatment-gradient.json"))?;

    let blind_obs = assert_blind_observables(&E3_BLIND_OBSERVABLES, "e3")?;
    for m in [&mut base_u, &mut base_s, &mut base_t] {
        m.observable_ids = blind_obs.clone();
        m.seeds = args.seeds.clone();
    }

    let base_crop_cap_eu = measure_base_crop_cap_eu(&base_u, args.seeds[0])?;

    let defs = e3_cell_defs();
    let selected_defs: Vec<E3CellDef> = match &args.cells {
        Some(filter) => {
            let mut v = Vec::new();
            for def in defs {
                if filter.iter().any(|c| c == def.id) {
                    v.push(def);
                }
            }
            if v.is_empty() {
                return Err("no matching E3 cell ids".into());
            }
            v
        }
        None => defs,
    };

    for def in selected_defs {
        let cell_start = Instant::now();
        let mut mu = base_u.clone();
        let mut ms = base_s.clone();
        let mut mt = base_t.clone();
        for &(k, v) in &def.overrides {
            set_ic(&mut mu, k, v);
            set_ic(&mut ms, k, v);
            set_ic(&mut mt, k, v);
        }
        for m in [&mu, &ms, &mt] {
            m.validate(registry)
                .map_err(|e| format!("manifest validate failed for {}: {e}", def.id))?;
        }

        let u_scale = get_ic(&mu, "live.resource_capacity_scale").unwrap_or(1.0);
        let s_scale = get_ic(&ms, "live.resource_capacity_scale").unwrap_or(0.42);
        let t_scale = get_ic(&mt, "live.resource_capacity_scale").unwrap_or(1.0);

        let mut jobs: Vec<(usize, u8, u64)> = Vec::with_capacity(args.seeds.len() * 3);
        for (sidx, &seed) in args.seeds.iter().enumerate() {
            jobs.push((sidx, 0, seed));
            jobs.push((sidx, 1, seed));
            jobs.push((sidx, 2, seed));
        }

        let results: Vec<Result<(usize, u8, E3ArmMetrics), String>> = jobs
            .par_iter()
            .map(|&(sidx, arm_idx, seed)| {
                let (arm_name, manifest, scale) = match arm_idx {
                    0 => ("control_uniform", &mu, u_scale),
                    1 => ("control_uniform_scarcity", &ms, s_scale),
                    _ => ("treatment_gradient", &mt, t_scale),
                };
                let run =
                    run_manifest_seed::<LiveExperimentAdapter>(manifest, registry, seed, None, None);
                let metrics = summarize_e3_arm(arm_name, seed, scale, base_crop_cap_eu, &run)?;
                Ok((sidx, arm_idx, metrics))
            })
            .collect();

        let mut grouped: BTreeMap<usize, [Option<E3ArmMetrics>; 3]> = BTreeMap::new();
        for item in results {
            let (sidx, arm_idx, metrics) = item?;
            grouped.entry(sidx).or_insert([None, None, None])[arm_idx as usize] = Some(metrics);
        }

        let mut wid_set: BTreeSet<String> = BTreeSet::new();
        let mut wid_vec: Vec<WorldIdentity> = Vec::new();
        let mut all_terrain_generated = true;
        let mut per_seed = Vec::with_capacity(args.seeds.len());

        for (sidx, &seed) in args.seeds.iter().enumerate() {
            let mut slot = grouped.remove(&sidx).ok_or("missing seed group")?;
            let u = slot[0].take().ok_or("missing U")?;
            let s = slot[1].take().ok_or("missing S")?;
            let t = slot[2].take().ok_or("missing T")?;

            for arm in [&u, &s, &t] {
                if arm.terrain_source.as_deref() != Some("generated") {
                    all_terrain_generated = false;
                }
                if let Some(wid) = arm.world_identity {
                    let key = format!(
                        "{}:{}:{}x{}:{}",
                        wid.seed, wid.generator_version, wid.width, wid.height, wid.checksum
                    );
                    if wid_set.insert(key) {
                        wid_vec.push(wid);
                    }
                }
            }

            let all_survived = u.survived && s.survived && t.survived;
            let min_n_all_arms = u.min_n.min(s.min_n).min(t.min_n);
            let b1_rel_shift_t_vs_u = if u.crop_post_warmup > 0.0 {
                Some((t.crop_post_warmup - u.crop_post_warmup) / u.crop_post_warmup)
            } else {
                None
            };
            let b1_passed = u.final_checksum != t.final_checksum
                && t.deforest_ledger_count > 0
                && t.add_nutrient_ledger_count > 0
                && b1_rel_shift_t_vs_u
                    .is_some_and(|r| r.abs() >= E3_WP2_B1_MIN_REL_SHIFT);

            let b2_rel_mismatch_vs_t = if t.crop_post_warmup > 0.0 {
                Some((t.crop_post_warmup - s.crop_post_warmup) / t.crop_post_warmup)
            } else {
                None
            };
            let b2_rel_mismatch_vs_s = if s.crop_post_warmup > 0.0 {
                Some((t.crop_post_warmup - s.crop_post_warmup) / s.crop_post_warmup)
            } else {
                None
            };
            let b2_passed = s.final_checksum != t.final_checksum
                && s.deforest_ledger_count == 0
                && s.add_nutrient_ledger_count > 0
                && b2_rel_mismatch_vs_t
                    .is_some_and(|r| r.abs() <= E3_WP2_B2_MAX_REL_MISMATCH);

            let c_passed = [&u, &s, &t].iter().all(|a| {
                a.detritus_pos_before_3000.is_some()
                    && a.detritus_at_3000.is_some_and(|v| v > 0.0)
            });
            let wp2_all_passed = b1_passed && b2_passed && c_passed;

            per_seed.push(E3SeedGateRow {
                seed,
                u,
                s,
                t,
                all_survived,
                min_n_all_arms,
                b1_rel_shift_t_vs_u,
                b1_passed,
                b2_rel_mismatch_vs_t,
                b2_rel_mismatch_vs_s,
                b2_passed,
                c_passed,
                wp2_all_passed,
            });
        }

        if !all_terrain_generated {
            return Err(format!("non-generated terrain in {}", def.id));
        }

        let elapsed_secs = cell_start.elapsed().as_secs_f64();
        let u_surv = per_seed.iter().filter(|r| r.u.survived).count();
        let s_surv = per_seed.iter().filter(|r| r.s.survived).count();
        let t_surv = per_seed.iter().filter(|r| r.t.survived).count();
        let wp2_ok = per_seed.iter().filter(|r| r.wp2_all_passed).count();
        let min10 = per_seed
            .iter()
            .filter(|r| r.all_survived && r.min_n_all_arms >= 10.0)
            .count();

        let report = E3CellReport {
            suite: "e3",
            trunk_sha: args.trunk_sha.clone(),
            cell_id: def.id.to_string(),
            description: def.description.to_string(),
            overrides: def
                .overrides
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            base_crop_cap_eu,
            elapsed_secs,
            seeds_evaluated: args.seeds.clone(),
            world_identities_observed: wid_vec,
            all_terrain_generated,
            per_seed,
        };
        let out_path = args.out_dir.join(format!("{}.json", def.id));
        write_blind_json(&out_path, &report)?;
        println!(
            "[E3] {:24} ({:5.1}s): surv U={}/{} S={}/{} T={}/{} | all_minN>=10: {}/{} | wp2: {}/{}",
            def.id,
            elapsed_secs,
            u_surv,
            args.seeds.len(),
            s_surv,
            args.seeds.len(),
            t_surv,
            args.seeds.len(),
            min10,
            args.seeds.len(),
            wp2_ok,
            args.seeds.len()
        );
    }

    Ok(())
}

// ============================================================================
// SEL2-P suite
// ============================================================================

#[derive(Clone, Debug)]
struct Sel2pCellDef {
    id: &'static str,
    description: &'static str,
    overrides_all: Vec<(&'static str, f64)>,
    scarce_scale: Option<f64>,
    drift_reset_ticks: Option<f64>,
}

#[derive(Serialize)]
struct Sel2pArmMetrics {
    arm: String,
    seed: u64,
    completed: bool,
    survived: bool,
    extinct_tick: Option<u64>,
    final_checksum: u32,
    terrain_source: Option<String>,
    world_identity: Option<WorldIdentity>,
    min_n: f64,
    mean_n: f64,
    max_n: f64,
    final_n: f64,
    n_at_ticks: BTreeMap<u64, f64>,
    total_births: f64,
    total_deaths: f64,
    total_starvation: Option<f64>,
    total_dehydration: Option<f64>,
    starvation_share: Option<f64>,
    cause_partition_ok: Option<bool>,
    mean_lifespan_weighted: Option<f64>,
    generations_proxy: Option<f64>,
    births_last_quarter: f64,
    deaths_last_quarter: f64,
    crop_second_half: f64,
    pasture_fill_2nd_half: f64,
    max_abs_eu_drift: f64,
    closed_eu_spread: f64,
}

#[derive(Serialize)]
struct DriftDiagSummary {
    scheduled_deaths: u64,
    actual_deaths: u64,
    missed_deaths: u64,
    scheduled_births: u64,
    actual_births: u64,
    missed_births_extinct: u64,
    missed_births_insolvent: u64,
    missed_births_capped: u64,
    ticks_with_missed_deaths: u64,
    ticks_with_missed_births: u64,
    first_divergence_tick: Option<u64>,
    window_count: usize,
}

#[derive(Serialize)]
struct PooledBaselineSummary {
    n_requested: usize,
    n_surviving: usize,
    min_required: usize,
    surviving_seeds: Vec<u64>,
    pooled_crop_second_half: Option<f64>,
    sufficient: bool,
}

#[derive(Serialize)]
struct Sel2pSeedGateRow {
    seed: u64,
    baseline_ref: Sel2pArmMetrics,
    selected: Sel2pArmMetrics,
    scarce_mut0: Sel2pArmMetrics,
    scarce_drift: Sel2pArmMetrics,
    drift_diagnostics: DriftDiagSummary,
    g_selected: Option<f64>,
    g_scarce_mut0: Option<f64>,
    g_scarce_drift: Option<f64>,
    c1_min5: bool,
    c1_min10_scarce: bool,
    c1_min10_with_base: bool,
    c2: bool,
    c3: bool,
    c4: bool,
    c5: bool,
    drift_valid: bool,
    gate_pass: bool,
}

#[derive(Serialize)]
struct Sel2pCellReport {
    suite: &'static str,
    trunk_sha: String,
    cell_id: String,
    description: String,
    overrides_all: Vec<(String, f64)>,
    base_crop_cap_eu: f64,
    elapsed_secs: f64,
    seeds_evaluated: Vec<u64>,
    world_identities_observed: Vec<WorldIdentity>,
    all_terrain_generated: bool,
    pooled_baseline: PooledBaselineSummary,
    per_seed: Vec<Sel2pSeedGateRow>,
}

fn summarize_sel2p_arm(
    arm: &str,
    seed: u64,
    scale: f64,
    base_crop_cap_eu: f64,
    duration_ticks: u64,
    allow_cause_split: bool,
    run: &RunResult,
) -> Result<Sel2pArmMetrics, String> {
    let completed = run.status.is_completed();
    let final_n = sample_obs(&run.final_observables, "live.agent_count")?;
    let eu_final = sample_obs(&run.final_observables, "live.closed_eu_total")?;

    let target_ticks: [u64; 7] = [500, 1000, 2000, 5000, 20000, 100000, 200000];
    let mut n_at_ticks = BTreeMap::new();
    let mut min_n = f64::INFINITY;
    let mut max_n = f64::NEG_INFINITY;
    let mut sum_n = 0.0;
    let mut count_n = 0usize;
    let mut extinct_tick: Option<u64> = None;

    let mut total_births = 0.0;
    let mut total_deaths = 0.0;
    let mut lifespan_weighted = 0.0;
    let mut total_starvation = 0.0;
    let mut total_dehydration = 0.0;
    let mut per_sample_part_ok = true;
    let mut births_last_quarter = 0.0;
    let mut deaths_last_quarter = 0.0;
    let mut crop_2nd_sum = 0.0;
    let mut crop_2nd_n = 0usize;
    let mut eu_vals = Vec::with_capacity(run.series.len() + 1);

    for s in &run.series {
        let n = sample_obs(&s.observables, "live.agent_count")?;
        let sc = sample_obs(&s.observables, "live.standing_crop")?;
        let eu = sample_obs(&s.observables, "live.closed_eu_total")?;
        let b = sample_obs(&s.observables, "live.births")?;
        let d = sample_obs(&s.observables, "live.deaths")?;
        let ls = sample_obs(&s.observables, "live.mean_lifespan")?;

        min_n = min_n.min(n);
        max_n = max_n.max(n);
        sum_n += n;
        count_n += 1;
        if n == 0.0 && extinct_tick.is_none() {
            extinct_tick = Some(s.tick);
        }
        if target_ticks.contains(&s.tick) {
            n_at_ticks.insert(s.tick, n);
        }
        if s.tick.saturating_mul(2) > duration_ticks {
            crop_2nd_sum += sc;
            crop_2nd_n += 1;
        }
        if s.tick.saturating_mul(4) > duration_ticks.saturating_mul(3) {
            births_last_quarter += b;
            deaths_last_quarter += d;
        }
        total_births += b;
        total_deaths += d;
        lifespan_weighted += ls * d;

        if allow_cause_split {
            let ds = sample_obs(&s.observables, "live.deaths_starvation")?;
            let dh = sample_obs(&s.observables, "live.deaths_dehydration")?;
            total_starvation += ds;
            total_dehydration += dh;
            if ((ds + dh) - d).abs() > CAUSE_PARTITION_TOLERANCE {
                per_sample_part_ok = false;
            }
        }
        eu_vals.push(eu);
    }
    eu_vals.push(eu_final);

    if extinct_tick.is_none() && final_n == 0.0 {
        extinct_tick = Some(duration_ticks);
    }
    let survived = completed && extinct_tick.is_none() && final_n > 0.0;
    let mean_n = if count_n > 0 {
        sum_n / count_n as f64
    } else {
        0.0
    };
    let crop_second_half = if crop_2nd_n > 0 {
        crop_2nd_sum / crop_2nd_n as f64
    } else {
        0.0
    };
    let expected_cap = base_crop_cap_eu * scale;
    let pasture_fill_2nd_half = if expected_cap > 0.0 {
        crop_second_half / expected_cap
    } else {
        0.0
    };
    let mean_lifespan_weighted = if total_deaths > 0.0 {
        Some(lifespan_weighted / total_deaths)
    } else {
        None
    };
    let generations_proxy = match mean_lifespan_weighted {
        Some(ls) if ls.is_finite() && ls > 0.0 && mean_n > 0.0 => {
            let by_life = (duration_ticks as f64) / ls;
            let by_turn = total_births / mean_n.max(1.0);
            Some(by_life.min(by_turn))
        }
        _ => None,
    };

    let (starv_opt, dehyd_opt, starv_share, part_ok_opt) = if allow_cause_split {
        let total_ok = per_sample_part_ok
            && ((total_starvation + total_dehydration) - total_deaths).abs()
                <= CAUSE_PARTITION_TOLERANCE;
        let share = if total_ok && total_deaths > 0.0 {
            Some(total_starvation / total_deaths)
        } else {
            None
        };
        (
            Some(total_starvation),
            Some(total_dehydration),
            share,
            Some(total_ok),
        )
    } else {
        (None, None, None, None)
    };

    let eu_first = eu_vals.first().copied().unwrap_or(0.0);
    let eu_min = eu_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let eu_max = eu_vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let max_abs_eu_drift = eu_vals
        .iter()
        .map(|v| (*v - eu_first).abs())
        .fold(0.0, f64::max);
    let closed_eu_spread = (eu_max - eu_min).max(0.0);

    Ok(Sel2pArmMetrics {
        arm: arm.to_string(),
        seed,
        completed,
        survived,
        extinct_tick,
        final_checksum: run.final_checksum,
        terrain_source: run.provenance.terrain_source.clone(),
        world_identity: run.provenance.world_identity,
        min_n: min_n.min(final_n),
        mean_n,
        max_n: max_n.max(final_n),
        final_n,
        n_at_ticks,
        total_births,
        total_deaths,
        total_starvation: starv_opt,
        total_dehydration: dehyd_opt,
        starvation_share: starv_share,
        cause_partition_ok: part_ok_opt,
        mean_lifespan_weighted,
        generations_proxy,
        births_last_quarter,
        deaths_last_quarter,
        crop_second_half,
        pasture_fill_2nd_half,
        max_abs_eu_drift,
        closed_eu_spread,
    })
}

fn sel2p_cell_defs() -> Vec<Sel2pCellDef> {
    vec![
        Sel2pCellDef {
            id: "sel2p_a_reg",
            description: "Registered SEL2-P (base: f=100, cap=80, s=1.0; scarce: f=100, cap=320, s=0.63; strict=0, grass=0)",
            overrides_all: vec![],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_b_strict",
            description: "SEL2-P + live.strict_prey_cap=1 (all 4 arms)",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_c_grass",
            description: "SEL2-P + live.grass_steering=1 (all 4 arms)",
            overrides_all: vec![("live.grass_steering", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_d_both",
            description: "SEL2-P + strict_prey_cap=1 + grass_steering=1 (all 4 arms)",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.grass_steering", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_e1_sym100",
            description: "SEL2-P + both + symmetric founders=100, prey_cap=100 (all 4 arms incl. baseline)",
            overrides_all: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
            ],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_e2_sym160",
            description: "SEL2-P + both + symmetric founders=160, prey_cap=160 (all 4 arms incl. baseline)",
            overrides_all: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.grass_steering", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
            ],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_f1_strict_sym100_nograss",
            description: "SEL2-P + strict=1, grass=0 + symmetric founders=100, prey_cap=100 (all 4 arms)",
            overrides_all: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
            ],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_f2_strict_sym160_nograss",
            description: "SEL2-P + strict=1, grass=0 + symmetric founders=160, prey_cap=160 (all 4 arms)",
            overrides_all: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 160.0),
                ("live.prey_cap", 160.0),
            ],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_f3_strict_s075_cap320",
            description: "SEL2-P + strict=1, grass=0 + milder scarcity scale=0.75 (scarce arms only)",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_f4_strict_s063_k10k",
            description: "SEL2-P + strict=1, grass=0 + rolling drift_reset_ticks=10000 (scarce_drift only)",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(10_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_f5_strict_s063_k2k",
            description: "SEL2-P + strict=1, grass=0 + rolling drift_reset_ticks=2000 (scarce_drift only)",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_f6_strict_s100_cap80_mut0",
            description: "SEL2-P + strict=1, grass=0 + prey_cap=80, scarce scale=1.0 control",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.prey_cap", 80.0)],
            scarce_scale: Some(1.0),
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_b1_strict_s063_k0_fl1",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=0 + founder_layout=1",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_b2_strict_s063_k0_fl2",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=0 + founder_layout=2",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
            scarce_scale: None,
            drift_reset_ticks: None,
        },
        Sel2pCellDef {
            id: "sel2p_b1_strict_s063_k2k_fl1",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=2000 + founder_layout=1",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b2_strict_s063_k2k_fl2",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=2000 + founder_layout=2",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b1_strict_s063_k10k_fl1",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=10000 + founder_layout=1",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(10_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b2_strict_s063_k10k_fl2",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.63, K=10000 + founder_layout=2",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(10_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b1_strict_s075_k2k_fl1",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.75, cap=320, K=2000 + founder_layout=1",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b2_strict_s075_k2k_fl2",
            description: "Phase B1: SEL2-P + strict=1, grass=0, s=0.75, cap=320, K=2000 + founder_layout=2",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b1_strict_sym100_k2k_fl1",
            description: "Phase B1: SEL2-P + strict=1, grass=0, f=100, cap=100, s=0.63, K=2000 + founder_layout=1",
            overrides_all: vec![
                ("live.strict_prey_cap", 1.0),
                ("live.founders", 100.0),
                ("live.prey_cap", 100.0),
                ("live.founder_layout", 1.0),
            ],
            scarce_scale: None,
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b0_s063_k2k_fl0_12s",
            description: "Phase B2 12s: SEL2-P + strict=1, grass=0, s=0.63, K=2000 + founder_layout=0",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: None,
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b0_s075_k2k_fl0_12s",
            description: "Phase B2 12s: SEL2-P + strict=1, grass=0, s=0.75, cap=320, K=2000 + founder_layout=0",
            overrides_all: vec![("live.strict_prey_cap", 1.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b1_s075_k2k_fl1_12s",
            description: "Phase B2 12s: SEL2-P + strict=1, grass=0, s=0.75, cap=320, K=2000 + founder_layout=1",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 1.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: Some(2_000.0),
        },
        Sel2pCellDef {
            id: "sel2p_b2_s075_k2k_fl2_12s",
            description: "Phase B2 12s: SEL2-P + strict=1, grass=0, s=0.75, cap=320, K=2000 + founder_layout=2",
            overrides_all: vec![("live.strict_prey_cap", 1.0), ("live.founder_layout", 2.0)],
            scarce_scale: Some(0.75),
            drift_reset_ticks: Some(2_000.0),
        },
    ]
}

enum Sel2pJobOut {
    Baseline(usize, Sel2pArmMetrics),
    Mut0(usize, Sel2pArmMetrics),
    DriftPair(usize, Sel2pArmMetrics, Sel2pArmMetrics, DriftDiagSummary),
}

fn run_sel2p_suite(args: &Args, registry: &ObservableRegistry) -> Result<(), String> {
    let dir = args.manifest_root.join("experiments_sel2p");
    let mut base_ref = load_manifest(&dir.join("sel2p-baseline-ref.json"))?;
    let mut base_sel = load_manifest(&dir.join("sel2p-smoke-selected.json"))?;
    let mut base_mut0 = load_manifest(&dir.join("sel2p-smoke-scarce-mut0.json"))?;
    let mut base_drift = load_manifest(&dir.join("sel2p-smoke-scarce-drift.json"))?;

    let non_drift_obs =
        assert_blind_observables(&SEL2P_BLIND_NON_DRIFT_OBSERVABLES, "sel2p_non_drift")?;
    let drift_obs = assert_blind_observables(&SEL2P_BLIND_DRIFT_OBSERVABLES, "sel2p_drift")?;

    for m in [&mut base_ref, &mut base_sel, &mut base_mut0] {
        m.observable_ids = non_drift_obs.clone();
        m.seeds = args.seeds.clone();
    }
    base_drift.observable_ids = drift_obs;
    base_drift.seeds = args.seeds.clone();

    let base_crop_cap_eu = measure_base_crop_cap_eu(&base_ref, args.seeds[0])?;

    let defs = sel2p_cell_defs();
    let selected_defs: Vec<Sel2pCellDef> = match &args.cells {
        Some(filter) => {
            let mut v = Vec::new();
            for def in defs {
                if filter.iter().any(|c| c == def.id) {
                    v.push(def);
                }
            }
            if v.is_empty() {
                return Err("no matching SEL2-P cell ids".into());
            }
            v
        }
        None => defs,
    };

    for def in selected_defs {
        let cell_start = Instant::now();
        let mut mr = base_ref.clone();
        let mut ms = base_sel.clone();
        let mut mm = base_mut0.clone();
        let mut md = base_drift.clone();
        for &(k, v) in &def.overrides_all {
            set_ic(&mut mr, k, v);
            set_ic(&mut ms, k, v);
            set_ic(&mut mm, k, v);
            set_ic(&mut md, k, v);
        }
        if let Some(sc) = def.scarce_scale {
            set_ic(&mut ms, "live.resource_capacity_scale", sc);
            set_ic(&mut mm, "live.resource_capacity_scale", sc);
            set_ic(&mut md, "live.resource_capacity_scale", sc);
        }
        if let Some(k) = def.drift_reset_ticks {
            set_ic(&mut md, "live.drift_reset_ticks", k);
        }
        for m in [&mr, &ms, &mm, &md] {
            m.validate(registry)
                .map_err(|e| format!("manifest validate failed for {}: {e}", def.id))?;
        }

        let r_scale = get_ic(&mr, "live.resource_capacity_scale").unwrap_or(1.0);
        let s_scale = get_ic(&ms, "live.resource_capacity_scale").unwrap_or(0.63);
        let m_scale = get_ic(&mm, "live.resource_capacity_scale").unwrap_or(0.63);
        let d_scale = get_ic(&md, "live.resource_capacity_scale").unwrap_or(0.63);

        // Schedule drift-pair jobs first (roughly 2x single run cost) for better work-stealing balance.
        let mut jobs: Vec<(u8, usize, u64)> = Vec::with_capacity(args.seeds.len() * 3);
        for (sidx, &seed) in args.seeds.iter().enumerate() {
            jobs.push((2, sidx, seed));
        }
        for (sidx, &seed) in args.seeds.iter().enumerate() {
            jobs.push((0, sidx, seed));
            jobs.push((1, sidx, seed));
        }

        let outputs: Vec<Result<Sel2pJobOut, String>> = jobs
            .par_iter()
            .map(|&(kind, sidx, seed)| match kind {
                0 => {
                    let run = run_manifest_seed::<LiveExperimentAdapter>(
                        &mr, registry, seed, None, None,
                    );
                    let m = summarize_sel2p_arm(
                        "baseline_ref",
                        seed,
                        r_scale,
                        base_crop_cap_eu,
                        mr.duration_ticks,
                        true,
                        &run,
                    )?;
                    Ok(Sel2pJobOut::Baseline(sidx, m))
                }
                1 => {
                    let run = run_manifest_seed::<LiveExperimentAdapter>(
                        &mm, registry, seed, None, None,
                    );
                    let m = summarize_sel2p_arm(
                        "scarce_mut0",
                        seed,
                        m_scale,
                        base_crop_cap_eu,
                        mm.duration_ticks,
                        true,
                        &run,
                    )?;
                    Ok(Sel2pJobOut::Mut0(sidx, m))
                }
                _ => {
                    let mut ms_for_drift = ms.clone();
                    ms_for_drift.observable_ids = md.observable_ids.clone();
                    let pair: DriftPairReport =
                        run_live_drift_pair(&ms_for_drift, &md, registry, seed)
                            .map_err(|e| format!("drift pair failed ({} seed {seed}): {e}", def.id))?;
                    let sel_m = summarize_sel2p_arm(
                        "selected",
                        seed,
                        s_scale,
                        base_crop_cap_eu,
                        ms.duration_ticks,
                        true,
                        &pair.selected,
                    )?;
                    let drf_m = summarize_sel2p_arm(
                        "scarce_drift",
                        seed,
                        d_scale,
                        base_crop_cap_eu,
                        md.duration_ticks,
                        false,
                        &pair.drift,
                    )?;
                    let diag = DriftDiagSummary {
                        scheduled_deaths: pair.diagnostics.scheduled_deaths,
                        actual_deaths: pair.diagnostics.actual_deaths,
                        missed_deaths: pair.diagnostics.missed_deaths,
                        scheduled_births: pair.diagnostics.scheduled_births,
                        actual_births: pair.diagnostics.actual_births,
                        missed_births_extinct: pair.diagnostics.missed_births_extinct,
                        missed_births_insolvent: pair.diagnostics.missed_births_insolvent,
                        missed_births_capped: pair.diagnostics.missed_births_capped,
                        ticks_with_missed_deaths: pair.diagnostics.ticks_with_missed_deaths,
                        ticks_with_missed_births: pair.diagnostics.ticks_with_missed_births,
                        first_divergence_tick: pair.diagnostics.first_divergence_tick,
                        window_count: pair.window_count,
                    };
                    Ok(Sel2pJobOut::DriftPair(sidx, sel_m, drf_m, diag))
                }
            })
            .collect();

        let mut base_map: BTreeMap<usize, Sel2pArmMetrics> = BTreeMap::new();
        let mut mut0_map: BTreeMap<usize, Sel2pArmMetrics> = BTreeMap::new();
        let mut pair_map: BTreeMap<usize, (Sel2pArmMetrics, Sel2pArmMetrics, DriftDiagSummary)> =
            BTreeMap::new();

        for item in outputs {
            match item? {
                Sel2pJobOut::Baseline(sidx, m) => {
                    base_map.insert(sidx, m);
                }
                Sel2pJobOut::Mut0(sidx, m) => {
                    mut0_map.insert(sidx, m);
                }
                Sel2pJobOut::DriftPair(sidx, sel, drf, diag) => {
                    pair_map.insert(sidx, (sel, drf, diag));
                }
            }
        }

        let mut surviving_base_seeds = Vec::new();
        let mut base_crops = Vec::new();
        for (sidx, &seed) in args.seeds.iter().enumerate() {
            let b = base_map.get(&sidx).ok_or("missing baseline")?;
            if b.survived && b.crop_second_half.is_finite() {
                surviving_base_seeds.push(seed);
                base_crops.push(b.crop_second_half);
            }
        }
        let sufficient = surviving_base_seeds.len() >= SEL2P_MIN_SURVIVING_BASELINE;
        let pooled_crop = if sufficient && !base_crops.is_empty() {
            Some(base_crops.iter().sum::<f64>() / base_crops.len() as f64)
        } else {
            None
        };
        let pooled_baseline = PooledBaselineSummary {
            n_requested: args.seeds.len(),
            n_surviving: surviving_base_seeds.len(),
            min_required: SEL2P_MIN_SURVIVING_BASELINE,
            surviving_seeds: surviving_base_seeds,
            pooled_crop_second_half: pooled_crop,
            sufficient,
        };

        let mut wid_set: BTreeSet<String> = BTreeSet::new();
        let mut wid_vec: Vec<WorldIdentity> = Vec::new();
        let mut all_terrain_generated = true;
        let mut per_seed = Vec::with_capacity(args.seeds.len());

        for (sidx, &seed) in args.seeds.iter().enumerate() {
            let baseline_ref = base_map.remove(&sidx).ok_or("missing baseline")?;
            let scarce_mut0 = mut0_map.remove(&sidx).ok_or("missing mut0")?;
            let (selected, scarce_drift, drift_diagnostics) =
                pair_map.remove(&sidx).ok_or("missing pair")?;

            for arm in [&baseline_ref, &selected, &scarce_mut0, &scarce_drift] {
                if arm.terrain_source.as_deref() != Some("generated") {
                    all_terrain_generated = false;
                }
                if let Some(wid) = arm.world_identity {
                    let key = format!(
                        "{}:{}:{}x{}:{}",
                        wid.seed, wid.generator_version, wid.width, wid.height, wid.checksum
                    );
                    if wid_set.insert(key) {
                        wid_vec.push(wid);
                    }
                }
            }

            let c1_min5 = (selected.survived && selected.min_n >= SEL2P_C1_MIN_AGENTS)
                && (scarce_mut0.survived && scarce_mut0.min_n >= SEL2P_C1_MIN_AGENTS);
            let c1_min10_scarce = (selected.survived && selected.min_n >= 10.0)
                && (scarce_mut0.survived && scarce_mut0.min_n >= 10.0);
            let c1_min10_with_base = c1_min10_scarce
                && baseline_ref.survived
                && baseline_ref.min_n >= 10.0
                && scarce_drift.survived
                && scarce_drift.min_n >= 10.0;

            let ref_crop = pooled_crop.filter(|c| sufficient && c.is_finite() && *c > 0.0);
            let g_fn = |a: &Sel2pArmMetrics| -> Option<f64> {
                let rc = ref_crop?;
                if a.crop_second_half.is_finite() {
                    Some(a.crop_second_half / rc)
                } else {
                    None
                }
            };
            let g_selected = g_fn(&selected);
            let g_scarce_mut0 = g_fn(&scarce_mut0);
            let g_scarce_drift = g_fn(&scarce_drift);
            let g_ok = |g: Option<f64>| g.is_some_and(|v| (SEL2P_C2_G_LOW..=SEL2P_C2_G_HIGH).contains(&v));
            let c2 = sufficient && g_ok(g_selected) && g_ok(g_scarce_mut0);

            let c3 = selected
                .generations_proxy
                .is_some_and(|g| g >= SEL2P_C3_MIN_GENERATIONS)
                && scarce_mut0
                    .generations_proxy
                    .is_some_and(|g| g >= SEL2P_C3_MIN_GENERATIONS);

            let c4 = selected
                .starvation_share
                .is_some_and(|s| s > SEL2P_C4_MIN_STARVATION_SHARE)
                && scarce_mut0
                    .starvation_share
                    .is_some_and(|s| s > SEL2P_C4_MIN_STARVATION_SHARE);

            let c5_arm = |a: &Sel2pArmMetrics| a.births_last_quarter > 0.0 && a.deaths_last_quarter > 0.0;
            let c5 = c5_arm(&selected) && c5_arm(&scarce_mut0);
            let drift_valid = scarce_drift.survived;
            let gate_pass = c1_min5 && c2 && c3 && c4 && c5 && drift_valid;

            per_seed.push(Sel2pSeedGateRow {
                seed,
                baseline_ref,
                selected,
                scarce_mut0,
                scarce_drift,
                drift_diagnostics,
                g_selected,
                g_scarce_mut0,
                g_scarce_drift,
                c1_min5,
                c1_min10_scarce,
                c1_min10_with_base,
                c2,
                c3,
                c4,
                c5,
                drift_valid,
                gate_pass,
            });
        }

        if !all_terrain_generated {
            return Err(format!("non-generated terrain in {}", def.id));
        }

        let elapsed_secs = cell_start.elapsed().as_secs_f64();
        let b_surv = per_seed.iter().filter(|r| r.baseline_ref.survived).count();
        let sel_surv = per_seed.iter().filter(|r| r.selected.survived).count();
        let mut0_surv = per_seed.iter().filter(|r| r.scarce_mut0.survived).count();
        let drf_surv = per_seed.iter().filter(|r| r.scarce_drift.survived).count();
        let gate_ok = per_seed.iter().filter(|r| r.gate_pass).count();
        let min10_all = per_seed.iter().filter(|r| r.c1_min10_with_base).count();

        let report = Sel2pCellReport {
            suite: "sel2p",
            trunk_sha: args.trunk_sha.clone(),
            cell_id: def.id.to_string(),
            description: def.description.to_string(),
            overrides_all: def
                .overrides_all
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            base_crop_cap_eu,
            elapsed_secs,
            seeds_evaluated: args.seeds.clone(),
            world_identities_observed: wid_vec,
            all_terrain_generated,
            pooled_baseline,
            per_seed,
        };
        let out_path = args.out_dir.join(format!("{}.json", def.id));
        write_blind_json(&out_path, &report)?;
        println!(
            "[SEL2P] {:18} ({:5.1}s): surv B={}/{} S={}/{} M={}/{} D={}/{} | all_minN>=10: {}/{} | gate_pass: {}/{}",
            def.id,
            elapsed_secs,
            b_surv,
            args.seeds.len(),
            sel_surv,
            args.seeds.len(),
            mut0_surv,
            args.seeds.len(),
            drf_surv,
            args.seeds.len(),
            min10_all,
            args.seeds.len(),
            gate_ok,
            args.seeds.len()
        );
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("viab1_probe error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    verify_pre_run_host()?;
    let args = parse_args()?;
    fs::create_dir_all(&args.out_dir)
        .map_err(|e| format!("create_dir_all failed: {e}"))?;

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .map_err(|e| format!("rayon init failed: {e}"))?;

    let registry = ObservableRegistry::live_default();
    match args.suite.as_str() {
        "e3" => run_e3_suite(&args, &registry),
        "sel2p" => run_sel2p_suite(&args, &registry),
        other => Err(format!("unknown --suite {other} (expected e3|sel2p)")),
    }
}
