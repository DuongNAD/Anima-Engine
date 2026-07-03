use crate::core::components::*;
use crate::core::resources::*;
use bevy_ecs::prelude::*;

pub fn fruit_growth_system(
    mut tree_query: Query<&mut Tree>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    for mut tree in tree_query.iter_mut() {
        tree.current_fruit = (tree.current_fruit + tree.fruit_growth_rate * dt).min(tree.max_fruit);
    }
}

/// Advance the per-cell NPP resource field one logistic step. When the closed biomass ledger
/// is present the regrowth is GATED by the detritus pool (Bibites rule: plants only grow by
/// drawing free biomass, so energy is conserved and runaway growth is impossible); the
/// consumed biomass is subtracted from detritus and the standing plant total mirrored back.
/// Both resources are `Option` so unit-test worlds without them simply skip. Allocation-free.
pub fn resource_field_regrowth_system(
    field: Option<ResMut<crate::core::ecology::ResourceField>>,
    biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let Some(mut field) = field else {
        return;
    };
    // Global fertility of 1.0 (a seasonal driver could animate this over time).
    match biomass {
        Some(mut pool) => {
            let consumed = field.step_regrowth_gated(time_step.0, 1.0, pool.detritus);
            pool.detritus -= consumed;
            pool.plants = field.total_biomass();
        }
        None => field.step_regrowth(time_step.0, 1.0),
    }
}

/// Herbivores (prey) graze the standing plant resource at their position — the producer →
/// primary-consumer trophic link that closes the bottom of the food web. Intake saturates
/// (Type II) and a grazed-down cell yields little, so herbivores naturally disperse toward
/// ungrazed patches (giving-up density → spatial refuges). Grazed energy leaves the field and
/// enters the animal; the ecosystem census mirrors it into the `animals` compartment.
pub fn herbivore_grazing_system(
    mut prey_query: Query<
        (&Position, &mut crate::ai::hrrl::HomeostaticState),
        (With<Agent>, With<Prey>),
    >,
    field: Option<ResMut<crate::core::ecology::ResourceField>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let Some(mut field) = field else {
        return;
    };
    let max_bite = 8.0 * time_step.0; // per-tick grazing ceiling
    for (pos, mut homeo) in prey_query.iter_mut() {
        let hunger = (homeo.energy_target - homeo.energy).max(0.0);
        if hunger <= 0.0 {
            continue;
        }
        if let Some(i) = field.cell_index(pos.0.x, pos.0.z) {
            let bite = crate::core::ecology::herbivore_intake(field.r[i], hunger, max_bite);
            let taken = field.graze(pos.0.x, pos.0.z, bite);
            homeo.energy = (homeo.energy + taken).min(homeo.energy_target);
        }
    }
}

/// Recompute the living-animal energy compartment of the closed ledger each tick (a simple
/// census over agent reserves), so the dashboard can watch plants / detritus / animals trade
/// energy and verify the system stays conserved. Allocation-free.
pub fn ecosystem_census_system(
    agent_query: Query<&crate::ai::hrrl::HomeostaticState, With<Agent>>,
    biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
) {
    if let Some(mut pool) = biomass {
        let mut total = 0.0f64;
        for homeo in agent_query.iter() {
            total += homeo.energy.max(0.0) as f64;
        }
        pool.animals = total;
    }
}

pub fn lake_replenishment_system(
    mut lake_query: Query<&mut Lake>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    for mut lake in lake_query.iter_mut() {
        lake.current_water =
            (lake.current_water + lake.replenishment_rate * dt).min(lake.max_water);
    }
}

pub fn seed_dropping_system(
    mut commands: Commands,
    mut tree_query: Query<(&Position, &mut Tree)>,
    bounds: Res<MapBounds>,
    time_step: Res<crate::ai::cpg::TimeStep>,
    settings: Res<EnvironmentalSpawnSettings>,
) {
    let dt = time_step.0;
    let tree_count = tree_query.iter().count();
    let mut spawned_this_tick = 0;

    for (pos, mut tree) in tree_query.iter_mut() {
        tree.time_since_last_drop += dt;
        if tree.time_since_last_drop >= tree.seed_drop_cooldown {
            tree.time_since_last_drop = 0.0;

            if tree_count + spawned_this_tick < settings.max_tree_count {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist = rng.gen_range(2.0..tree.seed_spread_radius);
                let spawn_x = (pos.0.x + angle.cos() * dist).clamp(bounds.min.x, bounds.max.x);
                let spawn_z = (pos.0.z + angle.sin() * dist).clamp(bounds.min.z, bounds.max.z);

                commands.spawn((
                    Tree {
                        current_fruit: 0.0,
                        max_fruit: tree.max_fruit,
                        fruit_growth_rate: tree.fruit_growth_rate,
                        time_since_last_drop: 0.0,
                        seed_drop_cooldown: tree.seed_drop_cooldown,
                        seed_spread_radius: tree.seed_spread_radius,
                    },
                    Position(glam::Vec3::new(spawn_x, 0.0, spawn_z)),
                    crate::physics::SpatialCollider { radius: 1.5 },
                ));
                spawned_this_tick += 1;
            }
        }
    }
}

pub fn detect_environmental_collisions_system(
    mut agent_query: Query<
        (
            Entity,
            &Position,
            &mut crate::ai::hrrl::HomeostaticState,
            Option<&Prey>,
        ),
        With<Agent>,
    >,
    segment_query: Query<(&Position, &ParentAgent)>,
    mut lake_query: Query<(&Position, &crate::physics::SpatialCollider, &mut Lake)>,
    mut tree_query: Query<(&Position, &crate::physics::SpatialCollider, &mut Tree)>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    let max_drinking_rate = 15.0; // units/sec
    let max_eating_rate = 15.0; // units/sec

    for (agent_entity, agent_pos, mut homeo, opt_prey) in agent_query.iter_mut() {
        let mut sum_pos = glam::Vec3::ZERO;
        let mut count = 0;
        for (seg_pos, parent_agent) in segment_query.iter() {
            if parent_agent.0 == agent_entity {
                sum_pos += seg_pos.0;
                count += 1;
            }
        }
        let centroid = if count > 0 {
            sum_pos / count as f32
        } else {
            agent_pos.0
        };

        if homeo.hydration < homeo.hydration_target {
            let needed = homeo.hydration_target - homeo.hydration;
            for (lake_pos, collider, mut lake) in lake_query.iter_mut() {
                if centroid.distance(lake_pos.0) < collider.radius {
                    let amount = needed.min(lake.current_water).min(max_drinking_rate * dt);
                    if amount > 0.0 {
                        lake.current_water -= amount;
                        homeo.hydration = (homeo.hydration + amount).min(homeo.hydration_target);
                    }
                    break;
                }
            }
        }

        if opt_prey.is_some() && homeo.energy < homeo.energy_target {
            let needed = homeo.energy_target - homeo.energy;
            for (tree_pos, collider, mut tree) in tree_query.iter_mut() {
                if centroid.distance(tree_pos.0) < collider.radius {
                    let amount = needed.min(tree.current_fruit).min(max_eating_rate * dt);
                    if amount > 0.0 {
                        tree.current_fruit -= amount;
                        homeo.energy = (homeo.energy + amount).min(homeo.energy_target);
                    }
                    break;
                }
            }
        }
    }
}
