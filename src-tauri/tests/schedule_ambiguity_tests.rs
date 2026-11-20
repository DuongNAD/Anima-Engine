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
//! **Zero ambiguous pairs** since F-041: the tick schedule is one total order, and
//! `the_tick_schedule_has_no_ambiguous_pair_at_all` asserts that Bevy reports none. Before that
//! change it reported 128 (about 80 with real component conflicts, the rest only through
//! `Commands` spawn/despawn), the executed order differed between processes, and mortality plus
//! continuous reproduction gave two different worlds from one seed. The energy and `SimRng` gates
//! below are now implied by it but are kept: they name the invariant that matters if someone ever
//! relaxes the chain. `declared_order_keeps_the_causal_sequences` pins, by position in the executed
//! order, the readings the old per-system `.after(...)` edges stated.
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

/// Systems that take `ResMut<SimRng>`. `ResMut` makes Bevy serialise them, not order them: two of
/// these with no declared edge between them draw from the shared stream in an order chosen by the
/// schedule's topological sort, which depends on a per-process hash seed.
const SIMRNG_SYSTEMS: [&str; 5] = [
    "autonomous_reproduction_system",
    "check_epoch_completion_system",
    "manual_migration_system",
    "seed_dropping_system",
    "spawn_food_system",
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
///
/// # The names must come from `Schedule::systems`, not `ScheduleGraph::systems`
///
/// This function got that wrong once, and the mistake is worth a comment because it is silent.
/// `ScheduleGraph::systems` yields `system_node.inner.as_deref()?` — and initialization *moves*
/// every system out of its graph node into `Schedule::executable`, leaving `inner` as `None`. After
/// the schedule has run even once, that iterator is therefore **empty**: the name map came out
/// empty, every lookup fell through to the `{id:?}` fallback, and a gate asserting that no pair of
/// named energy systems is ambiguous passed by comparing `"System(13)"` against a list it could
/// never match. It printed `System(0) <-> System(13)`, which was the visible evidence that nothing
/// was being checked.
///
/// `Schedule::systems` reads `executable`, which is where the systems actually are, and returns
/// `Err(ScheduleNotInitialized)` rather than an empty iterator if they are not — a failure that is
/// loud instead of vacuous.
fn ambiguous_pairs(a: &mut LiveExperimentAdapter) -> Vec<(String, String, usize)> {
    let schedule: &bevy_ecs::schedule::Schedule = a.schedule_mut();
    let names: std::collections::HashMap<_, _> = schedule
        .systems()
        .expect("the schedule has been initialized by run_schedule_once")
        .map(|(id, system)| (id, short(system.name().as_ref())))
        .collect();
    assert!(
        !names.is_empty(),
        "no system names were resolved, so any assertion built on them would be vacuous"
    );
    let mut out: Vec<(String, String, usize)> = schedule
        .graph()
        .conflicting_systems()
        .iter()
        .map(|(a, b, comps)| {
            // No `unwrap_or_else` fallback: an unresolved id is the failure this function exists to
            // never have again, so it panics rather than printing something that looks like data.
            let (x, y) = (
                names
                    .get(a)
                    .unwrap_or_else(|| panic!("no name for {a:?}"))
                    .clone(),
                names
                    .get(b)
                    .unwrap_or_else(|| panic!("no name for {b:?}"))
                    .clone(),
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
        let simrng = SIMRNG_SYSTEMS.contains(&x.as_str()) && SIMRNG_SYSTEMS.contains(&y.as_str());
        let tag = match (energy, simrng) {
            (true, _) => "ENERGY  ",
            (false, true) => "SIMRNG  ",
            (false, false) => "        ",
        };
        println!("  {tag}{x} <-> {y} ({n} conflicting components)");
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

#[test]
fn no_two_systems_that_draw_simrng_are_left_unordered() {
    let mut a = built_adapter();
    let offenders: Vec<String> = ambiguous_pairs(&mut a)
        .into_iter()
        .filter(|(x, y, _)| {
            SIMRNG_SYSTEMS.contains(&x.as_str()) && SIMRNG_SYSTEMS.contains(&y.as_str())
        })
        .map(|(x, y, _)| format!("{x} <-> {y}"))
        .collect();
    assert!(
        offenders.is_empty(),
        "these pairs both draw from `SimRng` and have no declared order, so which one draws first \
         is decided by the schedule's topological sort rather than by the schedule: {offenders:#?}\n\n\
         `ResMut<SimRng>` serialises draws, it does not order them — two orders give two different \
         random sequences per process. Declare the order in \
         `core::simulation_schedule::build_tick_schedule`."
    );
}
/// The whole point of F-041: with one total order there is nothing left for the per-process
/// topological sort to choose, whether the pair conflicts on a component, on `World` access, or
/// only through `Commands` spawn/despawn (the last kind is invisible to Bevy's conflict report
/// *unless* the pair is unordered, which is why zero unordered pairs is the right assertion).
#[test]
fn the_tick_schedule_has_no_ambiguous_pair_at_all() {
    let mut a = built_adapter();
    let pairs = ambiguous_pairs(&mut a);
    assert!(
        pairs.is_empty(),
        "{} ambiguous pairs in build_tick_schedule; the executed order of each is chosen per \
         process (F-029, F-041). Put every system in the chain in \
         `core::simulation_schedule::build_tick_schedule`: {pairs:#?}",
        pairs.len()
    );
}

/// The executed order, by name. `Schedule::systems` yields `executable.system_ids`, which is the
/// topological order the single-threaded executor walks.
fn executed_order(a: &mut LiveExperimentAdapter) -> Vec<String> {
    a.schedule_mut()
        .systems()
        .expect("the schedule has been initialized by run_schedule_once")
        .map(|(_, s)| short(s.name().as_ref()))
        .collect()
}

fn assert_in_order(order: &[String], chain: &[&str]) {
    let pos = |n: &str| {
        order
            .iter()
            .position(|x| x == n)
            .unwrap_or_else(|| panic!("{n} is not in the tick schedule"))
    };
    for w in chain.windows(2) {
        assert!(
            pos(w[0]) < pos(w[1]),
            "{} must run before {} in the tick schedule",
            w[0],
            w[1]
        );
    }
}

#[test]
fn declared_order_keeps_the_causal_sequences() {
    let mut a = built_adapter();
    let order = executed_order(&mut a);
    // Energy: acquire -> contest -> spend -> count.
    assert_in_order(
        &order,
        &[
            "herbivore_grazing_system",
            "resource_field_regrowth_system",
            "detect_food_collisions_system",
            "combat_system",
            "metabolic_decay_system",
            "ecosystem_census_system",
        ],
    );
    // SimRng draws: migration -> food -> seeds -> reproduction -> epoch.
    assert_in_order(
        &order,
        &[
            "manual_migration_system",
            "spawn_food_system",
            "seed_dropping_system",
            "autonomous_reproduction_system",
            "check_epoch_completion_system",
        ],
    );
    // Body: sense -> act -> CPG -> joints -> physics -> terrain_navigation -> wrap -> grid.
    assert_in_order(
        &order,
        &[
            "sensory_system",
            "action_resolution_system",
            "update_cpg_system",
            "resolve_joints_system",
            "steering_drive_system",
            "integrate_physics_system",
            "terrain_navigation_system",
            "wrap_coordinates_system",
            "rebuild_spatial_grid_system",
        ],
    );
    // Interventions reach the world before the systems they perturb.
    for later in [
        "apply_environmental_effects_system",
        "resource_field_regrowth_system",
        "metabolic_decay_system",
    ] {
        assert_in_order(&order, &["apply_live_interventions_system", later]);
    }
    // Death sits between metabolism and census; reproduction is flushed before the census.
    assert_in_order(
        &order,
        &[
            "metabolic_decay_system",
            "mortality_system",
            "autonomous_reproduction_system",
            "ecosystem_census_system",
        ],
    );
    assert_in_order(
        &order,
        &[
            "dehydrate_cold_agents_system",
            "rehydrate_wakeable_chunks_system",
        ],
    );
}

/// Flush points are declared (`auto_insert_apply_deferred` is off), so a `Commands` system's
/// effects must not be left for the end of the tick between a spawner and an observer.
#[test]
fn spawns_are_flushed_before_their_observers() {
    let mut a = built_adapter();
    let order = executed_order(&mut a);
    let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
    let flush_between =
        |x: &str, y: &str| order[pos(x)..pos(y)].iter().any(|n| n == "apply_deferred");
    assert!(flush_between(
        "autonomous_reproduction_system",
        "ecosystem_census_system"
    ));
    assert!(flush_between("spawn_food_system", "combat_system"));
    assert!(flush_between(
        "mortality_system",
        "autonomous_reproduction_system"
    ));
    assert!(flush_between(
        "dehydrate_cold_agents_system",
        "rehydrate_wakeable_chunks_system"
    ));
}
