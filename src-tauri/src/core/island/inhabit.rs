//! Authoritative host `Inhabit` actuation, disconnect/kick/death release, WR1/WR2 guest intent
//! recording with change-only coalescing, and deterministic offline replay (`ADR-0014` / Card I2).
//!
//! All steady-state tick systems here return on their first line when [`IslandInhabitState`] is
//! absent so legacy solo runs execute with zero overhead and zero heap allocations.

use super::{
    effective_genesis_phase, CreatureControlIntent, GenesisPhase, GuestId, GuestRole,
    HostModerationAction, IslandGuestPolicy, IslandId, OrganismTargetKind, PlantControlIntent,
    MAX_ISLAND_GUESTS_CEILING,
};
use crate::ai::hrrl::HomeostaticState;
use crate::core::agent_systems::AgentLineageId;
use crate::core::causal::{is_island_guest_cause, CausalLedger, CauseId, CAUSE_OBSERVER};
use crate::core::components::{ActionGates, Agent};
use crate::core::ecs::Position;
use crate::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use crate::core::experiment_runner::ExperimentModel;
use crate::core::live_experiment::{LiveExperimentAdapter, LiveSnapshot};
use crate::core::observer::ObserverPolicy;
use crate::core::world_artifact::WorldIdentity;
use crate::core::world_record::chain::{validate_header, verify_record};
use crate::core::world_record::system::{build_island_header_from_world, WorldRecordRecorder};
use crate::core::world_record::types::{
    GuestIntentBoundary, GuestSessionRecord, InputKind, IslandRecordHeader, RecordedGuestIntent,
    WorldRecord, WorldRecordError, WorldRecordHeader,
};
use crate::core::world_record::writer::WorldRecordWriterConfig;
use bevy_ecs::prelude::{Component, Entity, Query, QueryState, ResMut, Resource, With, World};
use serde::{Deserialize, Serialize};

use super::{DrainedGuestIntents, GuestIntent, SharedGuestIntents};

/// Default keepalive timeout (in simulation ticks) before an unresponsive guest lease is released
/// immediately per `ISLAND-D1(a)`.
pub const DEFAULT_ISLAND_KEEPALIVE_TIMEOUT_TICKS: u64 = 300;

/// Pre-allocated per-tick capacity for coalesced guest inputs forwarded to `WorldRecordRecorder`.
pub const DEFAULT_RECORDED_INTENT_SCRATCH_CAP: usize = 256;

/// Neutral animal control baseline attached on initial `ClaimOrganism`.
pub const NEUTRAL_CREATURE_CONTROL: CreatureControlIntent = CreatureControlIntent {
    move_dir_x: 0.0,
    move_dir_z: 0.0,
    speed_fraction: 0.0,
    feed_intent: false,
    attack_intent: false,
    pheromone_emit: 0.0,
};

/// Compute a deterministic, non-zero 64-bit organism identifier from a creature's lineage string
/// (or entity index fallback), remaining stable across `save -> load -> continue` entity restores.
pub fn stable_organism_id_from_lineage(lineage_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in lineage_id.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    if hash == 0 {
        1
    } else {
        hash
    }
}

/// Resolve an agent entity's stable `organism_id` using [`AgentLineageId`] when present.
pub fn stable_organism_id(lineage: Option<&AgentLineageId>, entity: Entity) -> u64 {
    match lineage {
        Some(l) if !l.0.is_empty() => stable_organism_id_from_lineage(&l.0),
        _ => (entity.index() as u64).saturating_add(1),
    }
}

/// List `(organism_id, Entity, lineage_id, is_predator)` for every living `Agent` in deterministic order.
pub fn list_living_organism_targets(world: &mut World) -> Vec<(u64, Entity, String)> {
    let mut out = Vec::new();
    let mut q = world.query_filtered::<(Entity, Option<&AgentLineageId>), With<Agent>>();
    for (entity, lineage) in q.iter(world) {
        let id = stable_organism_id(lineage, entity);
        let label = lineage.map_or_else(|| format!("entity-{}", entity.index()), |l| l.0.clone());
        out.push((id, entity, label));
    }
    out.sort_by_key(|(id, entity, _)| (*id, entity.index()));
    out
}

/// Compute normalized CPG drive scalar in `[0.0, 1.0]` from a validated [`CreatureControlIntent`].
#[inline]
pub fn creature_cpg_drive(ctrl: CreatureControlIntent) -> f32 {
    let dir_sq = ctrl.move_dir_x * ctrl.move_dir_x + ctrl.move_dir_z * ctrl.move_dir_z;
    if !dir_sq.is_finite() || dir_sq <= 1e-6 {
        0.0
    } else {
        ctrl.speed_fraction.clamp(0.0, 1.0)
    }
}

#[inline]
fn creature_control_bits_eq(a: &CreatureControlIntent, b: &CreatureControlIntent) -> bool {
    a.move_dir_x.to_bits() == b.move_dir_x.to_bits()
        && a.move_dir_z.to_bits() == b.move_dir_z.to_bits()
        && a.speed_fraction.to_bits() == b.speed_fraction.to_bits()
        && a.feed_intent == b.feed_intent
        && a.attack_intent == b.attack_intent
        && a.pheromone_emit.to_bits() == b.pheromone_emit.to_bits()
}

#[inline]
fn plant_control_bits_eq(a: &PlantControlIntent, b: &PlantControlIntent) -> bool {
    a.root_share.to_bits() == b.root_share.to_bits()
        && a.canopy_share.to_bits() == b.canopy_share.to_bits()
        && a.defence_share.to_bits() == b.defence_share.to_bits()
        && a.reproduction_share.to_bits() == b.reproduction_share.to_bits()
        && a.trigger_flowering == b.trigger_flowering
        && a.seed_size_bias.to_bits() == b.seed_size_bias.to_bits()
}

/// Component attached to a single leased organism while inhabited by one guest (`ADR-0014`).
///
/// Grants **zero** extra speed, strength, energy, sensory range, or immunity (`QD-PLAYRULES`
/// Rule 5); exempts the leased entity from cold LOD freeze/dehydration while holding only
/// the guest's normalized movement/action intents.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct InhabitedControl {
    pub guest_id: GuestId,
    pub cause_id: CauseId,
    pub organism_id: u64,
    pub kind: OrganismTargetKind,
    pub claimed_tick: u64,
    pub last_intent_tick: u64,
    pub creature: CreatureControlIntent,
    pub plant: Option<PlantControlIntent>,
}

/// Current lease/observation state of a connected guest slot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GuestLeaseStatus {
    Spectate,
    Inhabiting {
        organism_id: u64,
        kind: OrganismTargetKind,
        claimed_tick: u64,
    },
    SpectateAtDeathSite {
        dead_organism_id: u64,
        death_tick: u64,
        death_pos: [f32; 3],
    },
}

/// Plant inhabitation stub storing individual-plant allocation intents until `ADR-0015` flora
/// entities land (`D-PLANT-1(a)`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlantInhabitHook {
    pub organism_id: u64,
    pub guest_id: GuestId,
    pub cause_id: CauseId,
    pub claimed_tick: u64,
    pub last_intent_tick: u64,
    pub intent: Option<PlantControlIntent>,
}

