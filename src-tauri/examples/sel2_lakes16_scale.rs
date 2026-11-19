//! Card S14 exploratory measurement: narrow SEL2 exploration at 16 spread lakes
//! (`live.lake_layout = 1`, `live.plant_reserve = 0.0`, `live.resource_capacity_scale` ∈
//! `{0.63, 0.65, 0.67}`).
//!
//! **Blind to body mass**, with the same allow-list as `sel2_spread_lakes.rs` and
//! `sel2_lake_layout.rs`; seeds 1–6 are exploratory, not confirmation seeds.
//!
//! Fixed world (matching S13 Stage A): 100 founders, `predator_fraction` 0.0, 8 trees,
//! `lakes` 16.0, `lake_layout` 1.0, `reserve` 0.0, `mortality` 1.0, `reproduction` 1.0,
//! `locomotion` 1.0, `drive_floor` 0.5, `thirst_steering` 1.0, `dehydration_ticks` 960,
//! `prey_cap` 320, seeds 1–6, 200,000 ticks each, sampled every 50 ticks:
//! - Factor grid: `resource_capacity_scale` ∈ `{0.63, 0.65, 0.67}` × seeds 1–6 (18 runs).
//! - Baseline per seed: `prey_cap = 80, scale = 1.0` × seeds 1–6 (6 runs, run once and shared).
//!
//! Diagnostic additions (reported only, NOT used by `VIABLE`):
//! - `baseline_extinct` (`1` if the seed's baseline reached `0` agents, else `0`) and
//!   `baseline_extinct_tick`;
//! - `g_ungrazed = (crop_second_half - floor) / (sum_r_max_at_scale_1.0 - floor)`;
//! - in `SUMMARY`, a second C2 column computed with `g_ungrazed`, labelled
//!   `"diagnostic, not the declared rule"`.
//!
//! ```text
//! cargo run --release --example sel2_lakes16_scale -- --probe
//! cargo run --release --example sel2_lakes16_scale > s14_out.tsv
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
const SCALES: [f64; 3] = [0.63, 0.65, 0.67];
const GRACE: f64 = 960.0;
const PREY_CAP: f64 = 320.0;
const BASE_PREY_CAP: f64 = 80.0;
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
            "lakes {} reserve {} seed {} tick {tick}: starvation {s} + dehydration {h} != deaths {d}",
            LAKES,
            RESERVE,
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

