//! Off-tick interest-area projection and quantized `bincode` snapshot/delta stream (`ADR-0014` /
//! `QD-SHAREDISLAND`, Card I3).
//!
//! ## Structural invariants
//! 1. **Observer camera isolation:** Guest interest windows (`GuestInterestWindow`) live strictly in
//!    this off-tick projection module and NEVER touch `SharedLodFocus`, `LodFocus`, or host ECS
//!    state. A host with active `Spectate` guests matches a Solo trajectory bit-for-bit.
//! 2. **`bincode` 2.0 compatibility:** All wire frames in [`IslandStreamFrame`] use default
//!    externally tagged enum encoding and full positional struct fields (no `#[serde(tag)]`,
//!    `#[serde(flatten)]`, or `#[serde(skip_serializing_if)]`).
//! 3. **Host state stripping (`QD-PLAYRULES`):** [`IslandJoinSnapshot`] projects only public visual
//!    state, world identity, genesis phase provenance, static environment geometry, and chronicle
//!    headlines. Private `SimRng` cursors, `AgentBrain` weights, evolution archives, and learner
//!    state never cross the wire.
//! 4. **Non-blocking fan-out:** Slow or stalled guests drop/coalesce stale queued frames in their
//!    bounded per-guest mailbox and automatically receive a fresh self-contained [`IslandSnapshot`]
//!    keyframe next turn, never stalling host simulation ticks or healthy guests.

use super::{
    GenesisPhase, GuestId, GuestRole, IslandId, DEFAULT_ISLAND_MAX_GUESTS,
    MAX_ISLAND_GUESTS_CEILING,
};
use crate::ai::pheromone::{PheromoneGridState, CELL_COUNT, MAX_CONCENTRATION};
use crate::core::components::{
    AgentClass, AgentType, CombatEvent, EnvironmentalElement, EnvironmentalState,
};
use crate::core::simulation_state::{ChronicleEvent, SavedSimulationState, SegmentState};
use crate::core::world_artifact::WorldIdentity;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Wire schema version for `IslandJoinSnapshot` / `IslandStreamFrame`.
pub const ISLAND_STREAM_PROTOCOL_VERSION: u16 = 1;

/// Decided off-tick network projection frequency (`ISLAND-D3(a)`: `~20 Hz`).
pub const DEFAULT_ISLAND_STREAM_HZ: u32 = 20;

/// Off-tick interval corresponding to [`DEFAULT_ISLAND_STREAM_HZ`] (`50 ms`).
pub const DEFAULT_ISLAND_STREAM_INTERVAL: Duration = Duration::from_millis(50);

/// Population threshold up to which every guest receives the full island (`ISLAND-D4(b)`).
/// Above this threshold, spatial interest-area culling filters transmitted entities.
pub const WHOLE_ISLAND_BROADCAST_AGENT_LIMIT: usize = 250;

/// Default spatial interest radius around a guest's camera focus when culling is active.
pub const DEFAULT_INTEREST_RADIUS: f32 = 75.0;

/// Bounded per-guest queued frame capacity before slow-consumer drop/coalesce engages.
pub const DEFAULT_GUEST_MAILBOX_CAPACITY: usize = 4;

/// Hard upper bound on one encoded `IslandStreamFrame` (`1 MiB`).
pub const MAX_ISLAND_STREAM_FRAME_BYTES: usize = 1_048_576;

/// Minimum quantized pheromone delta magnitude emitted in steady-state frames.
pub const PHEROMONE_DELTA_THRESHOLD: f32 = 0.04;

const POS_QUANT_SCALE: f32 = 32.0;
const ANGLE_QUANT_SCALE: f32 = 32_767.0 / std::f32::consts::PI;
const DIM_QUANT_SCALE: f32 = 1_000.0;
const VITAL_QUANT_SCALE: f32 = 10.0;
const ENV_COORD_QUANT_SCALE: f32 = 16.0;
const ENV_RESOURCE_QUANT_SCALE: f32 = 10.0;
const COMBAT_QUANT_SCALE: f32 = 100.0;
const RADIANS_INV_QUANT_SCALE: f32 = std::f32::consts::PI / 32_767.0;

fn wrap_angle_pi(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    let tau = std::f32::consts::TAU;
    let wrapped = (angle + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    wrapped.clamp(-std::f32::consts::PI, std::f32::consts::PI)
}

#[inline]
fn quantize_i16(value: f32, scale: f32) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    let scaled = (value * scale).round();
    scaled.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

#[inline]
fn quantize_u16(value: f32, scale: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    let scaled = (value * scale).round();
    scaled.clamp(0.0, f32::from(u16::MAX)) as u16
}

#[inline]
fn quantize_angle(radians: f32) -> i16 {
    quantize_i16(wrap_angle_pi(radians), ANGLE_QUANT_SCALE)
}

#[inline]
fn quantize_pheromone(value: f32) -> u8 {
    if !value.is_finite() || value <= 0.0 || MAX_CONCENTRATION <= 0.0 {
        return 0;
    }
    ((value / MAX_CONCENTRATION).clamp(0.0, 1.0) * 255.0).round() as u8
}

#[inline]
fn encode_agent_type(agent_type: Option<AgentType>) -> u8 {
    match agent_type {
        None => 0,
        Some(AgentType::Predator) => 1,
        Some(AgentType::Prey) => 2,
    }
}

#[inline]
fn decode_agent_type(tag: u8) -> Option<AgentType> {
    match tag {
        1 => Some(AgentType::Predator),
        2 => Some(AgentType::Prey),
        _ => None,
    }
}

/// Environmental landmark category projected onto the guest stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticEnvKind {
    Lake,
    Tree,
    Food,
}

/// Quantized environmental feature (`8 B` wire size vs variable JSON string objects).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticEnvFeature {
    pub kind: StaticEnvKind,
    pub x_q: i16,
    pub z_q: i16,
    pub radius_q: u16,
    pub resources_q: u16,
}

impl StaticEnvFeature {
    pub fn from_environmental_element(el: &EnvironmentalElement) -> Self {
        let kind = match el.element_type.as_str() {
            "lake" => StaticEnvKind::Lake,
            "food" => StaticEnvKind::Food,
            _ => StaticEnvKind::Tree,
        };
        Self {
            kind,
            x_q: quantize_i16(el.x, ENV_COORD_QUANT_SCALE),
            z_q: quantize_i16(el.y, ENV_COORD_QUANT_SCALE),
            radius_q: quantize_u16(el.radius, DIM_QUANT_SCALE),
            resources_q: quantize_u16(el.resources, ENV_RESOURCE_QUANT_SCALE),
        }
    }