/// Reason a guest lease ended (`ISLAND-D1(a)` / `QD-PLAYRULES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseReason {
    ExplicitRelease,
    DisconnectTimeout,
    HostKick,
    OrganismDied,
}

/// Pre-stamped `Copy` guest input record emitted on the authoritative simulation tick for
/// `record_world_record_tick_system`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickGuestInputRecord {
    pub tick: u64,
    pub cause_id: CauseId,
    pub intent: RecordedGuestIntent,
}

/// Host counters diagnosing lease arbitration, coalescing, releases, and rejections.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IslandInhabitCounters {
    pub claims_accepted: u64,
    pub claims_rejected_conflict: u64,
    pub claims_rejected_missing: u64,
    pub control_updates_applied: u64,
    pub coalesced_unchanged_controls: u64,
    pub releases_explicit: u64,
    pub releases_disconnect: u64,
    pub releases_kick: u64,
    pub deaths_reclaimed: u64,
    pub zero_carryover_reclaims: u64,
}

/// Host speed multiplier gate enforcing `QD-PLAYRULES` Rule 1 (`>1x` forbidden whenever guests are
/// connected).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct IslandTimeScaleGate {
    requested_multiplier: u32,
    effective_multiplier: u32,
    connected_guests: usize,
}

impl Default for IslandTimeScaleGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned when requesting faster-than-real-time speed with connected guests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandTimeScaleError {
    FasterThanRealTimeForbiddenWithGuests {
        connected_guests: usize,
        requested: u32,
    },
    ZeroMultiplierForbidden,
}

impl std::fmt::Display for IslandTimeScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FasterThanRealTimeForbiddenWithGuests {
                connected_guests,
                requested,
            } => write!(
                f,
                "faster-than-real-time multiplier {requested}x refused while {connected_guests} \
                 guest(s) are connected (QD-PLAYRULES Rule 1)"
            ),
            Self::ZeroMultiplierForbidden => write!(f, "time scale multiplier must be >= 1"),
        }
    }
}

impl std::error::Error for IslandTimeScaleError {}

impl IslandTimeScaleGate {
    pub const fn new() -> Self {
        Self {
            requested_multiplier: 1,
            effective_multiplier: 1,
            connected_guests: 0,
        }
    }

    pub const fn effective_multiplier(&self) -> u32 {
        self.effective_multiplier
    }

    pub const fn connected_guests(&self) -> usize {
        self.connected_guests
    }

    pub fn sync_guest_count(&mut self, connected_guests: usize) {
        self.connected_guests = connected_guests;
        self.effective_multiplier = if connected_guests == 0 {
            self.requested_multiplier.max(1)
        } else {
            1
        };
    }

    pub fn set_speed_multiplier(&mut self, multiplier: u32) -> Result<u32, IslandTimeScaleError> {
        if multiplier == 0 {
            return Err(IslandTimeScaleError::ZeroMultiplierForbidden);
        }
        if multiplier > 1 && self.connected_guests > 0 {
            self.requested_multiplier = 1;
            self.effective_multiplier = 1;
            return Err(
                IslandTimeScaleError::FasterThanRealTimeForbiddenWithGuests {
                    connected_guests: self.connected_guests,
                    requested: multiplier,
                },
            );
        }
        self.requested_multiplier = multiplier;
        self.effective_multiplier = multiplier;
        Ok(self.effective_multiplier)
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingDisconnect {
    guest_id: GuestId,
}

#[derive(Clone, Copy, Debug)]
struct GuestLeaseSlot {
    guest_id: GuestId,
    cause_id: CauseId,
    role: GuestRole,
    join_tick: u64,
    status: GuestLeaseStatus,
    leased_entity: Option<Entity>,
    last_heartbeat_tick: u64,
    last_known_pos: [f32; 3],
    last_recorded_creature: Option<CreatureControlIntent>,
    last_recorded_plant: Option<PlantControlIntent>,
    claimed_creature: Option<CreatureControlIntent>,
    last_intent_tick: u64,
    plant_hook_idx: Option<usize>,
}

/// Serializable checkpoint of a single active guest slot for `save -> load -> continue`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuestSlotCheckpoint {
    pub guest_id: GuestId,
    pub cause_id: CauseId,
    pub role: GuestRole,
    pub join_tick: u64,
    pub status: GuestLeaseStatus,
    pub last_heartbeat_tick: u64,
    pub last_known_pos: [f32; 3],
    pub last_recorded_creature: Option<CreatureControlIntent>,
    pub last_recorded_plant: Option<PlantControlIntent>,
    pub claimed_creature: Option<CreatureControlIntent>,
    pub last_intent_tick: u64,
}

/// Serializable host lease/tick checkpoint paired with [`LiveSnapshot`] (`save -> load ->
/// continue`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IslandInhabitCheckpoint {
    pub island_id: IslandId,
    pub current_tick: u64,
    pub keepalive_timeout_ticks: u64,
    pub slots: Vec<GuestSlotCheckpoint>,
    pub session_history: Vec<GuestSessionRecord>,
    pub plant_hooks: Vec<PlantInhabitHook>,
    pub next_synthetic_seq: u64,
    pub had_living_agents: bool,
    pub extinct_since_tick: Option<u64>,
}

/// Authoritative host ECS resource mediating exclusive creature/plant leases, disconnect/kick
/// releases, inhabited-death spectate fallback, and simulation-tick-stamped `WorldRecord` inputs.
#[derive(Resource, Clone, Debug)]
pub struct IslandInhabitState {
    island_id: IslandId,
    current_tick: u64,
    keepalive_timeout_ticks: u64,
    slots: Vec<Option<GuestLeaseSlot>>,
    session_history: Vec<GuestSessionRecord>,
    plant_hooks: Vec<PlantInhabitHook>,
    pending_disconnects: Vec<PendingDisconnect>,
    tick_recorded_inputs: Vec<TickGuestInputRecord>,
    next_synthetic_seq: u64,
    had_living_agents: bool,
    extinct_since_tick: Option<u64>,
    counters: IslandInhabitCounters,
}

impl IslandInhabitState {
    pub fn new(island_id: IslandId) -> Self {
        Self::with_keepalive_timeout(island_id, DEFAULT_ISLAND_KEEPALIVE_TIMEOUT_TICKS)
    }

    pub fn with_keepalive_timeout(island_id: IslandId, keepalive_timeout_ticks: u64) -> Self {
        let cap = MAX_ISLAND_GUESTS_CEILING as usize;
        Self {
            island_id,
            current_tick: 0,
            keepalive_timeout_ticks: keepalive_timeout_ticks.max(1),
            slots: vec![None; cap],
            session_history: Vec::with_capacity(cap * 2),
            plant_hooks: Vec::with_capacity(cap),
            pending_disconnects: Vec::with_capacity(cap),
            tick_recorded_inputs: Vec::with_capacity(DEFAULT_RECORDED_INTENT_SCRATCH_CAP),
            next_synthetic_seq: 1,
            had_living_agents: false,
            extinct_since_tick: None,
            counters: IslandInhabitCounters::default(),
        }
    }

    pub const fn island_id(&self) -> IslandId {
        self.island_id
    }

    pub const fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub const fn keepalive_timeout_ticks(&self) -> u64 {
        self.keepalive_timeout_ticks
    }

    pub fn set_keepalive_timeout_ticks(&mut self, timeout_ticks: u64) {
        self.keepalive_timeout_ticks = timeout_ticks.max(1);
    }

