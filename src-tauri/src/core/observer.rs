//! Observer policy (ADR-0004) — the declared relationship between whoever is watching and the world.
//!
//! ## Why this exists
//!
//! Watching is not free of consequence here. [`LodFocus`](crate::core::simulation_lod::LodFocus)
//! carries the observer's camera position, [`tier_at`](crate::core::simulation_lod::tier_at) bands
//! agents by their distance from it, and a `Cold` agent **does not think at all** — held by
//! `cold_agents_stop_asking_entirely`. So where a user looks decides which agents get to think.
//!
//! That is an observer effect, it is already in the engine, and until this module it sat outside
//! every provenance mechanism the project has. `DETERMINISM_CONTRACT.md` §2 lists four sources of
//! outside-world leakage — `Uuid::new_v4()`, `SystemTime::now()`, Gemini, Bevy's system order. The
//! camera is the fifth.
//!
//! Research runs are clean today, but only because `LodFocus::default()` is disabled and a headless
//! run has no camera to write one. That is a side effect of having no UI, not a contract. This type
//! turns it into a contract.
//!
//! ## Reproducible is not the same as non-perturbing
//!
//! Tiering is already **reproducible**: it is a pure function of `(focus, entity_index, tick)` with
//! no clock and no RNG (`tiering_is_reproducible`). It is not **non-perturbing**, and it cannot be —
//! a `Cold` agent skipping inference is the entire point of the saving. Conflating the two is the
//! mistake this module is shaped to prevent, so the policy names the distinction instead of
//! pretending it away.
//!
//! ## Unset is not the same as [`ObserverPolicy::Absent`]
//!
//! A missing policy **resource** means nobody declared one, and must behave exactly as the engine did
//! before this module existed — the camera is obeyed. `Absent` is the opposite: a positive
//! declaration that no observer exists, which forbids the camera from tiering anything.
//!
//! The difference is load-bearing. The live app has been driving `set_lod_focus` from
//! `PixiViewport.tsx` since simulation LOD was wired up; if "unset" denied the focus, this module
//! would silently switch that off and call it a safety improvement.

use crate::core::simulation_lod::LodFocus;
use anima_domain::causal::{CauseId, CAUSE_BACKGROUND, CAUSE_OBSERVER};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// How an observer is allowed to relate to the world for the duration of a run.
///
/// Internally tagged so a manifest reads as `{"mode": "spectate"}` and gains a `cause_id` only where
/// one is meaningful.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ObserverPolicy {
    /// No observer. The headless path, and the rollback: a manifest without an `observer` key reads
    /// as this, and behaves as every manifest did before ADR-0004.
    #[default]
    Absent,

    /// Someone is watching, read-only, and **tiering is off**.
    ///
    /// The trajectory is required to be identical to [`Absent`] — that equality is the whole promise,
    /// and `spectate_matches_absent` is what holds it. The price is the LOD saving: every agent stays
    /// `Hot`, so fewer of them fit in the frame budget. That is a declared trade, not a surprise.
    Spectate,

    /// The observer perturbs the world, and says so.
    ///
    /// The camera is allowed to tier, which means this run is a **different treatment** from a
    /// headless one — not a contaminated version of the same one. `cause_id` roots the resulting
    /// effects in the causal ledger so `trace_to_root` can answer "a human did this".
    ///
    /// O1 delivers the declaration and the enforcement. **Recording the observer's trace is O2 and
    /// replay is O3**, so an `Inhabit` run is reproducible only to the extent the live engine already
    /// is — see `DETERMINISM_CONTRACT.md` §5. Do not claim replay for it yet.
    Inhabit { cause_id: CauseId },
}

impl ObserverPolicy {
    /// Whether the camera may tier the world. Only [`Inhabit`](Self::Inhabit) may.
    ///
    /// [`Absent`](Self::Absent) and [`Spectate`](Self::Spectate) agree here and differ only in what
    /// they declare — which is correct: they are required to produce the same trajectory.
    pub fn allows_focus(&self) -> bool {
        matches!(self, Self::Inhabit { .. })
    }

    /// Whether a run under this policy may be compared, trajectory for trajectory, against a
    /// headless run of the same manifest.
    ///
    /// `false` for [`Inhabit`](Self::Inhabit): that run is a different treatment, and presenting the
    /// two side by side as repeats of one experiment is the failure mode ADR-0004 is guarding.
    pub fn is_comparable_to_headless(&self) -> bool {
        !self.allows_focus()
    }