    pub fn to_environmental_element(&self) -> EnvironmentalElement {
        let element_type = match self.kind {
            StaticEnvKind::Lake => "lake".to_string(),
            StaticEnvKind::Tree => "tree".to_string(),
            StaticEnvKind::Food => "food".to_string(),
        };
        EnvironmentalElement {
            element_type,
            x: f32::from(self.x_q) / ENV_COORD_QUANT_SCALE,
            y: f32::from(self.z_q) / ENV_COORD_QUANT_SCALE,
            radius: f32::from(self.radius_q) / DIM_QUANT_SCALE,
            resources: f32::from(self.resources_q) / ENV_RESOURCE_QUANT_SCALE,
        }
    }
}

/// Global and viewport census counts accompanying every projected frame.
///
/// Even when spatial culling reduces transmitted segments (`total_agents > 250`), guests still
/// receive exact whole-island predator/prey/population totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IslandCensusSummary {
    pub total_agents: u32,
    pub visible_agents: u32,
    pub total_segments: u32,
    pub visible_segments: u32,
    pub predators: u32,
    pub prey: u32,
    pub interest_culled: bool,
}

/// Compact quantized pose and phenotype dimensions for one body segment (`ISLAND-D3(a)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedSegmentPose {
    pub agent_id: u32,
    pub segment_id: u16,
    /// `u16::MAX` represents `None` (root segment).
    pub parent_segment_id: u16,
    pub x_q: i16,
    pub y_q: i16,
    pub z_q: i16,
    pub yaw_q: i16,
    pub pitch_q: i16,
    pub roll_q: i16,
    pub length_q: u16,
    pub radius_q: u16,
    pub energy_q: u16,
    pub hydration_q: u16,
    pub agent_type_tag: u8,
}

impl QuantizedSegmentPose {
    #[inline]
    pub const fn key(&self) -> (u32, u16) {
        (self.agent_id, self.segment_id)
    }

    pub fn from_segment_state(seg: &SegmentState) -> Self {
        let segment_id = u16::try_from(seg.segment_id).unwrap_or(u16::MAX);
        let parent_segment_id = match seg.parent_segment_id {
            Some(pid) => u16::try_from(pid).unwrap_or(u16::MAX),
            None => u16::MAX,
        };
        Self {
            agent_id: seg.agent_id,
            segment_id,
            parent_segment_id,
            x_q: quantize_i16(seg.x, POS_QUANT_SCALE),
            y_q: quantize_i16(seg.y, POS_QUANT_SCALE),
            z_q: quantize_i16(seg.z, POS_QUANT_SCALE),
            yaw_q: quantize_angle(seg.yaw),
            pitch_q: quantize_angle(seg.pitch),
            roll_q: quantize_angle(seg.roll),
            length_q: quantize_u16(seg.length, DIM_QUANT_SCALE),
            radius_q: quantize_u16(seg.radius, DIM_QUANT_SCALE),
            energy_q: quantize_u16(seg.energy, VITAL_QUANT_SCALE),
            hydration_q: quantize_u16(seg.hydration, VITAL_QUANT_SCALE),
            agent_type_tag: encode_agent_type(seg.agent_type),
        }
    }

    pub fn to_segment_state(&self) -> SegmentState {
        let yaw = f32::from(self.yaw_q) * RADIANS_INV_QUANT_SCALE;
        let pitch = f32::from(self.pitch_q) * RADIANS_INV_QUANT_SCALE;
        let roll = f32::from(self.roll_q) * RADIANS_INV_QUANT_SCALE;
        let head_rot = glam::Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
        let head_direction = (head_rot * glam::Vec3::Z).to_array();
        let parent_segment_id = if self.parent_segment_id == u16::MAX {
            None
        } else {
            Some(u32::from(self.parent_segment_id))
        };
        SegmentState {
            agent_id: self.agent_id,
            segment_id: u32::from(self.segment_id),
            parent_segment_id,
            length: f32::from(self.length_q) / DIM_QUANT_SCALE,
            radius: f32::from(self.radius_q) / DIM_QUANT_SCALE,
            x: f32::from(self.x_q) / POS_QUANT_SCALE,
            y: f32::from(self.y_q) / POS_QUANT_SCALE,
            z: f32::from(self.z_q) / POS_QUANT_SCALE,
            yaw,
            pitch,
            roll,
            joint_anchor_x: 0.0,
            joint_anchor_y: 0.0,
            joint_anchor_z: 0.0,
            joint_axis_x: 0.0,
            joint_axis_y: 0.0,
            joint_axis_z: 0.0,
            energy: f32::from(self.energy_q) / VITAL_QUANT_SCALE,
            hydration: f32::from(self.hydration_q) / VITAL_QUANT_SCALE,
            head_direction,
            agent_type: decode_agent_type(self.agent_type_tag),
        }
    }

    fn apply_delta(&mut self, delta: &SegmentDelta) {
        self.x_q = self.x_q.saturating_add(delta.dx);
        self.y_q = self.y_q.saturating_add(delta.dy);
        self.z_q = self.z_q.saturating_add(delta.dz);
        self.yaw_q = self.yaw_q.saturating_add(delta.dyaw);
        self.pitch_q = self.pitch_q.saturating_add(delta.dpitch);
        self.roll_q = self.roll_q.saturating_add(delta.droll);
        self.energy_q = add_signed_u16(self.energy_q, delta.denergy);
        self.hydration_q = add_signed_u16(self.hydration_q, delta.dhydration);
    }
}

#[inline]
fn add_signed_u16(base: u16, delta: i16) -> u16 {
    let sum = i32::from(base) + i32::from(delta);
    sum.clamp(0, i32::from(u16::MAX)) as u16
}

/// Compact per-segment motion & vital delta (`22 B` in `bincode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentDelta {
    pub agent_id: u32,
    pub segment_id: u16,
    pub dx: i16,
    pub dy: i16,
    pub dz: i16,
    pub dyaw: i16,
    pub dpitch: i16,
    pub droll: i16,
    pub denergy: i16,
    pub dhydration: i16,
}

enum DeltaClassify {
    Unchanged,
    Delta(SegmentDelta),
    Replace,
}