    pub const fn counters(&self) -> IslandInhabitCounters {
        self.counters
    }

    pub const fn is_extinct(&self) -> bool {
        self.extinct_since_tick.is_some()
    }

    pub const fn extinct_since_tick(&self) -> Option<u64> {
        self.extinct_since_tick
    }

    pub fn session_records(&self) -> &[GuestSessionRecord] {
        &self.session_history
    }

    pub fn tick_recorded_inputs(&self) -> &[TickGuestInputRecord] {
        &self.tick_recorded_inputs
    }

    pub fn plant_hook(&self, organism_id: u64) -> Option<PlantInhabitHook> {
        self.plant_hooks
            .iter()
            .copied()
            .find(|h| h.organism_id == organism_id)
    }

    pub fn active_guest_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn has_active_leases(&self) -> bool {
        self.slots
            .iter()
            .flatten()
            .any(|s| matches!(s.status, GuestLeaseStatus::Inhabiting { .. }))
    }

    pub fn guest_status(&self, guest_id: GuestId) -> Option<GuestLeaseStatus> {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.guest_id == guest_id)
            .map(|s| s.status)
    }

    pub fn leased_entity(&self, guest_id: GuestId) -> Option<Entity> {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.guest_id == guest_id)
            .and_then(|s| s.leased_entity)
    }

    /// Register or refresh a guest slot, recording its `join_tick` (`self.current_tick`) in
    /// `session_history`.
    pub fn admit_guest(
        &mut self,
        guest_id: GuestId,
        cause_id: CauseId,
        role: GuestRole,
    ) -> Result<(), &'static str> {
        if !guest_id.is_valid() {
            return Err("guest_id must be non-zero");
        }
        if !is_island_guest_cause(cause_id) {
            return Err("cause_id must lie inside the reserved island guest band");
        }
        for existing in self.slots.iter_mut().flatten() {
            if existing.guest_id == guest_id {
                existing.cause_id = cause_id;
                existing.role = role;
                existing.last_heartbeat_tick = self.current_tick;
                return Ok(());
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(GuestLeaseSlot {
                    guest_id,
                    cause_id,
                    role,
                    join_tick: self.current_tick,
                    status: GuestLeaseStatus::Spectate,
                    leased_entity: None,
                    last_heartbeat_tick: self.current_tick,
                    last_known_pos: [0.0; 3],
                    last_recorded_creature: None,
                    last_recorded_plant: None,
                    claimed_creature: None,
                    last_intent_tick: self.current_tick,
                    plant_hook_idx: None,
                });
                if self.session_history.len() < self.session_history.capacity() {
                    self.session_history.push(GuestSessionRecord {
                        guest_id,
                        cause_id,
                        role,
                        join_tick: self.current_tick,
                        leave_tick: None,
                    });
                }
                return Ok(());
            }
        }
        Err("island guest slot table is full")
    }

    /// Mark an active guest as disconnecting so its lease releases at the next tick boundary.
    pub fn request_disconnect(&mut self, guest_id: GuestId, _reason: ReleaseReason) -> bool {
        if !self.slots.iter().flatten().any(|s| s.guest_id == guest_id) {
            return false;
        }
        if self.pending_disconnects.len() >= self.pending_disconnects.capacity() {
            return false;
        }
        self.pending_disconnects
            .push(PendingDisconnect { guest_id });
        true
    }

    /// Snapshot an `IslandRecordHeader` with `genesis_phase` label (`"designed founders"` for `G1`)
    /// and current `guest_sessions`.
    pub fn to_record_header(
        &self,
        genesis_phase: GenesisPhase,
        prehistory_ticks: u64,
    ) -> IslandRecordHeader {
        let mut header =
            IslandRecordHeader::for_phase(self.island_id, genesis_phase, prehistory_ticks);
        header.guest_sessions = self.session_history.clone();
        header
    }

    /// Mirror this tick's accepted guest provenance records into a [`CausalLedger`].
    pub fn mirror_tick_to_ledger(&self, ledger: &mut CausalLedger) {
        for rec in &self.tick_recorded_inputs {
            let mechanism = match rec.intent.boundary {
                GuestIntentBoundary::Claim { .. } => "island.guest_claim",
                GuestIntentBoundary::Control(_) => "island.guest_creature_control",
                GuestIntentBoundary::PlantControl(_) => "island.guest_plant_control",
                GuestIntentBoundary::Release => "island.guest_release",
                GuestIntentBoundary::DisconnectRelease => "island.guest_disconnect_release",
                GuestIntentBoundary::Kick => "island.guest_kick",
                GuestIntentBoundary::Death { .. } => "island.guest_organism_death",
            };
            ledger.record(
                rec.cause_id,
                None,
                rec.tick,
                format!("guest:{}", rec.intent.guest_id.raw()),
                rec.intent.seq as f64,
                1.0,
                mechanism,
            );
        }
    }

    /// Serialize an `IslandInhabitCheckpoint` preserving exact simulation tick & leases across
    /// `save -> load -> continue`.
    pub fn checkpoint(&self) -> IslandInhabitCheckpoint {
        let slots = self
            .slots
            .iter()
            .flatten()
            .map(|s| GuestSlotCheckpoint {
                guest_id: s.guest_id,
                cause_id: s.cause_id,
                role: s.role,
                join_tick: s.join_tick,
                status: s.status,
                last_heartbeat_tick: s.last_heartbeat_tick,
                last_known_pos: s.last_known_pos,
                last_recorded_creature: s.last_recorded_creature,
                last_recorded_plant: s.last_recorded_plant,
                claimed_creature: s.claimed_creature,
                last_intent_tick: s.last_intent_tick,
            })
            .collect();
        IslandInhabitCheckpoint {
            island_id: self.island_id,
            current_tick: self.current_tick,
            keepalive_timeout_ticks: self.keepalive_timeout_ticks,
            slots,
            session_history: self.session_history.clone(),
            plant_hooks: self.plant_hooks.clone(),
            next_synthetic_seq: self.next_synthetic_seq,
            had_living_agents: self.had_living_agents,
            extinct_since_tick: self.extinct_since_tick,
        }
    }

    /// Restore `IslandInhabitState` and re-attach `InhabitedControl` onto restored matching entities
    /// inside `world`.
    pub fn restore_from_checkpoint(world: &mut World, cp: &IslandInhabitCheckpoint) {
        let mut agent_map: Vec<(u64, Entity, [f32; 3])> = Vec::new();
        {
            let mut q = world
                .query_filtered::<(Entity, Option<&AgentLineageId>, Option<&Position>), With<Agent>>(
                );
            for (entity, lineage, pos) in q.iter(world) {
                let id = stable_organism_id(lineage, entity);
                let p = pos.map_or([0.0; 3], |v| [v.0.x, v.0.y, v.0.z]);
                agent_map.push((id, entity, p));
            }
        }

        let mut state = Self::with_keepalive_timeout(cp.island_id, cp.keepalive_timeout_ticks);
        state.current_tick = cp.current_tick;
        state.session_history = cp.session_history.clone();
        if state.session_history.capacity() < (MAX_ISLAND_GUESTS_CEILING as usize) * 2 {
            let cap = (MAX_ISLAND_GUESTS_CEILING as usize) * 2;
            state
                .session_history
                .reserve(cap - state.session_history.len());
        }
        state.plant_hooks = cp.plant_hooks.clone();
        state.next_synthetic_seq = cp.next_synthetic_seq;
        state.had_living_agents = cp.had_living_agents;
        state.extinct_since_tick = cp.extinct_since_tick;

        for (idx, sc) in cp.slots.iter().enumerate() {
            if idx >= state.slots.len() {
                break;
            }
            let mut leased_entity = None;
            let mut plant_hook_idx = None;
            let mut status = sc.status;
            let mut last_known_pos = sc.last_known_pos;
            if let GuestLeaseStatus::Inhabiting {
                organism_id,
                kind,
                claimed_tick,
            } = sc.status
            {
                match kind {
                    OrganismTargetKind::Animal => {
                        if let Some((_, ent, p)) = agent_map
                            .iter()
                            .find(|(id, _, _)| *id == organism_id)
                            .copied()
                        {
                            leased_entity = Some(ent);
                            last_known_pos = p;
                            let creature = sc.claimed_creature.unwrap_or(NEUTRAL_CREATURE_CONTROL);
                            world.entity_mut(ent).insert(InhabitedControl {
                                guest_id: sc.guest_id,
                                cause_id: sc.cause_id,
                                organism_id,
                                kind,
                                claimed_tick,
                                last_intent_tick: sc.last_intent_tick,
                                creature,
                                plant: None,
                            });
                        } else {
                            status = GuestLeaseStatus::Spectate;
                        }
                    }
                    OrganismTargetKind::IndividualPlant => {
                        plant_hook_idx = state.plant_hooks.iter().position(|h| {
                            h.organism_id == organism_id && h.guest_id == sc.guest_id
                        });
                    }
                }
            }
            state.slots[idx] = Some(GuestLeaseSlot {
                guest_id: sc.guest_id,
                cause_id: sc.cause_id,
                role: sc.role,
                join_tick: sc.join_tick,
                status,
                leased_entity,
                last_heartbeat_tick: sc.last_heartbeat_tick,
                last_known_pos,
                last_recorded_creature: sc.last_recorded_creature,
                last_recorded_plant: sc.last_recorded_plant,
                claimed_creature: sc.claimed_creature,
                last_intent_tick: sc.last_intent_tick,
                plant_hook_idx,
            });
        }

        world.insert_resource(state);
    }

    fn push_recorded_input(
        &mut self,
        tick: u64,
        cause_id: CauseId,
        guest_id: GuestId,
        seq: u64,
        boundary: GuestIntentBoundary,
    ) {
        if self.tick_recorded_inputs.len() < self.tick_recorded_inputs.capacity() {
            self.tick_recorded_inputs.push(TickGuestInputRecord {
                tick,
                cause_id,
                intent: RecordedGuestIntent {
                    guest_id,
                    seq,
                    boundary,
                },
            });
        }
    }

    fn alloc_synthetic_seq(&mut self) -> u64 {
        let s = self.next_synthetic_seq;
        self.next_synthetic_seq = self.next_synthetic_seq.saturating_add(1);
        s
    }

    fn mark_session_left(&mut self, guest_id: GuestId, cause_id: CauseId, sim_tick: u64) {
        for rec in self.session_history.iter_mut().rev() {
            if rec.guest_id == guest_id && rec.cause_id == cause_id && rec.leave_tick.is_none() {
                rec.leave_tick = Some(sim_tick);
                break;
            }
        }
    }

    fn organism_claimed_by_other(
        &self,
        organism_id: u64,
        kind: OrganismTargetKind,
        caller: GuestId,
    ) -> bool {
        self.slots.iter().flatten().any(|slot| {
            slot.guest_id != caller
                && matches!(
                    slot.status,
                    GuestLeaseStatus::Inhabiting {
                        organism_id: held_id,
                        kind: held_kind,
                        ..
                    } if held_id == organism_id && held_kind == kind
                )
        })
    }

    fn find_slot_mut(&mut self, guest_id: GuestId) -> Option<&mut GuestLeaseSlot> {
        self.slots
            .iter_mut()
            .flatten()
            .find(|s| s.guest_id == guest_id)
    }

    fn take_slot(&mut self, guest_id: GuestId) -> Option<GuestLeaseSlot> {
        for slot in &mut self.slots {
            if slot.as_ref().is_some_and(|s| s.guest_id == guest_id) {
                return slot.take();
            }
        }
        None
    }
}

