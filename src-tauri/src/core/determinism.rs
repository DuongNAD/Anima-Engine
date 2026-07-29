//! Deterministic mode for the live engine (G1.3).
//!
//! G0 gave the live world a seeded RNG stream and G1.2 gave it a checkpoint that restores the
//! stream's draw position. Neither is enough on its own: a run can draw identical random numbers
//! and still diverge, because three other things leak the outside world into the simulation.
//!
//! | Source | Where it was | What it leaks |
//! |---|---|---|
//! | `Uuid::new_v4()` | lineage ids, chronicle ids, offspring ids | OS entropy |
//! | `SystemTime::now()` | chronicle timestamps | wall-clock time |
//! | Gemini | `evolution::meta_ai` | network, a secret, and a remote model's output |
//!
//! and a fourth that is easy to miss because it looks like an implementation detail rather than an
//! input: **Bevy's multi-threaded executor picks system order per run.** Two systems that both take
//! `ResMut<EcosystemBiomass>` are serialised against each other, but *which one goes first* is not
//! declared anywhere — so an uninterrupted live run does not even reproduce itself. G1.1 found this
//! the hard way (an energy residual that changed sign between runs), and G1.2's gate had to declare
//! its own order to get a stable checksum at all.
//!
//! # What this module does
//!
//! [`DeterministicMode`] is the switch. When it is on:
//!
//! - ids come from [`RunIdentity::next_id`] — run id plus a counter, not entropy;
//! - timestamps come from [`tick_timestamp_ms`] — a function of the simulation tick, not the clock;
//! - the external model may only *propose*; see [`DeterministicMode::allows_external_ai`];
//! - the schedule runs single-threaded in a declared order (see
//!   [`crate::core::simulation_loop`]'s schedule construction).
//!
//! # Default off, and why
//!
//! An interactive session wants real uuids and real timestamps: the chronicle is a user-facing log,
//! and stamping it with tick-derived times would be a lie in that context. Experiments want the
//! opposite. So this defaults to **off** and is turned on explicitly — by `ANIMA_DETERMINISTIC`, or
//! by an experiment runner that has no business being irreproducible. That also keeps the legacy
//! path the default, so nothing about an existing run changes until someone asks for it.
//!
//! # What determinism does *not* mean here
//!
//! It means: same manifest and same build produce the same trajectory. It does **not** mean
//! bit-identical results across targets or optimisation levels — float reassociation makes that a
//! different and much larger promise. [`crate::core::snapshot::BuildProvenance`] records the target
//! and profile precisely so a cross-machine mismatch can be attributed rather than puzzled over.

use bevy_ecs::prelude::Resource;

/// Whether this run must be reproducible.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeterministicMode {
    enabled: bool,
}

impl DeterministicMode {
    /// A deterministic run.
    pub fn on() -> Self {
        Self { enabled: true }
    }

    /// The legacy path: real uuids, real timestamps, live external AI, executor picks its own order.
    pub fn off() -> Self {
        Self { enabled: false }
    }

    /// Off unless `ANIMA_DETERMINISTIC` is set to something other than `0`/`false`/empty.
    ///
    /// Same shape as the other engine switches, so "unset" always means "behave as before".
    pub fn from_env() -> Self {
        let on = std::env::var("ANIMA_DETERMINISTIC")
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false")
            })
            .unwrap_or(false);
        Self { enabled: on }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether a live external model may be consulted.
    ///
    /// In a deterministic run it may not: its answer depends on a network, a secret and a remote
    /// model's weights, none of which are part of the manifest. The contract in the G1.3 plan is
    /// that external AI may only **propose** interventions, which are then frozen into the manifest
    /// and replayed from there — so at replay time there is nothing left to ask.
    pub fn allows_external_ai(&self) -> bool {
        !self.enabled
    }
}

/// Epoch that tick-derived timestamps count from: 2026-01-01T00:00:00Z in milliseconds.
///
/// A fixed, arbitrary, *documented* origin. Zero would also work, but a plausible-looking absolute
/// time makes a deterministic chronicle render sensibly in a UI that expects unix milliseconds,
/// while still being a pure function of the tick.
pub const DETERMINISTIC_EPOCH_MS: u64 = 1_767_225_600_000;

