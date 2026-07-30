use crate::core::components::*;
use crate::core::resources::*;
use bevy_ecs::prelude::*;
use glam::Vec3;
use smallvec::SmallVec;

pub fn apply_environmental_effects_system(
    active_event: Res<ActiveEnvironmentEvent>,
    forcings: Option<Res<crate::core::live_experiment::LiveForcings>>,
    mut food_settings: ResMut<FoodSpawnSettings>,
    mut agent_query: Query<&mut crate::ai::hrrl::HomeostaticState, With<Agent>>,
) {
    let (max_food_multiplier, target_temp_shift) = match active_event.0 {
        crate::evolution::meta_ai::EnvironmentalEvent::ResourceDrought => (0.5, 0.0),
        crate::evolution::meta_ai::EnvironmentalEvent::TemperatureSpike => (1.0, 5.0),
        crate::evolution::meta_ai::EnvironmentalEvent::GlacialPeriod => (1.0, -5.0),
        crate::evolution::meta_ai::EnvironmentalEvent::ToxicDeluge => (0.8, 0.0),
        crate::evolution::meta_ai::EnvironmentalEvent::Stable => (1.0, 0.0),
    };

    food_settings.max_food_count = (50.0 * max_food_multiplier) as usize;

    // A declared temperature intervention shifts the same homeostatic target the engine's own
    // environmental events shift, rather than writing body temperature directly — see
    // `core::live_experiment`. Absent the resource the shift is 0.0 and this is the old line.
    let declared_shift = forcings.map(|f| f.temp_target_shift_c).unwrap_or(0.0);
    for mut homeo in agent_query.iter_mut() {
        homeo.temp_target = 37.0 + target_temp_shift + declared_shift;
    }
}

pub fn update_positions_system(
    mut query: Query<(&mut Position, &Velocity)>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    for (mut pos, vel) in query.iter_mut() {
        pos.0 += vel.0 * time_step.0;
    }
}

pub fn wrap_coordinates_system(mut query: Query<&mut Position>, bounds: Res<MapBounds>) {
    let x_min = bounds.min.x;
    let x_max = bounds.max.x;
    let x_range = x_max - x_min;

    let z_min = bounds.min.z;
    let z_max = bounds.max.z;
    let z_range = z_max - z_min;

    for mut pos in query.iter_mut() {
        if x_range > 0.0 {
            pos.0.x = x_min + (pos.0.x - x_min).rem_euclid(x_range);
        }
        if z_range > 0.0 {
            pos.0.z = z_min + (pos.0.z - z_min).rem_euclid(z_range);
        }
    }
}

pub fn energy_decay_system(
    mut query: Query<&mut crate::ai::hrrl::HomeostaticState>,
    time_step: Res<crate::ai::cpg::TimeStep>,
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
) {
    let decay = 0.5 * time_step.0;
    // Recycled into detritus exactly as `metabolic_decay_system` does, so this stays a transfer
    // rather than an EU sink if a schedule ever runs it beside the closed ledger. Subtracting two
    // nearby f32s is exact, so detritus is credited with precisely what the reserve lost.
    let mut respired = 0.0f64;
    for mut homeo in query.iter_mut() {
        let before = homeo.energy;
        homeo.energy = (homeo.energy - decay).max(0.0);
        respired += (before - homeo.energy) as f64;
    }
    if let Some(ref mut pool) = biomass {
        pool.detritus += respired;
    }
}