fn release_entity_control(world: &mut World, entity: Entity) {
    if let Some(mut ent) = world.get_entity_mut(entity) {
        ent.remove::<InhabitedControl>();
        if let Some(mut gates) = ent.get_mut::<ActionGates>() {
            *gates = ActionGates::OPEN;
        }
    }
}

fn release_plant_hook(state: &mut IslandInhabitState, organism_id: u64, guest_id: GuestId) {
    if let Some(pos) = state
        .plant_hooks
        .iter()
        .position(|h| h.organism_id == organism_id && h.guest_id == guest_id)
    {
        state.plant_hooks.swap_remove(pos);
    }
}

/// Exclusive pre-sensory system wired right after [`crate::core::island::drain_guest_intents_system`]
/// inside `build_tick_schedule` (`Card I2`).
///
/// Stamps accepted intents strictly by the authoritative simulation tick (`IslandInhabitState::current_tick`),
/// enforces exclusive 1-guest leases (`ISLAND-D2(a)`), performs change-only coalescing before recording
/// into `WorldRecord`, and executes zero heap allocations on steady-state ticks.
#[allow(clippy::type_complexity)]
pub fn apply_island_inhabit_system(
    world: &mut World,
    agent_query: &mut QueryState<
        (
            Entity,
            Option<&AgentLineageId>,
            &HomeostaticState,
            Option<&Position>,
        ),
        With<Agent>,
    >,
) {
    if !world.contains_resource::<IslandInhabitState>() {
        return;
    }

    let sim_tick = {
        let mut state = world.resource_mut::<IslandInhabitState>();
        state.current_tick = state.current_tick.saturating_add(1);
        state.tick_recorded_inputs.clear();
        state.current_tick
    };

    if matches!(
        world.get_resource::<ObserverPolicy>(),
        Some(ObserverPolicy::Absent)
    ) {
        if let Some(mut state) = world.get_resource_mut::<IslandInhabitState>() {
            state.pending_disconnects.clear();
        }
        return;
    }

    let mut state = world
        .remove_resource::<IslandInhabitState>()
        .expect("checked contains_resource::<IslandInhabitState>");
    let drained_res = world.remove_resource::<DrainedGuestIntents>();

    // 1. Explicit disconnect requests (`ISLAND-D1(a)` immediate release at tick boundary).
    while let Some(disc) = state.pending_disconnects.pop() {
        if let Some(removed) = state.take_slot(disc.guest_id) {
            if let Some(ent) = removed.leased_entity {
                release_entity_control(world, ent);
            }
            if let GuestLeaseStatus::Inhabiting {
                organism_id,
                kind: OrganismTargetKind::IndividualPlant,
                ..
            } = removed.status
            {
                release_plant_hook(&mut state, organism_id, disc.guest_id);
            }
            state.mark_session_left(disc.guest_id, removed.cause_id, sim_tick);
            let seq = state.alloc_synthetic_seq();
            state.push_recorded_input(
                sim_tick,
                removed.cause_id,
                disc.guest_id,
                seq,
                GuestIntentBoundary::DisconnectRelease,
            );
            state.counters.releases_disconnect =
                state.counters.releases_disconnect.saturating_add(1);
            if let Some(mut policy) = world.get_resource_mut::<IslandGuestPolicy>() {
                policy.kick_guest(disc.guest_id);
            }
        }
    }

    // 2. Host moderation revocations (`KickGuest`) drained from `DrainedGuestIntents`.
    if let Some(ref drained) = drained_res {
        for action in drained.moderation() {
            let HostModerationAction::KickGuest {
                guest_id,
                cause_id: _,
            } = *action;
            if let Some(removed) = state.take_slot(guest_id) {
                if let Some(ent) = removed.leased_entity {
                    release_entity_control(world, ent);
                }
                if let GuestLeaseStatus::Inhabiting {
                    organism_id,
                    kind: OrganismTargetKind::IndividualPlant,
                    ..
                } = removed.status
                {
                    release_plant_hook(&mut state, organism_id, guest_id);
                }
                state.mark_session_left(guest_id, removed.cause_id, sim_tick);
                let seq = state.alloc_synthetic_seq();
                state.push_recorded_input(
                    sim_tick,
                    removed.cause_id,
                    guest_id,
                    seq,
                    GuestIntentBoundary::Kick,
                );
                state.counters.releases_kick = state.counters.releases_kick.saturating_add(1);
                if let Some(mut policy) = world.get_resource_mut::<IslandGuestPolicy>() {
                    policy.kick_guest(guest_id);
                }
            }
        }
    }

    // 3. Process drained guest intents (`Claim`, `Release`, `CreatureControl`, `PlantControl`)
    // stamping strictly with `sim_tick` (`MSG_20261124_192500` §4 — ignoring `Local<u64> apply_tick`).
    if let Some(ref drained) = drained_res {
        for stamped in drained.intents() {
            let guest_id = stamped.guest_id;
            let cause_id = stamped.cause_id;
            let seq = stamped.seq;

            match stamped.intent {
                GuestIntent::ClaimOrganism { organism_id, kind } => {
                    if state.organism_claimed_by_other(organism_id, kind, guest_id) {
                        state.counters.claims_rejected_conflict =
                            state.counters.claims_rejected_conflict.saturating_add(1);
                        continue;
                    }
                    let Some(slot_role) = state
                        .slots
                        .iter()
                        .flatten()
                        .find(|s| s.guest_id == guest_id)
                        .map(|s| s.role)
                    else {
                        continue;
                    };
                    if slot_role != GuestRole::Inhabit {
                        continue;
                    }

                    // Zero carryover: detach any prior creature/plant lease cleanly first.
                    if let Some(slot) = state.find_slot_mut(guest_id) {
                        if let Some(prev_ent) = slot.leased_entity.take() {
                            release_entity_control(world, prev_ent);
                        }
                        if let GuestLeaseStatus::Inhabiting {
                            organism_id: prev_plant_id,
                            kind: OrganismTargetKind::IndividualPlant,
                            ..
                        } = slot.status
                        {
                            slot.plant_hook_idx = None;
                            release_plant_hook(&mut state, prev_plant_id, guest_id);
                        }
                    }

                    match kind {
                        OrganismTargetKind::Animal => {
                            let mut target: Option<(Entity, [f32; 3])> = None;
                            for (entity, lineage, homeo, pos) in agent_query.iter(world) {
                                if homeo.energy <= 0.0 || homeo.hydration <= 0.0 {
                                    continue;
                                }
                                if stable_organism_id(lineage, entity) == organism_id {
                                    let p = pos.map_or([0.0; 3], |v| [v.0.x, v.0.y, v.0.z]);
                                    target = Some((entity, p));
                                    break;
                                }
                            }
                            let Some((target_entity, target_pos)) = target else {
                                state.counters.claims_rejected_missing =
                                    state.counters.claims_rejected_missing.saturating_add(1);
                                continue;
                            };
                            world
                                .entity_mut(target_entity)
                                .insert(InhabitedControl {
                                    guest_id,
                                    cause_id,
                                    organism_id,
                                    kind: OrganismTargetKind::Animal,
                                    claimed_tick: sim_tick,
                                    last_intent_tick: sim_tick,
                                    creature: NEUTRAL_CREATURE_CONTROL,
                                    plant: None,
                                })
                                .remove::<crate::core::aggregate_population::DormancyWatch>();
                            if let Some(slot) = state.find_slot_mut(guest_id) {
                                slot.leased_entity = Some(target_entity);
                                slot.status = GuestLeaseStatus::Inhabiting {
                                    organism_id,
                                    kind: OrganismTargetKind::Animal,
                                    claimed_tick: sim_tick,
                                };
                                slot.last_heartbeat_tick = sim_tick;
                                slot.last_known_pos = target_pos;
                                slot.last_recorded_creature = None;
                                slot.last_recorded_plant = None;
                                slot.claimed_creature = Some(NEUTRAL_CREATURE_CONTROL);
                                slot.last_intent_tick = sim_tick;
                            }
                            state.counters.claims_accepted =
                                state.counters.claims_accepted.saturating_add(1);
                            state.push_recorded_input(
                                sim_tick,
                                cause_id,
                                guest_id,
                                seq,
                                GuestIntentBoundary::Claim {
                                    organism_id,
                                    kind: OrganismTargetKind::Animal,
                                },
                            );
                        }
                        OrganismTargetKind::IndividualPlant => {
                            if state.plant_hooks.len() >= state.plant_hooks.capacity() {
                                continue;
                            }
                            let hook_idx = state.plant_hooks.len();
                            state.plant_hooks.push(PlantInhabitHook {
                                organism_id,
                                guest_id,
                                cause_id,
                                claimed_tick: sim_tick,
                                last_intent_tick: sim_tick,
                                intent: None,
                            });
                            if let Some(slot) = state.find_slot_mut(guest_id) {
                                slot.leased_entity = None;
                                slot.plant_hook_idx = Some(hook_idx);
                                slot.status = GuestLeaseStatus::Inhabiting {
                                    organism_id,
                                    kind: OrganismTargetKind::IndividualPlant,
                                    claimed_tick: sim_tick,
                                };
                                slot.last_heartbeat_tick = sim_tick;
                                slot.last_recorded_creature = None;
                                slot.last_recorded_plant = None;
                                slot.claimed_creature = None;
                                slot.last_intent_tick = sim_tick;
                            }
                            state.counters.claims_accepted =
                                state.counters.claims_accepted.saturating_add(1);
                            state.push_recorded_input(
                                sim_tick,
                                cause_id,
                                guest_id,
                                seq,
                                GuestIntentBoundary::Claim {
                                    organism_id,
                                    kind: OrganismTargetKind::IndividualPlant,
                                },
                            );
                        }
                    }
                }
                GuestIntent::ReleaseOrganism => {
                    let mut released_any = false;
                    let mut plant_to_remove: Option<u64> = None;
                    if let Some(slot) = state.find_slot_mut(guest_id) {
                        slot.last_heartbeat_tick = sim_tick;
                        if let Some(ent) = slot.leased_entity.take() {
                            release_entity_control(world, ent);
                            released_any = true;
                        }
                        if let GuestLeaseStatus::Inhabiting {
                            organism_id,
                            kind: OrganismTargetKind::IndividualPlant,
                            ..
                        } = slot.status
                        {
                            plant_to_remove = Some(organism_id);
                            slot.plant_hook_idx = None;
                            released_any = true;
                        }
                        if released_any {
                            slot.status = GuestLeaseStatus::Spectate;
                            slot.last_recorded_creature = None;
                            slot.last_recorded_plant = None;
                            slot.claimed_creature = None;
                        }
                    }
                    if let Some(pid) = plant_to_remove {
                        release_plant_hook(&mut state, pid, guest_id);
                    }
                    if released_any {
                        state.counters.releases_explicit =
                            state.counters.releases_explicit.saturating_add(1);
                        state.push_recorded_input(
                            sim_tick,
                            cause_id,
                            guest_id,
                            seq,
                            GuestIntentBoundary::Release,
                        );
                    }
                }
                GuestIntent::CreatureControl(ctrl) => {
                    let mut emit_change = false;
                    let mut target_ent = None;
                    if let Some(slot) = state.find_slot_mut(guest_id) {
                        slot.last_heartbeat_tick = sim_tick;
                        if let GuestLeaseStatus::Inhabiting {
                            kind: OrganismTargetKind::Animal,
                            ..
                        } = slot.status
                        {
                            target_ent = slot.leased_entity;
                            slot.claimed_creature = Some(ctrl);
                            slot.last_intent_tick = sim_tick;
                            let changed = match slot.last_recorded_creature {
                                Some(ref prev) => !creature_control_bits_eq(prev, &ctrl),
                                None => true,
                            };
                            if changed {
                                slot.last_recorded_creature = Some(ctrl);
                                emit_change = true;
                            } else {
                                state.counters.coalesced_unchanged_controls = state
                                    .counters
                                    .coalesced_unchanged_controls
                                    .saturating_add(1);
                            }
                        }
                    }
                    if let Some(ent) = target_ent {
                        if let Some(mut inhabited) = world.get_mut::<InhabitedControl>(ent) {
                            inhabited.creature = ctrl;
                            inhabited.last_intent_tick = sim_tick;
                            state.counters.control_updates_applied =
                                state.counters.control_updates_applied.saturating_add(1);
                        }
                        if emit_change {
                            state.push_recorded_input(
                                sim_tick,
                                cause_id,
                                guest_id,
                                seq,
                                GuestIntentBoundary::Control(ctrl),
                            );
                        }
                    }
                }
                GuestIntent::PlantControl(ctrl) => {
                    let mut emit_change = false;
                    let mut organism_id_opt = None;
                    if let Some(slot) = state.find_slot_mut(guest_id) {
                        slot.last_heartbeat_tick = sim_tick;
                        if let GuestLeaseStatus::Inhabiting {
                            organism_id,
                            kind: OrganismTargetKind::IndividualPlant,
                            ..
                        } = slot.status
                        {
                            organism_id_opt = Some(organism_id);
                            slot.last_intent_tick = sim_tick;
                            let changed = match slot.last_recorded_plant {
                                Some(ref prev) => !plant_control_bits_eq(prev, &ctrl),
                                None => true,
                            };
                            if changed {
                                slot.last_recorded_plant = Some(ctrl);
                                emit_change = true;
                            } else {
                                state.counters.coalesced_unchanged_controls = state
                                    .counters
                                    .coalesced_unchanged_controls
                                    .saturating_add(1);
                            }
                        }
                    }
                    if let Some(organism_id) = organism_id_opt {
                        if let Some(hook) = state
                            .plant_hooks
                            .iter_mut()
                            .find(|h| h.organism_id == organism_id && h.guest_id == guest_id)
                        {
                            hook.intent = Some(ctrl);
                            hook.last_intent_tick = sim_tick;
                            state.counters.control_updates_applied =
                                state.counters.control_updates_applied.saturating_add(1);
                        }
                        if emit_change {
                            state.push_recorded_input(
                                sim_tick,
                                cause_id,
                                guest_id,
                                seq,
                                GuestIntentBoundary::PlantControl(ctrl),
                            );
                        }
                    }
                }
            }
        }
    }

    // 4. Keepalive timeout check (`ISLAND-D1(a)` immediate release without zombie stall).
    let timeout = state.keepalive_timeout_ticks;
    for slot_idx in 0..state.slots.len() {
        let should_timeout = state.slots[slot_idx].as_ref().is_some_and(|s| {
            matches!(s.status, GuestLeaseStatus::Inhabiting { .. })
                && sim_tick.saturating_sub(s.last_heartbeat_tick) >= timeout
        });
        if should_timeout {
            if let Some(removed) = state.slots[slot_idx].take() {
                if let Some(ent) = removed.leased_entity {
                    release_entity_control(world, ent);
                }
                if let GuestLeaseStatus::Inhabiting {
                    organism_id,
                    kind: OrganismTargetKind::IndividualPlant,
                    ..
                } = removed.status
                {
                    release_plant_hook(&mut state, organism_id, removed.guest_id);
                }
                state.mark_session_left(removed.guest_id, removed.cause_id, sim_tick);
                let seq = state.alloc_synthetic_seq();
                state.push_recorded_input(
                    sim_tick,
                    removed.cause_id,
                    removed.guest_id,
                    seq,
                    GuestIntentBoundary::DisconnectRelease,
                );
                state.counters.releases_disconnect =
                    state.counters.releases_disconnect.saturating_add(1);
                if let Some(mut policy) = world.get_resource_mut::<IslandGuestPolicy>() {
                    policy.kick_guest(removed.guest_id);
                }
            }
        }
    }

    // Sync TimeScaleGate if present (`QD-PLAYRULES` Rule 1).
    let connected_now = state.active_guest_count();
    if let Some(mut gate) = world.get_resource_mut::<IslandTimeScaleGate>() {
        gate.sync_guest_count(connected_now);
    }

    if let Some(drained) = drained_res {
        world.insert_resource(drained);
    }
    world.insert_resource(state);
}

