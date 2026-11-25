//! Zero-allocation guest intent decode, backpressure throttling (`ISLAND-D5(a)`), and
//! pre-sensory tick-boundary drain (`ADR-0014` / Card I1).

use super::{GuestId, GuestRole, HostModerationAction, IslandId};
use crate::core::observer::ObserverPolicy;
use anima_domain::causal::{is_island_guest_cause, CauseId};
use bevy_ecs::prelude::Resource;
use bevy_ecs::system::{Local, Res, ResMut};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Upper byte limit on one serialized `GuestIntentMessage` frame (`4 KiB`).
pub const MAX_GUEST_INTENT_FRAME_BYTES: usize = 4_096;

/// Default bounded capacity of the host pre-tick guest intent ingress ring.
pub const DEFAULT_GUEST_INTENT_INGRESS_CAPACITY: usize = 256;

/// Default pre-allocated capacity of [`DrainedGuestIntents`].
pub const DEFAULT_DRAINED_INTENT_CAPACITY: usize = 256;

/// JSON keys representing raw world/entity state that guests are strictly forbidden to author.
const FORBIDDEN_RAW_STATE_KEYS: &[&str] = &[
    "segment_states",
    "segments",
    "segment_state",
    "saved_simulation_state",
    "saved_state",
    "agent_migration_data",
    "agent_migration",
    "migration_data",
    "position",
    "positions",
    "velocity",
    "velocities",
    "energy",
    "energies",
    "genotype",
    "genotypes",
    "brain_weights",
    "weights",
    "sim_rng",
    "rng_state",
    "world_identity",
    "world_checksum",
    "eco_plants",
    "eco_animals",
    "detritus",
    "world_record",
    "grass_patch",
    "extra_speed",
];

/// JSON keys/tags representing host-only environment or macro interventions (`QD-PLAYRULES` Rule 5).
const FORBIDDEN_ENV_INTERVENTION_KEYS: &[&str] = &[
    "intervention",
    "interventions",
    "environment_intervention",
    "exotic_intervention",
    "exotic_interventions",
    "observer_action",
    "rainfall_delta",
    "temperature_delta",
    "deforest",
    "remove_predators",
    "add_nutrient",
    "genetic_injection",
    "toxicity_plume",
    "pathogen_transmission",
    "evolution_settings_changed",
    "evolution_toggled",
    "migration_triggered",
    "sharding_config_changed",
    "add_source",
    "remove_source",
    "pulse",
];

/// Inhabitable organism categories in Milestone 1 (`D-PLANT-1(a)`: animals and individual plants
/// only; grass cells remain non-inhabitable populations).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganismTargetKind {
    Animal,
    IndividualPlant,
}

/// Typed embodied animal intent (`action_resolution_system` + `steering_drive_system` in `I2`).
///
/// Guests submit desired movement direction, normalized speed fraction, and discrete action intents;
/// they never transmit raw coordinates, velocities, energy, or stats (`QD-PLAYRULES` Rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureControlIntent {
    pub move_dir_x: f32,
    pub move_dir_z: f32,
    pub speed_fraction: f32,
    pub feed_intent: bool,
    pub attack_intent: bool,
    pub pheromone_emit: f32,
}

impl CreatureControlIntent {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.move_dir_x.is_finite()
            || !self.move_dir_z.is_finite()
            || !self.speed_fraction.is_finite()
            || !self.pheromone_emit.is_finite()
        {
            return Err("non-finite float in creature control intent");
        }
        if !(-1.0..=1.0).contains(&self.move_dir_x) || !(-1.0..=1.0).contains(&self.move_dir_z) {
            return Err("movement direction components must lie in [-1.0, 1.0]");
        }
        let sq_norm = self.move_dir_x * self.move_dir_x + self.move_dir_z * self.move_dir_z;
        if sq_norm > 1.0 + 1e-3 {
            return Err("movement direction vector norm exceeds 1.0");
        }
        if !(0.0..=1.0).contains(&self.speed_fraction) {
            return Err("speed_fraction must lie in [0.0, 1.0]");
        }
        if !(0.0..=1.0).contains(&self.pheromone_emit) {
            return Err("pheromone_emit must lie in [0.0, 1.0]");
        }
        Ok(())
    }
}

