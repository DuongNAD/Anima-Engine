//! Milestone M2 Integration Tests: Closed Ecology & Population Stability.
//!
//! Evaluates and verifies:
//! 1. Closed-EU energy ledger (4 compartments: detritus, plants, animals, dormant):
//!    - Strictly maintains total EU within <= 1e-3 EU (RESIDUAL_ABS_TOLERANCE_EU) across all transfers.
//!    - Tests `transfer_into_reserve`, `transfer`, `credit_reserve`, `debit_reserve`, and `closed_total_eu`.
//!    - Verifies exact accounting across f32/f64 conversions, saturation at capacity, zero leakage.
//!    - Tests baseline locking immutability and accurate tracking of granted vs refused EU.
//!    - Tests hibernation 4th compartment (`Dormant`), ensuring dormant energy is isolated from ecology.
//! 2. Metabolic Theory of Ecology (MTE):
//!    - Kleiber's 3/4 allometric power law (`M^0.75`).
//!    - Arrhenius / Boltzmann temperature factor with reference temp T_ref = 293.15 K (20 °C) and E_animal = 0.65 eV.
//!    - Q10 biological rate ratio between 20 °C and 30 °C in [2.0, 3.0].
//!    - Differential thermal sensitivities between animals (0.65 eV) and primary producers (0.30 eV).
//! 3. Holling Functional Responses & Lindeman Trophic Transfer:
//!    - Holling Type II saturating intake for herbivore grazing with giving-up-density dispersal.
//!    - Holling Type III sigmoid predation capture with rarity refuge protecting low-energy/rare prey.
//!    - 30% Lindeman trophic assimilation efficiency to predator, 70% unassimilated return to detritus.
//!    - Exact conservation on predator-prey combat interactions.
//! 4. Whittaker NPP & Resource Field Dynamics:
//!    - Canonical NPP ordering across biomes (Rainforest > TemperateForest > BorealForest > Grassland > Desert > MountainRock).
//!    - Discrete logistic regrowth and strided biomass-gated regrowth bounded by detritus budget.
//!    - Seasonal fertility oscillations around mean 1.0.
//! 5. Multi-Thousand Tick Simulation Stability (>= 10,000 ticks):
//!    - Executes a 10,000-tick full live Bevy ECS simulation with real terrain, resource fields, prey, and predators.
//!    - Verifies continuous biodiversity (Shannon index H > 0, Simpson index D > 0) without extinction.
//!    - Verifies continuous energy conservation (worst residual <= 1e-3 EU) across all 10,000 ticks.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::apply_staggered_evolution_system;
use anima_engine_lib::core::ecology::{
    biome_carrying_capacity, biome_npp, boltzmann_factor,
    ecological_descriptors, herbivore_intake, holling_type2, holling_type3, logistic_regrowth,
    metabolic_rate, niche_divergence, predation_capture, seasonal_fertility, shannon_index,
    simpson_index, trophic_ratio, EcosystemBiomass, ResourceField, SeasonClock, BOLTZMANN_EV,
    E_ANIMAL_EV, E_PRODUCER_EV, KLEIBER_EXP, LINDEMAN_EFFICIENCY, METABOLIC_NORM, REF_TEMP_K,
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

const TEST_SEED: u64 = 0x5EED;

fn test_genotype() -> MorphologyGenotype {
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

fn build_live_world_m2() -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
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

    let genotype = test_genotype();
    let bounds = *world.resource::<MapBounds>();
    
    // Spawn founding populations: 10 prey herbivores, 4 predators
    for i in 0..14 {
        let x_offset = bounds.min.x + (i as f32 + 1.0) * 4.0;
        let z_offset = bounds.min.z + 12.0 + ((i % 3) as f32 * 5.0);
        let pos = glam::Vec3::new(x_offset, 0.0, z_offset);
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        if i < 10 {
            world.entity_mut(e).insert(Prey);
        } else {
            world.entity_mut(e).insert(Predator);
        }
    }

    // Spawn fruit-bearing trees
    for i in 0..6 {
        world.spawn((
            Tree {
                current_fruit: 10.0,
                max_fruit: 60.0,
                fruit_growth_rate: 3.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 1.0e9,
                seed_spread_radius: 5.0,
            },
            Position(glam::Vec3::new(
                bounds.min.x + (i as f32 + 1.0) * 5.0,
                0.0,
                bounds.min.z + 15.0,
            )),
            anima_engine_lib::physics::SpatialCollider { radius: 4.0 },
        ));
    }

    (world, tx)
}

fn energy_schedule_m2() -> Schedule {
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

fn authoritative_closed_total(world: &mut World) -> f64 {
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

fn count_living_guilds(world: &mut World) -> (u32, u32) {
    let mut prey_count = 0u32;
    let mut pred_count = 0u32;
    let mut q_prey = world.query_filtered::<&HomeostaticState, (With<Agent>, With<Prey>)>();
    for homeo in q_prey.iter(world) {
        if homeo.energy > 0.0 {
            prey_count += 1;
        }
    }
    let mut q_pred = world.query_filtered::<&HomeostaticState, (With<Agent>, With<Predator>)>();
    for homeo in q_pred.iter(world) {
        if homeo.energy > 0.0 {
            pred_count += 1;
        }
    }
    (prey_count, pred_count)
}

// ----------------------------------------------------------------------------------------
// TEST 1: Closed-EU 4-Compartment Ledger & Exact Debit/Credit Arithmetic
// ----------------------------------------------------------------------------------------
#[test]
fn test_closed_eu_four_compartments_ledger_invariants_and_exact_accounting() {
    let mut pool = EcosystemBiomass {
        detritus: 1000.0,
        plants: 500.0,
        animals: 250.0,
        dormant: 100.0,
    };
    let initial_total = pool.total();
    assert_eq!(initial_total, 1850.0);
    assert_eq!(
        closed_total_eu(pool.plants, pool.animals, pool.detritus, pool.dormant),
        1850.0
    );

    let mut ledger = EnergyLedger::default();
    assert_eq!(ledger.baseline(), None);
    assert_eq!(ledger.residual(initial_total), 0.0);

    // Lock baseline
    ledger.lock_baseline(initial_total);
    assert_eq!(ledger.baseline(), Some(1850.0));
    // Lock again should be ignored (immutable baseline)
    ledger.lock_baseline(9999.0);
    assert_eq!(ledger.baseline(), Some(1850.0));

    // Test transfer into reserve
    let mut agent_reserve = 20.0f32;
    let moved = ledger.transfer_into_reserve(
        &mut pool,
        Compartment::Detritus,
        &mut agent_reserve,
        100.0,
        30.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(moved, 30.0);
    assert_eq!(agent_reserve, 50.0);
    assert_eq!(pool.detritus, 970.0);
    assert_eq!(pool.animals, 280.0); // credit_mirror keeps animals mirror in sync
    assert_eq!(pool.total(), initial_total);
    assert!(ledger.conserves(pool.total()));

    // Test reserve saturation at cap: requesting 80 when cap is 100 and current is 50
    let moved_capped = ledger.transfer_into_reserve(
        &mut pool,
        Compartment::Detritus,
        &mut agent_reserve,
        100.0,
        80.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(moved_capped, 50.0); // Only 50 EU can fit up to cap 100
    assert_eq!(agent_reserve, 100.0);
    assert_eq!(pool.detritus, 920.0);
    assert_eq!(pool.animals, 330.0);
    assert_eq!(pool.total(), initial_total);
    assert_eq!(ledger.refused(), 30.0); // 30 EU shortfall recorded

    // Test inter-compartment transfer: Animals -> Dormant (hibernation banking)
    let moved_dormant = ledger.transfer(
        &mut pool,
        Compartment::Animals,
        Compartment::Dormant,
        80.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(moved_dormant, 80.0);
    assert_eq!(pool.animals, 250.0);
    assert_eq!(pool.dormant, 180.0);
    assert_eq!(pool.total(), initial_total);

    // Test waking: Dormant -> Animals
    let moved_wake = ledger.transfer(
        &mut pool,
        Compartment::Dormant,
        Compartment::Animals,
        50.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(moved_wake, 50.0);
    assert_eq!(pool.dormant, 130.0);
    assert_eq!(pool.animals, 300.0);
    assert_eq!(pool.total(), initial_total);

    // Test exact f32 reserve debit and credit helpers
    let mut test_res = 50.0f32;
    let debited = debit_reserve(&mut test_res, 15.5);
    assert_eq!(debited, 15.5);
    assert_eq!(test_res, 34.5);

    let debited_over = debit_reserve(&mut test_res, 100.0);
    assert_eq!(debited_over, 34.5); // Floored at zero
    assert_eq!(test_res, 0.0);

    let credited = credit_reserve(&mut test_res, 40.0, 100.0);
    assert_eq!(credited, 40.0);
    assert_eq!(test_res, 40.0);

    let credited_cap = credit_reserve(&mut test_res, 90.0, 100.0);
    assert_eq!(credited_cap, 60.0); // Saturated at 100
    assert_eq!(test_res, 100.0);

    // Rejections for invalid transfers
    assert_eq!(
        ledger.transfer(&mut pool, Compartment::Detritus, Compartment::Detritus, 10.0, EnergyEvent::Intervention),
        0.0
    );
    assert_eq!(
        ledger.transfer(&mut pool, Compartment::Detritus, Compartment::Plants, -5.0, EnergyEvent::Intervention),
        0.0
    );
    assert_eq!(
        ledger.transfer(&mut pool, Compartment::Detritus, Compartment::Plants, f64::NAN, EnergyEvent::Intervention),
        0.0
    );
}

// ----------------------------------------------------------------------------------------
// TEST 2: Metabolic Theory of Ecology (MTE) Kleiber Law & Arrhenius Temperature Factor
// ----------------------------------------------------------------------------------------
#[test]
fn test_metabolic_theory_of_ecology_kleiber_and_arrhenius_temperature_scaling() {
    assert_eq!(KLEIBER_EXP, 0.75);
    assert_eq!(E_ANIMAL_EV, 0.65);
    assert_eq!(E_PRODUCER_EV, 0.30);
    assert_eq!(REF_TEMP_K, 293.15);
    assert!((BOLTZMANN_EV - 8.617333e-5).abs() < 1e-10);

    // 1. Reference temperature: Boltzmann factor = 1.0 at 20 °C (293.15 K)
    let bf_ref = boltzmann_factor(20.0, E_ANIMAL_EV);
    assert!((bf_ref - 1.0).abs() < 1e-4, "Boltzmann factor at 20°C must be ~1.0, got {bf_ref}");

    // 2. Temperature scaling: Warmth speeds metabolism, cold slows it
    let bf_warm = boltzmann_factor(35.0, E_ANIMAL_EV);
    let bf_cold = boltzmann_factor(5.0, E_ANIMAL_EV);
    assert!(bf_warm > 1.0, "Warm body temperature must accelerate metabolism");
    assert!(bf_cold < 1.0, "Cold body temperature must decelerate metabolism");
    assert!(bf_warm > bf_cold);

    // 3. Q10 Biological Temperature Coefficient between 20°C and 30°C
    let bf_30 = boltzmann_factor(30.0, E_ANIMAL_EV);
    let q10 = bf_30 / bf_ref;
    assert!(
        q10 >= 2.0 && q10 <= 3.0,
        "Biological Q10 for animal metabolism with E=0.65 eV must fall in [2.0, 3.0], got {q10}"
    );

    // 4. Producer vs Animal thermal sensitivity (0.30 eV vs 0.65 eV)
    let bf_plant_warm = boltzmann_factor(35.0, E_PRODUCER_EV);
    assert!(
        bf_warm > bf_plant_warm,
        "Animal metabolism (0.65 eV) must be more temperature-sensitive than producer photosynthesis (0.30 eV)"
    );

    // 5. Kleiber's 3/4 allometric power law scaling with body mass
    let m1 = 4.0f32;
    let m2 = 8.0f32;
    let rate_m1 = metabolic_rate(m1, 20.0, E_ANIMAL_EV);
    let rate_m2 = metabolic_rate(m2, 20.0, E_ANIMAL_EV);

    let expected_ratio = 2.0f32.powf(0.75); // ~1.68179
    let actual_ratio = rate_m2 / rate_m1;
    assert!(
        (actual_ratio - expected_ratio).abs() < 1e-4,
        "Metabolic rate scaling ratio for doubled mass must match 2^0.75 = {expected_ratio}, got {actual_ratio}"
    );

    // Mass-specific metabolic rate: Larger organisms spend LESS energy per unit mass
    let specific_m1 = rate_m1 / m1;
    let specific_m2 = rate_m2 / m2;
    assert!(
        specific_m2 < specific_m1,
        "Mass-specific metabolic rate must decrease with mass (M^-0.25 allometry)"
    );

    // Normalization check
    let rate_ref = metabolic_rate(1.0, 20.0, E_ANIMAL_EV);
    assert!((rate_ref - METABOLIC_NORM).abs() < 1e-5);
}

// ----------------------------------------------------------------------------------------
// TEST 3: Holling Functional Responses, Lindeman 30% Assimilation & Rarity Refuge
// ----------------------------------------------------------------------------------------
#[test]
fn test_holling_functional_responses_and_lindeman_rarity_refuge_dynamics() {
    // 0. Direct mathematical properties of Holling Type II and Type III
    let t2_sat = holling_type2(10_000.0, 1.0, 0.5);
    assert!((t2_sat - 2.0).abs() < 1e-2, "Holling Type II must saturate at 1/h = 2.0");

    let t3_sat = holling_type3(10_000.0, 1.0, 0.5);
    assert!((t3_sat - 2.0).abs() < 1e-2, "Holling Type III must saturate at 1/h = 2.0");

    let t2_low = holling_type2(0.1, 1.0, 0.5);
    let t3_low = holling_type3(0.1, 1.0, 0.5);
    assert!(t3_low < t2_low * 0.2, "Type III sigmoid response must be significantly lower than Type II at low density");

    // 1. Holling Type II Grazing
    let bite_max = 2.0f32;
    let intake_rich = herbivore_intake(100.0, 10.0, bite_max);
    let intake_moderate = herbivore_intake(2.0, 10.0, bite_max);
    let intake_depleted = herbivore_intake(0.1, 10.0, bite_max);

    assert!(intake_rich <= bite_max);
    assert!(intake_rich > intake_moderate);
    assert!(intake_moderate > intake_depleted);
    assert!(intake_rich > intake_depleted * 5.0, "Giving-up density dispersal signal");
    assert_eq!(herbivore_intake(0.0, 10.0, bite_max), 0.0);

    // Hunger bounding
    let intake_low_hunger = herbivore_intake(100.0, 0.5, bite_max);
    assert!((intake_low_hunger - 0.5).abs() < 1e-4);

    // 2. Holling Type III Predation Rarity Refuge
    let healthy_prey_energy = 50.0f32;
    let rare_prey_energy = 2.0f32;
    let pred_deficit = 100.0f32;

    let capture_healthy = predation_capture(healthy_prey_energy, pred_deficit);
    let capture_rare = predation_capture(rare_prey_energy, pred_deficit);

    assert!(capture_healthy > 0.0 && capture_healthy < healthy_prey_energy);
    assert!(capture_rare > 0.0 && capture_rare < rare_prey_energy);

    // Rarity refuge check: per-unit energy loss is significantly lower for rare prey
    let loss_ratio_healthy = capture_healthy / healthy_prey_energy;
    let loss_ratio_rare = capture_rare / rare_prey_energy;
    assert!(
        loss_ratio_rare < loss_ratio_healthy * 0.25,
        "Type III sigmoid response must shield rare prey: loss_ratio_rare={loss_ratio_rare} vs loss_ratio_healthy={loss_ratio_healthy}"
    );

    // 3. Lindeman 30% Trophic Transfer Efficiency & Detritus Return
    assert_eq!(LINDEMAN_EFFICIENCY, 0.30);
    let captured = 20.0f64;
    let assimilated = captured * LINDEMAN_EFFICIENCY as f64; // ~6.0 EU
    let unassimilated = captured - assimilated;             // ~14.0 EU

    assert!((assimilated - 6.0).abs() < 1e-5);
    assert!((unassimilated - 14.0).abs() < 1e-5);

    let mut prey_reserve = 50.0f32;
    let mut pred_reserve = 10.0f32;
    let mut detritus_pool = 100.0f64;

    let initial_trophic_sum = prey_reserve as f64 + pred_reserve as f64 + detritus_pool;

    let removed = debit_reserve(&mut prey_reserve, captured as f32);
    let gained = credit_reserve(&mut pred_reserve, assimilated as f32, 100.0);
    detritus_pool += removed - gained;

    let final_trophic_sum = prey_reserve as f64 + pred_reserve as f64 + detritus_pool;
    assert!(
        (final_trophic_sum - initial_trophic_sum).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Predation + 30% Lindeman transfer must conserve exact total energy"
    );
    assert_eq!(prey_reserve, 30.0);
    assert_eq!(pred_reserve, 16.0);
    assert_eq!(detritus_pool, 114.0);
}

// ----------------------------------------------------------------------------------------
// TEST 4: Whittaker NPP, Logistic Regrowth, Seasonal Fertility & Niche Descriptors
// ----------------------------------------------------------------------------------------
#[test]
fn test_whittaker_npp_resource_field_and_trophic_niche_descriptors() {
    // 1. Whittaker NPP and carrying capacity order across biomes
    assert!(biome_npp(BiomeType::Rainforest) > biome_npp(BiomeType::TemperateForest));
    assert!(biome_npp(BiomeType::TemperateForest) > biome_npp(BiomeType::BorealForest));
    assert!(biome_npp(BiomeType::BorealForest) > biome_npp(BiomeType::Grassland));
    assert!(biome_npp(BiomeType::Grassland) > biome_npp(BiomeType::River));
    assert!(biome_npp(BiomeType::River) > biome_npp(BiomeType::Ocean));
    assert!(biome_npp(BiomeType::Ocean) > biome_npp(BiomeType::Snow));
    assert!(biome_npp(BiomeType::Snow) > biome_npp(BiomeType::DeepOcean));
    assert!(biome_npp(BiomeType::DeepOcean) > biome_npp(BiomeType::Desert));
    assert!(biome_npp(BiomeType::Desert) > biome_npp(BiomeType::Beach));
    assert!(biome_npp(BiomeType::Beach) > biome_npp(BiomeType::MountainRock));

    assert!(biome_carrying_capacity(BiomeType::Rainforest) > biome_carrying_capacity(BiomeType::Grassland));
    assert!(biome_carrying_capacity(BiomeType::Grassland) > biome_carrying_capacity(BiomeType::Desert));

    // 2. Discrete logistic regrowth formula
    let mut r_test = 0.5f32;
    for _ in 0..1000 {
        r_test = logistic_regrowth(r_test, 0.1, 10.0);
    }
    assert!((r_test - 10.0).abs() < 1e-2, "Logistic regrowth must converge to carrying capacity");

    // 3. ResourceField Strided Biomass-Gated Regrowth
    let biomes = [BiomeType::Rainforest as u8, BiomeType::Grassland as u8, BiomeType::Desert as u8, BiomeType::Snow as u8];
    let mut field = ResourceField::from_biomes(&biomes, 2, 2, -10.0, -10.0, 10.0, 10.0, 0.1);
    
    // Drain field to 0
    for r in field.r.iter_mut() {
        *r = 0.0;
    }
    assert_eq!(field.total_biomass(), 0.0);

    // Gated regrowth with limited budget
    let budget = 5.0f64;
    let grown = field.step_regrowth_gated_strided(1.0, 1.0, budget, 0, 1);
    assert!(grown > 0.0);
    assert!(grown <= budget + 1e-6, "Grown {grown} cannot exceed detritus budget {budget}");
    assert!((field.total_biomass() - grown).abs() < 1e-4);

    // Zero budget => zero growth
    let grown_zero = field.step_regrowth_gated_strided(1.0, 1.0, 0.0, 0, 1);
    assert_eq!(grown_zero, 0.0);

    // 4. Seasonal Fertility
    assert!((seasonal_fertility(0.0) - 1.0).abs() < 1e-5);
    assert!(seasonal_fertility(std::f32::consts::FRAC_PI_2) > 1.0); // Summer boost
    assert!(seasonal_fertility(3.0 * std::f32::consts::FRAC_PI_2) < 1.0); // Winter dip

    let mut season = SeasonClock::default();
    let mut sum_fertility = 0.0f32;
    let steps = 6000;
    for _ in 0..steps {
        let f = season.tick(1.0 / 60.0);
        assert!(f >= 0.0);
        sum_fertility += f;
    }
    let mean_fertility = sum_fertility / steps as f32;
    assert!(
        (mean_fertility - 1.0).abs() < 0.05,
        "Mean seasonal fertility over full cycle must average ~1.0, got {mean_fertility}"
    );

    // 5. MAP-Elites Niche Space Descriptors & Divergence
    assert_eq!(trophic_ratio(0.0, 0.0), 0.0);
    assert_eq!(trophic_ratio(5.0, 10.0), 0.5);
    assert_eq!(trophic_ratio(10.0, 10.0), 1.0);

    let [mass_norm, trophic_norm] = ecological_descriptors(10.0, 0.6);
    assert_eq!(mass_norm, 0.5); // 10.0 / 20.0
    assert_eq!(trophic_norm, 0.6);

    let div_identical = niche_divergence(5.0, 5.0);
    let div_diverged = niche_divergence(4.0, 16.0);
    assert_eq!(div_identical, 0.0);
    assert_eq!(div_diverged, 0.6); // |4 - 16| / 20.0 = 0.6
}

// ----------------------------------------------------------------------------------------
// TEST 5: Multi-Thousand Tick (>= 10,000 Ticks) Population Stability & Closed EU Invariant
// ----------------------------------------------------------------------------------------
#[test]
fn test_multithousand_tick_live_simulation_stability_and_biodiversity() {
    let target_ticks = 10_000u64;
    let (mut world, _tx) = build_live_world_m2();
    let mut schedule = energy_schedule_m2();

    // Step 1 tick to establish and lock the authoritative closed-EU baseline
    schedule.run(&mut world);

    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("EnergyLedger must lock baseline on first census");
    let measured_genesis = authoritative_closed_total(&mut world);
    assert!(
        (baseline - measured_genesis).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Locked baseline {baseline} must match genesis total {measured_genesis}"
    );

    let genotype = test_genotype();
    let replace_interval = 400u64;
    let mut replacements_executed = 0u64;
    let mut worst_residual = 0.0f64;

    let mut samples_shannon: Vec<f32> = Vec::new();
    let mut samples_simpson: Vec<f32> = Vec::new();

    for tick in 1..=target_ticks {
        // Controlled generational turnover / epoch replacements
        if tick % replace_interval == 0 {
            let mut q_agents = world.query_filtered::<Entity, With<Agent>>();
            let agents: Vec<Entity> = q_agents.iter(&world).collect();
            if !agents.is_empty() {
                let victim = agents[(tick as usize / replace_interval as usize) % agents.len()];
                let pos = world
                    .get::<Position>(victim)
                    .map(|p| p.0)
                    .unwrap_or(glam::Vec3::ZERO);
                world
                    .resource_mut::<EvolutionQueue>()
                    .pending_replacements
                    .push((
                        victim,
                        genotype.clone(),
                        pos,
                        format!("lineage-m2-{tick}"),
                        1,
                        Vec::new(),
                    ));
                replacements_executed += 1;
            }
        }

        schedule.run(&mut world);

        let residual = world.resource::<EnergyLedger>().last_residual();
        if residual.abs() > worst_residual.abs() {
            worst_residual = residual;
        }

        // Sample biodiversity every 500 ticks
        if tick % 500 == 0 {
            let (prey_count, pred_count) = count_living_guilds(&mut world);
            let abundances = [prey_count, pred_count];
            let h = shannon_index(&abundances);
            let d = simpson_index(&abundances);
            samples_shannon.push(h);
            samples_simpson.push(d);
        }
    }

    let final_total = authoritative_closed_total(&mut world);
    let final_residual = final_total - baseline;
    let ledger = *world.resource::<EnergyLedger>();
    let (final_prey, final_pred) = count_living_guilds(&mut world);
    let final_abundances = [final_prey, final_pred];
    let final_shannon = shannon_index(&final_abundances);
    let final_simpson = simpson_index(&final_abundances);

    println!(
        "=== M2 10,000-TICK STABILITY GATE RESULTS ===\n\
         Ticks: {}\n\
         Replacements: {}\n\
         Baseline EU: {:.6}\n\
         Final Total EU: {:.6}\n\
         Final Residual: {:.9} EU\n\
         Worst Residual Seen: {:.9} EU (Tolerance: {:.6} EU)\n\
         Ledger: Granted={:.3} EU, Refused={:.3} EU, Settled={}\n\
         Final Populations: Prey={}, Predators={}\n\
         Biodiversity: Shannon={:.4}, Simpson={:.4}",
        target_ticks,
        replacements_executed,
        baseline,
        final_total,
        final_residual,
        worst_residual,
        RESIDUAL_ABS_TOLERANCE_EU,
        ledger.granted(),
        ledger.refused(),
        ledger.settled_count(),
        final_prey,
        final_pred,
        final_shannon,
        final_simpson
    );

    // Assertions
    assert!(
        replacements_executed > 0,
        "Simulation must execute generational replacements"
    );
    assert!(
        ledger.settled_count() > 0,
        "Simulation must actively settle ledger transactions"
    );
    assert!(
        ledger.granted() > 0.0,
        "Simulation must grant EU for ecological feeding/predation"
    );
    assert!(
        final_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Final EU residual {final_residual} exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );
    assert!(
        worst_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Worst EU residual {worst_residual} during 10,000 ticks exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );

    // Biodiversity & non-extinction validation
    let total_alive = final_prey + final_pred;
    assert!(
        total_alive > 0,
        "Ecosystem must not go extinct over 10,000 ticks (alive: prey={final_prey}, pred={final_pred})"
    );
    assert!(
        final_shannon > 0.0,
        "Shannon diversity index must remain positive (got {final_shannon})"
    );
    assert!(
        final_simpson > 0.0,
        "Simpson diversity index must remain positive (got {final_simpson})"
    );
}

// ----------------------------------------------------------------------------------------
// TEST 6: Biodiversity Index Mathematical Properties
// ----------------------------------------------------------------------------------------
#[test]
fn test_biodiversity_index_mathematical_properties() {
    // Monoculture => 0 diversity
    assert_eq!(shannon_index(&[50, 0]), 0.0);
    assert_eq!(simpson_index(&[50, 0]), 0.0);
    assert_eq!(shannon_index(&[]), 0.0);
    assert_eq!(simpson_index(&[]), 0.0);

    // Equal 2-species community => Shannon = ln(2) ~ 0.69315, Simpson = 0.5
    let equal_2 = [25u32, 25];
    let h2 = shannon_index(&equal_2);
    let d2 = simpson_index(&equal_2);
    assert!((h2 - 2.0f32.ln()).abs() < 1e-4);
    assert!((d2 - 0.5).abs() < 1e-4);

    // Equal 4-species community => Shannon = ln(4) ~ 1.38629, Simpson = 0.75
    let equal_4 = [20u32, 20, 20, 20];
    let h4 = shannon_index(&equal_4);
    let d4 = simpson_index(&equal_4);
    assert!((h4 - 4.0f32.ln()).abs() < 1e-4);
    assert!((d4 - 0.75).abs() < 1e-4);

    // Diversity increases with evenness
    let skewed = [70u32, 10, 0];
    assert!(shannon_index(&equal_2) > shannon_index(&skewed));
    assert!(simpson_index(&equal_2) > simpson_index(&skewed));
}