    /// The cause the observer's effects root at, if this policy has one.
    pub fn cause_id(&self) -> Option<CauseId> {
        match *self {
            Self::Inhabit { cause_id } => Some(cause_id),
            _ => None,
        }
    }

    /// Why this policy is malformed, or `None` when it is well-formed.
    ///
    /// An [`Inhabit`](Self::Inhabit) rooted at [`CAUSE_BACKGROUND`] is the one way to state this
    /// wrongly: it declares that a human perturbed the world and then files the consequences under
    /// baseline dynamics, so `trace_to_root` would report the observer's own doing as something the
    /// world did by itself. That is worse than not declaring at all.
    pub fn rejection_reason(&self) -> Option<String> {
        match *self {
            Self::Inhabit { cause_id } if cause_id == CAUSE_BACKGROUND => Some(
                "Inhabit must carry a cause id of its own; CAUSE_BACKGROUND would file the \
                 observer's effects as baseline dynamics"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

// ---- Observer trace (ADR-0004 O2) ---------------------------------------------------------------

/// 60 Hz for an hour is 216 000 ticks, and a continuously-panning camera changes on every one of
/// them. Sized for that worst case rather than the typical one: being too small truncates the
/// record, and being too large costs a few MiB once.
///
/// The exact cost is asserted rather than estimated — see
/// `a_full_hour_of_trace_fits_a_declared_budget`.
pub const DEFAULT_OBSERVER_TRACE_CAPACITY: usize = 216_000;

/// Room for the actions a human takes in a long session.
///
/// Sized against a person, not against the tick rate: an observer clicking steadily every two seconds
/// for an hour produces about 1,800 actions. Ten thousand is generous for that and costs ~160 KiB,
/// which is nothing beside the focus buffer beside it.
pub const DEFAULT_OBSERVER_ACTION_CAPACITY: usize = 10_000;

/// What the world saw of the observer at one tick.
///
/// The **effective** focus, after [`ObserverPolicy`] has had its say — not what the UI asked for.
/// Under `Spectate` the camera moves and this does not, which is exactly the record wanted: the
/// trace is evidence of what the world was subjected to, not of where a human happened to look.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObserverSample {
    pub tick: u64,
    pub focus: LodFocus,
}

/// The observer's effect on the world over a run, recorded on change (ADR-0004 C2).
///
/// ## Why recording is enough to be useful before replay exists
///
/// O3 — replaying an `Inhabit` run without a human — needs the live engine to be deterministic, and
/// it is not yet (`DETERMINISM_CONTRACT` §5). Provenance does not wait for that. "Why did that herd
/// die out" is answerable as soon as the observer's presence is on the record; "run it again exactly"
/// is a later and larger promise. This type delivers the first without pretending to the second.
///
/// ## Bounded, and honest about it
///
/// The buffer is allocated once and never grows, because this is written from the tick path and the
/// hot loop may not allocate. When it fills, further samples are **counted, not silently dropped** —
/// [`dropped`](Self::dropped) and [`is_truncated`](Self::is_truncated) exist so a trace can say it is
/// partial. A trace that quietly stopped recording would read exactly like a camera that stopped
/// moving, and those two must never look the same.
#[derive(Resource, Clone, Debug)]
pub struct ObserverTrace {
    samples: Vec<ObserverSample>,
    /// What the observer *did*, kept beside what they *saw* rather than inside `ObserverSample`.
    ///
    /// Two buffers because they answer different questions and have different shapes. The focus
    /// timeline is a step function that replay reconstructs by holding between samples; actions are
    /// discrete events at a tick and holding one would be meaningless. Folding actions into
    /// `ObserverSample` would also change the type `ObserverReplay` reads, for no gain.
    actions: Vec<ObserverActionRecord>,
    dropped: u64,
    dropped_actions: u64,
    last: Option<LodFocus>,
}

impl Default for ObserverTrace {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_OBSERVER_TRACE_CAPACITY)
    }
}

impl ObserverTrace {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            // Actions come from a human at UI speed, not from the tick loop, so the buffer is sized
            // for a long session of clicking rather than for 60 Hz. A capacity tied to `capacity`
            // would scale it against the wrong thing entirely.
            actions: Vec::with_capacity(DEFAULT_OBSERVER_ACTION_CAPACITY),
            dropped: 0,
            dropped_actions: 0,
            last: None,
        }
    }

    /// Store one action, or count it if the buffer is full.
    ///
    /// Same contract as [`record`](Self::record): the buffer never grows, because this runs from the
    /// tick path. Unlike focus samples there is no de-duplication — two identical actions a second
    /// apart are two things a human did, not one.
    pub fn record_action(&mut self, record: ObserverActionRecord) -> bool {
        if self.actions.len() == self.actions.capacity() {
            self.record_dropped_actions(1);
            return false;
        }
        self.actions.push(record);
        true
    }

    fn record_dropped_actions(&mut self, count: u64) {
        self.dropped_actions = self.dropped_actions.saturating_add(count);
    }

    pub fn actions(&self) -> &[ObserverActionRecord] {
        &self.actions
    }

    /// How many actions could not be stored. Counted separately from focus samples: losing a
    /// human's action is a provenance hole, while losing a focus sample only costs replay fidelity.
    pub fn dropped_actions(&self) -> u64 {
        self.dropped_actions
    }

    /// Record `focus` at `tick` if it differs from the last thing recorded.
    ///
    /// Returns whether a sample was stored. Allocation-free: the buffer never grows, so a full trace
    /// counts the sample as dropped instead of reallocating on the tick path.
    ///
    /// A focus whose centre has gone to NaN compares unequal to itself and would therefore record
    /// every tick until the buffer fills. That is deliberate — it is a real fault becoming visible
    /// and self-limiting, rather than one being smoothed over.
    pub fn record(&mut self, tick: u64, focus: LodFocus) -> bool {
        if self.last == Some(focus) {
            return false;
        }
        if self.samples.len() == self.samples.capacity() {
            self.dropped += 1;
            return false;
        }
        self.samples.push(ObserverSample { tick, focus });
        self.last = Some(focus);
        true
    }

    pub fn samples(&self) -> &[ObserverSample] {
        &self.samples
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// How many changes could not be stored because the buffer was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Whether this trace is missing samples, and therefore cannot support a faithful replay.
    pub fn is_truncated(&self) -> bool {
        self.dropped > 0
    }
}