fn run_sweep(threads: usize) -> usize {
    let sweep_start = Instant::now();
    let mut jobs = Vec::new();
    for &seed in &SEEDS {
        jobs.push(Job {
            prey_cap: BASE_PREY_CAP,
            scale: 1.0,
            seed,
        });
        for &scale in &SCALES {
            jobs.push(Job {
                prey_cap: PREY_CAP,
                scale,
                seed,
            });
        }
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
                    "[S14] done {}/{} ({dt:.1}s; lakes={}, reserve={}, prey_cap={}, scale={}, seed={})",
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

    let base = |seed: u64| {
        all.iter()
            .find(|r| r.job.prey_cap == BASE_PREY_CAP && r.job.seed == seed)
            .expect("baseline ran")
    };
    let g = |r: &RunStats| {
        let floor = RESERVE * r.sum_r_max;
        let b = base(r.job.seed);
        (r.crop_second_half - floor) / (b.crop_second_half - floor)
    };
    let g_ungrazed = |r: &RunStats| {
        let floor = RESERVE * r.sum_r_max;
        let b = base(r.job.seed);
        (r.crop_second_half - floor) / (b.sum_r_max - floor)
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
        "STAGE_WALL stage=S14 lakes={LAKES} reserve={RESERVE} runs={total} threads={threads} wall_secs={wall_secs:.2} wall_mins={:.2}",
        wall_secs / 60.0
    );
    println!("layout\tscale\tgrace\tfounders\tlakes\tprey_cap\treserve\tseed\tsum_r_max\tg\tbaseline_extinct\tbaseline_extinct_tick\tg_ungrazed\tmin_agents\tmean_agents\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tstarvation_share\tdehydration_share\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover\tgenerations\tbirths_last_q\tdeaths_last_q");
    for r in &all {
        let (gl, gt, g_min) = generations(r);
        let gv = if r.job.prey_cap == PREY_CAP {
            g(r)
        } else {
            f64::NAN
        };
        let b = base(r.job.seed);
        let base_extinct = u8::from(b.extinct_tick.is_some());
        let base_extinct_tick = b
            .extinct_tick
            .map_or_else(|| "none".to_string(), |t| t.to_string());
        let gu = g_ungrazed(r);
        let (st_share, de_share) = if r.deaths > 0.0 {
            (r.starvation / r.deaths, r.dehydration / r.deaths)
        } else {
            (f64::NAN, f64::NAN)
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.3}\t{}\t{}\t{:.3}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}",
            LAKE_LAYOUT,
            r.job.scale,
            GRACE,
            FOUNDERS,
            LAKES,
            r.job.prey_cap,
            RESERVE,
            r.job.seed,
            r.sum_r_max,
            gv,
            base_extinct,
            base_extinct_tick,
            gu,
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
    println!("SUMMARY stage\tlakes\tscale\treserve\tprey_cap\textinctions\tc1_min_agents\tmean_agents_med\tc2_g_med\tc2_g_min..max\tc3_generations_med\tc3_generations_min\tc4_share_starvation\tc4_share_dehydration\tc4_share_min..max\tc5_seeds_active\tVIABLE\tfailed_conditions\tc2_g_ungrazed_med\tc2_g_ungrazed_min..max\tc2_ungrazed (diagnostic, not the declared rule)");
    let mut viable_count = 0usize;
    for &scale in &SCALES {
        let rows: Vec<&RunStats> = all
            .iter()
            .filter(|r| r.job.prey_cap == PREY_CAP && r.job.scale == scale)
            .collect();
        let extinctions = rows.iter().filter(|r| r.final_agents == 0.0).count();
        let min_pop = rows.iter().map(|r| r.min_agents).fold(f64::MAX, f64::min);
        let mut means: Vec<f64> = rows.iter().map(|r| r.mean_agents).collect();
        let mean_med = median(&mut means);
        let mut gs: Vec<f64> = rows.iter().map(|r| g(r)).collect();
        let g_med = median(&mut gs);
        let (glo, ghi) = (
            gs.iter().cloned().fold(f64::MAX, f64::min),
            gs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let mut gus: Vec<f64> = rows.iter().map(|r| g_ungrazed(r)).collect();
        let gu_med = median(&mut gus);
        let (gulo, guhi) = (
            gus.iter().cloned().fold(f64::MAX, f64::min),
            gus.iter().cloned().fold(f64::MIN, f64::max),
        );
        let c2_ungrazed = gulo >= 0.4 && guhi <= 0.6;
        let mut gens: Vec<f64> = rows
            .iter()
            .map(|r| {
                let v = generations(r).2;
                if v.is_nan() {
                    0.0
                } else {
                    v
                }
            })
            .collect();
        let gen_med = median(&mut gens);
        let gen_min = gens.iter().cloned().fold(f64::MAX, f64::min);
        let (st, de): (f64, f64) = rows.iter().fold((0.0, 0.0), |(s, d), r| {
            (s + r.starvation, d + r.dehydration)
        });
        let (share, de_share) = if st + de > 0.0 {
            (st / (st + de), de / (st + de))
        } else {
            (f64::NAN, f64::NAN)
        };
        let shares: Vec<f64> = rows
            .iter()
            .map(|r| {
                if r.deaths > 0.0 {
                    r.starvation / r.deaths
                } else {
                    f64::NAN
                }
            })
            .collect();
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
        if ok {
            viable_count += 1;
        }
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
        println!(
            "SUMMARY S14\t{LAKES}\t{scale}\t{RESERVE}\t{PREY_CAP}\t{extinctions}/{}\t{min_pop}\t{mean_med:.1}\t{g_med:.3}\t{glo:.3}..{ghi:.3}\t{gen_med:.1}\t{gen_min:.1}\t{share:.3}\t{de_share:.3}\t{slo:.3}..{shi:.3}\t{active}/{}\t{}\t{failed_str}\t{gu_med:.3}\t{gulo:.3}..{guhi:.3}\t{}",
            rows.len(),
            rows.len(),
            if ok { "yes" } else { "no" },
            if c2_ungrazed { "yes" } else { "no" }
        );
    }
    println!();
    viable_count
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
                prey_cap: PREY_CAP,
                scale: 0.65,
                seed: 1,
            };
            let t0 = Instant::now();
            let r = run(job);
            let secs = t0.elapsed().as_secs_f64();
            let proj_parallel_hours = secs * (24.0 / threads as f64) / 3600.0;
            let proj_serial_hours = secs * 24.0 / 3600.0;
            println!(
                "PROBE_200K ticks={TICKS} lakes={LAKES} reserve={RESERVE} scale={} prey_cap={} seed={} wall_secs={secs:.2} threads={threads} proj_sweep_parallel_hours={proj_parallel_hours:.4} proj_sweep_serial_hours={proj_serial_hours:.4} births={} deaths={} starvation={} dehydration={} final_agents={}",
                job.scale,
                job.prey_cap,
                job.seed,
                r.births,
                r.deaths,
                r.starvation,
                r.dehydration,
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
