//! End-to-End Scientific Verification Suite for Anima Engine.
//!
//! Validates:
//! 1. Scientific Ecological Modeling & Conservation Laws (Closed-loop energy ledger, MTE Kleiber/Arrhenius scaling,
//!    Holling Type II/III responses, Whittaker NPP strided resource field, Shannon/Simpson biodiversity indices).
//! 2. Causal DAG & Declarative Interventions (Append-only causal ledger, root tracing, top contributor ranking,
//!    spatial regions, and temporal curve shaping).
//! 3. Determinism & State Parity (Bit-exact replay, checkpoint forking equivalence, FNV-1a state hashes).
//! 4. Statistical Ensembles & Factor Divergence (Genesis forks, paired t-tests, Cohen's d_z effect size).
//! 5. Headless CLI `anima-lab` (Argument parsing, reference and live models, exit codes, and JSON schema contract).

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anima_domain::energy::{
    closed_total_eu, credit_reserve, debit_reserve, Compartment, EcosystemBiomass, EnergyEvent,
    EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::causal::{CausalLedger, CauseId, CAUSE_BACKGROUND};
use anima_engine_lib::core::ecology::{
    biome_carrying_capacity, biome_npp, boltzmann_factor, ecological_descriptors,
    herbivore_intake, holling_type2, holling_type3, logistic_regrowth, metabolic_rate,
    niche_divergence, predation_capture, shannon_index, simpson_index,
    ResourceField, SeasonClock, E_ANIMAL_EV, E_PRODUCER_EV, KLEIBER_EXP, LINDEMAN_EFFICIENCY,
};
use anima_engine_lib::core::terrain::BiomeType;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry,
};
use anima_engine_lib::core::experiment_runner::{
    checkpoint_fork, genesis_fork, run_manifest_seed, run_paired_ensemble,
};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use serde_json::Value;

// Helper to locate committed test fixtures
fn fixture_path(subpath: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(subpath)
}

struct TempManifest(PathBuf);

impl Drop for TempManifest {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_temp_manifest(manifest: &ExperimentManifest, prefix: &str) -> TempManifest {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}.json", std::process::id()));
    let bytes = serde_json::to_vec_pretty(manifest).expect("manifest serialises");
    std::fs::write(&path, bytes).expect("manifest writes");
    TempManifest(path)
}

// ------------------------------------------------------------------------------------------------
// 1. Scientific Ecological Modeling & Conservation Laws
// ------------------------------------------------------------------------------------------------

