//! Card SEL2-E follow-up: why live populations die out over 100,000 ticks.
//!
//! **Blind to body mass**: the allow-list below has no `live.trait_mass_*`; seeds 1–6 are
//! exploratory. Two worlds, 100 founders and 8 lakes each, 100,000 ticks, seeds 1–6:
//! - `base`: `prey_cap` 80, `resource_capacity_scale` 1.0, default dehydration grace (120);
//! - `best`: `prey_cap` 320, scale 0.7, `live.dehydration_ticks` 960 (the long-run cell with the
//!   fewest extinctions in `sel2e_long_runs.rs`).
//!
//! Per run it prints the extinction tick (if any), the season (`sin` of `live.season_phase`) there,
//! the population's minimum and maximum in each 6,000-tick year, and the births and deaths by cause
//! in the 3,000 ticks before extinction (or before the end).
//!
//! ```text
//! cargo run --release --example sel2e_extinction_probe > sel2e_extinction_probe.txt
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};

const TICKS: u64 = 100_000;
const YEAR: u64 = 6_000;
const WINDOW: u64 = 3_000;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const ALLOWED: [&str; 6] = [
    "live.agent_count",
    "live.births",
    "live.deaths",
    "live.deaths_starvation",
    "live.deaths_dehydration",
    "live.season_phase",
];

fn run(name: &str, prey_cap: f64, scale: f64, grace: f64, seed: u64) -> String {
    let initial = InitialConditionSet::new(
        [
            ("live.founders", 100.0),
            ("live.predator_fraction", 0.0),
            ("live.trees", 8.0),
            ("live.lakes", 8.0),
            ("live.mortality", 1.0),
            ("live.reproduction", 1.0),
            ("live.locomotion", 1.0),
            ("live.drive_floor", 0.5),
            ("live.thirst_steering", 1.0),
            ("live.prey_cap", prey_cap),
            ("live.resource_capacity_scale", scale),
            ("live.dehydration_ticks", grace),
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
    // Rolling per-tick history for the last WINDOW ticks: (births, starvation, dehydration).
    let mut hist = vec![(0.0f64, 0.0f64, 0.0f64); WINDOW as usize];
    let mut years: Vec<(f64, f64)> = Vec::new();
    let (mut ymin, mut ymax) = (f64::MAX, 0.0f64);
    let mut extinct_at: Option<(u64, f64)> = None;
    let mut last_tick = 0;
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
                .unwrap_or(0.0)
        };
        hist[(tick % WINDOW) as usize] = (
            get("live.births"),
            get("live.deaths_starvation"),
            get("live.deaths_dehydration"),
        );
        let n = get("live.agent_count");
        ymin = ymin.min(n);
        ymax = ymax.max(n);
        if tick % YEAR == 0 {
            years.push((ymin, ymax));
            ymin = f64::MAX;
            ymax = 0.0;
        }
        last_tick = tick;
        if n == 0.0 {
            extinct_at = Some((tick, (get("live.season_phase") as f32).sin() as f64));
            break;
        }
    }
    if ymax > 0.0 || ymin < f64::MAX {
        years.push((ymin, ymax));
    }
    let (b, s, d) = hist.iter().fold((0.0, 0.0, 0.0), |acc, h| {
        (acc.0 + h.0, acc.1 + h.1, acc.2 + h.2)
    });
    let yrs: Vec<String> = years.iter().map(|(lo, hi)| format!("{lo}-{hi}")).collect();
    format!(
        "{name}\tseed={seed}\t{}\tlast{WINDOW}: births={b} starv={s} dehyd={d}\tyears(min-max)={}",
        match extinct_at {
            Some((t, s)) => format!("EXTINCT tick={t} year={} season_sin={s:.2}", t / YEAR),
            None => format!("alive at {last_tick}"),
        },
        yrs.join(" ")
    )
}

fn main() {
    let handles: Vec<_> = SEEDS
        .iter()
        .flat_map(|&seed| {
            [("base", 80.0, 1.0, 120.0), ("best", 320.0, 0.7, 960.0)]
                .into_iter()
                .map(move |(name, cap, scale, grace)| {
                    std::thread::spawn(move || run(name, cap, scale, grace, seed))
                })
        })
        .collect();
    let mut lines: Vec<String> = handles
        .into_iter()
        .map(|h| h.join().expect("a run panicked"))
        .collect();
    lines.sort();
    for l in lines {
        println!("{l}");
    }
}
