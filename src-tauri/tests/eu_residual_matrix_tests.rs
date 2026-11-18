//! CARD M4 — Closed-EU residual matrix gate (debug suite, < 120 s budget).
//!
//! Exercises the real `LiveExperimentAdapter` schedule (`core::simulation_schedule::build_tick_schedule`)
//! against [`RESIDUAL_ABS_TOLERANCE_EU`] across:
//! 1. A verified strength-2 (pairwise) covering array over the 7 policy factors affecting EU flow,
//! 2. All 8 `InterventionKind` variants (individually + combined) on baseline, full SEL2 stack, and `drift_control`,
//! 3. Locked-step neutral-drift replay pairs (`K = 0` and rolling `K = 60` Channon resets),
//! 4. An excluded-key spot guard (`ecological_fitness`, `w_surplus`, `mutation_rate`, `target_min_energy`).

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::energy_ledger::RESIDUAL_ABS_TOLERANCE_EU;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{ExperimentModel, RunStatus};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region, TraitMod,
};
use anima_engine_lib::core::live_experiment::{
    run_live_drift_pair, LiveExperimentAdapter, LIVE_EXTENDED_OBSERVABLE_IDS,
    LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIFT_RESET_TICKS,
    LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_EVOLVED_BRAINS,
    LIVE_KEY_KILL_EFFICIENCY, LIVE_KEY_KILL_HYDRATION, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY,
    LIVE_KEY_MUTATION_RATE, LIVE_KEY_PLANT_RESERVE, LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_STARVE_TICKS, LIVE_KEY_TARGET_MIN_ENERGY,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_W_SURPLUS, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::{BrainPolicy, LifetimeLearning};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;
use std::time::Instant;

const SEED: u64 = 20261118;
const RUN_TICKS: u64 = 90;
const DRIFT_TICKS: u64 = 120;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolicyPoint {
    mortality: u8,
    reproduction: u8,
    locomotion: u8,
    reserve: u8,
    capacity: u8,
    brain: u8,
    predator_income: u8,
}

impl PolicyPoint {
    fn as_array(self) -> [u8; 7] {
        [
            self.mortality,
            self.reproduction,
            self.locomotion,
            self.reserve,
            self.capacity,
            self.brain,
            self.predator_income,
        ]
    }

    fn from_array(a: [u8; 7]) -> Self {
        Self {
            mortality: a[0],
            reproduction: a[1],
            locomotion: a[2],
            reserve: a[3],
            capacity: a[4],
            brain: a[5],
            predator_income: a[6],
        }
    }

    fn label(self) -> String {
        let brain_tag = match self.brain {
            0 => "sh",
            1 => "ev",
            _ => "ev+ll+cost",
        };
        format!(
            "m{}_r{}_l{}_p{}_c{}_b({})_k{}",
            self.mortality,
            self.reproduction,
            self.locomotion,
            self.reserve,
            self.capacity,
            brain_tag,
            self.predator_income
        )
    }

    fn to_initial(self, extra: &[(&str, f64)]) -> InitialConditionSet {
        let mut pairs: Vec<(String, f64)> = vec![
            ("live.founders".to_string(), 16.0),
            ("live.predator_fraction".to_string(), 0.25),
            ("live.trees".to_string(), 8.0),
            ("live.lakes".to_string(), 2.0),
            ("live.food_cap".to_string(), 50.0),
        ];
        if self.mortality == 1 {
            pairs.push((LIVE_KEY_MORTALITY.to_string(), 1.0));
            pairs.push((LIVE_KEY_STARVE_TICKS.to_string(), 15.0));
            pairs.push((LIVE_KEY_DEHYDRATION_TICKS.to_string(), 20.0));
        }
        if self.reproduction == 1 {
            pairs.push((LIVE_KEY_REPRODUCTION.to_string(), 1.0));
            pairs.push((LIVE_KEY_PREY_CAP.to_string(), 14.0));
        }
        if self.locomotion == 1 {
            pairs.push((LIVE_KEY_LOCOMOTION.to_string(), 1.0));
            pairs.push((LIVE_KEY_DRIVE_FLOOR.to_string(), 0.5));
            pairs.push((LIVE_KEY_THIRST_STEERING.to_string(), 1.0));
        }
        if self.reserve == 1 {
            pairs.push((LIVE_KEY_PLANT_RESERVE.to_string(), 0.3));
        }
        if self.capacity == 1 {
            pairs.push((LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string(), 0.5));
        }
        if self.brain >= 1 {
            pairs.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), 1.0));
        }
        if self.predator_income == 1 {
            pairs.push((LIVE_KEY_KILL_EFFICIENCY.to_string(), 0.65));
            pairs.push((LIVE_KEY_KILL_HYDRATION.to_string(), 0.5));
        }
        for (k, v) in extra {
            pairs.retain(|(ek, _)| ek != k);
            pairs.push(((*k).to_string(), *v));
        }
        InitialConditionSet::new(pairs)
    }

    fn apply_runtime_brain_policy(self, adapter: &mut LiveExperimentAdapter) {
        if self.brain == 2 {
            let mut world = adapter.world();
            world.insert_resource(BrainPolicy {
                evolved: true,
                lifetime_learning: LifetimeLearning {
                    enabled: true,
                    learning_rate: 1e-3,
                    discount: 0.99,
                    interval: 4,
                    active_radius: f32::INFINITY,
                },
                brain_metabolic_cost: 0.5,
                ..Default::default()
            });
        }
    }
}

