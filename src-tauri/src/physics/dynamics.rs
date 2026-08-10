use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use crate::core::ecs::{Position, Rotation, Velocity, Segment, JointAxis, ParentAgent, SegmentJointForce, Prey, MapBounds};
use crate::ai::cpg::{CpgOscillator, TimeStep};

#[derive(Component, Clone, Copy, Debug)]
pub struct RigidBody {
    pub mass: f32,
    pub velocity: Vec3,
    pub force: Vec3,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct JointConstraint {
    pub parent_entity: Entity,
    pub anchor_offset: Vec3,
    pub stiffness: f32,
    pub damping: f32,
}

pub fn resolve_joints_system(
    mut components_query: Query<(
        &mut RigidBody,
        &Position,
        &mut Rotation,
        &Velocity,
        &Segment,
    )>,
    mut joint_query: Query<(
        Entity,
        &JointConstraint,
        Option<&JointAxis>,
        Option<&CpgOscillator>,
        Option<&mut SegmentJointForce>,
    )>,
    time_step: Res<TimeStep>,
    bounds: Res<MapBounds>,
) {
    let dt = time_step.0;
    for (child_entity, constraint, opt_axis, opt_cpg, opt_joint_force) in joint_query.iter_mut() {
        let parent_entity = constraint.parent_entity;
        if let Ok([
            (mut child_body, child_pos, mut child_rot, child_vel, child_segment),
            (mut parent_body, parent_pos, parent_rot, parent_vel, _parent_segment),
        ]) = components_query.get_many_mut([child_entity, parent_entity]) {
            // Update child orientation
            if let (Some(axis), Some(cpg)) = (opt_axis, opt_cpg) {
                let axis_val = axis.0;
                let normalized_axis = axis_val.try_normalize().unwrap_or(Vec3::Y);
                let target_rel_rot = Quat::from_axis_angle(normalized_axis, cpg.output);
                let r_target = parent_rot.0 * target_rel_rot;
                
                let mut slerp_factor = constraint.stiffness * dt;
                if slerp_factor.is_nan() {
                    slerp_factor = 0.0;
                } else {
                    slerp_factor = slerp_factor.clamp(0.0, 1.0);
                }
                child_rot.0 = child_rot.0.slerp(r_target, slerp_factor);
            }

            // Calculate joint positions in global space
            let p_joint_parent = parent_pos.0 + parent_rot.0 * constraint.anchor_offset;
            let p_joint_child = child_pos.0 - child_rot.0 * Vec3::new(0.0, 0.0, child_segment.length / 2.0);

            // Calculate displacement error
            let mut e_joint = p_joint_parent - p_joint_child;

            let x_range = bounds.max.x - bounds.min.x;
            if x_range > 0.0 {
                if e_joint.x > x_range * 0.5 { e_joint.x -= x_range; }
                else if e_joint.x < -x_range * 0.5 { e_joint.x += x_range; }
            }
            let z_range = bounds.max.z - bounds.min.z;
            if z_range > 0.0 {
                if e_joint.z > z_range * 0.5 { e_joint.z -= z_range; }
                else if e_joint.z < -z_range * 0.5 { e_joint.z += z_range; }
            }

            // Calculate spring-damper force
            let f_spring = constraint.stiffness * e_joint - constraint.damping * (child_vel.0 - parent_vel.0);

            // Apply forces
            child_body.force += f_spring;
            parent_body.force -= f_spring;

            // Write joint force magnitude
            if let Some(mut jf) = opt_joint_force {
                jf.0 = f_spring.length();
            }
        }
    }
}

pub fn integrate_physics_system(
    mut query: Query<(
        Entity,
        &mut RigidBody,
        &mut Position,
        &mut Velocity,
        Option<&ParentAgent>,
    )>,
    agent_query: Query<&crate::ai::hrrl::HomeostaticState>,
    prey_query: Query<&Prey>,
    time_step: Res<TimeStep>,
    terrain: Option<Res<crate::core::terrain::TerrainGrid>>,
    bounds: Option<Res<MapBounds>>,
) {
    let mut dt = time_step.0;
    if dt.is_nan() {
        dt = 0.0;
    }
    for (entity, mut body, mut pos, mut vel, parent_agent) in query.iter_mut() {
        let is_depleted = if let Some(parent) = parent_agent {
            if let Ok(homeo) = agent_query.get(parent.0) {
                homeo.energy <= 0.0
            } else {
                false
            }
        } else if let Ok(homeo) = agent_query.get(entity) {
            let is_prey = prey_query.get(entity).is_ok();
            is_prey && homeo.energy <= 0.0
        } else {
            false
        };

        if is_depleted {
            body.velocity = Vec3::ZERO;
            body.force = Vec3::ZERO;
            vel.0 = Vec3::ZERO;
            continue;
        }

        if body.mass > 1e-5 {
            let accel = body.force / body.mass;
            body.velocity += accel * dt;
        }

        let mut multiplier = 1.0;
        if let (Some(terrain), Some(bounds)) = (terrain.as_ref(), bounds.as_ref()) {
            if let Some(idx) = terrain.pos_to_index(pos.0, bounds) {
                let cell = terrain.cells[idx];
                
                // Biome Factor
                let biome_factor = match cell.biome {
                    crate::core::terrain::BiomeType::Grassland => 1.0,
                    crate::core::terrain::BiomeType::Rock => 0.95,
                    crate::core::terrain::BiomeType::Sand => 0.85,
                    crate::core::terrain::BiomeType::Mud => 0.65,
                    crate::core::terrain::BiomeType::Water => 1.0,
                };
                multiplier *= biome_factor;

                // Water depth drag
                if cell.water_depth > 0.0 {
                    multiplier *= (1.0 - cell.water_depth * 0.40).max(0.15);
                }

                // Slope penalty
                let step = 1.0;
                let get_elevation = |p: Vec3, terrain: &crate::core::terrain::TerrainGrid, bounds: &MapBounds| -> f32 {
                    if let Some(i) = terrain.pos_to_index(p, bounds) {
                        terrain.cells[i].elevation
                    } else {
                        0.0
                    }
                };

                let h_x1 = get_elevation(pos.0 + Vec3::new(step, 0.0, 0.0), terrain, bounds);
                let h_x0 = get_elevation(pos.0 - Vec3::new(step, 0.0, 0.0), terrain, bounds);
                let h_z1 = get_elevation(pos.0 + Vec3::new(0.0, 0.0, step), terrain, bounds);
                let h_z0 = get_elevation(pos.0 - Vec3::new(0.0, 0.0, step), terrain, bounds);

                let grad_x = (h_x1 - h_x0) / (2.0 * step);
                let grad_z = (h_z1 - h_z0) / (2.0 * step);

                let vel_len = body.velocity.length();
                let slope = if vel_len > 1e-5 {
                    let dir = body.velocity / vel_len;
                    grad_x * dir.x + grad_z * dir.z
                } else {
                    0.0
                };

                let slope_scale = if slope >= 0.0 {
                    (1.0 - slope * 1.0).max(0.2)
                } else {
                    (1.0 - slope * 0.5).min(1.25)
                };
                multiplier *= slope_scale;
            }
        }

        body.velocity *= multiplier;
        vel.0 = body.velocity;
        pos.0 += vel.0 * dt;
        body.force = Vec3::ZERO;
    }
}

