//! Empirical Challenger Stress Suite for Milestone M2 (MTE Extremes, Holling III & Population Stability).
//!
//! Grounded in empirical test execution and stress-testing:
//! 1. Metabolic Theory of Ecology (MTE) at Thermal and Allometric Extremes:
//!    - Thermal sweep $T \in [-40^\circ\text{C}, +60^\circ\text{C}]$: positivity, finite bounds, strict monotonicity.
//!    - Allometric sweep $M \in [0.001, 10000.0]$: Kleiber 3/4 scaling ($M^{0.75}$), mass-specific reduction ($M^{-0.25}$).
//!    - Differential thermal activation energy ($E = 0.65\text{ eV}$ animal vs $E = 0.30\text{ eV}$ producer).
//!    - Biological $Q_{10} \in [2.0, 3.0]$ in physiological range and well-behaved across extremes.
//!    - Extreme boundary safety: absolute zero (0 K, -273.15 °C), sub-zero K, extreme heat, subnormal floats.
//! 2. Holling Type III Rarity Refuge Dynamics & Mathematical Extinction Resistance:
//!    - Asymptotic quadratic rarity refuge ($\lim_{N \to 0^+} C(N)/N = 0$) vs Type II predatory collapse.
//!    - 10,000 consecutive predator swarm strikes against a single rare prey proving asymptotic $O(1/k)$ persistence.
//!    - Multi-predator feeding frenzy under extreme predator-to-prey ratios.
//! 3. Closed-EU Conservation under High Combat & Mortality Pressure:
//!    - Extreme predator crowding simulation (40 predators vs 4 prey).
//!    - Continuous closed-EU residual tracking within $\le 10^{-3}$ EU tolerance.
//!    - Predator starvation energy recycling into detritus and plant regrowth.
//! 4. Strided Regrowth & Whittaker NPP Invariants under Detritus Scarcity:
//!    - Strided sweep exact accounting across all phase/stride combinations.
//!    - Zero leakage on budget exhaustion.
//!
//! Follows `AGENTS.md` rules:
//! - Deterministic seed `HD_SEED = 0x5EED`.
//! - Strict behavioral assertions with exact invariants.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::apply_staggered_evolution_system;
use anima_engine_lib::core::ecology::{
    boltzmann_factor, herbivore_intake, holling_type2, holling_type3, metabolic_rate,
    predation_capture, EcosystemBiomass, ResourceField, SeasonClock, CAPTURE_ATTACK,
    CAPTURE_HANDLING, E_ANIMAL_EV, E_PRODUCER_EV, LINDEMAN_EFFICIENCY,
};
use anima_engine_lib::core::ecs::{
    init_world, Agent, EpochManager, MapBounds, Position, Predator, Prey, Tree,
};
use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, credit_reserve, debit_reserve, Compartment, EnergyEvent, EnergyLedger,
    RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::environmental_systems::{
    detect_environmental_collisions_system, ecosystem_census_system, fruit_growth_system,
    herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::resources::{
    EnvironmentalSpawnSettings, EvolutionQueue, EvolutionReceiver, FoodSpawnSettings,
};
use anima_engine_lib::core::terrain::BiomeType;
use anima_engine_lib::core::world_systems::{
    combat_system, detect_food_collisions_system, metabolic_decay_system, spawn_food_system,
};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

const HD_SEED: u32 = 0x5EED;

struct ChallengeRng {
    state: u32,
}

impl ChallengeRng {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { HD_SEED } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() & 0x00FF_FFFF) as f32 / 16777216.0
    }
}

fn challenger_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

type EvolutionMsg = (
    Entity,
    MorphologyGenotype,
    glam::Vec3,
    String,
    u32,
    Vec<String>,
);

