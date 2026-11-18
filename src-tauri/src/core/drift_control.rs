//! Lockstep neutral-drift control for live-world selection experiments (card SEL2-D).
//!
//! A live run with reproduction and mortality active records exact per-tick trophic births and
//! deaths into a [`TurnoverSchedule`]. A paired drift run replays from identical founders (or from a
//! periodic Channon checkpoint when `live.drift_reset_ticks > 0`) under the same per-tick event
//! counts while substituting:
//! - uniformly random victims without replacement in [`crate::core::mortality::mortality_system`],
//!   still reclaimed through [`crate::core::agent_systems::ReclaimAndDespawnAgentCommand`];
//! - uniformly random genetic parents across **all** living agents of the guild (decoupled from
//!   energy reserves to prevent a fecundity-selection leak under disabled starvation death), funded
//!   by a living guild member holding at least [`MIN_DRIFT_BIRTH_PAYMENT_EU`].
//!
//! All victim/parent random draws come exclusively from `sim_stream::LIVE_DRIFT_SELECTION`, and all
//! per-tick scratch buffers are pre-allocated once at world setup so warm death and zero-birth ticks
//! perform zero heap allocations.

use bevy_ecs::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::core::resources::{derived_sim_rng, sim_stream, SimRng};

/// Minimum energy reserve (EU) required for a living guild agent to act as energetic birth payer
/// (`P_pay`) during neutral-drift replay.
///
/// Genetic parent sampling (`P_gen`) still spans every living agent of the guild regardless of
/// reserve; this threshold only ensures closed-EU parental debit produces a positive newborn
/// endowment.
pub const MIN_DRIFT_BIRTH_PAYMENT_EU: f32 = 2.0;

/// Pre-allocated entity scratch buffer capacity per trophic guild inside [`DriftControlState`].
pub const DRIFT_SCRATCH_CAPACITY: usize = 4096;

/// Recorded per-tick births and deaths, partitioned by trophic guild.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickTurnover {
    pub prey_deaths: u16,
    pub pred_deaths: u16,
    pub prey_births: u16,
    pub pred_births: u16,
}

impl TickTurnover {
    #[inline]
    pub fn deaths(self, is_predator: bool) -> usize {
        if is_predator {
            self.pred_deaths as usize
        } else {
            self.prey_deaths as usize
        }
    }

    #[inline]
    pub fn births(self, is_predator: bool) -> usize {
        if is_predator {
            self.pred_births as usize
        } else {
            self.prey_births as usize
        }
    }
}

/// Complete lockstep turnover schedule captured from a `selected` live run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnoverSchedule {
    pub seed: u64,
    pub source_manifest_fingerprint: u64,
    pub duration_ticks: u64,
    /// Indexed by 0-based tick offset (`clock.tick - 1`), length equal to `duration_ticks`.
    pub ticks: Vec<TickTurnover>,
}

impl TurnoverSchedule {
    /// Cumulative `(prey_deaths, pred_deaths, prey_births, pred_births)` across all recorded ticks.
    pub fn total_counts(&self) -> (u64, u64, u64, u64) {
        self.ticks.iter().fold((0, 0, 0, 0), |(pd, rd, pb, rb), t| {
            (
                pd + t.prey_deaths as u64,
                rd + t.pred_deaths as u64,
                pb + t.prey_births as u64,
                rb + t.pred_births as u64,
            )
        })
    }

    /// Slice a sub-schedule for a Channon periodic-reset window `[start, end)`.
    pub fn slice_window(&self, start_zero_based: usize, end_exclusive: usize) -> Self {
        let bounded_end = end_exclusive.min(self.ticks.len());
        let bounded_start = start_zero_based.min(bounded_end);
        let ticks = self.ticks[bounded_start..bounded_end].to_vec();
        Self {
            seed: self.seed,
            source_manifest_fingerprint: self.source_manifest_fingerprint,
            duration_ticks: ticks.len() as u64,
            ticks,
        }
    }
}

/// Execution mode of [`DriftControlState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriftMode {
    /// Read-only observer mode for the `selected` arm: existing death and birth rules run
    /// untouched while incrementing per-tick trophic turnover counters.
    Record,
    /// Lockstep neutral-drift replay mode: skips physiological deaths and threshold/cap fecundity
    /// gates, executing the recorded per-tick counts with uniform trait-blind selection.
    Replay,
}

/// Summary of how faithfully the drift replay tracked the recorded [`TurnoverSchedule`].
///
/// Shortfalls are never hidden and EU is never fabricated when the drift population shrinks below
/// scheduled counts or lacks solvent payers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftDiagnostics {
    pub scheduled_deaths: u64,
    pub actual_deaths: u64,
    pub missed_deaths: u64,
    pub scheduled_births: u64,
    pub actual_births: u64,
    pub missed_births_extinct: u64,
    pub missed_births_insolvent: u64,
    pub ticks_with_missed_deaths: u64,
    pub ticks_with_missed_births: u64,
    pub first_divergence_tick: Option<u64>,
}

