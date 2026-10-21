//! Tier 5 Backend Adversarial Test Suite for Anima Engine.
//!
//! Conducts Phase 2 Tier 5 Adversarial Coverage Hardening across:
//! 1. MTE Kleiber scaling & Arrhenius thermal scaling at extreme temperatures (0K, boiling, subnormals).
//! 2. Holling Type II & III responses under zero prey, infinite prey, extreme handling times.
//! 3. Closed-loop energy conservation under rapid catastrophic intervention fluxes and overdrafts.
//! 4. Causal DAG ledger under deep branching, cycles defense, empty traces, and tie-breaking.
//! 5. Parameter sweeps with degenerate parameter spaces (0-range, single-point Sobol/LHS, Pearson zero-variance).
//! 6. State snapshot serialization with extreme float values (subnormals, precision roundtrips, tolerance bounds).
//! 7. Causal Branch Tree topology validation, trajectory divergence, and DTW distance edge cases.

use anima_domain::energy::{
    debit_reserve, Compartment, EcosystemBiomass, EnergyEvent, EnergyLedger,
    RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_domain::laws::{
    boltzmann_factor as domain_boltzmann, holling_type_2 as domain_holling2,
    holling_type_3 as domain_holling3, mte_basal_metabolism as domain_mte,
    BOLTZMANN_EV, E_ANIMAL_EV, KLEIBER_EXP, METABOLIC_NORM, REF_TEMP_K,
};
use anima_engine_lib::core::branch_tree::{
    dynamic_time_warping, BranchNodeSpecification, BranchSeedStrategy, BranchTreeManifest,
};
use anima_engine_lib::core::causal::{
    is_reserved_cause, CausalLedger, CauseId, CAUSE_BACKGROUND, CAUSE_OBSERVER,
};
use anima_engine_lib::core::ecology::{
    boltzmann_factor, herbivore_intake, holling_type2, holling_type3,
    metabolic_rate, predation_capture, E_ANIMAL_EV as E_ANIMAL_EV_F32, LINDEMAN_EFFICIENCY,
};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, ObservableRegistry,
};
use anima_engine_lib::core::snapshot::{
    checksum_bytes, from_bytes, SnapshotEnvelope, SnapshotError, MIN_SUPPORTED_SCHEMA,
    SCHEMA_VERSION,
};
use anima_engine_lib::core::sweeps::{
    pearson_correlation, DimensionSpec, ParameterDim, SamplingStrategy, SobolSequence,
    SweepManifest, SweepTarget,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ================================================================================================
// 1. MTE Kleiber Scaling & Arrhenius Thermal Scaling at Extreme Temperatures & Masses
// ================================================================================================

#[test]
fn test_adv_mte_absolute_zero_and_sub_zero_temperature() {
    // 0 Kelvin (-273.15 °C)
    let b_zero_k = domain_mte(1.0, 0.0, METABOLIC_NORM, E_ANIMAL_EV);
    assert_eq!(b_zero_k, 0.0, "Metabolism at absolute zero must be 0.0");

    let factor_zero_k = boltzmann_factor(-273.15, E_ANIMAL_EV_F32);
    assert_eq!(factor_zero_k, 0.0, "Arrhenius factor at -273.15 °C must be 0.0");

    let factor_domain_zero_k = domain_boltzmann(-273.15, E_ANIMAL_EV);
    assert_eq!(factor_domain_zero_k, 0.0, "Domain Arrhenius factor at -273.15 °C must be 0.0");

    // Sub-absolute zero (e.g. -300.0 °C / negative Kelvin)
    let b_neg_k = domain_mte(1.0, -10.0, METABOLIC_NORM, E_ANIMAL_EV);
    assert_eq!(b_neg_k, 0.0, "Metabolism at negative Kelvin must be 0.0");

    let factor_neg_c = boltzmann_factor(-300.0, E_ANIMAL_EV_F32);
    assert_eq!(factor_neg_c, 0.0, "Arrhenius factor below -273.15 °C must clamp to 0.0");

    // Near absolute zero (1e-6 Kelvin)
    let b_near_zero = domain_mte(1.0, 1e-6, METABOLIC_NORM, E_ANIMAL_EV);
    assert!(
        b_near_zero >= 0.0 && b_near_zero < 1e-100,
        "Near-zero Kelvin must underflow gracefully to ~0.0 without NaN: got {b_near_zero}"
    );
    assert!(!b_near_zero.is_nan());
}

#[test]
fn test_adv_mte_hyper_thermal_boiling_extremes() {
    // Boiling water: 100 °C (373.15 K)
    let b_boiling = domain_mte(1.0, 373.15, METABOLIC_NORM, E_ANIMAL_EV);
    let b_ref = domain_mte(1.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
    assert!(b_boiling > b_ref, "Metabolism at 100°C must exceed reference 20°C");
    assert!(b_boiling.is_finite(), "Metabolism at 100°C must be finite");

    // Extreme hyper-thermal: 1,000 K and 10,000 K
    let b_1000k = domain_mte(1.0, 1000.0, METABOLIC_NORM, E_ANIMAL_EV);
    let b_10000k = domain_mte(1.0, 10000.0, METABOLIC_NORM, E_ANIMAL_EV);
    assert!(b_1000k.is_finite());
    assert!(b_10000k.is_finite());
    assert!(b_10000k >= b_1000k);

    // Asymptotic upper bound at stellar temperatures: as T -> infinity, exp(E/k * (1/T0 - 1/T)) -> exp(E/(k*T0))
    let b_stellar = domain_mte(1.0, 1e8, METABOLIC_NORM, E_ANIMAL_EV);
    let theoretical_asymptote = METABOLIC_NORM * ((E_ANIMAL_EV / BOLTZMANN_EV) * (1.0 / REF_TEMP_K)).exp();
    assert!(
        (b_stellar - theoretical_asymptote).abs() / theoretical_asymptote < 1e-4,
        "Stellar core temperature metabolism should approach theoretical asymptote: {b_stellar} vs {theoretical_asymptote}"
    );
}

#[test]
fn test_adv_mte_subnormal_and_extreme_masses() {
    // Zero mass
    assert_eq!(domain_mte(0.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV), 0.0);
    assert_eq!(metabolic_rate(0.0, 20.0, E_ANIMAL_EV_F32), 0.0);

    // Negative mass
    assert_eq!(domain_mte(-10.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV), 0.0);
    assert_eq!(metabolic_rate(-10.0, 20.0, E_ANIMAL_EV_F32), 0.0);

    // Subnormal / micro mass: 1e-30
    let b_subnormal = domain_mte(1e-30, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
    assert!(b_subnormal > 0.0 && b_subnormal.is_finite());
    assert!(!b_subnormal.is_nan());

    // Massive mass: 1e12
    let b_massive = domain_mte(1e12, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
    assert!(b_massive.is_finite());
    let expected_ratio = (1e12f64).powf(0.75);
    assert!(((b_massive / METABOLIC_NORM) - expected_ratio).abs() / expected_ratio < 1e-9);
}

#[test]
fn test_adv_mte_mass_specific_monotonic_decay_across_decades() {
    // Across 12 orders of magnitude (from 1e-6 to 1e6 mass units)
    let mut prev_specific = f64::INFINITY;
    for decade in -6..=6 {
        let mass = 10.0f64.powi(decade);
        let b = domain_mte(mass, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
        let specific = b / mass; // M^(-0.25)
        assert!(
            specific < prev_specific,
            "Mass-specific metabolic rate must strictly monotonically decrease with mass: decade {decade}"
        );
        prev_specific = specific;
    }
}

#[test]
fn test_adv_mte_kleiber_scaling_ratio_precision() {
    // Kleiber 3/4 power law: Doubling mass always multiplies basal rate by 2^0.75
    let expected_doubling_factor = 2.0f64.powf(KLEIBER_EXP);
    for &m in &[0.001, 0.1, 1.0, 5.0, 50.0, 1000.0, 1e6] {
        let b1 = domain_mte(m, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
        let b2 = domain_mte(m * 2.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
        let ratio = b2 / b1;
        assert!(
            (ratio - expected_doubling_factor).abs() < 1e-8,
            "Kleiber doubling ratio at mass {m} drifted: {ratio} vs {expected_doubling_factor}"
        );
    }
}

// ================================================================================================
// 2. Holling Functional Responses under Degenerate, Infinite, and Boundary Conditions
// ================================================================================================

#[test]
fn test_adv_holling_zero_and_negative_prey_density() {
    // Zero prey
    assert_eq!(domain_holling2(0.0, 0.1, 0.2), 0.0);
    assert_eq!(domain_holling3(0.0, 0.1, 0.2, 0.0), 0.0);
    assert_eq!(domain_holling3(0.0, 0.1, 0.2, 0.5), 0.0);
    assert_eq!(holling_type2(0.0, 0.1, 0.2), 0.0);
    assert_eq!(holling_type3(0.0, 0.1, 0.2), 0.0);

    // Negative prey density
    assert_eq!(domain_holling2(-5.0, 0.1, 0.2), 0.0);
    assert_eq!(domain_holling3(-5.0, 0.1, 0.2, 0.2), 0.0);
    assert_eq!(holling_type2(-5.0, 0.1, 0.2), 0.0);
    assert_eq!(holling_type3(-5.0, 0.1, 0.2), 0.0);
}

#[test]
fn test_adv_holling_infinite_and_hyper_dense_prey_asymptote() {
    let attack = 0.05f64;
    let handling = 0.25f64;
    let asymptote = 1.0 / handling; // 4.0

    for &n in &[1e8, 1e12, 1e30, 1e100] {
        let h2 = domain_holling2(n, attack, handling);
        assert!(
            (h2 - asymptote).abs() < 1e-4,
            "Holling II at N={n} did not converge to 1/h: {h2} vs {asymptote}"
        );
        let h3 = domain_holling3(n, attack, handling, 0.1);
        assert!(
            (h3 - asymptote).abs() < 1e-4,
            "Holling III at N={n} did not converge to 1/h: {h3} vs {asymptote}"
        );
    }
}

#[test]
fn test_adv_holling_handling_time_extremes() {
    let attack = 0.5f64;
    let n = 10.0f64;

    // Zero handling time: pure linear Lotka-Volterra Type I
    let h2_zero_h = domain_holling2(n, attack, 0.0);
    assert_eq!(h2_zero_h, attack * n, "Zero handling time must yield linear Lotka-Volterra response");

    // Huge handling time: 1e9
    let h2_huge_h = domain_holling2(n, attack, 1e9);
    assert!(h2_huge_h > 0.0 && h2_huge_h <= 1e-9);

    // Infinite handling time: returns 0.0
    let h2_inf_h = domain_holling2(n, attack, f64::INFINITY);
    assert_eq!(h2_inf_h, 0.0);
}

#[test]
fn test_adv_holling_type3_rarity_refuge_monotonicity() {
    let attack = 0.02f64;
    let handling = 0.02f64;
    let rare_n = 0.1f64;

    // Increasing refuge parameter epsilon must monotonically decrease predation pressure on rare prey
    let mut prev_rate = f64::INFINITY;
    for &refuge in &[0.0, 0.05, 0.1, 0.2, 0.5, 1.0, 5.0] {
        let rate = domain_holling3(rare_n, attack, handling, refuge);
        assert!(
            rate <= prev_rate,
            "Refuge parameter {refuge} did not monotonically reduce predation: {rate} > {prev_rate}"
        );
        prev_rate = rate;
    }
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
                ledger.transfer(&mut pool, Compartment::Animals, Compartment::Detritus, debited, EnergyEvent::MetabolicDecay);
            }
        }

        // Verify that neither compartment went negative beyond numerical tolerance
        assert!(pool.detritus >= -RESIDUAL_ABS_TOLERANCE_EU, "Detritus was negative: {}", pool.detritus);
        assert!(pool.plants >= -RESIDUAL_ABS_TOLERANCE_EU, "Plants was negative: {}", pool.plants);
        assert!(pool.animals >= -RESIDUAL_ABS_TOLERANCE_EU, "Animals was negative: {}", pool.animals);

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

    let corrupted_ledger: CausalLedger = serde_json::from_str(json_corrupted).expect("parses corrupted ledger");

    // Self-parent on e1: trace must terminate immediately without spinning forever
    let trace1 = corrupted_ledger.trace_to_root(1);
    assert_eq!(trace1, vec![1], "Self-parent cycle must terminate at the node");

    // Forward parent on e0: trace must terminate immediately
    let trace0 = corrupted_ledger.trace_to_root(0);
    assert_eq!(trace0, vec![0], "Forward-parent link must terminate at the node");
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
// 5. Parameter Sweeps with Degenerate Parameter Spaces & Pearson Correlation
// ================================================================================================

#[test]
fn test_adv_sweep_zero_range_dimensions() {
    // 0-range continuous dimension: min = 10.0, max = 10.0
    let dim_cont = DimensionSpec::Continuous {
        min: 10.0,
        max: 10.0,
    };
    dim_cont.validate("cont_deg").expect("valid degenerate bound");
    assert_eq!(dim_cont.map_normalized(0.0), 10.0);
    assert_eq!(dim_cont.map_normalized(0.5), 10.0);
    assert_eq!(dim_cont.map_normalized(0.999), 10.0);
    assert_eq!(dim_cont.grid_values(5), vec![10.0]);

    // 0-range logarithmic dimension: min = 5.0, max = 5.0
    let dim_log = DimensionSpec::Logarithmic {
        min: 5.0,
        max: 5.0,
    };
    dim_log.validate("log_deg").expect("valid degenerate bound");
    assert_eq!(dim_log.map_normalized(0.0), 5.0);
    assert_eq!(dim_log.map_normalized(0.5), 5.0);
    assert_eq!(dim_log.grid_values(5), vec![5.0]);

    // 0-range integer range: min = 42, max = 42, step = 1
    let dim_int = DimensionSpec::IntegerRange {
        min: 42,
        max: 42,
        step: 1,
    };
    dim_int.validate("int_deg").expect("valid integer degenerate bound");
    assert_eq!(dim_int.map_normalized(0.5), 42.0);
    assert_eq!(dim_int.grid_values(5), vec![42.0]);

    // Single-value discrete
    let dim_disc = DimensionSpec::Discrete {
        values: vec![123.45],
    };
    dim_disc.validate("disc_deg").expect("valid discrete degenerate bound");
    assert_eq!(dim_disc.map_normalized(0.0), 123.45);
    assert_eq!(dim_disc.map_normalized(0.99), 123.45);
    assert_eq!(dim_disc.grid_values(5), vec![123.45]);
}

#[test]
fn test_adv_sweep_single_point_sobol_and_lhs() {
    let registry = ObservableRegistry::reference_default();
    let base_manifest: ExperimentManifest = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/experiments/baseline-no-exotic.json")
            .expect("fixture"),
    )
    .expect("parses");

    // 1. Single-point Sobol sweep
    let sobol_manifest = SweepManifest {
        schema_version: 1,
        sweep_id: "sobol_single".into(),
        name: "Sobol Single Point".into(),
        base_manifest: base_manifest.clone(),
        parameters: vec![ParameterDim {
            name: "initial_plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous {
                min: 100.0,
                max: 500.0,
            },
        }],
        sampling: SamplingStrategy::Sobol {
            num_samples: 1,
            offset: 0,
        },
        max_threads: Some(1),
    };
    sobol_manifest.validate(&registry).expect("valid manifest");
    let (names, matrix) = sobol_manifest.generate_sample_matrix().expect("generates matrix");
    assert_eq!(names, vec!["initial_plants"]);
    assert_eq!(matrix.len(), 1);
    assert_eq!(matrix[0].len(), 1);
    assert!(matrix[0][0] >= 100.0 && matrix[0][0] <= 500.0);

    // 2. Single-point Latin Hypercube sweep
    let lhs_manifest = SweepManifest {
        schema_version: 1,
        sweep_id: "lhs_single".into(),
        name: "LHS Single Point".into(),
        base_manifest,
        parameters: vec![ParameterDim {
            name: "initial_plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous {
                min: 100.0,
                max: 500.0,
            },
        }],
        sampling: SamplingStrategy::LatinHypercube {
            num_samples: 1,
            seed: Some(12345),
            centered: true,
        },
        max_threads: Some(1),
    };
    lhs_manifest.validate(&registry).expect("valid manifest");
    let (_, lhs_matrix) = lhs_manifest.generate_sample_matrix().expect("generates matrix");
    assert_eq!(lhs_matrix.len(), 1);
    assert_eq!(lhs_matrix[0].len(), 1);
    // Centered 1-sample LHS evaluates at midpoint u = 0.5: 100 + 0.5 * 400 = 300
    assert!((lhs_matrix[0][0] - 300.0).abs() < 1e-6);
}

#[test]
fn test_adv_sweep_32_dimensional_sobol_boundary() {
    let mut sobol = SobolSequence::new(32);
    for _ in 0..1024 {
        let pt = sobol.next_point();
        assert_eq!(pt.len(), 32);
        for (dim, &u) in pt.iter().enumerate() {
            assert!(
                u >= 0.0 && u < 1.0,
                "Sobol point out of [0, 1) in dim {dim}: {u}"
            );
            assert!(u.is_finite());
        }
    }
}

#[test]
fn test_adv_pearson_correlation_zero_variance_and_degeneracies() {
    // 1. Length mismatch
    assert_eq!(pearson_correlation(&[1.0, 2.0], &[1.0]), None);

    // 2. Single element
    assert_eq!(pearson_correlation(&[1.0], &[1.0]), None);

    // 3. Constant series (zero variance)
    let const_x = vec![5.0; 100];
    let y = (0..100).map(|i| i as f64).collect::<Vec<_>>();
    assert_eq!(pearson_correlation(&const_x, &y), None);
    assert_eq!(pearson_correlation(&y, &const_x), None);

    // 4. Perfect positive correlation (r = 1.0)
    let x = (0..50).map(|i| i as f64).collect::<Vec<_>>();
    let y_pos = (0..50).map(|i| 2.0 * (i as f64) + 3.0).collect::<Vec<_>>();
    let r_pos = pearson_correlation(&x, &y_pos).unwrap();
    assert!((r_pos - 1.0).abs() < 1e-9);

    // 5. Perfect negative correlation (r = -1.0)
    let y_neg = (0..50).map(|i| -5.0 * (i as f64) + 10.0).collect::<Vec<_>>();
    let r_neg = pearson_correlation(&x, &y_neg).unwrap();
    assert!((r_neg - (-1.0)).abs() < 1e-9);
}

// ================================================================================================
// 6. State Snapshot Serialization, Subnormals, Precision Roundtrips & Error Gates
// ================================================================================================

#[test]
fn test_adv_snapshot_extreme_float_precision_and_subnormals() {
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();

    // Set high-precision and subnormal float values
    state.eco_animals = 990.5102615356445;
    state.eco_detritus = 1e-300; // Subnormal float
    state.eco_plants = 123456789.987654321;
    state.season_phase = 3.141592653589793;

    let envelope = SnapshotEnvelope::seal(state.clone()).expect("seals extreme float state");
    let bytes = serde_json::to_vec(&envelope).expect("serializes envelope");
    let restored = from_bytes(&bytes).expect("restores extreme float state");

    // Exact bit-for-bit float roundtrips
    assert_eq!(restored.eco_animals, 990.5102615356445);
    assert_eq!(restored.eco_detritus, 1e-300);
    assert_eq!(restored.eco_plants, 123456789.987654321);
    assert_eq!(restored.season_phase, 3.141592653589793f32);
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

// ================================================================================================
// 7. Causal Branch Tree Topology Validation & Dynamic Time Warping (DTW)
// ================================================================================================

#[test]
fn test_adv_dtw_boundary_and_degenerate_series() {
    // 1. Empty series
    assert_eq!(dynamic_time_warping(&[], &[]), 0.0);
    assert_eq!(dynamic_time_warping(&[1.0, 2.0], &[]), 0.0);
    assert_eq!(dynamic_time_warping(&[], &[1.0, 2.0]), 0.0);

    // 2. Identical series: DTW distance = 0.0
    let s = vec![1.0, 4.0, 9.0, 16.0, 25.0];
    assert_eq!(dynamic_time_warping(&s, &s), 0.0);

    // 3. Constant series offset: s1 = [2.0, 2.0, 2.0], s2 = [5.0, 5.0, 5.0]
    // Every pairwise cost is (2 - 5)^2 = 9. Over N=3 steps, cumulative cost = 3 * 9 = 27, DTW = sqrt(27)
    let c1 = vec![2.0, 2.0, 2.0];
    let c2 = vec![5.0, 5.0, 5.0];
    let dtw_const = dynamic_time_warping(&c1, &c2);
    let expected = (3.0f64 * 9.0).sqrt();
    assert!((dtw_const - expected).abs() < 1e-9);
}

#[test]
fn test_adv_branch_tree_invalid_topology_rejection() {
    let registry = ObservableRegistry::reference_default();
    let base_manifest: ExperimentManifest = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/experiments/baseline-no-exotic.json")
            .expect("fixture"),
    )
    .expect("parses");

    // 1. Multiple roots
    let multi_root_manifest = BranchTreeManifest {
        schema_version: 1,
        tree_id: "multi_root".into(),
        name: "Multi Root Tree".into(),
        base_manifest: base_manifest.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 1000,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root 1".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: None, // Second root!
                fork_tick: 0,
                duration_ticks: 1000,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(matches!(
        multi_root_manifest.validate(&registry),
        Err(ExperimentError::InvalidLaw { .. })
    ));

    // 2. Cyclic branch tree topology (Node 1 parent is 2, Node 2 parent is 1)
    let cyclic_manifest = BranchTreeManifest {
        schema_version: 1,
        tree_id: "cyclic_tree".into(),
        name: "Cyclic Tree".into(),
        base_manifest: base_manifest.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 1000,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(2), // Cycle!
                fork_tick: 500,
                duration_ticks: 1000,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 1".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1), // Cycle!
                fork_tick: 500,
                duration_ticks: 1000,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(matches!(
        cyclic_manifest.validate(&registry),
        Err(ExperimentError::InvalidLaw { .. })
    ));

    // 3. Fork tick >= duration ticks
    let invalid_fork_manifest = BranchTreeManifest {
        schema_version: 1,
        tree_id: "invalid_fork".into(),
        name: "Invalid Fork".into(),
        base_manifest,
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 500,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 600, // Exceeds parent duration!
                duration_ticks: 800,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(matches!(
        invalid_fork_manifest.validate(&registry),
        Err(ExperimentError::OutOfRange { .. })
    ));
}