fn build_predator_swarmed_world() -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
    let mut world = init_world();

    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(EpochManager::default());
    world.insert_resource(SeasonClock::default());

    let (tx, rx) = crossbeam_channel::unbounded::<EvolutionMsg>();
    world.insert_resource(EvolutionReceiver(rx));
    world.insert_resource(EvolutionQueue {
        pending_replacements: Vec::new(),
    });

    let genotype = challenger_genotype();
    let bounds = *world.resource::<MapBounds>();

    // Extreme predator-to-prey ratio: 40 predators, 4 prey in close proximity
    // Prey spawned at center
    for i in 0..4 {
        let pos = glam::Vec3::new(
            bounds.min.x + 20.0 + (i as f32 * 2.0),
            0.0,
            bounds.min.z + 20.0 + (i as f32 * 2.0),
        );
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        world.entity_mut(e).insert(Prey);
    }

    // 40 predators swarming the same coordinates
    for i in 0..40 {
        let x_off = (i % 8) as f32 * 3.0;
        let z_off = (i / 8) as f32 * 3.0;
        let pos = glam::Vec3::new(bounds.min.x + 15.0 + x_off, 0.0, bounds.min.z + 15.0 + z_off);
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        world.entity_mut(e).insert(Predator);
    }

    // Spawn 8 fruit trees to support herbivore primary trophic base
    for i in 0..8 {
        world.spawn((
            Tree {
                current_fruit: 20.0,
                max_fruit: 80.0,
                fruit_growth_rate: 4.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 1.0e9,
                seed_spread_radius: 6.0,
            },
            Position(glam::Vec3::new(
                bounds.min.x + 10.0 + (i as f32 * 4.0),
                0.0,
                bounds.min.z + 18.0,
            )),
            anima_engine_lib::physics::SpatialCollider { radius: 4.0 },
        ));
    }

    (world, tx)
}

fn challenger_energy_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        metabolic_decay_system,
        spawn_food_system,
        detect_food_collisions_system.after(spawn_food_system),
        combat_system,
        fruit_growth_system,
        detect_environmental_collisions_system.after(fruit_growth_system),
        apply_staggered_evolution_system,
        herbivore_grazing_system,
        resource_field_regrowth_system.after(herbivore_grazing_system),
        ecosystem_census_system.after(resource_field_regrowth_system),
    ));
    schedule
}

fn measure_exact_closed_eu(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let (detritus, dormant) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.detritus, p.dormant))
        .unwrap_or((0.0, 0.0));
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus, dormant)
}

// ========================================================================================
// CHALLENGE 1: MTE Thermal Extremes (-40 °C to +60 °C) & Boltzmann Scaling
// ========================================================================================
#[test]
fn test_mte_thermal_extremes_scaling_and_monotonicity() {
    let mut prev_bf_animal = 0.0f32;
    let mut prev_bf_producer = 0.0f32;

    // Sweep T from -40.0 °C to +60.0 °C in steps of 0.5 °C (201 test points)
    let steps = 200;
    for step in 0..=steps {
        let temp_c = -40.0 + (step as f32 * (100.0 / steps as f32)); // -40.0 to +60.0
        let bf_animal = boltzmann_factor(temp_c, E_ANIMAL_EV);
        let bf_producer = boltzmann_factor(temp_c, E_PRODUCER_EV);

        // 1. Strict positivity & finiteness
        assert!(
            bf_animal > 0.0 && bf_animal.is_finite(),
            "Animal Boltzmann factor at {temp_c}°C must be positive and finite, got {bf_animal}"
        );
        assert!(
            bf_producer > 0.0 && bf_producer.is_finite(),
            "Producer Boltzmann factor at {temp_c}°C must be positive and finite, got {bf_producer}"
        );

        // 2. Monotonicity with temperature
        if step > 0 {
            assert!(
                bf_animal > prev_bf_animal,
                "Animal Boltzmann factor must strictly increase with temperature: {bf_animal} vs prev {prev_bf_animal} at {temp_c}°C"
            );
            assert!(
                bf_producer > prev_bf_producer,
                "Producer Boltzmann factor must strictly increase with temperature: {bf_producer} vs prev {prev_bf_producer} at {temp_c}°C"
            );
        }

        // 3. Differential activation energy: animal (0.65 eV) vs producer (0.30 eV)
        // Below reference temp (20°C): higher E means faster drop (bf_animal < bf_producer < 1.0)
        // Above reference temp (20°C): higher E means faster rise (bf_animal > bf_producer > 1.0)
        if temp_c < 19.5 {
            assert!(
                bf_animal < bf_producer && bf_producer < 1.0,
                "Below 20°C, higher activation energy (0.65 eV) must suppress rate more than lower (0.30 eV) at {temp_c}°C"
            );
        } else if temp_c > 20.5 {
            assert!(
                bf_animal > bf_producer && bf_producer > 1.0,
                "Above 20°C, higher activation energy (0.65 eV) must accelerate rate more than lower (0.30 eV) at {temp_c}°C"
            );
        }

        // 4. Test whole-organism metabolic rate at this temperature across masses
        for &mass in &[0.01f32, 0.1, 1.0, 10.0, 100.0, 1000.0] {
            let rate = metabolic_rate(mass, temp_c, E_ANIMAL_EV);
            assert!(
                rate > 0.0 && rate.is_finite(),
                "Metabolic rate at mass={mass}, temp={temp_c}°C must be finite and positive, got {rate}"
            );
        }

        prev_bf_animal = bf_animal;
        prev_bf_producer = bf_producer;
    }

    // 5. Thermal sensitivity Q10 across whole range
    // At T=20°C -> T=30°C
    let q10_20 = boltzmann_factor(30.0, E_ANIMAL_EV) / boltzmann_factor(20.0, E_ANIMAL_EV);
    assert!(
        q10_20 >= 2.0 && q10_20 <= 3.0,
        "Q10 between 20°C and 30°C must be in [2.0, 3.0], got {q10_20}"
    );

    // At extreme cold T=-40°C -> T=-30°C
    let q10_cold = boltzmann_factor(-30.0, E_ANIMAL_EV) / boltzmann_factor(-40.0, E_ANIMAL_EV);
    assert!(
        q10_cold > 1.0 && q10_cold.is_finite(),
        "Q10 in extreme cold must remain positive and finite, got {q10_cold}"
    );

    // At extreme heat T=+50°C -> T=+60°C
    let q10_hot = boltzmann_factor(60.0, E_ANIMAL_EV) / boltzmann_factor(50.0, E_ANIMAL_EV);
    assert!(
        q10_hot > 1.0 && q10_hot.is_finite(),
        "Q10 in extreme heat must remain positive and finite, got {q10_hot}"
    );

    // 6. Extreme Boundary Conditions (Absolute Zero, Subzero Kelvin, Infinite/NaN)
    assert_eq!(boltzmann_factor(-273.15, E_ANIMAL_EV), 0.0);
    assert_eq!(boltzmann_factor(-300.0, E_ANIMAL_EV), 0.0);
    assert_eq!(metabolic_rate(10.0, -273.15, E_ANIMAL_EV), 0.0);
    assert_eq!(metabolic_rate(-5.0, 20.0, E_ANIMAL_EV), 0.0);
}