/// Typed individual-plant allocation intent (`ADR-0014` + `ADR-0015` hook stub).
///
/// Shares are normalized allocation fractions in `[0.0, 1.0]` summing to `<= 1.0`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlantControlIntent {
    pub root_share: f32,
    pub canopy_share: f32,
    pub defence_share: f32,
    pub reproduction_share: f32,
    pub trigger_flowering: bool,
    pub seed_size_bias: f32,
}

impl PlantControlIntent {
    pub fn validate(&self) -> Result<(), &'static str> {
        for v in [
            self.root_share,
            self.canopy_share,
            self.defence_share,
            self.reproduction_share,
            self.seed_size_bias,
        ] {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err("plant control fractions must be finite and in [0.0, 1.0]");
            }
        }
        let total =
            self.root_share + self.canopy_share + self.defence_share + self.reproduction_share;
        if total > 1.0 + 1e-4 {
            return Err("plant allocation shares sum past 1.0");
        }
        Ok(())
    }
}

/// Strict enum of guest-authorized embodied intents (`ADR-0014`).
///
/// Raw state mutation frames and host-only environmental interventions are unrepresentable here.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum GuestIntent {
    ClaimOrganism {
        organism_id: u64,
        kind: OrganismTargetKind,
    },
    ReleaseOrganism,
    CreatureControl(CreatureControlIntent),
    PlantControl(PlantControlIntent),
}

impl GuestIntent {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::ClaimOrganism { organism_id, .. } => {
                if *organism_id == 0 {
                    return Err("claim_organism organism_id must be non-zero");
                }
                Ok(())
            }
            Self::ReleaseOrganism => Ok(()),
            Self::CreatureControl(ctrl) => ctrl.validate(),
            Self::PlantControl(ctrl) => ctrl.validate(),
        }
    }

    pub const fn requires_inhabit_role(&self) -> bool {
        !matches!(self, Self::ReleaseOrganism)
    }
}

/// Wire envelope accepted at the host guest ingress seam (`I1` / `I4`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestIntentMessage {
    pub island_id: IslandId,
    pub guest_id: GuestId,
    pub client_seq: u64,
    pub intent: GuestIntent,
}

/// Structured rejection reasons returned before simulation queues (`ISLAND-D5(a)` / `ADR-0014`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestIntentDecodeError {
    FrameTooLarge { bytes: usize, max: usize },
    RejectedRawStateOrEnvIntervention { reason: &'static str },
    InvalidEncoding,
    InvalidFieldValue { reason: &'static str },
    IslandMismatch,
    InvalidGuestId,
    InvalidCauseId,
    RoleForbidden,
    QueueFullThrottled,
    LockPoisoned,
}

fn contains_forbidden_json_payload(value: &serde_json::Value) -> Option<&'static str> {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let lower = k.to_ascii_lowercase();
                if FORBIDDEN_RAW_STATE_KEYS.contains(&lower.as_str()) {
                    return Some("forged raw state field rejected before simulation queues");
                }
                if FORBIDDEN_ENV_INTERVENTION_KEYS.contains(&lower.as_str()) {
                    return Some("guest environment intervention rejected (host-only)");
                }
                if k == "intent" {
                    if let Some(tag) = v.as_str() {
                        let tag_lower = tag.to_ascii_lowercase();
                        if FORBIDDEN_RAW_STATE_KEYS.contains(&tag_lower.as_str()) {
                            return Some(
                                "forged raw state intent rejected before simulation queues",
                            );
                        }
                        if FORBIDDEN_ENV_INTERVENTION_KEYS.contains(&tag_lower.as_str()) {
                            return Some(
                                "guest environment intervention intent rejected (host-only)",
                            );
                        }
                    }
                }
                if let Some(hit) = contains_forbidden_json_payload(v) {
                    return Some(hit);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(hit) = contains_forbidden_json_payload(item) {
                    return Some(hit);
                }
            }
            None
        }
        _ => None,
    }
}

/// Decode and validate a bounded JSON [`GuestIntentMessage`] before simulation ingress.
pub fn decode_guest_intent_json(
    bytes: &[u8],
    expected_island: IslandId,
) -> Result<GuestIntentMessage, GuestIntentDecodeError> {
    if bytes.len() > MAX_GUEST_INTENT_FRAME_BYTES {
        return Err(GuestIntentDecodeError::FrameTooLarge {
            bytes: bytes.len(),
            max: MAX_GUEST_INTENT_FRAME_BYTES,
        });
    }
    let val: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| GuestIntentDecodeError::InvalidEncoding)?;
    if let Some(reason) = contains_forbidden_json_payload(&val) {
        return Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention { reason });
    }
    let msg: GuestIntentMessage =
        serde_json::from_value(val).map_err(|_| GuestIntentDecodeError::InvalidEncoding)?;
    validate_decoded_message(msg, expected_island)?;
    Ok(msg)
}