pub fn metabolic_decay_system(
    mut agent_query: Query<(
        Entity,
        &mut crate::ai::hrrl::HomeostaticState,
        Option<&mut FeatureTracker>,
        Option<&Velocity>,
        Option<&Predator>,
        Option<&crate::core::components::AgentBrain>,
    )>,
    segment_query: Query<(
        &ParentAgent,
        &crate::physics::dynamics::RigidBody,
        &Velocity,
        Option<&SegmentJointForce>,
    )>,
    time_step: Res<crate::ai::cpg::TimeStep>,
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    brain_policy: Option<Res<crate::core::resources::BrainPolicy>>,
    counter_fault: Option<Res<ScientificCounterFault>>,
) {
    let brain_cost_per_1k = brain_policy
        .map(|p| p.brain_metabolic_cost)
        .unwrap_or_default();
    let dt = time_step.0;
    // Energy burned by metabolism this tick is recycled into the closed detritus pool
    // (conservation) rather than vanishing.
    let mut respired = 0.0f64;

    let k_velocity = 0.2;
    let k_force = 0.3;

    for (agent_entity, mut homeo, opt_tracker, velocity, opt_predator, opt_brain) in
        agent_query.iter_mut()
    {
        // Split the cost into (1) MAINTENANCE — governed by the Metabolic Theory of Ecology,
        // I = i0·M^(3/4)·e^(−E/kT), so bigger bodies cost less per gram and warmth speeds the
        // burn — and (2) ACTIVITY — the linear locomotion cost of moving mass and firing
        // joints. Predators carry a higher baseline (hunting is costly).
        let k_base = if opt_predator.is_some() { 0.2 } else { 0.1 };
        let mut body_mass = 0.0f32;
        let mut activity_cost = 0.0f32;
        for (parent, body, vel, joint_force) in segment_query.iter() {
            if parent.0 == agent_entity {
                body_mass += body.mass;
                let segment_speed = vel.0.length();
                let force_output = joint_force.map(|jf| jf.0).unwrap_or(0.0);
                activity_cost += (k_velocity * segment_speed) + (k_force * force_output);
            }
        }
        let maintenance = crate::core::ecology::metabolic_rate(
            body_mass,
            homeo.temperature,
            crate::core::ecology::E_ANIMAL_EV,
        );
        // (3) COGNITION — neural tissue costs energy to keep running, scaled by brain size. Folded
        // into `total_cost` rather than deducted separately on purpose: everything in `total_cost`
        // flows through `decay` into `respired` and out into the detritus pool below, so the charge
        // is *moved* rather than destroyed and closed energy holds by construction (gate EB-S06).
        // A separate deduction would have been the natural way to write it and would have leaked.
        //
        // `brain_metabolic_cost` is `0.0` unless a run turns it on, so this term vanishes by default.
        let cognition_cost = opt_brain
            .map(|b| b.metabolic_cost(brain_cost_per_1k))
            .unwrap_or(0.0);
        let total_cost = k_base + maintenance + activity_cost + cognition_cost;

        let sweat_rate = if homeo.temperature > homeo.temp_target {
            0.5 * (homeo.temperature - homeo.temp_target)
        } else {
            0.0
        };

        let decay_hyd = (0.05 + 0.15 * total_cost + sweat_rate) * dt;
        homeo.hydration = (homeo.hydration - decay_hyd).max(0.0);

        let h_prod = 0.5 * total_cost;
        let h_diss = 0.1 * (homeo.temperature - homeo.temp_target);
        let h_evap = 0.2 * sweat_rate;
        let delta_temp = (h_prod - h_diss - h_evap) * dt;
        homeo.temperature = (homeo.temperature + delta_temp).clamp(30.0, 45.0);

        let decay = total_cost * dt;
        let before = homeo.energy;
        homeo.energy = (homeo.energy - decay).max(0.0);
        respired += (before - homeo.energy) as f64; // actual energy removed (floored at 0)

        if let Some(mut tracker) = opt_tracker {
            let speed = velocity.map(|v| v.0.length()).unwrap_or(0.0);
            tracker.cumulative_energy_decay += decay;
            tracker.cumulative_distance += speed * dt;
            if let Some(next_tick) = checked_scientific_increment_u32(
                tracker.tick_count,
                "agent feature counter",
                counter_fault.as_deref(),
            ) {
                tracker.tick_count = next_tick;
            }
        }
    }

    if let Some(ref mut pool) = biomass {
        pool.detritus += respired;
    }
}

pub fn spawn_food_system(
    mut commands: Commands,
    food_query: Query<&Food>,
    bounds: Res<MapBounds>,
    settings: Res<FoodSpawnSettings>,
    mut sim_rng: ResMut<crate::core::resources::SimRng>,
) {
    use rand::Rng;
    let current_food_count = food_query.iter().count();
    if current_food_count < settings.max_food_count {
        let to_spawn = settings.max_food_count - current_food_count;
        let rng = sim_rng.rng();
        for _ in 0..to_spawn {
            let x = rng.gen_range(bounds.min.x..bounds.max.x);
            let z = rng.gen_range(bounds.min.z..bounds.max.z);
            commands.spawn((
                Food {
                    energy_value: settings.default_energy,
                    hydration_value: settings.default_hydration,
                },
                Position(glam::Vec3::new(x, 0.0, z)),
                crate::physics::SpatialCollider { radius: 0.5 },
            ));
        }
    }
}

