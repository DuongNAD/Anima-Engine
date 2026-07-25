use crate::core::components::*;
use crate::core::energy_ledger::{Compartment, EnergyEvent, EnergyLedger};
use crate::core::resources::*;
use bevy_ecs::prelude::*;

pub fn fruit_growth_system(
    mut tree_query: Query<(Option<&Position>, &mut Tree)>,
    field: Option<Res<crate::core::ecology::ResourceField>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    let rainforest_cap =
        crate::core::ecology::biome_carrying_capacity(crate::core::terrain::BiomeType::Rainforest);
    for (opt_pos, mut tree) in tree_query.iter_mut() {
        // Fruiting is limited by local net primary productivity: a tree in the rainforest bears
        // fruit far faster than one clinging to desert or rock. Trees without a position (or
        // worlds without the NPP field) fall back to the flat base rate.
        let npp_factor = match (opt_pos, field.as_ref()) {
            (Some(pos), Some(f)) => match f.cell_index(pos.0.x, pos.0.z) {
                Some(i) => 0.3 + 0.7 * (f.r_max[i] / rainforest_cap).clamp(0.0, 1.0),
                None => 1.0,
            },
            _ => 1.0,
        };
        tree.current_fruit =
            (tree.current_fruit + tree.fruit_growth_rate * dt * npp_factor).min(tree.max_fruit);
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
    season: Option<ResMut<crate::core::ecology::SeasonClock>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let Some(mut field) = field else {
        return;
    };
    // Seasonal fertility: summer boosts regrowth, winter suppresses it (defaults to 1.0 if no
    // season clock is present).
    let fertility = match season {
        Some(mut clock) => clock.tick(time_step.0),
        None => 1.0,
    };
    match biomass {
        Some(mut pool) => {
            // Debit detritus by what the field ACTUALLY grew, not by what regrowth intended to
            // apply. `step_regrowth_gated` accumulates its return value from the `f32` deltas it
            // asked for, but `cell += delta` rounds, so the two differ — and over a long run that
            // difference is a one-way drift rather than noise. Measuring the field on both sides
            // keeps plants and detritus exactly complementary.
            let before = field.total_biomass();
            let _intended = field.step_regrowth_gated(time_step.0, fertility, pool.detritus);
            let after = field.total_biomass();
            pool.detritus -= after - before;
            pool.plants = after;
        }
        None => field.step_regrowth(time_step.0, fertility),
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
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
) {
    let Some(mut field) = field else {
        return;
    };
    let max_bite = 8.0 * time_step.0; // per-tick grazing ceiling
                                      // Rounding remainders across every grazing agent this tick, banked once at the end.
    let mut spill = 0.0f64;
    for (pos, mut homeo) in prey_query.iter_mut() {
        let cap = homeo.energy_target;
        let hunger = (cap - homeo.energy).max(0.0);
        if hunger <= 0.0 {
            continue;
        }
        if let Some(i) = field.cell_index(pos.0.x, pos.0.z) {
            let bite = crate::core::ecology::herbivore_intake(field.r[i], hunger, max_bite);
            // Measure BOTH sides. Plants and animal reserves are both `f32`, so neither
            // `cell -= taken` nor `reserve += taken` is exact, and "the cell lost what the animal
            // gained" is false at ULP scale. Over a long run that difference is a trend, not
            // noise — it showed up as a real ~0.3 EU sink in the G1.1 conservation gate. The
            // amount the cell actually lost and the amount the reserve actually gained are both
            // read back, and whatever grazing spilled between them becomes detritus rather than
            // disappearing.
            let cell_before = field.r[i] as f64;
            let _ = field.graze(pos.0.x, pos.0.z, bite);
            let removed = cell_before - field.r[i] as f64;
            let landed =
                crate::core::energy_ledger::credit_reserve(&mut homeo.energy, removed as f32, cap);
            spill += removed - landed;
        }
    }
    if spill != 0.0 {
        if let Some(ref mut pool) = biomass {
            pool.detritus += spill;
        }
    }
}

/// Recompute the living-animal energy compartment of the closed ledger each tick (a simple
/// census over agent reserves), so the dashboard can watch plants / detritus / animals trade
/// energy and verify the system stays conserved. Allocation-free.
pub fn ecosystem_census_system(
    agent_query: Query<&crate::ai::hrrl::HomeostaticState, With<Agent>>,
    biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    ledger: Option<ResMut<EnergyLedger>>,
) {
    if let Some(mut pool) = biomass {
        let mut total = 0.0f64;
        for homeo in agent_query.iter() {
            total += homeo.energy.max(0.0) as f64;
        }
        pool.animals = total;
        // The census is the moment all three authoritative stores are simultaneously current
        // (grazing and regrowth have already run this tick), so it is where the closed-EU
        // invariant is measured. Recording it here makes conservation a property you can read off
        // a running world instead of one you can only compute at the end of a test.
        if let Some(mut ledger) = ledger {
            let closed = crate::core::energy_ledger::closed_total_eu(
                pool.plants,
                pool.animals,
                pool.detritus,
            );
            // Invariant D06: the baseline is locked *after* genesis has initialised plants,
            // animals and detritus. Locking it lazily on the first census — rather than at some
            // chosen point inside the 1600-line engine setup — means the genesis path and the
            // restore path both get a baseline that reflects a fully built world, and there is
            // exactly one place that decides it. `lock_baseline` ignores repeat calls, so a
            // reload cannot re-baseline away a leak that has already happened.
            ledger.lock_baseline(closed);
            ledger.observe(closed);
        }
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
    mut sim_rng: ResMut<crate::core::resources::SimRng>,
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
                let rng = sim_rng.rng();
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
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    mut ledger: Option<ResMut<EnergyLedger>>,
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
                        // Fruit is a *claim* on detritus, not a store of its own: `current_fruit`
                        // says how much an animal is allowed to draw, and the ledger decides how
                        // much the free pool can actually fund. Picking fruit that nothing paid
                        // for is exactly the leak G1.1 exists to close, so the tree's counter is
                        // debited by what the ledger granted rather than by what was asked for.
                        let cap = homeo.energy_target;
                        let taken = match (biomass.as_mut(), ledger.as_mut()) {
                            (Some(pool), Some(ledger)) => ledger.transfer_into_reserve(
                                pool,
                                Compartment::Detritus,
                                &mut homeo.energy,
                                cap,
                                amount,
                                EnergyEvent::Fruiting,
                            ) as f32,
                            // A world with no closed ledger (the unit-test harnesses that build a
                            // bare `World`) keeps the original behaviour instead of silently
                            // refusing to feed.
                            _ => {
                                homeo.energy = (homeo.energy + amount).min(cap);
                                amount
                            }
                        };
                        tree.current_fruit -= taken;
                    }
                    break;
                }
            }
        }
    }
}
