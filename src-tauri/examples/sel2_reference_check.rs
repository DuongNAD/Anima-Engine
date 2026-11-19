//! Card S15 exploratory measurement: candidate references for SEL2's standing-crop ratio `g`
//! at 16 spread lakes (`live.lake_layout = 1`, `live.plant_reserve = 0.0`,
//! `live.resource_capacity_scale = 0.63`).
//!
//! **Blind to body mass**, with the same allow-list as `sel2_lakes16_scale.rs` and
//! `sel2_spread_lakes.rs`; seeds 1–6 are exploratory, not confirmation seeds.
//!
//! Fixed world: 100 founders, `predator_fraction` 0.0, 8 trees, `lakes` 16.0, `lake_layout` 1.0,
//! `reserve` 0.0, `mortality` 1.0, `reproduction` 1.0, `locomotion` 1.0, `drive_floor` 0.5,
//! `thirst_steering` 1.0, `dehydration_ticks` 960, seeds 1–6, 200,000 ticks each, sampled every 50
//! ticks:
//! - Scarcity (6 runs): `prey_cap = 320, scale = 0.63` (must reproduce S14's numbers on Dell).
//! - Candidate references at `scale = 1.0` (18 runs): `prey_cap` ∈ `{80, 120, 160}`.
//!
//! Diagnostic additions (reported only, NOT used by declared `VIABLE`):
//! - `g_80` (declared rule), `g_120`, `g_160`, `g_pooled80` (ratio to mean second-half crop of
//!   surviving `prey_cap = 80` baselines), and `g_ungrazed`;
//! - `S14_PARITY` side-by-side comparison against S14's recorded `scale = 0.63` results;
//! - `SUMMARY_REF` C2 verdict for each candidate labelled `"diagnostic, not the declared rule"`.
//!
//! ```text
//! cargo run --release --example sel2_reference_check -- --probe
//! cargo run --release --example sel2_reference_check > s15_out.tsv
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const TICKS: u64 = 200_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const LAKE_LAYOUT: u8 = 1;
const FOUNDERS: f64 = 100.0;
const LAKES: f64 = 16.0;
const RESERVE: f64 = 0.0;
const SCARCITY_SCALE: f64 = 0.63;
const SCARCITY_PREY_CAP: f64 = 320.0;
const REF_PREY_CAPS: [f64; 3] = [80.0, 120.0, 160.0];
const GRACE: f64 = 960.0;
const DEFAULT_THREADS: usize = 4;

