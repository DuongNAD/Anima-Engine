//! Card S12 exploratory measurement: SEL2-E best world with `live.lake_layout` ∈ {0, 1}.
//!
//! **Blind to body mass**, with the same allow-list as `sel2e_long_runs.rs`; seeds 1–6 are
//! exploratory, as in S6, S9, SEL2-E, and S11.
//!
//! World: 100 founders, `predator_fraction` 0.0, 8 trees, 8 lakes, `mortality` 1.0,
//! `reproduction` 1.0, `locomotion` 1.0, `drive_floor` 0.5, `thirst_steering` 1.0,
//! `dehydration_ticks` 960, `plant_reserve` 0.0, `prey_cap` 320,
//! `lake_layout` ∈ {0, 1} × `resource_capacity_scale` ∈ {1.0, 0.7, 0.5} × seeds 1–6 (36 runs),
//! plus baseline runs (`prey_cap` 80, `resource_capacity_scale` 1.0) per `(lake_layout, seed)` (12
//! runs) so each arm computes `g` against its matching layout baseline; 100,000 ticks each.
//!
//! ```text
//! cargo run --release --example sel2_lake_layout > sel2_lake_layout.tsv
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use std::sync::{Arc, Mutex};

const TICKS: u64 = 100_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const LAYOUTS: [u8; 2] = [0, 1];
const FOUNDERS: f64 = 100.0;
const LAKES: f64 = 8.0;
const SCALES: [f64; 3] = [1.0, 0.7, 0.5];
const GRACE: f64 = 960.0;
const RESERVE: f64 = 0.0;
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
    lake_layout: u8,
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
            ("live.lake_layout", job.lake_layout as f64),
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
            "layout {} seed {} tick {tick}: starvation {s} + dehydration {h} != deaths {d}",
            job.lake_layout,
            job.seed
        );
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
            let agents = get(&obs, "live.agent_count");
            min_agents = min_agents.min(agents);
            final_agents = agents;
            agents_sum += agents;
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

fn main() {
    let threads = std::env::var("ANIMA_SEL2_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|n| n.clamp(1, 4))
        .unwrap_or(DEFAULT_THREADS);

    let mut jobs = Vec::new();
    for &layout in &LAYOUTS {
        for &seed in &SEEDS {
            jobs.push(Job {
                lake_layout: layout,
                prey_cap: BASE_PREY_CAP,
                scale: 1.0,
                seed,
            });
            for &scale in &SCALES {
                jobs.push(Job {
                    lake_layout: layout,
                    prey_cap: PREY_CAP,
                    scale,
                    seed,
                });
            }
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
                let r = run(job);
                let mut out = results.lock().unwrap();
                out.push(r);
                eprintln!(
                    "done {}/{} (layout={}, prey_cap={}, scale={}, seed={})",
                    out.len(),
                    total,
                    job.lake_layout,
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

    let mut all = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    all.sort_by(|a, b| {
        (a.job.lake_layout, a.job.prey_cap, a.job.scale, a.job.seed)
            .partial_cmp(&(b.job.lake_layout, b.job.prey_cap, b.job.scale, b.job.seed))
            .unwrap()
    });

    let base = |layout: u8, seed: u64| {
        all.iter()
            .find(|r| {
                r.job.lake_layout == layout && r.job.prey_cap == BASE_PREY_CAP && r.job.seed == seed
            })
            .expect("baseline ran")
    };
    let g = |r: &RunStats| {
        let floor = RESERVE * r.sum_r_max;
        let b = base(r.job.lake_layout, r.job.seed);
        (r.crop_second_half - floor) / (b.crop_second_half - floor)
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

    println!("layout\tscale\tgrace\tfounders\tlakes\tprey_cap\treserve\tseed\tsum_r_max\tg\tmin_agents\tmean_agents\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tstarvation_share\tdehydration_share\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover\tgenerations\tbirths_last_q\tdeaths_last_q");
    for r in &all {
        let (gl, gt, g_min) = generations(r);
        let gv = if r.job.prey_cap == PREY_CAP {
            g(r)
        } else {
            f64::NAN
        };
        let (st_share, de_share) = if r.deaths > 0.0 {
            (r.starvation / r.deaths, r.dehydration / r.deaths)
        } else {
            (f64::NAN, f64::NAN)
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.3}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}",
            r.job.lake_layout,
            r.job.scale,
            GRACE,
            FOUNDERS,
            LAKES,
            r.job.prey_cap,
            RESERVE,
            r.job.seed,
            r.sum_r_max,
            gv,
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
    println!("SUMMARY layout\tscale\tprey_cap\textinctions\tc1_min_agents\tmean_agents_med\tc2_g_med\tc2_g_min..max\tc3_generations_med\tc3_generations_min\tc4_share_starvation\tc4_share_dehydration\tc4_share_min..max\tc5_seeds_active\tVIABLE");
    for &layout in &LAYOUTS {
        for &scale in &SCALES {
            let rows: Vec<&RunStats> = all
                .iter()
                .filter(|r| {
                    r.job.lake_layout == layout
                        && r.job.prey_cap == PREY_CAP
                        && r.job.scale == scale
                })
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
            let ok = min_pop >= 5.0
                && glo >= 0.4
                && ghi <= 0.6
                && gen_min >= 30.0
                && share > 0.5
                && active == rows.len();
            println!(
                "SUMMARY {layout}\t{scale}\t{PREY_CAP}\t{extinctions}/{}\t{min_pop}\t{mean_med:.1}\t{g_med:.3}\t{glo:.3}..{ghi:.3}\t{gen_med:.1}\t{gen_min:.1}\t{share:.3}\t{de_share:.3}\t{slo:.3}..{shi:.3}\t{active}/{}\t{}",
                rows.len(),
                rows.len(),
                if ok { "yes" } else { "no" }
            );
        }
    }
}