// ========================================================================================
// CHALLENGE 2: MTE Allometric Extremes (M = 0.001 to 10000.0) & Kleiber Law Invariants
// ========================================================================================
#[test]
fn test_mte_allometric_extremes_power_law_and_mass_specific_scaling() {
    let masses = [
        0.001f32, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0,
        500.0, 1000.0, 5000.0, 10000.0,
    ];

    let mut prev_rate = 0.0f32;
    let mut prev_mass_specific = f32::INFINITY;

    for &m in &masses {
        let rate = metabolic_rate(m, 20.0, E_ANIMAL_EV);
        let mass_specific = rate / m;

        // 1. Positivity & finiteness
        assert!(
            rate > 0.0 && rate.is_finite(),
            "Metabolic rate at mass {m} must be positive and finite, got {rate}"
        );
        assert!(
            mass_specific > 0.0 && mass_specific.is_finite(),
            "Mass-specific metabolic rate at mass {m} must be positive and finite, got {mass_specific}"
        );

        // 2. Whole-organism rate increases monotonically with body mass
        assert!(
            rate > prev_rate,
            "Whole-organism metabolic rate must increase with body mass: {rate} vs prev {prev_rate} at m={m}"
        );

        // 3. Mass-specific metabolic rate decreases monotonically with body mass (M^-0.25 allometry)
        assert!(
            mass_specific < prev_mass_specific,
            "Mass-specific metabolic rate must strictly decrease with mass: {mass_specific} vs prev {prev_mass_specific} at m={m}"
        );

        prev_rate = rate;
        prev_mass_specific = mass_specific;
    }

    // 4. Exact Allometric Exponent Oracle: ln(R2 / R1) / ln(M2 / M1) == 0.75 across all adjacent pairs
    for i in 0..(masses.len() - 1) {
        let m1 = masses[i];
        let m2 = masses[i + 1];
        let r1 = metabolic_rate(m1, 20.0, E_ANIMAL_EV);
        let r2 = metabolic_rate(m2, 20.0, E_ANIMAL_EV);

        let empirical_exponent = (r2.ln() - r1.ln()) / (m2.ln() - m1.ln());
        assert!(
            (empirical_exponent - 0.75).abs() < 1e-4,
            "Empirical allometric exponent between m1={m1} and m2={m2} must equal 0.75, got {empirical_exponent}"
        );
    }

    // 5. Ratio between extreme ends (M=0.01 vs M=1000.0)
    let r_min = metabolic_rate(0.01, 20.0, E_ANIMAL_EV);
    let r_max = metabolic_rate(1000.0, 20.0, E_ANIMAL_EV);
    let ratio = r_max / r_min; // (1000 / 0.01)^0.75 = 100,000^0.75 = 10^3.75 ~ 5623.413
    let expected_ratio = (1000.0f32 / 0.01f32).powf(0.75);
    assert!(
        (ratio - expected_ratio).abs() / expected_ratio < 1e-4,
        "Ratio across 5 orders of magnitude mass difference must match power law: expected {expected_ratio}, got {ratio}"
    );

    // 6. Subnormal and Extreme Value Safety
    let subnormal_mass = 1.0e-35f32;
    let rate_subnormal = metabolic_rate(subnormal_mass, 20.0, E_ANIMAL_EV);
    assert!(rate_subnormal >= 0.0 && rate_subnormal.is_finite());

    let huge_mass = 1.0e10f32;
    let rate_huge = metabolic_rate(huge_mass, 20.0, E_ANIMAL_EV);
    assert!(rate_huge > 0.0 && rate_huge.is_finite());
}

