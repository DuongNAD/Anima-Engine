//! ADR-0004 O1 at the manifest level: the rollback path, run identity, and the ER01 boundary.
//! The behaviour of the policy in a live world is `tests/observer_policy_tests.rs`.
use super::tests::base_manifest;
use super::*;
use crate::core::intervention::InterventionKind;
use anima_domain::causal::CauseId;

/// The rollback path. A manifest written before ADR-0004 has no `observer` key, and must load
/// and validate exactly as it did — which is why `MANIFEST_SCHEMA_VERSION` was deliberately not
/// bumped for an additive field.
#[test]
fn a_manifest_without_an_observer_key_reads_as_absent() {
    let m = base_manifest();
    let mut json: serde_json::Value = serde_json::to_value(&m).expect("manifest should serialize");
    json.as_object_mut()
        .expect("manifest is an object")
        .remove("observer")
        .expect("the key should have been there to remove");

    let restored: ExperimentManifest =
        serde_json::from_value(json).expect("a pre-ADR-0004 manifest must still load");
    assert_eq!(restored.observer, ObserverPolicy::Absent);
    assert!(restored
        .validate(&ObservableRegistry::reference_default())
        .is_ok());
}

/// The policy is a declared input, so it is part of the run's identity — including `Absent` vs
/// `Spectate`, which produce the *same trajectory* but are different declarations about how the
/// run was watched.
#[test]
fn the_observer_policy_changes_the_run_identity() {
    let absent = base_manifest();
    let spectate = ExperimentManifest {
        observer: ObserverPolicy::Spectate,
        ..absent.clone()
    };
    let inhabit = ExperimentManifest {
        observer: ObserverPolicy::Inhabit { cause_id: 9 },
        ..absent.clone()
    };

    let (a, s, i) = (
        absent.fingerprint(),
        spectate.fingerprint(),
        inhabit.fingerprint(),
    );
    assert_ne!(a, s, "Absent and Spectate must not share a run identity");
    assert_ne!(a, i, "Absent and Inhabit must not share a run identity");
    assert_ne!(s, i, "Spectate and Inhabit must not share a run identity");
}

#[test]
fn an_observer_change_is_an_experimental_factor() {
    let control = base_manifest();
    let treatment = ExperimentManifest {
        observer: ObserverPolicy::Inhabit { cause_id: 9 },
        ..control.clone()
    };

    assert_eq!(
        FactorDiff::diff_paths(&control, &treatment),
        vec!["observer".to_string()]
    );
    assert_eq!(
        FactorDiff::genesis_exotic().validate(&control, &treatment),
        Err(ExperimentError::UndeclaredFactorDifference {
            path: "observer".to_string()
        })
    );
}

/// **ER01.** The observer is state, not law. If it reached the world-law fingerprint, a
/// checkpoint could not be forked to drop the observer — which is the whole point of recording
/// one. This is the nearest trap in ADR-0004 and it is cheap to guard.
#[test]
fn the_observer_policy_leaves_the_law_fingerprint_alone() {
    let absent = base_manifest();
    let inhabit = ExperimentManifest {
        observer: ObserverPolicy::Inhabit { cause_id: 9 },
        ..absent.clone()
    };
    assert_eq!(
        absent.laws.fingerprint(),
        inhabit.laws.fingerprint(),
        "the observer policy moved the world-law fingerprint; a checkpoint branch can no \
             longer drop the observer without changing the laws"
    );
}

/// An `Inhabit` that roots at the background cause would file a human's doing as baseline
/// dynamics, so `trace_to_root` would report the observer's own effects as something the world
/// did by itself. Refused at validation rather than discovered in a ledger.
#[test]
fn an_inhabit_rooted_at_the_background_cause_is_refused() {
    let m = ExperimentManifest {
        observer: ObserverPolicy::Inhabit {
            cause_id: anima_domain::causal::CAUSE_BACKGROUND,
        },
        ..base_manifest()
    };
    assert!(matches!(
        m.validate(&ObservableRegistry::reference_default()),
        Err(ExperimentError::InvalidObserverPolicy { .. })
    ));

    let ok = ExperimentManifest {
        observer: ObserverPolicy::Inhabit { cause_id: 1 },
        ..base_manifest()
    };
    assert!(ok
        .validate(&ObservableRegistry::reference_default())
        .is_ok());
}

/// A manifest cannot author an intervention as if a live human had caused it. The run had not
/// started when the manifest was written, so the claim is false by construction — and if it were
/// allowed through, a scenario forcing and the observer's own doing would share a root and
/// `root_cause` would stop distinguishing them.
/// A run simulates ticks `1..=duration_ticks`, and `validate_intervention` refuses a window that
/// can never fire inside it. `start_tick: 1` keeps these fixtures about cause ids rather than
/// about scheduling.
fn intervention_with_cause(cause_id: CauseId) -> InterventionCommand {
    InterventionCommand {
        id: 1,
        cause_id,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 1,
        intensity: 0.1,
        signed_negative: true,
        curve: Curve::Step,
        reversible: true,
    }
}

#[test]
fn a_declared_intervention_may_not_claim_the_observer_cause() {
    let m = ExperimentManifest {
        interventions: vec![intervention_with_cause(
            anima_domain::causal::CAUSE_OBSERVER,
        )],
        ..base_manifest()
    };
    assert!(
        matches!(
            m.validate(&ObservableRegistry::reference_default()),
            Err(ExperimentError::InvalidObserverPolicy { .. })
        ),
        "a declared intervention claiming CAUSE_OBSERVER must be refused, got {:?}",
        m.validate(&ObservableRegistry::reference_default())
    );
}

/// The rule above must not have swept up the ids a manifest author actually writes. Without this
/// the check could reject everything and still look correct.
#[test]
fn ordinary_hand_written_cause_ids_are_still_accepted() {
    for cause_id in [1u32, 2, 7, 4096] {
        let m = ExperimentManifest {
            interventions: vec![intervention_with_cause(cause_id)],
            ..base_manifest()
        };
        assert!(
            m.validate(&ObservableRegistry::reference_default()).is_ok(),
            "cause id {cause_id} should be free for a manifest author to use, got {:?}",
            m.validate(&ObservableRegistry::reference_default())
        );
    }
}