/// The only observables this driver reads. Nothing about body mass or traits.
const ALLOWED: [&str; 7] = [
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

/// Recorded S14 values for `lakes=16, scale=0.63, reserve=0, prey_cap=320` on Dell (`seeds 1..=6`),
/// printed side-by-side in `S14_PARITY` so exact reproduction is explicit.
#[derive(Clone, Copy)]
struct S14Record {
    seed: u64,
    g_str: &'static str,
    min_agents: u64,
    mean_str: &'static str,
    final_agents: u64,
    births: u64,
    deaths: u64,
    starvation: u64,
    dehydration: u64,
    births_last_q: u64,
    deaths_last_q: u64,
}

const S14_EXPECTED: [S14Record; 6] = [
    S14Record {
        seed: 1,
        g_str: "0.322",
        min_agents: 9,
        mean_str: "123.7",
        final_agents: 130,
        births: 5255,
        deaths: 5225,
        starvation: 3948,
        dehydration: 1277,
        births_last_q: 1330,
        deaths_last_q: 1307,
    },
    S14Record {
        seed: 2,
        g_str: "0.528",
        min_agents: 20,
        mean_str: "101.9",
        final_agents: 126,
        births: 3240,
        deaths: 3214,
        starvation: 2321,
        dehydration: 893,
        births_last_q: 996,
        deaths_last_q: 993,
    },
    S14Record {
        seed: 3,
        g_str: "0.299",
        min_agents: 10,
        mean_str: "133.4",
        final_agents: 161,
        births: 5682,
        deaths: 5621,
        starvation: 4260,
        dehydration: 1361,
        births_last_q: 1526,
        deaths_last_q: 1512,
    },
    S14Record {
        seed: 4,
        g_str: "0.520",
        min_agents: 41,
        mean_str: "115.8",
        final_agents: 91,
        births: 3616,
        deaths: 3625,
        starvation: 2588,
        dehydration: 1037,
        births_last_q: 971,
        deaths_last_q: 990,
    },
    S14Record {
        seed: 5,
        g_str: "0.546",
        min_agents: 25,
        mean_str: "96.7",
        final_agents: 143,
        births: 3171,
        deaths: 3128,
        starvation: 2254,
        dehydration: 874,
        births_last_q: 840,
        deaths_last_q: 827,
    },
    S14Record {
        seed: 6,
        g_str: "0.558",
        min_agents: 9,
        mean_str: "78.2",
        final_agents: 85,
        births: 2573,
        deaths: 2588,
        starvation: 1876,
        dehydration: 712,
        births_last_q: 804,
        deaths_last_q: 843,
    },
];

#[derive(Clone, Copy, Debug)]
struct Job {
    prey_cap: f64,
    scale: f64,
    seed: u64,
}

#[derive(Clone, Debug)]
struct RunStats {
    job: Job,
    sum_r_max: f64,
    crop_second_half: f64,
    min_agents: f64,
    mean_agents: f64,
    final_agents: f64,
    extinct_tick: Option<u64>,
    births: f64,
    deaths: f64,
    starvation: f64,
    dehydration: f64,
    lifespan_weighted: f64,
    births_last_quarter: f64,
    deaths_last_quarter: f64,
}

fn run(job: Job) -> RunStats {
    let initial = InitialConditionSet::new(
        [
            ("live.founders", FOUNDERS),
            ("live.predator_fraction", 0.0),
            ("live.trees", 8.0),
            ("live.lakes", LAKES),
            ("live.lake_layout", LAKE_LAYOUT as f64),
            ("live.mortality", 1.0),
            ("live.reproduction", 1.0),
            ("live.locomotion", 1.0),
            ("live.drive_floor", 0.5),
            ("live.thirst_steering", 1.0),
            ("live.prey_cap", job.prey_cap),
            ("live.resource_capacity_scale", job.scale),
            ("live.dehydration_ticks", GRACE),
            ("live.plant_reserve", RESERVE),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    );
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        job.seed,
        (16, 16),
        TICKS,
    )
    .expect("the declared world builds");
    let read = |a: &mut LiveExperimentAdapter| -> Vec<(String, f64)> {
        let mut w = a.world();
        live_observables(&mut w)
            .into_iter()
            .filter(|(k, _)| ALLOWED.contains(&k.as_str()))
            .collect()
    };
    let get = |obs: &[(String, f64)], id: &str| {
        obs.iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    };
    let sum_r_max = get(&read(&mut a), "live.standing_crop");

    let (mut crop_sum, mut crop_n) = (0.0, 0u64);
    let (mut min_agents, mut final_agents) = (f64::MAX, 0.0);
    let (mut agents_sum, mut agents_n) = (0.0, 0u64);
    let mut extinct_tick: Option<u64> = None;
    let (mut births, mut deaths, mut starvation, mut dehydration) = (0.0, 0.0, 0.0, 0.0);
    let (mut lifespan_weighted, mut births_lq, mut deaths_lq) = (0.0, 0.0, 0.0);
    for tick in 1..=TICKS {
        a.run_schedule_once();
        let obs = read(&mut a);
        let (b, d) = (get(&obs, "live.births"), get(&obs, "live.deaths"));
        let (s, h) = (
            get(&obs, "live.deaths_starvation"),
            get(&obs, "live.deaths_dehydration"),
        );
        assert!(
            (s + h - d).abs() < 1e-9,
            "lakes {} reserve {} prey_cap {} scale {} seed {} tick {tick}: starvation {s} + dehydration {h} != deaths {d}",
            LAKES,
            RESERVE,
            job.prey_cap,
            job.scale,
            job.seed
        );
        let agents_now = get(&obs, "live.agent_count");
        if extinct_tick.is_none() && agents_now == 0.0 {
            extinct_tick = Some(tick);
        }
        births += b;
        deaths += d;
        starvation += s;
        dehydration += h;
        lifespan_weighted += get(&obs, "live.mean_lifespan") * d;
        if tick > TICKS * 3 / 4 {
            births_lq += b;
            deaths_lq += d;
        }
        if tick % SAMPLE_EVERY == 0 {
            min_agents = min_agents.min(agents_now);
            final_agents = agents_now;
            agents_sum += agents_now;
            agents_n += 1;
            if tick > TICKS / 2 {
                crop_sum += get(&obs, "live.standing_crop");
                crop_n += 1;
            }
        }
    }
    RunStats {
        job,
        sum_r_max,
        crop_second_half: crop_sum / crop_n.max(1) as f64,
        min_agents,
        mean_agents: agents_sum / agents_n.max(1) as f64,
        final_agents,
        extinct_tick,
        births,
        deaths,
        starvation,
        dehydration,
        lifespan_weighted,
        births_last_quarter: births_lq,
        deaths_last_quarter: deaths_lq,
    }
}

fn median(xs: &mut [f64]) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let mid = xs.len() / 2;
    if xs.len() % 2 == 1 {
        xs[mid]
    } else {
        0.5 * (xs[mid - 1] + xs[mid])
    }
}