fn classify_segment_delta(
    prev: &QuantizedSegmentPose,
    next: &QuantizedSegmentPose,
) -> DeltaClassify {
    if prev.parent_segment_id != next.parent_segment_id
        || prev.length_q != next.length_q
        || prev.radius_q != next.radius_q
        || prev.agent_type_tag != next.agent_type_tag
    {
        return DeltaClassify::Replace;
    }
    let dx_i = i32::from(next.x_q) - i32::from(prev.x_q);
    let dy_i = i32::from(next.y_q) - i32::from(prev.y_q);
    let dz_i = i32::from(next.z_q) - i32::from(prev.z_q);
    let dyaw_i = i32::from(next.yaw_q) - i32::from(prev.yaw_q);
    let dpitch_i = i32::from(next.pitch_q) - i32::from(prev.pitch_q);
    let droll_i = i32::from(next.roll_q) - i32::from(prev.roll_q);
    let denergy_i = i32::from(next.energy_q) - i32::from(prev.energy_q);
    let dhydration_i = i32::from(next.hydration_q) - i32::from(prev.hydration_q);

    if (dx_i | dy_i | dz_i | dyaw_i | dpitch_i | droll_i | denergy_i | dhydration_i) == 0 {
        return DeltaClassify::Unchanged;
    }

    let (Ok(dx), Ok(dy), Ok(dz), Ok(dyaw), Ok(dpitch), Ok(droll), Ok(denergy), Ok(dhydration)) = (
        i16::try_from(dx_i),
        i16::try_from(dy_i),
        i16::try_from(dz_i),
        i16::try_from(dyaw_i),
        i16::try_from(dpitch_i),
        i16::try_from(droll_i),
        i16::try_from(denergy_i),
        i16::try_from(dhydration_i),
    ) else {
        return DeltaClassify::Replace;
    };

    DeltaClassify::Delta(SegmentDelta {
        agent_id: next.agent_id,
        segment_id: next.segment_id,
        dx,
        dy,
        dz,
        dyaw,
        dpitch,
        droll,
        denergy,
        dhydration,
    })
}

/// Resource delta for a lake or tree at index `index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvResourceDelta {
    pub index: u32,
    pub resources_q: u16,
}

/// Quantized sparse pheromone cell entry (`u8` quantized against `MAX_CONCENTRATION`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PheromoneCellDelta {
    pub cell_index: u16,
    pub value_q: u8,
}

/// Quantized transient combat event summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedCombatEvent {
    pub predator_id: u32,
    pub prey_id: u32,
    pub damage_q: u16,
    pub energy_transferred_q: u16,
}

impl QuantizedCombatEvent {
    pub fn from_combat_event(ev: &CombatEvent) -> Self {
        Self {
            predator_id: ev.predator_id,
            prey_id: ev.prey_id,
            damage_q: quantize_u16(ev.damage, COMBAT_QUANT_SCALE),
            energy_transferred_q: quantize_u16(ev.energy_transferred, COMBAT_QUANT_SCALE),
        }
    }
}

/// Full keyframe projected for a guest's current interest region (or whole island).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IslandSnapshot {
    pub island_id: IslandId,
    pub tick: u64,
    pub frame_seq: u64,
    pub census: IslandCensusSummary,
    pub segments: Vec<QuantizedSegmentPose>,
    pub env_features: Vec<StaticEnvFeature>,
    pub pheromone_cells: Vec<PheromoneCellDelta>,
    pub combat_events: Vec<QuantizedCombatEvent>,
}

/// Incremental quantized delta encoded relative to `base_frame_seq`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IslandDelta {
    pub island_id: IslandId,
    pub base_frame_seq: u64,
    pub frame_seq: u64,
    pub tick: u64,
    pub census: IslandCensusSummary,
    pub added_segments: Vec<QuantizedSegmentPose>,
    pub updated_segments: Vec<SegmentDelta>,
    pub removed_segments: Vec<(u32, u16)>,
    pub env_deltas: Vec<EnvResourceDelta>,
    pub pheromone_deltas: Vec<PheromoneCellDelta>,
    pub combat_events: Vec<QuantizedCombatEvent>,
}

/// Headline-only chronicle summary included in [`IslandJoinSnapshot`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IslandJoinChronicleSummary {
    pub id: String,
    pub event_type: String,
    pub timestamp: u64,
    pub title: String,
}

impl IslandJoinChronicleSummary {
    pub fn from_chronicle_event(event: &ChronicleEvent) -> Self {
        Self {
            id: event.id.clone(),
            event_type: event.event_type.clone(),
            timestamp: event.timestamp,
            title: event.title.clone(),
        }
    }
}

/// Stripped public join snapshot sent to newly admitted LAN guests (`Card I3`, `QD-PLAYRULES`).
///
/// Explicitly OMITS `SimRng` seeds/cursors, `AgentBrain` weights, `MorphologyGenotype` recipes,
/// `evolution_worker`, `shared_learning`, `map_elites_grid`, and `dormant_cohorts`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IslandJoinSnapshot {
    pub protocol_version: u16,
    pub island_id: IslandId,
    pub genesis_phase: GenesisPhase,
    pub genesis_label: String,
    pub tick: u64,
    pub world_identity: WorldIdentity,
    pub map_width: f32,
    pub map_height: f32,
    pub connected_guest_ids: Vec<GuestId>,
    pub static_env: Vec<StaticEnvFeature>,
    pub initial_snapshot: IslandSnapshot,
    pub chronicle_summary: Vec<IslandJoinChronicleSummary>,
}

