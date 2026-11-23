//! Live ECS recorder and tick-schedule system for [`WorldRecord`] (ADR-0008 / WR1 + WR2).
//!
//! ## Zero-feedback guarantee
//!
//! [`record_world_record_tick_system`] is inserted at the very end of
//! [`crate::core::simulation_schedule::build_tick_schedule`]. When [`WorldRecordRecorder`] is
//! absent it returns on its first line without touching any query or resource. When present, all
//! simulation components and resources are accessed immutably (no `ResMut` on any simulation state,
//! no `Commands`, no `SimRng` reads) so recording ON vs OFF executes the exact same trajectory.
//!
//! ## Zero-allocation hot loop (WR2)
//!
//! With the pre-allocated scratch tables and [`AsyncWriterHandle`], ticks with events, inputs, or
//! segment seals transport `Copy` [`RingEntry`] items through a fixed SPSC ring buffer without heap
//! allocations or blocking I/O on the simulation thread.

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

use super::ring::{CompactIntervention, InlineStr, RingEntry, RingProducer, INLINE_ID_LEN};
use super::types::{
    DeathCauseTag, InputKind, PlatformClass, RecordedEvent, RecordedInput, SegmentBody,
    SignedSegment, WorldEvent, WorldKeyframe, WorldRecord, WorldRecordError, WorldRecordHeader,
};
use super::writer::{AsyncWriterHandle, WorldRecordWriterConfig};

/// Default upper bound on buffered external inputs per segment before overflow tracking triggers.
pub const DEFAULT_MAX_INPUTS_PER_SEGMENT: usize = 1_024;

/// Default upper bound on buffered events per segment before overflow tracking triggers.
pub const DEFAULT_MAX_EVENTS_PER_SEGMENT: usize = 4_096;

const DEFAULT_AGENT_SCRATCH_CAP: usize = 4_096;
const DEFAULT_INTERVENTION_SEEN_CAP: usize = 256;
const DEFAULT_COMBAT_HITS_CAP: usize = 256;
const DEFAULT_CHECKSUM_BYTES_CAP: usize = 65_536;
const DEFAULT_FOODS_SCRATCH_CAP: usize = 2_048;

#[derive(Clone, Copy, Debug)]
struct CompactAgentMeta {
    lineage_id: InlineStr<INLINE_ID_LEN>,
    entity_index: u32,
    is_predator: bool,
    generation: u32,
    born_tick: u64,
    base_age: u64,
    energy: f64,
}

struct ChecksumScratch {
    bytes: Vec<u8>,
    agents: Vec<[u32; 6]>,
    foods: Vec<[u32; 3]>,
}

impl ChecksumScratch {
    fn with_capacities(bytes_cap: usize, agents_cap: usize, foods_cap: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(bytes_cap),
            agents: Vec::with_capacity(agents_cap),
            foods: Vec::with_capacity(foods_cap),
        }
    }
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

struct InlineSink {
    record: WorldRecord,
    max_inputs_per_segment: usize,
    max_events_per_segment: usize,
    pending_inputs: Vec<RecordedInput>,
    pending_events: Vec<RecordedEvent>,
}

struct AsyncSink {
    preview_record: WorldRecord,
    producer: RingProducer<RingEntry>,
    handle: AsyncWriterHandle,
    finished: bool,
}

enum RecorderSink {
    Inline(InlineSink),
    Async(AsyncSink),
}

/// Optional ECS resource enabling non-intrusive [`WorldRecord`] capture during schedule ticks.
#[derive(Resource)]
pub struct WorldRecordRecorder {
    sink: RecorderSink,
    next_tick: u64,
    segment_start_tick: u64,
    last_recorded_tick: Option<u64>,
    pending_dropped_inputs: u64,
    pending_dropped_events: u64,
    total_dropped_inputs: u64,
    total_dropped_events: u64,
    known_agents: Vec<(u64, CompactAgentMeta)>,
    living_scan_scratch: Vec<(u64, CompactAgentMeta)>,
    next_known_scratch: Vec<(u64, CompactAgentMeta)>,
    seen_interventions: Vec<(u32, u32, u64, u64)>,
    seen_observer_actions: usize,
    prev_total_births: u64,
    prev_total_deaths: u64,
    prev_deaths_starvation: u64,
    prev_deaths_dehydration: u64,
    prev_epoch: u32,
    checksum_scratch: ChecksumScratch,
    prey_combat_hits_scratch: Vec<u32>,
    last_seal_error: Option<WorldRecordError>,
}

