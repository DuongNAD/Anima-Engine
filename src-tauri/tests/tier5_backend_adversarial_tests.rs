//! Tier 5 backend adversarial tests for Anima Engine.
//!
//! Covers, at their boundaries:
//! 1. The engine's Arrhenius factor and Kleiber metabolic rate at absolute zero and at zero or
//!    negative mass.
//! 2. The engine's Holling Type II/III responses, predation capture and herbivore intake.
//! 3. Closed-loop energy conservation under rapid catastrophic fluxes and overdrafts.
//! 4. The causal DAG ledger under deep branching, cycle defence, empty traces and ties.
//! 5. Snapshot serialization with extreme float values and schema-version gates.
//!
//! Until 2026-11-05 this file also exercised `anima_domain::laws`' duplicate f64 MTE/Holling
//! functions, `core::sweeps` and `core::branch_tree`. Those modules were removed from `main` with the
//! rest of the 2026-08-16 block (`STATE_OF_THE_PROJECT.md` §1.g), and their tests went with them;
//! every assertion left here runs against code that exists.

use anima_domain::energy::{
    debit_reserve, Compartment, EcosystemBiomass, EnergyEvent, EnergyLedger,
    RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::causal::{
    is_reserved_cause, CausalLedger, CauseId, CAUSE_BACKGROUND, CAUSE_OBSERVER,
};
use anima_engine_lib::core::ecology::{
    boltzmann_factor, herbivore_intake, holling_type2, holling_type3, metabolic_rate,
    predation_capture, E_ANIMAL_EV as E_ANIMAL_EV_F32, LINDEMAN_EFFICIENCY,
};
use anima_engine_lib::core::snapshot::{
    checksum_bytes, from_bytes, SnapshotEnvelope, SnapshotError, MIN_SUPPORTED_SCHEMA,
    SCHEMA_VERSION,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ================================================================================================
// 1. Arrhenius factor and Kleiber metabolic rate at their physical limits
// ================================================================================================

#[test]
fn test_adv_mte_absolute_zero_and_sub_zero_temperature() {
    // 0 Kelvin (-273.15 °C)
    let factor_zero_k = boltzmann_factor(-273.15, E_ANIMAL_EV_F32);
    assert_eq!(
        factor_zero_k, 0.0,
        "Arrhenius factor at -273.15 °C must be 0.0"
    );

    // Below absolute zero
    let factor_neg_c = boltzmann_factor(-300.0, E_ANIMAL_EV_F32);
    assert_eq!(
        factor_neg_c, 0.0,
        "Arrhenius factor below -273.15 °C must clamp to 0.0"
    );
}

#[test]
fn test_adv_mte_zero_and_negative_mass() {
    assert_eq!(metabolic_rate(0.0, 20.0, E_ANIMAL_EV_F32), 0.0);
    assert_eq!(metabolic_rate(-10.0, 20.0, E_ANIMAL_EV_F32), 0.0);
}

// ================================================================================================
// 2. Holling Functional Responses under Degenerate and Boundary Conditions
// ================================================================================================

#[test]
fn test_adv_holling_zero_and_negative_prey_density() {
    assert_eq!(holling_type2(0.0, 0.1, 0.2), 0.0);
    assert_eq!(holling_type3(0.0, 0.1, 0.2), 0.0);
    assert_eq!(holling_type2(-5.0, 0.1, 0.2), 0.0);
    assert_eq!(holling_type3(-5.0, 0.1, 0.2), 0.0);
}

#[test]
fn test_adv_predation_and_herbivore_intake_boundary_conditions() {
    // Predation capture with predator deficit = 0
    assert_eq!(predation_capture(100.0, 0.0), 0.0);
    assert_eq!(predation_capture(100.0, -10.0), 0.0);
    assert_eq!(predation_capture(0.0, 100.0), 0.0);

    // Predation capture deficit clamp: predator with small deficit (2.0) cannot eat 10,000 units
    let captured = predation_capture(10_000.0, 2.0);
    let max_allowed = 2.0 / LINDEMAN_EFFICIENCY + 1e-3;
    assert!(
        captured <= max_allowed,
        "Predation capture exceeded deficit capacity: {captured} vs {max_allowed}"
    );

    // Herbivore intake boundary conditions
    assert_eq!(herbivore_intake(0.0, 10.0, 5.0), 0.0);
    assert_eq!(herbivore_intake(100.0, 0.0, 5.0), 0.0);
    assert_eq!(herbivore_intake(100.0, 10.0, 0.0), 0.0);
    assert_eq!(herbivore_intake(-5.0, 10.0, 5.0), 0.0);
    assert_eq!(herbivore_intake(100.0, -5.0, 5.0), 0.0);

    // Herbivore intake clamp to bite ceiling and hunger
    let intake_capped_by_bite = herbivore_intake(100.0, 50.0, 2.0);
    assert!(intake_capped_by_bite <= 2.0 + 1e-5);

    let intake_capped_by_hunger = herbivore_intake(100.0, 0.5, 5.0);
    assert!(intake_capped_by_hunger <= 0.5 + 1e-5);
}

// ================================================================================================
// 3. Closed-Loop Energy Conservation under Rapid Catastrophic Shocks
// ================================================================================================

#[test]
fn test_adv_closed_loop_catastrophic_mass_culling() {
    let mut ledger = EnergyLedger::default();
    let initial_detritus = 500.0;
    let initial_plants = 1000.0;

    // Setup 1,000 agents holding varying energy reserves summing to ~500.0
    let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
    let mut agent_reserves = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let r = rng.gen_range(0.1f32..1.0f32);
        agent_reserves.push(r);
    }
    // Calculate authoritative animal sum
    let initial_animal_sum: f64 = agent_reserves.iter().map(|&r| r as f64).sum();
    let mut pool = EcosystemBiomass {
        detritus: initial_detritus,
        plants: initial_plants,
        animals: initial_animal_sum,
    };
    let baseline = pool.total();
    ledger.lock_baseline(baseline);

    // Catastrophic shock: cull every agent simultaneously, returning exact energy to detritus
    for r in agent_reserves.iter_mut() {
        let corpse_eu = debit_reserve(r, *r);
        assert_eq!(*r, 0.0f32);
        pool.animals = (pool.animals - corpse_eu).max(0.0);
        pool.detritus += corpse_eu;
    }

    assert!(
        (pool.total() - baseline).abs() < 1e-9,
        "Total closed biomass drifted after catastrophic culling: {} vs {}",
        pool.total(),
        baseline
    );
    assert!(ledger.conserves(pool.total()));
    assert_eq!(pool.animals, 0.0);
    assert!((pool.detritus - (initial_detritus + initial_animal_sum)).abs() < 1e-6);
}

#[test]
fn test_adv_closed_loop_10k_random_adversarial_flux_sequence() {
    let mut ledger = EnergyLedger::default();
    let mut pool = EcosystemBiomass {
        detritus: 10_000.0,
        plants: 10_000.0,
        animals: 10_000.0,
    };
    let baseline = pool.total();
    ledger.lock_baseline(baseline);

    let mut rng = StdRng::seed_from_u64(0xC0FFEE);
    let compartments = [
        Compartment::Detritus,
        Compartment::Plants,
        Compartment::Animals,
    ];

    let mut animal_reserve = 50.0f32;

    for _ in 0..10_000 {
        let op = rng.gen_range(0..4);
        match op {
            0 => {
                // Inter-compartment transfer
                let from = compartments[rng.gen_range(0..3)];
                let to = compartments[rng.gen_range(0..3)];
                let amt = rng.gen_range(0.0..500.0);
                ledger.transfer(&mut pool, from, to, amt, EnergyEvent::Intervention);
            }
            1 => {
                // Attempted overdraft (exceeding balance)
                let from = compartments[rng.gen_range(0..3)];
                let to = compartments[rng.gen_range(0..3)];
                let amt = 1_000_000.0; // massive overdraft
                ledger.transfer(&mut pool, from, to, amt, EnergyEvent::Intervention);
            }
            2 => {
                // Transfer into f32 animal reserve
                let desired = rng.gen_range(0.0f32..100.0f32);
                ledger.transfer_into_reserve(
                    &mut pool,
                    Compartment::Detritus,
                    &mut animal_reserve,
                    200.0,
                    desired,
                    EnergyEvent::Feeding,
                );
            }
            _ => {
                // Animal reserve burn / debit back to detritus
                let burn = rng.gen_range(0.0f32..30.0f32);
                let debited = debit_reserve(&mut animal_reserve, burn);
                ledger.transfer(
                    &mut pool,
                    Compartment::Animals,
                    Compartment::Detritus,
                    debited,
                    EnergyEvent::Intervention,
                );
            }
        }

        // Verify that neither compartment went negative beyond numerical tolerance
        assert!(
            pool.detritus >= -RESIDUAL_ABS_TOLERANCE_EU,
            "Detritus was negative: {}",
            pool.detritus
        );
        assert!(
            pool.plants >= -RESIDUAL_ABS_TOLERANCE_EU,
            "Plants was negative: {}",
            pool.plants
        );
        assert!(
            pool.animals >= -RESIDUAL_ABS_TOLERANCE_EU,
            "Animals was negative: {}",
            pool.animals
        );

        // Strict conservation invariant after every single transaction
        let current_total = pool.total();
        assert!(
            (current_total - baseline).abs() < RESIDUAL_ABS_TOLERANCE_EU,
            "Energy conservation violated: current={current_total}, baseline={baseline}, drift={}",
            (current_total - baseline).abs()
        );
    }

    assert!(ledger.conserves(pool.total()));
}

#[test]
fn test_adv_closed_loop_subnormal_and_rounding_immunity() {
    // Tests that tiny increments into a large reserve never leak phantom energy
    let mut ledger = EnergyLedger::default();
    let mut pool = EcosystemBiomass {
        detritus: 1_000.0,
        plants: 0.0,
        animals: 1.0e7,
    };
    let initial_detritus = pool.detritus;
    let mut reserve = 1.0e7f32; // large f32 reserve

    for _ in 0..10_000 {
        // Desired amount is 1e-5 which rounds away when added to 1e7 in f32
        let moved = ledger.transfer_into_reserve(
            &mut pool,
            Compartment::Detritus,
            &mut reserve,
            f32::MAX,
            1.0e-5,
            EnergyEvent::Feeding,
        );
        assert_eq!(moved, 0.0, "Rounded-away add must result in 0.0 moved");
    }

    assert_eq!(
        pool.detritus, initial_detritus,
        "Source detritus must remain untouched when destination add rounds away"
    );
}

#[test]
fn test_adv_energy_ledger_refused_deficit_accounting() {
    let mut ledger = EnergyLedger::default();
    let mut pool = EcosystemBiomass {
        detritus: 15.0,
        plants: 0.0,
        animals: 0.0,
    };

    let mut reserve = 0.0f32;
    let moved = ledger.transfer_into_reserve(
        &mut pool,
        Compartment::Detritus,
        &mut reserve,
        1000.0,
        100.0,
        EnergyEvent::Replacement,
    );

    assert_eq!(moved, 15.0);
    assert_eq!(reserve, 15.0);
    assert_eq!(pool.detritus, 0.0);
    assert!(
        (ledger.refused() - 85.0).abs() < 1e-9,
        "Shortfall must be accurately recorded in refused(): {}",
        ledger.refused()
    );
}

// ================================================================================================
// 4. Causal DAG Ledger Robustness, Cycle Defense & Deep Branching
// ================================================================================================

#[test]
fn test_adv_causal_dag_deep_branching_10k() {
    let mut ledger = CausalLedger::new();
    let root_cause: CauseId = 777;

    // Build a 10,000-deep linear causal chain
    let mut parent = None;
    for i in 0..10_000 {
        let eff = ledger.record(
            root_cause,
            parent,
            i as u64,
            "state_var",
            100.0 + (i as f64),
            1.0,
            format!("step {i}"),
        );
        parent = Some(eff);
    }

    let leaf_id = parent.unwrap();
    assert_eq!(ledger.len(), 10_000);

    // trace_to_root must execute without stack overflow and return 10,000 elements
    let trace = ledger.trace_to_root(leaf_id);
    assert_eq!(trace.len(), 10_000);
    assert_eq!(trace.first(), Some(&leaf_id));
    assert_eq!(trace.last(), Some(&0));

    // Root cause must match root_cause for all records
    assert_eq!(ledger.root_cause(leaf_id), Some(root_cause));
    assert_eq!(ledger.root_cause(0), Some(root_cause));
}

#[test]
fn test_adv_causal_dag_cycle_and_forward_link_defense() {
    let mut ledger = CausalLedger::new();
    let e0 = ledger.record(1, None, 0, "var", 1.0, 1.0, "root");
    let e1 = ledger.record(1, Some(e0), 1, "var", 2.0, 1.0, "step 1");

    // Verify normal chain
    assert_eq!(ledger.trace_to_root(e1), vec![e1, e0]);

    // Construct an adversarial/corrupted ledger with cycle (parent == self or parent > self)
    // using serde deserialization to bypass the constructor
    let json_corrupted = r#"{
        "effects": [
            {
                "effect_id": 0,
                "cause_id": 1,
                "parent": 1,
                "tick": 0,
                "target": "v",
                "quantity": 1.0,
                "delta": 1.0,
                "mechanism": "cyclic"
            },
            {
                "effect_id": 1,
                "cause_id": 1,
                "parent": 1,
                "tick": 1,
                "target": "v",
                "quantity": 2.0,
                "delta": 1.0,
                "mechanism": "self-parent"
            }
        ]
    }"#;

    let corrupted_ledger: CausalLedger =
        serde_json::from_str(json_corrupted).expect("parses corrupted ledger");

    // Self-parent on e1: trace must terminate immediately without spinning forever
    let trace1 = corrupted_ledger.trace_to_root(1);
    assert_eq!(
        trace1,
        vec![1],
        "Self-parent cycle must terminate at the node"
    );

    // Forward parent on e0: trace must terminate immediately
    let trace0 = corrupted_ledger.trace_to_root(0);
    assert_eq!(
        trace0,
        vec![0],
        "Forward-parent link must terminate at the node"
    );
}