fn run_sweep(threads: usize) {
    let sweep_start = Instant::now();
    let mut jobs = Vec::new();
    for &seed in &SEEDS {
        for &ref_cap in &REF_PREY_CAPS {
            jobs.push(Job {
                prey_cap: ref_cap,
                scale: 1.0,
                seed,
            });
        }
        jobs.push(Job {
            prey_cap: SCARCITY_PREY_CAP,
            scale: SCARCITY_SCALE,
            seed,
        });
    }

    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let results = Arc::new(Mutex::new(Vec::with_capacity(total)));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (queue, results) = (Arc::clone(&queue), Arc::clone(&results));
            std::thread::spawn(move || loop {
                let job = queue.lock().unwrap().pop();
                let Some(job) = job else { break };
                let t0 = Instant::now();
                let r = run(job);
                let dt = t0.elapsed().as_secs_f64();
                let mut out = results.lock().unwrap();
                out.push(r);
                eprintln!(
                    "[S15] done {}/{} ({dt:.1}s; lakes={}, reserve={}, prey_cap={}, scale={}, seed={})",
                    out.len(),
                    total,
                    LAKES,
                    RESERVE,
                    job.prey_cap,
                    job.scale,
                    job.seed
                );
            })
        })
        .collect();
    for h in handles {
        h.join().expect("a worker thread panicked");
    }
    let wall_secs = sweep_start.elapsed().as_secs_f64();

    let mut all = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    all.sort_by(|a, b| {
        (a.job.prey_cap, a.job.scale, a.job.seed)
            .partial_cmp(&(b.job.prey_cap, b.job.scale, b.job.seed))
            .unwrap()
    });

    let find_ref = |ref_cap: f64, seed: u64| {
        all.iter()
            .find(|r| r.job.prey_cap == ref_cap && r.job.scale == 1.0 && r.job.seed == seed)
            .expect("reference run completed")
    };
    let find_scarcity = |seed: u64| {
        all.iter()
            .find(|r| r.job.prey_cap == SCARCITY_PREY_CAP && r.job.seed == seed)
            .expect("scarcity run completed")
    };

    // Pooled reference: mean crop_second_half across surviving prey_cap=80 baselines.
    let surviving_80: Vec<&RunStats> = SEEDS
        .iter()
        .map(|&s| find_ref(80.0, s))
        .filter(|r| r.extinct_tick.is_none())
        .collect();
    let pooled80_crop = if surviving_80.is_empty() {
        f64::NAN
    } else {
        surviving_80.iter().map(|r| r.crop_second_half).sum::<f64>() / surviving_80.len() as f64
    };

    let ratio = |scarcity: &RunStats, denom_crop: f64, denom_sum_r_max: f64| -> f64 {
        let floor_s = RESERVE * scarcity.sum_r_max;
        let floor_d = RESERVE * denom_sum_r_max;
        (scarcity.crop_second_half - floor_s) / (denom_crop - floor_d)
    };
    let g_80 = |r: &RunStats| {
        let b = find_ref(80.0, r.job.seed);
        ratio(r, b.crop_second_half, b.sum_r_max)
    };
    let g_120 = |r: &RunStats| {
        let b = find_ref(120.0, r.job.seed);
        ratio(r, b.crop_second_half, b.sum_r_max)
    };
    let g_160 = |r: &RunStats| {
        let b = find_ref(160.0, r.job.seed);
        ratio(r, b.crop_second_half, b.sum_r_max)
    };
    let g_pooled80 = |r: &RunStats| {
        let b0 = find_ref(80.0, SEEDS[0]);
        ratio(r, pooled80_crop, b0.sum_r_max)
    };
    let g_ungrazed = |r: &RunStats| {
        let b0 = find_ref(80.0, r.job.seed);
        ratio(r, b0.sum_r_max, b0.sum_r_max)
    };

    let lifespan = |r: &RunStats| {
        if r.deaths > 0.0 {
            r.lifespan_weighted / r.deaths
        } else {
            f64::NAN
        }
    };
    let generations = |r: &RunStats| {
        let by_lifespan = TICKS as f64 / lifespan(r);
        let by_turnover = r.births / r.mean_agents.max(1.0);
        (by_lifespan, by_turnover, by_lifespan.min(by_turnover))
    };

    println!(
        "STAGE_WALL stage=S15 lakes={LAKES} scale={SCARCITY_SCALE} reserve={RESERVE} runs={total} threads={threads} wall_secs={wall_secs:.2} wall_mins={:.2}",
        wall_secs / 60.0
    );
    println!(
        "POOL_REF80 surviving={}/{} mean_crop={pooled80_crop:.1}",
        surviving_80.len(),
        SEEDS.len()
    );
    println!();

    // Exact reproduction comparison against S14 on the 6 scale=0.63 scarcity runs.
    println!("S14_PARITY seed\ts14_g80\ts15_g80\ts14_min\ts15_min\ts14_mean\ts15_mean\ts14_final\ts15_final\ts14_births\ts15_births\ts14_deaths\ts15_deaths\ts14_starv\ts15_starv\ts14_dehyd\ts15_dehyd\ts14_lq_b/d\ts15_lq_b/d\tmatch");
    let mut all_parity = true;
    for rec in &S14_EXPECTED {
        let r = find_scarcity(rec.seed);
        let s15_g_str = format!("{:.3}", g_80(r));
        let s15_mean_str = format!("{:.1}", r.mean_agents);
        let eq = s15_g_str == rec.g_str
            && r.min_agents as u64 == rec.min_agents
            && s15_mean_str == rec.mean_str
            && r.final_agents as u64 == rec.final_agents
            && r.births as u64 == rec.births
            && r.deaths as u64 == rec.deaths
            && r.starvation as u64 == rec.starvation
            && r.dehydration as u64 == rec.dehydration
            && r.births_last_quarter as u64 == rec.births_last_q
            && r.deaths_last_quarter as u64 == rec.deaths_last_q;
        if !eq {
            all_parity = false;
        }
        println!(
            "S14_PARITY {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}/{}\t{}/{}\t{}",
            rec.seed,
            rec.g_str,
            s15_g_str,
            rec.min_agents,
            r.min_agents,
            rec.mean_str,
            s15_mean_str,
            rec.final_agents,
            r.final_agents,
            rec.births,
            r.births,
            rec.deaths,
            r.deaths,
            rec.starvation,
            r.starvation,
            rec.dehydration,
            r.dehydration,
            rec.births_last_q,
            rec.deaths_last_q,
            r.births_last_quarter,
            r.deaths_last_quarter,
            if eq { "yes" } else { "NO" }
        );
    }
    println!(
        "S14_PARITY_OVERALL all_6_seeds_exact_match={}",
        if all_parity { "yes" } else { "NO" }
    );
    println!();

    // Per-seed table: survival, crops, and every candidate g.
    println!("PER_SEED seed\tb80_ext_tick\tb80_crop\tb120_ext_tick\tb120_crop\tb160_ext_tick\tb160_crop\tpooled80_crop\tungrazed_crop\tscarcity_crop\tg_80\tg_120\tg_160\tg_pooled80\tg_ungrazed");
    for &seed in &SEEDS {
        let r80 = find_ref(80.0, seed);
        let r120 = find_ref(120.0, seed);
        let r160 = find_ref(160.0, seed);
        let rs = find_scarcity(seed);
        let tick_s = |r: &RunStats| {
            r.extinct_tick
                .map_or_else(|| "none".to_string(), |t| t.to_string())
        };
        println!(
            "PER_SEED {seed}\t{}\t{:.1}\t{}\t{:.1}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",
            tick_s(r80),
            r80.crop_second_half,
            tick_s(r120),
            r120.crop_second_half,
            tick_s(r160),
            r160.crop_second_half,
            pooled80_crop,
            r80.sum_r_max,
            rs.crop_second_half,
            g_80(rs),
            g_120(rs),
            g_160(rs),
            g_pooled80(rs),
            g_ungrazed(rs),
        );
    }
    println!();

    // Full 24-run TSV.
    println!("layout\tscale\tgrace\tfounders\tlakes\tprey_cap\treserve\tseed\tsum_r_max\textinct_tick\tcrop_second_half\tg_80\tg_120\tg_160\tg_pooled80\tg_ungrazed\tmin_agents\tmean_agents\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tstarvation_share\tdehydration_share\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover\tgenerations\tbirths_last_q\tdeaths_last_q");
    for r in &all {
        let (gl, gt, g_min) = generations(r);
        let ext_s = r
            .extinct_tick
            .map_or_else(|| "none".to_string(), |t| t.to_string());
        let is_scarcity = r.job.prey_cap == SCARCITY_PREY_CAP;
        let (v80, v120, v160, vp) = if is_scarcity {
            (g_80(r), g_120(r), g_160(r), g_pooled80(r))
        } else {
            (f64::NAN, f64::NAN, f64::NAN, f64::NAN)
        };
        let vu = g_ungrazed(r);
        let (st_share, de_share) = if r.deaths > 0.0 {
            (r.starvation / r.deaths, r.dehydration / r.deaths)
        } else {
            (f64::NAN, f64::NAN)
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{:.1}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}",
            LAKE_LAYOUT,
            r.job.scale,
            GRACE,
            FOUNDERS,
            LAKES,
            r.job.prey_cap,
            RESERVE,
            r.job.seed,
            r.sum_r_max,
            ext_s,
            r.crop_second_half,
            v80,
            v120,
            v160,
            vp,
            vu,
            r.min_agents,
            r.mean_agents,
            r.final_agents,
            r.births,
            r.deaths,
            r.starvation,
            r.dehydration,
            st_share,
            de_share,
            lifespan(r),
            gl,
            gt,
            g_min,
            r.births_last_quarter,
            r.deaths_last_quarter
        );
    }
    println!();

    // Declared SUMMARY (unchanged g_80).
    let rows: Vec<&RunStats> = SEEDS.iter().map(|&s| find_scarcity(s)).collect();
    let extinctions = rows.iter().filter(|r| r.final_agents == 0.0).count();
    let min_pop = rows.iter().map(|r| r.min_agents).fold(f64::MAX, f64::min);
    let mut means: Vec<f64> = rows.iter().map(|r| r.mean_agents).collect();
    let mean_med = median(&mut means);
    let mut gs: Vec<f64> = rows.iter().map(|r| g_80(r)).collect();
    let g_med = median(&mut gs);
    let (glo, ghi) = (
        gs.iter().cloned().fold(f64::MAX, f64::min),
        gs.iter().cloned().fold(f64::MIN, f64::max),
    );
    let mut gens: Vec<f64> = rows.iter().map(|r| generations(r).2).collect();
    let gen_med = median(&mut gens);
    let gen_min = gens.iter().cloned().fold(f64::MAX, f64::min);
    let (st, de): (f64, f64) = rows.iter().fold((0.0, 0.0), |(s, d), r| {
        (s + r.starvation, d + r.dehydration)
    });
    let (share, de_share) = (st / (st + de), de / (st + de));
    let shares: Vec<f64> = rows.iter().map(|r| r.starvation / r.deaths).collect();
    let (slo, shi) = (
        shares.iter().cloned().fold(f64::MAX, f64::min),
        shares.iter().cloned().fold(f64::MIN, f64::max),
    );
    let active = rows
        .iter()
        .filter(|r| r.births_last_quarter > 0.0 && r.deaths_last_quarter > 0.0)
        .count();

    let c1 = min_pop >= 5.0;
    let c2 = glo >= 0.4 && ghi <= 0.6;
    let c3 = gen_min >= 30.0;
    let c4 = share > 0.5;
    let c5 = active == rows.len();
    let ok = c1 && c2 && c3 && c4 && c5;
    let mut failed = Vec::new();
    if !c1 {
        failed.push("C1(min_agents<5)");
    }
    if !c2 {
        failed.push("C2(g_out_of_[0.4,0.6])");
    }
    if !c3 {
        failed.push("C3(gen_min<30)");
    }
    if !c4 {
        failed.push("C4(starvation_share<=0.5)");
    }
    if !c5 {
        failed.push("C5(seeds_active<6/6)");
    }
    let failed_str = if failed.is_empty() {
        "none".to_string()
    } else {
        failed.join(",")
    };
    println!("SUMMARY stage\tlakes\tscale\treserve\tprey_cap\textinctions\tc1_min_agents\tmean_agents_med\tc2_g80_med\tc2_g80_min..max\tc3_generations_med\tc3_generations_min\tc4_share_starvation\tc4_share_dehydration\tc4_share_min..max\tc5_seeds_active\tVIABLE\tfailed_conditions");
    println!(
        "SUMMARY S15\t{LAKES}\t{SCARCITY_SCALE}\t{RESERVE}\t{SCARCITY_PREY_CAP}\t{extinctions}/{}\t{min_pop}\t{mean_med:.1}\t{g_med:.3}\t{glo:.3}..{ghi:.3}\t{gen_med:.1}\t{gen_min:.1}\t{share:.3}\t{de_share:.3}\t{slo:.3}..{shi:.3}\t{active}/{}\t{}\t{failed_str}",
        rows.len(),
        rows.len(),
        if ok { "yes" } else { "no" }
    );
    println!();

    // Alternative reference C2 diagnostics.
    println!("SUMMARY_REF candidate\tref_extinct\tref_crop_min..max\tg_med\tg_min..max\tc2_in_band_seeds\tc2_verdict\trule_status");
    let report_ref = |label: &str,
                      rule_status: &str,
                      ref_ext: usize,
                      ref_crops: Vec<f64>,
                      vals: Vec<f64>| {
        let mut s = vals.clone();
        let med = median(&mut s);
        let lo = vals.iter().cloned().fold(f64::MAX, f64::min);
        let hi = vals.iter().cloned().fold(f64::MIN, f64::max);
        let pass_n = vals.iter().filter(|&&v| (0.4..=0.6).contains(&v)).count();
        let verdict = if lo >= 0.4 && hi <= 0.6 { "yes" } else { "no" };
        let clo = ref_crops.iter().cloned().fold(f64::MAX, f64::min);
        let chi = ref_crops.iter().cloned().fold(f64::MIN, f64::max);
        println!(
            "SUMMARY_REF {label}\t{ref_ext}/{}\t{clo:.1}..{chi:.1}\t{med:.3}\t{lo:.3}..{hi:.3}\t{pass_n}/{}\t{verdict}\t{rule_status}",
            SEEDS.len(),
            SEEDS.len()
        );
    };
    let collect_ref_stats = |cap: f64| -> (usize, Vec<f64>) {
        let rs: Vec<&RunStats> = SEEDS.iter().map(|&s| find_ref(cap, s)).collect();
        let ext = rs.iter().filter(|r| r.extinct_tick.is_some()).count();
        let crops = rs.iter().map(|r| r.crop_second_half).collect();
        (ext, crops)
    };
    let (e80, c80) = collect_ref_stats(80.0);
    let (e120, c120) = collect_ref_stats(120.0);
    let (e160, c160) = collect_ref_stats(160.0);
    report_ref(
        "g_80",
        "declared rule",
        e80,
        c80,
        rows.iter().map(|r| g_80(r)).collect(),
    );
    report_ref(
        "g_120",
        "diagnostic, not the declared rule",
        e120,
        c120,
        rows.iter().map(|r| g_120(r)).collect(),
    );
    report_ref(
        "g_160",
        "diagnostic, not the declared rule",
        e160,
        c160,
        rows.iter().map(|r| g_160(r)).collect(),
    );
    report_ref(
        "g_pooled80",
        "diagnostic, not the declared rule",
        e80,
        vec![pooled80_crop],
        rows.iter().map(|r| g_pooled80(r)).collect(),
    );
    report_ref(
        "g_ungrazed",
        "diagnostic, not the declared rule",
        0,
        SEEDS.iter().map(|&s| find_ref(80.0, s).sum_r_max).collect(),
        rows.iter().map(|r| g_ungrazed(r)).collect(),
    );
}

fn main() {
    let threads = std::env::var("ANIMA_SEL2_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|n| n.clamp(1, 4))
        .unwrap_or(DEFAULT_THREADS);
    let mode = std::env::args().nth(1);

    match mode.as_deref() {
        Some("--probe") | Some("probe") => {
            let job = Job {
                prey_cap: 120.0,
                scale: 1.0,
                seed: 1,
            };
            let t0 = Instant::now();
            let r = run(job);
            let secs = t0.elapsed().as_secs_f64();
            println!(
                "PROBE_200K ticks={TICKS} lakes={LAKES} reserve={RESERVE} scale={} prey_cap={} seed={} wall_secs={secs:.2} extinct_tick={:?} crop_second_half={:.1} births={} deaths={} final_agents={}",
                job.scale,
                job.prey_cap,
                job.seed,
                r.extinct_tick,
                r.crop_second_half,
                r.births,
                r.deaths,
                r.final_agents
            );
        }
        Some(other) => {
            eprintln!("unknown argument {other:?}; expected --probe or none");
            std::process::exit(2);
        }
        None => {
            run_sweep(threads);
        }
    }
}
