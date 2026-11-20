//! Card FIX-CAP / FIX-CAP2 (`live.strict_prey_cap`, FINDINGS F-052): opt-in strict population
//! caps in `autonomous_reproduction_system` (normal/selected mode and neutral-drift replay).
//!
//! Covers every acceptance requirement without pinning any absolute world checksum or trajectory:
//! - Key registration (`LIVE_KEY_STRICT_PREY_CAP = "live.strict_prey_cap"`, `LIVE_KEYS.len() = 29`),
//!   exact boolean validation, `PreyCapPolicy::default()` / `LiveWorldConfig::default()` parity, and
//!   serde backward compatibility.
//! - (a) With `live.reproduction = 1.0` and `live.strict_prey_cap = 1.0`, prey count stays `<= prey_cap`
//!   at every end of tick over several thousand ticks (60 founders with cap 80, and 100 founders with
//!   cap 80 after deaths bring the population to/below cap).
//! - (b) Non-vacuity: `total_births > 0` in strict mode (both on initial approach and after deaths).
//! - (c) Positive control: in the same configurations with `live.strict_prey_cap` absent, the legacy
//!   soft-gate path overshoots `> prey_cap`.
//! - (d) Key absent and `0.0` produce identical checksums in the same process.
//! - (e) Predators: strict mode enforces both the `<= 8` predator ceiling and the pre-birth `5:1`
//!   prey-to-predator running gate (`pred <= ((prey / 5) + 1).min(8)`) against legacy overshoot.
//! - (f) Neutral-drift `Replay` (`live.drift_control = 1.0`): `strict = 1.0` enforces the exact same
//!   running-count caps in drift replay, records cap-blocked births under
//!   `DriftDiagnostics::missed_births_capped`, omits zero values via `skip_serializing_if`, keeps
//!   legacy `absent == 0.0` bypass bit-identical, and surfaces `capped=` in `shortfall_warning`.

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{Agent, Predator, Prey};
use anima_engine_lib::core::drift_control::{DriftDiagnostics, TickTurnover, TurnoverSchedule};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{ExperimentModel, RunStatus};
use anima_engine_lib::core::live_experiment::{
    live_observables, run_live_drift_pair, LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS,
    LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_STRICT_PREY_CAP,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::{PreyCapPolicy, VitalStatistics, DEFAULT_PREY_CAP};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::*;
use rand::{rngs::StdRng, SeedableRng};

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn guild_counts(a: &mut LiveExperimentAdapter) -> (usize, usize, u64) {
    let mut w = a.world();
    let prey = w
        .query_filtered::<(), (With<Agent>, With<Prey>)>()
        .iter(&w)
        .count();
    let pred = w
        .query_filtered::<(), (With<Agent>, With<Predator>)>()
        .iter(&w)
        .count();
    let births = w.resource::<VitalStatistics>().total_births;
    (prey, pred, births)
}

fn obs_value(obs: &[(String, f64)], key: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("missing observable {key}"))
        .1
}

// ---- Key registration, validation, and serde backward compatibility -----------------------------

