//! The E2 preregistration, checked by machine rather than trusted as prose.
//!
//! A preregistration is only worth the paper it is on if nobody can quietly edit it between the
//! registration and the run. These tests pin the committed manifests of the E2 experiment — "does a
//! per-agent evolved brain change outcomes relative to the shared `BrainModel`, and in which
//! direction?" — so that the declared factor, the seed set, the duration, the sampling plan and the
//! metric list are fixed in git history *before* any experimental run exists.
//!
//! # What this file deliberately does NOT do
//!
//! It never constructs a [`LiveExperimentAdapter`], never steps a tick and never touches the
//! experiment runner. Parsing and validating a manifest is a schema check; running one is the
//! experiment. Keeping those apart is the whole reason the preregistration is a separate commit from
//! the run — see `docs/ai/planning/2026-07-27-experiment-e2-evolved-brain-default.md`.
//!
//! # The seam tripwire
//!
//! [`P1_SEAM_OPEN`] records, as a machine-checked boolean rather than a sentence in a document,
//! whether the live adapter can yet honour `live.evolved_brains`. It is `false` today: the treatment
//! arm does not exist, which is exactly why E2-A registers the experiment and E2-B runs it. When
//! E2-B opens the seam, flipping this constant in the same commit is what keeps the record honest.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
    MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::live_experiment::{LIVE_KEYS, LIVE_OBSERVABLE_IDS};

/// Where the preregistered manifests live, relative to the crate root.
const PREREG_DIR: &str = "tests/fixtures/experiments_e2";

/// The initial-condition key the treatment declares to ask for per-agent evolved brains.
const EVOLVED_KEY: &str = "live.evolved_brains";

/// The seed reserved for calibrating the harness. It is **not** one of the experimental seeds and
/// never enters the analysis.
const SMOKE_SEED: u64 = 999983;

/// Whether `core::live_experiment` can yet honour [`EVOLVED_KEY`].
///
/// `false` at registration time, and that is a statement about the code rather than an oversight:
/// `build_live_world` hard-codes `BrainPolicy::default()` and `live_experiment::genesis` never
/// inserts an `AgentBrain`. E2-B's first task is to open that seam to the specification in the
/// design doc; flipping this constant belongs in the same commit.
const P1_SEAM_OPEN: bool = false;

fn load(name: &str) -> ExperimentManifest {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{name} parses as ExperimentManifest: {e}"))
}

fn prereg() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join("e2-preregistration.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("e2-preregistration.json parses as JSON")
}

fn control() -> ExperimentManifest {
    load("e2-control-shared-brain.json")
}
fn treatment() -> ExperimentManifest {
    load("e2-treatment-evolved-brain.json")
}

fn keys(m: &ExperimentManifest) -> Vec<&str> {
    m.initial_conditions
        .values
        .iter()
        .map(|(k, _)| k.as_str())
        .collect()
}

// ---- The manifests are real manifests --------------------------------------------------------

#[test]
fn every_preregistered_manifest_parses_and_validates_against_the_live_registry() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("the live registry is valid");
    for name in [
        "e2-control-shared-brain.json",
        "e2-treatment-evolved-brain.json",
        "e2-smoke-control-shared-brain.json",
        "e2-smoke-treatment-evolved-brain.json",
    ] {
        let m = load(name);
        assert_eq!(m.schema_version, MANIFEST_SCHEMA_VERSION, "{name}");
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} must validate: {e}"));
    }
}

#[test]
fn the_declared_factor_is_exactly_one_initial_condition_key() {
    let (c, t) = (control(), treatment());

    // The allowlist the runner will be handed. `FactorDiff` works at manifest-path granularity, so
    // this alone would tolerate ANY initial-condition difference — the key-level assertion below is
    // what actually pins the factor.
    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string()],
    };
    let diffs = allowed
        .validate(&c, &t)
        .expect("control and treatment differ only at declared factors");
    assert_eq!(
        diffs,
        vec!["initial_conditions".to_string()],
        "the pair must differ at exactly one manifest path"
    );

    let (ck, tk) = (keys(&c), keys(&t));
    let only_in_treatment: Vec<&str> = tk.iter().filter(|k| !ck.contains(k)).copied().collect();
    let only_in_control: Vec<&str> = ck.iter().filter(|k| !tk.contains(k)).copied().collect();
    assert_eq!(
        only_in_treatment,
        vec![EVOLVED_KEY],
        "the treatment may add exactly one initial condition"
    );
    assert!(
        only_in_control.is_empty(),
        "the control must not declare anything the treatment lacks: {only_in_control:?}"
    );

    // Every shared key must hold the same value. A silent change to `live.founders` alongside the
    // brain flag would still pass the path-level check above and would make the comparison
    // uninterpretable.
    for (k, cv) in &c.initial_conditions.values {
        let tv = t
            .initial_conditions
            .get(k)
            .unwrap_or_else(|| panic!("treatment is missing shared initial condition '{k}'"));
        assert_eq!(
            *cv, tv,
            "shared initial condition '{k}' must be identical in both arms"
        );
    }
}

