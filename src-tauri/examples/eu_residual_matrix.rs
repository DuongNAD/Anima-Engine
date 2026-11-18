//! CARD M4 — Release-mode full-factorial closed-EU residual matrix runner.
//!
//! Covers all 192 policy flag combinations (`2 * 2 * 2 * 2 * 2 * 3 * 2`), 16 `drift_control`
//! paired runs (`K in {0, 60}` across `plant_reserve`, `locomotion`, `evolved_brains`), 27
//! `(profile x InterventionKind)` runs (all 8 `InterventionKind` variants + `All8Combined` on
//! `baseline`, `sel2_full`, and `drift_k60`), and excluded-key guards.
//!
//! Build in release mode and execute the binary directly (never `cargo run`):
//!   `cargo build --release --features desktop --example eu_residual_matrix`
//!   `target\release\examples\eu_residual_matrix.exe`

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
use std::process::ExitCode;
use std::time::Instant;

const SEED: u64 = 20261118;
const RUN_TICKS: u64 = 90;
const DRIFT_TICKS: u64 = 120;

#[derive(Clone, Copy, Debug)]
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

fn obs_val(obs: &[(String, f64)], key: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("missing observable '{key}'"))
        .1
}

struct Metrics {
    eu_0: f64,
    delta_t0: f64,
    delta_t1: f64,
    worst_abs_t1: f64,
    plants_field_gap: f64,
    ledger_effects: usize,
}

impl Metrics {
    fn max_abs(&self) -> f64 {
        self.delta_t0
            .abs()
            .max(self.delta_t1.abs())
            .max(self.worst_abs_t1)
            .max(self.plants_field_gap)
    }
}

fn measure_single_run(
    pt: PolicyPoint,
    extra: &[(&str, f64)],
    interventions: Vec<InterventionCommand>,
    run_ticks: u64,
) -> Metrics {
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

    Metrics {
        eu_0,
        delta_t0: eu_final - eu_0,
        delta_t1: eu_final - eu_1,
        worst_abs_t1,
        plants_field_gap,
        ledger_effects: causal.len(),
    }
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
) -> (f64, f64, f64) {
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

    let eu_0 = {
        let adapter0 = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &sel_init,
            &[],
            SEED,
            (16, 16),
            DRIFT_TICKS,
        )
        .expect("baseline adapter builds");
        let o0 = adapter0.observables();
        obs_val(&o0, "live.closed_eu_total")
            + obs_val(&o0, "live.agent_count") * obs_val(&o0, "live.mean_agent_energy")
    };

    let m_sel = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: format!("m4-full-{label}-sel"),
        name: label.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: sel_init,
        interventions,
        seeds: vec![SEED],
        duration_ticks: DRIFT_TICKS,
        sample_period: 1,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let m_drf = ExperimentManifest {
        experiment_id: format!("m4-full-{label}-drf"),
        initial_conditions: drf_init,
        ..m_sel.clone()
    };

    let pair = run_live_drift_pair(&m_sel, &m_drf, &registry, SEED).expect("drift pair builds");
    assert_eq!(pair.selected.status, RunStatus::Completed);
    assert_eq!(pair.drift.status, RunStatus::Completed);

    let sel_eu1 = obs_val(&pair.selected.series[0].observables, "live.closed_eu_total");
    let drf_eu1 = obs_val(&pair.drift.series[0].observables, "live.closed_eu_total");

    let sel_worst = pair
        .selected
        .series
        .iter()
        .map(|s| {
            let v = obs_val(&s.observables, "live.closed_eu_total");
            (v - sel_eu1).abs().max((v - eu_0).abs())
        })
        .fold(0.0f64, f64::max);
    let drf_worst = pair
        .drift
        .series
        .iter()
        .map(|s| {
            let v = obs_val(&s.observables, "live.closed_eu_total");
            (v - drf_eu1).abs().max((v - eu_0).abs())
        })
        .fold(0.0f64, f64::max);

    (eu_0, sel_worst, drf_worst)
}

