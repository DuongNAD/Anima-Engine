//! Comprehensive Verification Suite: Metabolic Theory of Ecology (MTE) & Holling Functional Responses
//!
//! Verifies:
//! 1. MTE Kleiber 3/4 allometric mass scaling: B(M, T) = B0 * M^(3/4) * exp(Ea/k * (1/T0 - 1/T)).
//! 2. Arrhenius-Boltzmann thermal scaling: Q10 in [2.0, 3.0], hyperthermic acceleration, hypothermic deceleration.
//! 3. Holling Type II hyperbolic saturation: f_II(N) = aN / (1 + ahN), asymptotic limit 1/h, giving-up density.
//! 4. Holling Type III sigmoidal response with rarity refuge: f_III(N) = aN^2 / (1 + ahN^2 + eps), quadratic vanishing extraction.
//! 5. Exact bit-level energy conservation in predation transfers with Lindeman efficiency (eta = 0.30).

use anima_domain::laws as domain_laws;
use anima_engine_lib::core::ecology as engine_eco;
use anima_engine_lib::core::energy_ledger::{credit_reserve, debit_reserve};

#[test]
fn test_mte_kleiber_allometric_power_law() {
    // Kleiber's Law states that metabolic rate scales as M^0.75 (sublinearly).
    // Test a wide range of body masses (0.1 kg to 1000.0 kg).
    let masses = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0];
    let b_ref = domain_laws::mte_basal_metabolism(
        1.0,
        domain_laws::REF_TEMP_K,
        domain_laws::METABOLIC_NORM,
        domain_laws::E_ANIMAL_EV,
    );
    assert!((b_ref - domain_laws::METABOLIC_NORM).abs() < 1e-9);

    for &m in &masses {
        let b = domain_laws::mte_basal_metabolism(
            m,
            domain_laws::REF_TEMP_K,
            domain_laws::METABOLIC_NORM,
            domain_laws::E_ANIMAL_EV,
        );
        let expected = domain_laws::METABOLIC_NORM * m.powf(0.75);
        let rel_err = (b - expected).abs() / expected;
        assert!(
            rel_err < 1e-6,
            "MTE Kleiber scaling deviated for mass {m}: got {b}, expected {expected}"
        );

        // Mass-specific metabolic rate B(M)/M = M^(-0.25) must strictly decrease with mass
        let mass_specific = b / m;
        let expected_specific = domain_laws::METABOLIC_NORM * m.powf(-0.25);
        assert!((mass_specific - expected_specific).abs() / expected_specific < 1e-6);
    }

    // Explicit mass doubling check: 2^0.75 = 1.6817928305...
    let b_1kg = domain_laws::mte_basal_metabolism(
        1.0,
        domain_laws::REF_TEMP_K,
        domain_laws::METABOLIC_NORM,
        domain_laws::E_ANIMAL_EV,
    );
    let b_2kg = domain_laws::mte_basal_metabolism(
        2.0,
        domain_laws::REF_TEMP_K,
        domain_laws::METABOLIC_NORM,
        domain_laws::E_ANIMAL_EV,
    );
    let doubling_ratio = b_2kg / b_1kg;
    assert!(
        (doubling_ratio - 2.0f64.powf(0.75)).abs() < 1e-7,
        "Doubling mass ratio was {doubling_ratio}"
    );

    // Engine f32 metabolic_rate consistency
    let engine_b1 = engine_eco::metabolic_rate(1.0, 20.0, engine_eco::E_ANIMAL_EV);
    assert!((engine_b1 - engine_eco::METABOLIC_NORM).abs() < 1e-5);
}

#[test]
fn test_arrhenius_boltzmann_temperature_response() {
    // Reference temperature 20 °C (293.15 K) factor must be exactly 1.0
    let f_20c = domain_laws::boltzmann_factor(20.0, domain_laws::E_ANIMAL_EV);
    assert!((f_20c - 1.0).abs() < 1e-4);

    // Q10 across biological range (20 °C to 30 °C, 10 °C to 20 °C)
    let f_10c = domain_laws::boltzmann_factor(10.0, domain_laws::E_ANIMAL_EV);
    let f_30c = domain_laws::boltzmann_factor(30.0, domain_laws::E_ANIMAL_EV);
    let q10_20_30 = f_30c / f_20c;
    let q10_10_20 = f_20c / f_10c;

    assert!(
        q10_20_30 >= 2.0 && q10_20_30 <= 3.0,
        "Q10 (20->30C) was {q10_20_30}, expected in [2.0, 3.0]"
    );
    assert!(
        q10_10_20 >= 2.0 && q10_10_20 <= 3.0,
        "Q10 (10->20C) was {q10_10_20}, expected in [2.0, 3.0]"
    );

    // Thermal activation comparison: Animal metabolism (0.65 eV) is more temperature-sensitive
    // than plant photosynthesis (0.30 eV)
    let f_animal_warm = domain_laws::boltzmann_factor(35.0, domain_laws::E_ANIMAL_EV);
    let f_plant_warm = domain_laws::boltzmann_factor(35.0, domain_laws::E_PRODUCER_EV);
    assert!(
        f_animal_warm > f_plant_warm,
        "Animal activation should exceed plant activation at warm temp: animal={f_animal_warm}, plant={f_plant_warm}"
    );

    // Cold ambient deceleration (0 °C)
    let f_0c = domain_laws::boltzmann_factor(0.0, domain_laws::E_ANIMAL_EV);
    assert!(
        f_0c > 0.10 && f_0c < 0.30,
        "Cold ambient factor was {f_0c}, expected ~0.176"
    );
}