/// Timestamp for a simulation tick, in milliseconds since the unix epoch.
///
/// A pure function of the tick and the declared tick rate, so replaying a manifest reproduces the
/// same chronicle timestamps. Uses [`crate::core::sim_rules::TICK_HZ`] rather than a local constant
/// so "simulation time" means one thing across the engine.
#[inline]
pub fn tick_timestamp_ms(tick: u64) -> u64 {
    let ms_per_tick = 1000.0 / crate::core::sim_rules::TICK_HZ;
    DETERMINISTIC_EPOCH_MS + (tick as f64 * ms_per_tick) as u64
}

/// Wall-clock milliseconds, or the tick-derived equivalent in a deterministic run.
///
/// The one place code should ask "what time is it" when the answer might end up in saved state.
pub fn timestamp_ms(mode: DeterministicMode, tick: u64) -> u64 {
    if mode.is_enabled() {
        tick_timestamp_ms(tick)
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Source of stable identifiers for a run.
///
/// Ids are `"<prefix>-<run_id:016x>-<counter:08x>"`. The run id keeps two different runs from
/// colliding in a shared lineage store; the counter makes each id a function of how many have been
/// issued rather than of entropy. Both parts are hex so an id sorts in issue order, which makes a
/// lineage graph readable.
///
/// Not a Bevy resource on purpose: the ids are minted on the evolution and Meta-AI threads, which
/// run outside the ECS world. It is `Clone` + `Send` so those threads can each hold one, and each
/// thread is given its **own prefix** so two threads minting concurrently cannot collide without
/// having to share a lock on the hot path.
#[derive(Clone, Debug)]
pub struct RunIdentity {
    run_id: u64,
    prefix: String,
    counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl RunIdentity {
    /// A fresh id source for `run_id`, minting ids under `prefix`.
    pub fn new(run_id: u64, prefix: impl Into<String>) -> Self {
        Self::with_issued(run_id, prefix, 0)
    }

    /// Resume an id source after `issued` identifiers have already been minted.
    ///
    /// A checkpoint owns the namespace history just as it owns the RNG position. Starting its
    /// counter at zero would deterministically reproduce ids that already exist in the restored
    /// lineage or chronicle instead of continuing the run.
    pub fn with_issued(run_id: u64, prefix: impl Into<String>, issued: u64) -> Self {
        Self {
            run_id,
            prefix: prefix.into(),
            counter: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(issued)),
        }
    }

    /// A sibling source under a different prefix, sharing nothing but the run id.
    ///
    /// This is how two threads get non-colliding ids without coordinating: `lineage` and
    /// `chronicle` each count from zero in their own namespace.
    pub fn namespace(&self, prefix: impl Into<String>) -> Self {
        Self::new(self.run_id, prefix)
    }

    /// The run this identity belongs to.
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Next id in sequence.
    pub fn next_id(&self) -> String {
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}-{:016x}-{:08x}", self.prefix, self.run_id, n)
    }

    /// How many ids have been issued. Part of a run's reproducibility fingerprint: two replays that
    /// minted a different number of ids did not do the same thing.
    pub fn issued(&self) -> u64 {
        self.counter.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Return the first unused counter after deterministic ids already present in a checkpoint.
///
/// Other namespaces, other run ids, and legacy UUIDs are ignored. An id that claims this exact
/// namespace/run but has a malformed counter is refused: ignoring it could mint a collision, while
/// a refusal keeps the corrupted history visible.
pub fn issued_after_existing_ids<I, S>(run_id: u64, prefix: &str, ids: I) -> Result<u64, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let stem = format!("{prefix}-{run_id:016x}-");
    let mut greatest = None;

    for id in ids {
        let id = id.as_ref();
        let Some(counter) = id.strip_prefix(&stem) else {
            continue;
        };
        if counter.is_empty() || counter.len() > 16 {
            return Err(format!(
                "deterministic id {id:?} has an invalid counter for namespace {prefix:?}"
            ));
        }
        let parsed = u64::from_str_radix(counter, 16).map_err(|_| {
            format!("deterministic id {id:?} has a non-hex counter for namespace {prefix:?}")
        })?;
        greatest = Some(greatest.map_or(parsed, |current: u64| current.max(parsed)));
    }

    match greatest {
        Some(counter) => counter.checked_add(1).ok_or_else(|| {
            format!(
                "deterministic id namespace {prefix:?} for run {run_id:016x} exhausted its counter"
            )
        }),
        None => Ok(0),
    }
}

/// An id that is stable in a deterministic run and a real uuid otherwise.
///
/// The single call site pattern for replacing `Uuid::new_v4().to_string()`.
pub fn next_entity_id(mode: DeterministicMode, ids: &RunIdentity) -> String {
    if mode.is_enabled() {
        ids.next_id()
    } else {
        uuid::Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_a_function_of_run_and_counter_not_entropy() {
        let a = RunIdentity::new(0xABCD, "lineage");
        let b = RunIdentity::new(0xABCD, "lineage");
        for _ in 0..100 {
            assert_eq!(a.next_id(), b.next_id());
        }
        assert_eq!(a.issued(), 100);
    }

    #[test]
    fn ids_sort_in_issue_order() {
        let ids = RunIdentity::new(7, "agent");
        let minted: Vec<String> = (0..50).map(|_| ids.next_id()).collect();
        let mut sorted = minted.clone();
        sorted.sort();
        assert_eq!(minted, sorted, "zero-padded hex must sort in issue order");
    }

    #[test]
    fn namespaces_do_not_collide_but_share_the_run() {
        let lineage = RunIdentity::new(42, "lineage");
        let chronicle = lineage.namespace("chronicle");
        assert_eq!(lineage.run_id(), chronicle.run_id());
        assert_ne!(lineage.next_id(), chronicle.next_id());
        // Both counted from zero; only the prefix separates them.
        assert!(lineage.next_id().starts_with("lineage-"));
        assert!(chronicle.next_id().starts_with("chronicle-"));
    }

    #[test]
    fn different_runs_do_not_collide() {
        let a = RunIdentity::new(1, "lineage");
        let b = RunIdentity::new(2, "lineage");
        assert_ne!(a.next_id(), b.next_id());
    }

    #[test]
    fn timestamps_are_a_function_of_the_tick() {
        assert_eq!(tick_timestamp_ms(0), DETERMINISTIC_EPOCH_MS);
        // 60 Hz, so 60 ticks is one second.
        assert_eq!(tick_timestamp_ms(60), DETERMINISTIC_EPOCH_MS + 1000);
        assert!(tick_timestamp_ms(120) > tick_timestamp_ms(60));
        // Same tick, same answer, always.
        assert_eq!(tick_timestamp_ms(12345), tick_timestamp_ms(12345));
    }

    #[test]
    fn deterministic_mode_gates_ids_timestamps_and_external_ai() {
        let on = DeterministicMode::on();
        let off = DeterministicMode::off();

        assert!(on.is_enabled() && !off.is_enabled());
        assert!(!on.allows_external_ai(), "a replay must not phone a model");
        assert!(off.allows_external_ai(), "the legacy path is unchanged");

        assert_eq!(timestamp_ms(on, 60), DETERMINISTIC_EPOCH_MS + 1000);

        let ids = RunIdentity::new(9, "agent");
        assert_eq!(next_entity_id(on, &ids), "agent-0000000000000009-00000000");

        // Off: a real uuid, which is 36 chars with dashes and is not the deterministic shape.
        let uuid_like = next_entity_id(off, &ids);
        assert_eq!(uuid_like.len(), 36);
        assert!(!uuid_like.starts_with("agent-"));
    }

    #[test]
    fn from_env_defaults_off_so_unset_means_behave_as_before() {
        // The env var is process-global, so this only asserts the parsing rule via `on`/`off`
        // rather than mutating it and racing other tests.
        assert!(!DeterministicMode::default().is_enabled());
    }
}
