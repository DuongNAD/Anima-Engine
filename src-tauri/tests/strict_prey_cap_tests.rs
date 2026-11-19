//! Card FIX-CAP (`live.strict_prey_cap`, FINDINGS F-052): opt-in strict population caps in
//! `autonomous_reproduction_system`.
//!
//! Covers every acceptance requirement without pinning any absolute world checksum or trajectory:
//! - Key registration (`LIVE_KEY_STRICT_PREY_CAP = "live.strict_prey_cap"`, `LIVE_KEYS.len() = 27`),
//!   exact boolean validation, `PreyCapPolicy::default()` / `LiveWorldConfig::default()` parity, and
//!   serde backward compatibility.
//! - (a) With `live.reproduction = 1.0` and `live.strict_prey_cap = 1.0`, prey count stays `<= prey_cap`
//!   at every end of tick over several thousand ticks (60 founders with cap 80, and 100 founders with
//!   cap 80 after deaths bring the population to/below cap).
//! - (b) Non-vacuity: `total_births > 0` in strict mode (both on initial approach and after deaths).
//! - (c) Positive control: in the same configurations with `live.strict_prey_cap` absent, the legacy
//!   soft-gate path overshoots `> prey_cap`.
//! - (d) Key absent and `0.0` produce identical checksums in the same process.
//! - (e) Predators: strict mode enforces both the `<= 8` predator ceiling and the `5:1`
//!   prey-to-predator gate against legacy overshoot.

use anima_engine_lib::core::components::{Agent, Predator, Prey};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT,
    LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_PREY_CAP,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_STRICT_PREY_CAP, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::resources::{PreyCapPolicy, VitalStatistics, DEFAULT_PREY_CAP};
use bevy_ecs::prelude::*;

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
    assert_eq!(LIVE_KEYS.len(), 27);
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
    assert_eq!(
        peak_strict_pred, 8,
        "non-vacuity: strict predator count must grow from 6 up to ceiling 8 without overshooting"
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

    // 2. Predator 5:1 prey/predator ratio gate:
    // 20 founders with 15% predators -> 3 predators, 17 prey (with prey_cap = 17 so prey stays 17).
    // Gate checks `prey_count >= pred_count * 5` before each birth:
    // - Legacy checks once at start (`17 >= 15`) and all 3 reproduce -> 6 predators.
    // - Strict allows 1st birth (`3 -> 4`), then sees `17 < 4 * 5 = 20` and stops -> 4 predators.
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
        5,
    )
    .expect("strict ratio world builds");
    for tick in 1..=5 {
        strict_ratio.run_schedule_once();
        let (prey, pred, _) = guild_counts(&mut strict_ratio);
        assert_eq!(prey, 17, "tick {tick}: prey capped at 17");
        assert_eq!(
            pred, 4,
            "tick {tick}: strict 5:1 ratio gate must stop after 3 -> 4 (got {pred})"
        );
    }

    let mut legacy_ratio = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &ratio_init(None),
        &[],
        37,
        (16, 16),
        5,
    )
    .expect("legacy ratio world builds");
    legacy_ratio.run_schedule_once();
    let (_, legacy_pred_t1, _) = guild_counts(&mut legacy_ratio);
    assert_eq!(
        legacy_pred_t1, 6,
        "positive control: legacy ratio gate overshoots 3 -> 6 in one tick"
    );
}
