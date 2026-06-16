use bevy_ecs::prelude::*;
use glam::Vec3;
use crate::core::components::*;
use crate::core::resources::*;

pub fn apply_environmental_effects_system(
    active_event: Res<ActiveEnvironmentEvent>,
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

    for mut homeo in agent_query.iter_mut() {
        homeo.temp_target = 37.0 + target_temp_shift;
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
) {
    let decay = 0.5 * time_step.0;
    for mut homeo in query.iter_mut() {
        homeo.energy = (homeo.energy - decay).max(0.0);
    }
}

pub fn metabolic_decay_system(
    mut agent_query: Query<(Entity, &mut crate::ai::hrrl::HomeostaticState, Option<&mut FeatureTracker>, Option<&Velocity>, Option<&Predator>)>,
    segment_query: Query<(&ParentAgent, &crate::physics::dynamics::RigidBody, &Velocity, Option<&SegmentJointForce>)>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;

    let k_mass = 0.05;
    let k_velocity = 0.2;
    let k_force = 0.3;

    for (agent_entity, mut homeo, opt_tracker, velocity, opt_predator) in agent_query.iter_mut() {
        let k_base = if opt_predator.is_some() { 0.2 } else { 0.1 };
        let mut total_cost = k_base;
        for (parent, body, vel, joint_force) in segment_query.iter() {
            if parent.0 == agent_entity {
                let segment_mass = body.mass;
                let segment_speed = vel.0.length();
                let force_output = joint_force.map(|jf| jf.0).unwrap_or(0.0);

                let segment_cost = (k_mass * segment_mass)
                    + (k_velocity * segment_speed)
                    + (k_force * force_output);

                total_cost += segment_cost;
            }
        }

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
        homeo.energy = (homeo.energy - decay).max(0.0);

        if let Some(mut tracker) = opt_tracker {
            let speed = velocity.map(|v| v.0.length()).unwrap_or(0.0);
            tracker.cumulative_energy_decay += decay;
            tracker.cumulative_distance += speed * dt;
            tracker.tick_count += 1;
        }
    }
}

pub fn spawn_food_system(
    mut commands: Commands,
    food_query: Query<&Food>,
    bounds: Res<MapBounds>,
    settings: Res<FoodSpawnSettings>,
) {
    use rand::Rng;
    let current_food_count = food_query.iter().count();
    if current_food_count < settings.max_food_count {
        let to_spawn = settings.max_food_count - current_food_count;
        let mut rng = rand::thread_rng();
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
    mut agent_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), With<Agent>>,
    segment_query: Query<(&Position, &ParentAgent)>,
    food_query: Query<(Entity, &Position, &Food)>,
) {
    for (agent_entity, agent_pos, mut homeo) in agent_query.iter_mut() {
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
                homeo.energy = (homeo.energy + food.energy_value).min(homeo.energy_target);
                homeo.hydration = (homeo.hydration + food.hydration_value).min(homeo.hydration_target);
                break;
            }
        }
    }
}