impl IslandJoinSnapshot {
    /// Construct a stripped join payload from a checkpoint without exposing RNG or brain weights.
    pub fn from_saved_state(
        island_id: IslandId,
        genesis_phase: GenesisPhase,
        connected_guest_ids: &[GuestId],
        saved: &SavedSimulationState,
    ) -> Self {
        let mut sorted_guests: Vec<GuestId> = connected_guest_ids
            .iter()
            .copied()
            .filter(|id| id.is_valid())
            .collect();
        sorted_guests.sort_unstable();
        sorted_guests.dedup();

        let mut static_env =
            Vec::with_capacity(saved.lakes.len() + saved.trees.len() + saved.foods.len());
        for lake in &saved.lakes {
            static_env.push(StaticEnvFeature {
                kind: StaticEnvKind::Lake,
                x_q: quantize_i16(lake.position.x, ENV_COORD_QUANT_SCALE),
                z_q: quantize_i16(lake.position.z, ENV_COORD_QUANT_SCALE),
                radius_q: quantize_u16(lake.radius, DIM_QUANT_SCALE),
                resources_q: quantize_u16(lake.current_water, ENV_RESOURCE_QUANT_SCALE),
            });
        }
        for tree in &saved.trees {
            static_env.push(StaticEnvFeature {
                kind: StaticEnvKind::Tree,
                x_q: quantize_i16(tree.position.x, ENV_COORD_QUANT_SCALE),
                z_q: quantize_i16(tree.position.z, ENV_COORD_QUANT_SCALE),
                radius_q: quantize_u16(tree.radius, DIM_QUANT_SCALE),
                resources_q: quantize_u16(tree.current_fruit, ENV_RESOURCE_QUANT_SCALE),
            });
        }
        for food in &saved.foods {
            static_env.push(StaticEnvFeature {
                kind: StaticEnvKind::Food,
                x_q: quantize_i16(food.position.x, ENV_COORD_QUANT_SCALE),
                z_q: quantize_i16(food.position.z, ENV_COORD_QUANT_SCALE),
                radius_q: quantize_u16(0.25, DIM_QUANT_SCALE),
                resources_q: quantize_u16(food.energy_value, ENV_RESOURCE_QUANT_SCALE),
            });
        }

        let mut segments = Vec::new();
        let mut predators = 0u32;
        let mut prey = 0u32;
        for (idx, agent) in saved.agents.iter().enumerate() {
            let agent_id = (idx as u32).saturating_add(1);
            let agent_type = match agent.class {
                AgentClass::Predator => {
                    predators += 1;
                    Some(AgentType::Predator)
                }
                AgentClass::Prey => {
                    prey += 1;
                    Some(AgentType::Prey)
                }
            };
            let agent_type_tag = encode_agent_type(agent_type);
            let energy_q = quantize_u16(agent.homeostatic_state.energy, VITAL_QUANT_SCALE);
            let hydration_q = quantize_u16(agent.homeostatic_state.hydration, VITAL_QUANT_SCALE);

            // Identify morphology root id without retaining any genotype recipe bytes.
            let root_id = agent
                .genotype
                .nodes
                .iter()
                .find(|node| {
                    !agent
                        .genotype
                        .edges
                        .iter()
                        .any(|edge| edge.target_node == node.id)
                })
                .map(|node| node.id)
                .or_else(|| agent.genotype.nodes.first().map(|node| node.id))
                .unwrap_or(0);

            let (root_len, root_rad) = agent
                .genotype
                .nodes
                .iter()
                .find(|node| node.id == root_id)
                .map(|node| (node.length, node.radius))
                .unwrap_or((1.0, 0.25));
            let (root_yaw, root_pitch, root_roll) =
                agent.root_rotation.to_euler(glam::EulerRot::YXZ);

            segments.push(QuantizedSegmentPose {
                agent_id,
                segment_id: u16::try_from(root_id).unwrap_or(u16::MAX),
                parent_segment_id: u16::MAX,
                x_q: quantize_i16(agent.root_position.x, POS_QUANT_SCALE),
                y_q: quantize_i16(agent.root_position.y, POS_QUANT_SCALE),
                z_q: quantize_i16(agent.root_position.z, POS_QUANT_SCALE),
                yaw_q: quantize_angle(root_yaw),
                pitch_q: quantize_angle(root_pitch),
                roll_q: quantize_angle(root_roll),
                length_q: quantize_u16(root_len, DIM_QUANT_SCALE),
                radius_q: quantize_u16(root_rad, DIM_QUANT_SCALE),
                energy_q,
                hydration_q,
                agent_type_tag,
            });

            for child in &agent.segments {
                let (len, rad) = agent
                    .genotype
                    .nodes
                    .iter()
                    .find(|node| node.id == child.segment_id)
                    .map(|node| (node.length, node.radius))
                    .unwrap_or((0.6, 0.2));
                let parent_id = agent
                    .genotype
                    .edges
                    .iter()
                    .find(|edge| edge.target_node == child.segment_id)
                    .map(|edge| edge.source_node)
                    .unwrap_or(root_id);
                let (yaw, pitch, roll) = child.rotation.to_euler(glam::EulerRot::YXZ);

                segments.push(QuantizedSegmentPose {
                    agent_id,
                    segment_id: u16::try_from(child.segment_id).unwrap_or(u16::MAX),
                    parent_segment_id: u16::try_from(parent_id).unwrap_or(u16::MAX),
                    x_q: quantize_i16(child.position.x, POS_QUANT_SCALE),
                    y_q: quantize_i16(child.position.y, POS_QUANT_SCALE),
                    z_q: quantize_i16(child.position.z, POS_QUANT_SCALE),
                    yaw_q: quantize_angle(yaw),
                    pitch_q: quantize_angle(pitch),
                    roll_q: quantize_angle(roll),
                    length_q: quantize_u16(len, DIM_QUANT_SCALE),
                    radius_q: quantize_u16(rad, DIM_QUANT_SCALE),
                    energy_q,
                    hydration_q,
                    agent_type_tag,
                });
            }
        }
        segments.sort_unstable_by_key(QuantizedSegmentPose::key);

        let total_agents = u32::try_from(saved.agents.len()).unwrap_or(u32::MAX);
        let total_segments = u32::try_from(segments.len()).unwrap_or(u32::MAX);
        let census = IslandCensusSummary {
            total_agents,
            visible_agents: total_agents,
            total_segments,
            visible_segments: total_segments,
            predators,
            prey,
            interest_culled: false,
        };

        let mut pheromone_cells = Vec::new();
        for (idx, &val) in saved
            .pheromone_grid
            .values
            .iter()
            .take(CELL_COUNT)
            .enumerate()
        {
            if val > PHEROMONE_DELTA_THRESHOLD {
                let q = quantize_pheromone(val);
                if q > 0 {
                    pheromone_cells.push(PheromoneCellDelta {
                        cell_index: idx as u16,
                        value_q: q,
                    });
                }
            }
        }

        let initial_snapshot = IslandSnapshot {
            island_id,
            tick: saved.tick_count,
            frame_seq: 1,
            census,
            segments,
            env_features: static_env.clone(),
            pheromone_cells,
            combat_events: Vec::new(),
        };

        let chronicle_summary = saved
            .chronicle_history
            .iter()
            .map(IslandJoinChronicleSummary::from_chronicle_event)
            .collect();

        Self {
            protocol_version: ISLAND_STREAM_PROTOCOL_VERSION,
            island_id,
            genesis_phase,
            genesis_label: genesis_phase.display_label().to_string(),
            tick: saved.tick_count,
            world_identity: saved.world_identity,
            map_width: (saved.map_bounds.max.x - saved.map_bounds.min.x).max(0.0),
            map_height: (saved.map_bounds.max.z - saved.map_bounds.min.z).max(0.0),
            connected_guest_ids: sorted_guests,
            static_env,
            initial_snapshot,
            chronicle_summary,
        }
    }
}

/// Top-level `bincode`-framed server-to-guest message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IslandStreamFrame {
    Join(IslandJoinSnapshot),
    Snapshot(IslandSnapshot),
    Delta(IslandDelta),
}

/// Structured stream encoding/fan-out errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IslandStreamError {
    InvalidGuestId,
    InvalidInterestWindow,
    GuestLimitReached { max: u16 },
    DuplicateGuest(GuestId),
    UnknownGuest(GuestId),
    FrameTooLarge { bytes: usize, max: usize },
    EncodeFailed(String),
    DecodeFailed(String),
    DeltaBaseMismatch { expected: u64, found: u64 },
    LockPoisoned,
}

