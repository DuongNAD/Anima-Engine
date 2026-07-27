//! In-process tick capture: what a tick of the **live** schedule actually costs.
//!
//! `docs/how-to/BENCHMARKING.md` measures individual systems with Criterion, on synthetic fixtures,
//! outside any running engine. That is a genuine lower bound and it is explicitly *not* a frame: it
//! omits ECS scheduling, change detection, inference, the telemetry publish and everything else the
//! sim thread does between two `schedule.run` calls. This module measures the thing Criterion cannot
//! — the running engine's own tick — without running the desktop app.
//!
//! # What it measures, precisely
//!
//! Three numbers are **exact**, because the simulation loop brackets them directly with one
//! `Instant`:
//!
//! | Phase | Bracket |
//! |---|---|
//! | [`TickPhase::Schedule`] | around `schedule.run(&mut world)` |
//! | [`TickPhase::TelemetryPublish`] | around the state-extraction / IPC publish block |
//! | [`TickPhase::FullTick`] | the sum of the two — the tick's real work, excluding the frame-pacing sleep |
//!
//! The rest are **checkpoint-bounded**, and the distinction matters enough to be in the exported
//! JSON rather than only in this comment. Bevy's schedule has no per-system timing hook, and adding
//! one by splitting the schedule would change execution order — which is the one thing a profiler
//! must not do. So the schedule carries three *checkpoint* systems, each declared `.after(...)` a
//! named boundary system and chained only to each other. A phase is then "the schedule work the
//! executor performed between checkpoint k−1 and checkpoint k". It always contains the boundary
//! system's group; under the multi-threaded executor it may also contain work the executor chose to
//! interleave. [`PhaseSummary::exact`] says which kind a row is, and [`CaptureExport::executor`]
//! says which executor produced it.
//!
//! Chaining the checkpoints only to each other, and anchoring each with `.after` alone, is what
//! keeps this non-perturbing: every edge added points *into* a checkpoint and none points back out
//! into the simulation, so no two pre-existing systems gain an ordering they did not already have.
//! `capture_does_not_change_the_trajectory` is the gate on that claim.
//!
//! # What it is not
//!
//! Operational evidence, not scientific state. Nothing here is serialized into
//! [`crate::core::simulation_state::SavedSimulationState`], read by
//! [`crate::core::snapshot::world_checksum`], or reachable from any RNG stream. A run with capture
//! on and a run with capture off produce the same trajectory, and that is a test rather than a
//! promise.
//!
//! # Bounded by construction
//!
//! The ring buffer is allocated once at [`SharedTickCapture::start`] and never grows: a capture left
//! running overnight costs exactly `capacity * size_of::<TickSample>()` bytes and overwrites its
//! oldest sample, counting each overwrite. There is no channel, so there is nothing to grow
//! unboundedly and no thread to join.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Schema version of the exported JSON. Bumped when a field changes meaning or disappears.
pub const CAPTURE_SCHEMA_VERSION: u32 = 1;

/// Largest ring the configuration will accept.
///
/// `TickSample` is 48 bytes, so this is ~48 MiB — far past any useful capture and still a number a
/// machine can hold. A request above it is refused rather than silently clamped: a caller that asked
/// for a billion samples has a bug, and quietly giving them a million would hide it.
pub const MAX_CAPTURE_CAPACITY: usize = 1_000_000;

/// Largest warm-up this configuration will accept (about 46 hours at 60 Hz).
pub const MAX_WARMUP_TICKS: u64 = 10_000_000;

// ---- Phases ----------------------------------------------------------------------------------

/// A phase of one simulation tick.
///
/// The order of the variants is the order of the tick, and [`TickPhase::ALL`] is the export order.
/// The discriminants are used as indices into a fixed-size array, so they are dense and stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TickPhase {
    /// Schedule + telemetry: the tick's real work, excluding the frame-pacing sleep. Exact.
    FullTick,
    /// `schedule.run(&mut world)` in full. Exact.
    Schedule,
    /// Schedule work up to the checkpoint after `action_resolution_system`.
    SensorBrain,
    /// Schedule work between the sensor/brain checkpoint and the one after
    /// `rebuild_spatial_grid_system`.
    PhysicsMovement,
    /// Schedule work between the physics checkpoint and the one after `ecosystem_census_system`.
    EcologyResources,
    /// Whatever the schedule still had to do after the last checkpoint.
    ScheduleTail,
    /// The state-extraction and IPC publish block the sim thread runs after the schedule. Exact.
    TelemetryPublish,
}

impl TickPhase {
    /// Every phase, in tick order.
    pub const ALL: [TickPhase; 7] = [
        TickPhase::FullTick,
        TickPhase::Schedule,
        TickPhase::SensorBrain,
        TickPhase::PhysicsMovement,
        TickPhase::EcologyResources,
        TickPhase::ScheduleTail,
        TickPhase::TelemetryPublish,
    ];

    /// How many phases there are. The width of every per-phase array in this module.
    pub const COUNT: usize = Self::ALL.len();

    /// Dense index into a per-phase array.
    pub const fn index(self) -> usize {
        match self {
            TickPhase::FullTick => 0,
            TickPhase::Schedule => 1,
            TickPhase::SensorBrain => 2,
            TickPhase::PhysicsMovement => 3,
            TickPhase::EcologyResources => 4,
            TickPhase::ScheduleTail => 5,
            TickPhase::TelemetryPublish => 6,
        }
    }

    /// Stable identifier used in the exported JSON and in configuration strings.
    pub const fn id(self) -> &'static str {
        match self {
            TickPhase::FullTick => "full_tick",
            TickPhase::Schedule => "schedule",
            TickPhase::SensorBrain => "sensor_brain",
            TickPhase::PhysicsMovement => "physics_movement",
            TickPhase::EcologyResources => "ecology_resources",
            TickPhase::ScheduleTail => "schedule_tail",
            TickPhase::TelemetryPublish => "telemetry_publish",
        }
    }

    /// Parse an [`id`](Self::id).
    pub fn from_id(s: &str) -> Option<TickPhase> {
        TickPhase::ALL.into_iter().find(|p| p.id() == s)
    }

    /// Whether the bracket around this phase is exact, or bounded by a checkpoint that is only
    /// guaranteed to run *after* a named system.
    pub const fn is_exact(self) -> bool {
        matches!(
            self,
            TickPhase::FullTick | TickPhase::Schedule | TickPhase::TelemetryPublish
        )
    }

    /// The system a checkpoint-bounded phase ends after, or `None` for an exactly-bracketed phase.
    pub const fn boundary_system(self) -> Option<&'static str> {
        match self {
            TickPhase::SensorBrain => Some("action_resolution_system"),
            TickPhase::PhysicsMovement => Some("rebuild_spatial_grid_system"),
            TickPhase::EcologyResources => Some("ecosystem_census_system"),
            TickPhase::ScheduleTail => Some("<end of schedule>"),
            _ => None,
        }
    }
}

/// A phase the engine could be expected to have but does not, and why.
///
/// Exported so a reader never has to infer "no row" as "zero cost". The plant/soil/weather fields
/// are the live example: [`crate::core::dynamic_fields`] exists, is tested, and is **not** in the
/// live schedule — no system inserts or steps it — so there is nothing to time.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnavailablePhase {
    pub id: String,
    pub reason: String,
}

/// Phases named by the benchmarking backlog that this build cannot measure, with the reason.
pub fn unavailable_phases() -> Vec<UnavailablePhase> {
    vec![UnavailablePhase {
        id: "plant_soil_weather".to_string(),
        reason: "core::dynamic_fields is not part of the live schedule in this build: nothing \
                 inserts DynamicFields as a resource and no system steps it, so there is no live \
                 work to time. Criterion measures the functions directly — see \
                 docs/how-to/BENCHMARKING.md."
            .to_string(),
    }]
}