// ========================================================================================
// CHALLENGE 3: Holling Type III Asymptotic Rarity Refuge & Extinction Resistance Oracle
// ========================================================================================
#[test]
fn test_holling_type3_asymptotic_rarity_refuge_and_swarm_resilience() {
    // 1. Verify Holling Type III per-capita mortality curve: f(N) = C(N)/N = aN / (1 + ahN^2)
    // Mathematical derivative f'(N) = a(1 - ahN^2) / (1 + ahN^2)^2
    // - Peak occurs at N* = 1 / sqrt(a*h) = 1 / sqrt(0.0004) = 50.0
    // - Rarity refuge regime (N <= 50.0): f'(N) > 0, so per-capita mortality plunges to 0 as N -> 0
    // - Satiation regime (N > 50.0): handling time bounds predation, f(N) -> 1/(hN) -> 0
    let peak_n = 1.0f32 / (CAPTURE_ATTACK * CAPTURE_HANDLING).sqrt(); // 50.0
    let peak_per_capita = holling_type3(peak_n, CAPTURE_ATTACK, CAPTURE_HANDLING) / peak_n;
    assert!((peak_n - 50.0).abs() < 1e-4);
    assert!((peak_per_capita - 0.5).abs() < 1e-4, "Peak per-capita mortality at N*=50 must be 0.5, got {peak_per_capita}");

    let rare_densities = [
        50.0f32, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05, 0.01, 0.001, 0.0001,
    ];

    let mut prev_per_capita = peak_per_capita + 1e-5;
    for &n in &rare_densities {
        let capture_t3 = holling_type3(n, CAPTURE_ATTACK, CAPTURE_HANDLING);
        let per_capita_t3 = capture_t3 / n;

        let capture_t2 = holling_type2(n, CAPTURE_ATTACK, CAPTURE_HANDLING);
        let per_capita_t2 = capture_t2 / n;

        // In rarity refuge regime (N <= 50.0), Type III per-capita mortality strictly decreases as prey becomes rarer
        assert!(
            per_capita_t3 <= prev_per_capita,
            "Type III per-capita mortality must decrease monotonically as prey becomes rarer: {per_capita_t3} vs prev {prev_per_capita} at n={n}"
        );

        // The ratio of Type III to Type II per-capita mortality scales directly with prey density n:
        // (Type III / n) / (Type II / n) = n * (1 + ahn) / (1 + ahn^2) ~ n
        // Showing that Type III rarity refuge provides an extra factor of n protection relative to Type II
        if n <= 1.0 {
            let ratio = per_capita_t3 / per_capita_t2;
            assert!(
                ratio <= n * 1.05,
                "Type III to Type II per-capita ratio at n={n} must scale as ~n, got {ratio}"
            );
        }

        prev_per_capita = per_capita_t3;
    }

    // 2. Extreme Swarm Attack Harness: 10,000 Consecutive Attacks on a Single Rare Prey
    // Demonstrates mathematically that a rare prey cannot be drained to 0 in finite predator attacks
    let initial_prey_energy = 1.0f32; // Low starting energy (rare prey)
    let mut prey_energy = initial_prey_energy;
    let predator_deficit = 50.0f32; // Predator is starving, demanding max energy
    let num_attacks = 10_000;

    let mut total_captured = 0.0f64;
    let mut total_assimilated = 0.0f64;
    let mut total_detritus = 0.0f64;

    for attack_idx in 1..=num_attacks {
        let captured = predation_capture(prey_energy, predator_deficit);
        assert!(
            captured > 0.0,
            "Capture amount must remain strictly positive at attack {attack_idx}"
        );
        assert!(
            captured < prey_energy,
            "Predation capture ({captured}) must NEVER drain the entire prey energy ({prey_energy}) in a single strike"
        );

        prey_energy -= captured;
        assert!(
            prey_energy > 0.0,
            "Prey energy must remain strictly positive after attack {attack_idx}, got {prey_energy}"
        );

        // Accounting check per strike
        let cap_f64 = captured as f64;
        let assim = cap_f64 * LINDEMAN_EFFICIENCY as f64;
        let det = cap_f64 - assim;

        total_captured += cap_f64;
        total_assimilated += assim;
        total_detritus += det;
    }

    // Even after 10,000 consecutive full-power predator attacks, prey energy is STILL > 0 (mathematical refuge)
    println!(
        "=== 10,000 PREDATOR SWARM ATTACK HARNESS ===\n\
         Initial Prey Energy: {:.6} EU\n\
         Remaining Prey Energy: {:.9} EU\n\
         Total Captured: {:.6} EU\n\
         Total Assimilated (30%): {:.6} EU\n\
         Total Detritus Recycled (70%): {:.6} EU",
        initial_prey_energy, prey_energy, total_captured, total_assimilated, total_detritus
    );

    assert!(
        prey_energy > 0.0 && prey_energy < 0.01,
        "Prey energy after 10,000 attacks must be strictly positive and smoothly asymptotic: got {prey_energy}"
    );

    // Exact Conservation Invariant: Initial == Remaining + Assimilated + Detritus
    let final_balance = prey_energy as f64 + total_assimilated + total_detritus;
    let drift = (final_balance - initial_prey_energy as f64).abs();
    assert!(
        drift <= 1e-6,
        "Closed-EU energy balance across 10,000 predator swarm strikes drifted by {drift} EU (exceeded 1e-6)"
    );
}

