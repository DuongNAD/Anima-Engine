//! Which pairs of tick systems have no declared order, and could therefore run either way round.
//!
//! CLAUDE.md's G1.3 rule is that system execution order must be **declared, not incidental**. Bevy
//! enforces that two systems with conflicting data access never run *concurrently*, but under the
//! single-threaded executor it still has to pick one of them to run first, and if nothing orders
//! them it picks by topological sort — which is not a declared property of the schedule.
//!
//! `ScheduleGraph::conflicting_systems` is Bevy's own answer to "which pairs are those", so this
//! asks it rather than guessing from reading `.after(...)` chains. The list is printed by
//! `report_every_ambiguous_pair_in_the_tick_schedule` (run with `--nocapture`) and the pairs that
//! matter — the ones that move EU — are asserted absent by
//! `no_two_systems_that_move_energy_are_left_unordered`.
//!
//! **130 ambiguous pairs** were reported at the time of writing. Most are benign and this file does
//! not claim otherwise; the count is printed rather than asserted precisely because driving it to
//! zero is a separate and much larger piece of work than declaring the energy ordering. One of those
//! remaining pairs does move the trajectory — `tests/live_cross_process_probe.rs` shows the world
//! checksum changing in roughly one process in twelve — and finding which is open work.
//!
//! # Why the energy systems specifically
//!
//! An ambiguity is only a bug if the two orders give different answers. Most do not: two systems
//! writing disjoint components of different entities commute. The ones that do not commute are the
//! systems that move EU between compartments, because addition of `f32` reserves with clamping at
//! zero is order-dependent, and because whether an agent eats before or after it burns energy
//! decides whether it starves.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;

/// Systems that move EU into or out of an agent reserve or a biomass compartment. Two of these
/// running in an undeclared order is a trajectory that depends on a hash seed.
const ENERGY_SYSTEMS: [&str; 6] = [
    "metabolic_decay_system",
    "detect_food_collisions_system",
    "combat_system",
    "herbivore_grazing_system",
    "resource_field_regrowth_system",
    "ecosystem_census_system",
];

fn short(name: &str) -> String {
    // `System::name()` is a full path, sometimes with generic parameters.
    name.rsplit("::").next().unwrap_or(name).to_string()
}

fn built_adapter() -> LiveExperimentAdapter {
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 10.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        4_242_424_242,
        (16, 16),
        0,
    )
    .expect("the live world builds");
    // The graph is only populated once the schedule has been built against a world.
    a.run_schedule_once();
    a
}

/// Every ambiguous pair, as Bevy sees it. `(NodeId, NodeId, Vec<ComponentId>)`; an empty component
/// list means the pair conflicts on `World` access rather than on a named component.
fn ambiguous_pairs(a: &mut LiveExperimentAdapter) -> Vec<(String, String, usize)> {
    let graph = a.schedule_mut().graph();
    let names: std::collections::HashMap<_, _> = graph
        .systems()
        .map(|(id, system, _)| (id, short(&system.name())))
        .collect();
    let mut out: Vec<(String, String, usize)> = graph
        .conflicting_systems()
        .iter()
        .map(|(a, b, comps)| {
            let (x, y) = (
                names.get(a).cloned().unwrap_or_else(|| format!("{a:?}")),
                names.get(b).cloned().unwrap_or_else(|| format!("{b:?}")),
            );
            if x <= y {
                (x, y, comps.len())
            } else {
                (y, x, comps.len())
            }
        })
        .collect();
    out.sort();
    out
}

#[test]
fn report_every_ambiguous_pair_in_the_tick_schedule() {
    let mut a = built_adapter();
    let pairs = ambiguous_pairs(&mut a);
    println!("\n{} ambiguous pairs in build_tick_schedule:", pairs.len());
    for (x, y, n) in &pairs {
        let energy = ENERGY_SYSTEMS.contains(&x.as_str()) && ENERGY_SYSTEMS.contains(&y.as_str());
        println!(
            "  {}{} <-> {} ({n} conflicting components)",
            if energy { "ENERGY  " } else { "        " },
            x,
            y
        );
    }
}

#[test]
fn no_two_systems_that_move_energy_are_left_unordered() {
    let mut a = built_adapter();
    let offenders: Vec<String> = ambiguous_pairs(&mut a)
        .into_iter()
        .filter(|(x, y, _)| {
            ENERGY_SYSTEMS.contains(&x.as_str()) && ENERGY_SYSTEMS.contains(&y.as_str())
        })
        .map(|(x, y, _)| format!("{x} <-> {y}"))
        .collect();
    assert!(
        offenders.is_empty(),
        "these pairs both move EU and have no declared order, so which one runs first is decided \
         by the schedule's topological sort rather than by the schedule: {offenders:#?}\n\n\
         Two orders that give different answers is not an optimisation detail — it is a trajectory \
         that depends on a per-process hash seed. Declare the order in \
         `core::simulation_schedule::build_tick_schedule`."
    );
}