impl fmt::Display for IslandStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGuestId => f.write_str("guest_id must be non-zero"),
            Self::InvalidInterestWindow => {
                f.write_str("guest interest focus and radius must be finite and radius > 0")
            }
            Self::GuestLimitReached { max } => {
                write!(f, "island guest stream limit reached (max {max})")
            }
            Self::DuplicateGuest(id) => {
                write!(f, "guest {} already registered on stream", id.raw())
            }
            Self::UnknownGuest(id) => write!(f, "unknown guest {} on island stream", id.raw()),
            Self::FrameTooLarge { bytes, max } => {
                write!(
                    f,
                    "island stream frame size {bytes} B exceeds limit {max} B"
                )
            }
            Self::EncodeFailed(err) => write!(f, "failed encoding island stream frame: {err}"),
            Self::DecodeFailed(err) => write!(f, "failed decoding island stream frame: {err}"),
            Self::DeltaBaseMismatch { expected, found } => write!(
                f,
                "island delta base mismatch: expected frame_seq {expected}, found {found}"
            ),
            Self::LockPoisoned => f.write_str("shared island stream lock poisoned"),
        }
    }
}

impl std::error::Error for IslandStreamError {}

/// Serialize an [`IslandStreamFrame`] using compact `bincode` framing (`ISLAND-D3(a)`).
pub fn encode_stream_frame_bincode(
    frame: &IslandStreamFrame,
) -> Result<Vec<u8>, IslandStreamError> {
    let bytes = bincode::serde::encode_to_vec(frame, bincode::config::standard())
        .map_err(|err| IslandStreamError::EncodeFailed(err.to_string()))?;
    if bytes.len() > MAX_ISLAND_STREAM_FRAME_BYTES {
        return Err(IslandStreamError::FrameTooLarge {
            bytes: bytes.len(),
            max: MAX_ISLAND_STREAM_FRAME_BYTES,
        });
    }
    Ok(bytes)
}

/// Deserialize an [`IslandStreamFrame`] with strict byte-limit enforcement.
pub fn decode_stream_frame_bincode(bytes: &[u8]) -> Result<IslandStreamFrame, IslandStreamError> {
    if bytes.len() > MAX_ISLAND_STREAM_FRAME_BYTES {
        return Err(IslandStreamError::FrameTooLarge {
            bytes: bytes.len(),
            max: MAX_ISLAND_STREAM_FRAME_BYTES,
        });
    }
    let cfg = bincode::config::standard().with_limit::<MAX_ISLAND_STREAM_FRAME_BYTES>();
    let (frame, _len) = bincode::serde::decode_from_slice(bytes, cfg)
        .map_err(|err| IslandStreamError::DecodeFailed(err.to_string()))?;
    Ok(frame)
}

/// Per-guest off-tick camera spatial filter (`ISLAND-D4(b)`).
///
/// Stored strictly inside [`IslandStreamHub`]; never synced to [`crate::core::simulation_lod::SharedLodFocus`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuestInterestWindow {
    pub guest_id: GuestId,
    pub role: GuestRole,
    pub focus_x: f32,
    pub focus_z: f32,
    pub radius: f32,
}

impl GuestInterestWindow {
    pub fn new(
        guest_id: GuestId,
        role: GuestRole,
        focus_x: f32,
        focus_z: f32,
        radius: f32,
    ) -> Result<Self, IslandStreamError> {
        let win = Self {
            guest_id,
            role,
            focus_x,
            focus_z,
            radius,
        };
        win.validate()?;
        Ok(win)
    }