fn main() -> ExitCode {
    let total_start = Instant::now();
    let mut violations: Vec<String> = Vec::new();

    // 1. Full 192-point policy factorial.
    let policy_start = Instant::now();
    let mut policy_worst = 0.0f64;
    let mut policy_worst_label = String::new();
    let mut policy_count = 0usize;

    for m in 0..2u8 {
        for r in 0..2u8 {
            for l in 0..2u8 {
                for p in 0..2u8 {
                    for c in 0..2u8 {
                        for b in 0..3u8 {
                            for k in 0..2u8 {
                                let pt = PolicyPoint {
                                    mortality: m,
                                    reproduction: r,
                                    locomotion: l,
                                    reserve: p,
                                    capacity: c,
                                    brain: b,
                                    predator_income: k,
                                };
                                let label = pt.label();
                                let res = measure_single_run(pt, &[], Vec::new(), RUN_TICKS);
                                let peak = res.max_abs();
                                if peak > policy_worst {
                                    policy_worst = peak;
                                    policy_worst_label = label.clone();
                                }
                                if peak > RESIDUAL_ABS_TOLERANCE_EU {
                                    violations.push(format!("{label}: {peak:.3e}"));
                                }
                                if m == r && l == p && c == 0 && b == 2 && k == 1
                                    || peak == policy_worst
                                    || policy_count == 0
                                    || policy_count == 191
                                {
                                    println!(
                                        "POLICY[{policy_count:03}] {label:<24} eu_0={:.3} delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
                                        res.eu_0, res.delta_t0, res.delta_t1, res.worst_abs_t1, res.plants_field_gap
                                    );
                                }
                                policy_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "FULL_POLICY_SUMMARY runs={policy_count} elapsed_s={:.2} worst={policy_worst:.3e} worst_case={policy_worst_label}",
        policy_start.elapsed().as_secs_f64()
    );

    // 2. 16-point drift_control matrix (K in {0, 60} x reserve x locomotion x evolved_brains).
    let drift_start = Instant::now();
    let mut drift_worst = 0.0f64;
    let mut drift_worst_label = String::new();
    let mut drift_count = 0usize;
    for k in [0u64, 60u64] {
        for p in 0..2u8 {
            for l in 0..2u8 {
                for b in 0..2u8 {
                    let pt = PolicyPoint {
                        mortality: 1,
                        reproduction: 1,
                        locomotion: l,
                        reserve: p,
                        capacity: 0,
                        brain: b,
                        predator_income: 0,
                    };
                    let label = format!("k{k}_p{p}_l{l}_b{b}");
                    let (eu_0, sel_w, drf_w) = run_drift_pair_case(&label, pt, k, Vec::new());
                    let peak = sel_w.max(drf_w);
                    if peak > drift_worst {
                        drift_worst = peak;
                        drift_worst_label = label.clone();
                    }
                    if peak > RESIDUAL_ABS_TOLERANCE_EU {
                        violations.push(format!("drift/{label}: {peak:.3e}"));
                    }
                    println!(
                        "DRIFT_PAIR[{drift_count:02}] {label:<16} eu_0={eu_0:.3} sel_worst={sel_w:.3e} drf_worst={drf_w:.3e}"
                    );
                    drift_count += 1;
                }
            }
        }
    }
    println!(
        "FULL_DRIFT_SUMMARY pairs={drift_count} elapsed_s={:.2} worst={drift_worst:.3e} worst_case={drift_worst_label}",
        drift_start.elapsed().as_secs_f64()
    );

    // 3. Interventions across Baseline, SEL2 Full, and Drift K=60.
    let int_start = Instant::now();
    let baseline = PolicyPoint {
        mortality: 0,
        reproduction: 0,
        locomotion: 0,
        reserve: 0,
        capacity: 0,
        brain: 0,
        predator_income: 0,
    };
    let sel2_full = PolicyPoint {
        mortality: 1,
        reproduction: 1,
        locomotion: 1,
        reserve: 1,
        capacity: 1,
        brain: 2,
        predator_income: 1,
    };
    let drift_full = PolicyPoint {
        mortality: 1,
        reproduction: 1,
        locomotion: 1,
        reserve: 1,
        capacity: 1,
        brain: 1,
        predator_income: 1,
    };
    let mut int_worst = 0.0f64;
    let mut int_worst_label = String::new();
    let mut int_count = 0usize;

    for (name, cmds) in all_intervention_cases() {
        let min_effects = if name == "All8Combined" { 8 } else { 1 };
        for (prof, pt) in [("baseline", baseline), ("sel2_full", sel2_full)] {
            let m = measure_single_run(pt, &[], cmds.clone(), RUN_TICKS);
            assert!(
                m.ledger_effects >= min_effects,
                "{prof}/{name}: expected >= {min_effects} causal effects, got {}",
                m.ledger_effects
            );
            let peak = m.max_abs();
            let full_label = format!("{prof}/{name}");
            if peak > int_worst {
                int_worst = peak;
                int_worst_label = full_label.clone();
            }
            if peak > RESIDUAL_ABS_TOLERANCE_EU {
                violations.push(format!("{full_label}: {peak:.3e}"));
            }
            println!(
                "INTERVENTION {full_label:<34} eff={} delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
                m.ledger_effects, m.delta_t0, m.delta_t1, m.worst_abs_t1, m.plants_field_gap
            );
            int_count += 1;
        }
        let (eu_0, sel_w, drf_w) =
            run_drift_pair_case(&format!("k60-{name}"), drift_full, 60, cmds);
        let peak = sel_w.max(drf_w);
        let dlabel = format!("drift_k60/{name}");
        if peak > int_worst {
            int_worst = peak;
            int_worst_label = dlabel.clone();
        }
        if peak > RESIDUAL_ABS_TOLERANCE_EU {
            violations.push(format!("{dlabel}: {peak:.3e}"));
        }
        println!(
            "INTERVENTION {dlabel:<34} eu_0={eu_0:.3} sel_worst={sel_w:.3e} drf_worst={drf_w:.3e}"
        );
        int_count += 1;
    }
    println!(
        "FULL_INTERVENTIONS_SUMMARY entries={int_count} elapsed_s={:.2} worst={int_worst:.3e} worst_case={int_worst_label}",
        int_start.elapsed().as_secs_f64()
    );

    // 4. Excluded keys spot-check.
    let excl = measure_single_run(
        drift_full,
        &[
            (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
            (LIVE_KEY_W_SURPLUS, 2.0),
            (LIVE_KEY_MUTATION_RATE, 0.0),
            (LIVE_KEY_TARGET_MIN_ENERGY, 25.0),
        ],
        Vec::new(),
        RUN_TICKS,
    );
    let excl_peak = excl.max_abs();
    if excl_peak > RESIDUAL_ABS_TOLERANCE_EU {
        violations.push(format!("excluded_guard: {excl_peak:.3e}"));
    }
    println!(
        "EXCLUDED_GUARD delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e}",
        excl.delta_t0, excl.delta_t1, excl.worst_abs_t1, excl.plants_field_gap
    );

    let overall_worst = policy_worst.max(drift_worst).max(int_worst).max(excl_peak);
    println!(
        "M4_OVERALL_SUMMARY total_s={:.2} tolerance={RESIDUAL_ABS_TOLERANCE_EU:.1e} overall_worst={overall_worst:.3e} violations={}",
        total_start.elapsed().as_secs_f64(),
        violations.len()
    );
    if violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("LEAK_VIOLATIONS: {:?}", violations);
        ExitCode::FAILURE
    }
}