impl WorldRecordRecorder {
    /// Construct an in-memory WR1 recorder starting at tick `1` from a pre-built header.
    pub fn new(header: WorldRecordHeader) -> Result<Self, WorldRecordError> {
        Self::with_limits(
            header,
            DEFAULT_MAX_INPUTS_PER_SEGMENT,
            DEFAULT_MAX_EVENTS_PER_SEGMENT,
        )
    }

    /// Construct an in-memory WR1 recorder with explicit per-segment input and event capacities.
    pub fn with_limits(
        header: WorldRecordHeader,
        max_inputs_per_segment: usize,
        max_events_per_segment: usize,
    ) -> Result<Self, WorldRecordError> {
        let record = WorldRecord::build_empty(header)?;
        Ok(Self::from_sink(
            RecorderSink::Inline(InlineSink {
                record,
                max_inputs_per_segment,
                max_events_per_segment,
                pending_inputs: Vec::with_capacity(max_inputs_per_segment.min(64)),
                pending_events: Vec::with_capacity(max_events_per_segment.min(256)),
            }),
            DEFAULT_AGENT_SCRATCH_CAP,
            DEFAULT_CHECKSUM_BYTES_CAP,
        ))
    }

    /// Construct a WR2 recorder backed by a fixed SPSC ring buffer and background writer thread.
    pub fn with_writer(
        header: WorldRecordHeader,
        config: WorldRecordWriterConfig,
    ) -> Result<Self, WorldRecordError> {
        let preview_record = WorldRecord::build_empty(header.clone())?;
        let (producer, handle) = AsyncWriterHandle::spawn(header, config)?;
        Ok(Self::from_sink(
            RecorderSink::Async(AsyncSink {
                preview_record,
                producer,
                handle,
                finished: false,
            }),
            DEFAULT_AGENT_SCRATCH_CAP,
            DEFAULT_CHECKSUM_BYTES_CAP,
        ))
    }

    fn from_sink(sink: RecorderSink, agent_cap: usize, checksum_bytes_cap: usize) -> Self {
        Self {
            sink,
            next_tick: 1,
            segment_start_tick: 1,
            last_recorded_tick: None,
            pending_dropped_inputs: 0,
            pending_dropped_events: 0,
            total_dropped_inputs: 0,
            total_dropped_events: 0,
            known_agents: Vec::with_capacity(agent_cap),
            living_scan_scratch: Vec::with_capacity(agent_cap),
            next_known_scratch: Vec::with_capacity(agent_cap),
            seen_interventions: Vec::with_capacity(DEFAULT_INTERVENTION_SEEN_CAP),
            seen_observer_actions: 0,
            prev_total_births: 0,
            prev_total_deaths: 0,
            prev_deaths_starvation: 0,
            prev_deaths_dehydration: 0,
            prev_epoch: 0,
            checksum_scratch: ChecksumScratch::with_capacities(
                checksum_bytes_cap,
                agent_cap,
                DEFAULT_FOODS_SCRATCH_CAP,
            ),
            prey_combat_hits_scratch: Vec::with_capacity(DEFAULT_COMBAT_HITS_CAP),
            last_seal_error: None,
        }
    }

    /// Build a WR1 in-memory [`WorldRecordRecorder`] anchored to `world`'s genesis state before tick `1`.
    pub fn from_world(
        world: &mut World,
        seed: u64,
        segment_ticks: u64,
        deterministic: bool,
        initial_conditions: Vec<(String, f64)>,
        omissions: Vec<String>,
    ) -> Result<Self, WorldRecordError> {
        let header = build_header_from_world(
            world,
            seed,
            segment_ticks,
            deterministic,
            initial_conditions,
            omissions,
        );
        let mut recorder = Self::new(header)?;
        recorder.sync_baseline_from_world(world);
        Ok(recorder)
    }