#[test]
fn key_is_registered_validated_and_serde_backward_compatible() {
    assert_eq!(LIVE_KEY_STRICT_PREY_CAP, "live.strict_prey_cap");
    assert_eq!(LIVE_KEYS.len(), 29);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_STRICT_PREY_CAP));

    assert!(!LiveWorldConfig::default().strict_prey_cap);
    assert_eq!(
        PreyCapPolicy::default(),
        PreyCapPolicy {
            cap: DEFAULT_PREY_CAP,
            strict: false,
        }
    );

    let cfg_absent =
        LiveWorldConfig::from_initial_conditions(&initial(&[])).expect("empty manifest builds");
    assert!(!cfg_absent.strict_prey_cap);

    let cfg_zero =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_STRICT_PREY_CAP, 0.0)]))
            .expect("0.0 builds");
    assert!(!cfg_zero.strict_prey_cap);

    let cfg_one =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_STRICT_PREY_CAP, 1.0)]))
            .expect("1.0 builds");
    assert!(cfg_one.strict_prey_cap);

    for bad in [-1.0, 0.5, 2.0] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_STRICT_PREY_CAP, bad)]))
                .expect_err("non-boolean strict_prey_cap must be refused");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "expected OutOfRange for {bad}, got {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[(
            LIVE_KEY_STRICT_PREY_CAP,
            non_finite,
        )]))
        .expect_err("non-finite strict_prey_cap must be refused");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {non_finite}, got {err:?}"
        );
    }

    let mut val = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    val.as_object_mut()
        .expect("json object")
        .remove("strict_prey_cap");
    let decoded: LiveWorldConfig = serde_json::from_value(val).expect("backward compatible serde");
    assert!(!decoded.strict_prey_cap);

    let strict_adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(&[(LIVE_KEY_STRICT_PREY_CAP, 1.0), (LIVE_KEY_PREY_CAP, 42.0)]),
        &[],
        1,
        (16, 16),
        1,
    )
    .expect("strict adapter builds");
    let pol = *strict_adapter.world().resource::<PreyCapPolicy>();
    assert_eq!(
        pol,
        PreyCapPolicy {
            cap: 42,
            strict: true,
        }
    );
}

// ---- (a), (b), (c) Strict prey cap holds over thousands of ticks while legacy overshoots --------

fn prey_scarcity_init(founders: f64, prey_cap: f64, strict: Option<f64>) -> InitialConditionSet {
    let mut pairs = vec![
        (LIVE_KEY_FOUNDERS, founders),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_PREY_CAP, prey_cap),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 8.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ];
    if let Some(val) = strict {
        pairs.push((LIVE_KEY_STRICT_PREY_CAP, val));
    }
    initial(&pairs)
}

#[test]
fn strict_prey_cap_holds_with_births_while_legacy_overshoots() {
    const CAP: usize = 80;
    const TICKS_60: u64 = 3_000;

    // 1. DIAG setup: 60 founders at cap 80 over 3,000 ticks.
    let mut strict_60 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &prey_scarcity_init(60.0, CAP as f64, Some(1.0)),
        &[],
        19,
        (16, 16),
        TICKS_60,
    )
    .expect("strict 60 builds");

    let mut strict_60_births_at_100 = 0;
    for tick in 1..=TICKS_60 {
        strict_60.run_schedule_once();
        let (prey, pred, births) = guild_counts(&mut strict_60);
        assert_eq!(pred, 0);
        assert!(
            prey <= CAP,
            "60-founder strict tick {tick}: prey_count {prey} exceeded prey_cap {CAP}"
        );
        if tick == 100 {
            strict_60_births_at_100 = births;
        }
        if tick % 500 == 0 {
            let mut w = strict_60.world();
            let obs = live_observables(&mut w);
            assert!(
                obs_value(&obs, "live.herbivore_count") <= CAP as f64,
                "observable herbivore_count exceeded cap at tick {tick}"
            );
        }
    }
    let (_, _, strict_60_births) = guild_counts(&mut strict_60);
    assert!(
        strict_60_births > 20 && strict_60_births >= strict_60_births_at_100,
        "non-vacuity (60 founders): strict run must produce births across ticks (got total={strict_60_births}, t100={strict_60_births_at_100})"
    );

    // Positive control (c): same 60-founder config with key absent overshoots (60 -> 120 on tick 1).
    let mut legacy_60 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &prey_scarcity_init(60.0, CAP as f64, None),
        &[],
        19,
        (16, 16),
        50,
    )
    .expect("legacy 60 builds");
    let mut max_legacy_60 = 0usize;
    for _ in 1..=50 {
        legacy_60.run_schedule_once();
        let (prey, _, _) = guild_counts(&mut legacy_60);
        max_legacy_60 = max_legacy_60.max(prey);
    }
    assert!(
        max_legacy_60 > CAP,
        "positive control (60 founders): legacy soft cap must overshoot {CAP}, got peak {max_legacy_60}"
    );

    // 2. DIAG setup: 100 founders with cap 80 after deaths reduce population to/below cap.
    const TICKS_100: u64 = 3_000;
    let init_100 = |strict: Option<f64>| prey_scarcity_init(100.0, CAP as f64, strict);

    let mut strict_100 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init_100(Some(1.0)),
        &[],
        23,
        (16, 16),
        TICKS_100,
    )
    .expect("strict 100 builds");

    let mut enforced = false;
    for tick in 1..=TICKS_100 {
        strict_100.run_schedule_once();
        let (prey, _, births) = guild_counts(&mut strict_100);
        if !enforced && (prey <= CAP || births > 0) {
            enforced = true;
        }
        if enforced {
            assert!(
                prey <= CAP,
                "100-founder strict tick {tick}: after deaths allowed births / prey <= {CAP}, prey reached {prey}"
            );
        }
    }
    let (_, _, strict_100_births) = guild_counts(&mut strict_100);
    assert!(
        enforced && strict_100_births > 0,
        "100-founder strict run must drop to/below {CAP} after deaths and record births (enforced={enforced}, births={strict_100_births})"
    );

    let mut legacy_100 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init_100(None),
        &[],
        23,
        (16, 16),
        TICKS_100,
    )
    .expect("legacy 100 builds");
    let mut prev_births = 0u64;
    let mut max_legacy_on_birth_tick = 0usize;
    for _ in 1..=TICKS_100 {
        legacy_100.run_schedule_once();
        let (prey, _, births) = guild_counts(&mut legacy_100);
        if births > prev_births {
            max_legacy_on_birth_tick = max_legacy_on_birth_tick.max(prey);
            prev_births = births;
            if prey > CAP {
                break;
            }
        }
    }
    assert!(
        max_legacy_on_birth_tick > CAP,
        "positive control (100 founders after deaths): legacy must drop below {CAP} mid-tick and overshoot > {CAP} (max_legacy_on_birth_tick={max_legacy_on_birth_tick})"
    );
}