    pub fn validate(&self) -> Result<(), IslandStreamError> {
        if !self.guest_id.is_valid() {
            return Err(IslandStreamError::InvalidGuestId);
        }
        if !self.focus_x.is_finite()
            || !self.focus_z.is_finite()
            || !self.radius.is_finite()
            || self.radius <= 0.0
        {
            return Err(IslandStreamError::InvalidInterestWindow);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct AgentRootSummary {
    x: f32,
    z: f32,
    is_root: bool,
    visible: bool,
    agent_type: Option<AgentType>,
}

/// Project `segments` into `out` according to `ISLAND-D4(b)` threshold + radius culling.
///
/// Returns exact global census counts regardless of whether culling trimmed `out`.
pub fn filter_segments_by_interest(
    segments: &[SegmentState],
    window: &GuestInterestWindow,
    out: &mut Vec<QuantizedSegmentPose>,
) -> IslandCensusSummary {
    out.clear();
    let mut agent_map: HashMap<u32, AgentRootSummary> = HashMap::new();
    for seg in segments {
        let is_root = seg.parent_segment_id.is_none() || seg.segment_id == 0;
        agent_map
            .entry(seg.agent_id)
            .and_modify(|entry| {
                if is_root && !entry.is_root {
                    entry.x = seg.x;
                    entry.z = seg.z;
                    entry.is_root = true;
                }
                if entry.agent_type.is_none() {
                    entry.agent_type = seg.agent_type;
                }
            })
            .or_insert(AgentRootSummary {
                x: seg.x,
                z: seg.z,
                is_root,
                visible: false,
                agent_type: seg.agent_type,
            });
    }

    let total_agents_usize = agent_map.len();
    let interest_culled = total_agents_usize > WHOLE_ISLAND_BROADCAST_AGENT_LIMIT;
    let radius_sq = window.radius * window.radius;

    let mut predators = 0u32;
    let mut prey = 0u32;
    let mut visible_agents = 0u32;
    for entry in agent_map.values_mut() {
        match entry.agent_type {
            Some(AgentType::Predator) => predators += 1,
            Some(AgentType::Prey) => prey += 1,
            None => {}
        }
        let in_view = if !interest_culled {
            true
        } else {
            let dx = entry.x - window.focus_x;
            let dz = entry.z - window.focus_z;
            (dx * dx + dz * dz) <= radius_sq
        };
        entry.visible = in_view;
        if in_view {
            visible_agents += 1;
        }
    }

    for seg in segments {
        if agent_map.get(&seg.agent_id).is_some_and(|e| e.visible) {
            out.push(QuantizedSegmentPose::from_segment_state(seg));
        }
    }
    out.sort_unstable_by_key(QuantizedSegmentPose::key);

    IslandCensusSummary {
        total_agents: u32::try_from(total_agents_usize).unwrap_or(u32::MAX),
        visible_agents,
        total_segments: u32::try_from(segments.len()).unwrap_or(u32::MAX),
        visible_segments: u32::try_from(out.len()).unwrap_or(u32::MAX),
        predators,
        prey,
        interest_culled,
    }
}

/// Client-side reconstructor consuming [`IslandStreamFrame`]s back into sorted poses & state.
#[derive(Clone, Debug)]
pub struct IslandStreamReconstructor {
    pub last_frame_seq: Option<u64>,
    pub last_tick: u64,
    pub census: IslandCensusSummary,
    segments: Vec<QuantizedSegmentPose>,
    env_features: Vec<StaticEnvFeature>,
    pheromone_cells: Vec<u8>,
}

impl Default for IslandStreamReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

impl IslandStreamReconstructor {
    pub fn new() -> Self {
        Self {
            last_frame_seq: None,
            last_tick: 0,
            census: IslandCensusSummary::default(),
            segments: Vec::new(),
            env_features: Vec::new(),
            pheromone_cells: vec![0; CELL_COUNT],
        }
    }

    pub fn segments(&self) -> &[QuantizedSegmentPose] {
        &self.segments
    }

    pub fn env_features(&self) -> &[StaticEnvFeature] {
        &self.env_features
    }

    pub fn pheromone_cells(&self) -> &[u8] {
        &self.pheromone_cells
    }

    pub fn to_segment_states(&self) -> Vec<SegmentState> {
        self.segments
            .iter()
            .map(QuantizedSegmentPose::to_segment_state)
            .collect()
    }

    pub fn apply_frame(&mut self, frame: &IslandStreamFrame) -> Result<(), IslandStreamError> {
        match frame {
            IslandStreamFrame::Join(join) => self.apply_snapshot(&join.initial_snapshot),
            IslandStreamFrame::Snapshot(snap) => self.apply_snapshot(snap),
            IslandStreamFrame::Delta(delta) => self.apply_delta(delta),
        }
    }

    fn apply_snapshot(&mut self, snap: &IslandSnapshot) -> Result<(), IslandStreamError> {
        self.last_frame_seq = Some(snap.frame_seq);
        self.last_tick = snap.tick;
        self.census = snap.census;
        self.segments.clear();
        self.segments.extend_from_slice(&snap.segments);
        self.segments
            .sort_unstable_by_key(QuantizedSegmentPose::key);
        self.env_features.clear();
        self.env_features.extend_from_slice(&snap.env_features);
        self.pheromone_cells.fill(0);
        for cell in &snap.pheromone_cells {
            if let Some(dst) = self.pheromone_cells.get_mut(usize::from(cell.cell_index)) {
                *dst = cell.value_q;
            }
        }
        Ok(())
    }

    fn apply_delta(&mut self, delta: &IslandDelta) -> Result<(), IslandStreamError> {
        let Some(prev_seq) = self.last_frame_seq else {
            return Err(IslandStreamError::DeltaBaseMismatch {
                expected: 0,
                found: delta.base_frame_seq,
            });
        };
        if prev_seq != delta.base_frame_seq {
            return Err(IslandStreamError::DeltaBaseMismatch {
                expected: prev_seq,
                found: delta.base_frame_seq,
            });
        }

        if !delta.removed_segments.is_empty() {
            self.segments
                .retain(|seg| delta.removed_segments.binary_search(&seg.key()).is_err());
        }

        if !delta.updated_segments.is_empty() {
            for upd in &delta.updated_segments {
                let key = (upd.agent_id, upd.segment_id);
                if let Ok(idx) = self
                    .segments
                    .binary_search_by_key(&key, QuantizedSegmentPose::key)
                {
                    self.segments[idx].apply_delta(upd);
                }
            }
        }

        for added in &delta.added_segments {
            let key = added.key();
            match self
                .segments
                .binary_search_by_key(&key, QuantizedSegmentPose::key)
            {
                Ok(idx) => self.segments[idx] = *added,
                Err(idx) => self.segments.insert(idx, *added),
            }
        }

        for env_delta in &delta.env_deltas {
            if let Some(el) = self.env_features.get_mut(env_delta.index as usize) {
                el.resources_q = env_delta.resources_q;
            }
        }

        for cell in &delta.pheromone_deltas {
            if let Some(dst) = self.pheromone_cells.get_mut(usize::from(cell.cell_index)) {
                *dst = cell.value_q;
            }
        }

        self.last_frame_seq = Some(delta.frame_seq);
        self.last_tick = delta.tick;
        self.census = delta.census;
        Ok(())
    }
}

/// Framed binary packet queued in one guest's non-blocking outbound mailbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedIslandFrame {
    pub frame_seq: u64,
    pub tick: u64,
    pub is_keyframe: bool,
    pub bytes: Vec<u8>,
}

/// Per-guest stream accounting snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestStreamStats {
    pub guest_id: GuestId,
    pub role: GuestRole,
    pub queued_frames: usize,
    pub sent_frames: u64,
    pub dropped_frames: u64,
    pub keyframes_sent: u64,
    pub deltas_sent: u64,
    pub last_frame_seq: u64,
}

struct GuestStreamSlot {
    window: GuestInterestWindow,
    mailbox: VecDeque<EncodedIslandFrame>,
    last_frame_seq: u64,
    needs_keyframe: bool,
    last_segments: Vec<QuantizedSegmentPose>,
    last_env: Vec<StaticEnvFeature>,
    last_pheromone_q: Vec<u8>,
    sent_frames: u64,
    dropped_frames: u64,
    keyframes_sent: u64,
    deltas_sent: u64,
}

impl GuestStreamSlot {
    fn new(window: GuestInterestWindow, mailbox_capacity: usize) -> Self {
        Self {
            window,
            mailbox: VecDeque::with_capacity(mailbox_capacity),
            last_frame_seq: 0,
            needs_keyframe: true,
            last_segments: Vec::new(),
            last_env: Vec::new(),
            last_pheromone_q: vec![0; CELL_COUNT],
            sent_frames: 0,
            dropped_frames: 0,
            keyframes_sent: 0,
            deltas_sent: 0,
        }
    }

    fn enqueue_with_backpressure(&mut self, mailbox_capacity: usize, frame: EncodedIslandFrame) {
        if self.mailbox.len() >= mailbox_capacity {
            // Coalesce stale deltas on slow consumers: drain queued frames so the guest jumps
            // directly to the newest self-contained keyframe on next poll.
            let drained = u64::try_from(self.mailbox.len()).unwrap_or(u64::MAX);
            self.mailbox.clear();
            self.dropped_frames = self.dropped_frames.saturating_add(drained);
            self.needs_keyframe = true;
        }
        self.mailbox.push_back(frame);
        self.sent_frames = self.sent_frames.saturating_add(1);
    }

    fn stats(&self) -> GuestStreamStats {
        GuestStreamStats {
            guest_id: self.window.guest_id,
            role: self.window.role,
            queued_frames: self.mailbox.len(),
            sent_frames: self.sent_frames,
            dropped_frames: self.dropped_frames,
            keyframes_sent: self.keyframes_sent,
            deltas_sent: self.deltas_sent,
            last_frame_seq: self.last_frame_seq,
        }
    }
}

/// Multi-guest off-tick encoder + bounded slow-consumer fan-out ring (`Card I3`).
pub struct IslandStreamHub {
    island_id: IslandId,
    max_guests: u16,
    mailbox_capacity: usize,
    guests: Vec<GuestStreamSlot>,
    scratch_poses: Vec<QuantizedSegmentPose>,
    scratch_env: Vec<StaticEnvFeature>,
    scratch_pheromone: Vec<PheromoneCellDelta>,
    scratch_combat: Vec<QuantizedCombatEvent>,
    scratch_pheromone_q: Vec<u8>,
}

impl IslandStreamHub {
    pub fn new(island_id: IslandId, max_guests: u16, mailbox_capacity: usize) -> Self {
        Self {
            island_id,
            max_guests: max_guests.clamp(1, MAX_ISLAND_GUESTS_CEILING),
            mailbox_capacity: mailbox_capacity.max(1),
            guests: Vec::new(),
            scratch_poses: Vec::with_capacity(512),
            scratch_env: Vec::with_capacity(256),
            scratch_pheromone: Vec::with_capacity(128),
            scratch_combat: Vec::with_capacity(32),
            scratch_pheromone_q: vec![0; CELL_COUNT],
        }
    }

