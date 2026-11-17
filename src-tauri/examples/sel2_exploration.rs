//! Card S6: the exploratory measurement before any new scarcity preregistration (QD-SEL1b = A).
//!
//! Question: with the prey reproduction ceiling raised (`live.prey_cap`), which pasture capacity
//! (`live.resource_capacity_scale`) brings the standing crop into the 0.4–0.6 band a scarcity
//! preregistration needs, while the population stays at five or more on every seed?
//!
//! **Blind to body mass.** The driver reads the live observables through an allow-list
//! (`ALLOWED`) and drops everything else unread; no `live.trait_mass_*` value, genotype or body
//! size is printed, stored or used. Seeds 1–6 match the E-SEL1 diagnosis and are not confirmatory
//! seeds: a preregistration built on this must choose its own.
//!
//! The base world is the E-SEL1 one: 30 prey founders, no predators, 8 trees, 2 lakes, mortality,
//! reproduction, steering with `drive_floor` 0.5 and thirst steering, shared brain, `food_cap` 50.
//!
//! ```text
//! cargo run --release --example sel2_exploration > sel2_exploration.tsv
//! ```
//!
//! Output: one tab-separated row per (prey_cap, scale, seed), then a summary per (prey_cap, scale).
//! The crop ratio is the mean standing crop over the second half of the run divided by the same
//! seed's baseline (prey_cap 80, scale 1.0), as in the diagnosis.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};

const TICKS: u64 = 18_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const PREY_CAPS: [f64; 3] = [80.0, 160.0, 320.0];
const SCALES: [f64; 4] = [1.0, 0.7, 0.5, 0.35];
/// The only observables this driver reads. Nothing about body mass.
const ALLOWED: [&str; 4] = [
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
];

#[derive(Clone, Debug)]
struct RunStats {
    prey_cap: f64,
    scale: f64,
    seed: u64,
    crop_second_half: f64,
    min_agents: f64,
    max_agents: f64,
    final_agents: f64,
    births: f64,
    deaths: f64,
}

fn run(prey_cap: f64, scale: f64, seed: u64) -> RunStats {
    let initial = InitialConditionSet::new(
        [
            ("live.founders", 30.0),
            ("live.predator_fraction", 0.0),
            ("live.trees", 8.0),
            ("live.lakes", 2.0),
            ("live.mortality", 1.0),
            ("live.reproduction", 1.0),
            ("live.locomotion", 1.0),
            ("live.drive_floor", 0.5),
            ("live.thirst_steering", 1.0),
            ("live.prey_cap", prey_cap),
            ("live.resource_capacity_scale", scale),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    );
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        seed,
        (16, 16),
        TICKS,
    )
    .expect("the declared world builds");
    let (mut crop_sum, mut crop_n) = (0.0, 0u64);
    let (mut min_agents, mut max_agents, mut final_agents) = (f64::MAX, 0.0f64, 0.0);
    let (mut births, mut deaths) = (0.0, 0.0);
    for tick in 1..=TICKS {
        a.run_schedule_once();
        // Every tick is read, so the births/deaths windows add up to the run's totals.
        let obs: Vec<(String, f64)> = {
            let mut w = a.world();
            live_observables(&mut w)
        }
        .into_iter()
        .filter(|(k, _)| ALLOWED.contains(&k.as_str()))
        .collect();
        let get = |id: &str| {
            obs.iter()
                .find(|(k, _)| k == id)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        };
        births += get("live.births");
        deaths += get("live.deaths");
        if tick % SAMPLE_EVERY == 0 {
            let agents = get("live.agent_count");
            min_agents = min_agents.min(agents);
            max_agents = max_agents.max(agents);
            final_agents = agents;
            if tick > TICKS / 2 {
                crop_sum += get("live.standing_crop");
                crop_n += 1;
            }
        }
    }
    RunStats {
        prey_cap,
        scale,
        seed,
        crop_second_half: crop_sum / crop_n.max(1) as f64,
        min_agents,
        max_agents,
        final_agents,
        births,
        deaths,
    }
}

fn main() {
    let configs: Vec<(f64, f64)> = PREY_CAPS
        .iter()
        .flat_map(|&c| SCALES.iter().map(move |&s| (c, s)))
        .collect();
    // One thread per seed; each builds its own worlds.
    let handles: Vec<_> = SEEDS
        .iter()
        .map(|&seed| {
            let configs = configs.clone();
            std::thread::spawn(move || {
                configs
                    .iter()
                    .map(|&(c, s)| run(c, s, seed))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut all: Vec<RunStats> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("a seed thread panicked"))
        .collect();
    all.sort_by(|a, b| {
        (a.prey_cap, a.scale, a.seed)
            .partial_cmp(&(b.prey_cap, b.scale, b.seed))
            .unwrap()
    });
    let baseline = |seed: u64| {
        all.iter()
            .find(|r| r.seed == seed && r.prey_cap == 80.0 && r.scale == 1.0)
            .map(|r| r.crop_second_half)
            .expect("the baseline ran for every seed")
    };
    println!(
        "prey_cap\tscale\tseed\tcrop_ratio\tmin_agents\tmax_agents\tfinal_agents\tbirths\tdeaths"
    );
    for r in &all {
        println!(
            "{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
            r.prey_cap,
            r.scale,
            r.seed,
            r.crop_second_half / baseline(r.seed),
            r.min_agents,
            r.max_agents,
            r.final_agents,
            r.births,
            r.deaths
        );
    }
    println!();
    println!("SUMMARY prey_cap\tscale\tcrop_ratio_min..max\tmin_agents_min\tseeds_extinct\tseeds_min_ge_5");
    for &(c, s) in &configs {
        let rows: Vec<&RunStats> = all
            .iter()
            .filter(|r| r.prey_cap == c && r.scale == s)
            .collect();
        let ratios: Vec<f64> = rows
            .iter()
            .map(|r| r.crop_second_half / baseline(r.seed))
            .collect();
        let lo = ratios.iter().cloned().fold(f64::MAX, f64::min);
        let hi = ratios.iter().cloned().fold(f64::MIN, f64::max);
        let min_pop = rows.iter().map(|r| r.min_agents).fold(f64::MAX, f64::min);
        let extinct = rows.iter().filter(|r| r.final_agents == 0.0).count();
        let ge5 = rows.iter().filter(|r| r.min_agents >= 5.0).count();
        println!(
            "SUMMARY {c}\t{s}\t{lo:.3}..{hi:.3}\t{min_pop}\t{extinct}/{}\t{ge5}/{}",
            rows.len(),
            rows.len()
        );
    }
}