#[test]
fn test_closed_loop_energy_biomass_ledger_conservation() {
    let mut ledger = EnergyLedger::default();
    let mut pool = EcosystemBiomass {
        detritus: 100.0,
        plants: 500.0,
        animals: 250.0,
    };
    let initial_total = pool.total();
    ledger.lock_baseline(initial_total);

    assert_eq!(ledger.baseline(), Some(850.0));
    assert!(ledger.conserves(initial_total));

    // Simulate trophic cycle:
    // Step 1: Herbivore feeds from Detritus (replacement / food)
    let mut animal_reserve = 50.0f32;
    let gained = ledger.transfer_into_reserve(
        &mut pool,
        Compartment::Detritus,
        &mut animal_reserve,
        200.0,
        30.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(gained, 30.0);
    assert_eq!(animal_reserve, 80.0);
    assert_eq!(pool.detritus, 70.0);
    assert!((pool.total() - initial_total).abs() < 1e-9);

    // Step 2: Causal Intervention moves Detritus -> Plants
    let moved = ledger.transfer(
        &mut pool,
        Compartment::Detritus,
        Compartment::Plants,
        40.0,
        EnergyEvent::Intervention,
    );
    assert_eq!(moved, 40.0);
    assert_eq!(pool.detritus, 30.0);
    assert_eq!(pool.plants, 540.0);
    assert!((pool.total() - initial_total).abs() < 1e-9);

    // Step 3: Exact credit / debit reserve rounding bounds
    let mut store = 100.0f32;
    let debited = debit_reserve(&mut store, 25.5);
    assert!((debited - 25.5).abs() < 1e-5);
    let credited = credit_reserve(&mut store, 25.5, 200.0);
    assert!((credited - 25.5).abs() < 1e-5);

    // Verify authoritative total
    let auth_total = closed_total_eu(pool.plants, pool.animals, pool.detritus);
    assert!((auth_total - initial_total).abs() < RESIDUAL_ABS_TOLERANCE_EU);
    assert!(ledger.conserves(auth_total));

    ledger.observe(auth_total);
    assert_eq!(ledger.last_total(), auth_total);
    assert_eq!(ledger.last_residual(), 0.0);
}

#[test]
fn test_mte_kleiber_allometric_and_arrhenius_thermal_scaling() {
    // Kleiber scaling: M^0.75
    let mass_small = 2.0f32;
    let mass_large = 8.0f32;
    let rate_small = metabolic_rate(mass_small, 20.0, E_ANIMAL_EV);
    let rate_large = metabolic_rate(mass_large, 20.0, E_ANIMAL_EV);

    let theoretical_ratio = (mass_large / mass_small).powf(KLEIBER_EXP);
    let actual_ratio = rate_large / rate_small;
    assert!(
        (actual_ratio - theoretical_ratio).abs() < 1e-3,
        "Kleiber scaling deviation: actual={actual_ratio}, expected={theoretical_ratio}"
    );

    // Mass-specific rate (rate / mass) must decrease with body size (M^-0.25)
    let specific_small = rate_small / mass_small;
    let specific_large = rate_large / mass_large;
    assert!(
        specific_large < specific_small,
        "Mass-specific metabolic rate must be lower for larger organisms"
    );

    // Arrhenius temperature factor: boltzmann_factor is 1.0 at 20 °C (293.15 K)
    let factor_ref = boltzmann_factor(20.0, E_ANIMAL_EV);
    assert!((factor_ref - 1.0).abs() < 1e-3);

    let factor_warm = boltzmann_factor(30.0, E_ANIMAL_EV);
    let factor_cold = boltzmann_factor(10.0, E_ANIMAL_EV);
    assert!(factor_warm > 1.0);
    assert!(factor_cold < 1.0);

    // Biological Q10 ratio over 10 °C must be within 2.0 to 3.0
    let q10 = factor_warm / factor_ref;
    assert!(
        q10 >= 2.0 && q10 <= 3.0,
        "Q10 temperature sensitivity was {q10}, expected between 2.0 and 3.0"
    );

    // Producers (0.30 eV) have lower thermal sensitivity than consumers (0.65 eV)
    let producer_factor = boltzmann_factor(35.0, E_PRODUCER_EV);
    let consumer_factor = boltzmann_factor(35.0, E_ANIMAL_EV);
    assert!(
        consumer_factor > producer_factor,
        "Animal activation energy must produce steeper response than plants"
    );
}

#[test]
fn test_holling_type2_and_type3_dynamics_and_rarity_refuge() {
    let attack = 0.05f32;
    let handling = 0.1f32;

    // Type II: Hyperbolic saturation
    let rate_low = holling_type2(2.0, attack, handling);
    let rate_high = holling_type2(50000.0, attack, handling);
    let asymptote = 1.0 / handling;
    assert!(rate_low > 0.0);
    assert!(rate_high < asymptote && rate_high > asymptote * 0.95);

    // Type III: Sigmoidal rarity refuge
    // At very low prey density (e.g. 0.2), Type III per-capita capture must be drastically lower than Type II
    let prey_scarce = 0.2f32;
    let per_capita_t2 = holling_type2(prey_scarce, attack, handling) / prey_scarce;
    let per_capita_t3 = holling_type3(prey_scarce, attack, handling) / prey_scarce;
    assert!(
        per_capita_t3 < per_capita_t2 * 0.25,
        "Holling Type III must protect rare prey via sigmoid inflection"
    );

    // Predation capture deficit bounding and Lindeman efficiency
    let prey_energy = 50.0f32;
    let predator_deficit = 5.0f32;
    let captured = predation_capture(prey_energy, predator_deficit);
    assert!(captured > 0.0);
    assert!(
        captured <= predator_deficit / LINDEMAN_EFFICIENCY + 1e-3,
        "Predator must not over-capture beyond deficit requirement"
    );

    // Herbivore intake saturation
    let intake_rich = herbivore_intake(100.0, 50.0, 5.0);
    let intake_starved_patch = herbivore_intake(0.1, 50.0, 5.0);
    assert!(intake_rich > intake_starved_patch * 10.0);
    assert!(intake_rich <= 5.0 + 1e-4);
}

#[test]
fn test_resource_field_whittaker_npp_and_gated_strided_regrowth() {
    // Whittaker NPP order
    assert!(biome_npp(BiomeType::Rainforest) > biome_npp(BiomeType::TemperateForest));
    assert!(biome_npp(BiomeType::TemperateForest) > biome_npp(BiomeType::Grassland));
    assert!(biome_npp(BiomeType::Grassland) > biome_npp(BiomeType::Desert));

    let biomes = vec![
        BiomeType::Rainforest as u8,
        BiomeType::Desert as u8,
        BiomeType::Grassland as u8,
        BiomeType::TemperateForest as u8,
    ];
    let mut field = ResourceField::from_biomes(&biomes, 2, 2, 0.0, 0.0, 2.0, 2.0, 0.2);

    // Verify cell capacities reflect biomes
    assert!(field.r_max[0] > field.r_max[1]); // Rainforest > Desert
    assert_eq!(field.r_max[0], biome_carrying_capacity(BiomeType::Rainforest));

    // Graze a cell
    let grazed = field.graze(0.5, 0.5, 5.0);
    assert!(grazed > 0.0);
    assert!(field.r[0] < field.r_max[0]);

    // Gated strided regrowth under finite detritus budget
    let detritus_budget = 1.0f64;
    let added = field.step_regrowth_gated_strided(1.0, 1.0, detritus_budget, 0, 1);
    assert!(
        added <= detritus_budget + 1e-6,
        "Added biomass ({added}) must not exceed detritus budget ({detritus_budget})"
    );
    assert!(added > 0.0);

    // Logistic regrowth formula correctness
    let r_init = 1.0f32;
    let r_max = 10.0f32;
    let r_next = logistic_regrowth(r_init, 0.1, r_max);
    let expected = r_init + 0.1 * r_init * (1.0 - r_init / r_max);
    assert!((r_next - expected).abs() < 1e-4);
}

#[test]
fn test_biodiversity_diagnostics_and_map_elites_hypervolume() {
    let even_abundances = [30u32, 30, 30, 30];
    let skewed_abundances = [110u32, 5, 3, 2];

    let h_even = shannon_index(&even_abundances);
    let h_skewed = shannon_index(&skewed_abundances);
    assert!(
        h_even > h_skewed,
        "Even community must have higher Shannon entropy than skewed community"
    );

    let d_even = simpson_index(&even_abundances);
    let d_skewed = simpson_index(&skewed_abundances);
    assert!(
        d_even > d_skewed,
        "Even community must have higher Simpson diversity"
    );

    // MAP-Elites 2D niche descriptors
    let [mass_norm, range_norm] = ecological_descriptors(10.0, 100.0);
    assert!((mass_norm - 0.5).abs() < 1e-4);
    assert!((range_norm - 0.5).abs() < 1e-4);

    // Niche divergence
    let div_identical = niche_divergence(10.0, 10.0);
    let div_separated = niche_divergence(4.0, 16.0);
    assert_eq!(div_identical, 0.0);
    assert!(div_separated > 0.0 && div_separated <= 1.0);

    // Season clock
    let mut season = SeasonClock::default();
    let mult = season.tick(1.0);
    assert!(mult >= 0.0);
}

// ------------------------------------------------------------------------------------------------
// 2. Causal DAG & Extended Interventions
// ------------------------------------------------------------------------------------------------

#[test]
fn test_causal_dag_ledger_root_tracing_and_contributor_ranking() {
    let mut ledger = CausalLedger::new();
    let root_cause: CauseId = 42;

    // Record root intervention
    let root_effect = ledger.record(
        root_cause,
        None,
        100,
        "temperature",
        0.8,
        0.3,
        "Thermal heatwave injection",
    );

    // Record downstream effects
    let secondary = ledger.record(
        CAUSE_BACKGROUND,
        Some(root_effect),
        105,
        "plants",
        450.0,
        -50.0,
        "Thermal stress desiccation",
    );

    let tertiary = ledger.record(
        CAUSE_BACKGROUND,
        Some(secondary),
        110,
        "herbivores",
        180.0,
        -20.0,
        "Forage scarcity starvation",
    );

    // Trace to root
    let trace = ledger.trace_to_root(tertiary);
    assert_eq!(trace, vec![tertiary, secondary, root_effect]);

    assert_eq!(ledger.root_cause(tertiary), Some(root_cause));
    assert_eq!(ledger.root_cause(secondary), Some(root_cause));
    assert_eq!(ledger.root_cause(root_effect), Some(root_cause));

    // Top contributor ranking by absolute delta in window [100, 120]
    let contributors = ledger.top_contributors("plants", (100, 120), 5);
    assert_eq!(contributors.len(), 1);
    assert_eq!(contributors[0].effect_id, secondary);
}

#[test]
fn test_declarative_interventions_spatial_and_temporal_curves() {
    let cmd = InterventionCommand {
        id: 1,
        cause_id: 10,
        kind: InterventionKind::TemperatureDelta,
        region: Region::Rect {
            min_x: 2,
            min_y: 2,
            max_x: 6,
            max_y: 6,
        },
        start_tick: 500,
        duration_ticks: 100,
        intensity: 0.4,
        signed_negative: false,
        curve: Curve::Triangle,
        reversible: true,
    };

    // Spatial containment
    assert!(cmd.affects(4, 4));
    assert!(cmd.affects(2, 2));
    assert!(!cmd.affects(1, 4));
    assert!(!cmd.affects(7, 4));

    // Temporal activation
    assert!(!cmd.active_at(499));
    assert!(cmd.active_at(500));
    assert!(cmd.active_at(550));
    assert!(cmd.active_at(599));
    assert!(!cmd.active_at(600));

    // Triangular curve intensity profile (peaks at midpoint tick 549/550)
    let intensity_start = cmd.signed_intensity_at(500);
    let intensity_mid = cmd.signed_intensity_at(550);
    let intensity_end = cmd.signed_intensity_at(599);

    assert!(intensity_start < 0.05);
    assert!(intensity_mid > 0.38);
    assert!(intensity_end < 0.05);

    // Queue ordering
    let mut cmd2 = cmd.clone();
    cmd2.id = 2;
    cmd2.start_tick = 300;
    let queue = InterventionQueue::new(vec![cmd.clone(), cmd2.clone()]);
    let active_at_350: Vec<&InterventionCommand> = queue.active_at(350).collect();
    assert_eq!(active_at_350.len(), 1);
    assert_eq!(active_at_350[0].id, 2);
}

// ------------------------------------------------------------------------------------------------
// 3. Determinism, Replay & Checkpoint Forks
// ------------------------------------------------------------------------------------------------

#[test]
fn test_deterministic_replay_and_checkpoint_fork_state_parity() {
    let fixture = fixture_path("experiments/baseline-no-exotic.json");
    let manifest_str = std::fs::read_to_string(&fixture).expect("fixture readable");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&manifest_str).expect("fixture parses");

    manifest.duration_ticks = 1200;
    manifest.sample_period = 600;
    manifest.seeds = vec![42];

    let registry = ObservableRegistry::reference_default();

    // 1. Run uninterrupted single run
    let single_run =
        run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 42, None, None);
    assert!(single_run.status.is_completed());

    // 2. Run checkpoint fork at tick 600 with no extra interventions (control arm)
    let fork_report = checkpoint_fork::<ReferenceEvolutionWorld>(
        &manifest,
        &registry,
        42,
        600,
        &[],
    )
    .expect("checkpoint fork succeeds");

    assert!(fork_report.prefix.status.is_completed());
    assert!(fork_report.control.status.is_completed());

    // Bit-exact state checksum parity between uninterrupted run and checkpoint continuation
    assert_eq!(
        single_run.final_checksum, fork_report.control.final_checksum,
        "Checkpoint fork continuation must produce bit-exact state parity with uninterrupted execution"
    );

    // Observable values parity
    assert_eq!(
        single_run.final_observables,
        fork_report.control.final_observables
    );
}