// ---- (d) Absent and 0.0 produce identical checksums in the same process -----------------------

#[test]
fn absent_and_zero_produce_identical_checksums() {
    let sample = |pairs: &[(&str, f64)], seed: u64| -> Vec<(u64, u32)> {
        let mut a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(pairs),
            &[],
            seed,
            (16, 16),
            1_000,
        )
        .expect("adapter builds");
        let mut out = Vec::new();
        for tick in 1..=1_000 {
            a.run_schedule_once();
            if tick == 1 || tick % 100 == 0 {
                out.push((tick, a.checksum()));
            }
        }
        out
    };

    // Default world
    let stock_abs = sample(&[], 1);
    let stock_zero = sample(&[(LIVE_KEY_STRICT_PREY_CAP, 0.0)], 1);
    assert_eq!(
        stock_abs, stock_zero,
        "default world: absent vs 0.0 differed"
    );

    // Active reproduction + cap-overshooting world
    let active_abs_pairs = [
        (LIVE_KEY_FOUNDERS, 60.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.1),
        (LIVE_KEY_PREY_CAP, 80.0),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 8.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ];
    let mut active_zero_pairs = active_abs_pairs.to_vec();
    active_zero_pairs.push((LIVE_KEY_STRICT_PREY_CAP, 0.0));
    let active_abs = sample(&active_abs_pairs, 7);
    let active_zero = sample(&active_zero_pairs, 7);
    assert_eq!(
        active_abs, active_zero,
        "active world: absent vs 0.0 differed"
    );
}

// ---- (e) Predator strict cap (ceiling <= 8 and 5:1 prey/predator ratio gate) --------------------