pub fn combat_system(
    mut predator_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), (With<Agent>, With<Predator>)>,
    mut prey_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), (With<Agent>, With<Prey>, Without<Predator>)>,
    segment_query: Query<(&Position, &ParentAgent)>,
    mut combat_events: Option<ResMut<CombatEvents>>,
) {
    if let Some(ref mut events_res) = combat_events {
        events_res.events.clear();
        events_res.predator_centroids.clear();
        events_res.prey_centroids.clear();

        for (entity, pos, _) in predator_query.iter() {
            events_res.predator_centroids.push((entity, pos.0, Vec3::ZERO, 0));
        }
        for (entity, pos, _) in prey_query.iter() {
            events_res.prey_centroids.push((entity, pos.0, Vec3::ZERO, 0));
        }

        for (seg_pos, parent_agent) in segment_query.iter() {
            if let Some(entry) = events_res.predator_centroids.iter_mut().find(|e| e.0 == parent_agent.0) {
                entry.2 += seg_pos.0;
                entry.3 += 1;
            } else if let Some(entry) = events_res.prey_centroids.iter_mut().find(|e| e.0 == parent_agent.0) {
                entry.2 += seg_pos.0;
                entry.3 += 1;
            }
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
                        if let Ok((_, _, mut pred_homeo)) = predator_query.get_mut(pred_entity) {
                            let needed = (pred_homeo.energy_target - pred_homeo.energy).max(0.0);
                            if needed > 0.0 {
                                let transfer = needed.min(prey_homeo.energy);
                                prey_homeo.energy = (prey_homeo.energy - transfer).max(0.0);
                                pred_homeo.energy = (pred_homeo.energy + transfer).min(pred_homeo.energy_target);
                                if transfer > 0.0 {
                                    if events_res.events.len() < events_res.events.capacity() {
                                        events_res.events.push(CombatEvent {
                                            predator_id: pred_entity.index(),
                                            prey_id: prey_entity.index(),
                                            damage: transfer,
                                            energy_transferred: transfer,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    } else {
        for (pred_entity, pred_pos, mut pred_homeo) in predator_query.iter_mut() {
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
                        let transfer = needed.min(prey_homeo.energy);
                        prey_homeo.energy = (prey_homeo.energy - transfer).max(0.0);
                        pred_homeo.energy = (pred_homeo.energy + transfer).min(pred_homeo.energy_target);
                    }
                    break;
                }
            }
        }
    }
}

pub fn check_migration_boundaries_system(
    mut commands: Commands,
    agent_query: Query<(
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
    ), With<Agent>>,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
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

    for (entity, pos, vel, homeo, genotype, lineage_id, generation, opt_parents, opt_predator, opt_eval, opt_tracker, opt_last_transition) in agent_query.iter() {
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
            };

            let _ = sender.0.send(OutboundMigration {
                target_port: port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            });

            let mut stack = [entity; 64];
            let mut stack_len = 1;
            while stack_len > 0 {
                stack_len -= 1;
                let current = stack[stack_len];
                commands.entity(current).despawn();
                if let Ok(children) = children_query.get(current) {
                    for &child in &children.0 {
                        if stack_len < 64 {
                            stack[stack_len] = child;
                            stack_len += 1;
                        }
                    }
                }
            }
        }
    }
}

pub fn manual_migration_system(
    mut commands: Commands,
    trigger: Option<Res<BevyMigrationTrigger>>,
    agent_query: Query<(
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
    ), With<Agent>>,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
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

    while let Ok(target_port) = trigger.0.try_recv() {
        use rand::seq::IteratorRandom;
        let mut rng = rand::thread_rng();
        if let Some((entity, pos, vel, homeo, genotype, lineage_id, generation, opt_parents, opt_predator, opt_eval, opt_tracker, opt_last_transition)) = agent_query.iter().choose(&mut rng) {
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
            };

            let _ = sender.0.send(OutboundMigration {
                target_port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            });

            let mut stack = [entity; 64];
            let mut stack_len = 1;
            while stack_len > 0 {
                stack_len -= 1;
                let current = stack[stack_len];
                commands.entity(current).despawn();
                if let Ok(children) = children_query.get(current) {
                    for &child in &children.0 {
                        if stack_len < 64 {
                            stack[stack_len] = child;
                            stack_len += 1;
                        }
                    }
                }
            }
        }
    }
}

pub struct SpawnMigrationCommand {
    pub data: AgentMigrationData,
}

impl bevy_ecs::system::Command for SpawnMigrationCommand {
    fn apply(self, world: &mut World) {
        use crate::physics::dynamics::RigidBody;
        use crate::evolution::genotype::decode_genotype;
        use crate::core::agent_systems::{AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration};

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

        if let Some(lts) = self.data.last_transition_state {
            world.entity_mut(root_entity).insert(lts);
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
        let mut stack = [root_entity; 64];
        let mut stack_len = 1;
        while stack_len > 0 {
            stack_len -= 1;
            let current = stack[stack_len];
            
            if let Some(mut vel) = world.get_mut::<Velocity>(current) {
                vel.0 = velocity;
            }
            if let Some(mut body) = world.get_mut::<RigidBody>(current) {
                body.velocity = velocity;
            }
            
            if let Some(children) = world.get::<ChildrenLinks>(current) {
                for &child in &children.0 {
                    if stack_len < 64 {
                        stack[stack_len] = child;
                        stack_len += 1;
                    }
                }
            }
        }
    }
}

pub fn process_inbound_migrations_system(
    mut commands: Commands,
    inbound_receiver: Option<Res<InboundMigrationReceiver>>,
) {
    let receiver = match inbound_receiver {
        Some(r) => r,
        None => return,
    };

    while let Ok(data) = receiver.0.try_recv() {
        commands.add(SpawnMigrationCommand { data });
    }
}