/// A recorded trace, played back in place of a live camera (ADR-0004 O3).
///
/// ## The interpolation is declared, not assumed
///
/// The trace stores changes, not ticks, so playback has to say what happens between two samples.
/// It **holds**: the focus in force at a tick is the last one recorded at or before it. A step
/// function, chosen because it is what recording actually observed — `record` only stores a sample
/// when the value changed, so holding reconstructs the original signal exactly rather than
/// approximating it. ADR-0004 C2 requires this to be stated somewhere rather than left to whoever
/// reads the buffer next; this is that statement.
///
/// ## Replay excludes the live camera
///
/// When this resource is present, [`sync_lod_focus_system`](crate::core::simulation_lod::sync_lod_focus_system)
/// ignores [`SharedLodFocus`](crate::core::simulation_lod::SharedLodFocus) entirely. Not blended,
/// not preferred — ignored. A `set_lod_focus` arriving mid-replay from a UI nobody remembered to
/// close would otherwise steer the run while the trace was still being credited for it, which is
/// the one way a replay can lie about what it reproduced.
#[derive(Resource, Clone, Debug)]
pub struct ObserverReplay {
    samples: Vec<ObserverSample>,
    cursor: usize,
    current: LodFocus,
}

impl ObserverReplay {
    pub fn from_samples(samples: Vec<ObserverSample>) -> Self {
        Self {
            samples,
            cursor: 0,
            current: LodFocus::default(),
        }
    }

    pub fn from_trace(trace: &ObserverTrace) -> Self {
        Self::from_samples(trace.samples().to_vec())
    }

    /// The focus in force at `tick`, advancing through any samples that have come due.
    ///
    /// Allocation-free: it walks a cursor forward over a buffer it already owns.
    pub fn focus_at(&mut self, tick: u64) -> LodFocus {
        while self.cursor < self.samples.len() && self.samples[self.cursor].tick <= tick {
            self.current = self.samples[self.cursor].focus;
            self.cursor += 1;
        }
        self.current
    }

