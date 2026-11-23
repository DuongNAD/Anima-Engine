//! Live ECS recorder and tick-schedule system for [`WorldRecord`] (ADR-0008 / WR1).
//!
//! ## Zero-feedback guarantee
//!
//! [`record_world_record_tick_system`] is inserted at the very end of
//! [`crate::core::simulation_schedule::build_tick_schedule`]. When [`WorldRecordRecorder`] is
//! absent it returns on its first line without touching any query or resource. When present, all
//! simulation components and resources are accessed immutably (no `ResMut` on any simulation state,
//! no `Commands`, no `SimRng` reads) so recording ON vs OFF executes the exact same trajectory.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::ai::hrrl::HomeostaticState;
use crate::core::agent_systems::{AgentEvaluation, AgentGeneration, AgentLineageId};
use crate::core::components::{Agent, AgentParentLineageIds, Food, Predator};
use crate::core::ecology::{EcosystemBiomass, ResourceField, SeasonClock};
use crate::core::ecs::Position;
use crate::core::intervention::InterventionCommand;
use crate::core::live_experiment::{LiveInterventions, LIVE_MODEL_VERSION};
use crate::core::observer::ObserverTrace;
use crate::core::resources::{CombatEvents, EpochManager, SimRng, VitalStatistics};
use crate::core::snapshot::{world_checksum, BuildProvenance, SCHEMA_VERSION};
use crate::core::world_artifact::{fnv1a_32, WorldIdentity};

use super::types::{
    DeathCauseTag, InputKind, PlatformClass, RecordedEvent, RecordedInput, SegmentBody,
    SignedSegment, WorldEvent, WorldKeyframe, WorldRecord, WorldRecordError, WorldRecordHeader,
};

/// Default upper bound on buffered external inputs per segment before overflow tracking triggers.
pub const DEFAULT_MAX_INPUTS_PER_SEGMENT: usize = 1_024;

/// Default upper bound on buffered events per segment before overflow tracking triggers.
pub const DEFAULT_MAX_EVENTS_PER_SEGMENT: usize = 4_096;

#[derive(Clone, Debug)]
struct AgentTrackMeta {
    lineage_id: String,
    entity_index: u32,
    is_predator: bool,
    generation: u32,
    age_ticks: u64,
}

#[derive(Default)]
struct ChecksumScratch {
    bytes: Vec<u8>,
    agents: Vec<[u32; 6]>,
    foods: Vec<[u32; 3]>,
}

/// Read-only view of all simulation compartments needed to hash checkpoints and observe deltas.
#[derive(SystemParam)]
pub struct RecordReadParams<'w, 's> {
    agents: Query<
        'w,
        's,
        (
            Entity,
            &'static HomeostaticState,
            &'static Position,
            Option<&'static Predator>,
            Option<&'static AgentLineageId>,
            Option<&'static AgentGeneration>,
            Option<&'static AgentParentLineageIds>,
            Option<&'static AgentEvaluation>,
        ),
        With<Agent>,
    >,
    foods: Query<'w, 's, (&'static Position, &'static Food)>,
    field: Option<Res<'w, ResourceField>>,
    biomass: Option<Res<'w, EcosystemBiomass>>,
    rng: Option<Res<'w, SimRng>>,
    season: Option<Res<'w, SeasonClock>>,
    vitals: Option<Res<'w, VitalStatistics>>,
    epoch_manager: Option<Res<'w, EpochManager>>,
    combat_events: Option<Res<'w, CombatEvents>>,
    interventions: Option<Res<'w, LiveInterventions>>,
    observer_trace: Option<Res<'w, ObserverTrace>>,
}

