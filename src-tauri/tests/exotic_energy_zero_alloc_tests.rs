//! AE-203 (AE2): the exotic-energy field update hot loop must not allocate. Mirrors the
//! dynamic-field zero-alloc test — a process-global tracking allocator counts heap allocations
//! across a batch of field ticks (source → decay → diffusion) and asserts zero.

mod common;

use anima_engine_lib::core::evolution_pathway::{
    EnergyPathwayGenotype, ReferencePopulation, ReferencePopulationConfig,
};
use anima_engine_lib::core::exotic_energy::{
    ExoticEnergyField, ExoticEnergyLaw, ExoticIntervention, ExoticInterventionKind,
};
use anima_engine_lib::core::intervention::{Curve, Region};
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_exotic_field_hotloop_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Build the field (allocates) BEFORE tracking, from a deterministic patchy law. The initial
    // amount is within the field's capacity (8 hotspots × max_density over a 64² grid).
    let law = ExoticEnergyLaw::mana_patchy(300.0, 8);
    let mut field = ExoticEnergyField::from_law(&law, 64, 64, 2026).expect("feasible field");

    // Warm up so any lazy/one-time allocations are done before we start counting.
    for _ in 0..5 {
        field.step();
    }

    ALLOCATOR.start_tracking();

    // The steady-state per-tick update: source injection → decay → 5-point diffusion into the
    // preallocated double buffer. No allocation should occur.
    for _ in 0..1000 {
        field.step();
    }

    let alloc_count = ALLOCATOR.stop_tracking();
    assert_eq!(
        alloc_count, 0,
        "exotic-field hot loop triggered {alloc_count} heap allocations!"
    );
}

/// AE-209 (audit D3): the FINAL hot path — per-tick suppression reset, `RemoveSource` suppression
/// accumulation, `AddSource`/`Pulse` injection and the field step — must also allocate nothing.
#[test]
fn test_exotic_forcing_hotloop_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let law = ExoticEnergyLaw::mana_patchy(300.0, 8);
    let mut field = ExoticEnergyField::from_law(&law, 64, 64, 2026).expect("feasible field");

    // Two overlapping suppressions and one injection, all long-lived so they stay active.
    let mk = |id: u32, kind: ExoticInterventionKind, amount: f32| ExoticIntervention {
        id,
        cause_id: 900 + id,
        kind,
        region: Region::Global,
        start_tick: 60,
        duration_ticks: 600_000,
        amount,
        curve: Curve::Step,
    };
    let sup_a = mk(1, ExoticInterventionKind::RemoveSource, 0.02);
    let sup_b = mk(2, ExoticInterventionKind::RemoveSource, 0.02);
    let add = mk(3, ExoticInterventionKind::AddSource, 0.01);

    // Warm up outside the tracked window.
    for tick in [60u64, 120, 180, 240, 300] {
        field.begin_tick_suppression();
        field.apply_forcing(&add, tick);
        field.add_source_suppression(&sup_a, tick);
        field.add_source_suppression(&sup_b, tick);
        field.step();
    }

    ALLOCATOR.start_tracking();

    let mut tick = 360u64;
    for _ in 0..500 {
        field.begin_tick_suppression();
        field.apply_forcing(&add, tick);
        field.add_source_suppression(&sup_a, tick);
        field.add_source_suppression(&sup_b, tick);
        field.step();
        tick += 60;
    }

    let alloc_count = ALLOCATOR.stop_tracking();
    assert_eq!(
        alloc_count, 0,
        "exotic forcing hot loop triggered {alloc_count} heap allocations!"
    );
}

/// AE3 (AE-305/306): the per-ecology-firing organism physiology — sense/uptake, pay cost and spend,
/// derive performance — must allocate nothing either, since it runs on every ecology tick alongside
/// the field update.
///
/// Scope note (deliberate): this covers [`ReferencePopulation::step_physiology`] only. The
/// generation boundary (`reproduce`) clones genotype source-id `String`s and therefore *does*
/// allocate — it runs once per generation, not per tick, so it is not a hot path and is excluded
/// here rather than silently counted as zero.
#[test]
fn test_ae3_population_physiology_hotloop_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let law = ExoticEnergyLaw::mana_patchy(300.0, 8);
    let mut field = ExoticEnergyField::from_law(&law, 64, 64, 2026).expect("feasible field");

    // Build the population (allocates genotype strings) BEFORE tracking.
    let config = ReferencePopulationConfig {
        total: 100.0,
        capacity: 100.0,
        pathway_fraction: 0.5,
        generation_ticks: 600,
        base_performance: 1.0,
        mutation_rate: 0.0,
        genotype: EnergyPathwayGenotype::new(law.id.clone(), 0.8, 0.05, 0.5, 0.6, 0.5, 0.01, 0.2),
    };
    let mut pop = ReferencePopulation::new(&config, 2026).expect("valid population");
    let mut dissipated = 0.0f64;

    // Warm up outside the tracked window.
    for _ in 0..5 {
        pop.step_physiology(Some(&mut field), &mut dissipated);
        field.step();
    }

    ALLOCATOR.start_tracking();

    for _ in 0..500 {
        pop.step_physiology(Some(&mut field), &mut dissipated);
        field.step();
    }

    let alloc_count = ALLOCATOR.stop_tracking();
    assert_eq!(
        alloc_count, 0,
        "AE3 physiology hot loop triggered {alloc_count} heap allocations!"
    );
}