impl DriftDiagnostics {
    #[inline]
    pub fn missed_births(&self) -> u64 {
        self.missed_births_extinct + self.missed_births_insolvent
    }

    #[inline]
    pub fn has_shortfall(&self) -> bool {
        self.missed_deaths > 0 || self.missed_births() > 0
    }

    #[inline]
    pub fn note_divergence(&mut self, world_tick: u64) {
        if world_tick > 0 && self.first_divergence_tick.is_none() {
            self.first_divergence_tick = Some(world_tick);
        }
    }

    /// Accumulate a Channon reset window's diagnostics into an ensemble/pair total.
    pub fn merge_window(&mut self, other: &DriftDiagnostics) {
        self.scheduled_deaths += other.scheduled_deaths;
        self.actual_deaths += other.actual_deaths;
        self.missed_deaths += other.missed_deaths;
        self.scheduled_births += other.scheduled_births;
        self.actual_births += other.actual_births;
        self.missed_births_extinct += other.missed_births_extinct;
        self.missed_births_insolvent += other.missed_births_insolvent;
        self.ticks_with_missed_deaths += other.ticks_with_missed_deaths;
        self.ticks_with_missed_births += other.ticks_with_missed_births;
        self.first_divergence_tick = match (self.first_divergence_tick, other.first_divergence_tick)
        {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
}

/// ECS resource inserted only into live experiment runs when recording a selected arm or running a
/// neutral-drift control arm (`live.drift_control = 1.0`). Absent in all other worlds.
#[derive(Resource)]
pub struct DriftControlState {
    mode: DriftMode,
    /// 0-based index into `schedule` for the tick currently executing.
    pub tick_cursor: usize,
    /// World tick offset of the start of this schedule window (`0` for unreset runs, `w * K` for
    /// Channon window `w`).
    pub tick_offset: u64,
    /// 1-based `clock.tick` currently executing, for `first_divergence_tick` reporting.
    pub current_world_tick: u64,
    schedule: Vec<TickTurnover>,
    rng: SimRng,
    pub diagnostics: DriftDiagnostics,
    pub(crate) reported_missed_deaths: u64,
    pub(crate) reported_missed_births: u64,
    /// Pre-allocated scratch buffer for living prey candidates (cleared and reused each tick).
    pub(crate) prey_buf: Vec<Entity>,
    /// Pre-allocated scratch buffer for living predator candidates (cleared and reused each tick).
    pub(crate) pred_buf: Vec<Entity>,
    /// Pre-allocated scratch buffer for solvent energetic payers in a guild.
    pub(crate) solvent_buf: Vec<Entity>,
}

impl std::fmt::Debug for DriftControlState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriftControlState")
            .field("mode", &self.mode)
            .field("tick_cursor", &self.tick_cursor)
            .field("tick_offset", &self.tick_offset)
            .field("current_world_tick", &self.current_world_tick)
            .field("schedule_len", &self.schedule.len())
            .field("rng_seed", &self.rng.seed())
            .field("rng_stream_pos", &self.rng.stream_pos())
            .field("diagnostics", &self.diagnostics)
            .finish_non_exhaustive()
    }
}

impl DriftControlState {
    /// Construct a read-only recorder sized upfront for `duration_ticks`.
    pub fn new_recorder(seed: u64, duration_ticks: u64) -> Self {
        Self {
            mode: DriftMode::Record,
            tick_cursor: 0,
            tick_offset: 0,
            current_world_tick: 0,
            schedule: vec![TickTurnover::default(); duration_ticks as usize],
            rng: derived_sim_rng(seed, sim_stream::LIVE_DRIFT_SELECTION),
            diagnostics: DriftDiagnostics::default(),
            reported_missed_deaths: 0,
            reported_missed_births: 0,
            prey_buf: Vec::new(),
            pred_buf: Vec::new(),
            solvent_buf: Vec::new(),
        }
    }

    /// Construct a replay state seeded on `sim_stream::LIVE_DRIFT_SELECTION` with pre-allocated
    /// scratch buffers so warm ticks perform no buffer growth.
    pub fn new_replay(seed: u64, ticks: Vec<TickTurnover>) -> Self {
        Self::new_replay_with_rng(
            derived_sim_rng(seed, sim_stream::LIVE_DRIFT_SELECTION),
            ticks,
            0,
        )
    }

    /// Construct a replay state continuing an existing `LIVE_DRIFT_SELECTION` stream across a
    /// Channon reset boundary starting at `tick_offset`.
    pub fn new_replay_with_rng(rng: SimRng, ticks: Vec<TickTurnover>, tick_offset: u64) -> Self {
        Self {
            mode: DriftMode::Replay,
            tick_cursor: 0,
            tick_offset,
            current_world_tick: tick_offset,
            schedule: ticks,
            rng,
            diagnostics: DriftDiagnostics::default(),
            reported_missed_deaths: 0,
            reported_missed_births: 0,
            prey_buf: Vec::with_capacity(DRIFT_SCRATCH_CAPACITY),
            pred_buf: Vec::with_capacity(DRIFT_SCRATCH_CAPACITY),
            solvent_buf: Vec::with_capacity(DRIFT_SCRATCH_CAPACITY),
        }
    }