    /// Whether every recorded sample has been played.
    ///
    /// Past the end the last focus keeps holding, which is right: the recording ended because the
    /// camera stopped changing, not because it vanished.
    pub fn is_exhausted(&self) -> bool {
        self.cursor >= self.samples.len()
    }

    pub fn remaining(&self) -> usize {
        self.samples.len().saturating_sub(self.cursor)
    }
}

// ---- Observer actions (ADR-0004 C3) -------------------------------------------------------------

/// Something a human did to the running world, beyond looking at it.
///
/// ## These are not future actions
///
/// ADR-0004 O2 deferred `ObserverSample.actions` on the grounds that the engine had no embodied
/// actions yet — the observer had a camera and nothing else. That was wrong, and finding out changed
/// the scope: **every variant below is an IPC command that already existed and already wrote straight
/// into shared state with no declaration, no record and no attribution.**
///
/// The ADR named the camera as the fifth source of outside-world leakage in
/// `DETERMINISM_CONTRACT` §2. These are stronger than the camera. The camera changes *which agents
/// think*; [`EvolutionSettingsChanged`](Self::EvolutionSettingsChanged) changes the mutation rate and
/// selection bias that selection itself runs under, and
/// [`MigrationTriggered`](Self::MigrationTriggered) moves populations between shards.
///
/// ## What this type does, and does not, change
///
/// Recording only. The commands still do exactly what they did — this makes their effects
/// **attributable**, it does not yet make the seam mandatory. Enforcement (an observer may write the
/// world *only* through here) is a separate change, and it needs the causal ledger to exist in the
/// live Bevy world, which is G2. Record first, enforce second, mirroring O2-then-O3.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObserverAction {
    /// `update_evolution_settings`. A human changing the mutation rate mid-run is a change to the
    /// laws selection operates under, applied to a population already living under the old ones.
    EvolutionSettingsChanged {
        /// `f64` to match `EvolutionSettings` exactly. Narrowing to `f32` here would make the record
        /// a lossy copy of the value the world actually took, which is the one thing a provenance
        /// record must not be.
        mutation_rate: f64,
        selection_bias: f64,
        grid_resolution: u32,
    },
    /// `toggle_evolution`. Selection stops or starts; the ecology keeps running either way.
    EvolutionToggled { running: bool },
    /// `trigger_migration`. Agents leave for another shard because someone asked them to.
    MigrationTriggered { target_port: u16 },
    /// `set_sharding_config`. Changes how the world is partitioned under a running population.
    ShardingConfigChanged { local_port: u16 },
}

impl ObserverAction {
    /// The IPC command this action came from, so a trace can be read back against `PROJECT.md`'s
    /// documented surface rather than against this enum's own naming.
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::EvolutionSettingsChanged { .. } => "update_evolution_settings",
            Self::EvolutionToggled { .. } => "toggle_evolution",
            Self::MigrationTriggered { .. } => "trigger_migration",
            Self::ShardingConfigChanged { .. } => "set_sharding_config",
        }
    }

    /// Human-readable "why" for a [`CausalLedger`](anima_domain::causal::CausalLedger) entry.
    ///
    /// Allocates, and deliberately not called from the tick path — recording a sample stores the
    /// `Copy` action and nothing else; this is for the ledger, which lives in the headless slice.
    pub fn mechanism(&self) -> String {
        match *self {
            Self::EvolutionSettingsChanged {
                mutation_rate,
                selection_bias,
                grid_resolution,
            } => format!(
                "an observer set mutation_rate={mutation_rate}, selection_bias={selection_bias}, \
                 grid_resolution={grid_resolution} mid-run"
            ),
            Self::EvolutionToggled { running } => {
                format!(
                    "an observer turned evolution {}",
                    if running { "on" } else { "off" }
                )
            }
            Self::MigrationTriggered { target_port } => {
                format!("an observer sent agents to shard on port {target_port}")
            }
            Self::ShardingConfigChanged { local_port } => {
                format!("an observer repartitioned the world, local_port={local_port}")
            }
        }
    }
}

/// One action, when it happened, and whose doing it was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObserverActionRecord {
    pub tick: u64,
    pub action: ObserverAction,
    /// Always [`CAUSE_OBSERVER`] today. Carried explicitly rather than implied so a trace merged from
    /// more than one source cannot lose which effects a human owns.
    pub cause_id: CauseId,
}