/// Decode and validate a bounded `bincode` [`GuestIntentMessage`] before simulation ingress.
pub fn decode_guest_intent_bincode(
    bytes: &[u8],
    expected_island: IslandId,
) -> Result<GuestIntentMessage, GuestIntentDecodeError> {
    if bytes.len() > MAX_GUEST_INTENT_FRAME_BYTES {
        return Err(GuestIntentDecodeError::FrameTooLarge {
            bytes: bytes.len(),
            max: MAX_GUEST_INTENT_FRAME_BYTES,
        });
    }
    let cfg = bincode::config::standard().with_limit::<MAX_GUEST_INTENT_FRAME_BYTES>();
    let (msg, read_len): (GuestIntentMessage, usize) =
        bincode::serde::decode_from_slice(bytes, cfg)
            .map_err(|_| GuestIntentDecodeError::InvalidEncoding)?;
    if read_len != bytes.len() {
        return Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention {
            reason: "trailing bytes rejected in guest intent frame",
        });
    }
    validate_decoded_message(msg, expected_island)?;
    Ok(msg)
}

fn validate_decoded_message(
    msg: GuestIntentMessage,
    expected_island: IslandId,
) -> Result<(), GuestIntentDecodeError> {
    if !msg.island_id.is_valid() || msg.island_id != expected_island {
        return Err(GuestIntentDecodeError::IslandMismatch);
    }
    if !msg.guest_id.is_valid() {
        return Err(GuestIntentDecodeError::InvalidGuestId);
    }
    msg.intent
        .validate()
        .map_err(|reason| GuestIntentDecodeError::InvalidFieldValue { reason })
}

#[derive(Clone, Copy, Debug)]
struct QueuedGuestIntent {
    seq: u64,
    guest_id: GuestId,
    cause_id: CauseId,
    intent: GuestIntent,
}

/// Monotonic counters exposed for host diagnostics and negative-control gates (`ADR-0014`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestIngressCounters {
    pub accepted: u64,
    pub throttled_full: u64,
    pub rejected_decode: u64,
    pub rejected_policy: u64,
    pub drained: u64,
    pub drained_overflow: u64,
}

/// Pre-tick stamped intent record produced by [`drain_guest_intents_system`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampedGuestIntent {
    pub apply_tick: u64,
    pub seq: u64,
    pub guest_id: GuestId,
    pub cause_id: CauseId,
    pub intent: GuestIntent,
}

struct GuestIntentQueue {
    island_id: IslandId,
    records: Vec<QueuedGuestIntent>,
    limit: usize,
    moderation: Vec<HostModerationAction>,
    moderation_limit: usize,
    next_seq: u64,
    counters: GuestIngressCounters,
}

/// Bounded thread-safe ingress handle shared between async transport/UI tasks and the ECS tick.
///
/// All internal buffers are pre-allocated at construction so steady-state enqueue/drain paths never
/// reallocate (`ISLAND-D5(a)` rejects on full rather than growing or silently dropping accepted
/// intents).
#[derive(Resource, Clone)]
pub struct SharedGuestIntents(Arc<Mutex<GuestIntentQueue>>);

impl SharedGuestIntents {
    pub fn new(island_id: IslandId) -> Self {
        Self::with_capacity(island_id, DEFAULT_GUEST_INTENT_INGRESS_CAPACITY)
    }

    pub fn with_capacity(island_id: IslandId, capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self(Arc::new(Mutex::new(GuestIntentQueue {
            island_id,
            records: Vec::with_capacity(cap),
            limit: cap,
            moderation: Vec::with_capacity(cap),
            moderation_limit: cap,
            next_seq: 1,
            counters: GuestIngressCounters::default(),
        })))
    }

    pub fn island_id(&self) -> IslandId {
        match self.0.lock() {
            Ok(q) => q.island_id,
            Err(_) => IslandId(0),
        }
    }

    pub fn counters(&self) -> GuestIngressCounters {
        match self.0.lock() {
            Ok(q) => q.counters,
            Err(_) => GuestIngressCounters::default(),
        }
    }