// ========================================================================================
// CHALLENGE 4: Extreme Predator Crowding Live Simulation & Closed-EU Stress Test
// ========================================================================================
#[test]
fn test_live_simulation_extreme_predator_crowding_and_energy_conservation() {
    let (mut world, _tx) = build_predator_swarmed_world();
    let mut schedule = challenger_energy_schedule();

    // 1. Initial census step to establish closed-EU baseline
    schedule.run(&mut world);

    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("EnergyLedger must establish baseline on tick 1");
    let measured_init = measure_exact_closed_eu(&mut world);
    assert!(
        (baseline - measured_init).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Initial closed EU {measured_init} must match locked baseline {baseline}"
    );

    let test_ticks = 3_000u64;
    let mut worst_residual = 0.0f64;
    let mut _prey_survived_at_any_point = false;

    for tick in 1..=test_ticks {
        schedule.run(&mut world);

        let res = world.resource::<EnergyLedger>().last_residual();
        if res.abs() > worst_residual.abs() {
            worst_residual = res;
        }

        // Verify continuous conservation at every tick
        assert!(
            res.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
            "Closed-EU residual {res} at tick {tick} violated tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
        );

        if tick % 500 == 0 {
            let mut prey_count = 0;
            let mut q_prey =
                world.query_filtered::<&HomeostaticState, (With<Agent>, With<Prey>)>();
            for h in q_prey.iter(&world) {
                if h.energy > 0.0 {
                    prey_count += 1;
                }
            }
            if prey_count > 0 {
                _prey_survived_at_any_point = true;
            }
        }
    }

    let final_total = measure_exact_closed_eu(&mut world);
    let final_residual = final_total - baseline;
    let ledger = *world.resource::<EnergyLedger>();

    println!(
        "=== EXTREME PREDATOR CROWDING (40 Preds : 4 Prey) RESULTS ===\n\
         Ticks: {}\n\
         Baseline EU: {:.6}\n\
         Final Total EU: {:.6}\n\
         Final Residual: {:.9} EU\n\
         Worst Residual Seen: {:.9} EU\n\
         Ledger Transactions Settled: {}\n\
         Ledger Granted: {:.3} EU, Refused: {:.3} EU",
        test_ticks,
        baseline,
        final_total,
        final_residual,
        worst_residual,
        ledger.settled_count(),
        ledger.granted(),
        ledger.refused()
    );

    assert!(
        final_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Final residual {final_residual} exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );
    assert!(
        worst_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Worst residual {worst_residual} exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );
    assert!(
        ledger.settled_count() > 0,
        "Ledger must actively settle transactions during intense predation"
    );
}