/// Post-mortality/reproduction reconciliation system running directly before
/// `record_world_record_tick_system` (`Card I2`).
///
/// Detects when an inhabited creature despawned via `ReclaimAndDespawnAgentCommand` (no respawn,
/// `V4` carcass), reverts its guest slot to `SpectateAtDeathSite` with zero stat carryover, emits
/// `GuestIntentBoundary::Death` at the exact simulation tick, and records total island extinction
/// without auto-reviving (`QD-PLAYRULES` Rules 3 & 4).
pub fn reconcile_inhabited_deaths_system(
    mut island_state: Option<ResMut<IslandInhabitState>>,
    agents: Query<&Position, With<Agent>>,
) {
    let Some(ref mut state) = island_state else {
        return;
    };
    let sim_tick = state.current_tick;
    let living_count = agents.iter().len();
    if living_count > 0 {
        state.had_living_agents = true;
        state.extinct_since_tick = None;
    } else if state.had_living_agents && state.extinct_since_tick.is_none() {
        state.extinct_since_tick = Some(sim_tick);
    }

    let mut next_seq = state.next_synthetic_seq;
    let mut pending_death_idx: [Option<(usize, GuestId, CauseId, u64, [f32; 3])>;
        MAX_ISLAND_GUESTS_CEILING as usize] = [None; MAX_ISLAND_GUESTS_CEILING as usize];
    let mut pending_count = 0usize;

    for (idx, slot_opt) in state.slots.iter_mut().enumerate() {
        let Some(slot) = slot_opt.as_mut() else {
            continue;
        };
        if let GuestLeaseStatus::Inhabiting {
            organism_id,
            kind: OrganismTargetKind::Animal,
            ..
        } = slot.status
        {
            if let Some(ent) = slot.leased_entity {
                if let Ok(pos) = agents.get(ent) {
                    slot.last_known_pos = [pos.0.x, pos.0.y, pos.0.z];
                } else {
                    let death_pos = slot.last_known_pos;
                    slot.leased_entity = None;
                    slot.last_recorded_creature = None;
                    slot.claimed_creature = None;
                    slot.status = GuestLeaseStatus::SpectateAtDeathSite {
                        dead_organism_id: organism_id,
                        death_tick: sim_tick,
                        death_pos,
                    };
                    if pending_count < pending_death_idx.len() {
                        pending_death_idx[pending_count] =
                            Some((idx, slot.guest_id, slot.cause_id, organism_id, death_pos));
                        pending_count += 1;
                    }
                }
            }
        }
    }

    for entry in &pending_death_idx[..pending_count] {
        if let Some((_, guest_id, cause_id, organism_id, pos)) = *entry {
            let seq = next_seq;
            next_seq = next_seq.saturating_add(1);
            state.counters.deaths_reclaimed = state.counters.deaths_reclaimed.saturating_add(1);
            state.counters.zero_carryover_reclaims =
                state.counters.zero_carryover_reclaims.saturating_add(1);
            state.push_recorded_input(
                sim_tick,
                cause_id,
                guest_id,
                seq,
                GuestIntentBoundary::Death {
                    organism_id,
                    death_x: pos[0],
                    death_y: pos[1],
                    death_z: pos[2],
                },
            );
        }
    }
    state.next_synthetic_seq = next_seq;
}

