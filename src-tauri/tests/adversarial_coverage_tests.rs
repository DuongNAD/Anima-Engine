mod common;

use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::Mutex;
use std::panic;

use anima_engine_lib::physics::{
    SpatialHashGrid, Ray3D, rebuild_spatial_grid_system, SpatialCollider,
};
use anima_engine_lib::core::ecs::{
    Position, MapBounds, Agent, Predator, Prey,
    combat_system, CombatEvents
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::pheromone::{PheromoneGrid, update_pheromone_grid_system};
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// 1. Test that SpatialHashGrid::raycast panics with division by zero when bounds are very small
#[test]
fn test_spatial_grid_raycast_panic_on_small_bounds() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();

    // MapBounds with a range smaller than 1e-4
    let bounds = MapBounds {
        min: Vec3::new(0.0, 0.0, 0.0),
        max: Vec3::new(1e-5, 10.0, 1e-5),
    };
    world.insert_resource(bounds);

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    let mut system_state: bevy_ecs::system::SystemState<Query<(&Position, &SpatialCollider)>> =
        bevy_ecs::system::SystemState::new(&mut world);
    let query = system_state.get(&world);
    let grid_res = world.get_resource::<SpatialHashGrid>().unwrap();

    let ray = Ray3D {
        origin: Vec3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(1.0, 0.0, 0.0),
    };

    // This should NOT panic due to safe division/modulo checks
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let _ = grid_res.raycast(&ray, 10.0, &bounds, &query);
    }));

    assert!(result.is_ok(), "Expected raycast to not panic under small bounds");
}

/// 2. Test that SpatialHashGrid::new_prepopulated has infinite loop / integer overflow when cell_size is zero or negative.
#[test]
fn test_float_division_by_zero_prepopulated() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 0.0, 10.0),
    };
    
    // Assert that new_prepopulated handles cell_size = 0.0 safely without infinite loop/OOM
    let grid = SpatialHashGrid::new_prepopulated(0.0, &bounds);
    assert_eq!(grid.cell_size, 10.0); // Falls back to default cell size
    assert!(!grid.cells.is_empty(), "Expected successfully prepopulated grid with fallback cell size");
}

/// 3. Test that PheromoneGrid::pos_to_index and sample_bilinear propagate NaN coordinates instead of handling them.
#[test]
fn test_pheromone_grid_nan_propagation() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    let grid = PheromoneGrid::new(0.1, 0.1);

    // Coordinate containing NaN in both axes
    let nan_pos = Vec3::new(f32::NAN, 0.0, f32::NAN);

    // pos_to_index on NaN position:
    // It should return None because the position is invalid.
    let idx = grid.pos_to_index(nan_pos, &bounds);
    assert!(idx.is_none(), "Expected pos_to_index to return None on NaN coordinates");

    // sample_bilinear on NaN position should return 0.0
    let val = grid.sample_bilinear(nan_pos, &bounds);
    assert!(!val.is_nan(), "Expected sample_bilinear to not propagate NaN");
    assert_eq!(val, 0.0);
}

/// 4. Test that rebuild_spatial_grid_system violates the zero-heap allocation requirement
/// when more than 32 entities cluster in the same cell.
#[test]
fn test_spatial_grid_rebuild_allocates_when_clustered() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Spawn 50 entities in the exact same cell (coordinates 0, 0, 0)
    for _ in 0..50 {
        world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            SpatialCollider { radius: 1.0 },
        ));
    }

    // Run system once to warm up Bevy's internal system queries and pre-allocate cell vector capacities
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_spatial_grid_system);
    schedule.run(&mut world);

    // Now track allocations during the rebuild with 50 entities in the same cell
    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let allocs = ALLOCATOR.stop_tracking();

    // Rebuilding with 50 entities in the same cell triggers zero heap allocations because of capacity pre-allocation
    assert_eq!(allocs, 0, "Expected zero heap allocations on rebuild");
}

/// 5. Test that rebuild_spatial_grid_system allocates when an entity is out of bounds/non-prepopulated cell
#[test]
fn test_spatial_grid_rebuild_allocates_on_new_cell() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    // Prepopulate bounds which covers cx in [-1, 1], cy in [-1, 1]
    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Spawn entity way out of bounds (e.g. 500, 0, 500)
    world.spawn((
        Position(Vec3::new(500.0, 0.0, 500.0)),
        SpatialCollider { radius: 1.0 },
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_spatial_grid_system);
    // Run once to warm up Bevy system cache and perform first-time inserts
    schedule.run(&mut world);

    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let allocs = ALLOCATOR.stop_tracking();

    // Since out of bounds coordinates are clamped to prepopulated bounds, no new cells are allocated
    assert_eq!(allocs, 0, "Expected zero allocations on out of bounds insert");
}

/// 6. Test that PheromoneGrid can blow up/overflow under negative dt or unstable parameters
#[test]
fn test_pheromone_grid_instability() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    
    // Negative dt
    world.insert_resource(TimeStep(-0.1));
    
    let mut grid = PheromoneGrid::new(0.5, 0.5);
    grid.values[0] = 10.0;
    world.insert_resource(grid);

    let mut schedule = Schedule::default();
    schedule.add_systems(update_pheromone_grid_system);
    
    schedule.run(&mut world);
    
    let grid_res = world.resource::<PheromoneGrid>();
    // decay_factor = 1.0 - 0.5 * 0.0 = 1.0
    assert!(grid_res.values[0] <= 10.0, "Pheromone concentration should not grow under negative TimeStep");
}

/// 7. Test that combat_system allocates on the heap when event capacity is exceeded
#[test]
fn test_combat_system_allocates_when_capacity_exceeded() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    
    // Initialize CombatEvents with capacity 2 (small) and other fields pre-allocated
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(2),
        predator_centroids: Vec::with_capacity(10),
        prey_centroids: Vec::with_capacity(10),
    });
    
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems(combat_system);
    
    // Spawn 5 predators and 5 preys close to each other
    for i in 0..5 {
        // Predator with energy = 0 and energy_target = 1000
        world.spawn((
            Agent,
            Predator,
            Position(Vec3::new(i as f32 * 0.01, 0.0, 0.0)),
            HomeostaticState {
                energy: 0.0,
                energy_target: 1000.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        ));

        // Prey with energy = 10.0
        world.spawn((
            Agent,
            Prey,
            Position(Vec3::new(i as f32 * 0.01, 0.0, 0.1)),
            HomeostaticState {
                energy: 10.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        ));
    }

    // Warm up queries, etc. with the entities spawned
    schedule.run(&mut world);

    // Reset energy values so combat happens again in the tracked run
    for mut homeo in world.query::<&mut HomeostaticState>().iter_mut(&mut world) {
        if homeo.energy_target == 1000.0 {
            homeo.energy = 0.0;
        } else {
            homeo.energy = 10.0;
        }
    }
    
    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let allocs = ALLOCATOR.stop_tracking();
    
    // We expect zero heap allocations because combat events are clamped to vector capacity
    assert_eq!(allocs, 0, "Expected zero heap allocations when capacity is exceeded");
}
