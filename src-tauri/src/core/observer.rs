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
}
