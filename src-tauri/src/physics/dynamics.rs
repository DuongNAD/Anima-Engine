use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use crate::core::ecs::{Position, Rotation, Velocity, Segment, JointAxis, ParentAgent, SegmentJointForce, Prey};
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
                let target_rel_rot = Quat::from_axis_angle(axis_val.normalize(), cpg.output);
                let r_target = parent_rot.0 * target_rel_rot;
                child_rot.0 = child_rot.0.slerp(r_target, (constraint.stiffness * dt).clamp(0.0, 1.0));
            }

            // Calculate joint positions in global space
            let p_joint_parent = parent_pos.0 + parent_rot.0 * constraint.anchor_offset;
            let p_joint_child = child_pos.0 - child_rot.0 * Vec3::new(0.0, 0.0, child_segment.length / 2.0);

            // Calculate displacement error
            let e_joint = p_joint_parent - p_joint_child;

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
        Option<&mut crate::core::ecs::InertiaComponent>,
        Option<&mut crate::core::ecs::CognitiveState>,
    )>,
    agent_query: Query<&crate::ai::hrrl::HomeostaticState>,
    prey_query: Query<&Prey>,
    time_step: Res<TimeStep>,
    segment_query: Query<(Entity, &ParentAgent, &Segment)>,
    mut oscillator_query: Query<&mut CpgOscillator>,
    mut child_buf: Local<Vec<(u32, Entity)>>,
) {
    let dt = time_step.0;
    for (entity, mut body, mut pos, mut vel, parent_agent, mut opt_inertia, mut opt_cog) in query.iter_mut() {
        if let Some(ref mut inertia) = opt_inertia {
            // Apply forces towards target_velocity if desired
            if inertia.target_velocity.length_squared() > 1e-6 {
                let force_to_target = (inertia.target_velocity - body.velocity) * body.mass * 2.0;
                body.force += force_to_target;
            }

            // Timeout check
            if let Some(ref mut cog_state) = opt_cog {
                if let crate::core::ecs::CognitiveState::PendingInference(_) = **cog_state {
                    inertia.ticks_pending += 1;
                    if inertia.ticks_pending > 5 {
                        **cog_state = crate::core::ecs::CognitiveState::Ready;
                        inertia.ticks_pending = 0;
                        inertia.cpg_parameters = [1.0, 0.0, 1.0, 0.0];

                        // Reset actions to CPG baseline on oscillators
                        crate::core::agent_systems::apply_inertia_to_oscillators(
                            entity,
                            &inertia.cpg_parameters,
                            &segment_query,
                            &mut oscillator_query,
                            &mut child_buf,
                        );
                    }
                }
            }
        }

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
        vel.0 = body.velocity;
        pos.0 += vel.0 * dt;
        body.force = Vec3::ZERO;
    }
}

