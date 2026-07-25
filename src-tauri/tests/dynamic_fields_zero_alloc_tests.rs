//! S17 (M3): the dynamic-field update hot loop must not allocate. Mirrors the terrain zero-alloc
//! test — a process-global tracking allocator counts heap allocations across a batch of field ticks
//! and asserts zero.

mod common;

use anima_engine_lib::core::dynamic_fields::DynamicFields;
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::terrain::{MapSettings, TerrainMap};
use std::sync::Mutex;

fn cmd(id: u32, kind: InterventionKind, signed_negative: bool) -> InterventionCommand {
    InterventionCommand {
        id,
        cause_id: id,
        kind,
        region: Region::Rect {
            min_x: 4,
            min_y: 4,
            max_x: 12,
            max_y: 12,
        },
        start_tick: 0,
        duration_ticks: 100_000,
        intensity: 0.3,
        signed_negative,
        curve: Curve::Step,
        reversible: true,
    }
}

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_dynamic_fields_hotloop_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Build the fields (allocates) BEFORE tracking, from a small deterministic terrain.
    let terrain = TerrainMap::generate(&MapSettings {
        width: 32,
        height: 32,
        ..MapSettings::default()
    });
    let mut fields = DynamicFields::from_terrain(&terrain);

    // Build the active-intervention slice BEFORE tracking so its construction is not counted. These
    // exercise the intervention code paths a real tick uses: apply_region's closure (TemperatureDelta
    // + RainfallDelta in step_climate) and the AddNutrient region loop in step_soil.
    let temp = cmd(1, InterventionKind::TemperatureDelta, false);
    let rain = cmd(2, InterventionKind::RainfallDelta, true);
    let nutrient = cmd(3, InterventionKind::AddNutrient, false);
    let active: [&InterventionCommand; 3] = [&temp, &rain, &nutrient];

    // Warm up so any lazy/one-time allocations are done before we start counting.
    for tick in 1..=5u64 {
        fields.step_climate(tick, 0.0, &active);
        fields.step_water();
        fields.step_soil(tick, &active);
        fields.step_erosion();
    }

    ALLOCATOR.start_tracking();

    // The steady-state per-tick update: climate → water budget → soil → erosion, WITH active
    // interventions, so the region-application paths are measured too. The scratch buffer is reused.
    for tick in 6..=1006u64 {
        fields.step_climate(tick, tick as f32 * 0.001, &active);
        fields.step_water();
        fields.step_soil(tick, &active);
        fields.step_erosion();
    }

    let alloc_count = ALLOCATOR.stop_tracking();
    assert_eq!(
        alloc_count, 0,
        "dynamic-field hot loop triggered {alloc_count} heap allocations!"
    );
}