/// Actions as the **app** holds them, across the thread boundary.
///
/// Tauri commands run on their own thread and cannot touch the world's resources, which is the same
/// constraint that made [`SharedLodFocus`](crate::core::simulation_lod::SharedLodFocus) a handle. A
/// command pushes here; [`drain_observer_actions_system`] moves them into the
/// [`ObserverTrace`] once per tick.
const DEFAULT_OBSERVER_ACTION_INGRESS_CAPACITY: usize = 64;

struct ObserverActionQueue {
    records: Vec<ObserverActionRecord>,
    limit: usize,
    dropped: u64,
}

#[derive(Resource, Clone)]
pub struct SharedObserverActions(Arc<Mutex<ObserverActionQueue>>);

impl Default for SharedObserverActions {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedObserverActions {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_OBSERVER_ACTION_INGRESS_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(Mutex::new(ObserverActionQueue {
            records: Vec::with_capacity(capacity),
            limit: capacity,
            dropped: 0,
        })))
    }

    /// Called from a Tauri command. Never blocks the world: a poisoned lock drops the record rather
    /// than propagating a panic into the IPC layer, and says so.
    pub fn push(&self, action: ObserverAction) {
        match self.0.lock() {
            Ok(mut queue) => {
                if queue.records.len() >= queue.limit {
                    queue.dropped = queue.dropped.saturating_add(1);
                    return;
                }
                queue.records.push(ObserverActionRecord {
                    // The tick is stamped by the drain, which is the only place that knows it. Zero
                    // here means "not yet stamped" and is never observed outside this queue.
                    tick: 0,
                    action,
                    cause_id: CAUSE_OBSERVER,
                });
            }
            Err(_) => eprintln!(
                "observer action '{}' was not recorded: the action queue's lock is poisoned",
                action.command_name()
            ),
        }
    }
}

/// The only place a human's write reaches the world (ADR-0004 C3, enforcement).
///
/// ## What this removes
///
/// Before this, each of the four commands pushed an [`ObserverAction`] and *then* wrote shared state
/// — two statements that a new command, or an edited one, could trivially get down to one. The
/// source-scanning test in `observer_action_tests` existed precisely because nothing structural
/// stopped that.
///
/// Here recording and writing are **one call**. There is no way to perform the write and skip the
/// record, because the caller never sees the write.
///
/// ## Why the caller does not pass the action
///
/// Each method builds its own [`ObserverAction`] from the value being written. An earlier sketch had
/// a single `apply(action)`, which would have let a caller record `EvolutionToggled { running: true }`
/// while writing `false` — a record that disagrees with the world is worse than no record, because it
/// is believed. Deriving the action from the write makes that unrepresentable.
///
/// ## Two honest limits
///
/// 1. **The record is a summary, not a replayable payload.** `ShardingConfigChanged` carries
///    `local_port`, not the whole [`ShardingConfig`]. Enough to answer "who changed this and when",
///    not enough to replay the change. Replay of actions is O3 territory and needs the full payload.
/// 2. **Level A, not level B.** The handles below are still reachable elsewhere in the crate: they
///    live on `AppState` because `SimulationEngine::start` takes them as arguments. Making the raw
///    write path unreachable by the compiler means changing that signature, which touches 10 call
///    sites across 7 test files. Until then this closes the *forgetting* failure mode, not the
///    *deliberate bypass* one, and the source scan stays as the backstop for the latter.
#[derive(Clone)]
pub struct ObserverSeam {
    actions: SharedObserverActions,
    evolution_settings: Arc<Mutex<crate::commands::EvolutionSettings>>,
    evolution_running: Arc<std::sync::atomic::AtomicBool>,
    sharding_config: Arc<std::sync::RwLock<crate::core::ecs::ShardingConfig>>,
    migration_trigger: crossbeam_channel::Sender<u16>,
}

impl ObserverSeam {
    pub fn new(
        actions: SharedObserverActions,
        evolution_settings: Arc<Mutex<crate::commands::EvolutionSettings>>,
        evolution_running: Arc<std::sync::atomic::AtomicBool>,
        sharding_config: Arc<std::sync::RwLock<crate::core::ecs::ShardingConfig>>,
        migration_trigger: crossbeam_channel::Sender<u16>,
    ) -> Self {
        Self {
            actions,
            evolution_settings,
            evolution_running,
            sharding_config,
            migration_trigger,
        }
    }

