//! Card SEL2-E follow-up: long runs on the cells the thirst probe made promising.
//!
//! **Blind to body mass**, with the same allow-list as `sel2e_exploration.rs`; seeds 1–6 are
//! exploratory, as in SEL2-E.
//!
//! SEL2-E (18,000 ticks) found about 6 generations per run and thirst killing most animals; the
//! thirst probe (`sel2e_thirst_probe.rs`) showed a longer `live.dehydration_ticks` moving deaths to
//! starvation. Grid: 100 founders, 8 lakes, `prey_cap` 320, `resource_capacity_scale` ∈ {0.7, 0.5}
//! × `live.dehydration_ticks` ∈ {480, 960} × `live.plant_reserve` ∈ {0, 0.2} × seeds 1–6, 100,000
//! ticks each, plus one baseline per (grace, seed) at `prey_cap` 80, scale 1.0, no reserve, for `g`.
//! The five viability conditions are the same as in `sel2e_exploration.rs`.
//!
//! ```text
//! cargo run --release --example sel2e_long_runs > sel2e_long_runs.tsv
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use std::sync::{Arc, Mutex};

const TICKS: u64 = 100_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const FOUNDERS: [f64; 1] = [100.0];
const RESERVES: [f64; 2] = [0.0, 0.2];
const LAKES: [f64; 1] = [8.0];
const SCALES: [f64; 2] = [0.7, 0.5];
const GRACES: [f64; 2] = [480.0, 960.0];
const PREY_CAP: f64 = 320.0;
const BASE_PREY_CAP: f64 = 80.0;
const THREADS: usize = 8;
/// The only observables this driver reads. Nothing about body mass.
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
    founders: f64,
    reserve: f64,
    lakes: f64,
    prey_cap: f64,
    scale: f64,
    grace: f64,
    seed: u64,
}

#[derive(Clone, Debug)]
struct RunStats {
    job: Job,
    sum_r_max: f64,
    crop_second_half: f64,
    min_agents: f64,
    mean_agents: f64,
    mean_agents_second_half: f64,
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
            ("live.founders", job.founders),
            ("live.predator_fraction", 0.0),
            ("live.trees", 8.0),
            ("live.lakes", job.lakes),
            ("live.mortality", 1.0),
            ("live.reproduction", 1.0),
            ("live.locomotion", 1.0),
            ("live.drive_floor", 0.5),
            ("live.thirst_steering", 1.0),
            ("live.prey_cap", job.prey_cap),
            ("live.resource_capacity_scale", job.scale),
            ("live.dehydration_ticks", job.grace),
            ("live.plant_reserve", job.reserve),
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
    // Every cell starts full and r_max only changes at genesis, so the crop before tick 1 is Σr_max.
    let sum_r_max = get(&read(&mut a), "live.standing_crop");

    let (mut crop_sum, mut crop_n) = (0.0, 0u64);
    let (mut min_agents, mut final_agents) = (f64::MAX, 0.0);
    let (mut agents_sum, mut agents_n, mut agents2_sum, mut agents2_n) = (0.0, 0u64, 0.0, 0u64);
    let (mut births, mut deaths, mut starvation, mut dehydration) = (0.0, 0.0, 0.0, 0.0);
    let (mut lifespan_weighted, mut births_lq, mut deaths_lq) = (0.0, 0.0, 0.0);
    for tick in 1..=TICKS {
        a.run_schedule_once();
        // Every tick is read, so the windows add up to the run's totals.
        let obs = read(&mut a);
        let (b, d) = (get(&obs, "live.births"), get(&obs, "live.deaths"));
        let (s, h) = (
            get(&obs, "live.deaths_starvation"),
            get(&obs, "live.deaths_dehydration"),
        );
        assert!(
            (s + h - d).abs() < 1e-9,
            "seed {} tick {tick}: starvation {s} + dehydration {h} != deaths {d}",
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
                agents2_sum += agents;
                agents2_n += 1;
            }
        }
    }
    RunStats {
        job,
        sum_r_max,
        crop_second_half: crop_sum / crop_n.max(1) as f64,
        min_agents,
        mean_agents: agents_sum / agents_n.max(1) as f64,
        mean_agents_second_half: agents2_sum / agents2_n.max(1) as f64,
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

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
    }
    if sxx == 0.0 || syy == 0.0 {
        f64::NAN
    } else {
        sxy / (sxx * syy).sqrt()
    }
}