#[test]
fn test_holling_type_2_hyperbolic_saturation_and_gud() {
    let attack = 1.0;
    let handling = 0.2; // saturation ceiling = 1 / handling = 5.0
    let sat_limit = 1.0 / handling;

    // 1. Asymptotic saturation limit
    let h2_inf = domain_laws::holling_type_2(1e9, attack, handling);
    assert!(
        (h2_inf - sat_limit).abs() < 1e-4,
        "Holling II did not saturate to 1/h: got {h2_inf}, expected {sat_limit}"
    );

    // 2. Half-saturation constant: K_m = 1 / (a * h) = 5.0
    let k_m = 1.0 / (attack * handling);
    let h2_half = domain_laws::holling_type_2(k_m, attack, handling);
    assert!(
        (h2_half - sat_limit / 2.0).abs() < 1e-6,
        "Holling II at K_m did not equal half maximum: got {h2_half}, expected {}",
        sat_limit / 2.0
    );

    // 3. Herbivore intake giving-up density (GUD)
    let max_bite = 0.15f32; // per tick
    let intake_rich = engine_eco::herbivore_intake(100.0, 50.0, max_bite);
    let intake_poor = engine_eco::herbivore_intake(0.05, 50.0, max_bite);
    assert!(
        intake_rich <= max_bite + 1e-5,
        "Intake exceeded max bite ceiling"
    );
    assert!(
        intake_poor < intake_rich * 0.4,
        "Depleted patch intake should drop significantly: rich={intake_rich}, poor={intake_poor}"
    );
}

#[test]
fn test_holling_type_3_sigmoidal_rarity_refuge() {
    let attack = domain_laws::CAPTURE_ATTACK;
    let handling = domain_laws::CAPTURE_HANDLING;
    let sat_limit = 1.0 / handling; // = 50.0

    // 1. Asymptotic saturation at high density
    let h3_high = domain_laws::holling_type_3(10_000.0, attack, handling, 0.0);
    assert!(
        (h3_high - sat_limit).abs() < 1e-2,
        "Holling III failed to saturate to 50.0: got {h3_high}"
    );

    // 2. Sigmoidal inflection: per-capita intake phi(N) = f(N)/N = aN / (1 + ahN^2)
    // As N -> 0, phi(N) -> 0 (quadratic vanishing of predation)
    let low_prey = [0.1, 0.5, 1.0, 2.0, 5.0];
    let high_prey = [25.0, 50.0, 100.0];

    for &n in &low_prey {
        let captured = domain_laws::holling_type_3(n, attack, handling, 0.0);
        let per_capita = captured / n;
        // At low N, captured is tiny (< 0.10 * N)
        assert!(
            per_capita < 0.10,
            "Low-prey extraction not protected: n={n}, captured={captured}, per_capita={per_capita}"
        );
    }

    for &n in &high_prey {
        let captured = domain_laws::holling_type_3(n, attack, handling, 0.0);
        assert!(
            captured >= 10.0 && captured <= sat_limit,
            "High-prey extraction out of bounds: n={n}, captured={captured}"
        );
    }


    // 3. Comparison with Holling II: Holling III extraction fraction is strictly smaller at low densities
    let n_rare = 0.05;
    let h2_rare_per_capita = domain_laws::holling_type_2(n_rare, attack, handling) / n_rare;
    let h3_rare_per_capita = domain_laws::holling_type_3(n_rare, attack, handling, 0.0) / n_rare;
    assert!(
        h3_rare_per_capita < h2_rare_per_capita * 0.10,
        "Holling III should provide >= 10x rarity protection over Holling II at N=0.05: h3={h3_rare_per_capita}, h2={h2_rare_per_capita}"
    );


    // 4. Explicit refuge parameter epsilon
    let h3_refuged = domain_laws::holling_type_3(2.0, attack, handling, 0.10);
    let h3_standard = domain_laws::holling_type_3(2.0, attack, handling, 0.0);
    assert!(
        h3_refuged < h3_standard,
        "Refuge parameter did not reduce capture: refuged={h3_refuged}, standard={h3_standard}"
    );
}

#[test]
fn test_predation_lindeman_energy_conservation() {
    // Verify that every predation strike preserves 100% of energy:
    // prey loss = predator gain (Lindeman eta) + detritus return (1 - eta)
    let mut prey_reserve = 50.0f32;
    let mut pred_reserve = 20.0f32;
    let pred_cap = 150.0f32;
    let needed = pred_cap - pred_reserve;

    let captured = engine_eco::predation_capture(prey_reserve, needed);
    assert!(captured > 0.0 && captured < prey_reserve);

    let assimilated = captured * engine_eco::LINDEMAN_EFFICIENCY;
    let removed = debit_reserve(&mut prey_reserve, captured);
    let gained = credit_reserve(&mut pred_reserve, assimilated, pred_cap);

    let mut pool = engine_eco::EcosystemBiomass {
        detritus: 100.0,
        plants: 1000.0,
        animals: (prey_reserve + pred_reserve) as f64,
    };
    let total_before = pool.detritus + pool.plants + (50.0 + 20.0);

    // Detritus gets the exact unassimilated difference
    let detritus_credit = removed - gained;
    pool.detritus += detritus_credit;
    let total_after = pool.detritus + pool.plants + (prey_reserve as f64 + pred_reserve as f64);

    let delta = (total_after - total_before).abs();
    assert!(
        delta < 1e-12,
        "Predation transaction drifted energy: delta = {delta:.15} EU"
    );
}