    pub fn pending_len(&self) -> usize {
        match self.0.lock() {
            Ok(q) => q.records.len(),
            Err(_) => 0,
        }
    }

    /// Enqueue an already-decoded intent after verifying reserved [`CauseId`] provenance and fixed
    /// capacity (`ISLAND-D5(a)` backpressure throttling).
    pub fn try_enqueue(
        &self,
        guest_id: GuestId,
        cause_id: CauseId,
        intent: GuestIntent,
    ) -> Result<u64, GuestIntentDecodeError> {
        let mut queue = self
            .0
            .lock()
            .map_err(|_| GuestIntentDecodeError::LockPoisoned)?;
        if !guest_id.is_valid() {
            queue.counters.rejected_policy = queue.counters.rejected_policy.saturating_add(1);
            return Err(GuestIntentDecodeError::InvalidGuestId);
        }
        if !is_island_guest_cause(cause_id) {
            queue.counters.rejected_policy = queue.counters.rejected_policy.saturating_add(1);
            return Err(GuestIntentDecodeError::InvalidCauseId);
        }
        if let Err(reason) = intent.validate() {
            queue.counters.rejected_decode = queue.counters.rejected_decode.saturating_add(1);
            return Err(GuestIntentDecodeError::InvalidFieldValue { reason });
        }
        if queue.records.len() >= queue.limit {
            queue.counters.throttled_full = queue.counters.throttled_full.saturating_add(1);
            return Err(GuestIntentDecodeError::QueueFullThrottled);
        }
        let seq = queue.next_seq;
        queue.next_seq = queue.next_seq.saturating_add(1);
        queue.records.push(QueuedGuestIntent {
            seq,
            guest_id,
            cause_id,
            intent,
        });
        queue.counters.accepted = queue.counters.accepted.saturating_add(1);
        Ok(seq)
    }

    /// Decode a JSON frame and enqueue it for `role` + `cause_id`, counting every rejection cleanly
    /// before touching the simulation queue.
    pub fn decode_and_enqueue_json(
        &self,
        bytes: &[u8],
        cause_id: CauseId,
        role: GuestRole,
    ) -> Result<u64, GuestIntentDecodeError> {
        let expected_island = self.island_id();
        let msg = match decode_guest_intent_json(bytes, expected_island) {
            Ok(msg) => msg,
            Err(err) => {
                if let Ok(mut queue) = self.0.lock() {
                    queue.counters.rejected_decode =
                        queue.counters.rejected_decode.saturating_add(1);
                }
                return Err(err);
            }
        };
        if role == GuestRole::Spectate && msg.intent.requires_inhabit_role() {
            if let Ok(mut queue) = self.0.lock() {
                queue.counters.rejected_policy = queue.counters.rejected_policy.saturating_add(1);
            }
            return Err(GuestIntentDecodeError::RoleForbidden);
        }
        self.try_enqueue(msg.guest_id, cause_id, msg.intent)
    }

    /// Decode a `bincode` frame and enqueue it for `role` + `cause_id`, counting every rejection.
    pub fn decode_and_enqueue_bincode(
        &self,
        bytes: &[u8],
        cause_id: CauseId,
        role: GuestRole,
    ) -> Result<u64, GuestIntentDecodeError> {
        let expected_island = self.island_id();
        let msg = match decode_guest_intent_bincode(bytes, expected_island) {
            Ok(msg) => msg,
            Err(err) => {
                if let Ok(mut queue) = self.0.lock() {
                    queue.counters.rejected_decode =
                        queue.counters.rejected_decode.saturating_add(1);
                }
                return Err(err);
            }
        };
        if role == GuestRole::Spectate && msg.intent.requires_inhabit_role() {
            if let Ok(mut queue) = self.0.lock() {
                queue.counters.rejected_policy = queue.counters.rejected_policy.saturating_add(1);
            }
            return Err(GuestIntentDecodeError::RoleForbidden);
        }
        self.try_enqueue(msg.guest_id, cause_id, msg.intent)
    }

