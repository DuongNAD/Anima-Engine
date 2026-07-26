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
use anima_domain::causal::{CauseId, CAUSE_BACKGROUND};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

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
    dropped: u64,
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
            dropped: 0,
            last: None,
        }
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