#[test]
fn test_adv_causal_dag_empty_and_out_of_bounds_traces() {
    let ledger = CausalLedger::new();
    assert!(ledger.is_empty());
    assert_eq!(ledger.len(), 0);

    // Query non-existent ID
    assert_eq!(ledger.trace_to_root(0), Vec::<u32>::new());
    assert_eq!(ledger.trace_to_root(u32::MAX), Vec::<u32>::new());
    assert_eq!(ledger.root_cause(0), None);
    assert_eq!(ledger.root_cause(u32::MAX), None);

    // Reserved cause IDs
    assert!(is_reserved_cause(CAUSE_BACKGROUND));
    assert!(is_reserved_cause(CAUSE_OBSERVER));
    assert!(!is_reserved_cause(1));
    assert!(!is_reserved_cause(42));
}

#[test]
fn test_adv_causal_dag_top_contributors_adversarial() {
    let mut ledger = CausalLedger::new();

    // 1. Query empty ledger
    let top_empty = ledger.top_contributors("npp", (0, 100), 5);
    assert!(top_empty.is_empty());

    // 2. Add records with tied deltas and out-of-window ticks
    ledger.record(1, None, 10, "target_a", 10.0, 5.0, "mech_1");
    ledger.record(1, None, 20, "target_a", 15.0, -5.0, "mech_2"); // Tied absolute delta
    ledger.record(1, None, 30, "target_a", 20.0, 2.0, "mech_3");
    ledger.record(1, None, 200, "target_a", 50.0, 30.0, "out_of_window");
    ledger.record(1, None, 15, "target_b", 10.0, 10.0, "wrong_target");

    // Window (0, 50), k = 2
    let top = ledger.top_contributors("target_a", (0, 50), 2);
    assert_eq!(top.len(), 2);
    // Stable tie order: mech_1 then mech_2
    assert_eq!(top[0].mechanism, "mech_1");
    assert_eq!(top[1].mechanism, "mech_2");

    // Inverted window (50, 0)
    let top_inv = ledger.top_contributors("target_a", (50, 0), 5);
    assert!(top_inv.is_empty());

    // Window with no matches
    let top_nomatch = ledger.top_contributors("target_c", (0, 100), 5);
    assert!(top_nomatch.is_empty());
}