#[test]
fn the_two_arms_share_everything_the_pairing_depends_on() {
    let (c, t) = (control(), treatment());
    assert_eq!(
        c.seeds, t.seeds,
        "paired runs need the same seeds in the same order"
    );
    assert_eq!(c.duration_ticks, t.duration_ticks);
    assert_eq!(c.sample_period, t.sample_period);
    assert_eq!(c.world_identity, t.world_identity);
    assert_eq!(c.laws, t.laws);
    assert_eq!(c.laws.fingerprint(), t.laws.fingerprint());
    assert_eq!(c.observable_ids, t.observable_ids);
    assert!(c.interventions.is_empty() && t.interventions.is_empty());
    assert!(c.exotic_interventions.is_empty() && t.exotic_interventions.is_empty());

    // Labels differ, and labels are excluded from the fingerprint — but the declared factor is not,
    // so the two manifests must NOT share an identity.
    assert_ne!(
        c.fingerprint(),
        t.fingerprint(),
        "the declared factor must change the manifest fingerprint, or the run's identity would not \
         record which arm it was"
    );
}

// ---- Seeds -------------------------------------------------------------------------------------

#[test]
fn twelve_experimental_seeds_are_declared_in_execution_order_and_are_unique() {
    let c = control();
    assert_eq!(c.seeds.len(), 12, "N as preregistered");
    let mut sorted = c.seeds.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 12, "seeds must be unique");

    // The stated rule, checked rather than trusted: seed_k = 700001 + 1000*k, in execution order.
    let expected: Vec<u64> = (0..12).map(|k| 700_001 + 1_000 * k).collect();
    assert_eq!(
        c.seeds, expected,
        "the seed list must match the rule the preregistration states, in execution order"
    );
}

#[test]
fn the_smoke_seed_cannot_enter_the_experimental_ensemble() {
    // Mechanical, not a promise: `run_manifest_seed` refuses a seed a manifest does not declare
    // (`ExperimentError::SeedNotInManifest`), so a smoke seed absent from both experimental
    // manifests cannot contribute a run even if somebody asks for it by hand.
    for (name, m) in [("control", control()), ("treatment", treatment())] {
        assert!(
            !m.seeds.contains(&SMOKE_SEED),
            "{name} must not declare the smoke seed {SMOKE_SEED}"
        );
    }
    for name in [
        "e2-smoke-control-shared-brain.json",
        "e2-smoke-treatment-evolved-brain.json",
    ] {
        let m = load(name);
        assert_eq!(
            m.seeds,
            vec![SMOKE_SEED],
            "{name} must declare the smoke seed and nothing else"
        );
    }
}

// ---- Metrics -----------------------------------------------------------------------------------

#[test]
fn the_requested_metrics_are_exactly_what_the_live_adapter_emits() {
    let reg = ObservableRegistry::live_default();
    let c = control();
    let declared: Vec<&str> = c.observable_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        declared,
        LIVE_OBSERVABLE_IDS.to_vec(),
        "the manifest must request the adapter's emitted set, in its emission order"
    );
    for id in &declared {
        let spec = reg
            .get(id)
            .unwrap_or_else(|| panic!("'{id}' has no registry spec"));
        assert!(!spec.unit.is_empty(), "{id} needs a unit");
        assert!(spec.cadence_period > 0, "{id} needs a real cadence");
    }

    // The primary and secondary metrics named in the preregistration must all be registry
    // observables — never a provenance or checksum field dressed up as a metric.
    let p = prereg();
    for h in p["hypotheses"].as_array().expect("hypotheses is a list") {
        let obs = h["observable"].as_str().expect("each hypothesis names one");
        assert!(
            LIVE_OBSERVABLE_IDS.contains(&obs),
            "hypothesis {} names '{obs}', which the live adapter does not emit",
            h["id"]
        );
    }
}