// ---- Configuration ---------------------------------------------------------------------------

/// A set of phases, as a bitmask over [`TickPhase::index`].
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PhaseMask(pub u16);

impl PhaseMask {
    /// Every phase.
    pub fn all() -> Self {
        let mut m = 0u16;
        for p in TickPhase::ALL {
            m |= 1 << p.index();
        }
        PhaseMask(m)
    }

    /// No phase at all — an empty mask, which [`CaptureConfig::validate`] refuses.
    pub fn none() -> Self {
        PhaseMask(0)
    }

    /// The mask holding exactly `phases`.
    pub fn of(phases: &[TickPhase]) -> Self {
        let mut m = 0u16;
        for p in phases {
            m |= 1 << p.index();
        }
        PhaseMask(m)
    }

    pub fn contains(self, phase: TickPhase) -> bool {
        self.0 & (1 << phase.index()) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The phases in this mask, in tick order.
    pub fn phases(self) -> Vec<TickPhase> {
        TickPhase::ALL
            .into_iter()
            .filter(|p| self.contains(*p))
            .collect()
    }
}

/// What a capture run is allowed to do.
///
/// Every count crosses the Tauri bridge as a JSON number rather than a bigint, for the reason on
/// [`crate::core::simulation_state::SimulationStatus::tick_count`]: serde_json emits `u64` as a bare
/// number and JS parses it into a `number`.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Ticks to discard before recording anything. A cold engine's first ticks pay for lazy
    /// allocation, cache misses and the first inference batch; recording them reports a warm-up as
    /// though it were a frame.
    #[ts(type = "number")]
    pub warmup_ticks: u64,
    /// Ring capacity, in samples. Allocated once.
    pub capacity: usize,
    /// Stop recording after this many samples have been taken. `None` runs until stopped, which the
    /// ring bounds anyway.
    #[ts(type = "number | null")]
    pub max_samples: Option<u64>,
    /// Record one tick in every `sample_every`. `1` records every tick.
    #[ts(type = "number")]
    pub sample_every: u64,
    /// Which phases to summarise. Every phase is still *measured* — the mask decides what is
    /// exported, so a narrow mask never changes the numbers of a wide one.
    pub groups: PhaseMask,
}

impl Default for CaptureConfig {
    /// Two seconds of warm-up at 60 Hz, then every tick for about a minute.
    fn default() -> Self {
        Self {
            warmup_ticks: 120,
            capacity: 3_600,
            max_samples: None,
            sample_every: 1,
            groups: PhaseMask::all(),
        }
    }
}

/// Why a [`CaptureConfig`] was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureConfigError {
    ZeroCapacity,
    CapacityTooLarge {
        found: usize,
        limit: usize,
    },
    ZeroSampleRate,
    WarmupTooLong {
        found: u64,
        limit: u64,
    },
    ZeroMaxSamples,
    NoGroups,
    /// `warmup_ticks + max_samples * sample_every` does not fit in a `u64`.
    ScheduleOverflows,
}

impl std::fmt::Display for CaptureConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => write!(f, "capacity must be at least 1 sample"),
            Self::CapacityTooLarge { found, limit } => write!(
                f,
                "capacity {found} exceeds the {limit}-sample ceiling; capture is bounded on purpose"
            ),
            Self::ZeroSampleRate => write!(f, "sample_every must be at least 1"),
            Self::WarmupTooLong { found, limit } => {
                write!(f, "warmup_ticks {found} exceeds the {limit}-tick ceiling")
            }
            Self::ZeroMaxSamples => write!(
                f,
                "max_samples must be at least 1 when set; use None to run until stopped"
            ),
            Self::NoGroups => write!(f, "at least one phase group must be enabled"),
            Self::ScheduleOverflows => write!(
                f,
                "warmup_ticks + max_samples * sample_every overflows u64; the capture could never end"
            ),
        }
    }
}

impl std::error::Error for CaptureConfigError {}

impl CaptureConfig {
    /// Reject a configuration that cannot be honoured, rather than clamping it into one that can.
    ///
    /// Clamping is the tempting choice and the wrong one: a caller asking for zero capacity gets a
    /// capture that records nothing and reports success, which reads exactly like "the engine is
    /// infinitely fast".
    pub fn validate(&self) -> Result<(), CaptureConfigError> {
        if self.capacity == 0 {
            return Err(CaptureConfigError::ZeroCapacity);
        }
        if self.capacity > MAX_CAPTURE_CAPACITY {
            return Err(CaptureConfigError::CapacityTooLarge {
                found: self.capacity,
                limit: MAX_CAPTURE_CAPACITY,
            });
        }
        if self.sample_every == 0 {
            return Err(CaptureConfigError::ZeroSampleRate);
        }
        if self.warmup_ticks > MAX_WARMUP_TICKS {
            return Err(CaptureConfigError::WarmupTooLong {
                found: self.warmup_ticks,
                limit: MAX_WARMUP_TICKS,
            });
        }
        if self.groups.is_empty() {
            return Err(CaptureConfigError::NoGroups);
        }
        if let Some(max) = self.max_samples {
            if max == 0 {
                return Err(CaptureConfigError::ZeroMaxSamples);
            }
            max.checked_mul(self.sample_every)
                .and_then(|span| span.checked_add(self.warmup_ticks))
                .ok_or(CaptureConfigError::ScheduleOverflows)?;
        }
        Ok(())
    }

    /// A capture configured from `ANIMA_TICK_CAPTURE`, or `None` when the variable is unset.
    ///
    /// Format is `key=value` pairs separated by commas, e.g.
    /// `ANIMA_TICK_CAPTURE=warmup=300,capacity=1800,every=2`. An unparseable value leaves that key
    /// at its default rather than taking the run down — same rule as every other engine switch, and
    /// for the same reason: a malformed environment variable must not be able to stop a simulation.
    /// An invalid *combination* is still refused, because that one cannot be silently survived.
    pub fn from_env() -> Option<Result<Self, CaptureConfigError>> {
        let raw = std::env::var("ANIMA_TICK_CAPTURE").ok()?;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
            return None;
        }
        Some(Self::parse(trimmed))
    }
}

/// File name a completed capture writes itself to, from `ANIMA_TICK_CAPTURE_OUT`.
///
/// # Why a capture needs to be able to write itself
///
/// Every route out of a capture is IPC: [`crate::commands::capture`] holds all four commands, no
/// component in `src/` calls any of them, and a **release** binary has no DevTools to call them
/// from — Tauri v2 only enables the inspector automatically in debug builds, and this app does not
/// enable the `devtools` feature. So on the profile whose numbers are actually worth having,
/// `ANIMA_TICK_CAPTURE` could fill its ring and the samples would die with the process.
///
/// This is the way out that needs no window and no console. Unset changes nothing: the capture is
/// retrieved over IPC exactly as before.
///
/// The value is a **name**, not a path, and is resolved under the app data directory by the same
/// [`crate::commands::save_paths`] rules a save name follows — a run started from a shell must not
/// be able to aim an engine thread at an arbitrary file.
pub fn auto_export_name_from_env() -> Option<String> {
    let raw = std::env::var("ANIMA_TICK_CAPTURE_OUT").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

impl CaptureConfig {
    /// The parsing half of [`from_env`](Self::from_env), separated so it is testable without
    /// touching process state.
    pub fn parse(spec: &str) -> Result<Self, CaptureConfigError> {
        let mut config = CaptureConfig::default();
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() || part == "1" || part.eq_ignore_ascii_case("true") {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "warmup" => {
                    if let Ok(v) = value.parse() {
                        config.warmup_ticks = v;
                    }
                }
                "capacity" => {
                    if let Ok(v) = value.parse() {
                        config.capacity = v;
                    }
                }
                "every" => {
                    if let Ok(v) = value.parse() {
                        config.sample_every = v;
                    }
                }
                "max_samples" => {
                    if let Ok(v) = value.parse::<u64>() {
                        config.max_samples = Some(v);
                    }
                }
                "groups" => {
                    let mut mask = PhaseMask::none();
                    for name in value.split('+') {
                        if let Some(p) = TickPhase::from_id(name.trim()) {
                            mask.0 |= 1 << p.index();
                        }
                    }
                    config.groups = mask;
                }
                _ => {}
            }
        }
        config.validate()?;
        Ok(config)
    }
}