/// Construct a verified new-era [`WorldRecordHeader`] after total island extinction, chaining
/// `era_index + 1` and `prev_era_tail_hash` (`QD-PLAYRULES` Rule 4).
pub fn open_new_era_header(
    prev_record: &WorldRecord,
    world: &mut World,
    next_run_id: String,
    next_seed: u64,
    genesis_phase: GenesisPhase,
    prehistory_ticks: u64,
) -> Result<WorldRecordHeader, WorldRecordError> {
    let island_id = prev_record
        .header
        .island
        .as_ref()
        .map(|i| i.island_id)
        .ok_or_else(|| {
            WorldRecordError::InvalidIslandHeader(
                "cannot open new island era from non-island WorldRecord".into(),
            )
        })?;
    let prev_tail = prev_record
        .segments
        .last()
        .map(|s| s.hash)
        .unwrap_or(prev_record.header_hash);
    if let Some(mut identity) = world.get_resource_mut::<WorldIdentity>() {
        identity.genesis_phase = Some(genesis_phase);
    }
    let island_header = IslandRecordHeader::for_phase(island_id, genesis_phase, prehistory_ticks);
    let mut header = build_island_header_from_world(
        world,
        next_seed,
        prev_record.header.segment_ticks,
        true,
        prev_record.header.initial_conditions.clone(),
        prev_record.header.omissions.clone(),
        island_header,
    );
    header.run_id = next_run_id;
    header.era_index = prev_record.header.era_index.saturating_add(1);
    header.prev_era_tail_hash = Some(prev_tail);
    validate_header(&header)?;
    Ok(header)
}

