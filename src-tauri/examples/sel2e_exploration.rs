//! Card SEL2-E: the second exploratory measurement of the SEL2 redesign (QD-SEL2a–e).
//!
//! Question: is there a configuration that keeps the population alive on every seed, for enough
//! generations, while food is what actually limits it?
//!
//! **Blind to body mass.** The driver reads the live observables through an allow-list
//! (`ALLOWED`) and drops everything else unread; no `live.trait_mass_*` value, genotype or body
//! size is printed, stored or used. Seeds 1–6 are exploratory, as in S6 and S9: a preregistration
//! built on this must choose its own seeds.
//!
//! Base world (S6/E-SEL1): no predators, 8 trees, mortality, reproduction, steering with
//! `drive_floor` 0.5 and thirst steering, shared brain, `prey_cap` 320, `resource_capacity_scale`
//! 1.0. Grid: `live.founders` ∈ {30, 60, 100} × `live.plant_reserve` ∈ {0, 0.2, 0.3, 0.4} ×
//! `live.lakes` ∈ {2, 4, 8} (QD-SEL2e) × seeds 1–6 = 216 runs, plus one baseline per
//! (founders, lakes, seed) at `prey_cap` 80 with no reserve (54 runs) for the crop ratio.
//!
//! ```text
//! cargo run --release --example sel2e_exploration > sel2e_exploration.tsv
//! ```
//!
//! Viability of a cell (SEL2 proposal §5, SEL2-E), every condition on every seed unless stated:
//! 1. the population sampled every 50 ticks never falls below 5;
//! 2. the grazeable-crop ratio `g` of the second half lies in 0.4–0.6, where
//!    `g = (C̄ − ρ·Σr_max) / (C̄_base − ρ·Σr_max)` (S10 note, condition 5) and `Σr_max` is the standing
//!    crop of the freshly built world before tick 1;
//! 3. at least 30 generations, estimated conservatively as the smaller of `TICKS / L̄` (mean
//!    lifespan at death over the whole run) and `births / N̄` (population turnovers);
//! 4. starvation causes more than half of the cell's pooled deaths;
//! 5. births and deaths both happen in the last quarter of the run.
//!
//! Also reported (QD-SEL2, research note §2.4): the across-seed Pearson correlation of the mean
//! population of each reserve cell with the no-reserve cell of the same founders and lakes. Six
//! seeds make it a rough proxy; the real pair correlation is on the mass endpoint, which this
//! driver must not read.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use std::sync::{Arc, Mutex};

const TICKS: u64 = 18_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const FOUNDERS: [f64; 3] = [30.0, 60.0, 100.0];
const RESERVES: [f64; 4] = [0.0, 0.2, 0.3, 0.4];
const LAKES: [f64; 3] = [2.0, 4.0, 8.0];
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
            ("live.resource_capacity_scale", 1.0),
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
        for &founders in &FOUNDERS {
            for &lakes in &LAKES {
                jobs.push(Job {
                    founders,
                    reserve: 0.0,
                    lakes,
                    prey_cap: BASE_PREY_CAP,
                    seed,
                });
                for &reserve in &RESERVES {
                    jobs.push(Job {
                        founders,
                        reserve,
                        lakes,
                        prey_cap: PREY_CAP,
                        seed,
                    });
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
            a.job.founders,
            a.job.lakes,
            a.job.prey_cap,
            a.job.reserve,
            a.job.seed,
        )
            .partial_cmp(&(
                b.job.founders,
                b.job.lakes,
                b.job.prey_cap,
                b.job.reserve,
                b.job.seed,
            ))
            .unwrap()
    });

    let base = |founders: f64, lakes: f64, seed: u64| {
        all.iter()
            .find(|r| {
                r.job.prey_cap == BASE_PREY_CAP
                    && r.job.founders == founders
                    && r.job.lakes == lakes
                    && r.job.seed == seed
            })
            .expect("the baseline ran")
    };
    let g = |r: &RunStats| {
        let floor = r.job.reserve * r.sum_r_max;
        let b = base(r.job.founders, r.job.lakes, r.job.seed);
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

    println!("founders\tlakes\tprey_cap\treserve\tseed\tsum_r_max\tg\tmin_agents\tmean_agents\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover\tbirths_last_q\tdeaths_last_q");
    for r in &all {
        let (gl, gt, _) = generations(r);
        let gv = if r.job.prey_cap == PREY_CAP {
            g(r)
        } else {
            f64::NAN
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.3}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}",
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
    println!("SUMMARY founders\tlakes\treserve\tc1_min_agents\tc2_g_min..max\tc3_generations_min\tc4_share_starvation\tc4_share_min..max_over_seeds\tc5_seeds_active\tpair_r_mean_agents_vs_reserve0\tVIABLE");
    let mut viable = Vec::new();
    for &founders in &FOUNDERS {
        for &lakes in &LAKES {
            for &reserve in &RESERVES {
                let rows: Vec<&RunStats> = all
                    .iter()
                    .filter(|r| {
                        r.job.prey_cap == PREY_CAP
                            && r.job.founders == founders
                            && r.job.lakes == lakes
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
                    viable.push((founders, lakes, reserve));
                }
                println!(
                    "SUMMARY {founders}\t{lakes}\t{reserve}\t{min_pop}\t{glo:.3}..{ghi:.3}\t{gen_min:.1}\t{share:.3}\t{slo:.3}..{shi:.3}\t{active}/{}\t{pair_r:.2}\t{}",
                    rows.len(),
                    if ok { "yes" } else { "no" }
                );
            }
        }
    }
    println!();
    println!("VIABLE cells: {}", viable.len());
    for (f, l, r) in viable {
        println!("VIABLE founders={f} lakes={l} reserve={r}");
    }
}