// ---- Samples and the ring --------------------------------------------------------------------

/// One captured tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TickSample {
    /// The engine's tick counter at the moment this was recorded.
    pub tick: u64,
    /// Live agents at the end of the tick, so a per-agent cost can be derived rather than assumed.
    pub agent_count: u32,
    /// Elapsed nanoseconds per phase, indexed by [`TickPhase::index`].
    pub phase_ns: [u64; TickPhase::COUNT],
}

/// What a capture is doing right now.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureStatus {
    /// Nothing has been started.
    Idle,
    /// Started, still discarding warm-up ticks.
    Warmup,
    /// Recording.
    Recording,
    /// `max_samples` was reached and recording stopped on its own.
    Complete,
    /// Stopped by a caller.
    Stopped,
}

impl CaptureStatus {
    /// Whether the capture is still consuming ticks.
    pub fn is_active(self) -> bool {
        matches!(self, CaptureStatus::Warmup | CaptureStatus::Recording)
    }
}

/// Sample bookkeeping that a summary alone cannot express.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureAccounting {
    /// Ticks the capture saw while active, including warm-up.
    #[ts(type = "number")]
    pub ticks_observed: u64,
    /// Ticks discarded because warm-up had not finished.
    #[ts(type = "number")]
    pub warmup_discarded: u64,
    /// Ticks skipped by `sample_every`.
    #[ts(type = "number")]
    pub rate_skipped: u64,
    /// Samples recorded, including any later overwritten.
    #[ts(type = "number")]
    pub samples_recorded: u64,
    /// Samples overwritten because the ring was full. The ring holds the most recent
    /// `capacity` samples; this says how many older ones it dropped to do that.
    #[ts(type = "number")]
    pub samples_overwritten: u64,
    /// Samples refused because a checkpoint stamp was missing — a phase that never ran.
    #[ts(type = "number")]
    pub dropped_incomplete: u64,
    /// Samples refused because the checkpoint stamps did not increase monotonically, which is what
    /// a parallel executor reordering the checkpoints looks like from here.
    #[ts(type = "number")]
    pub dropped_out_of_order: u64,
    /// Samples refused because a phase duration did not fit in the tick it belonged to.
    #[ts(type = "number")]
    pub dropped_overflow: u64,
}

/// The measured shape of the world a capture ran against.
///
/// Not a configuration: these are read off the running world, so a capture cannot claim a workload
/// it did not have. The dimensions are the **live** field's, which is `MapSettings::default()` —
/// 256×256 in this build — and deliberately not any benchmark constant.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadContext {
    /// Resource-field cells across X, as the running world has them.
    pub world_width: u32,
    /// Resource-field cells across Z.
    pub world_height: u32,
    /// Whether the two dimensions above were read from a live `ResourceField`. `false` means no
    /// field was present and the dimensions are unknown rather than zero-sized.
    pub dimensions_measured: bool,
}

/// The live state of a capture. Held behind [`SharedTickCapture`]'s mutex.
#[derive(Debug)]
pub struct TickCaptureState {
    status: CaptureStatus,
    config: CaptureConfig,
    ring: Vec<TickSample>,
    /// Where the next sample goes. Only meaningful once `ring.len() == capacity`.
    write: usize,
    accounting: CaptureAccounting,
    workload: WorkloadContext,
    /// Which executor the schedule ran under while this capture was recording.
    executor: &'static str,
}

impl Default for TickCaptureState {
    fn default() -> Self {
        Self::new()
    }
}

impl TickCaptureState {
    pub fn new() -> Self {
        Self {
            status: CaptureStatus::Idle,
            config: CaptureConfig::default(),
            ring: Vec::new(),
            write: 0,
            accounting: CaptureAccounting::default(),
            workload: WorkloadContext::default(),
            executor: "unknown",
        }
    }

    /// Begin a capture, discarding whatever the previous one held and allocating the ring once.
    pub fn start(&mut self, config: CaptureConfig) -> Result<(), CaptureConfigError> {
        config.validate()?;
        self.ring = Vec::with_capacity(config.capacity);
        self.write = 0;
        self.accounting = CaptureAccounting::default();
        self.workload = WorkloadContext::default();
        self.status = if config.warmup_ticks == 0 {
            CaptureStatus::Recording
        } else {
            CaptureStatus::Warmup
        };
        self.config = config;
        Ok(())
    }

    /// Stop recording, keeping everything captured so far so it can still be exported.
    pub fn stop(&mut self) {
        if self.status.is_active() {
            self.status = CaptureStatus::Stopped;
        }
    }

    pub fn status(&self) -> CaptureStatus {
        self.status
    }

    pub fn config(&self) -> CaptureConfig {
        self.config
    }

    pub fn accounting(&self) -> CaptureAccounting {
        self.accounting
    }

    pub fn workload(&self) -> WorkloadContext {
        self.workload
    }