/// Outcome returned by [`run_g1_prehistory_warmup`] (`D-START-2(b)`).
#[derive(Clone, Debug)]
pub struct G1PrehistoryResult {
    pub prehistory_ticks: u64,
    pub living_agents_at_open: usize,
    pub end_checksum: u32,
    pub genesis_label: &'static str,
}

/// Initialize headless `G1` (`"designed founders"`) prehistory on an empty island (`0` guests) and
/// step `prehistory_ticks` inside the same `WorldRecord` before opening to guests (`D-START-2(b)`).
pub fn run_g1_prehistory_warmup(
    adapter: &mut LiveExperimentAdapter,
    island_id: IslandId,
    prehistory_ticks: u64,
    segment_ticks: u64,
    writer_config: Option<WorldRecordWriterConfig>,
) -> Result<G1PrehistoryResult, WorldRecordError> {
    {
        let mut world = adapter.world();
        if let Some(mut id) = world.get_resource_mut::<WorldIdentity>() {
            id.genesis_phase = Some(GenesisPhase::G1);
        }
        let mut gate = IslandTimeScaleGate::new();
        gate.sync_guest_count(0);
        let _ = gate.set_speed_multiplier(8);
        world.insert_resource(gate);
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(SharedGuestIntents::new(island_id));
        world.insert_resource(DrainedGuestIntents::default());
        world.insert_resource(IslandInhabitState::new(island_id));

        let island_header =
            IslandRecordHeader::for_phase(island_id, GenesisPhase::G1, prehistory_ticks);
        let header = build_island_header_from_world(
            &mut world,
            adapter.seed(),
            segment_ticks.max(1),
            true,
            Vec::new(),
            Vec::new(),
            island_header,
        );
        let recorder =
            WorldRecordRecorder::from_world_with_header(&mut world, header, writer_config)?;
        world.insert_resource(recorder);
    }

    for _ in 0..prehistory_ticks {
        adapter.run_schedule_once();
    }

    let living_agents_at_open = {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        q.iter(&world).len()
    };

    Ok(G1PrehistoryResult {
        prehistory_ticks,
        living_agents_at_open,
        end_checksum: adapter.checksum(),
        genesis_label: GenesisPhase::G1.display_label(),
    })
}

/// Summary returned by [`replay_island_world_record`].
#[derive(Clone, Debug)]
pub struct IslandReplayReport {
    pub segments_replayed: usize,
    pub last_tick_replayed: u64,
    pub end_checksum: u32,
    pub ledger: CausalLedger,
    pub guest_inputs_replayed: usize,
    pub deaths_matched: usize,
}