    /// Build a WR2 async-writer [`WorldRecordRecorder`] anchored to `world`'s genesis state before tick `1`.
    pub fn from_world_with_writer(
        world: &mut World,
        seed: u64,
        segment_ticks: u64,
        deterministic: bool,
        initial_conditions: Vec<(String, f64)>,
        omissions: Vec<String>,
        config: WorldRecordWriterConfig,
    ) -> Result<Self, WorldRecordError> {
        let header = build_header_from_world(
            world,
            seed,
            segment_ticks,
            deterministic,
            initial_conditions,
            omissions,
        );
        let mut recorder = Self::with_writer(header, config)?;
        recorder.sync_baseline_from_world(world);
        Ok(recorder)
    }

    fn sync_baseline_from_world(&mut self, world: &mut World) {
        if let Some(field) = world.get_resource::<ResourceField>() {
            let needed = field.r.len().saturating_mul(4).saturating_add(32_768);
            if self.checksum_scratch.bytes.capacity() < needed {
                self.checksum_scratch
                    .bytes
                    .reserve(needed - self.checksum_scratch.bytes.capacity());
            }
        }
        self.known_agents.clear();
        {
            let mut q = world.query_filtered::<(
                Entity,
                &HomeostaticState,
                Option<&Predator>,
                Option<&AgentLineageId>,
                Option<&AgentGeneration>,
                Option<&AgentEvaluation>,
            ), With<Agent>>();
            for (entity, homeo, pred, lineage, gen, eval) in q.iter(world) {
                let lineage_id = match lineage {
                    Some(l) => InlineStr::from_str_truncating(&l.0),
                    None => InlineStr::from_entity_index(entity.index()),
                };
                self.known_agents.push((
                    entity.to_bits(),
                    CompactAgentMeta {
                        lineage_id,
                        entity_index: entity.index(),
                        is_predator: pred.is_some(),
                        generation: gen.map_or(0, |g| g.0),
                        born_tick: 0,
                        base_age: eval.map_or(0, |e| e.survival_ticks as u64),
                        energy: homeo.energy as f64,
                    },
                ));
            }
        }
        self.known_agents.sort_unstable_by_key(|item| item.0);
        let extra = self.known_agents.len().saturating_add(2_048);
        reserve_exact_min(&mut self.known_agents, extra);
        reserve_exact_min(&mut self.living_scan_scratch, extra);
        reserve_exact_min(&mut self.next_known_scratch, extra);
        reserve_exact_min(&mut self.checksum_scratch.agents, extra);

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

    /// Enqueue an external input entry into the active sink (bounded; excess increments drops).
    pub fn push_input(&mut self, input: RecordedInput) {
        match &mut self.sink {
            RecorderSink::Inline(inline) => {
                if inline.pending_inputs.len() >= inline.max_inputs_per_segment {
                    self.pending_dropped_inputs = self.pending_dropped_inputs.saturating_add(1);
                    self.total_dropped_inputs = self.total_dropped_inputs.saturating_add(1);
                    return;
                }
                inline.pending_inputs.push(input);
            }
            RecorderSink::Async(_) => {
                let entry = RingEntry::from_recorded_input(&input);
                self.push_ring_entry(true, entry);
            }
        }
    }

    /// Enqueue an event entry into the active sink (bounded; excess increments drops).
    pub fn push_event(&mut self, tick: u64, kind: WorldEvent) {
        match &mut self.sink {
            RecorderSink::Inline(inline) => {
                if inline.pending_events.len() >= inline.max_events_per_segment {
                    self.pending_dropped_events = self.pending_dropped_events.saturating_add(1);
                    self.total_dropped_events = self.total_dropped_events.saturating_add(1);
                    return;
                }
                inline.pending_events.push(RecordedEvent { tick, kind });
            }
            RecorderSink::Async(_) => {
                let entry = RingEntry::from_world_event(tick, &kind);
                self.push_ring_entry(false, entry);
            }
        }
    }

    /// Push a `Copy` entry onto the pre-allocated WR2 ring without blocking or allocating.
    fn push_ring_entry(&mut self, is_input: bool, entry: RingEntry) -> bool {
        if let RecorderSink::Async(ref mut async_sink) = self.sink {
            // Reserve one slot so segment seals have priority on non-catastrophic ticks.
            if async_sink.producer.available_slots() > 1
                && async_sink.producer.try_push(entry).is_ok()
            {
                return true;
            }
        }
        if is_input {
            self.pending_dropped_inputs = self.pending_dropped_inputs.saturating_add(1);
            self.total_dropped_inputs = self.total_dropped_inputs.saturating_add(1);
        } else {
            self.pending_dropped_events = self.pending_dropped_events.saturating_add(1);
            self.total_dropped_events = self.total_dropped_events.saturating_add(1);
        }
        false
    }

    /// Total inputs dropped due to buffer capacity bounds since run genesis.
    pub fn dropped_inputs(&self) -> u64 {
        self.total_dropped_inputs
    }

    /// Total events dropped due to buffer capacity bounds since run genesis.
    pub fn dropped_events(&self) -> u64 {
        self.total_dropped_events
    }

    /// Wake the background writer thread (no-op on the in-memory WR1 sink).
    pub fn unpark_writer(&self) {
        if let RecorderSink::Async(ref async_sink) = self.sink {
            async_sink.handle.writer_thread().unpark();
        }
    }

    /// Seal any pending ticks up to `last_recorded_tick` when ending a run mid-segment.
    pub fn flush_segment(
        &mut self,
        world: &mut World,
    ) -> Result<Option<SignedSegment>, WorldRecordError> {
        let Some(tick_end) = self.last_recorded_tick else {
            return Ok(None);
        };
        if tick_end < self.segment_start_tick {
            return Ok(None);
        }
        let end_checksum = world_checksum(world);
        let keyframe = keyframe_from_world(world, tick_end);
        self.seal_with(tick_end, end_checksum, keyframe)
    }

    /// Drain queued entries, stop the WR2 background thread, and yield the assembled [`WorldRecord`].
    ///
    /// On an inline WR1 recorder this returns a clone of the accumulated record cleanly.
    pub fn finish_writer(&mut self) -> Result<WorldRecord, WorldRecordError> {
        match &mut self.sink {
            RecorderSink::Inline(inline) => Ok(inline.record.clone()),
            RecorderSink::Async(async_sink) => {
                if !async_sink.finished {
                    let record = async_sink.handle.finish()?;
                    async_sink.preview_record = record;
                    async_sink.finished = true;
                }
                Ok(async_sink.preview_record.clone())
            }
        }
    }

    /// Flush any unsealed trailing ticks and finish the writer thread cleanly.
    pub fn flush_and_finish_writer(
        &mut self,
        world: &mut World,
    ) -> Result<WorldRecord, WorldRecordError> {
        let _ = self.flush_segment(world)?;
        self.finish_writer()
    }

    /// Read the accumulated [`WorldRecord`] so far (populated on async recorders after `finish_writer`).
    pub fn record(&self) -> &WorldRecord {
        match &self.sink {
            RecorderSink::Inline(inline) => &inline.record,
            RecorderSink::Async(async_sink) => &async_sink.preview_record,
        }
    }

    /// Consume the recorder and yield its sealed [`WorldRecord`] (draining any background writer).
    pub fn into_record(mut self) -> Result<WorldRecord, WorldRecordError> {
        self.finish_writer()
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
    ) -> Result<Option<SignedSegment>, WorldRecordError> {
        match &mut self.sink {
            RecorderSink::Inline(inline) => {
                let index = inline.record.segments.len() as u64;
                let tick_start = self.segment_start_tick;
                let mut inputs = std::mem::take(&mut inline.pending_inputs);
                let mut events = std::mem::take(&mut inline.pending_events);
                inputs.sort_by_key(|i| i.tick);
                events.sort_by_key(|e| e.tick);
                let body = SegmentBody {
                    index,
                    tick_start,
                    tick_end,
                    end_checksum,
                    keyframe,
                    keyframe_snapshot_sha256: None,
                    inputs,
                    events,
                    dropped_inputs_since_prev: std::mem::take(&mut self.pending_dropped_inputs),
                    dropped_events_since_prev: std::mem::take(&mut self.pending_dropped_events),
                };
                let sealed = inline.record.append_segment(body)?.clone();
                self.segment_start_tick = tick_end.saturating_add(1);
                Ok(Some(sealed))
            }
            RecorderSink::Async(async_sink) => {
                let entry = RingEntry::SegmentSeal {
                    tick_start: self.segment_start_tick,
                    tick_end,
                    end_checksum,
                    keyframe,
                    dropped_inputs_since_prev: self.pending_dropped_inputs,
                    dropped_events_since_prev: self.pending_dropped_events,
                };
                match async_sink.producer.try_push(entry) {
                    Ok(()) => {
                        self.pending_dropped_inputs = 0;
                        self.pending_dropped_events = 0;
                        self.segment_start_tick = tick_end.saturating_add(1);
                        async_sink.handle.writer_thread().unpark();
                    }
                    Err(_) => {
                        self.pending_dropped_events = self.pending_dropped_events.saturating_add(1);
                        self.total_dropped_events = self.total_dropped_events.saturating_add(1);
                    }
                }
                Ok(None)
            }
        }
    }
}

fn reserve_exact_min<T>(vec: &mut Vec<T>, min_cap: usize) {
    if vec.capacity() < min_cap {
        vec.reserve(min_cap - vec.capacity());
    }
}

fn build_header_from_world(
    world: &mut World,
    seed: u64,
    segment_ticks: u64,
    deterministic: bool,
    mut initial_conditions: Vec<(String, f64)>,
    omissions: Vec<String>,
) -> WorldRecordHeader {
    initial_conditions.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let world_identity = world
        .get_resource::<WorldIdentity>()
        .copied()
        .unwrap_or_default();
    let initial_world_checksum = world_checksum(world);
    let (manifest_fingerprint, law_fingerprint, registry_fingerprint) = world
        .get_resource::<crate::core::live_experiment::LiveExperimentState>()
        .map(|state| {
            (
                Some(state.manifest_fingerprint),
                Some(state.law_fingerprint),
                Some(state.registry_fingerprint),
            )
        })
        .unwrap_or((None, None, None));
    WorldRecordHeader {
        schema_version: super::types::WORLD_RECORD_SCHEMA_VERSION,
        run_id: format!("run-{seed}"),
        parent_run_id: None,
        fork_tick: None,
        fork_keyframe_hash: None,
        manifest_fingerprint,
        law_fingerprint,
        registry_fingerprint,
        seed,
        world_identity,
        world_artifact_sha256: None,
        model_version: LIVE_MODEL_VERSION.to_string(),
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
        let mut q = world
            .query_filtered::<(&HomeostaticState, &Position, Option<&Predator>), With<Agent>>();
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

fn record_intervention_input(
    recorder: &mut WorldRecordRecorder,
    tick: u64,
    cmd: &InterventionCommand,
) {
    match &recorder.sink {
        RecorderSink::Async(_) => {
            let entry = RingEntry::InputIntervention {
                tick,
                cause_id: cmd.cause_id,
                command: CompactIntervention::from_command(cmd),
            };
            recorder.push_ring_entry(true, entry);
        }
        RecorderSink::Inline(_) => {
            recorder.push_input(RecordedInput {
                tick,
                cause_id: cmd.cause_id,
                kind: InputKind::Intervention(cmd.clone()),
            });
        }
    }
}

fn record_kill_event(
    recorder: &mut WorldRecordRecorder,
    tick: u64,
    predator_id: u32,
    prey_id: u32,
    damage: f64,
    energy_transferred: f64,
) {
    match &recorder.sink {
        RecorderSink::Async(_) => {
            let entry = RingEntry::EventKill {
                tick,
                predator_index: predator_id,
                prey_index: prey_id,
                damage,
                energy_transferred,
            };
            recorder.push_ring_entry(false, entry);
        }
        RecorderSink::Inline(_) => {
            recorder.push_event(
                tick,
                WorldEvent::Kill {
                    predator_id: format!("entity-{predator_id}"),
                    prey_id: format!("entity-{prey_id}"),
                    damage,
                    energy_transferred,
                },
            );
        }
    }
}

fn record_death_event(
    recorder: &mut WorldRecordRecorder,
    tick: u64,
    meta: &CompactAgentMeta,
    cause: DeathCauseTag,
) {
    let age_ticks = meta
        .base_age
        .saturating_add(tick.saturating_sub(meta.born_tick));
    match &recorder.sink {
        RecorderSink::Async(_) => {
            let entry = RingEntry::EventDeath {
                tick,
                agent_id: meta.lineage_id,
                cause,
                age_ticks,
            };
            recorder.push_ring_entry(false, entry);
        }
        RecorderSink::Inline(_) => {
            recorder.push_event(
                tick,
                WorldEvent::Death {
                    agent_id: meta.lineage_id.to_owned_string(),
                    cause,
                    age_ticks,
                },
            );
        }
    }
}

fn record_birth_event(
    recorder: &mut WorldRecordRecorder,
    tick: u64,
    meta: &CompactAgentMeta,
    parent_id: Option<InlineStr<INLINE_ID_LEN>>,
) {
    match &recorder.sink {
        RecorderSink::Async(_) => {
            let entry = RingEntry::EventBirth {
                tick,
                agent_id: meta.lineage_id,
                parent_id,
                generation: meta.generation,
                energy_transferred: meta.energy,
            };
            recorder.push_ring_entry(false, entry);
        }
        RecorderSink::Inline(_) => {
            recorder.push_event(
                tick,
                WorldEvent::Birth {
                    agent_id: meta.lineage_id.to_owned_string(),
                    parent_id: parent_id.map(|p| p.to_owned_string()),
                    generation: meta.generation,
                    energy_transferred: meta.energy,
                },
            );
        }
    }
}

/// Non-intrusive, zero-allocation end-of-tick recorder wired at the tail of `build_tick_schedule`.
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

    let mut pushed_or_sealed = false;

    // 1. Inputs: live scenario interventions and observer actions (strictly read-only, zero allocs).
    if let Some(ref interventions) = params.interventions {
        if !interventions.active.is_empty() && interventions.tick == tick {
            for cmd in &interventions.active {
                let key = intervention_key(cmd);
                if cmd.active_at(tick) && !recorder.seen_interventions.contains(&key) {
                    if recorder.seen_interventions.len() < recorder.seen_interventions.capacity() {
                        recorder.seen_interventions.push(key);
                    }
                    record_intervention_input(recorder, tick, cmd);
                    pushed_or_sealed = true;
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
                match &recorder.sink {
                    RecorderSink::Async(_) => {
                        recorder.push_ring_entry(
                            true,
                            RingEntry::InputObserver {
                                tick: bounded_tick,
                                cause_id: rec.cause_id,
                                action: rec.action,
                            },
                        );
                    }
                    RecorderSink::Inline(_) => {
                        recorder.push_input(RecordedInput {
                            tick: bounded_tick,
                            cause_id: rec.cause_id,
                            kind: InputKind::ObserverAction(rec.action),
                        });
                    }
                }
                pushed_or_sealed = true;
            }
            recorder.seen_observer_actions = actions.len();
        }
    }

    // 2. Combat kills on this tick (fast empty skip on quiet ticks; sorted array, zero allocs).
    recorder.prey_combat_hits_scratch.clear();
    if let Some(ref combat) = params.combat_events {
        if !combat.events.is_empty() {
            for ev in &combat.events {
                if recorder.prey_combat_hits_scratch.len()
                    < recorder.prey_combat_hits_scratch.capacity()
                {
                    recorder.prey_combat_hits_scratch.push(ev.prey_id);
                }
                record_kill_event(
                    recorder,
                    tick,
                    ev.predator_id,
                    ev.prey_id,
                    ev.damage as f64,
                    ev.energy_transferred as f64,
                );
                pushed_or_sealed = true;
            }
            recorder.prey_combat_hits_scratch.sort_unstable();
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
        pushed_or_sealed = true;
    }

    let vitals_changed = cur_births != recorder.prev_total_births
        || cur_deaths != recorder.prev_total_deaths
        || epoch_advanced;
    let living_count = params.agents.iter().len();

    if vitals_changed || living_count != recorder.known_agents.len() {
        recorder.living_scan_scratch.clear();
        recorder.next_known_scratch.clear();

        for (entity, homeo, _pos, pred, lineage, gen, _parents, eval) in params.agents.iter() {
            if recorder.living_scan_scratch.len() >= recorder.living_scan_scratch.capacity() {
                break;
            }
            let bits = entity.to_bits();
            let lineage_id = match lineage {
                Some(l) => InlineStr::from_str_truncating(&l.0),
                None => InlineStr::from_entity_index(entity.index()),
            };
            recorder.living_scan_scratch.push((
                bits,
                CompactAgentMeta {
                    lineage_id,
                    entity_index: entity.index(),
                    is_predator: pred.is_some(),
                    generation: gen.map_or(0, |g| g.0),
                    born_tick: tick,
                    base_age: eval.map_or(0, |e| e.survival_ticks as u64),
                    energy: homeo.energy as f64,
                },
            ));
        }
        // `sort_unstable_by_key` uses in-place pdqsort with zero heap allocations.
        recorder
            .living_scan_scratch
            .sort_unstable_by_key(|item| item.0);

        let mut rem_starv = cur_starv.saturating_sub(recorder.prev_deaths_starvation);
        let mut rem_dehyd = cur_dehyd.saturating_sub(recorder.prev_deaths_dehydration);

        // Pass 1: emit deaths in deterministic sorted `Entity::to_bits()` order.
        let (mut i, mut j) = (0usize, 0usize);
        while i < recorder.known_agents.len() {
            let (k_bits, meta) = recorder.known_agents[i];
            if j < recorder.living_scan_scratch.len() {
                let l_bits = recorder.living_scan_scratch[j].0;
                if l_bits < k_bits {
                    j += 1;
                    continue;
                }
                if l_bits == k_bits {
                    i += 1;
                    j += 1;
                    continue;
                }
            }
            let cause = if rem_starv > 0 {
                rem_starv -= 1;
                DeathCauseTag::Starvation
            } else if rem_dehyd > 0 {
                rem_dehyd -= 1;
                DeathCauseTag::Dehydration
            } else if !meta.is_predator
                && recorder
                    .prey_combat_hits_scratch
                    .binary_search(&meta.entity_index)
                    .is_ok()
            {
                DeathCauseTag::Predation
            } else if epoch_advanced {
                DeathCauseTag::EpochReplacement
            } else {
                DeathCauseTag::Reclaim
            };
            record_death_event(recorder, tick, &meta, cause);
            pushed_or_sealed = true;
            i += 1;
        }

        // Pass 2: build `next_known_scratch` (preserving existing `born_tick`/`base_age`) and emit births.
        let (mut i, mut j) = (0usize, 0usize);
        while i < recorder.known_agents.len() || j < recorder.living_scan_scratch.len() {
            match (
                recorder.known_agents.get(i).copied(),
                recorder.living_scan_scratch.get(j).copied(),
            ) {
                (Some((k_bits, old_meta)), Some((l_bits, _))) if k_bits == l_bits => {
                    if recorder.next_known_scratch.len() < recorder.next_known_scratch.capacity() {
                        recorder.next_known_scratch.push((k_bits, old_meta));
                    }
                    i += 1;
                    j += 1;
                }
                (Some((k_bits, _)), Some((l_bits, _))) if k_bits < l_bits => {
                    i += 1;
                }
                (Some((k_bits, _)), None) => {
                    let _ = k_bits;
                    i += 1;
                }
                (_, Some((l_bits, new_meta))) => {
                    if recorder.next_known_scratch.len() < recorder.next_known_scratch.capacity() {
                        recorder.next_known_scratch.push((l_bits, new_meta));
                    }
                    let entity = Entity::from_bits(l_bits);
                    let parent_id = params.agents.get(entity).ok().and_then(
                        |(_, _, _, _, _, _, parents, _)| {
                            parents
                                .and_then(|p| p.0.first())
                                .map(|s| InlineStr::from_str_truncating(s))
                        },
                    );
                    record_birth_event(recorder, tick, &new_meta, parent_id);
                    pushed_or_sealed = true;
                    j += 1;
                }
                (None, None) => break,
            }
        }
        std::mem::swap(&mut recorder.known_agents, &mut recorder.next_known_scratch);
    }

    recorder.prev_total_births = cur_births;
    recorder.prev_total_deaths = cur_deaths;
    recorder.prev_deaths_starvation = cur_starv;
    recorder.prev_deaths_dehydration = cur_dehyd;
    recorder.prev_epoch = cur_epoch;

    // 4. Seal segment at configured cadence (`tick % segment_ticks == 0`).
    let segment_ticks = recorder.record().header.segment_ticks;
    if tick.is_multiple_of(segment_ticks) {
        let (end_checksum, keyframe) =
            compute_checksum_and_keyframe(tick, &params, &mut recorder.checksum_scratch);
        if let Err(err) = recorder.seal_with(tick, end_checksum, keyframe) {
            recorder.last_seal_error = Some(err);
        }
        pushed_or_sealed = true;
    }

    if pushed_or_sealed {
        recorder.unpark_writer();
    }
}