    pub fn executor(&self) -> &'static str {
        self.executor
    }

    /// Samples currently held, oldest first.
    pub fn samples(&self) -> Vec<TickSample> {
        if self.ring.len() < self.config.capacity {
            return self.ring.clone();
        }
        let mut out = Vec::with_capacity(self.ring.len());
        out.extend_from_slice(&self.ring[self.write..]);
        out.extend_from_slice(&self.ring[..self.write]);
        out
    }

    /// How many samples are held right now.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Record the durations of one tick.
    ///
    /// `checkpoint_ns` holds the elapsed nanoseconds, measured from the start of the tick, at each
    /// schedule checkpoint in tick order; `schedule_ns` and `telemetry_ns` are the exact brackets.
    /// Returns whether a sample was stored.
    ///
    /// This is the whole ingestion path and it is a pure function of its arguments, so the
    /// adversarial cases — a zero clock, a clock that went backwards, durations that do not fit in
    /// the tick that contains them — are reachable from a test without needing a real slow tick.
    pub fn observe_tick(
        &mut self,
        tick: u64,
        agent_count: u32,
        checkpoint_ns: [u64; 3],
        schedule_ns: u64,
        telemetry_ns: u64,
        workload: WorkloadContext,
    ) -> bool {
        if !self.status.is_active() {
            return false;
        }
        if workload.dimensions_measured {
            self.workload = workload;
        }
        self.accounting.ticks_observed += 1;

        if self.accounting.ticks_observed <= self.config.warmup_ticks {
            self.accounting.warmup_discarded += 1;
            return false;
        }
        if self.status == CaptureStatus::Warmup {
            self.status = CaptureStatus::Recording;
        }

        // Rate is counted from the first post-warm-up tick, so `every=2` records the first tick
        // after warm-up and every second one after that — never an empty capture because the
        // absolute tick numbers happened to land on the wrong parity.
        let since_warmup = self.accounting.ticks_observed - self.config.warmup_ticks;
        if !(since_warmup - 1).is_multiple_of(self.config.sample_every) {
            self.accounting.rate_skipped += 1;
            return false;
        }

        let Some(sample) =
            build_sample(tick, agent_count, checkpoint_ns, schedule_ns, telemetry_ns)
        else {
            // `build_sample` distinguishes the three ways a tick can be unusable; each is counted
            // separately because they mean different things about the run.
            match classify_rejection(checkpoint_ns, schedule_ns, telemetry_ns) {
                Rejection::Incomplete => self.accounting.dropped_incomplete += 1,
                Rejection::OutOfOrder => self.accounting.dropped_out_of_order += 1,
                Rejection::Overflow => self.accounting.dropped_overflow += 1,
            }
            return false;
        };

        self.push(sample);
        self.accounting.samples_recorded += 1;

        if let Some(max) = self.config.max_samples {
            if self.accounting.samples_recorded >= max {
                self.status = CaptureStatus::Complete;
            }
        }
        true
    }

    fn push(&mut self, sample: TickSample) {
        if self.ring.len() < self.config.capacity {
            self.ring.push(sample);
            // Keep `write` pointing at the slot after the newest sample so the wrap below starts
            // overwriting the oldest one the moment the ring fills.
            self.write = self.ring.len() % self.config.capacity;
            return;
        }
        self.ring[self.write] = sample;
        self.write = (self.write + 1) % self.config.capacity;
        self.accounting.samples_overwritten += 1;
    }

    /// Record which executor the schedule is running under, so the export can say whether the
    /// checkpoint-bounded phases could have overlapped.
    pub fn set_executor(&mut self, executor: &'static str) {
        self.executor = executor;
    }
}

/// Why a tick was refused.
enum Rejection {
    Incomplete,
    OutOfOrder,
    Overflow,
}

fn classify_rejection(checkpoint_ns: [u64; 3], schedule_ns: u64, telemetry_ns: u64) -> Rejection {
    if checkpoint_ns.contains(&0) {
        return Rejection::Incomplete;
    }
    if checkpoint_ns[0] > checkpoint_ns[1]
        || checkpoint_ns[1] > checkpoint_ns[2]
        || checkpoint_ns[2] > schedule_ns
    {
        return Rejection::OutOfOrder;
    }
    let _ = telemetry_ns;
    Rejection::Overflow
}

/// Turn one tick's raw stamps into a sample, or refuse it.
///
/// A stamp of zero means the checkpoint never ran — a phase that was not measured, which must not be
/// reported as a zero-cost phase. Stamps that do not increase mean the executor ran the checkpoints
/// out of order, which makes every derived duration meaningless rather than merely noisy.
fn build_sample(
    tick: u64,
    agent_count: u32,
    checkpoint_ns: [u64; 3],
    schedule_ns: u64,
    telemetry_ns: u64,
) -> Option<TickSample> {
    if checkpoint_ns.contains(&0) {
        return None;
    }
    if checkpoint_ns[0] > checkpoint_ns[1] || checkpoint_ns[1] > checkpoint_ns[2] {
        return None;
    }
    if checkpoint_ns[2] > schedule_ns {
        return None;
    }
    let full_tick = schedule_ns.checked_add(telemetry_ns)?;

    let mut phase_ns = [0u64; TickPhase::COUNT];
    phase_ns[TickPhase::FullTick.index()] = full_tick;
    phase_ns[TickPhase::Schedule.index()] = schedule_ns;
    phase_ns[TickPhase::SensorBrain.index()] = checkpoint_ns[0];
    phase_ns[TickPhase::PhysicsMovement.index()] = checkpoint_ns[1] - checkpoint_ns[0];
    phase_ns[TickPhase::EcologyResources.index()] = checkpoint_ns[2] - checkpoint_ns[1];
    phase_ns[TickPhase::ScheduleTail.index()] = schedule_ns - checkpoint_ns[2];
    phase_ns[TickPhase::TelemetryPublish.index()] = telemetry_ns;

    Some(TickSample {
        tick,
        agent_count,
        phase_ns,
    })
}

// ---- Summaries ---------------------------------------------------------------------------------

/// The distribution of one phase over a capture.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseSummary {
    pub phase: String,
    /// Whether the bracket around this phase is exact — see the module docs.
    pub exact: bool,
    /// The system a checkpoint-bounded phase ends after.
    pub boundary_system: Option<String>,
    pub count: usize,
    pub mean_ns: f64,
    #[ts(type = "number")]
    pub p50_ns: u64,
    #[ts(type = "number")]
    pub p95_ns: u64,
    #[ts(type = "number")]
    pub p99_ns: u64,
    #[ts(type = "number")]
    pub max_ns: u64,
    #[ts(type = "number")]
    pub min_ns: u64,
    /// Mean nanoseconds per live agent, over the samples that had at least one agent. `None` when
    /// every sample was of an empty world, where a per-agent figure would be a division by zero
    /// dressed up as a measurement.
    pub mean_ns_per_agent: Option<f64>,
    /// Samples whose agent count was zero, and which therefore did not contribute to
    /// `mean_ns_per_agent`.
    pub samples_without_agents: usize,
}

/// The percentile convention this module uses, stated once so a reader never has to guess.
///
/// **Nearest rank on the sorted sample list**: the p-th percentile of `n` sorted values is the
/// element at index `ceil(p/100 * n) - 1`. It always returns a value that was actually measured —
/// no interpolation invents a duration the engine never took — and for `n = 1` every percentile is
/// that one sample. `p100` is the maximum by construction.
pub fn percentile_nearest_rank(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil();
    let idx = if rank < 1.0 {
        0usize
    } else {
        rank as usize - 1
    };
    sorted[idx.min(sorted.len() - 1)]
}

/// Summarise every enabled phase of a capture.
pub fn summarise(samples: &[TickSample], groups: PhaseMask) -> Vec<PhaseSummary> {
    let mut out = Vec::new();
    let mut scratch: Vec<u64> = Vec::with_capacity(samples.len());
    for phase in groups.phases() {
        scratch.clear();
        let i = phase.index();
        scratch.extend(samples.iter().map(|s| s.phase_ns[i]));
        scratch.sort_unstable();

        let count = scratch.len();
        let sum: u128 = scratch.iter().map(|v| *v as u128).sum();
        let mean_ns = if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        };

        let mut per_agent_sum = 0.0f64;
        let mut per_agent_n = 0usize;
        let mut without_agents = 0usize;
        for s in samples {
            if s.agent_count == 0 {
                without_agents += 1;
                continue;
            }
            per_agent_sum += s.phase_ns[i] as f64 / s.agent_count as f64;
            per_agent_n += 1;
        }

        out.push(PhaseSummary {
            phase: phase.id().to_string(),
            exact: phase.is_exact(),
            boundary_system: phase.boundary_system().map(str::to_string),
            count,
            mean_ns,
            p50_ns: percentile_nearest_rank(&scratch, 50.0),
            p95_ns: percentile_nearest_rank(&scratch, 95.0),
            p99_ns: percentile_nearest_rank(&scratch, 99.0),
            max_ns: scratch.last().copied().unwrap_or(0),
            min_ns: scratch.first().copied().unwrap_or(0),
            mean_ns_per_agent: if per_agent_n == 0 {
                None
            } else {
                Some(per_agent_sum / per_agent_n as f64)
            },
            samples_without_agents: without_agents,
        });
    }
    out
}