// ================================================================================================
// 5. State Snapshot Serialization, Subnormals, Precision Roundtrips & Error Gates
// ================================================================================================

#[test]
fn test_adv_snapshot_extreme_float_precision_and_subnormals() {
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();

    // Set high-precision and subnormal float values
    state.eco_animals = 990.5102615356445;
    state.eco_detritus = 1e-300; // Subnormal float
    state.eco_plants = 123_456_789.987_654_33; // the f64 nearest 123456789.987654321
    state.season_phase = std::f32::consts::PI;

    let envelope = SnapshotEnvelope::seal(state.clone()).expect("seals extreme float state");
    let bytes = serde_json::to_vec(&envelope).expect("serializes envelope");
    let restored = from_bytes(&bytes).expect("restores extreme float state");

    // Exact bit-for-bit float roundtrips
    assert_eq!(restored.eco_animals, 990.5102615356445);
    assert_eq!(restored.eco_detritus, 1e-300);
    assert_eq!(restored.eco_plants, 123_456_789.987_654_33);
    assert_eq!(restored.season_phase, std::f32::consts::PI);
}

#[test]
fn test_adv_snapshot_negative_tolerance_boundary() {
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();

    // Allowed tiny numerical noise: -1e-4 within SNAPSHOT_NEGATIVE_TOLERANCE (1e-3)
    state.eco_detritus = -1e-4;
    assert!(SnapshotEnvelope::seal(state.clone()).is_ok());

    // Materially negative balance: -0.5 must be rejected
    state.eco_detritus = -0.5;
    match SnapshotEnvelope::seal(state) {
        Err(SnapshotError::InvalidState(err)) => {
            assert!(err.contains("ecosystem"));
        }
        other => panic!("expected InvalidState rejection for negative balance, got {other:?}"),
    }
}