/// Optional ECS resource enabling non-intrusive [`WorldRecord`] capture during schedule ticks.
#[derive(Resource)]
pub struct WorldRecordRecorder {
    record: WorldRecord,
    next_tick: u64,
    segment_start_tick: u64,
    last_recorded_tick: Option<u64>,
    max_inputs_per_segment: usize,
    max_events_per_segment: usize,
    pending_inputs: Vec<RecordedInput>,
    pending_events: Vec<RecordedEvent>,
    dropped_inputs_since_prev: u64,
    dropped_events_since_prev: u64,
    known_agents: BTreeMap<u64, AgentTrackMeta>,
    seen_interventions: BTreeSet<(u32, u32, u64, u64)>,
    seen_observer_actions: usize,
    prev_total_births: u64,
    prev_total_deaths: u64,
    prev_deaths_starvation: u64,
    prev_deaths_dehydration: u64,
    prev_epoch: u32,
    checksum_scratch: ChecksumScratch,
    living_scan_scratch: BTreeSet<u64>,
    prey_combat_hits_scratch: BTreeSet<u32>,
    last_seal_error: Option<WorldRecordError>,
}

impl WorldRecordRecorder {
    /// Construct a recorder starting at tick `1` from a pre-built [`WorldRecordHeader`].
    pub fn new(header: WorldRecordHeader) -> Result<Self, WorldRecordError> {
        Self::with_limits(
            header,
            DEFAULT_MAX_INPUTS_PER_SEGMENT,
            DEFAULT_MAX_EVENTS_PER_SEGMENT,
        )
    }

    /// Construct a recorder with explicit per-segment input and event capacities (overflows count).
    pub fn with_limits(
        header: WorldRecordHeader,
        max_inputs_per_segment: usize,
        max_events_per_segment: usize,
    ) -> Result<Self, WorldRecordError> {
        let record = WorldRecord::build_empty(header)?;
        Ok(Self {
            record,
            next_tick: 1,
            segment_start_tick: 1,
            last_recorded_tick: None,
            max_inputs_per_segment,
            max_events_per_segment,
            pending_inputs: Vec::with_capacity(max_inputs_per_segment.min(64)),
            pending_events: Vec::with_capacity(max_events_per_segment.min(256)),
            dropped_inputs_since_prev: 0,
            dropped_events_since_prev: 0,
            known_agents: BTreeMap::new(),
            seen_interventions: BTreeSet::new(),
            seen_observer_actions: 0,
            prev_total_births: 0,
            prev_total_deaths: 0,
            prev_deaths_starvation: 0,
            prev_deaths_dehydration: 0,
            prev_epoch: 0,
            checksum_scratch: ChecksumScratch::default(),
            living_scan_scratch: BTreeSet::new(),
            prey_combat_hits_scratch: BTreeSet::new(),
            last_seal_error: None,
        })
    }

    /// Build a [`WorldRecordRecorder`] anchored to `world`'s genesis state before tick `1`.
    pub fn from_world(
        world: &mut World,
        seed: u64,
        segment_ticks: u64,
        deterministic: bool,
        mut initial_conditions: Vec<(String, f64)>,
        omissions: Vec<String>,
    ) -> Result<Self, WorldRecordError> {
        initial_conditions.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let world_identity = world
            .get_resource::<WorldIdentity>()
            .copied()
            .unwrap_or_default();
        let initial_world_checksum = world_checksum(world);
        let header = WorldRecordHeader {
            schema_version: super::types::WORLD_RECORD_SCHEMA_VERSION,
            seed,
            world_identity,
            world_artifact_sha256: None,
            model_version: LIVE_MODEL_VERSION,
            snapshot_schema_version: SCHEMA_VERSION,
            provenance: BuildProvenance::current(),
            platform_class: PlatformClass::current(),
            deterministic,
            initial_world_checksum,
            segment_ticks,
            initial_conditions,
            era_index: 0,
            prev_era_tail_hash: None,
            omissions,
        };
        let mut recorder = Self::new(header)?;
        recorder.sync_baseline_from_world(world);
        Ok(recorder)
    }

