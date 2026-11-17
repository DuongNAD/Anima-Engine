//! G3 / brief B6(a): the scientific export must report what the running world measured, and say so
//! when it measured nothing.
//!
//! The export used to carry values nothing measured: a seed of 42 for every run, the animals' energy
//! split 70/30 into prey and predators, an energy residual of zero, two metabolic rates, an ambient
//! temperature of 20 degrees, and an energy-flux report that was nine hard-coded numbers because
//! nothing writes the `EnergyFluxMatrix`. A value like that in a data export is indistinguishable from
//! a measurement.
//!
//! These tests run a real `SimulationEngine` on its real thread and compare the export with
//! independent witnesses: the seed with what a save request reads off the world's own `SimRng`, and
//! the trophic biomass with the reserves of the agents the simulation thread published.

use anima_engine_lib::commands::{
    build_scientific_export, read_energy_flux, EvolutionSettings, MapElitesGridState,
    ScientificExportRequest,
};
use anima_engine_lib::core::ecs::AgentType;
use anima_engine_lib::core::engine::SimulationEngine;
use anima_engine_lib::core::simulation_lifecycle::SavedSimulationState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The engine reads `ANIMA_SIM_SEED` when it starts, and a test that sets it must not share the
/// process with another test that starts an engine at the same moment.
static ENGINE_LOCK: Mutex<()> = Mutex::new(());

const READY_DEADLINE: Duration = Duration::from_secs(30);

fn start(engine: &SimulationEngine) {
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        })),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        })),
    );
}

fn await_tick(engine: &SimulationEngine, tick: u64) {
    let started = Instant::now();
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count >= tick {
            return;
        }
        assert!(
            started.elapsed() < READY_DEADLINE,
            "the simulation did not reach tick {tick} within {READY_DEADLINE:?}; status {status:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn request_save(engine: &SimulationEngine) -> SavedSimulationState {
    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .expect("the save request is accepted");
    rx.recv_timeout(Duration::from_secs(30))
        .expect("the simulation thread answers a save request")
        .expect("a live world saves")
}

fn empty_map_elites() -> anima_engine_lib::core::telemetry_export::MapElitesSummary {
    MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }
    .compute_summary()
}

fn everything() -> ScientificExportRequest {
    ScientificExportRequest {
        include_time_series: true,
        include_demographics: true,
        include_energy_flux: true,
        include_spatial_density: true,
        include_phylogeny: true,
        ..Default::default()
    }
}

/// An engine that has run a few ticks and been stopped: its published state is frozen, so the
/// witnesses and the export read the same world.
fn ran_and_stopped() -> SimulationEngine {
    let engine = SimulationEngine::new();
    start(&engine);
    await_tick(&engine, 5);
    engine.stop();
    engine
}

// ---- The seed -------------------------------------------------------------------------------------

#[test]
fn the_exported_seed_is_the_seed_the_run_started_with() {
    let _guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // A seed that is not 42, set the way an experiment sweep sets it.
    std::env::set_var("ANIMA_SIM_SEED", "777");
    let engine = SimulationEngine::new();
    start(&engine);
    await_tick(&engine, 3);
    std::env::remove_var("ANIMA_SIM_SEED");

    // The witness: what the world's own RNG reports when the simulation thread serializes it. If
    // this is not 777 the premise of the test is broken, not the export.
    let saved = request_save(&engine);
    assert_eq!(
        saved.sim_rng_seed, 777,
        "premise: the override must reach the world's RNG"
    );
    engine.stop();

    let parts = build_scientific_export(&engine, empty_map_elites(), &everything())
        .expect("a run that ticked can be exported");
    assert_eq!(
        parts.package.provenance.seed, 777,
        "the export reports a seed this run did not use"
    );
}

#[test]
fn an_engine_that_never_ran_has_no_seed_to_export() {
    let _guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // There is no run, so there is no seed. Reporting one anyway is exactly the fabrication this
    // card removes.
    let engine = SimulationEngine::new();
    let result = build_scientific_export(&engine, empty_map_elites(), &everything());
    assert!(
        result.is_err(),
        "an export of a run that never started claimed seed {:?}",
        result.map(|p| p.package.provenance.seed)
    );
}

// ---- Trophic biomass ------------------------------------------------------------------------------

#[test]
fn exported_trophic_biomass_is_what_the_agents_hold_by_role() {
    let _guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = ran_and_stopped();

    // The witness sums each agent once, through its root segment. Every segment of an agent carries
    // the agent's whole reserve, so summing segments would count a three-segment body three times.
    let (prey, predator, segments, roots) = {
        let states = engine
            .agent_states
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let (mut prey, mut predator, mut roots) = (0.0f64, 0.0f64, 0usize);
        for s in states.iter().filter(|s| s.parent_segment_id.is_none()) {
            roots += 1;
            match s.agent_type {
                Some(AgentType::Prey) => prey += s.energy.max(0.0) as f64,
                Some(AgentType::Predator) => predator += s.energy.max(0.0) as f64,
                None => {}
            }
        }
        (prey, predator, states.len(), roots)
    };
    assert!(
        segments > roots && prey > 0.0 && predator > 0.0,
        "premise: a world with bodies of several segments and both roles alive \
         (segments {segments}, agents {roots}, prey {prey}, predators {predator})"
    );

    let parts = build_scientific_export(&engine, empty_map_elites(), &everything())
        .expect("a run that ticked can be exported");
    let sample = parts
        .package
        .time_series
        .first()
        .expect("one time-series sample");
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
    assert!(
        close(sample.prey_biomass, prey),
        "prey biomass {} is not the {prey} the prey agents hold",
        sample.prey_biomass
    );
    assert!(
        close(sample.predator_biomass, predator),
        "predator biomass {} is not the {predator} the predator agents hold",
        sample.predator_biomass
    );
}

// ---- Values nothing measured ----------------------------------------------------------------------

#[test]
fn the_export_carries_no_value_that_nothing_measured() {
    let _guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = ran_and_stopped();

    let parts = build_scientific_export(&engine, empty_map_elites(), &everything())
        .expect("a run that ticked can be exported");
    let json = serde_json::to_string(&parts.package).expect("the package serializes");

    // The simulation measures none of these. Each used to be written as a constant, and a constant
    // in a data export reads as a measurement.
    for invented in [
        "avgPreyMetabolicRate",
        "avgPredatorMetabolicRate",
        "ambientTemperatureC",
        "energyResidual",
    ] {
        assert!(
            !json.contains(invented),
            "the export still carries `{invented}`, which nothing measures"
        );
    }
}

#[test]
fn an_unmeasured_energy_flux_is_absent_from_the_export_and_refused_by_the_command() {
    let _guard = ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = ran_and_stopped();

    // Nothing writes the `EnergyFluxMatrix`, so the live world has no measured flux at all.
    let parts = build_scientific_export(&engine, empty_map_elites(), &everything())
        .expect("a run that ticked can be exported");
    assert!(
        parts.package.energy_flux.is_none(),
        "the export carries an energy-flux report ({} nodes, {} links) that nothing measured",
        parts
            .package
            .energy_flux
            .as_ref()
            .map_or(0, |f| f.nodes.len()),
        parts
            .package
            .energy_flux
            .as_ref()
            .map_or(0, |f| f.links.len())
    );

    // The dashboard command says "not measured" instead of returning a diagram of invented numbers.
    assert!(
        read_energy_flux(&engine).is_err(),
        "get_energy_flux_matrix returned a report although nothing measured the flux"
    );
}