#[test]
fn test_adv_snapshot_non_finite_rejection() {
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();

    // NaN detritus
    state.eco_detritus = f64::NAN;
    assert!(matches!(
        SnapshotEnvelope::seal(state.clone()),
        Err(SnapshotError::InvalidState(_))
    ));

    // Infinity plants
    state.eco_detritus = 100.0;
    state.eco_plants = f64::INFINITY;
    assert!(matches!(
        SnapshotEnvelope::seal(state),
        Err(SnapshotError::InvalidState(_))
    ));
}

#[test]
fn test_adv_snapshot_schema_version_gates() {
    let state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();
    let state_text = serde_json::to_string(&state).expect("serializes");
    let checksum = checksum_bytes(&state_text);

    // Current schema (v7) passes
    let env_v7 = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        build_provenance: Default::default(),
        checksum,
        state: serde_json::value::RawValue::from_string(state_text.clone()).unwrap(),
    };
    let bytes_v7 = serde_json::to_vec(&env_v7).unwrap();
    assert!(from_bytes(&bytes_v7).is_ok());

    // Outdated enveloped schema (e.g. schema 4 < MIN_SUPPORTED_SCHEMA 5) is rejected
    let env_v4 = SnapshotEnvelope {
        schema_version: MIN_SUPPORTED_SCHEMA - 1,
        build_provenance: Default::default(),
        checksum,
        state: serde_json::value::RawValue::from_string(state_text.clone()).unwrap(),
    };
    let bytes_v4 = serde_json::to_vec(&env_v4).unwrap();
    match from_bytes(&bytes_v4) {
        Err(SnapshotError::TooOld { found, minimum }) => {
            assert_eq!(found, MIN_SUPPORTED_SCHEMA - 1);
            assert_eq!(minimum, MIN_SUPPORTED_SCHEMA);
        }
        other => panic!("expected TooOld, got {other:?}"),
    }

    // Future schema (v8) is rejected
    let env_v8 = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION + 1,
        build_provenance: Default::default(),
        checksum,
        state: serde_json::value::RawValue::from_string(state_text).unwrap(),
    };
    let bytes_v8 = serde_json::to_vec(&env_v8).unwrap();
    match from_bytes(&bytes_v8) {
        Err(SnapshotError::TooNew { found, current }) => {
            assert_eq!(found, SCHEMA_VERSION + 1);
            assert_eq!(current, SCHEMA_VERSION);
        }
        other => panic!("expected TooNew, got {other:?}"),
    }
}