#[test]
fn the_ensemble_fits_the_declared_memory_ceiling() {
    let reg = ObservableRegistry::live_default();
    let c = control();
    assert_eq!(c.samples_per_run(), 30, "T / sample_period");
    let bytes = c.estimated_result_bytes(&reg);
    assert!(
        bytes < MAX_ENSEMBLE_RESULT_BYTES,
        "one arm is estimated at {bytes} bytes against a ceiling of {MAX_ENSEMBLE_RESULT_BYTES}"
    );
}

// ---- The plan and the manifests cannot drift apart ---------------------------------------------

#[test]
fn the_preregistration_document_agrees_with_the_manifests_it_describes() {
    let p = prereg();
    let c = control();

    assert_eq!(
        p["contains_results"],
        serde_json::json!(false),
        "this artifact is registered before the run and must say so"
    );

    let planned: Vec<u64> = p["seeds"]["execution_order"]
        .as_array()
        .expect("execution_order is a list")
        .iter()
        .map(|v| v.as_u64().expect("a seed is an integer"))
        .collect();
    assert_eq!(planned, c.seeds, "the plan's seed order is the manifest's");
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(SMOKE_SEED));
    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(12));

    assert_eq!(
        p["duration"]["duration_ticks"].as_u64(),
        Some(c.duration_ticks),
        "the plan's T is the manifest's"
    );
    assert_eq!(
        p["duration"]["sample_period"].as_u64(),
        Some(c.sample_period)
    );
    assert_eq!(
        p["duration"]["samples_per_run"].as_u64(),
        Some(c.samples_per_run())
    );

    // The chosen T must be the top rung of the declared ladder, so a later run cannot claim a rung
    // that was never registered.
    let ladder: Vec<u64> = p["duration"]["duration_ticks_ladder"]
        .as_array()
        .expect("a ladder")
        .iter()
        .map(|v| v.as_u64().expect("integer ticks"))
        .collect();
    assert_eq!(ladder.first(), Some(&c.duration_ticks));
    assert!(
        ladder.windows(2).all(|w| w[0] > w[1]),
        "the ladder must descend: {ladder:?}"
    );

    // Total tick budget, stated once and checked against the manifests rather than retyped.
    let expected_ticks = (c.seeds.len() as u64) * 2 * c.duration_ticks;
    assert_eq!(
        p["cost"]["total_ticks"].as_u64(),
        Some(expected_ticks),
        "N x 2 arms x T"
    );

    assert_eq!(
        p["declared_factor"]["exact_initial_condition_difference"]["key"].as_str(),
        Some(EVOLVED_KEY)
    );
    assert_eq!(
        p["manifests"]["control"].as_str(),
        Some("e2-control-shared-brain.json")
    );
    assert_eq!(
        p["manifests"]["treatment"].as_str(),
        Some("e2-treatment-evolved-brain.json")
    );
}

// ---- The seam ----------------------------------------------------------------------------------

#[test]
fn the_control_arm_is_runnable_today_and_the_treatment_arm_is_not() {
    let c = control();
    for k in keys(&c) {
        assert!(
            LIVE_KEYS.contains(&k),
            "the control declares '{k}', which LiveWorldConfig::from_initial_conditions would \
             refuse; the control arm has to be runnable as committed"
        );
    }

    // The recorded state of blocking precondition P1. `LiveWorldConfig::from_initial_conditions`
    // rejects every key it does not know, so today the treatment manifest is refused at model
    // construction — the treatment arm does not exist yet, which is precisely why the experiment is
    // registered now and run later.
    assert_eq!(
        LIVE_KEYS.contains(&EVOLVED_KEY),
        P1_SEAM_OPEN,
        "P1_SEAM_OPEN says the live adapter {} honour '{EVOLVED_KEY}', and LIVE_KEYS disagrees. \
         When E2-B opens the seam it must flip P1_SEAM_OPEN in the same commit; nothing else in \
         this preregistration may change.",
        if P1_SEAM_OPEN { "does" } else { "does not" }
    );
}
