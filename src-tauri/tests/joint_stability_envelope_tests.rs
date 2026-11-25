//! Card P10 JOINT-STABILITY-ENVELOPE — dt = 1/60 joint solver stability gate.
//!
//! Verifies that a 2-segment chain spawned through [`decode_genotype`] with both segment masses at
//! the morphology mutation floor (`0.05 kg`, reduced mass `mu = 0.025 > c*dt/2 ~= 0.00833 kg`) stays
//! finite with bounded kinetic energy across 3,600 ticks at live `dt = 1/60`, whereas the negative
//! control with unscaled joint parameters (`k = 10`, `c = 1`) and `s = 0.25` juvenile mass scaling
//! (`0.05 * 0.25^3 = 0.00078125 kg`, `mu ~= 0.000391 < 0.00833 kg`) diverges.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::ecs::Position;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::{integrate_physics_system, resolve_joints_system, RigidBody};
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};

const LIVE_DT: f32 = 1.0 / 60.0;
const MUTATION_MASS_FLOOR_KG: f32 = 0.05;
const JUVENILE_SCALE: f32 = 0.25;
const INITIAL_DISPLACEMENT_M: f32 = 0.05;
const SIM_TICKS: usize = 3_600;

fn two_segment_genotype(segment_mass_kg: f32) -> MorphologyGenotype {
    let mut genotype = MorphologyGenotype::new();
    for id in [0u32, 1u32] {
        genotype.add_node(MorphologyNode {
            id,
            length: 1.0,
            radius: 0.2,
            mass: segment_mass_kg,
        });
    }
    genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5),
        joint_axis: Vec3::new(0.0, 1.0, 0.0),
    });
    genotype
}

struct ChainStats {
    final_tick: usize,
    all_positions_finite: bool,
    all_velocities_finite: bool,
    initial_spring_pe: f64,
    max_ke: f64,
    final_ke: f64,
    diverged_tick: Option<usize>,
}

fn simulate_chain(segment_mass_kg: f32) -> ChainStats {
    let mut world = World::new();
    world.insert_resource(TimeStep(LIVE_DT));

    let genotype = two_segment_genotype(segment_mass_kg);
    let root = decode_genotype(&mut world, &genotype, Vec3::ZERO, Quat::IDENTITY);

    // Perturb the child segment away from exact spring rest equilibrium (`0.05 m` along +Z) so
    // the spring-damper solver is exercised identically across the floor and juvenile cases.
    let mut perturbed = false;
    {
        let mut q = world.query::<(Entity, &mut Position)>();
        for (entity, mut pos) in q.iter_mut(&mut world) {
            if entity != root {
                pos.0.z += INITIAL_DISPLACEMENT_M;
                perturbed = true;
            }
        }
    }
    assert!(
        perturbed,
        "decoded 2-segment genotype must have a child segment"
    );

    let initial_spring_pe = 0.5 * 10.0 * (INITIAL_DISPLACEMENT_M as f64).powi(2);

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    let mut all_positions_finite = true;
    let mut all_velocities_finite = true;
    let mut max_ke = 0.0_f64;
    let mut final_ke = 0.0_f64;
    let mut diverged_tick = None;
    let mut final_tick = 0;

    for tick in 1..=SIM_TICKS {
        schedule.run(&mut world);
        final_tick = tick;

        let mut tick_ke = 0.0_f64;
        let mut q = world.query::<(&Position, &RigidBody)>();
        for (pos, body) in q.iter(&world) {
            if !pos.0.is_finite() {
                all_positions_finite = false;
            }
            if !body.velocity.is_finite() {
                all_velocities_finite = false;
            }
            let v2 = (body.velocity.length_squared()) as f64;
            tick_ke += 0.5 * (body.mass as f64) * v2;
        }
        final_ke = tick_ke;
        if tick_ke.is_finite() && tick_ke > max_ke {
            max_ke = tick_ke;
        }
        if !all_positions_finite
            || !all_velocities_finite
            || !tick_ke.is_finite()
            || tick_ke > initial_spring_pe * 1.0e6
        {
            diverged_tick = Some(tick);
            break;
        }
    }

    ChainStats {
        final_tick,
        all_positions_finite,
        all_velocities_finite,
        initial_spring_pe,
        max_ke,
        final_ke,
        diverged_tick,
    }
}

#[test]
fn mutation_floor_mass_chain_stays_finite_and_bounded_at_live_dt() {
    let stats = simulate_chain(MUTATION_MASS_FLOOR_KG);
    println!(
        "P10_FLOOR mass={} ticks={} finite_pos={} finite_vel={} init_pe={:.6} max_ke={:.6} final_ke={:.6}",
        MUTATION_MASS_FLOOR_KG,
        stats.final_tick,
        stats.all_positions_finite,
        stats.all_velocities_finite,
        stats.initial_spring_pe,
        stats.max_ke,
        stats.final_ke,
    );
    assert_eq!(stats.final_tick, SIM_TICKS);
    assert!(stats.diverged_tick.is_none());
    assert!(
        stats.all_positions_finite,
        "every position must remain finite over {SIM_TICKS} ticks at mass={MUTATION_MASS_FLOOR_KG} kg"
    );
    assert!(stats.all_velocities_finite);
    assert!(stats.max_ke.is_finite());
    assert!(
        stats.max_ke <= stats.initial_spring_pe * 2.0,
        "kinetic energy must stay bounded by initial potential energy order: max_ke={}, init_pe={}",
        stats.max_ke,
        stats.initial_spring_pe
    );
}

#[test]
fn juvenile_scaled_mass_with_unscaled_joint_params_must_diverge() {
    let juvenile_mass = MUTATION_MASS_FLOOR_KG * JUVENILE_SCALE.powi(3);
    let stats = simulate_chain(juvenile_mass);
    println!(
        "P10_NEGCTRL juvenile_mass={} diverged_tick={:?} finite_pos={} finite_vel={} max_ke={:.6e} final_ke={:.6e}",
        juvenile_mass,
        stats.diverged_tick,
        stats.all_positions_finite,
        stats.all_velocities_finite,
        stats.max_ke,
        stats.final_ke,
    );
    assert!(
        stats.diverged_tick.is_some(),
        "negative control at s={JUVENILE_SCALE} (mass={juvenile_mass} kg, k=10, c=1 unscaled) must \
         diverge within {SIM_TICKS} ticks, but remained bounded (final_ke={}, finite_pos={})",
        stats.final_ke,
        stats.all_positions_finite
    );
}