#[test]
fn strict_cap_holds_for_predators_ceiling_and_ratio() {
    // 1. Predator ceiling <= 8: 60 founders with 10% predators -> 6 predators, 54 prey.
    // Tick 1 legacy allows all 6 predators (since 6 < 8 and 54 >= 6*5 = 30) to breed -> 12 predators.
    // Strict mode caps predators cleanly at 8 across 1,500 ticks.
    let ceil_init = |strict: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS, 60.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.10),
            (LIVE_KEY_PREY_CAP, 80.0),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 8.0),
            (LIVE_KEY_LAKE_LAYOUT, 1.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_THIRST_STEERING, 1.0),
        ];
        if let Some(v) = strict {
            pairs.push((LIVE_KEY_STRICT_PREY_CAP, v));
        }
        initial(&pairs)
    };

    let mut strict_ceil = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &ceil_init(Some(1.0)),
        &[],
        31,
        (16, 16),
        1_500,
    )
    .expect("strict predator ceiling world builds");
    let mut peak_strict_pred = 0usize;
    for tick in 1..=1_500 {
        strict_ceil.run_schedule_once();
        let (prey, pred, _) = guild_counts(&mut strict_ceil);
        assert!(
            pred <= 8,
            "predator strict tick {tick}: pred_count {pred} exceeded 8"
        );
        assert!(
            prey <= 80,
            "predator strict tick {tick}: prey_count {prey} exceeded 80"
        );
        peak_strict_pred = peak_strict_pred.max(pred);
    }
    assert!(
        peak_strict_pred > 6 && peak_strict_pred <= 8,
        "non-vacuity: strict predator count must grow above 6 without exceeding ceiling 8 (got peak {peak_strict_pred})"
    );

    let mut legacy_ceil = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &ceil_init(None),
        &[],
        31,
        (16, 16),
        10,
    )
    .expect("legacy predator ceiling world builds");
    let mut peak_legacy_pred = 0usize;
    for _ in 1..=10 {
        legacy_ceil.run_schedule_once();
        let (_, pred, _) = guild_counts(&mut legacy_ceil);
        peak_legacy_pred = peak_legacy_pred.max(pred);
    }
    assert!(
        peak_legacy_pred > 8,
        "positive control: legacy predator gate must overshoot 8, got {peak_legacy_pred}"
    );

    // 2. Predator 5:1 prey/predator pre-birth running ratio gate:
    // 20 founders with 15% predators -> 3 predators, 17 prey (with prey_cap = 17 so prey <= 17).
    // Gate checks `prey_count >= pred_count * 5` before each birth against running counts:
    // - After each permitted birth, `pred <= (prey / 5) + 1` (so here `pred <= 4`).
    // - Legacy checks once per tick against the snapshot, allowing same-tick overshoot (`> 4`).
    let ratio_init = |strict: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS, 20.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.15),
            (LIVE_KEY_PREY_CAP, 17.0),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 0.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 0.0),
        ];
        if let Some(v) = strict {
            pairs.push((LIVE_KEY_STRICT_PREY_CAP, v));
        }
        initial(&pairs)
    };

    let mut strict_ratio = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &ratio_init(Some(1.0)),
        &[],
        37,
        (16, 16),
        20,
    )
    .expect("strict ratio world builds");
    let mut prev_pred = 3usize;
    let mut peak_strict_pred = 3usize;
    for tick in 1..=20 {
        strict_ratio.run_schedule_once();
        let (prey, pred, _) = guild_counts(&mut strict_ratio);
        assert!(prey <= 17, "tick {tick}: prey {prey} exceeded cap 17");
        let max_allowed_pred = ((prey / 5) + 1).min(8);
        assert!(
            pred <= max_allowed_pred,
            "tick {tick}: strict 5:1 ratio gate violated (prey={prey}, pred={pred}, max={max_allowed_pred})"
        );
        if pred > prev_pred {
            assert!(
                prey >= (pred - 1) * 5,
                "tick {tick}: predator growth {prev_pred}->{pred} lacked pre-birth 5:1 prey ratio (prey={prey})"
            );
        }
        prev_pred = pred;
        peak_strict_pred = peak_strict_pred.max(pred);
    }
    assert!(
        peak_strict_pred > 3,
        "non-vacuity: strict predator count must grow past initial 3 (got peak {peak_strict_pred})"
    );

    let mut legacy_ratio = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &ratio_init(None),
        &[],
        37,
        (16, 16),
        20,
    )
    .expect("legacy ratio world builds");
    let mut peak_legacy_pred = 0usize;
    for _ in 1..=20 {
        legacy_ratio.run_schedule_once();
        let (_, pred, _) = guild_counts(&mut legacy_ratio);
        peak_legacy_pred = peak_legacy_pred.max(pred);
    }
    assert!(
        peak_legacy_pred > 4,
        "positive control: legacy ratio gate must overshoot strict max 4 (got peak {peak_legacy_pred})"
    );
}