    pub fn with_defaults(island_id: IslandId) -> Self {
        Self::new(
            island_id,
            DEFAULT_ISLAND_MAX_GUESTS,
            DEFAULT_GUEST_MAILBOX_CAPACITY,
        )
    }

    pub const fn island_id(&self) -> IslandId {
        self.island_id
    }

    pub fn guest_count(&self) -> usize {
        self.guests.len()
    }

    pub fn register_guest(&mut self, window: GuestInterestWindow) -> Result<(), IslandStreamError> {
        window.validate()?;
        if self
            .guests
            .iter()
            .any(|g| g.window.guest_id == window.guest_id)
        {
            return Err(IslandStreamError::DuplicateGuest(window.guest_id));
        }
        if self.guests.len() >= usize::from(self.max_guests) {
            return Err(IslandStreamError::GuestLimitReached {
                max: self.max_guests,
            });
        }
        self.guests
            .push(GuestStreamSlot::new(window, self.mailbox_capacity));
        self.guests
            .sort_unstable_by_key(|slot| slot.window.guest_id);
        Ok(())
    }

    pub fn unregister_guest(&mut self, guest_id: GuestId) -> bool {
        let before = self.guests.len();
        self.guests.retain(|slot| slot.window.guest_id != guest_id);
        self.guests.len() != before
    }

    pub fn update_guest_interest(
        &mut self,
        guest_id: GuestId,
        focus_x: f32,
        focus_z: f32,
        radius: f32,
    ) -> Result<(), IslandStreamError> {
        if !focus_x.is_finite() || !focus_z.is_finite() || !radius.is_finite() || radius <= 0.0 {
            return Err(IslandStreamError::InvalidInterestWindow);
        }
        let slot = self
            .guests
            .iter_mut()
            .find(|g| g.window.guest_id == guest_id)
            .ok_or(IslandStreamError::UnknownGuest(guest_id))?;
        slot.window.focus_x = focus_x;
        slot.window.focus_z = focus_z;
        slot.window.radius = radius;
        Ok(())
    }

    pub fn pop_guest_frame(&mut self, guest_id: GuestId) -> Option<EncodedIslandFrame> {
        self.guests
            .iter_mut()
            .find(|slot| slot.window.guest_id == guest_id)
            .and_then(|slot| slot.mailbox.pop_front())
    }

    pub fn guest_stats(&self, guest_id: GuestId) -> Option<GuestStreamStats> {
        self.guests
            .iter()
            .find(|slot| slot.window.guest_id == guest_id)
            .map(GuestStreamSlot::stats)
    }