fn main() {
    let mut jobs = Vec::new();
    for &seed in &SEEDS {
        for &grace in &GRACES {
            for &founders in &FOUNDERS {
                for &lakes in &LAKES {
                    jobs.push(Job {
                        founders,
                        reserve: 0.0,
                        lakes,
                        prey_cap: BASE_PREY_CAP,
                        scale: 1.0,
                        grace,
                        seed,
                    });
                    for &scale in &SCALES {
                        for &reserve in &RESERVES {
                            jobs.push(Job {
                                founders,
                                reserve,
                                lakes,
                                prey_cap: PREY_CAP,
                                scale,
                                grace,
                                seed,
                            });
                        }
                    }
                }
            }
        }
    }
    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let results = Arc::new(Mutex::new(Vec::with_capacity(total)));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let (queue, results) = (Arc::clone(&queue), Arc::clone(&results));
            std::thread::spawn(move || loop {
                let job = queue.lock().unwrap().pop();
                let Some(job) = job else { break };
                let r = run(job);
                let mut out = results.lock().unwrap();
                out.push(r);
                eprintln!("done {}/{}", out.len(), total);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("a worker thread panicked");
    }
    let mut all = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    all.sort_by(|a, b| {
        (
            a.job.prey_cap,
            a.job.scale,
            a.job.grace,
            a.job.reserve,
            a.job.seed,
        )
            .partial_cmp(&(
                b.job.prey_cap,
                b.job.scale,
                b.job.grace,
                b.job.reserve,
                b.job.seed,
            ))
            .unwrap()
    });

    let base = |founders: f64, lakes: f64, grace: f64, seed: u64| {
        all.iter()
            .find(|r| {
                r.job.prey_cap == BASE_PREY_CAP
                    && r.job.founders == founders
                    && r.job.lakes == lakes
                    && r.job.grace == grace
                    && r.job.seed == seed
            })
            .expect("the baseline ran")
    };
    let g = |r: &RunStats| {
        let floor = r.job.reserve * r.sum_r_max;
        let b = base(r.job.founders, r.job.lakes, r.job.grace, r.job.seed);
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

    println!("scale\tgrace\tfounders\tlakes\tprey_cap\treserve\tseed\tsum_r_max\tg\tmin_agents\tmean_agents\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover\tbirths_last_q\tdeaths_last_q");
    for r in &all {
        let (gl, gt, _) = generations(r);
        let gv = if r.job.prey_cap == PREY_CAP {
            g(r)
        } else {
            f64::NAN
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.3}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}",
            r.job.scale,
            r.job.grace,
            r.job.founders,
            r.job.lakes,
            r.job.prey_cap,
            r.job.reserve,
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
            lifespan(r),
            gl,
            gt,
            r.births_last_quarter,
            r.deaths_last_quarter
        );
    }

    println!();
    println!("SUMMARY scale\tgrace\treserve\tc1_min_agents\tc2_g_min..max\tc3_generations_min\tc4_share_starvation\tc4_share_min..max_over_seeds\tc5_seeds_active\tpair_r_mean_agents_vs_reserve0\tVIABLE");
    let mut viable = Vec::new();
    let (founders, lakes) = (FOUNDERS[0], LAKES[0]);
    for &scale in &SCALES {
        for &grace in &GRACES {
            for &reserve in &RESERVES {
                let rows: Vec<&RunStats> = all
                    .iter()
                    .filter(|r| {
                        r.job.prey_cap == PREY_CAP
                            && r.job.founders == founders
                            && r.job.lakes == lakes
                            && r.job.scale == scale
                            && r.job.grace == grace
                            && r.job.reserve == reserve
                    })
                    .collect();
                let min_pop = rows.iter().map(|r| r.min_agents).fold(f64::MAX, f64::min);
                let gs: Vec<f64> = rows.iter().map(|r| g(r)).collect();
                let (glo, ghi) = (
                    gs.iter().cloned().fold(f64::MAX, f64::min),
                    gs.iter().cloned().fold(f64::MIN, f64::max),
                );
                let gen_min = rows
                    .iter()
                    .map(|r| generations(r).2)
                    .fold(f64::MAX, |a, b| if b.is_nan() { 0.0 } else { a.min(b) });
                let (st, de): (f64, f64) = rows.iter().fold((0.0, 0.0), |(s, d), r| {
                    (s + r.starvation, d + r.dehydration)
                });
                let share = if st + de > 0.0 {
                    st / (st + de)
                } else {
                    f64::NAN
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
                let pair_r = if reserve > 0.0 {
                    let x: Vec<f64> = rows.iter().map(|r| r.mean_agents_second_half).collect();
                    let y: Vec<f64> = SEEDS
                        .iter()
                        .map(|&s| {
                            all.iter()
                                .find(|r| {
                                    r.job.prey_cap == PREY_CAP
                                        && r.job.founders == founders
                                        && r.job.lakes == lakes
                                        && r.job.scale == scale
                                        && r.job.grace == grace
                                        && r.job.reserve == 0.0
                                        && r.job.seed == s
                                })
                                .unwrap()
                                .mean_agents_second_half
                        })
                        .collect();
                    pearson(&x, &y)
                } else {
                    f64::NAN
                };
                let ok = min_pop >= 5.0
                    && glo >= 0.4
                    && ghi <= 0.6
                    && gen_min >= 30.0
                    && share > 0.5
                    && active == rows.len();
                if ok {
                    viable.push((scale, grace, reserve));
                }
                println!(
                    "SUMMARY {scale}\t{grace}\t{reserve}\t{min_pop}\t{glo:.3}..{ghi:.3}\t{gen_min:.1}\t{share:.3}\t{slo:.3}..{shi:.3}\t{active}/{}\t{pair_r:.2}\t{}",
                    rows.len(),
                    if ok { "yes" } else { "no" }
                );
            }
        }
    }
    println!();
    println!("VIABLE cells: {}", viable.len());
    for (f, l, r) in viable {
        println!("VIABLE scale={f} grace={l} reserve={r}");
    }
}