// ---- (f) Neutral-drift Replay strict caps & DriftDiagnostics::missed_births_capped --------------

fn schedule_single_tick(seed: u64, prey_births: u16, pred_births: u16) -> TurnoverSchedule {
    TurnoverSchedule {
        seed,
        source_manifest_fingerprint: 0,
        duration_ticks: 1,
        ticks: vec![TickTurnover {
            prey_deaths: 0,
            pred_deaths: 0,
            prey_births,
            pred_births,
        }],
    }
}

fn step_ticks(adapter: &mut LiveExperimentAdapter, seed: u64, ticks: u64) -> CausalLedger {
    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    let mut rng = StdRng::seed_from_u64(seed);
    for _ in 0..ticks {
        clock.advance();
        adapter.step(&clock, &[], &mut ledger, &mut rng);
    }
    ledger
}

#[test]
fn drift_control_replay_respects_strict_cap_while_legacy_bypasses_it() {
    // 1. Prey cap in drift Replay: 18 prey founders with cap = 20 and 5 scheduled prey births.
    let prey_replay_init = |founders: f64, strict: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS, founders),
            (LIVE_KEY_PREDATOR_FRACTION, 0.0),
            (LIVE_KEY_PREY_CAP, 20.0),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 0.0),
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
        ];
        if let Some(v) = strict {
            pairs.push((LIVE_KEY_STRICT_PREY_CAP, v));
        }
        initial(&pairs)
    };
    let prey_sched = schedule_single_tick(71, 5, 0);

    let mut strict_prey = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &prey_replay_init(18.0, Some(1.0)),
        &[],
        71,
        0,
        0,
        &prey_sched,
    )
    .expect("strict prey drift replay builds");
    let sp_ledger = step_ticks(&mut strict_prey, 71, 1);
    let (sp_prey, _, _) = guild_counts(&mut strict_prey);
    let sp_diag = strict_prey
        .drift_diagnostics()
        .expect("strict drift diagnostics present");
    assert_eq!(sp_prey, 20, "strict replay must stop prey at prey_cap = 20");
    assert_eq!(sp_diag.scheduled_births, 5);
    assert_eq!(sp_diag.actual_births, 2);
    assert_eq!(sp_diag.missed_births_capped, 3);
    assert_eq!(sp_diag.missed_births_extinct, 0);
    assert_eq!(sp_diag.missed_births_insolvent, 0);
    assert_eq!(sp_diag.missed_births(), 3);
    assert!(sp_diag.has_shortfall());
    assert_eq!(sp_diag.ticks_with_missed_births, 1);
    assert_eq!(sp_diag.first_divergence_tick, Some(1));
    assert!(
        sp_ledger
            .all()
            .iter()
            .any(|r| r.target == "live.drift_shortfall_births"),
        "CausalLedger must record live.drift_shortfall_births when missed_births_capped > 0"
    );

    // Legacy absent vs explicit 0.0 must both bypass the cap and stay bit-identical.
    let mut legacy_abs = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &prey_replay_init(18.0, None),
        &[],
        71,
        0,
        0,
        &prey_sched,
    )
    .expect("legacy absent prey drift replay builds");
    let mut legacy_zero = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &prey_replay_init(18.0, Some(0.0)),
        &[],
        71,
        0,
        0,
        &prey_sched,
    )
    .expect("legacy 0.0 prey drift replay builds");
    step_ticks(&mut legacy_abs, 71, 1);
    step_ticks(&mut legacy_zero, 71, 1);
    let (la_prey, _, _) = guild_counts(&mut legacy_abs);
    let la_diag = legacy_abs
        .drift_diagnostics()
        .expect("legacy drift diagnostics present");
    assert_eq!(
        la_prey, 23,
        "legacy replay must bypass prey_cap and execute all 5 scheduled births"
    );
    assert_eq!(la_diag.scheduled_births, 5);
    assert_eq!(la_diag.actual_births, 5);
    assert_eq!(la_diag.missed_births_capped, 0);
    assert_eq!(la_diag.first_divergence_tick, None);
    assert_eq!(
        legacy_abs.checksum(),
        legacy_zero.checksum(),
        "drift replay legacy absent vs 0.0 checksum parity"
    );
    assert_eq!(
        la_diag,
        legacy_zero.drift_diagnostics().expect("0.0 diag present")
    );

    // At-cap legacy bypass: starting already AT prey_cap = 20 with strict absent/0.0 still spawns
    // all 5 scheduled births (20 -> 25) with zero cap misses.
    let mut legacy_at_cap_abs = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &prey_replay_init(20.0, None),
        &[],
        71,
        0,
        0,
        &prey_sched,
    )
    .expect("legacy at-cap absent prey drift replay builds");
    let mut legacy_at_cap_zero = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &prey_replay_init(20.0, Some(0.0)),
        &[],
        71,
        0,
        0,
        &prey_sched,
    )
    .expect("legacy at-cap 0.0 prey drift replay builds");
    step_ticks(&mut legacy_at_cap_abs, 71, 1);
    step_ticks(&mut legacy_at_cap_zero, 71, 1);
    let (lac_prey, _, _) = guild_counts(&mut legacy_at_cap_abs);
    let lac_diag = legacy_at_cap_abs
        .drift_diagnostics()
        .expect("at-cap legacy diag present");
    assert_eq!(
        lac_prey, 25,
        "at-cap legacy replay must bypass prey_cap = 20 and execute all 5 scheduled births"
    );
    assert_eq!(lac_diag.actual_births, 5);
    assert_eq!(lac_diag.missed_births_capped, 0);
    assert_eq!(lac_diag.first_divergence_tick, None);
    assert_eq!(
        legacy_at_cap_abs.checksum(),
        legacy_at_cap_zero.checksum(),
        "at-cap drift replay legacy absent vs 0.0 checksum parity"
    );

    // 2. Predator 5:1 ratio and same-tick prey birth visibility in Replay:
    // Case 2a: 17 prey / 3 pred with prey_cap = 17, requesting 3 prey + 3 pred births:
    // All 3 prey births are capped (`17 >= 17`), then pred allows 1st (`17 >= 15 -> 4`) and blocks
    // 2nd & 3rd (`17 < 20`), totaling 1 actual birth and 5 capped births.
    let pred_ratio_init = |prey_cap: f64, strict: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS, 20.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.15),
            (LIVE_KEY_PREY_CAP, prey_cap),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 0.0),
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
        ];
        if let Some(v) = strict {
            pairs.push((LIVE_KEY_STRICT_PREY_CAP, v));
        }
        initial(&pairs)
    };
    let ratio_sched = schedule_single_tick(73, 3, 3);
    let mut ratio_capped = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &pred_ratio_init(17.0, Some(1.0)),
        &[],
        73,
        0,
        0,
        &ratio_sched,
    )
    .expect("strict ratio replay builds");
    step_ticks(&mut ratio_capped, 73, 1);
    let (rc_prey, rc_pred, _) = guild_counts(&mut ratio_capped);
    let rc_diag = ratio_capped.drift_diagnostics().expect("rc diag");
    assert_eq!((rc_prey, rc_pred), (17, 4));
    assert_eq!(rc_diag.actual_births, 1);
    assert_eq!(rc_diag.missed_births_capped, 5);
    assert_eq!(rc_diag.missed_births_insolvent, 0);
    assert_eq!(rc_diag.missed_births_extinct, 0);
    assert_eq!(rc_diag.first_divergence_tick, Some(1));

    // Legacy bypass on Case 2a: same 17 prey / 3 pred with prey_cap = 17, strict = None,
    // executes all 3 prey + 3 pred births -> (20, 6) with 0 capped births.
    let mut ratio_legacy = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &pred_ratio_init(17.0, None),
        &[],
        73,
        0,
        0,
        &ratio_sched,
    )
    .expect("legacy ratio replay builds");
    step_ticks(&mut ratio_legacy, 73, 1);
    let (rl_prey, rl_pred, _) = guild_counts(&mut ratio_legacy);
    let rl_diag = ratio_legacy.drift_diagnostics().expect("rl diag");
    assert_eq!((rl_prey, rl_pred), (20, 6));
    assert_eq!(rl_diag.actual_births, 6);
    assert_eq!(rl_diag.missed_births_capped, 0);

    // Case 2b: Same founders (17 prey, 3 pred) with prey_cap = 20: the 3 same-tick prey births
    // raise running prey_count to 20 before the predator pass, allowing 2 predator births (3 -> 5,
    // since 20 >= 15 and 20 >= 20) and capping only the 3rd predator birth (20 < 25).
    let mut ratio_same_tick = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &pred_ratio_init(20.0, Some(1.0)),
        &[],
        73,
        0,
        0,
        &ratio_sched,
    )
    .expect("same-tick prey+pred replay builds");
    step_ticks(&mut ratio_same_tick, 73, 1);
    let (rs_prey, rs_pred, _) = guild_counts(&mut ratio_same_tick);
    let rs_diag = ratio_same_tick.drift_diagnostics().expect("rs diag");
    assert_eq!((rs_prey, rs_pred), (20, 5));
    assert_eq!(rs_diag.actual_births, 5);
    assert_eq!(rs_diag.missed_births_capped, 1);
    assert_eq!(rs_diag.missed_births_insolvent, 0);

    // 3. Predator ceiling 8 in Replay: 60 founders (54 prey, 6 pred), prey_cap = 80, pred_births = 4
    // -> allows 6 -> 8 (2 births), blocks 2 at ceiling 8.
    let ceil_replay_init = initial(&[
        (LIVE_KEY_FOUNDERS, 60.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.10),
        (LIVE_KEY_PREY_CAP, 80.0),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_FOOD_CAP, 50.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 0.0),
        (LIVE_KEY_DRIFT_CONTROL, 1.0),
        (LIVE_KEY_STRICT_PREY_CAP, 1.0),
    ]);
    let ceil_sched = schedule_single_tick(79, 0, 4);
    let mut ceil_replay = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &ceil_replay_init,
        &[],
        79,
        0,
        0,
        &ceil_sched,
    )
    .expect("pred ceiling replay builds");
    step_ticks(&mut ceil_replay, 79, 1);
    let (cl_prey, cl_pred, _) = guild_counts(&mut ceil_replay);
    let cl_diag = ceil_replay.drift_diagnostics().expect("ceil diag");
    assert_eq!((cl_prey, cl_pred), (54, 8));
    assert_eq!(cl_diag.actual_births, 2);
    assert_eq!(cl_diag.missed_births_capped, 2);
    assert_eq!(cl_diag.missed_births_extinct, 0);
    assert_eq!(cl_diag.missed_births_insolvent, 0);

    // 4. Serde skip_serializing_if = "is_zero", backward deserialization, and merge_window.
    let zero_json = serde_json::to_string(&la_diag).expect("serialize zero missed_births_capped");
    assert!(
        !zero_json.contains("missed_births_capped"),
        "zero missed_births_capped must be omitted by skip_serializing_if: {zero_json}"
    );
    let legacy_json = r#"{"scheduled_deaths":1,"actual_deaths":1,"missed_deaths":0,"scheduled_births":2,"actual_births":2,"missed_births_extinct":0,"missed_births_insolvent":0,"ticks_with_missed_deaths":0,"ticks_with_missed_births":0,"first_divergence_tick":null}"#;
    let parsed_legacy: DriftDiagnostics =
        serde_json::from_str(legacy_json).expect("legacy DriftDiagnostics JSON must deserialize");
    assert_eq!(parsed_legacy.missed_births_capped, 0);

    let mut merged = sp_diag.clone();
    merged.merge_window(&rc_diag);
    assert_eq!(merged.missed_births_capped, 8);
    assert_eq!(merged.missed_births(), 8);
    let merged_json = serde_json::to_string(&merged).expect("serialize non-zero capped");
    assert!(merged_json.contains("missed_births_capped"));
    let roundtrip: DriftDiagnostics =
        serde_json::from_str(&merged_json).expect("roundtrip non-zero capped");
    assert_eq!(roundtrip, merged);

    // 5. End-to-end paired selected vs drift scarcity + predator run with strict_prey_cap = 1.0:
    // Both arms enforce prey <= prey_cap and pred <= 8 at every sampled tick while completing
    // cleanly, and `capped=` warning formatting tracks whether `missed_births_capped > 0`.
    let obs_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mk_manifest = |id: &str, drift: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS.to_string(), 20.0),
            (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.15),
            (LIVE_KEY_PREY_CAP.to_string(), 30.0),
            (LIVE_KEY_STRICT_PREY_CAP.to_string(), 1.0),
            (LIVE_KEY_TREES.to_string(), 1.0),
            (LIVE_KEY_LAKES.to_string(), 1.0),
            (LIVE_KEY_FOOD_CAP.to_string(), 3.0),
            ("live.starve_ticks".to_string(), 10.0),
            ("live.dehydration_ticks".to_string(), 10.0),
            (LIVE_KEY_MORTALITY.to_string(), 1.0),
            (LIVE_KEY_REPRODUCTION.to_string(), 1.0),
            (LIVE_KEY_LOCOMOTION.to_string(), 1.0),
            (LIVE_KEY_DRIVE_FLOOR.to_string(), 0.5),
            (LIVE_KEY_THIRST_STEERING.to_string(), 1.0),
        ];
        if let Some(d) = drift {
            pairs.push((LIVE_KEY_DRIFT_CONTROL.to_string(), d));
        }
        ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: id.to_string(),
            name: id.to_string(),
            world_identity: WorldIdentity::default(),
            laws: WorldLawSet::baseline(),
            initial_conditions: InitialConditionSet::new(pairs),
            interventions: Vec::new(),
            seeds: vec![31],
            duration_ticks: 800,
            sample_period: 20,
            observable_ids: obs_ids.clone(),
            exotic_interventions: Vec::new(),
            observer: Default::default(),
        }
    };
    let pair = run_live_drift_pair(
        &mk_manifest("fixcap2-sel", None),
        &mk_manifest("fixcap2-drf", Some(1.0)),
        &ObservableRegistry::live_default(),
        31,
    )
    .expect("strict drift pair run must complete");
    assert_eq!(pair.selected.status, RunStatus::Completed);
    assert_eq!(pair.drift.status, RunStatus::Completed);
    for s in pair.selected.series.iter().chain(pair.drift.series.iter()) {
        let prey = obs_value(&s.observables, "live.herbivore_count");
        let pred = obs_value(&s.observables, "live.predator_count");
        assert!(
            prey <= 30.0,
            "tick {}: live.herbivore_count {prey} exceeded prey_cap 30",
            s.tick
        );
        assert!(
            pred <= 8.0,
            "tick {}: live.predator_count {pred} exceeded ceiling 8",
            s.tick
        );
    }
    assert!(
        pair.diagnostics.scheduled_births > 0 && pair.diagnostics.actual_births > 0,
        "non-vacuity: paired strict drift run must reproduce (diag={:?})",
        pair.diagnostics
    );
    if pair.diagnostics.missed_births_capped > 0 {
        assert!(
            pair.drift.warnings.iter().any(|w| w.contains("capped=")),
            "drift warnings must report capped= when missed_births_capped > 0: {:?}",
            pair.drift.warnings
        );
    } else if pair.diagnostics.has_shortfall() {
        assert!(
            pair.drift.warnings.iter().all(|w| !w.contains("capped=")),
            "drift warnings must omit capped= when missed_births_capped == 0: {:?}",
            pair.drift.warnings
        );
    }
}