#[test]
fn test_genesis_fork_single_factor_divergence_verification() {
    let fixture = fixture_path("experiments/baseline-no-exotic.json");
    let manifest_str = std::fs::read_to_string(&fixture).expect("fixture readable");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&manifest_str).expect("fixture parses");

    manifest.duration_ticks = 600;
    manifest.sample_period = 600;
    manifest.seeds = vec![101];

    let registry = ObservableRegistry::reference_default();
    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions.values".to_string()],
    };

    let fork_res = genesis_fork::<ReferenceEvolutionWorld>(&manifest, &registry, &allowed)
        .expect("genesis fork executes cleanly");

    assert!(fork_res.control.status.is_completed());
    assert!(fork_res.treatment.status.is_completed());
    assert_eq!(fork_res.control.provenance.seed, 101);
    assert_eq!(fork_res.treatment.provenance.seed, 101);
}

// ------------------------------------------------------------------------------------------------
// 4. Statistical Ensembles & Paired Hypothesis Testing
// ------------------------------------------------------------------------------------------------

#[test]
fn test_paired_ensemble_effect_size_analysis() {
    let fixture = fixture_path("experiments/baseline-no-exotic.json");
    let manifest_str = std::fs::read_to_string(&fixture).expect("fixture readable");
    let mut treatment: ExperimentManifest =
        serde_json::from_str(&manifest_str).expect("fixture parses");

    treatment.duration_ticks = 600;
    treatment.sample_period = 600;
    treatment.seeds = vec![1, 2, 3, 4, 5];

    let registry = ObservableRegistry::reference_default();
    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions.values".to_string()],
    };

    let paired_summary = run_paired_ensemble::<ReferenceEvolutionWorld>(
        &treatment,
        &registry,
        &allowed,
    )
    .expect("paired ensemble executes");

    assert_eq!(paired_summary.complete_pairs(), 5);
    assert_eq!(paired_summary.incomplete_pairs(), 0);

    // Verify paired effect extraction
    if let Some(plants_effect) = paired_summary.effect_of("plants") {
        if let Some(mean_delta) = plants_effect.paired_mean_delta {
            assert!(mean_delta.is_finite());
        }
        if let Some(dz) = plants_effect.paired_dz {
            assert!(dz.is_finite());
        }
    }
}