    /// Change the laws selection runs under, mid-run, and say so.
    pub fn set_evolution_settings(
        &self,
        settings: crate::commands::EvolutionSettings,
    ) -> Result<(), String> {
        settings.validate()?;
        // Recorded before the write, always. The reverse order leaves a window in which the world has
        // already changed and nothing says who changed it.
        self.actions.push(ObserverAction::EvolutionSettingsChanged {
            mutation_rate: settings.mutation_rate,
            selection_bias: settings.selection_bias,
            grid_resolution: settings.grid_resolution,
        });
        let mut current = self
            .evolution_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *current = settings;
        Ok(())
    }

    /// Turn selection on or off. Returns the new state, which is what the command reports back.
    pub fn toggle_evolution(&self) -> bool {
        use std::sync::atomic::Ordering;
        let next = !self.evolution_running.load(Ordering::SeqCst);
        self.actions
            .push(ObserverAction::EvolutionToggled { running: next });
        self.evolution_running.store(next, Ordering::SeqCst);
        next
    }

    /// Send agents to another shard.
    ///
    /// The record is pushed before the send and is **not** rolled back if the send fails. That is
    /// deliberate: a human asked for this, and "they asked and it failed" is a different fact from
    /// "they never asked". Losing the request would make a failed migration indistinguishable from
    /// one nobody requested.
    pub fn trigger_migration(&self, target_port: u16) -> Result<(), String> {
        self.actions
            .push(ObserverAction::MigrationTriggered { target_port });
        match self.migration_trigger.try_send(target_port) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(_)) => Err(
                "manual migration request queue is full; retry after the simulation drains it"
                    .to_owned(),
            ),
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                Err("manual migration request queue is disconnected".to_owned())
            }
        }
    }

    /// Repartition the world under a running population.
    pub fn set_sharding_config(
        &self,
        config: crate::core::ecs::ShardingConfig,
    ) -> Result<(), String> {
        self.actions.push(ObserverAction::ShardingConfigChanged {
            local_port: config.local_port,
        });
        let mut current = self.sharding_config.write().map_err(|e| e.to_string())?;
        *current = config;
        Ok(())
    }

    /// Read-only view of the settings, for commands that report rather than change them.
    pub fn evolution_settings(&self) -> crate::commands::EvolutionSettings {
        self.evolution_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Whether selection is currently running.
    pub fn evolution_is_running(&self) -> bool {
        self.evolution_running
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Move queued actions into the trace, stamping each with the tick the world saw it on.
///
/// Inert without both resources, like the rest of this subsystem. Allocation-free on the tick path:
/// `drain` reuses the queue's buffer and the trace's buffer is pre-allocated.
pub fn drain_observer_actions_system(
    queued: Option<bevy_ecs::system::Res<SharedObserverActions>>,
    trace: Option<bevy_ecs::system::ResMut<ObserverTrace>>,
    mut tick: bevy_ecs::system::Local<u64>,
) {
    let (Some(queued), Some(mut trace)) = (queued, trace) else {
        return;
    };
    *tick = tick.wrapping_add(1);
    let Ok(mut queue) = queued.0.lock() else {
        return;
    };
    let dropped = std::mem::take(&mut queue.dropped);
    trace.record_dropped_actions(dropped);
    for mut record in queue.records.drain(..) {
        record.tick = *tick;
        trace.record_action(record);
    }
}

/// Record what the world actually saw of the observer this tick.
///
/// Ordered **after** [`sync_lod_focus_system`](crate::core::simulation_lod::sync_lod_focus_system)
/// so it reads the policed focus. Reading the raw [`SharedLodFocus`] instead would record a camera
/// path the world never experienced, and a `Spectate` run would file evidence of a perturbation that
/// its whole promise is that it did not commit.
///
/// Inert without the resource, like every other part of this subsystem: no trace, no recording, and
/// a run that never installed one behaves exactly as it did before ADR-0004.
pub fn record_observer_trace_system(
    focus: Option<bevy_ecs::system::Res<crate::core::simulation_lod::LodFocus>>,
    trace: Option<bevy_ecs::system::ResMut<ObserverTrace>>,
    mut tick: bevy_ecs::system::Local<u64>,
) {
    let (Some(focus), Some(mut trace)) = (focus, trace) else {
        return;
    };
    *tick = tick.wrapping_add(1);
    trace.record(*tick, *focus);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_action_ingress_is_bounded_and_reports_provenance_gaps() {
        let queue = SharedObserverActions::with_capacity(2);
        queue.push(ObserverAction::EvolutionToggled { running: true });
        queue.push(ObserverAction::EvolutionToggled { running: false });
        queue.push(ObserverAction::MigrationTriggered { target_port: 7001 });

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(queue);
        world.insert_resource(ObserverTrace::with_capacity(8));
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drain_observer_actions_system);
        schedule.run(&mut world);

        let trace = world.resource::<ObserverTrace>();
        assert_eq!(trace.actions().len(), 2);
        assert_eq!(
            trace.dropped_actions(),
            1,
            "a bounded ingress queue must declare every provenance record it could not retain"
        );
    }

    #[test]
    fn a_full_migration_queue_refuses_immediately_and_keeps_the_action_record() {
        let actions = SharedObserverActions::with_capacity(8);
        let (migration_tx, migration_rx) = crossbeam_channel::bounded(1);
        let seam = ObserverSeam::new(
            actions.clone(),
            Arc::new(Mutex::new(crate::commands::EvolutionSettings {
                mutation_rate: 0.15,
                selection_bias: 1.5,
                grid_resolution: 50,
            })),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(std::sync::RwLock::new(
                crate::core::components::ShardingConfig::default(),
            )),
            migration_tx,
        );
        seam.trigger_migration(7001).expect("first queue slot");

        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let attempt = std::thread::spawn(move || {
            let _ = result_tx.send(seam.trigger_migration(7002));
        });
        let result = match result_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(result) => result,
            Err(error) => {
                drop(migration_rx);
                attempt.join().expect("blocked trigger thread");
                panic!("a full control-plane queue blocked the caller: {error}");
            }
        };
        attempt.join().expect("trigger thread");

        let error = result.expect_err("a full migration queue must apply backpressure");
        assert!(error.contains("full"), "{error}");
        assert_eq!(migration_rx.len(), 1);
        assert_eq!(
            actions.0.lock().expect("action queue").records.len(),
            2,
            "the failed request is still part of observer provenance"
        );
    }

    #[test]
    fn the_default_is_absent() {
        assert_eq!(ObserverPolicy::default(), ObserverPolicy::Absent);
    }

    #[test]
    fn only_inhabit_may_tier_the_world() {
        assert!(!ObserverPolicy::Absent.allows_focus());
        assert!(!ObserverPolicy::Spectate.allows_focus());
        assert!(ObserverPolicy::Inhabit { cause_id: 7 }.allows_focus());
    }

    #[test]
    fn spectate_is_comparable_to_headless_and_inhabit_is_not() {
        assert!(ObserverPolicy::Spectate.is_comparable_to_headless());
        assert!(ObserverPolicy::Absent.is_comparable_to_headless());
        assert!(!ObserverPolicy::Inhabit { cause_id: 7 }.is_comparable_to_headless());
    }

    #[test]
    fn an_inhabit_rooted_at_the_background_cause_is_rejected() {
        assert!(ObserverPolicy::Inhabit {
            cause_id: CAUSE_BACKGROUND
        }
        .rejection_reason()
        .is_some());
        assert!(ObserverPolicy::Inhabit { cause_id: 1 }
            .rejection_reason()
            .is_none());
        assert!(ObserverPolicy::Absent.rejection_reason().is_none());
        assert!(ObserverPolicy::Spectate.rejection_reason().is_none());
    }

    #[test]
    fn a_json_object_without_a_mode_is_not_silently_a_policy() {
        // Guards the serde shape: the tag is required, so a half-written policy fails loudly rather
        // than defaulting to something permissive.
        assert!(serde_json::from_str::<ObserverPolicy>("{}").is_err());
    }

    #[test]
    fn the_policy_round_trips_through_serde() {
        for policy in [
            ObserverPolicy::Absent,
            ObserverPolicy::Spectate,
            ObserverPolicy::Inhabit { cause_id: 42 },
        ] {
            let json = serde_json::to_string(&policy).expect("serialize");
            let back: ObserverPolicy = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(policy, back, "round trip failed for {policy:?}");
        }
    }

    #[test]
    fn spectate_serialises_as_a_tagged_mode() {
        assert_eq!(
            serde_json::to_string(&ObserverPolicy::Spectate).expect("serialize"),
            r#"{"mode":"spectate"}"#
        );
    }

    // ---- Observer trace -------------------------------------------------------------------------

    fn at(x: f32) -> LodFocus {
        LodFocus::at(glam::Vec3::new(x, 0.0, 0.0))
    }

    #[test]
    fn the_trace_records_only_when_the_focus_changes() {
        let mut trace = ObserverTrace::with_capacity(16);
        assert!(
            trace.record(1, at(0.0)),
            "the first sample sets the baseline"
        );
        assert!(
            !trace.record(2, at(0.0)),
            "an unchanged focus is not an event"
        );
        assert!(!trace.record(3, at(0.0)));
        assert!(trace.record(4, at(5.0)));
        assert_eq!(trace.len(), 2);
        assert_eq!(
            trace.samples().iter().map(|s| s.tick).collect::<Vec<_>>(),
            vec![1, 4],
            "the recorded ticks must be the ones the focus actually moved on"
        );
    }

    /// A `Spectate` run feeds this system a focus that is always disabled, so after the opening
    /// baseline there is nothing to record — the world was subjected to nothing.
    #[test]
    fn a_focus_that_never_changes_costs_one_sample() {
        let mut trace = ObserverTrace::with_capacity(16);
        for tick in 1..=100 {
            trace.record(tick, LodFocus::default());
        }
        assert_eq!(trace.len(), 1);
        assert!(!trace.is_truncated());
    }

    /// Full means **declared** full. A trace that quietly stopped recording reads exactly like a
    /// camera that stopped moving, and those two must never be indistinguishable.
    #[test]
    fn a_full_trace_counts_what_it_could_not_keep() {
        let mut trace = ObserverTrace::with_capacity(3);
        for tick in 0..10u64 {
            trace.record(tick, at(tick as f32));
        }
        assert_eq!(trace.len(), 3);
        assert_eq!(trace.dropped(), 7);
        assert!(
            trace.is_truncated(),
            "a truncated trace must say so, or a later replay would trust a partial record"
        );
    }

    /// The default buffer is allocated once per run and held for its life, so its cost belongs in a
    /// test rather than in a comment someone has to trust. ADR-0004 asks for a measured trace size;
    /// this is the ceiling that measurement has to stay under.
    #[test]
    fn a_full_hour_of_trace_fits_a_declared_budget() {
        const BUDGET_BYTES: usize = 8 * 1024 * 1024;
        let bytes = DEFAULT_OBSERVER_TRACE_CAPACITY * std::mem::size_of::<ObserverSample>();
        assert!(
            bytes <= BUDGET_BYTES,
            "an hour of observer trace now costs {bytes} bytes, over the {BUDGET_BYTES} budget — \
             either shrink ObserverSample or lower the capacity on purpose"
        );
        // Not a tautology: catches a sample that has quietly grown, e.g. by gaining a String.
        assert!(
            std::mem::size_of::<ObserverSample>() <= 32,
            "ObserverSample grew to {} bytes",
            std::mem::size_of::<ObserverSample>()
        );
    }

    #[test]
    fn a_fresh_trace_is_neither_truncated_nor_populated() {
        let trace = ObserverTrace::with_capacity(4);
        assert!(trace.is_empty());
        assert!(!trace.is_truncated());
        assert_eq!(trace.dropped(), 0);
    }

    /// The observer's cause id must not be reachable by a scenario author counting up from 1.
    #[test]
    fn the_observer_cause_is_reserved_and_far_from_hand_assigned_ids() {
        use anima_domain::causal::{is_reserved_cause, CAUSE_OBSERVER};
        assert!(is_reserved_cause(CAUSE_OBSERVER));
        assert!(is_reserved_cause(CAUSE_BACKGROUND));
        for hand_written in [1u32, 2, 3, 10, 100, 65_535] {
            assert!(
                !is_reserved_cause(hand_written),
                "{hand_written} is the sort of id a manifest author writes and must stay free"
            );
        }
    }
}