fn full_policy_factorial() -> Vec<PolicyPoint> {
    let mut out = Vec::with_capacity(192);
    for m in 0..2u8 {
        for r in 0..2u8 {
            for l in 0..2u8 {
                for p in 0..2u8 {
                    for c in 0..2u8 {
                        for b in 0..3u8 {
                            for k in 0..2u8 {
                                out.push(PolicyPoint::from_array([m, r, l, p, c, b, k]));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn all_pairwise_tuples() -> Vec<(usize, u8, usize, u8)> {
    const CARDS: [u8; 7] = [2, 2, 2, 2, 2, 3, 2];
    let mut pairs = Vec::new();
    for (i, &ci) in CARDS.iter().enumerate() {
        for (j, &cj) in CARDS.iter().enumerate().skip(i + 1) {
            for vi in 0..ci {
                for vj in 0..cj {
                    pairs.push((i, vi, j, vj));
                }
            }
        }
    }
    pairs
}

/// Deterministic greedy strength-2 covering design seeded with both extreme corners (`all-off` and `all-on`).
fn pairwise_covering_design() -> Vec<PolicyPoint> {
    let universe = full_policy_factorial();
    let target_pairs = all_pairwise_tuples();
    let mut uncovered = vec![true; target_pairs.len()];

    let mut chosen = vec![
        PolicyPoint::from_array([0, 0, 0, 0, 0, 0, 0]),
        PolicyPoint::from_array([1, 1, 1, 1, 1, 2, 1]),
    ];

    let mark_covered = |pt: PolicyPoint, uncovered: &mut [bool]| {
        let arr = pt.as_array();
        for (idx, &(i, vi, j, vj)) in target_pairs.iter().enumerate() {
            if arr[i] == vi && arr[j] == vj {
                uncovered[idx] = false;
            }
        }
    };

    for &pt in &chosen {
        mark_covered(pt, &mut uncovered);
    }

    while uncovered.iter().any(|&u| u) {
        let best = universe
            .iter()
            .copied()
            .max_by_key(|cand| {
                let arr = cand.as_array();
                target_pairs
                    .iter()
                    .enumerate()
                    .filter(|&(idx, &(i, vi, j, vj))| {
                        uncovered[idx] && arr[i] == vi && arr[j] == vj
                    })
                    .count()
            })
            .expect("universe is non-empty");
        mark_covered(best, &mut uncovered);
        chosen.push(best);
    }

    chosen
}

fn obs_val(obs: &[(String, f64)], key: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("missing observable '{key}'"))
        .1
}

#[derive(Clone, Debug)]
struct ResidualMetrics {
    eu_0: f64,
    eu_1: f64,
    eu_final: f64,
    delta_t0: f64,
    delta_t1: f64,
    worst_abs_t1: f64,
    plants_field_gap: f64,
    ledger_effects: usize,
}

fn measure_single_run(
    pt: PolicyPoint,
    extra: &[(&str, f64)],
    interventions: Vec<InterventionCommand>,
    run_ticks: u64,
) -> ResidualMetrics {
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &pt.to_initial(extra),
        &[],
        SEED,
        (16, 16),
        run_ticks,
    )
    .expect("manifest must build");
    pt.apply_runtime_brain_policy(&mut adapter);

    let obs0 = adapter.observables();
    let eu_0 = obs_val(&obs0, "live.closed_eu_total")
        + obs_val(&obs0, "live.agent_count") * obs_val(&obs0, "live.mean_agent_energy");

    let mut clock = SimClock::new();
    let mut causal = CausalLedger::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let queue = InterventionQueue::new(interventions);

    let mut eu_1 = 0.0;
    let mut worst_abs_t1 = 0.0f64;
    let mut eu_final = 0.0;

    for t in 1..=run_ticks {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        adapter.step(&clock, &active, &mut causal, &mut rng);

        let obs = if t == 1 || t == run_ticks {
            adapter.observables()
        } else {
            adapter.observables_unsampled()
        };
        let eu_t = obs_val(&obs, "live.closed_eu_total");
        if t == 1 {
            eu_1 = eu_t;
        }
        worst_abs_t1 = worst_abs_t1.max((eu_t - eu_1).abs());
        if t == run_ticks {
            eu_final = eu_t;
        }
    }

    let plants_field_gap = {
        let world = adapter.world();
        let carried = world.resource::<EcosystemBiomass>().plants;
        let actual = world.resource::<ResourceField>().total_biomass();
        (carried - actual).abs()
    };

    ResidualMetrics {
        eu_0,
        eu_1,
        eu_final,
        delta_t0: eu_final - eu_0,
        delta_t1: eu_final - eu_1,
        worst_abs_t1,
        plants_field_gap,
        ledger_effects: causal.len(),
    }
}

fn assert_residual_ok(label: &str, m: &ResidualMetrics) {
    assert!(
        m.delta_t0.abs() <= RESIDUAL_ABS_TOLERANCE_EU
            && m.delta_t1.abs() <= RESIDUAL_ABS_TOLERANCE_EU
            && m.worst_abs_t1 <= RESIDUAL_ABS_TOLERANCE_EU
            && m.plants_field_gap <= RESIDUAL_ABS_TOLERANCE_EU,
        "{label}: EU residual exceeded {RESIDUAL_ABS_TOLERANCE_EU:e} — \
         eu_0={:.6} eu_1={:.6} eu_final={:.6} delta_t0={:+.3e} delta_t1={:+.3e} \
         worst_t1={:.3e} plants_gap={:.3e}",
        m.eu_0,
        m.eu_1,
        m.eu_final,
        m.delta_t0,
        m.delta_t1,
        m.worst_abs_t1,
        m.plants_field_gap
    );
}

fn cmd_with(
    id: u32,
    kind: InterventionKind,
    start_tick: u64,
    duration_ticks: u64,
    intensity: f32,
) -> InterventionCommand {
    InterventionCommand {
        id,
        cause_id: 200 + id,
        kind,
        region: Region::Global,
        start_tick,
        duration_ticks,
        intensity,
        signed_negative: false,
        curve: Curve::Step,
        reversible: false,
    }
}

fn all_intervention_cases() -> Vec<(&'static str, Vec<InterventionCommand>)> {
    let single = vec![
        (
            "RainfallDelta",
            vec![cmd_with(1, InterventionKind::RainfallDelta, 30, 45, 0.5)],
        ),
        (
            "TemperatureDelta",
            vec![cmd_with(2, InterventionKind::TemperatureDelta, 30, 45, 0.5)],
        ),
        (
            "Deforest",
            vec![cmd_with(3, InterventionKind::Deforest, 60, 1, 0.5)],
        ),
        (
            "AddNutrient",
            vec![cmd_with(4, InterventionKind::AddNutrient, 60, 1, 1.0)],
        ),
        (
            "RemovePredators",
            vec![cmd_with(5, InterventionKind::RemovePredators, 60, 1, 1.0)],
        ),
        (
            "GeneticInjection(5Mods)",
            vec![cmd_with(
                6,
                InterventionKind::GeneticInjection {
                    target_species: None,
                    trait_modifications: vec![
                        TraitMod::ScaleMorphology(1.1),
                        TraitMod::MassMultiplier(1.05),
                        TraitMod::MetabolicEfficiencyDelta(0.15),
                        TraitMod::SensoryRangeMultiplier(1.2),
                        TraitMod::BrainParameterShift {
                            layer_index: 0,
                            bias_delta: 0.05,
                        },
                    ],
                    count: 4,
                },
                60,
                1,
                1.0,
            )],
        ),
        (
            "ToxicityPlume",
            vec![cmd_with(
                7,
                InterventionKind::ToxicityPlume {
                    center_x: 128.0,
                    center_y: 128.0,
                    radius: 400.0,
                    toxicity_level: 10.0,
                    decay_rate: 0.0,
                },
                60,
                1,
                1.0,
            )],
        ),
        (
            "PathogenTransmission",
            vec![cmd_with(
                8,
                InterventionKind::PathogenTransmission {
                    pathogen_id: 1,
                    transmission_rate: 10.0,
                    mortality_multiplier: 2.5,
                },
                60,
                1,
                1.0,
            )],
        ),
    ];

    let combined = vec![
        cmd_with(11, InterventionKind::RainfallDelta, 30, 45, 0.5),
        cmd_with(12, InterventionKind::TemperatureDelta, 30, 45, 0.5),
        cmd_with(13, InterventionKind::Deforest, 60, 1, 0.35),
        cmd_with(14, InterventionKind::AddNutrient, 60, 1, 1.0),
        cmd_with(15, InterventionKind::RemovePredators, 60, 1, 0.5),
        cmd_with(
            16,
            InterventionKind::GeneticInjection {
                target_species: None,
                trait_modifications: vec![
                    TraitMod::ScaleMorphology(1.1),
                    TraitMod::MassMultiplier(1.05),
                    TraitMod::MetabolicEfficiencyDelta(0.15),
                    TraitMod::SensoryRangeMultiplier(1.2),
                    TraitMod::BrainParameterShift {
                        layer_index: 0,
                        bias_delta: 0.05,
                    },
                ],
                count: 4,
            },
            60,
            1,
            1.0,
        ),
        cmd_with(
            17,
            InterventionKind::ToxicityPlume {
                center_x: 128.0,
                center_y: 128.0,
                radius: 250.0,
                toxicity_level: 8.0,
                decay_rate: 0.0,
            },
            60,
            1,
            1.0,
        ),
        cmd_with(
            18,
            InterventionKind::PathogenTransmission {
                pathogen_id: 2,
                transmission_rate: 8.0,
                mortality_multiplier: 2.0,
            },
            60,
            1,
            1.0,
        ),
    ];

    let mut all = single;
    all.push(("All8Combined", combined));
    all
}

fn shared_observable_ids() -> Vec<String> {
    LIVE_OBSERVABLE_IDS
        .iter()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS[..10].iter())
        .map(|s| (*s).to_string())
        .collect()
}

fn run_drift_pair_case(
    label: &str,
    pt: PolicyPoint,
    reset_ticks: u64,
    interventions: Vec<InterventionCommand>,
    duration_ticks: u64,
) -> (f64, f64) {
    let registry = ObservableRegistry::live_default();
    let sel_init = pt.to_initial(&[]);
    let drf_init = if reset_ticks > 0 {
        pt.to_initial(&[
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
            (LIVE_KEY_DRIFT_RESET_TICKS, reset_ticks as f64),
        ])
    } else {
        pt.to_initial(&[(LIVE_KEY_DRIFT_CONTROL, 1.0)])
    };

    let obs0 = {
        let adapter0 = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &sel_init,
            &[],
            SEED,
            (16, 16),
            duration_ticks,
        )
        .expect("baseline adapter builds for eu_0");
        let o0 = adapter0.observables();
        obs_val(&o0, "live.closed_eu_total")
            + obs_val(&o0, "live.agent_count") * obs_val(&o0, "live.mean_agent_energy")
    };

    let m_sel = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: format!("m4-{label}-sel"),
        name: label.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: sel_init,
        interventions: interventions.clone(),
        seeds: vec![SEED],
        duration_ticks,
        sample_period: 1,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let m_drf = ExperimentManifest {
        experiment_id: format!("m4-{label}-drf"),
        initial_conditions: drf_init,
        ..m_sel.clone()
    };

    let pair = run_live_drift_pair(&m_sel, &m_drf, &registry, SEED)
        .unwrap_or_else(|e| panic!("{label}: drift pair failed to build: {e}"));
    assert_eq!(pair.selected.status, RunStatus::Completed, "{label} sel");
    assert_eq!(pair.drift.status, RunStatus::Completed, "{label} drf");

    let sel_eu1 = obs_val(&pair.selected.series[0].observables, "live.closed_eu_total");
    let drf_eu1 = obs_val(&pair.drift.series[0].observables, "live.closed_eu_total");

    let sel_worst = pair
        .selected
        .series
        .iter()
        .map(|s| {
            let v = obs_val(&s.observables, "live.closed_eu_total");
            (v - sel_eu1).abs().max((v - obs0).abs())
        })
        .fold(0.0f64, f64::max);
    let drf_worst = pair
        .drift
        .series
        .iter()
        .map(|s| {
            let v = obs_val(&s.observables, "live.closed_eu_total");
            (v - drf_eu1).abs().max((v - obs0).abs())
        })
        .fold(0.0f64, f64::max);

    assert!(
        sel_worst <= RESIDUAL_ABS_TOLERANCE_EU && drf_worst <= RESIDUAL_ABS_TOLERANCE_EU,
        "{label}: drift pair EU exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU:e} \
         (sel_worst={sel_worst:.3e}, drf_worst={drf_worst:.3e}, eu_0={obs0:.6})"
    );
    (sel_worst, drf_worst)
}

#[test]
fn covering_design_covers_100_percent_of_all_pairwise_interactions() {
    let design = pairwise_covering_design();
    let pairs = all_pairwise_tuples();
    for &(i, vi, j, vj) in &pairs {
        assert!(
            design.iter().any(|pt| {
                let a = pt.as_array();
                a[i] == vi && a[j] == vj
            }),
            "missing 2-way pair ({i}={vi}, {j}={vj})"
        );
    }
    assert!(
        design.contains(&PolicyPoint::from_array([0, 0, 0, 0, 0, 0, 0])),
        "all-off corner must be included"
    );
    assert!(
        design.contains(&PolicyPoint::from_array([1, 1, 1, 1, 1, 2, 1])),
        "all-on corner must be included"
    );
    println!(
        "M4_COVERING_DESIGN runs={} pairwise_tuples={}",
        design.len(),
        pairs.len()
    );
}

#[test]
fn policy_flag_covering_array_conserves_closed_eu() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let started = Instant::now();
    let design = pairwise_covering_design();
    let mut global_worst = 0.0f64;

    for (idx, &pt) in design.iter().enumerate() {
        let m = measure_single_run(pt, &[], Vec::new(), RUN_TICKS);
        println!(
            "M4_PAIR[{idx:02}] {:<24} eu_0={:.3} delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
            pt.label(),
            m.eu_0,
            m.delta_t0,
            m.delta_t1,
            m.worst_abs_t1,
            m.plants_field_gap
        );
        global_worst = global_worst
            .max(m.delta_t0.abs())
            .max(m.delta_t1.abs())
            .max(m.worst_abs_t1)
            .max(m.plants_field_gap);
        assert_residual_ok(&pt.label(), &m);
    }

    println!(
        "M4_POLICY_COVERING_SUMMARY runs={} elapsed_s={:.2} max_abs_residual={:.3e}",
        design.len(),
        started.elapsed().as_secs_f64(),
        global_worst
    );
}

#[test]
fn every_intervention_kind_conserves_closed_eu_across_profiles() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let started = Instant::now();
    let baseline = PolicyPoint::from_array([0, 0, 0, 0, 0, 0, 0]);
    let full_sel2 = PolicyPoint::from_array([1, 1, 1, 1, 1, 2, 1]);
    let cases = all_intervention_cases();
    let mut global_worst = 0.0f64;

    for (name, cmds) in &cases {
        for (prof_name, pt) in [("baseline", baseline), ("sel2_full", full_sel2)] {
            let label = format!("{prof_name}/{name}");
            let m = measure_single_run(pt, &[], cmds.clone(), RUN_TICKS);
            let min_effects = if *name == "All8Combined" { 8 } else { 1 };
            assert!(
                m.ledger_effects >= min_effects,
                "{label}: expected >= {min_effects} causal ledger effect(s), got {}",
                m.ledger_effects
            );
            println!(
                "M4_INT {:<34} eff={} delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
                label, m.ledger_effects, m.delta_t0, m.delta_t1, m.worst_abs_t1, m.plants_field_gap
            );
            global_worst = global_worst
                .max(m.delta_t0.abs())
                .max(m.delta_t1.abs())
                .max(m.worst_abs_t1)
                .max(m.plants_field_gap);
            assert_residual_ok(&label, &m);
        }
    }

    println!(
        "M4_INTERVENTIONS_SUMMARY runs={} elapsed_s={:.2} max_abs_residual={:.3e}",
        cases.len() * 2,
        started.elapsed().as_secs_f64(),
        global_worst
    );
}

#[test]
fn drift_control_pairs_and_excluded_keys_conserve_closed_eu() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let started = Instant::now();
    let mut global_worst = 0.0f64;

    let drift_cases = [
        (
            "k0_nores_shared",
            PolicyPoint::from_array([1, 1, 1, 0, 0, 0, 0]),
            0,
        ),
        (
            "k0_res30_evolved",
            PolicyPoint::from_array([1, 1, 1, 1, 0, 1, 0]),
            0,
        ),
        (
            "k60_nores_shared",
            PolicyPoint::from_array([1, 1, 1, 0, 0, 0, 0]),
            60,
        ),
        (
            "k60_res30_evolved",
            PolicyPoint::from_array([1, 1, 1, 1, 1, 1, 1]),
            60,
        ),
    ];

    for (label, pt, k) in drift_cases {
        let (sel_w, drf_w) = run_drift_pair_case(label, pt, k, Vec::new(), DRIFT_TICKS);
        println!(
            "M4_DRIFT {:<22} k={k:<2} sel_worst={sel_w:.3e} drf_worst={drf_w:.3e}",
            label
        );
        global_worst = global_worst.max(sel_w).max(drf_w);
    }

    let drift_full = PolicyPoint::from_array([1, 1, 1, 1, 1, 1, 1]);
    for (name, cmds) in all_intervention_cases() {
        let label = format!("drift_k60/{name}");
        let (sel_w, drf_w) = run_drift_pair_case(&label, drift_full, 60, cmds, DRIFT_TICKS);
        println!(
            "M4_DRIFT_INT {:<34} sel_worst={sel_w:.3e} drf_worst={drf_w:.3e}",
            label
        );
        global_worst = global_worst.max(sel_w).max(drf_w);
    }

    let excluded_pt = PolicyPoint::from_array([1, 1, 1, 1, 0, 1, 1]);
    let excl = measure_single_run(
        excluded_pt,
        &[
            (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
            (LIVE_KEY_W_SURPLUS, 2.0),
            (LIVE_KEY_MUTATION_RATE, 0.0),
            (LIVE_KEY_TARGET_MIN_ENERGY, 25.0),
        ],
        Vec::new(),
        RUN_TICKS,
    );
    println!(
        "M4_EXCLUDED_GUARD delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
        excl.delta_t0, excl.delta_t1, excl.worst_abs_t1, excl.plants_field_gap
    );
    assert_residual_ok("excluded_guard", &excl);
    global_worst = global_worst
        .max(excl.delta_t0.abs())
        .max(excl.delta_t1.abs())
        .max(excl.worst_abs_t1)
        .max(excl.plants_field_gap);

    println!(
        "M4_DRIFT_AND_EXCLUDED_SUMMARY elapsed_s={:.2} max_abs_residual={:.3e}",
        started.elapsed().as_secs_f64(),
        global_worst
    );
}
