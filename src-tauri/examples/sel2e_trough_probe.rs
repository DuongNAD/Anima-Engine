//! Card SEL2-E follow-up: why reproduction stops in the trough before extinction.
//!
//! Reads no body mass, genotype or trait observable. Per agent it reads only the energy and
//! hydration ratios (`energy / energy_target`, `hydration / hydration_target`) and the grass ratio
//! (`r / r_max`) of the pasture cell under the agent. Prey reproduce when `energy >= 0.65 ×
//! energy_target` (`autonomous_reproduction_system`).
//!
//! World: the SEL2-E "best" long-run cell (100 founders, 8 lakes, `prey_cap` 320, scale 0.7,
//! `live.dehydration_ticks` 960), seeds 2, 5 and 6, up to 40,000 ticks.
//!
//! ```text
//! cargo run --release --example sel2e_trough_probe > sel2e_trough_probe.txt
//! ```

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::{Agent, Position};
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use bevy_ecs::prelude::With;

const TICKS: u64 = 40_000;
const EVERY: u64 = 500;

fn run(seed: u64) -> Vec<String> {
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
            ("live.prey_cap", 320.0),
            ("live.resource_capacity_scale", 0.7),
            ("live.dehydration_ticks", 960.0),
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
    let mut out = Vec::new();
    let (mut births, mut deaths) = (0.0, 0.0);
    for tick in 1..=TICKS {
        a.run_schedule_once();
        let obs: Vec<(String, f64)> = {
            let mut w = a.world();
            live_observables(&mut w)
        }
        .into_iter()
        .filter(|(k, _)| {
            matches!(
                k.as_str(),
                "live.births" | "live.deaths" | "plants" | "detritus" | "live.animals_eu"
            )
        })
        .collect();
        let get = |id: &str| {
            obs.iter()
                .find(|(k, _)| k == id)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        };
        births += get("live.births");
        deaths += get("live.deaths");
        if tick % EVERY != 0 {
            continue;
        }
        let mut w = a.world();
        let field = w.resource::<ResourceField>().clone();
        let field_ratio = {
            let (mut s, mut n) = (0.0f64, 0usize);
            for (r, m) in field.r.iter().zip(field.r_max.iter()) {
                if *m > 0.0 {
                    s += (*r / *m) as f64;
                    n += 1;
                }
            }
            s / n.max(1) as f64
        };
        let mut q = w.query_filtered::<(&HomeostaticState, &Position), With<Agent>>();
        let rows: Vec<(f64, f64, f64)> = q
            .iter(&w)
            .map(|(h, p)| {
                let grass = field
                    .cell_index(p.0.x, p.0.z)
                    .map(|i| {
                        if field.r_max[i] > 0.0 {
                            (field.r[i] / field.r_max[i]) as f64
                        } else {
                            0.0
                        }
                    })
                    .unwrap_or(f64::NAN);
                (
                    (h.energy / h.energy_target.max(1e-6)) as f64,
                    (h.hydration / h.hydration_target.max(1e-6)) as f64,
                    grass,
                )
            })
            .collect();
        let n = rows.len();
        let mean = |f: &dyn Fn(&(f64, f64, f64)) -> f64| {
            if n == 0 {
                f64::NAN
            } else {
                rows.iter().map(f).sum::<f64>() / n as f64
            }
        };
        let can_breed = rows.iter().filter(|r| r.0 >= 0.65).count();
        out.push(format!(
            "seed={seed}\ttick={tick}\tn={n}\tbirths={births}\tdeaths={deaths}\tcan_breed={can_breed}\tenergy_ratio={:.3}\thydration_ratio={:.3}\tgrass_under_agents={:.3}\tgrass_field_mean={:.3}\tplants={:.0}\tdetritus={:.0}\tanimals_eu={:.0}",
            mean(&|r| r.0),
            mean(&|r| r.1),
            mean(&|r| r.2),
            field_ratio,
            get("plants"),
            get("detritus"),
            get("live.animals_eu"),
        ));
        births = 0.0;
        deaths = 0.0;
        if n == 0 {
            break;
        }
    }
    out
}

fn main() {
    let handles: Vec<_> = [2u64, 5, 6]
        .into_iter()
        .map(|s| std::thread::spawn(move || run(s)))
        .collect();
    for h in handles {
        for line in h.join().expect("a run panicked") {
            println!("{line}");
        }
    }
}