    /// Encode and fan out the current telemetry snapshot to every connected guest.
    pub fn project_tick(
        &mut self,
        tick: u64,
        segments: &[SegmentState],
        env: &EnvironmentalState,
        pheromone: Option<&PheromoneGridState>,
        combat: &[CombatEvent],
    ) -> Result<usize, IslandStreamError> {
        if self.guests.is_empty() {
            return Ok(0);
        }

        self.scratch_env.clear();
        for el in &env.elements {
            self.scratch_env
                .push(StaticEnvFeature::from_environmental_element(el));
        }

        self.scratch_combat.clear();
        for ev in combat {
            self.scratch_combat
                .push(QuantizedCombatEvent::from_combat_event(ev));
        }

        self.scratch_pheromone_q.fill(0);
        if let Some(grid) = pheromone {
            for (idx, &val) in grid.grid.iter().take(CELL_COUNT).enumerate() {
                self.scratch_pheromone_q[idx] = quantize_pheromone(val);
            }
        }

        let mut encoded_count = 0usize;
        for slot in &mut self.guests {
            let census =
                filter_segments_by_interest(segments, &slot.window, &mut self.scratch_poses);

            // If this slow consumer already saturated its bounded mailbox on an earlier turn,
            // coalesce by dropping stale deltas now and projecting a self-contained Snapshot so
            // the guest can immediately resynchronize on its next read.
            let mailbox_full = slot.mailbox.len() >= self.mailbox_capacity;
            let env_structure_changed = slot.last_env.len() != self.scratch_env.len()
                || slot
                    .last_env
                    .iter()
                    .zip(self.scratch_env.iter())
                    .any(|(a, b)| {
                        a.kind != b.kind
                            || a.x_q != b.x_q
                            || a.z_q != b.z_q
                            || a.radius_q != b.radius_q
                    });
            let force_snapshot = slot.needs_keyframe
                || slot.last_frame_seq == 0
                || env_structure_changed
                || mailbox_full;

            if mailbox_full {
                let drained = u64::try_from(slot.mailbox.len()).unwrap_or(u64::MAX);
                slot.mailbox.clear();
                slot.dropped_frames = slot.dropped_frames.saturating_add(drained);
            }

            let next_seq = slot.last_frame_seq.saturating_add(1);
            let (is_keyframe, wire_bytes) = if force_snapshot {
                self.scratch_pheromone.clear();
                for (idx, &q) in self.scratch_pheromone_q.iter().enumerate() {
                    if q > 0 {
                        self.scratch_pheromone.push(PheromoneCellDelta {
                            cell_index: idx as u16,
                            value_q: q,
                        });
                    }
                }
                let frame = IslandStreamFrame::Snapshot(IslandSnapshot {
                    island_id: self.island_id,
                    tick,
                    frame_seq: next_seq,
                    census,
                    segments: self.scratch_poses.clone(),
                    env_features: self.scratch_env.clone(),
                    pheromone_cells: self.scratch_pheromone.clone(),
                    combat_events: self.scratch_combat.clone(),
                });
                let bytes = encode_stream_frame_bincode(&frame)?;
                slot.keyframes_sent = slot.keyframes_sent.saturating_add(1);
                slot.needs_keyframe = false;
                (true, bytes)
            } else {
                let mut added_segments = Vec::new();
                let mut updated_segments = Vec::new();
                let mut removed_segments = Vec::new();

                // Both `slot.last_segments` and `self.scratch_poses` are strictly sorted by
                // `(agent_id, segment_id)`, so a linear merge computes minimal diffs with no HashMap.
                let mut prev_idx = 0usize;
                let mut next_idx = 0usize;
                let prev = &slot.last_segments;
                let next = &self.scratch_poses;
                while prev_idx < prev.len() && next_idx < next.len() {
                    let p = &prev[prev_idx];
                    let n = &next[next_idx];
                    match p.key().cmp(&n.key()) {
                        std::cmp::Ordering::Less => {
                            removed_segments.push(p.key());
                            prev_idx += 1;
                        }
                        std::cmp::Ordering::Greater => {
                            added_segments.push(*n);
                            next_idx += 1;
                        }
                        std::cmp::Ordering::Equal => {
                            match classify_segment_delta(p, n) {
                                DeltaClassify::Unchanged => {}
                                DeltaClassify::Delta(delta) => updated_segments.push(delta),
                                DeltaClassify::Replace => added_segments.push(*n),
                            }
                            prev_idx += 1;
                            next_idx += 1;
                        }
                    }
                }
                while prev_idx < prev.len() {
                    removed_segments.push(prev[prev_idx].key());
                    prev_idx += 1;
                }
                while next_idx < next.len() {
                    added_segments.push(next[next_idx]);
                    next_idx += 1;
                }

                let mut env_deltas = Vec::new();
                for (idx, (prev_e, next_e)) in slot
                    .last_env
                    .iter()
                    .zip(self.scratch_env.iter())
                    .enumerate()
                {
                    if prev_e.resources_q != next_e.resources_q {
                        env_deltas.push(EnvResourceDelta {
                            index: idx as u32,
                            resources_q: next_e.resources_q,
                        });
                    }
                }

                self.scratch_pheromone.clear();
                for (idx, (&prev_q, &next_q)) in slot
                    .last_pheromone_q
                    .iter()
                    .zip(self.scratch_pheromone_q.iter())
                    .enumerate()
                {
                    if prev_q != next_q {
                        self.scratch_pheromone.push(PheromoneCellDelta {
                            cell_index: idx as u16,
                            value_q: next_q,
                        });
                    }
                }

                let frame = IslandStreamFrame::Delta(IslandDelta {
                    island_id: self.island_id,
                    base_frame_seq: slot.last_frame_seq,
                    frame_seq: next_seq,
                    tick,
                    census,
                    added_segments,
                    updated_segments,
                    removed_segments,
                    env_deltas,
                    pheromone_deltas: self.scratch_pheromone.clone(),
                    combat_events: self.scratch_combat.clone(),
                });
                let bytes = encode_stream_frame_bincode(&frame)?;
                slot.deltas_sent = slot.deltas_sent.saturating_add(1);
                (false, bytes)
            };

            slot.last_frame_seq = next_seq;
            slot.last_segments.clear();
            slot.last_segments.extend_from_slice(&self.scratch_poses);
            slot.last_env.clear();
            slot.last_env.extend_from_slice(&self.scratch_env);
            slot.last_pheromone_q
                .copy_from_slice(&self.scratch_pheromone_q);

            slot.enqueue_with_backpressure(
                self.mailbox_capacity,
                EncodedIslandFrame {
                    frame_seq: next_seq,
                    tick,
                    is_keyframe,
                    bytes: wire_bytes,
                },
            );
            encoded_count += 1;
        }

        Ok(encoded_count)
    }
}

/// Thread-safe handle shared between the off-tick emit/network threads (`SimulationEngine::island_stream`).
///
/// Inactive (`None`) by default so solo runs pay zero memory and only one relaxed atomic check.
#[derive(Clone)]
pub struct SharedIslandStream {
    inner: Arc<Mutex<Option<IslandStreamHub>>>,
    active_guests: Arc<AtomicUsize>,
}

impl Default for SharedIslandStream {
    fn default() -> Self {
        Self::new_inactive()
    }
}

impl SharedIslandStream {
    pub fn new_inactive() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            active_guests: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn from_hub(hub: IslandStreamHub) -> Self {
        let count = hub.guest_count();
        Self {
            inner: Arc::new(Mutex::new(Some(hub))),
            active_guests: Arc::new(AtomicUsize::new(count)),
        }
    }

    pub fn has_active_guests(&self) -> bool {
        self.active_guests.load(Ordering::Relaxed) > 0
    }

    pub fn attach_hub(&self, hub: IslandStreamHub) -> Result<(), IslandStreamError> {
        let count = hub.guest_count();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        *guard = Some(hub);
        self.active_guests.store(count, Ordering::Release);
        Ok(())
    }

    pub fn detach_hub(&self) -> Result<Option<IslandStreamHub>, IslandStreamError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        let prev = guard.take();
        self.active_guests.store(0, Ordering::Release);
        Ok(prev)
    }

    pub fn register_guest(&self, window: GuestInterestWindow) -> Result<(), IslandStreamError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        let hub = guard
            .as_mut()
            .ok_or(IslandStreamError::UnknownGuest(window.guest_id))?;
        hub.register_guest(window)?;
        self.active_guests
            .store(hub.guest_count(), Ordering::Release);
        Ok(())
    }

    pub fn unregister_guest(&self, guest_id: GuestId) -> Result<bool, IslandStreamError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        let Some(hub) = guard.as_mut() else {
            return Ok(false);
        };
        let removed = hub.unregister_guest(guest_id);
        self.active_guests
            .store(hub.guest_count(), Ordering::Release);
        Ok(removed)
    }

    pub fn update_guest_interest(
        &self,
        guest_id: GuestId,
        focus_x: f32,
        focus_z: f32,
        radius: f32,
    ) -> Result<(), IslandStreamError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        let hub = guard
            .as_mut()
            .ok_or(IslandStreamError::UnknownGuest(guest_id))?;
        hub.update_guest_interest(guest_id, focus_x, focus_z, radius)
    }

    pub fn project_tick(
        &self,
        tick: u64,
        segments: &[SegmentState],
        env: &EnvironmentalState,
        pheromone: Option<&PheromoneGridState>,
        combat: &[CombatEvent],
    ) -> Result<usize, IslandStreamError> {
        if !self.has_active_guests() {
            return Ok(0);
        }
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        let Some(hub) = guard.as_mut() else {
            self.active_guests.store(0, Ordering::Release);
            return Ok(0);
        };
        hub.project_tick(tick, segments, env, pheromone, combat)
    }

    pub fn pop_guest_frame(
        &self,
        guest_id: GuestId,
    ) -> Result<Option<EncodedIslandFrame>, IslandStreamError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        Ok(guard.as_mut().and_then(|hub| hub.pop_guest_frame(guest_id)))
    }

    pub fn guest_stats(
        &self,
        guest_id: GuestId,
    ) -> Result<Option<GuestStreamStats>, IslandStreamError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| IslandStreamError::LockPoisoned)?;
        Ok(guard.as_ref().and_then(|hub| hub.guest_stats(guest_id)))
    }
}