// ---- Export ------------------------------------------------------------------------------------

/// Machine context for an export.
///
/// Every field here is either read from the process or explicitly marked as not measured. There is
/// no CPU model string, no clock speed and no core count taken from a config file: this package did
/// not measure them, and a plausible-looking value nobody verified is worse than an honest absence.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareContext {
    pub target_arch: String,
    pub target_os: String,
    /// `std::thread::available_parallelism`, which is the process's usable concurrency rather than
    /// the machine's core count.
    pub available_parallelism: Option<u32>,
    /// Named so a reader knows what this export deliberately does **not** claim.
    pub not_measured: Vec<String>,
}

impl HardwareContext {
    pub fn current() -> Self {
        Self {
            target_arch: std::env::consts::ARCH.to_string(),
            target_os: std::env::consts::OS.to_string(),
            available_parallelism: std::thread::available_parallelism()
                .ok()
                .map(|n| n.get() as u32),
            not_measured: vec![
                "cpu_model".to_string(),
                "clock_speed".to_string(),
                "installed_ram".to_string(),
                "gpu".to_string(),
            ],
        }
    }
}

/// A capture, as it is written to disk and as the IPC surface returns it.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureExport {
    pub schema_version: u32,
    pub engine_version: String,
    pub profile: String,
    pub hardware: HardwareContext,
    pub workload: WorkloadContext,
    /// Which Bevy executor the schedule ran under while recording.
    pub executor: String,
    pub status: CaptureStatus,
    pub config: CaptureConfig,
    pub accounting: CaptureAccounting,
    pub phases: Vec<PhaseSummary>,
    /// Phases this build cannot measure, each with the reason.
    pub unavailable: Vec<UnavailablePhase>,
    /// Tick number of the oldest and newest retained sample, so a reader can see the window the
    /// summary covers rather than assuming it is the whole run.
    #[ts(type = "number | null")]
    pub first_tick: Option<u64>,
    #[ts(type = "number | null")]
    pub last_tick: Option<u64>,
}

/// Errors from exporting.
#[derive(Debug)]
pub enum CaptureExportError {
    Io(String),
    Serialize(String),
}

impl std::fmt::Display for CaptureExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "capture export could not be written: {e}"),
            Self::Serialize(e) => write!(f, "capture export could not be serialized: {e}"),
        }
    }
}

impl std::error::Error for CaptureExportError {}

// ---- The shared handle -------------------------------------------------------------------------

/// The handle the IPC commands and the simulation thread share.
///
/// Same shape as [`crate::core::simulation_lod::SharedLodFocus`] and for the same reason: commands
/// run on their own thread and cannot reach the simulation world's resources. A poisoned lock is
/// recovered rather than propagated — losing a profiler to a panic elsewhere must not take the
/// simulation with it.
#[derive(Clone, Debug)]
pub struct SharedTickCapture(pub Arc<Mutex<TickCaptureState>>);

impl Default for SharedTickCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedTickCapture {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(TickCaptureState::new())))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TickCaptureState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start a capture. Refuses an invalid configuration instead of clamping it.
    pub fn start(&self, config: CaptureConfig) -> Result<(), CaptureConfigError> {
        self.lock().start(config)
    }

    /// Stop recording. Idempotent, and keeps whatever was captured.
    pub fn stop(&self) {
        self.lock().stop();
    }

    pub fn status(&self) -> CaptureStatus {
        self.lock().status()
    }

    pub fn accounting(&self) -> CaptureAccounting {
        self.lock().accounting()
    }

    pub fn config(&self) -> CaptureConfig {
        self.lock().config()
    }

    pub fn sample_count(&self) -> usize {
        self.lock().len()
    }

    pub fn samples(&self) -> Vec<TickSample> {
        self.lock().samples()
    }

    pub fn set_executor(&self, executor: &'static str) {
        self.lock().set_executor(executor);
    }

    /// Whether the capture wants this tick's phases measured at all.
    ///
    /// The disabled path is this one boolean, read once per tick under an uncontended mutex, and
    /// nothing else: no allocation, no timestamps, no work inside the schedule.
    pub fn is_active(&self) -> bool {
        self.lock().status().is_active()
    }

    /// Hand one tick's measurements to the capture.
    pub fn observe_tick(
        &self,
        tick: u64,
        agent_count: u32,
        checkpoint_ns: [u64; 3],
        schedule_ns: u64,
        telemetry_ns: u64,
        workload: WorkloadContext,
    ) -> bool {
        self.lock().observe_tick(
            tick,
            agent_count,
            checkpoint_ns,
            schedule_ns,
            telemetry_ns,
            workload,
        )
    }

    /// Build the export document. The lock is held only long enough to copy the samples out, so a
    /// slow serialization never stalls the simulation thread.
    pub fn export(&self) -> CaptureExport {
        let (samples, status, config, accounting, workload, executor) = {
            let state = self.lock();
            (
                state.samples(),
                state.status(),
                state.config(),
                state.accounting(),
                state.workload(),
                state.executor(),
            )
        };
        let phases = summarise(&samples, config.groups);
        CaptureExport {
            schema_version: CAPTURE_SCHEMA_VERSION,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            hardware: HardwareContext::current(),
            workload,
            executor: executor.to_string(),
            status,
            config,
            accounting,
            phases,
            unavailable: unavailable_phases(),
            first_tick: samples.first().map(|s| s.tick),
            last_tick: samples.last().map(|s| s.tick),
        }
    }

    /// Write the export to `path`, atomically.
    ///
    /// Same temp-file-then-rename contract as [`crate::core::snapshot::write_atomic`], and it shares
    /// that function's implementation, so a failed write leaves whatever was at the target
    /// untouched and does not leave a partial file behind.
    pub fn export_to(&self, path: &std::path::Path) -> Result<CaptureExport, CaptureExportError> {
        let doc = self.export();
        let bytes = serde_json::to_vec_pretty(&doc)
            .map_err(|e| CaptureExportError::Serialize(e.to_string()))?;
        crate::core::snapshot::write_bytes_atomic(path, &bytes)
            .map_err(|e| CaptureExportError::Io(e.to_string()))?;
        Ok(doc)
    }
}

// ---- The in-world sink -------------------------------------------------------------------------

/// The world's end of a capture.
///
/// Absent from a world that is not capturing, which is what makes the disabled path free: the three
/// checkpoint systems return on their first line, and the simulation loop skips the whole block.
///
/// The per-tick stamps live here rather than behind the shared mutex so a tick takes the lock
/// exactly once — at commit — instead of once per checkpoint.
#[derive(bevy_ecs::prelude::Resource, Clone, Debug)]
pub struct TickCaptureSink {
    shared: SharedTickCapture,
    tick_start: Instant,
    checkpoint_ns: [u64; 3],
    agent_count: u32,
    workload: WorkloadContext,
    /// Set for the duration of a tick by [`begin_tick`](Self::begin_tick). The checkpoints do
    /// nothing while it is false, so a capture that stops mid-tick cannot leave half a sample.
    measuring: bool,
}

impl TickCaptureSink {
    pub fn new(shared: SharedTickCapture) -> Self {
        Self {
            shared,
            tick_start: Instant::now(),
            checkpoint_ns: [0; 3],
            agent_count: 0,
            workload: WorkloadContext::default(),
            measuring: false,
        }
    }

    /// The shared handle this sink writes into.
    pub fn shared(&self) -> &SharedTickCapture {
        &self.shared
    }