pub fn detect_food_collisions_system(
    mut commands: Commands,
    mut agent_query: Query<
        (
            Entity,
            &Position,
            &mut crate::ai::hrrl::HomeostaticState,
            Option<&crate::core::components::ActionGates>,
        ),
        With<Agent>,
    >,
    segment_query: Query<(&Position, &ParentAgent)>,
    food_query: Query<(Entity, &Position, &Food)>,
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    mut ledger: Option<ResMut<crate::core::energy_ledger::EnergyLedger>>,
) {
    for (agent_entity, agent_pos, mut homeo, gates) in agent_query.iter_mut() {
        // Feeding used to fire on contact alone. The gate defaults open and reads open when absent,
        // so this is a no-op until a brain drives it (ADR-0003 decision 4).
        if !crate::core::components::ActionGates::of(gates).feeds() {
            continue;
        }
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

        for (food_entity, food_pos, food) in food_query.iter() {
            if centroid.distance(food_pos.0) < 1.5 {
                commands.entity(food_entity).despawn();
                // A spawned food item is a *claim* on detritus, not a store: `spawn_food_system`
                // scatters markers, and the energy behind one is only found when something eats
                // it. Before G1.1 this line added `energy_value` to the animal with nothing
                // debited anywhere, which made food a permanent EU source. Hydration is a WU
                // quantity and is not part of the closed energy ledger, so it is untouched.
                let cap = homeo.energy_target;
                match (biomass.as_mut(), ledger.as_mut()) {
                    (Some(pool), Some(ledger)) => {
                        ledger.transfer_into_reserve(
                            pool,
                            crate::core::energy_ledger::Compartment::Detritus,
                            &mut homeo.energy,
                            cap,
                            food.energy_value,
                            crate::core::energy_ledger::EnergyEvent::Feeding,
                        );
                    }
                    // Bare test worlds without the closed ledger keep the original behaviour.
                    _ => {
                        homeo.energy = (homeo.energy + food.energy_value).min(cap);
                    }
                }
                homeo.hydration =
                    (homeo.hydration + food.hydration_value).min(homeo.hydration_target);
                break;
            }
        }
    }
}