    /// Enqueue a host moderation action (`KickGuest`) to execute at the next tick boundary.
    pub fn enqueue_moderation(
        &self,
        action: HostModerationAction,
    ) -> Result<(), GuestIntentDecodeError> {
        let mut queue = self
            .0
            .lock()
            .map_err(|_| GuestIntentDecodeError::LockPoisoned)?;
        let HostModerationAction::KickGuest { guest_id, cause_id } = action;
        if !guest_id.is_valid() {
            return Err(GuestIntentDecodeError::InvalidGuestId);
        }
        if !is_island_guest_cause(cause_id) {
            return Err(GuestIntentDecodeError::InvalidCauseId);
        }
        if queue.moderation.len() >= queue.moderation_limit {
            queue.counters.throttled_full = queue.counters.throttled_full.saturating_add(1);
            return Err(GuestIntentDecodeError::QueueFullThrottled);
        }
        queue.moderation.push(action);
        Ok(())
    }
}

/// Per-tick pre-allocated staging buffer populated by [`drain_guest_intents_system`].
///
/// `Card I2` reads these stamped intents inside actuation systems without touching locks or
/// allocating on the tick thread.
#[derive(Resource, Debug)]
pub struct DrainedGuestIntents {
    intents: Vec<StampedGuestIntent>,
    moderation: Vec<HostModerationAction>,
    last_tick: u64,
    total_drained: u64,
    overflow_drops: u64,
}

impl Default for DrainedGuestIntents {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_DRAINED_INTENT_CAPACITY)
    }
}

impl DrainedGuestIntents {
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            intents: Vec::with_capacity(cap),
            moderation: Vec::with_capacity(cap),
            last_tick: 0,
            total_drained: 0,
            overflow_drops: 0,
        }
    }

    pub fn intents(&self) -> &[StampedGuestIntent] {
        &self.intents
    }

    pub fn moderation(&self) -> &[HostModerationAction] {
        &self.moderation
    }

    pub const fn last_tick(&self) -> u64 {
        self.last_tick
    }

    pub const fn total_drained(&self) -> u64 {
        self.total_drained
    }

    pub const fn overflow_drops(&self) -> u64 {
        self.overflow_drops
    }
}

/// Pre-sensory tick-boundary drain for shared-island guest intents (`ADR-0014` / Card I1).
///
/// Completely inert without both [`SharedGuestIntents`] and [`DrainedGuestIntents`], and strictly
/// allocation-free on the simulation tick thread: `clear()` preserves pre-allocated capacities and
/// no buffer ever grows inside the tick loop. Under [`ObserverPolicy::Absent`], any queued intents
/// are refused/discarded and counted without reaching actuation.
pub fn drain_guest_intents_system(
    policy: Option<Res<ObserverPolicy>>,
    queued: Option<Res<SharedGuestIntents>>,
    drained: Option<ResMut<DrainedGuestIntents>>,
    mut tick: Local<u64>,
) {
    let (Some(queued), Some(mut drained)) = (queued, drained) else {
        return;
    };
    *tick = tick.wrapping_add(1);
    drained.last_tick = *tick;
    drained.intents.clear();
    drained.moderation.clear();

    let Ok(mut queue) = queued.0.lock() else {
        return;
    };

    if matches!(policy.as_deref(), Some(ObserverPolicy::Absent)) {
        let dropped_intents = queue.records.len() as u64;
        let dropped_moderation = queue.moderation.len() as u64;
        queue.records.clear();
        queue.moderation.clear();
        queue.counters.rejected_policy = queue
            .counters
            .rejected_policy
            .saturating_add(dropped_intents.saturating_add(dropped_moderation));
        return;
    }

    let queue_ref = &mut *queue;
    let mod_cap = drained.moderation.capacity();
    for action in queue_ref.moderation.drain(..) {
        if drained.moderation.len() < mod_cap {
            drained.moderation.push(action);
        } else {
            drained.overflow_drops = drained.overflow_drops.saturating_add(1);
            queue_ref.counters.drained_overflow =
                queue_ref.counters.drained_overflow.saturating_add(1);
        }
    }

    let intent_cap = drained.intents.capacity();
    for record in queue_ref.records.drain(..) {
        if drained.intents.len() < intent_cap {
            drained.intents.push(StampedGuestIntent {
                apply_tick: *tick,
                seq: record.seq,
                guest_id: record.guest_id,
                cause_id: record.cause_id,
                intent: record.intent,
            });
            drained.total_drained = drained.total_drained.saturating_add(1);
            queue_ref.counters.drained = queue_ref.counters.drained.saturating_add(1);
        } else {
            drained.overflow_drops = drained.overflow_drops.saturating_add(1);
            queue_ref.counters.drained_overflow =
                queue_ref.counters.drained_overflow.saturating_add(1);
        }
    }
}