    /// Open a tick. Returns whether the checkpoints should measure it.
    pub fn begin_tick(&mut self) -> bool {
        self.measuring = self.shared.is_active();
        self.checkpoint_ns = [0; 3];
        self.agent_count = 0;
        self.workload = WorkloadContext::default();
        if self.measuring {
            self.tick_start = Instant::now();
        }
        self.measuring
    }

    /// Whether this tick is being measured.
    pub fn measuring(&self) -> bool {
        self.measuring
    }

    /// Stamp checkpoint `slot` with the nanoseconds elapsed since the tick began.
    ///
    /// A stamp is never allowed to be zero: zero is the sentinel for "this checkpoint did not run",
    /// and a tick fast enough to round to zero nanoseconds would otherwise be discarded as
    /// incomplete. One nanosecond is the smallest honest answer here.
    pub fn stamp(&mut self, slot: usize) {
        if !self.measuring || slot >= self.checkpoint_ns.len() {
            return;
        }
        let ns = self.tick_start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.checkpoint_ns[slot] = ns.max(1);
    }

    /// Record the workload the checkpoints observed.
    pub fn note_workload(&mut self, agent_count: u32, dims: Option<(usize, usize)>) {
        if !self.measuring {
            return;
        }
        self.agent_count = agent_count;
        self.workload = match dims {
            Some((w, h)) => WorkloadContext {
                world_width: w as u32,
                world_height: h as u32,
                dimensions_measured: true,
            },
            None => WorkloadContext::default(),
        };
    }

    /// Close the tick, handing its measurements to the shared capture.
    pub fn commit(&mut self, tick: u64, schedule_ns: u64, telemetry_ns: u64) -> bool {
        if !self.measuring {
            return false;
        }
        self.measuring = false;
        self.shared.observe_tick(
            tick,
            self.agent_count,
            self.checkpoint_ns,
            schedule_ns,
            telemetry_ns,
            self.workload,
        )
    }
}

// ---- Checkpoint systems ------------------------------------------------------------------------

/// Checkpoint after `action_resolution_system`.
pub fn capture_checkpoint_sensor_brain_system(
    sink: Option<bevy_ecs::prelude::ResMut<TickCaptureSink>>,
) {
    if let Some(mut sink) = sink {
        sink.stamp(0);
    }
}

/// Checkpoint after `rebuild_spatial_grid_system`.
pub fn capture_checkpoint_physics_system(sink: Option<bevy_ecs::prelude::ResMut<TickCaptureSink>>) {
    if let Some(mut sink) = sink {
        sink.stamp(1);
    }
}