pub fn combat_system(
    mut predator_query: Query<
        (
            Entity,
            &Position,
            &mut crate::ai::hrrl::HomeostaticState,
            Option<&crate::core::components::ActionGates>,
        ),
        (With<Agent>, With<Predator>),
    >,
    mut prey_query: Query<
        (Entity, &Position, &mut crate::ai::hrrl::HomeostaticState),
        (With<Agent>, With<Prey>, Without<Predator>),
    >,
    segment_query: Query<(&Position, &ParentAgent)>,
    mut combat_events: Option<ResMut<CombatEvents>>,
    mut biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    // Maps each root agent to its centroid vector and preserves the mutually-exclusive predator/prey
    // classification enforced by the queries above. Reused between ticks to keep the warm path
    // allocation-free.
    mut centroid_index: Local<bevy_ecs::entity::EntityHashMap<(bool, usize)>>,
) {
    if let Some(ref mut events_res) = combat_events {
        events_res.events.clear();
        events_res.predator_centroids.clear();
        events_res.prey_centroids.clear();

        // Centroid telemetry still covers every predator, gated or not — a predator that chooses not
        // to strike is still present in the world and should stay visible to observers.
        for (entity, pos, _, _) in predator_query.iter() {
            events_res
                .predator_centroids
                .push((entity, pos.0, Vec3::ZERO, 0));
        }
        for (entity, pos, _) in prey_query.iter() {
            events_res
                .prey_centroids
                .push((entity, pos.0, Vec3::ZERO, 0));
        }

        let CombatEvents {
            predator_centroids,
            prey_centroids,
            ..
        } = &mut **events_res;
        let centroid_index = &mut *centroid_index;
        centroid_index.clear();
        for (index, entry) in predator_centroids.iter().enumerate() {
            centroid_index.insert(entry.0, (true, index));
        }
        for (index, entry) in prey_centroids.iter().enumerate() {
            // `prey_query` is `Without<Predator>`, so an entity cannot overwrite a predator entry.
            let previous = centroid_index.insert(entry.0, (false, index));
            debug_assert!(previous.is_none());
        }

        for (seg_pos, parent_agent) in segment_query.iter() {
            let Some(&(is_predator, index)) = centroid_index.get(&parent_agent.0) else {
                continue;
            };
            let entry = if is_predator {
                &mut predator_centroids[index]
            } else {
                &mut prey_centroids[index]
            };
            entry.2 += seg_pos.0;
            entry.3 += 1;
        }

        for entry in events_res.predator_centroids.iter_mut() {
            if entry.3 > 0 {
                entry.1 = entry.2 / entry.3 as f32;
            }
        }
        for entry in events_res.prey_centroids.iter_mut() {
            if entry.3 > 0 {
                entry.1 = entry.2 / entry.3 as f32;
            }
        }

        for i in 0..events_res.predator_centroids.len() {
            let (pred_entity, pred_centroid, _, _) = events_res.predator_centroids[i];
            for j in 0..events_res.prey_centroids.len() {
                let (prey_entity, prey_centroid, _, _) = events_res.prey_centroids[j];

                if pred_centroid.distance(prey_centroid) < 1.5 {
                    if let Ok((_, _, mut prey_homeo)) = prey_query.get_mut(prey_entity) {
                        if prey_homeo.energy <= 0.0 {
                            continue;
                        }
                        if let Ok((_, _, mut pred_homeo, pred_gates)) =
                            predator_query.get_mut(pred_entity)
                        {
                            // Striking used to be automatic on proximity. The gate defaults open and
                            // reads open when absent, so this is a no-op until a brain drives it
                            // (ADR-0003 decision 4).
                            if !crate::core::components::ActionGates::of(pred_gates).attacks() {
                                continue;
                            }
                            let needed = (pred_homeo.energy_target - pred_homeo.energy).max(0.0);
                            if needed > 0.0 {
                                // Holling Type III capture + Lindeman assimilation: the predator
                                // gains only a fraction of what it strips from the prey; the rest
                                // (and rare-prey encounters barely register) returns to the closed
                                // biomass pool as detritus — total energy conserved.
                                let captured = crate::core::ecology::predation_capture(
                                    prey_homeo.energy,
                                    needed,
                                );
                                let assimilated =
                                    captured * crate::core::ecology::LINDEMAN_EFFICIENCY;
                                // Both reserves are `f32`, so neither the strip nor the meal is
                                // exact and "detritus gets captured - assimilated" is only true
                                // in real arithmetic. Read back what each reserve actually
                                // changed by and give detritus the difference, so predation
                                // conserves to the bit instead of to three decimal places.
                                let pred_cap = pred_homeo.energy_target;
                                let removed = crate::core::energy_ledger::debit_reserve(
                                    &mut prey_homeo.energy,
                                    captured,
                                );
                                let gained = crate::core::energy_ledger::credit_reserve(
                                    &mut pred_homeo.energy,
                                    assimilated,
                                    pred_cap,
                                );
                                if let Some(ref mut pool) = biomass {
                                    pool.detritus += removed - gained;
                                }
                                if captured > 0.0
                                    && events_res.events.len() < events_res.events.capacity()
                                {
                                    events_res.events.push(CombatEvent {
                                        predator_id: pred_entity.index(),
                                        prey_id: prey_entity.index(),
                                        damage: captured,
                                        energy_transferred: assimilated,
                                    });
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    } else {
        for (pred_entity, pred_pos, mut pred_homeo, pred_gates) in predator_query.iter_mut() {
            // Same gate as the telemetry branch above — a predator must not strike in one code path
            // and hold back in the other just because combat events happen to be unavailable.
            if !crate::core::components::ActionGates::of(pred_gates).attacks() {
                continue;
            }
            let mut pred_sum = glam::Vec3::ZERO;
            let mut pred_count = 0;
            for (seg_pos, parent_agent) in segment_query.iter() {
                if parent_agent.0 == pred_entity {
                    pred_sum += seg_pos.0;
                    pred_count += 1;
                }
            }
            let pred_centroid = if pred_count > 0 {
                pred_sum / pred_count as f32
            } else {
                pred_pos.0
            };

            for (prey_entity, prey_pos, mut prey_homeo) in prey_query.iter_mut() {
                if prey_homeo.energy <= 0.0 {
                    continue;
                }
                let mut prey_sum = glam::Vec3::ZERO;
                let mut prey_count = 0;
                for (seg_pos, parent_agent) in segment_query.iter() {
                    if parent_agent.0 == prey_entity {
                        prey_sum += seg_pos.0;
                        prey_count += 1;
                    }
                }
                let prey_centroid = if prey_count > 0 {
                    prey_sum / prey_count as f32
                } else {
                    prey_pos.0
                };

                if pred_centroid.distance(prey_centroid) < 1.5 {
                    let needed = (pred_homeo.energy_target - pred_homeo.energy).max(0.0);
                    if needed > 0.0 {
                        let captured =
                            crate::core::ecology::predation_capture(prey_homeo.energy, needed);
                        let assimilated = captured * crate::core::ecology::LINDEMAN_EFFICIENCY;
                        // Same measured transfer as the telemetry branch above — a predator must
                        // not conserve energy in one code path and leak it in the other.
                        let pred_cap = pred_homeo.energy_target;
                        let removed = crate::core::energy_ledger::debit_reserve(
                            &mut prey_homeo.energy,
                            captured,
                        );
                        let gained = crate::core::energy_ledger::credit_reserve(
                            &mut pred_homeo.energy,
                            assimilated,
                            pred_cap,
                        );
                        if let Some(ref mut pool) = biomass {
                            pool.detritus += removed - gained;
                        }
                    }
                    break;
                }
            }
        }
    }
}

type AgentTraversal = SmallVec<[Entity; 64]>;

fn despawn_agent_tree(
    commands: &mut Commands,
    children_query: &Query<&ChildrenLinks>,
    root: Entity,
) {
    let mut stack = AgentTraversal::new();
    let mut visited = AgentTraversal::new();
    stack.push(root);

    while let Some(current) = stack.pop() {
        // Production genotypes decode to trees, but ChildrenLinks is a public ECS component and
        // restored/imported worlds can be corrupt. A cycle or shared child must not hang a tick or
        // enqueue the same deferred despawn repeatedly.
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);

        if let Ok(children) = children_query.get(current) {
            stack.extend(children.0.iter().copied());
        }
        commands.entity(current).despawn();
    }
}

pub fn check_migration_boundaries_system(
    mut commands: Commands,
    mut agent_query: Query<
        (
            Entity,
            &mut Position,
            &mut Velocity,
            &crate::ai::hrrl::HomeostaticState,
            &crate::core::agent_systems::AgentGenotype,
            &crate::core::agent_systems::AgentLineageId,
            &crate::core::agent_systems::AgentGeneration,
            Option<&AgentParentLineageIds>,
            Option<&Predator>,
            Option<&crate::core::agent_systems::AgentEvaluation>,
            Option<&FeatureTracker>,
            Option<&crate::ai::hrrl::LastTransitionState>,
            Option<&crate::core::components::AgentBrain>,
        ),
        With<Agent>,
    >,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
    diagnostics: Option<Res<MigrationHandoffDiagnostics>>,
    mut reported_handoff_failures: Local<u8>,
) {
    let sender = match outbound_sender {
        Some(s) => s,
        None => return,
    };

    let config = match sharding.0.read() {
        Ok(c) => c,
        Err(_) => return,
    };

    let x_min = bounds.min.x;
    let x_max = bounds.max.x;
    let x_range = x_max - x_min;

    for (
        entity,
        mut pos,
        mut vel,
        homeo,
        genotype,
        lineage_id,
        generation,
        opt_parents,
        opt_predator,
        opt_eval,
        opt_tracker,
        opt_last_transition,
        opt_brain,
    ) in agent_query.iter_mut()
    {
        if !pos.0.is_finite() || !vel.0.is_finite() {
            if let Some(ref diagnostics) = diagnostics {
                diagnostics.record_invalid_rejection();
            }
            let center_x = x_min + 0.5 * x_range;
            pos.0 = glam::Vec3::new(
                if pos.0.x.is_finite() {
                    pos.0.x
                } else {
                    center_x
                },
                if pos.0.y.is_finite() { pos.0.y } else { 0.0 },
                if pos.0.z.is_finite() { pos.0.z } else { 0.0 },
            );
            if !vel.0.is_finite() {
                vel.0 = glam::Vec3::ZERO;
            }
            if *reported_handoff_failures & 0b100 == 0 {
                eprintln!(
                    "automatic migration boundary repaired non-finite kinematics for agent {}; \
                     retaining it on shard {} (further invalid-payload reports are suppressed)",
                    lineage_id.0, config.local_port
                );
                *reported_handoff_failures |= 0b100;
            }
            continue;
        }
        let x = pos.0.x;
        let mut target_port = None;
        let mut target_x = pos.0.x;

        if x < x_min {
            if let Some(port) = config.left_target_port {
                target_port = Some(port);
                let wrapped = x_min + (x - x_min).rem_euclid(x_range);
                target_x = wrapped.clamp(x_min + 0.01, x_max - 0.01);
            }
        } else if x > x_max {
            if let Some(port) = config.right_target_port {
                target_port = Some(port);
                let wrapped = x_min + (x - x_min).rem_euclid(x_range);
                target_x = wrapped.clamp(x_min + 0.01, x_max - 0.01);
            }
        }

        if let Some(port) = target_port {
            let agent_class = if opt_predator.is_some() {
                AgentClass::Predator
            } else {
                AgentClass::Prey
            };

            let parent_ids = opt_parents.map(|p| p.0.clone()).unwrap_or_default();

            let migration_data = AgentMigrationData {
                genotype: genotype.0.clone(),
                homeostatic_state: homeo.clone(),
                position: glam::Vec3::new(target_x, pos.0.y, pos.0.z),
                velocity: vel.0,
                lineage_id: lineage_id.0.clone(),
                generation: generation.0,
                agent_class,
                parent_ids,
                evaluation: opt_eval.cloned(),
                feature_tracker: opt_tracker.cloned(),
                last_transition_state: opt_last_transition.cloned(),
                source_port: config.local_port,
                brain: opt_brain.cloned(),
            };

            if let Err(reason) = migration_data.validate() {
                if let Some(ref diagnostics) = diagnostics {
                    diagnostics.record_invalid_rejection();
                }
                let width = (x_max - x_min).max(0.0);
                let offset = 1.0_f32.min(0.1 * width);
                // A non-finite vector is contagious in integration/collision. Keep ownership, but
                // restore a finite local state before the corrupt payload can re-enter the tick.
                if !pos.0.is_finite() {
                    pos.0 = glam::Vec3::ZERO;
                }
                if !vel.0.is_finite() {
                    vel.0 = glam::Vec3::ZERO;
                }
                if x < x_min {
                    pos.0.x = x_min + offset;
                    vel.0.x = vel.0.x.abs();
                } else {
                    pos.0.x = x_max - offset;
                    vel.0.x = -vel.0.x.abs();
                }
                if *reported_handoff_failures & 0b100 == 0 {
                    eprintln!(
                        "automatic migration rejected invalid scientific state ({reason}); \
                         retaining the agent on shard {} (further invalid-payload reports are \
                         suppressed)",
                        config.local_port
                    );
                    *reported_handoff_failures |= 0b100;
                }
                continue;
            }

            if let Err(error) = sender.0.try_send(OutboundMigration {
                target_port: port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            }) {
                let (reason, report_bit) = match error {
                    crossbeam_channel::TrySendError::Full(_) => {
                        if let Some(ref diagnostics) = diagnostics {
                            diagnostics.record_full_rejection();
                        }
                        ("outbound queue is full", 0b01)
                    }
                    crossbeam_channel::TrySendError::Disconnected(_) => {
                        if let Some(ref diagnostics) = diagnostics {
                            diagnostics.record_disconnected_rejection();
                        }
                        ("outbound queue is disconnected", 0b10)
                    }
                };
                // A saturated or disconnected worker has not accepted ownership. Keep the complete
                // entity tree authoritative locally and reflect it back into the shard so the hot
                // tick path neither blocks nor rebuilds this allocation-heavy payload forever.
                // Peer-delivery failures after a successful enqueue are handled by the worker's
                // inbound bounce.
                let width = (x_max - x_min).max(0.0);
                let offset = 1.0_f32.min(0.1 * width);
                if x < x_min {
                    pos.0.x = x_min + offset;
                    vel.0.x = vel.0.x.abs();
                } else {
                    pos.0.x = x_max - offset;
                    vel.0.x = -vel.0.x.abs();
                }
                if *reported_handoff_failures & report_bit == 0 {
                    eprintln!(
                        "automatic migration for agent {} rejected because {reason}; \
                         retaining it on shard {} (further {reason} reports are suppressed)",
                        lineage_id.0, config.local_port
                    );
                    *reported_handoff_failures |= report_bit;
                }
                continue;
            }
            if let Some(ref diagnostics) = diagnostics {
                diagnostics.record_queued();
            }

            despawn_agent_tree(&mut commands, &children_query, entity);
        }
    }
}

pub fn manual_migration_system(
    mut commands: Commands,
    trigger: Option<Res<BevyMigrationTrigger>>,
    agent_query: Query<
        (
            Entity,
            &Position,
            &Velocity,
            &crate::ai::hrrl::HomeostaticState,
            &crate::core::agent_systems::AgentGenotype,
            &crate::core::agent_systems::AgentLineageId,
            &crate::core::agent_systems::AgentGeneration,
            Option<&AgentParentLineageIds>,
            Option<&Predator>,
            Option<&crate::core::agent_systems::AgentEvaluation>,
            Option<&FeatureTracker>,
            Option<&crate::ai::hrrl::LastTransitionState>,
            Option<&crate::core::components::AgentBrain>,
        ),
        With<Agent>,
    >,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
    mut sim_rng: ResMut<crate::core::resources::SimRng>,
    diagnostics: Option<Res<MigrationHandoffDiagnostics>>,
    mut reported_handoff_failures: Local<u8>,
) {
    let trigger = match trigger {
        Some(t) => t,
        None => return,
    };
    let sender = match outbound_sender {
        Some(s) => s,
        None => return,
    };
    let config = match sharding.0.read() {
        Ok(c) => c,
        Err(_) => return,
    };

    for _ in 0..crate::core::resources::MANUAL_MIGRATIONS_PER_TICK {
        let Ok(target_port) = trigger.0.try_recv() else {
            break;
        };
        use rand::seq::IteratorRandom;
        let rng = sim_rng.rng();
        if let Some((
            entity,
            pos,
            vel,
            homeo,
            genotype,
            lineage_id,
            generation,
            opt_parents,
            opt_predator,
            opt_eval,
            opt_tracker,
            opt_last_transition,
            opt_brain,
        )) = agent_query.iter().choose(&mut *rng)
        {
            let x_min = bounds.min.x;
            let x_max = bounds.max.x;

            let agent_class = if opt_predator.is_some() {
                AgentClass::Predator
            } else {
                AgentClass::Prey
            };

            let parent_ids = opt_parents.map(|p| p.0.clone()).unwrap_or_default();

            let migration_data = AgentMigrationData {
                genotype: genotype.0.clone(),
                homeostatic_state: homeo.clone(),
                position: pos.0,
                velocity: vel.0,
                lineage_id: lineage_id.0.clone(),
                generation: generation.0,
                agent_class,
                parent_ids,
                evaluation: opt_eval.cloned(),
                feature_tracker: opt_tracker.cloned(),
                last_transition_state: opt_last_transition.cloned(),
                source_port: config.local_port,
                brain: opt_brain.cloned(),
            };

            if let Err(reason) = migration_data.validate() {
                if let Some(ref diagnostics) = diagnostics {
                    diagnostics.record_invalid_rejection();
                }
                if *reported_handoff_failures & 0b100 == 0 {
                    eprintln!(
                        "manual migration to shard {target_port} rejected invalid scientific state \
                         ({reason}); retaining the agent on shard {} (further invalid-payload \
                         reports are suppressed)",
                        config.local_port
                    );
                    *reported_handoff_failures |= 0b100;
                }
                continue;
            }

            if let Err(error) = sender.0.try_send(OutboundMigration {
                target_port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            }) {
                let (reason, report_bit) = match error {
                    crossbeam_channel::TrySendError::Full(_) => {
                        if let Some(ref diagnostics) = diagnostics {
                            diagnostics.record_full_rejection();
                        }
                        ("outbound queue is full", 0b01)
                    }
                    crossbeam_channel::TrySendError::Disconnected(_) => {
                        if let Some(ref diagnostics) = diagnostics {
                            diagnostics.record_disconnected_rejection();
                        }
                        ("outbound queue is disconnected", 0b10)
                    }
                };
                // Preserve local ownership and surface the failed one-shot request. The observer
                // trace already records that the human requested this migration; this diagnostic
                // distinguishes a rejected handoff from a request that was never made.
                if *reported_handoff_failures & report_bit == 0 {
                    eprintln!(
                        "manual migration for agent {} to shard {target_port} rejected because \
                         {reason}; retaining it on shard {} (further {reason} reports are \
                         suppressed)",
                        lineage_id.0, config.local_port
                    );
                    *reported_handoff_failures |= report_bit;
                }
                continue;
            }
            if let Some(ref diagnostics) = diagnostics {
                diagnostics.record_queued();
            }

            despawn_agent_tree(&mut commands, &children_query, entity);
        }
    }
}

pub struct SpawnMigrationCommand {
    pub data: AgentMigrationData,
}

impl bevy_ecs::system::Command for SpawnMigrationCommand {
    fn apply(self, world: &mut World) {
        use crate::core::agent_systems::{
            AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
        };
        use crate::evolution::genotype::decode_genotype;
        use crate::physics::dynamics::RigidBody;

        if let Err(reason) = self.data.validate() {
            if let Some(diagnostics) = world.get_resource::<MigrationHandoffDiagnostics>() {
                diagnostics.record_invalid_rejection();
            }
            eprintln!("inbound migration rejected invalid scientific state ({reason})");
            return;
        }

        let initial_pos = self.data.position;
        let initial_rot = glam::Quat::IDENTITY;

        let root_entity = decode_genotype(world, &self.data.genotype, initial_pos, initial_rot);

        let eval = self.data.evaluation.unwrap_or(AgentEvaluation {
            start_position: initial_pos,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks: 0,
            last_position: initial_pos,
        });

        let tracker = self.data.feature_tracker.unwrap_or_default();

        world.entity_mut(root_entity).insert((
            AgentGenotype(self.data.genotype.clone()),
            eval,
            tracker,
            AgentLineageId(self.data.lineage_id.clone()),
            AgentGeneration(self.data.generation),
            AgentParentLineageIds(self.data.parent_ids.clone()),
        ));

        if let Some(mut lts) = self.data.last_transition_state {
            // `decode_genotype` intentionally reconstructs the controller and cognitive state from
            // their defaults. An in-flight inference ticket also belongs to the source shard's
            // worker and cannot follow this entity. Preserve the last finite values for diagnostics,
            // but never train on a transition that crosses this control discontinuity.
            lts.has_last = false;
            lts.pending_state = None;
            world.entity_mut(root_entity).insert(lts);
        }

        // Migration moves the same creature to another shard (invariant D01: it is not a birth), so
        // the brain travels with it rather than being rolled afresh on arrival. A `None` is a legacy
        // agent that keeps running on the shared model.
        if let Some(brain) = self.data.brain {
            match brain.validate() {
                Ok(()) => {
                    world.entity_mut(root_entity).insert(brain);
                }
                Err(e) => {
                    eprintln!(
                        "migrating agent {} carried an unreadable brain ({e}); \
                         it arrives on the shared model",
                        self.data.lineage_id
                    );
                }
            }
        }

        match self.data.agent_class {
            AgentClass::Predator => {
                world.entity_mut(root_entity).insert(Predator);
            }
            AgentClass::Prey => {
                world.entity_mut(root_entity).insert(Prey);
            }
        }

        if let Some(mut homeo) = world.get_mut::<crate::ai::hrrl::HomeostaticState>(root_entity) {
            *homeo = self.data.homeostatic_state;
        }

        let velocity = self.data.velocity;
        let mut stack = AgentTraversal::new();
        let mut visited = AgentTraversal::new();
        stack.push(root_entity);
        while let Some(current) = stack.pop() {
            // Decode currently builds a tree; keep the inbound boundary total even if a future
            // importer or restore path hands it a shared child or cycle.
            if visited.contains(&current) {
                continue;
            }
            visited.push(current);

            if let Some(mut vel) = world.get_mut::<Velocity>(current) {
                vel.0 = velocity;
            }
            if let Some(mut body) = world.get_mut::<RigidBody>(current) {
                body.velocity = velocity;
            }

            if let Some(children) = world.get::<ChildrenLinks>(current) {
                stack.extend(children.0.iter().copied());
            }
        }
    }
}

pub fn process_inbound_migrations_system(
    mut commands: Commands,
    inbound_receiver: Option<Res<InboundMigrationReceiver>>,
    diagnostics: Option<Res<MigrationHandoffDiagnostics>>,
    mut reported_invalid: Local<bool>,
) {
    let receiver = match inbound_receiver {
        Some(r) => r,
        None => return,
    };

    for _ in 0..crate::core::resources::INBOUND_MIGRATIONS_PER_TICK {
        let Ok(data) = receiver.0.try_recv() else {
            break;
        };
        if let Err(reason) = data.validate() {
            if let Some(ref diagnostics) = diagnostics {
                diagnostics.record_invalid_rejection();
            }
            if !*reported_invalid {
                eprintln!(
                    "inbound migration rejected invalid scientific state ({reason}) \
                     (further reports are suppressed)"
                );
                *reported_invalid = true;
            }
            continue;
        }
        commands.add(SpawnMigrationCommand { data });
    }
}