    fn sync_baseline_from_world(&mut self, world: &mut World) {
        self.known_agents.clear();
        let mut q = world.query_filtered::<(
            Entity,
            Option<&Predator>,
            Option<&AgentLineageId>,
            Option<&AgentGeneration>,
            Option<&AgentEvaluation>,
        ), With<Agent>>();
        for (entity, pred, lineage, gen, eval) in q.iter(world) {
            let lineage_id = lineage
                .map(|l| l.0.clone())
                .unwrap_or_else(|| format!("entity-{}", entity.index()));
            self.known_agents.insert(
                entity.to_bits(),
                AgentTrackMeta {
                    lineage_id,
                    entity_index: entity.index(),
                    is_predator: pred.is_some(),
                    generation: gen.map_or(0, |g| g.0),
                    age_ticks: eval.map_or(0, |e| e.survival_ticks as u64),
                },
            );
        }
        if let Some(vitals) = world.get_resource::<VitalStatistics>() {
            self.prev_total_births = vitals.total_births;
            self.prev_total_deaths = vitals.total_deaths;
            self.prev_deaths_starvation = vitals.total_deaths_starvation;
            self.prev_deaths_dehydration = vitals.total_deaths_dehydration;
        }
        if let Some(epoch) = world.get_resource::<EpochManager>() {
            self.prev_epoch = epoch.current_epoch;
        }
        if let Some(trace) = world.get_resource::<ObserverTrace>() {
            self.seen_observer_actions = trace.actions().len();
        }
    }

    /// Enqueue an external input entry into the open segment (bounded; excess increments drops).
    pub fn push_input(&mut self, input: RecordedInput) {
        if self.pending_inputs.len() >= self.max_inputs_per_segment {
            self.dropped_inputs_since_prev = self.dropped_inputs_since_prev.saturating_add(1);
            return;
        }
        self.pending_inputs.push(input);
    }

    /// Enqueue an event entry into the open segment (bounded; excess increments drops).
    pub fn push_event(&mut self, tick: u64, kind: WorldEvent) {
        if self.pending_events.len() >= self.max_events_per_segment {
            self.dropped_events_since_prev = self.dropped_events_since_prev.saturating_add(1);
            return;
        }
        self.pending_events.push(RecordedEvent { tick, kind });
    }

    /// Seal any pending ticks up to `last_recorded_tick` when ending a run mid-segment.
    pub fn flush_segment(&mut self, world: &mut World) -> Result<Option<SignedSegment>, WorldRecordError> {
        let Some(tick_end) = self.last_recorded_tick else {
            return Ok(None);
        };
        if tick_end < self.segment_start_tick {
            return Ok(None);
        }
        let end_checksum = world_checksum(world);
        let keyframe = keyframe_from_world(world, tick_end);
        let sealed = self.seal_with(tick_end, end_checksum, keyframe)?;
        Ok(Some(sealed.clone()))
    }

    /// Read the accumulated [`WorldRecord`] so far.
    pub fn record(&self) -> &WorldRecord {
        &self.record
    }

    /// Consume the recorder and yield its sealed [`WorldRecord`].
    pub fn into_record(self) -> WorldRecord {
        self.record
    }

    /// First error encountered if automatic sealing inside the tick system failed, if any.
    pub fn last_seal_error(&self) -> Option<&WorldRecordError> {
        self.last_seal_error.as_ref()
    }

    fn seal_with(
        &mut self,
        tick_end: u64,
        end_checksum: u32,
        keyframe: WorldKeyframe,
    ) -> Result<&SignedSegment, WorldRecordError> {
        let index = self.record.segments.len() as u64;
        let tick_start = self.segment_start_tick;
        let mut inputs = std::mem::take(&mut self.pending_inputs);
        let mut events = std::mem::take(&mut self.pending_events);
        inputs.sort_by_key(|i| i.tick);
        events.sort_by_key(|e| e.tick);
        let body = SegmentBody {
            index,
            tick_start,
            tick_end,
            end_checksum,
            keyframe,
            inputs,
            events,
            dropped_inputs_since_prev: std::mem::take(&mut self.dropped_inputs_since_prev),
            dropped_events_since_prev: std::mem::take(&mut self.dropped_events_since_prev),
        };
        let sealed = self.record.append_segment(body)?;
        self.segment_start_tick = tick_end.saturating_add(1);
        Ok(sealed)
    }
}

fn push_f32(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_bits().to_le_bytes());
}

fn push_f64(b: &mut Vec<u8>, v: f64) {
    b.extend_from_slice(&v.to_bits().to_le_bytes());
}