/// Checkpoint after `ecosystem_census_system`, which is also where the workload is read: by then
/// every system that could have spawned or destroyed an agent this tick has run.
pub fn capture_checkpoint_ecology_system(
    sink: Option<bevy_ecs::prelude::ResMut<TickCaptureSink>>,
    agents: bevy_ecs::prelude::Query<(), bevy_ecs::prelude::With<crate::core::components::Agent>>,
    field: Option<bevy_ecs::prelude::Res<crate::core::ecology::ResourceField>>,
) {
    let Some(mut sink) = sink else {
        return;
    };
    if !sink.measuring() {
        return;
    }
    sink.stamp(2);
    let dims = field.map(|f| (f.width, f.height));
    sink.note_workload(agents.iter().count() as u32, dims);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_config() -> CaptureConfig {
        CaptureConfig {
            warmup_ticks: 0,
            capacity: 4,
            max_samples: None,
            sample_every: 1,
            groups: PhaseMask::all(),
        }
    }

    fn feed(state: &mut TickCaptureState, tick: u64, agents: u32, base: u64) -> bool {
        state.observe_tick(
            tick,
            agents,
            [base, base * 2, base * 3],
            base * 4,
            base,
            WorkloadContext {
                world_width: 256,
                world_height: 256,
                dimensions_measured: true,
            },
        )
    }

    #[test]
    fn a_zero_capacity_capture_is_refused_not_clamped() {
        let mut c = ok_config();
        c.capacity = 0;
        assert_eq!(c.validate(), Err(CaptureConfigError::ZeroCapacity));
    }

    #[test]
    fn every_malformed_configuration_names_its_own_problem() {
        let cases: Vec<(CaptureConfig, CaptureConfigError)> = vec![
            (
                CaptureConfig {
                    capacity: MAX_CAPTURE_CAPACITY + 1,
                    ..ok_config()
                },
                CaptureConfigError::CapacityTooLarge {
                    found: MAX_CAPTURE_CAPACITY + 1,
                    limit: MAX_CAPTURE_CAPACITY,
                },
            ),
            (
                CaptureConfig {
                    sample_every: 0,
                    ..ok_config()
                },
                CaptureConfigError::ZeroSampleRate,
            ),
            (
                CaptureConfig {
                    warmup_ticks: MAX_WARMUP_TICKS + 1,
                    ..ok_config()
                },
                CaptureConfigError::WarmupTooLong {
                    found: MAX_WARMUP_TICKS + 1,
                    limit: MAX_WARMUP_TICKS,
                },
            ),
            (
                CaptureConfig {
                    max_samples: Some(0),
                    ..ok_config()
                },
                CaptureConfigError::ZeroMaxSamples,
            ),
            (
                CaptureConfig {
                    groups: PhaseMask::none(),
                    ..ok_config()
                },
                CaptureConfigError::NoGroups,
            ),
            (
                CaptureConfig {
                    max_samples: Some(u64::MAX),
                    sample_every: 2,
                    ..ok_config()
                },
                CaptureConfigError::ScheduleOverflows,
            ),
        ];
        for (config, expected) in cases {
            assert_eq!(config.validate(), Err(expected.clone()), "{config:?}");
            // Every rejection explains itself; an empty message is a rejection nobody can act on.
            assert!(!expected.to_string().is_empty());
        }
    }

    #[test]
    fn the_ring_holds_exactly_capacity_and_counts_what_it_overwrote() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        for tick in 1..=10u64 {
            assert!(feed(&mut state, tick, 3, tick));
        }
        assert_eq!(state.len(), 4, "the ring never grows past capacity");
        let held = state.samples();
        assert_eq!(
            held.iter().map(|s| s.tick).collect::<Vec<_>>(),
            vec![7, 8, 9, 10],
            "the ring keeps the most recent samples, oldest first"
        );
        assert_eq!(state.accounting().samples_recorded, 10);
        assert_eq!(state.accounting().samples_overwritten, 6);
    }

    #[test]
    fn warmup_and_rate_are_accounted_separately() {
        let mut state = TickCaptureState::new();
        state
            .start(CaptureConfig {
                warmup_ticks: 3,
                capacity: 16,
                sample_every: 2,
                ..ok_config()
            })
            .expect("valid config");
        for tick in 1..=11u64 {
            feed(&mut state, tick, 2, tick);
        }
        let acc = state.accounting();
        assert_eq!(acc.ticks_observed, 11);
        assert_eq!(acc.warmup_discarded, 3);
        // Eight ticks survive warm-up; every second one is recorded starting with the first.
        assert_eq!(acc.samples_recorded, 4);
        assert_eq!(acc.rate_skipped, 4);
        assert_eq!(
            state.samples().iter().map(|s| s.tick).collect::<Vec<_>>(),
            vec![4, 6, 8, 10]
        );
    }

    #[test]
    fn max_samples_completes_the_capture_without_a_caller() {
        let mut state = TickCaptureState::new();
        state
            .start(CaptureConfig {
                max_samples: Some(2),
                capacity: 8,
                ..ok_config()
            })
            .expect("valid config");
        assert!(feed(&mut state, 1, 1, 10));
        assert_eq!(state.status(), CaptureStatus::Recording);
        assert!(feed(&mut state, 2, 1, 10));
        assert_eq!(state.status(), CaptureStatus::Complete);
        // A completed capture stops consuming ticks but keeps what it has.
        assert!(!feed(&mut state, 3, 1, 10));
        assert_eq!(state.len(), 2);
    }

    #[test]
    fn a_missing_checkpoint_is_dropped_rather_than_reported_as_zero_cost() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        // The physics checkpoint never ran.
        assert!(!state.observe_tick(1, 5, [10, 0, 30], 40, 5, WorkloadContext::default()));
        assert_eq!(state.accounting().dropped_incomplete, 1);
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn checkpoints_that_ran_out_of_order_are_dropped_and_counted() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        assert!(!state.observe_tick(1, 5, [30, 20, 40], 50, 5, WorkloadContext::default()));
        assert_eq!(state.accounting().dropped_out_of_order, 1);
        // A checkpoint later than the schedule bracket that contains it is the same failure.
        assert!(!state.observe_tick(2, 5, [10, 20, 90], 50, 5, WorkloadContext::default()));
        assert_eq!(state.accounting().dropped_out_of_order, 2);
    }

    #[test]
    fn a_full_tick_that_overflows_u64_is_dropped_not_wrapped() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        assert!(!state.observe_tick(
            1,
            5,
            [1, 2, 3],
            u64::MAX,
            u64::MAX,
            WorkloadContext::default()
        ));
        assert_eq!(state.accounting().dropped_overflow, 1);
    }

    #[test]
    fn percentiles_are_nearest_rank_and_never_interpolate() {
        let one = [7u64];
        for p in [0.0, 50.0, 95.0, 99.0, 100.0] {
            assert_eq!(percentile_nearest_rank(&one, p), 7);
        }
        let two = [1u64, 2];
        assert_eq!(percentile_nearest_rank(&two, 50.0), 1);
        assert_eq!(percentile_nearest_rank(&two, 51.0), 2);
        assert_eq!(percentile_nearest_rank(&two, 100.0), 2);

        // 1..=100: nearest rank puts p50 at index 49 and p99 at index 98.
        let hundred: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile_nearest_rank(&hundred, 50.0), 50);
        assert_eq!(percentile_nearest_rank(&hundred, 95.0), 95);
        assert_eq!(percentile_nearest_rank(&hundred, 99.0), 99);
        assert_eq!(percentile_nearest_rank(&hundred, 100.0), 100);
        assert_eq!(percentile_nearest_rank(&[], 50.0), 0);
    }

    #[test]
    fn an_empty_capture_summarises_to_zeroes_rather_than_a_panic() {
        let rows = summarise(&[], PhaseMask::all());
        assert_eq!(rows.len(), TickPhase::COUNT);
        for row in rows {
            assert_eq!(row.count, 0);
            assert_eq!(row.mean_ns, 0.0);
            assert_eq!(row.p50_ns, 0);
            assert_eq!(row.max_ns, 0);
            assert_eq!(row.mean_ns_per_agent, None);
        }
    }

    #[test]
    fn a_single_sample_summarises_to_itself() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        assert!(state.observe_tick(
            9,
            4,
            [100, 300, 600],
            1_000,
            200,
            WorkloadContext::default()
        ));
        let rows = summarise(&state.samples(), PhaseMask::all());
        let full = rows.iter().find(|r| r.phase == "full_tick").expect("row");
        assert_eq!(full.count, 1);
        assert_eq!(full.p50_ns, 1_200);
        assert_eq!(full.p99_ns, 1_200);
        assert_eq!(full.max_ns, 1_200);
        assert_eq!(full.min_ns, 1_200);
        assert_eq!(full.mean_ns, 1_200.0);
        assert_eq!(full.mean_ns_per_agent, Some(300.0));

        let physics = rows
            .iter()
            .find(|r| r.phase == "physics_movement")
            .expect("row");
        assert_eq!(
            physics.p50_ns, 200,
            "300ns checkpoint minus the 100ns before it"
        );
        assert!(!physics.exact);
        assert_eq!(
            physics.boundary_system.as_deref(),
            Some("rebuild_spatial_grid_system")
        );
        let tail = rows
            .iter()
            .find(|r| r.phase == "schedule_tail")
            .expect("row");
        assert_eq!(tail.p50_ns, 400);
    }

    #[test]
    fn a_world_with_no_agents_reports_no_per_agent_figure() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        assert!(state.observe_tick(1, 0, [10, 20, 30], 40, 5, WorkloadContext::default()));
        let rows = summarise(&state.samples(), PhaseMask::all());
        for row in &rows {
            assert_eq!(row.mean_ns_per_agent, None);
            assert_eq!(row.samples_without_agents, 1);
        }
    }

    #[test]
    fn the_group_mask_selects_rows_without_changing_them() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        for tick in 1..=3u64 {
            feed(&mut state, tick, 2, tick * 10);
        }
        let samples = state.samples();
        let all = summarise(&samples, PhaseMask::all());
        let narrow = summarise(&samples, PhaseMask::of(&[TickPhase::Schedule]));
        assert_eq!(narrow.len(), 1);
        let wide_schedule = all.iter().find(|r| r.phase == "schedule").expect("row");
        assert_eq!(&narrow[0], wide_schedule, "a narrow mask never rescales");
    }

    #[test]
    fn a_stopped_capture_keeps_its_samples_and_ignores_further_ticks() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        feed(&mut state, 1, 1, 10);
        state.stop();
        assert_eq!(state.status(), CaptureStatus::Stopped);
        assert!(!feed(&mut state, 2, 1, 10));
        assert_eq!(state.len(), 1);
        // Stopping twice is not an error, and does not resurrect anything.
        state.stop();
        assert_eq!(state.status(), CaptureStatus::Stopped);
    }

    #[test]
    fn an_idle_capture_records_nothing() {
        let mut state = TickCaptureState::new();
        assert_eq!(state.status(), CaptureStatus::Idle);
        assert!(!feed(&mut state, 1, 1, 10));
        assert_eq!(state.accounting().ticks_observed, 0);
    }

    #[test]
    fn restarting_discards_the_previous_capture() {
        let mut state = TickCaptureState::new();
        state.start(ok_config()).expect("valid config");
        feed(&mut state, 1, 1, 10);
        assert_eq!(state.len(), 1);
        state.start(ok_config()).expect("valid config");
        assert_eq!(state.len(), 0);
        assert_eq!(state.accounting(), CaptureAccounting::default());
    }

    #[test]
    fn env_spec_parsing_ignores_a_bad_value_but_refuses_a_bad_combination() {
        let c = CaptureConfig::parse("warmup=10,capacity=32,every=3").expect("valid");
        assert_eq!(c.warmup_ticks, 10);
        assert_eq!(c.capacity, 32);
        assert_eq!(c.sample_every, 3);

        // An unparseable number leaves the default in place rather than taking the run down.
        let c = CaptureConfig::parse("capacity=banana").expect("valid");
        assert_eq!(c.capacity, CaptureConfig::default().capacity);

        // A combination that cannot work is still refused.
        assert_eq!(
            CaptureConfig::parse("capacity=0"),
            Err(CaptureConfigError::ZeroCapacity)
        );
        // A group list that names nothing known leaves an empty mask, which is refused.
        assert_eq!(
            CaptureConfig::parse("groups=nonsense"),
            Err(CaptureConfigError::NoGroups)
        );
        let c = CaptureConfig::parse("groups=schedule+full_tick").expect("valid");
        assert!(c.groups.contains(TickPhase::Schedule));
        assert!(c.groups.contains(TickPhase::FullTick));
        assert!(!c.groups.contains(TickPhase::SensorBrain));
    }

    #[test]
    fn phase_ids_round_trip_and_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for p in TickPhase::ALL {
            assert!(seen.insert(p.id()), "duplicate phase id {}", p.id());
            assert_eq!(TickPhase::from_id(p.id()), Some(p));
            assert!(p.index() < TickPhase::COUNT);
        }
        assert_eq!(seen.len(), TickPhase::COUNT);
        assert_eq!(TickPhase::from_id("not_a_phase"), None);
    }

    #[test]
    fn the_export_names_what_it_did_not_measure() {
        let shared = SharedTickCapture::new();
        shared.start(ok_config()).expect("valid config");
        shared.set_executor("single-threaded");
        shared.observe_tick(
            5,
            3,
            [10, 20, 30],
            40,
            5,
            WorkloadContext {
                world_width: 256,
                world_height: 256,
                dimensions_measured: true,
            },
        );
        let doc = shared.export();
        assert_eq!(doc.schema_version, CAPTURE_SCHEMA_VERSION);
        assert_eq!(doc.workload.world_width, 256);
        assert!(doc.workload.dimensions_measured);
        assert_eq!(doc.executor, "single-threaded");
        assert_eq!(doc.first_tick, Some(5));
        assert_eq!(doc.last_tick, Some(5));
        assert!(
            doc.unavailable.iter().any(|u| u.id == "plant_soil_weather"),
            "a phase this build cannot measure must be named, not omitted"
        );
        assert!(doc.hardware.not_measured.contains(&"cpu_model".to_string()));
    }

    #[test]
    fn concurrent_status_reads_do_not_block_or_tear() {
        let shared = SharedTickCapture::new();
        shared
            .start(CaptureConfig {
                capacity: 512,
                ..ok_config()
            })
            .expect("valid config");
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let s = shared.clone();
                std::thread::spawn(move || {
                    let mut seen = 0usize;
                    for _ in 0..500 {
                        let _ = s.status();
                        let acc = s.accounting();
                        // Recorded samples can never exceed the ticks the capture was given.
                        assert!(acc.samples_recorded <= acc.ticks_observed);
                        seen += s.sample_count();
                    }
                    seen
                })
            })
            .collect();
        for tick in 1..=200u64 {
            shared.observe_tick(tick, 2, [1, 2, 3], 4, 1, WorkloadContext::default());
        }
        for r in readers {
            let _ = r.join().expect("reader thread");
        }
        assert_eq!(shared.sample_count(), 200);
        assert_eq!(shared.accounting().samples_recorded, 200);
    }

    #[test]
    fn export_is_atomic_and_a_failed_write_preserves_what_was_there() {
        let dir = std::env::temp_dir().join(format!("anima_capture_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let shared = SharedTickCapture::new();
        shared.start(ok_config()).expect("valid config");
        shared.observe_tick(1, 2, [10, 20, 30], 40, 5, WorkloadContext::default());

        let path = dir.join("capture.json");
        shared.export_to(&path).expect("export");
        let back: CaptureExport =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("parse");
        assert_eq!(back.accounting.samples_recorded, 1);

        let strays: Vec<_> = std::fs::read_dir(&dir)
            .expect("dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "temp files left behind: {strays:?}");

        // A target that is a directory cannot be renamed over. The write must fail, the directory
        // must survive, and no debris may be left in it.
        let occupied = dir.join("occupied");
        std::fs::create_dir_all(&occupied).expect("occupied dir");
        let sentinel = occupied.join("keep.txt");
        std::fs::write(&sentinel, b"do not lose me").expect("sentinel");
        let err = shared.export_to(&occupied);
        assert!(err.is_err(), "writing over a directory must fail");
        assert!(sentinel.exists(), "a failed export destroyed existing data");
        let debris: Vec<_> = std::fs::read_dir(&dir)
            .expect("dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(debris.is_empty(), "temp files left behind: {debris:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sink_takes_the_lock_once_per_tick_and_never_half_commits() {
        let shared = SharedTickCapture::new();
        let mut sink = TickCaptureSink::new(shared.clone());
        // Nothing started: the whole tick is skipped.
        assert!(!sink.begin_tick());
        sink.stamp(0);
        assert!(!sink.commit(1, 100, 10));
        assert_eq!(shared.sample_count(), 0);

        shared.start(ok_config()).expect("valid config");
        assert!(sink.begin_tick());
        sink.stamp(0);
        sink.stamp(1);
        sink.stamp(2);
        sink.note_workload(7, Some((256, 256)));
        assert!(sink.commit(1, 10_000_000, 1_000));
        assert_eq!(shared.sample_count(), 1);
        assert_eq!(shared.samples()[0].agent_count, 7);
        assert!(shared.export().workload.dimensions_measured);

        // Committing twice cannot produce a second sample from one tick.
        assert!(!sink.commit(1, 10_000_000, 1_000));
        assert_eq!(shared.sample_count(), 1);
    }
}

#[cfg(test)]
mod auto_export_env_tests {
    use super::*;

    /// These tests write a process-wide environment variable, so they share a lock: Rust runs tests
    /// in one process on many threads, and two of them racing on the same variable is the failure
    /// that passes alone and fails in the suite.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard(Option<String>);

    impl EnvGuard {
        fn set(value: Option<&str>) -> Self {
            let previous = std::env::var("ANIMA_TICK_CAPTURE_OUT").ok();
            match value {
                Some(v) => std::env::set_var("ANIMA_TICK_CAPTURE_OUT", v),
                None => std::env::remove_var("ANIMA_TICK_CAPTURE_OUT"),
            }
            Self(previous)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(v) => std::env::set_var("ANIMA_TICK_CAPTURE_OUT", v),
                None => std::env::remove_var("ANIMA_TICK_CAPTURE_OUT"),
            }
        }
    }

    /// Unset is the whole compatibility story: no file, no path resolution, no behaviour change.
    #[test]
    fn unset_writes_nothing() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _guard = EnvGuard::set(None);
        assert_eq!(auto_export_name_from_env(), None);
    }

    /// An empty or blank value reads as "not asked for" rather than as a file called "". A shell
    /// script that builds the value from another variable produces exactly this when that other
    /// variable is missing, and it must not become a filesystem error deep in the engine thread.
    #[test]
    fn blank_reads_as_unset() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for blank in ["", "   ", "\t"] {
            let _guard = EnvGuard::set(Some(blank));
            assert_eq!(
                auto_export_name_from_env(),
                None,
                "{blank:?} should read as unset"
            );
        }
    }

    #[test]
    fn a_name_survives_surrounding_space() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _guard = EnvGuard::set(Some("  tick-capture-release  "));
        assert_eq!(
            auto_export_name_from_env().as_deref(),
            Some("tick-capture-release")
        );
    }

    /// The value is a name and stays a name. Rejecting a path is `save_paths`' job and it has its
    /// own tests; what this asserts is that this function does not quietly pre-empt that decision
    /// by trimming a traversal into something acceptable.
    #[test]
    fn a_path_shaped_value_is_passed_through_intact_for_save_paths_to_refuse() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _guard = EnvGuard::set(Some("../../etc/passwd"));
        assert_eq!(
            auto_export_name_from_env().as_deref(),
            Some("../../etc/passwd"),
            "the traversal must reach save_paths, not be silently repaired here"
        );
        let dir = std::env::temp_dir().join("anima-auto-export-test");
        assert!(
            crate::commands::save_paths::resolve_save_path(&dir, "../../etc/passwd").is_err(),
            "save_paths must refuse the value this function passes through"
        );
    }
}