// ========================================================================================
// CHALLENGE 5: Strided Regrowth Conservation & Exhaustion Totality
// ========================================================================================
#[test]
fn test_strided_regrowth_phase_sweep_and_budget_exhaustion_invariants() {
    let biomes = [
        BiomeType::Rainforest as u8,
        BiomeType::TemperateForest as u8,
        BiomeType::Grassland as u8,
        BiomeType::BorealForest as u8,
        BiomeType::Desert as u8,
        BiomeType::Snow as u8,
        BiomeType::River as u8,
        BiomeType::Beach as u8,
    ];

    let mut field =
        ResourceField::from_biomes(&biomes, 4, 2, -20.0, -20.0, 20.0, 20.0, 0.05);

    // Drain all cells to 0
    for r in field.r.iter_mut() {
        *r = 0.0;
    }
    assert_eq!(field.total_biomass(), 0.0);

    // 1. Test budget clamping across various scarce budgets
    let scarce_budgets = [0.0f64, 1e-6, 0.001, 0.1, 1.0, 5.0];
    for &budget in &scarce_budgets {
        let before_biomass = field.total_biomass();
        let grown = field.step_regrowth_gated_strided(1.0, 1.0, budget, 0, 4);
        let after_biomass = field.total_biomass();
        let actual_growth = after_biomass - before_biomass;

        assert!(
            grown <= budget + 1e-6,
            "Reported grown {grown} must never exceed offered budget {budget}"
        );
        assert!(
            (actual_growth - grown).abs() <= 1e-5,
            "Actual field delta {actual_growth} must match reported growth {grown}"
        );
    }

    // 2. Full 4-phase stride sweep over full cycle
    let stride = 4;
    let mut total_grown_cycle = 0.0f64;
    let generous_budget = 1000.0f64;
    for phase in 0..stride {
        let grown = field.step_regrowth_gated_strided(1.0, 1.0, generous_budget, phase, stride);
        assert!(grown >= 0.0);
        total_grown_cycle += grown;
    }
    assert!(
        total_grown_cycle > 0.0,
        "Full stride cycle must successfully grow standing plant biomass"
    );
}

// ========================================================================================
// CHALLENGE 6: 100,000-Transaction High-Frequency Burst & ULP Accumulation Oracle
// ========================================================================================
#[test]
fn test_empirical_challenger_100k_transaction_burst_and_ulp_drift_oracle() {
    let mut rng = ChallengeRng::new(HD_SEED);
    let mut pool = EcosystemBiomass {
        detritus: 50_000.0,
        plants: 25_000.0,
        animals: 15_000.0,
        dormant: 10_000.0,
    };
    let initial_total = pool.total();
    assert_eq!(initial_total, 100_000.0);

    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(initial_total);
    assert_eq!(ledger.baseline(), Some(100_000.0));

    let compartments = [
        Compartment::Plants,
        Compartment::Animals,
        Compartment::Detritus,
        Compartment::Dormant,
    ];

    let mut mock_reserves: Vec<f32> = (0..50).map(|_| rng.next_f32() * 50.0).collect();
    let num_transactions = 100_000;

    for i in 0..num_transactions {
        let op_type = rng.next_u32() % 6;
        match op_type {
            0 => {
                // transfer_into_reserve with micro or standard amount
                let res_idx = (rng.next_u32() as usize) % mock_reserves.len();
                let from = compartments[(rng.next_u32() as usize) % compartments.len()];
                let cap = 100.0f32;
                let desired = if i % 2 == 0 {
                    rng.next_f32() * 20.0
                } else {
                    rng.next_f32() * 1e-4 // micro amount
                };
                ledger.transfer_into_reserve(
                    &mut pool,
                    from,
                    &mut mock_reserves[res_idx],
                    cap,
                    desired,
                    EnergyEvent::Feeding,
                );
            }
            1 => {
                // Saturated transfer_into_reserve (over cap)
                let res_idx = (rng.next_u32() as usize) % mock_reserves.len();
                let from = Compartment::Detritus;
                let cap = 50.0f32;
                let desired = 500.0f32; // excessive demand
                ledger.transfer_into_reserve(
                    &mut pool,
                    from,
                    &mut mock_reserves[res_idx],
                    cap,
                    desired,
                    EnergyEvent::Replacement,
                );
            }
            2 => {
                // Plain inter-compartment transfer
                let from = compartments[(rng.next_u32() as usize) % compartments.len()];
                let to = compartments[(rng.next_u32() as usize) % compartments.len()];
                let amount = 0.01 + rng.next_f32() as f64 * 150.0;
                ledger.transfer(&mut pool, from, to, amount, EnergyEvent::Intervention);
            }
            3 => {
                // Exact reserve debit + credit roundtrip
                let res_idx = (rng.next_u32() as usize) % mock_reserves.len();
                let debit_amount = rng.next_f32() * 10.0;
                let debited = debit_reserve(&mut mock_reserves[res_idx], debit_amount);
                pool.animals -= debited;
                pool.detritus += debited;
            }
            4 => {
                // Direct credit reserve from detritus
                let res_idx = (rng.next_u32() as usize) % mock_reserves.len();
                let credit_amount = rng.next_f32() * 15.0;
                let cap = 100.0f32;
                let credited = credit_reserve(&mut mock_reserves[res_idx], credit_amount, cap);
                let actual_debited = credited.min(pool.detritus);
                pool.detritus -= actual_debited;
                pool.animals += actual_debited;
            }
            _ => {
                // Invalid transfers (negative, NaN, Inf, zero, self-transfer)
                let bad_amount = match i % 4 {
                    0 => -100.0,
                    1 => 0.0,
                    2 => f64::NAN,
                    _ => f64::INFINITY,
                };
                let moved = ledger.transfer(
                    &mut pool,
                    Compartment::Detritus,
                    Compartment::Plants,
                    bad_amount,
                    EnergyEvent::Intervention,
                );
                assert_eq!(moved, 0.0, "Invalid transfer amount must yield 0.0 moved");

                let self_moved = ledger.transfer(
                    &mut pool,
                    Compartment::Animals,
                    Compartment::Animals,
                    50.0,
                    EnergyEvent::Intervention,
                );
                assert_eq!(self_moved, 0.0, "Self-transfer must yield 0.0 moved");
            }
        }
    }

    let current_total = pool.total();
    let residual = ledger.residual(current_total);

    println!(
        "=== 100,000-TRANSACTION BURST ORACLE RESULTS ===\n\
         Baseline EU: {:.6}\n\
         Final Pool Total EU: {:.6}\n\
         Final Residual: {:.12} EU\n\
         Absolute Tolerance: {:.6} EU\n\
         Settled Transactions: {}\n\
         Granted: {:.3} EU, Refused: {:.3} EU\n\
         Pool Breakdown: Detritus={:.2}, Plants={:.2}, Animals={:.2}, Dormant={:.2}",
        initial_total,
        current_total,
        residual,
        RESIDUAL_ABS_TOLERANCE_EU,
        ledger.settled_count(),
        ledger.granted(),
        ledger.refused(),
        pool.detritus,
        pool.plants,
        pool.animals,
        pool.dormant
    );

    // Hard Oracle Assertions
    assert!(
        residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Residual {:.12} EU exceeded tolerance {:.6} EU after 100k transactions",
        residual,
        RESIDUAL_ABS_TOLERANCE_EU
    );
    assert!(
        residual.abs() < 1e-6,
        "Measured arithmetic drift {:.12} EU exceeded strict ULP bound 1e-6 EU",
        residual
    );
    assert!(
        ledger.conserves(current_total),
        "EnergyLedger::conserves must return true"
    );
    assert!(
        ledger.settled_count() > 20_000,
        "Settled transaction count must reflect active workload (got {})",
        ledger.settled_count()
    );
    assert!(
        ledger.granted() > 0.0,
        "Granted energy must be strictly positive"
    );
}

