use anima_engine_lib::ai::cpg::{update_cpg_system, CpgOscillator};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{init_world, update_positions_system, Position, Velocity};
use bevy_ecs::prelude::*;
use glam::Vec3;

#[test]
fn test_ecs_world_initialization_and_tick() {
    let mut world = init_world();

    // Spawn 1 agent cho môi trường thử nghiệm
    let entity = world
        .spawn((
            Position(Vec3::ZERO),
            Velocity(Vec3::new(1.0, 0.0, 0.0)),
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            CpgOscillator::new(1.0, 0.5),
        ))
        .id();

    let mut schedule = Schedule::default();
    schedule.add_systems((update_positions_system, update_cpg_system));

    // Chạy thử 1 tick (mô phỏng delta_time 1/60 giây)
    schedule.run(&mut world);

    // Kiểm tra vị trí đã di chuyển đúng vận tốc (X = 1/60)
    let pos = world.get::<Position>(entity).unwrap();
    assert!((pos.0.x - (1.0 / 60.0)).abs() < 1e-5);

    // Kiểm tra đầu ra của bộ phát dao động CPG đã thay đổi
    let cpg = world.get::<CpgOscillator>(entity).unwrap();
    assert_ne!(cpg.output, 0.0);
}
