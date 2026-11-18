//! Card S9: in the scarce S6 configurations, does hunger or thirst kill the population?
//!
//! Re-measures part of the S6 grid (`examples/sel2_exploration.rs`) with the death cause split
//! that S9 added: `live.prey_cap` in {80, 160} × `live.resource_capacity_scale` in {0.5, 0.7} ×
//! seeds 1–6, 18,000 ticks each, on the S6 base world (30 prey founders, no predators, 8 trees,
//! 2 lakes, mortality, reproduction, steering with `drive_floor` 0.5 and thirst steering).
//!
//! **Blind to body mass, exactly as S6.** The driver reads the live observables through an
//! allow-list (`ALLOWED`) and drops everything else unread; no `live.trait_mass_*` value, genotype
//! or body size is printed, stored or used. Besides the allow-list it reads one counter straight
//! from `VitalStatistics`: `total_deaths_both_lethal`, the deaths whose cause the attribution
//! rule decided because hunger and thirst were both lethal on the tick of death.
//!
//! ```text
//! cargo run --release --example s9_death_causes > s9_death_causes.tsv
//! ```
//!
//! Output: one tab-separated row per (prey_cap, scale, seed), then one `SUMMARY` row per
//! (prey_cap, scale) pooling its six seeds. Every tick is read, so the windows add up to the run's
//! totals, and every tick checks `starvation + dehydration == deaths`; a violation aborts the run.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use anima_engine_lib::core::resources::VitalStatistics;

const TICKS: u64 = 18_000;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const PREY_CAPS: [f64; 2] = [80.0, 160.0];
const SCALES: [f64; 2] = [0.5, 0.7];
/// The only observables this driver reads. Nothing about body mass.
const ALLOWED: [&str; 5] = [
    "live.agent_count",
    "live.births",
    "live.deaths",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

#[derive(Clone, Debug)]
struct RunStats {
    prey_cap: f64,
    scale: f64,
    seed: u64,
    final_agents: f64,
    births: f64,
    deaths: f64,
    starvation: f64,
    dehydration: f64,
    both_lethal: u64,
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
    let (mut births, mut deaths, mut starvation, mut dehydration) = (0.0, 0.0, 0.0, 0.0);
    let mut final_agents = 0.0;
    for tick in 1..=TICKS {
        a.run_schedule_once();
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
                .unwrap_or_else(|| panic!("live_observables has no `{id}`"))
        };
        let (d, s, h) = (
            get("live.deaths"),
            get("live.deaths_starvation"),
            get("live.deaths_dehydration"),
        );
        assert_eq!(
            s + h,
            d,
            "prey_cap {prey_cap} scale {scale} seed {seed} tick {tick}: cause split does not add up"
        );
        births += get("live.births");
        deaths += d;
        starvation += s;
        dehydration += h;
        final_agents = get("live.agent_count");
    }
    let both_lethal = a
        .world()
        .resource::<VitalStatistics>()
        .total_deaths_both_lethal;
    RunStats {
        prey_cap,
        scale,
        seed,
        final_agents,
        births,
        deaths,
        starvation,
        dehydration,
        both_lethal,
    }
}

fn share(part: f64, whole: f64) -> String {
    if whole > 0.0 {
        format!("{:.3}", part / whole)
    } else {
        "n/a".to_string()
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
    println!(
        "prey_cap\tscale\tseed\tfinal_agents\tbirths\tdeaths\tstarvation\tdehydration\tboth_lethal\tshare_starvation\tshare_dehydration"
    );
    for r in &all {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.prey_cap,
            r.scale,
            r.seed,
            r.final_agents,
            r.births,
            r.deaths,
            r.starvation,
            r.dehydration,
            r.both_lethal,
            share(r.starvation, r.deaths),
            share(r.dehydration, r.deaths),
        );
    }
    println!();
    println!(
        "SUMMARY prey_cap\tscale\tdeaths\tstarvation\tdehydration\tboth_lethal\tshare_starvation\tshare_dehydration\tshare_starvation_min..max_over_seeds\textinct_seeds"
    );
    for &(c, s) in &configs {
        let rows: Vec<&RunStats> = all
            .iter()
            .filter(|r| r.prey_cap == c && r.scale == s)
            .collect();
        let deaths: f64 = rows.iter().map(|r| r.deaths).sum();
        let starvation: f64 = rows.iter().map(|r| r.starvation).sum();
        let dehydration: f64 = rows.iter().map(|r| r.dehydration).sum();
        let both: u64 = rows.iter().map(|r| r.both_lethal).sum();
        let per_seed: Vec<f64> = rows
            .iter()
            .filter(|r| r.deaths > 0.0)
            .map(|r| r.starvation / r.deaths)
            .collect();
        let lo = per_seed.iter().cloned().fold(f64::MAX, f64::min);
        let hi = per_seed.iter().cloned().fold(f64::MIN, f64::max);
        let extinct = rows.iter().filter(|r| r.final_agents == 0.0).count();
        println!(
            "SUMMARY {c}\t{s}\t{deaths}\t{starvation}\t{dehydration}\t{both}\t{}\t{}\t{lo:.3}..{hi:.3}\t{extinct}/{}",
            share(starvation, deaths),
            share(dehydration, deaths),
            rows.len()
        );
    }
}