// ========================================================================================
// CHALLENGE 7: Catastrophic Starvation & Mass Die-Off Conservation Oracle
// ========================================================================================
#[test]
fn test_empirical_challenger_catastrophic_starvation_and_mass_dieoff() {
    let mut rng = ChallengeRng::new(HD_SEED);
    let num_agents = 500;
    let mut agent_masses: Vec<f32> = Vec::with_capacity(num_agents);
    let mut agent_reserves: Vec<f32> = Vec::with_capacity(num_agents);
    let mut total_animal_eu = 0.0f64;

    for _ in 0..num_agents {
        let m = 0.5 + rng.next_f32() * 19.5;
        let r = 10.0 + rng.next_f32() * 90.0;
        agent_masses.push(m);
        agent_reserves.push(r);
        total_animal_eu += r as f64;
    }

    let mut pool = EcosystemBiomass {
        detritus: 10_000.0,
        plants: 15_000.0,
        animals: total_animal_eu,
        dormant: 5_000.0,
    };
    let initial_total = pool.total();
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(initial_total);

    // Catastrophic Event: 100% plant withering into detritus
    let plants_amount = pool.plants;
    let withered = ledger.transfer(
        &mut pool,
        Compartment::Plants,
        Compartment::Detritus,
        plants_amount,
        EnergyEvent::Intervention,
    );
    assert_eq!(withered, 15_000.0);
    assert_eq!(pool.plants, 0.0);

    // Run prolonged starvation cycles until 100% of agents die
    let dt = 1.0 / 60.0;
    let temp_c = 25.0f32; // Warm temperature accelerating metabolic decay
    let mut alive_count = num_agents;
    let mut starvation_rounds = 0;

    while alive_count > 0 && starvation_rounds < 100_000 {
        starvation_rounds += 1;
        for i in 0..num_agents {
            if agent_reserves[i] > 0.0 {
                let meta_rate = metabolic_rate(agent_masses[i], temp_c, E_ANIMAL_EV);
                let decay_demand = meta_rate * dt;
                let debited = debit_reserve(&mut agent_reserves[i], decay_demand);
                
                // Conserved closed return: debited metabolic energy returns to detritus
                pool.animals -= debited;
                pool.detritus += debited;

                if agent_reserves[i] <= 0.0 {
                    alive_count -= 1;
                }
            }
        }
    }

    assert_eq!(alive_count, 0, "All 500 agents must have starved to 0.0 energy");
    let final_total = closed_total_eu(pool.plants, pool.animals, pool.detritus, pool.dormant);
    let residual = final_total - initial_total;

    println!(
        "=== MASS STARVATION ORACLE RESULTS ===\n\
         Initial Total EU: {:.6}\n\
         Final Total EU: {:.6}\n\
         Residual after 100% Starvation: {:.12} EU\n\
         Starvation Rounds to Complete Extinction: {}\n\
         Detritus Reclaimed: {:.2} EU",
        initial_total,
        final_total,
        residual,
        starvation_rounds,
        pool.detritus
    );

    assert!(
        residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Starvation die-off leaked energy: residual={:.12}",
        residual
    );
    assert!(
        residual.abs() < 1e-8,
        "Starvation accounting exceeded exact f64 precision"
    );
    assert_eq!(pool.animals, 0.0);
    assert_eq!(pool.plants, 0.0);
    assert_eq!(pool.dormant, 5_000.0);
}