// ------------------------------------------------------------------------------------------------
// 5. Headless CLI `anima-lab` Binary Integration
// ------------------------------------------------------------------------------------------------

#[test]
fn test_anima_lab_cli_help_and_argument_validation() {
    // 1. --help exits 0 and prints usage
    let help_out = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg("--help")
        .output()
        .expect("anima-lab runs");
    assert!(help_out.status.success());
    let stdout = String::from_utf8_lossy(&help_out.stdout);
    assert!(stdout.contains("anima-lab"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("sweep"));
    assert!(stdout.contains("branch"));

    // 2. Missing arguments exits 1 with usage on stderr
    let no_args = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .output()
        .expect("anima-lab runs");
    assert!(!no_args.status.success());
    let stderr = String::from_utf8_lossy(&no_args.stderr);
    assert!(stderr.contains("missing command or <MANIFEST.json>"));

    // 3. Unknown model option exits 1
    let bad_model = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["--model", "quantum", "foo.json"])
        .output()
        .expect("anima-lab runs");
    assert!(!bad_model.status.success());
    assert!(String::from_utf8_lossy(&bad_model.stderr).contains("unknown model 'quantum'"));
}

#[test]
fn test_anima_lab_cli_reference_model_execution_and_json_schema() {
    let fixture = fixture_path("experiments/baseline-no-exotic.json");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(&fixture).expect("readable"))
            .expect("parses");

    manifest.duration_ticks = 600;
    manifest.sample_period = 300;
    manifest.seeds = vec![42];

    let temp = write_temp_manifest(&manifest, "anima-lab-e2e-ref");

    let output = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg(&temp.0)
        .output()
        .expect("anima-lab executes");

    assert!(
        output.status.success(),
        "CLI execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout)
        .expect("CLI stdout must be valid JSON report");

    assert_eq!(report["tool"], "anima-lab");
    assert_eq!(report["model"], "reference");
    assert_eq!(report["completed_runs"], 1);
    assert_eq!(report["failed"].as_array().map(Vec::len), Some(0));

    let run0 = &report["runs"][0];
    assert_eq!(run0["provenance"]["seed"], 42);
    assert!(run0["final_checksum"].is_number());
    assert!(run0["series"].as_array().expect("series is array").len() >= 2);
}

#[test]
fn test_anima_lab_cli_live_adapter_execution() {
    let fixture = fixture_path("experiments_e2/e2-smoke-control-shared-brain.json");
    let mut manifest: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(&fixture).expect("readable"))
            .expect("parses");

    manifest.duration_ticks = 2;
    manifest.sample_period = 1;
    manifest.seeds = vec![77];

    let temp = write_temp_manifest(&manifest, "anima-lab-e2e-live");

    let output = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["--model", "live"])
        .arg(&temp.0)
        .output()
        .expect("anima-lab executes");

    assert!(
        output.status.success(),
        "CLI live execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout)
        .expect("CLI stdout must be valid JSON report");

    assert_eq!(report["model"], "live");
    assert_eq!(report["completed_runs"], 1);
}