    /// Synchronize the tick cursor before stepping world tick `world_tick` (1-based).
    #[inline]
    pub fn sync_tick(&mut self, world_tick: u64) {
        self.current_world_tick = world_tick;
        self.tick_cursor = world_tick.saturating_sub(self.tick_offset.saturating_add(1)) as usize;
    }

    #[inline]
    pub fn mode(&self) -> DriftMode {
        self.mode
    }

    #[inline]
    pub fn is_replay(&self) -> bool {
        self.mode == DriftMode::Replay
    }

    #[inline]
    pub fn rng_mut(&mut self) -> &mut SimRng {
        &mut self.rng
    }

    /// Checkpoint clone of the isolated drift RNG stream (`seed` + `word_pos`).
    pub fn rng_clone(&self) -> SimRng {
        SimRng::restore(self.rng.seed(), self.rng.stream_pos())
    }

    pub fn rng_stream_pos(&self) -> u128 {
        self.rng.stream_pos()
    }

    pub fn scratch_capacities(&self) -> (usize, usize, usize) {
        (
            self.prey_buf.capacity(),
            self.pred_buf.capacity(),
            self.solvent_buf.capacity(),
        )
    }

    #[inline]
    pub fn current_turnover(&self) -> TickTurnover {
        self.schedule
            .get(self.tick_cursor)
            .copied()
            .unwrap_or_default()
    }

    #[inline]
    pub fn record_death(&mut self, is_predator: bool) {
        if let Some(slot) = self.schedule.get_mut(self.tick_cursor) {
            if is_predator {
                slot.pred_deaths = slot.pred_deaths.saturating_add(1);
            } else {
                slot.prey_deaths = slot.prey_deaths.saturating_add(1);
            }
        }
    }

    #[inline]
    pub fn record_birth(&mut self, is_predator: bool) {
        if let Some(slot) = self.schedule.get_mut(self.tick_cursor) {
            if is_predator {
                slot.pred_births = slot.pred_births.saturating_add(1);
            } else {
                slot.prey_births = slot.prey_births.saturating_add(1);
            }
        }
    }

    pub fn export_schedule(&self, seed: u64, source_manifest_fingerprint: u64) -> TurnoverSchedule {
        TurnoverSchedule {
            seed,
            source_manifest_fingerprint,
            duration_ticks: self.schedule.len() as u64,
            ticks: self.schedule.clone(),
        }
    }

    pub(crate) fn split_death_buffers_and_rng(
        &mut self,
    ) -> (&mut Vec<Entity>, &mut Vec<Entity>, &mut SimRng) {
        (&mut self.prey_buf, &mut self.pred_buf, &mut self.rng)
    }

    pub(crate) fn split_birth_buffers_and_rng(
        &mut self,
    ) -> (
        &mut Vec<Entity>,
        &mut Vec<Entity>,
        &mut Vec<Entity>,
        &mut SimRng,
    ) {
        (
            &mut self.prey_buf,
            &mut self.pred_buf,
            &mut self.solvent_buf,
            &mut self.rng,
        )
    }
}

/// Deterministic, zero-allocation comparator for sorting living candidates before RNG sampling.
///
/// Same-generation siblings routinely share identical `AgentLineageId` strings
/// (`"live-founder-0000.1"`), so content-based tie-breakers (coordinates, energy, entity index)
/// are included to ensure save/restore stability without cloning strings.
#[inline]
pub fn candidate_cmp(
    a: (&str, u32, f32, f32, f32, u32),
    b: (&str, u32, f32, f32, f32, u32),
) -> std::cmp::Ordering {
    (a.0, a.1, a.2.to_bits(), a.3.to_bits(), a.4.to_bits(), a.5).cmp(&(
        b.0,
        b.1,
        b.2.to_bits(),
        b.3.to_bits(),
        b.4.to_bits(),
        b.5,
    ))
}

/// Sample `min(requested, candidates.len())` distinct victims uniformly without replacement via
/// partial Fisher–Yates shuffle using `rng`.
pub fn draw_victims_without_replacement<'a>(
    candidates: &'a mut [Entity],
    requested: usize,
    rng: &mut SimRng,
) -> &'a [Entity] {
    let k = requested.min(candidates.len());
    if k == 0 {
        return &candidates[..0];
    }
    let len = candidates.len();
    let r = rng.rng();
    for i in 0..k {
        let j = r.gen_range(i..len);
        candidates.swap(i, j);
    }
    &candidates[..k]
}

/// Sample one candidate uniformly from `candidates` using `rng`.
#[inline]
pub fn draw_uniform_entity(candidates: &[Entity], rng: &mut SimRng) -> Option<Entity> {
    if candidates.is_empty() {
        None
    } else {
        let idx = rng.rng().gen_range(0..candidates.len());
        Some(candidates[idx])
    }
}