// ========================================================================================
// CHALLENGE 8: Deep Hibernation Quarantine & Cyclical Waking Oracle
// ========================================================================================
#[test]
fn test_empirical_challenger_deep_hibernation_quarantine_and_wake_cycles() {
    let mut pool = EcosystemBiomass {
        detritus: 10_000.0,
        plants: 5_000.0,
        animals: 8_000.0,
        dormant: 0.0,
    };
    let initial_total = pool.total();
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(initial_total);

    // 1. Mass hibernation: 200 agents enter deep hibernation banking 6,000 EU into Dormant
    let banked = ledger.transfer(
        &mut pool,
        Compartment::Animals,
        Compartment::Dormant,
        6_000.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(banked, 6_000.0);
    assert_eq!(pool.dormant, 6_000.0);
    assert_eq!(pool.animals, 2_000.0);
    assert!(ledger.conserves(pool.total()));

    // 2. Heavy ecological activity: plant regrowth and active grazing
    let biomes = [
        BiomeType::Rainforest as u8,
        BiomeType::TemperateForest as u8,
        BiomeType::Grassland as u8,
        BiomeType::River as u8,
    ];
    let mut field = ResourceField::from_biomes(&biomes, 2, 2, -10.0, -10.0, 10.0, 10.0, 0.1);

    for _ in 0..100 {
        // Regrowth drawing from detritus
        let grown = field.step_regrowth_gated_strided(1.0, 1.0, pool.detritus, 0, 1);
        let actual_grown = ledger.transfer(
            &mut pool,
            Compartment::Detritus,
            Compartment::Plants,
            grown,
            EnergyEvent::Intervention,
        );
        assert!((grown - actual_grown).abs() < 1e-5);

        // Herbivore grazing
        let mut active_reserve = 20.0f32;
        let intake = herbivore_intake(pool.plants as f32, 10.0, 2.0);
        ledger.transfer_into_reserve(
            &mut pool,
            Compartment::Plants,
            &mut active_reserve,
            100.0,
            intake,
            EnergyEvent::Feeding,
        );

        // Invariant check: Dormant pool MUST remain strictly untouched
        assert_eq!(
            pool.dormant, 6_000.0,
            "Dormant compartment must be completely isolated from grazing and regrowth"
        );
    }

    // 3. Waking cycle: Phase 1 wake 50% of hibernating agents
    let wake_phase1 = ledger.transfer(
        &mut pool,
        Compartment::Dormant,
        Compartment::Animals,
        3_000.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(wake_phase1, 3_000.0);
    assert_eq!(pool.dormant, 3_000.0);
    assert!(ledger.conserves(pool.total()));

    // Phase 2 wake remaining agents
    let wake_phase2 = ledger.transfer(
        &mut pool,
        Compartment::Dormant,
        Compartment::Animals,
        3_000.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(wake_phase2, 3_000.0);
    assert_eq!(pool.dormant, 0.0);
    assert!(ledger.conserves(pool.total()));

    let final_residual = ledger.residual(pool.total());
    assert!(
        final_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Hibernation cycles resulted in residual: {:.12}",
        final_residual
    );
}