/// Offline, networking-free `WorldRecord` replay harness for shared-island sessions (`ADR-0014`
/// Gate #3). Supports full replay from genesis or post-checkpoint `save -> load -> continue` replay
/// when given `resume_from`.
pub fn replay_island_world_record(
    record: &WorldRecord,
    laws: &WorldLawSet,
    initial: &InitialConditionSet,
    resume_from: Option<(&LiveSnapshot, &IslandInhabitCheckpoint)>,
) -> Result<IslandReplayReport, String> {
    let verified = verify_record(record).map_err(|e| format!("verify_record failed: {e}"))?;
    if !verified.replayable {
        return Err("cannot deterministically replay non-replayable WorldRecord".into());
    }
    let Some(ref island_header) = record.header.island else {
        return Err("WorldRecordHeader.island missing on island replay".into());
    };
    let manifest_phase = effective_genesis_phase_hint(island_header);

    let (mut adapter, start_tick, start_segment_idx) = match resume_from {
        Some((snap, cp)) => {
            let resumed = LiveExperimentAdapter::from_snapshot(snap)
                .map_err(|e: ExperimentError| format!("from_snapshot failed: {e}"))?;
            let resume_tick = snap
                .saved
                .experiment
                .as_ref()
                .map(|e| e.clock_tick)
                .ok_or_else(|| "snapshot missing experiment clock_tick".to_string())?;
            let next_tick = resume_tick.saturating_add(1);
            let seg_idx = record
                .segments
                .iter()
                .position(|s| s.body.tick_start == next_tick)
                .ok_or_else(|| {
                    format!("no WorldRecord segment begins at resumed tick {next_tick}")
                })?;
            {
                let mut world = resumed.world();
                if let Some(mut id) = world.get_resource_mut::<WorldIdentity>() {
                    id.genesis_phase = Some(manifest_phase);
                }
                world.insert_resource(ObserverPolicy::Inhabit {
                    cause_id: CAUSE_OBSERVER,
                });
                let shared = SharedGuestIntents::new(island_header.island_id);
                world.insert_resource(shared);
                world.insert_resource(DrainedGuestIntents::default());
                IslandInhabitState::restore_from_checkpoint(&mut world, cp);
                world
                    .resource_mut::<IslandInhabitState>()
                    .set_keepalive_timeout_ticks(u64::MAX / 4);
            }
            (resumed, next_tick, seg_idx)
        }
        None => {
            let built = LiveExperimentAdapter::from_manifest(
                laws,
                initial,
                &[],
                record.header.seed,
                (16, 16),
                0,
            )
            .map_err(|e| format!("from_manifest failed: {e}"))?;
            {
                let mut world = built.world();
                if let Some(mut id) = world.get_resource_mut::<WorldIdentity>() {
                    id.genesis_phase = Some(manifest_phase);
                }
                world.insert_resource(ObserverPolicy::Inhabit {
                    cause_id: CAUSE_OBSERVER,
                });
                world.insert_resource(SharedGuestIntents::new(island_header.island_id));
                world.insert_resource(DrainedGuestIntents::default());
                let mut state = IslandInhabitState::with_keepalive_timeout(
                    island_header.island_id,
                    u64::MAX / 4,
                );
                for s in &island_header.guest_sessions {
                    let _ = state.admit_guest(s.guest_id, s.cause_id, s.role);
                }
                world.insert_resource(state);
            }
            if !record.segments.is_empty()
                && built.checksum() != record.header.initial_world_checksum
            {
                return Err(format!(
                    "initial checksum mismatch: expected {}, got {}",
                    record.header.initial_world_checksum,
                    built.checksum()
                ));
            }
            (built, 1u64, 0usize)
        }
    };

    let mut ledger = CausalLedger::new();
    let mut guest_inputs_replayed = 0usize;
    let mut deaths_matched = 0usize;
    let mut segments_replayed = 0usize;
    let mut last_tick = start_tick.saturating_sub(1);

    for seg in &record.segments[start_segment_idx..] {
        for t in seg.body.tick_start..=seg.body.tick_end {
            let mut expected_deaths: Vec<(GuestId, u64)> = Vec::new();
            {
                let mut world = adapter.world();
                let shared = world.resource::<SharedGuestIntents>().clone();
                let mut state = world.resource_mut::<IslandInhabitState>();
                for input in seg.body.inputs.iter().filter(|i| i.tick == t) {
                    let InputKind::GuestIntent(ref rec) = input.kind else {
                        continue;
                    };
                    if state.guest_status(rec.guest_id).is_none()
                        && !matches!(rec.boundary, GuestIntentBoundary::Death { .. })
                    {
                        let _ = state.admit_guest(rec.guest_id, input.cause_id, GuestRole::Inhabit);
                    }
                    match rec.boundary {
                        GuestIntentBoundary::Claim { organism_id, kind } => {
                            shared
                                .try_enqueue(
                                    rec.guest_id,
                                    input.cause_id,
                                    GuestIntent::ClaimOrganism { organism_id, kind },
                                )
                                .map_err(|e| format!("replay Claim enqueue failed: {e:?}"))?;
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::Control(ctrl) => {
                            shared
                                .try_enqueue(
                                    rec.guest_id,
                                    input.cause_id,
                                    GuestIntent::CreatureControl(ctrl),
                                )
                                .map_err(|e| format!("replay Control enqueue failed: {e:?}"))?;
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::PlantControl(ctrl) => {
                            shared
                                .try_enqueue(
                                    rec.guest_id,
                                    input.cause_id,
                                    GuestIntent::PlantControl(ctrl),
                                )
                                .map_err(|e| {
                                    format!("replay PlantControl enqueue failed: {e:?}")
                                })?;
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::Release => {
                            shared
                                .try_enqueue(
                                    rec.guest_id,
                                    input.cause_id,
                                    GuestIntent::ReleaseOrganism,
                                )
                                .map_err(|e| format!("replay Release enqueue failed: {e:?}"))?;
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::DisconnectRelease => {
                            if !state
                                .request_disconnect(rec.guest_id, ReleaseReason::DisconnectTimeout)
                            {
                                return Err(format!(
                                    "replay DisconnectRelease failed at tick {t} for guest {}",
                                    rec.guest_id.raw()
                                ));
                            }
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::Kick => {
                            shared
                                .enqueue_moderation(HostModerationAction::KickGuest {
                                    guest_id: rec.guest_id,
                                    cause_id: input.cause_id,
                                })
                                .map_err(|e| format!("replay Kick enqueue failed: {e:?}"))?;
                            guest_inputs_replayed += 1;
                        }
                        GuestIntentBoundary::Death { organism_id, .. } => {
                            expected_deaths.push((rec.guest_id, organism_id));
                        }
                    }
                }
            }

            adapter.run_schedule_once();
            last_tick = t;

            {
                let world = adapter.world();
                let state = world.resource::<IslandInhabitState>();
                state.mirror_tick_to_ledger(&mut ledger);
                for &(guest_id, exp_org_id) in &expected_deaths {
                    let matched = state.tick_recorded_inputs().iter().any(|r| {
                        r.tick == t
                            && r.intent.guest_id == guest_id
                            && matches!(
                                r.intent.boundary,
                                GuestIntentBoundary::Death { organism_id, .. }
                                    if organism_id == exp_org_id
                            )
                    });
                    if !matched {
                        return Err(format!(
                            "replay missed inhabited Death boundary at tick {t} for guest {} organism {exp_org_id}",
                            guest_id.raw()
                        ));
                    }
                    deaths_matched += 1;
                }
            }
        }

        let got = adapter.checksum();
        if got != seg.body.end_checksum {
            return Err(format!(
                "replay checksum mismatch at segment {} (tick {}): expected {}, got {}",
                seg.body.index, seg.body.tick_end, seg.body.end_checksum, got
            ));
        }
        segments_replayed += 1;
    }

    // Touch `effective_genesis_phase` to ensure manifest compatibility helpers remain wired.
    let _ = effective_genesis_phase
        as fn(&crate::core::experiment::ExperimentManifest) -> Option<GenesisPhase>;

    Ok(IslandReplayReport {
        segments_replayed,
        last_tick_replayed: last_tick,
        end_checksum: adapter.checksum(),
        ledger,
        guest_inputs_replayed,
        deaths_matched,
    })
}

fn effective_genesis_phase_hint(island: &IslandRecordHeader) -> GenesisPhase {
    island.genesis_phase
}