fn compute_checksum_and_keyframe(
    tick: u64,
    params: &RecordReadParams<'_, '_>,
    scratch: &mut ChecksumScratch,
) -> (u32, WorldKeyframe) {
    scratch.bytes.clear();
    scratch.agents.clear();
    scratch.foods.clear();

    let mut agent_count = 0u32;
    let mut predator_count = 0u32;
    let mut prey_count = 0u32;

    for (_entity, homeo, pos, pred, ..) in params.agents.iter() {
        agent_count = agent_count.saturating_add(1);
        if pred.is_some() {
            predator_count = predator_count.saturating_add(1);
        } else {
            prey_count = prey_count.saturating_add(1);
        }
        scratch.agents.push([
            homeo.energy.to_bits(),
            homeo.hydration.to_bits(),
            homeo.temperature.to_bits(),
            pos.0.x.to_bits(),
            pos.0.y.to_bits(),
            pos.0.z.to_bits(),
        ]);
    }
    scratch.agents.sort_unstable();
    let mut total_agent_energy_eu = 0.0f64;
    scratch
        .bytes
        .extend_from_slice(&(scratch.agents.len() as u32).to_le_bytes());
    for vals in &scratch.agents {
        total_agent_energy_eu += f32::from_bits(vals[0]) as f64;
        for v in vals {
            scratch.bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    let mut standing_crop_eu = 0.0f64;
    if let Some(ref field) = params.field {
        scratch
            .bytes
            .extend_from_slice(&(field.r.len() as u32).to_le_bytes());
        for &v in &field.r {
            standing_crop_eu += v as f64;
            scratch.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        scratch
            .bytes
            .extend_from_slice(&(field.regrowth_phase as u32).to_le_bytes());
    }

    let mut detritus_eu = 0.0f64;
    if let Some(ref pool) = params.biomass {
        detritus_eu = pool.detritus;
        push_f64(&mut scratch.bytes, pool.detritus);
    }

    if let Some(ref rng) = params.rng {
        scratch.bytes.extend_from_slice(&rng.seed().to_le_bytes());
        scratch
            .bytes
            .extend_from_slice(&rng.stream_pos().to_le_bytes());
    }

    if let Some(ref clock) = params.season {
        push_f32(&mut scratch.bytes, clock.phase);
        push_f32(&mut scratch.bytes, clock.rate);
    }

    for (pos, food) in params.foods.iter() {
        scratch.foods.push([
            pos.0.x.to_bits(),
            pos.0.z.to_bits(),
            food.energy_value.to_bits(),
        ]);
    }
    scratch.foods.sort_unstable();
    scratch
        .bytes
        .extend_from_slice(&(scratch.foods.len() as u32).to_le_bytes());
    for vals in &scratch.foods {
        for v in vals {
            scratch.bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    let checksum = fnv1a_32(&scratch.bytes);
    let (total_births, total_deaths) = params
        .vitals
        .as_deref()
        .map(|v| (v.total_births, v.total_deaths))
        .unwrap_or((0, 0));

    (
        checksum,
        WorldKeyframe {
            tick,
            agent_count,
            predator_count,
            prey_count,
            standing_crop_eu,
            detritus_eu,
            total_agent_energy_eu,
            total_births,
            total_deaths,
        },
    )
}

fn keyframe_from_world(world: &mut World, tick: u64) -> WorldKeyframe {
    let mut agent_count = 0u32;
    let mut predator_count = 0u32;
    let mut prey_count = 0u32;
    let mut energies: Vec<[u32; 6]> = Vec::new();
    {
        let mut q = world.query_filtered::<(
            &HomeostaticState,
            &Position,
            Option<&Predator>,
        ), With<Agent>>();
        for (homeo, pos, pred) in q.iter(world) {
            agent_count = agent_count.saturating_add(1);
            if pred.is_some() {
                predator_count = predator_count.saturating_add(1);
            } else {
                prey_count = prey_count.saturating_add(1);
            }
            energies.push([
                homeo.energy.to_bits(),
                homeo.hydration.to_bits(),
                homeo.temperature.to_bits(),
                pos.0.x.to_bits(),
                pos.0.y.to_bits(),
                pos.0.z.to_bits(),
            ]);
        }
    }
    energies.sort_unstable();
    let total_agent_energy_eu: f64 = energies.iter().map(|v| f32::from_bits(v[0]) as f64).sum();
    let standing_crop_eu = world
        .get_resource::<ResourceField>()
        .map(|f| f.r.iter().map(|&x| x as f64).sum())
        .unwrap_or(0.0);
    let detritus_eu = world
        .get_resource::<EcosystemBiomass>()
        .map(|b| b.detritus)
        .unwrap_or(0.0);
    let (total_births, total_deaths) = world
        .get_resource::<VitalStatistics>()
        .map(|v| (v.total_births, v.total_deaths))
        .unwrap_or((0, 0));

    WorldKeyframe {
        tick,
        agent_count,
        predator_count,
        prey_count,
        standing_crop_eu,
        detritus_eu,
        total_agent_energy_eu,
        total_births,
        total_deaths,
    }
}

fn intervention_key(cmd: &InterventionCommand) -> (u32, u32, u64, u64) {
    (cmd.id, cmd.cause_id, cmd.start_tick, cmd.duration_ticks)
}

/// Non-intrusive end-of-tick recorder wired at the tail of `build_tick_schedule`.
pub fn record_world_record_tick_system(
    recorder: Option<ResMut<WorldRecordRecorder>>,
    params: RecordReadParams<'_, '_>,
) {
    let Some(mut recorder) = recorder else {
        return;
    };
    let recorder = &mut *recorder;
    let tick = recorder.next_tick;
    recorder.next_tick = tick.saturating_add(1);
    recorder.last_recorded_tick = Some(tick);

    // 1. Inputs: live scenario interventions and observer actions (purely read-only).
    if let Some(ref interventions) = params.interventions {
        if !interventions.active.is_empty() && interventions.tick == tick {
            for cmd in &interventions.active {
                let key = intervention_key(cmd);
                if cmd.active_at(tick) && recorder.seen_interventions.insert(key) {
                    recorder.push_input(RecordedInput {
                        tick,
                        cause_id: cmd.cause_id,
                        kind: InputKind::Intervention(cmd.clone()),
                    });
                }
            }
        }
    }

    if let Some(ref trace) = params.observer_trace {
        let actions = trace.actions();
        if actions.len() > recorder.seen_observer_actions {
            let start = recorder.seen_observer_actions;
            for rec in &actions[start..] {
                let bounded_tick = rec.tick.clamp(recorder.segment_start_tick, tick);
                recorder.push_input(RecordedInput {
                    tick: bounded_tick,
                    cause_id: rec.cause_id,
                    kind: InputKind::ObserverAction(rec.action),
                });
            }
            recorder.seen_observer_actions = actions.len();
        }
    }

    // 2. Combat kills on this tick (fast empty skip on quiet ticks).
    recorder.prey_combat_hits_scratch.clear();
    let has_combat = if let Some(ref combat) = params.combat_events {
        !combat.events.is_empty()
    } else {
        false
    };
    if has_combat {
        if let Some(ref combat) = params.combat_events {
            for ev in &combat.events {
                recorder.prey_combat_hits_scratch.insert(ev.prey_id);
                let pred_label = format!("entity-{}", ev.predator_id);
                let prey_label = format!("entity-{}", ev.prey_id);
                recorder.push_event(
                    tick,
                    WorldEvent::Kill {
                        predator_id: pred_label,
                        prey_id: prey_label,
                        damage: ev.damage as f64,
                        energy_transferred: ev.energy_transferred as f64,
                    },
                );
            }
        }
    }

    // 3. Epoch replacement events & vital deltas.
    let (cur_births, cur_deaths, cur_starv, cur_dehyd) = params
        .vitals
        .as_deref()
        .map(|v| {
            (
                v.total_births,
                v.total_deaths,
                v.total_deaths_starvation,
                v.total_deaths_dehydration,
            )
        })
        .unwrap_or((0, 0, 0, 0));
    let cur_epoch = params
        .epoch_manager
        .as_deref()
        .map(|e| e.current_epoch)
        .unwrap_or(0);

    let epoch_advanced = cur_epoch > recorder.prev_epoch;
    if epoch_advanced {
        recorder.push_event(
            tick,
            WorldEvent::EpochReplacement {
                epoch: cur_epoch,
                replaced_count: cur_epoch.saturating_sub(recorder.prev_epoch),
            },
        );
    }

    let vitals_changed = cur_births != recorder.prev_total_births
        || cur_deaths != recorder.prev_total_deaths
        || epoch_advanced;
    let living_count = params.agents.iter().len();

    if vitals_changed || living_count != recorder.known_agents.len() {
        recorder.living_scan_scratch.clear();

        // Collect living agents cleanly into deterministic BTree order when membership changed.
        let mut new_births: Vec<(u64, AgentTrackMeta, Option<String>, f64)> = Vec::new();
        for (entity, homeo, _pos, pred, lineage, gen, parents, eval) in params.agents.iter() {
            let bits = entity.to_bits();
            recorder.living_scan_scratch.insert(bits);
            let age_ticks = eval.map_or(0, |e| e.survival_ticks as u64);
            if let Some(meta) = recorder.known_agents.get_mut(&bits) {
                meta.age_ticks = age_ticks;
            } else {
                let lineage_id = lineage
                    .map(|l| l.0.clone())
                    .unwrap_or_else(|| format!("entity-{}", entity.index()));
                let parent_id = parents.and_then(|p| p.0.first().cloned());
                let meta = AgentTrackMeta {
                    lineage_id,
                    entity_index: entity.index(),
                    is_predator: pred.is_some(),
                    generation: gen.map_or(0, |g| g.0),
                    age_ticks,
                };
                new_births.push((bits, meta, parent_id, homeo.energy as f64));
            }
        }
        new_births.sort_unstable_by_key(|item| item.0);

        // Emit deaths for vanished agents deterministically by `Entity::to_bits()`.
        let mut vanished: Vec<u64> = recorder
            .known_agents
            .keys()
            .copied()
            .filter(|bits| !recorder.living_scan_scratch.contains(bits))
            .collect();
        vanished.sort_unstable();

        let mut rem_starv = cur_starv.saturating_sub(recorder.prev_deaths_starvation);
        let mut rem_dehyd = cur_dehyd.saturating_sub(recorder.prev_deaths_dehydration);

        for bits in vanished {
            if let Some(meta) = recorder.known_agents.remove(&bits) {
                let cause = if rem_starv > 0 {
                    rem_starv -= 1;
                    DeathCauseTag::Starvation
                } else if rem_dehyd > 0 {
                    rem_dehyd -= 1;
                    DeathCauseTag::Dehydration
                } else if !meta.is_predator
                    && recorder.prey_combat_hits_scratch.contains(&meta.entity_index)
                {
                    DeathCauseTag::Predation
                } else if epoch_advanced {
                    DeathCauseTag::EpochReplacement
                } else {
                    DeathCauseTag::Reclaim
                };
                recorder.push_event(
                    tick,
                    WorldEvent::Death {
                        agent_id: meta.lineage_id,
                        cause,
                        age_ticks: meta.age_ticks,
                    },
                );
            }
        }

        for (bits, meta, parent_id, energy) in new_births {
            recorder.push_event(
                tick,
                WorldEvent::Birth {
                    agent_id: meta.lineage_id.clone(),
                    parent_id,
                    generation: meta.generation,
                    energy_transferred: energy,
                },
            );
            recorder.known_agents.insert(bits, meta);
        }
    }

    recorder.prev_total_births = cur_births;
    recorder.prev_total_deaths = cur_deaths;
    recorder.prev_deaths_starvation = cur_starv;
    recorder.prev_deaths_dehydration = cur_dehyd;
    recorder.prev_epoch = cur_epoch;

    // 4. Seal segment at configured cadence (`tick % segment_ticks == 0`).
    let segment_ticks = recorder.record.header.segment_ticks;
    if tick.is_multiple_of(segment_ticks) {
        let (end_checksum, keyframe) =
            compute_checksum_and_keyframe(tick, &params, &mut recorder.checksum_scratch);
        if let Err(err) = recorder.seal_with(tick, end_checksum, keyframe) {
            recorder.last_seal_error = Some(err);
        }
    }
}
