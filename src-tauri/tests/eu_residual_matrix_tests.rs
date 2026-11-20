//! CARD M4 — Closed-EU residual matrix gate (debug suite, < 120 s budget).
//!
//! Exercises the real `LiveExperimentAdapter` schedule (`core::simulation_schedule::build_tick_schedule`)
//! against [`RESIDUAL_ABS_TOLERANCE_EU`] with non-vacuity assertions on every active factor level:
//! 1. A verified strength-2 (pairwise) covering array (`8` rows covering `96/96` pairwise tuples)
//!    over all `7` energy-flow policy factors, asserting births, cause-split deaths, and predation
//!    transfers (`CombatEvents`) whenever enabled and mechanically reachable,
//! 2. All `8` `InterventionKind` variants (individually on `baseline`, plus direct EU-moving
//!    treatments & `All8Combined` on `sel2_full` and `drift_control`), asserting causal ledger effects,
//! 3. Locked-step neutral-drift replay pairs (`K = 0` and rolling `K = 800` Channon resets) +
//!    direct parity check against `run_live_drift_pair`, asserting `>= 1` birth and `>= 1` death
//!    replayed in the neutral-drift arm,
//! 4. An excluded-key spot guard (`ecological_fitness`, `w_surplus`, `mutation_rate`, `target_min_energy`).

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::drift_control::DriftDiagnostics;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::energy_ledger::{closed_total_eu, RESIDUAL_ABS_TOLERANCE_EU};
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
use anima_engine_lib::core::resources::{
    derived_sim_rng, sim_stream, BrainPolicy, CombatEvents, LifetimeLearning, VitalStatistics,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;
use std::time::Instant;

const SEED: u64 = 20261118;
const POLICY_TICKS: u64 = 2_000;
const DRIFT_TICKS: u64 = 2_400;
const DRIFT_RESET_K: u64 = 800;
const BASELINE_INTERVENTION_TICKS: u64 = 180;
const PLANT_GAP_STRIDE: u64 = 250;

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
            ("live.founders".to_string(), 20.0),
            ("live.predator_fraction".to_string(), 0.30),
            ("live.trees".to_string(), 4.0),
            ("live.lakes".to_string(), 1.0),
            ("live.food_cap".to_string(), 20.0),
        ];
        if self.mortality == 1 {
            pairs.push((LIVE_KEY_MORTALITY.to_string(), 1.0));
            pairs.push((LIVE_KEY_STARVE_TICKS.to_string(), 5.0));
            pairs.push((LIVE_KEY_DEHYDRATION_TICKS.to_string(), 30.0));
        }
        if self.reproduction == 1 {
            pairs.push((LIVE_KEY_REPRODUCTION.to_string(), 1.0));
            pairs.push((LIVE_KEY_PREY_CAP.to_string(), 18.0));
        }
        if self.locomotion == 1 {
            pairs.push((LIVE_KEY_LOCOMOTION.to_string(), 1.0));
            pairs.push((LIVE_KEY_DRIVE_FLOOR.to_string(), 1.0));
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
        let mut best_pt = universe[0];
        let mut best_gain = 0usize;
        for &cand in &universe {
            let arr = cand.as_array();
            let gain = target_pairs
                .iter()
                .enumerate()
                .filter(|(idx, &(i, vi, j, vj))| uncovered[*idx] && arr[i] == vi && arr[j] == vj)
                .count();
            if gain > best_gain {
                best_gain = gain;
                best_pt = cand;
            }
        }
        assert!(
            best_gain > 0,
            "covering search must strictly reduce uncovered tuples"
        );
        chosen.push(best_pt);
        mark_covered(best_pt, &mut uncovered);
    }

    chosen
}

fn obs_val(obs: &[(String, f64)], key: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("missing observable '{key}'"))
        .1
}

#[derive(Clone, Debug, Default)]
struct FlowCounts {
    births: u64,
    deaths: u64,
    deaths_starv: u64,
    deaths_dehyd: u64,
    deaths_other: u64,
    first_death_tick: Option<u64>,
    pred_strikes: u64,
    pred_eu: f64,
    first_pred_tick: Option<u64>,
}

fn snapshot_biomass_eu(adapter: &LiveExperimentAdapter) -> f64 {
    let world = adapter.world();
    let pool = world.resource::<EcosystemBiomass>();
    closed_total_eu(pool.plants, pool.animals, pool.detritus, pool.dormant)
}

fn read_plants_gap(adapter: &LiveExperimentAdapter) -> f64 {
    let world = adapter.world();
    let carried = world.resource::<EcosystemBiomass>().plants;
    let actual = world.resource::<ResourceField>().total_biomass();
    (carried - actual).abs()
}

fn accumulate_tick_flows(
    adapter: &LiveExperimentAdapter,
    tick: u64,
    flows: &mut FlowCounts,
    window_offset_births: u64,
    window_offset_deaths: u64,
) {
    let world = adapter.world();
    if let Some(v) = world.get_resource::<VitalStatistics>() {
        flows.births = window_offset_births + v.total_births;
        flows.deaths = window_offset_deaths + v.total_deaths;
        flows.deaths_starv = v.total_deaths_starvation;
        flows.deaths_dehyd = v.total_deaths_dehydration;
        flows.deaths_other = flows
            .deaths
            .saturating_sub(flows.deaths_starv + flows.deaths_dehyd);
        if flows.first_death_tick.is_none() && flows.deaths > 0 {
            flows.first_death_tick = Some(tick);
        }
    }
    if let Some(combat) = world.get_resource::<CombatEvents>() {
        if !combat.events.is_empty() {
            flows.pred_strikes += combat.events.len() as u64;
            flows.pred_eu += combat
                .events
                .iter()
                .map(|e| e.energy_transferred as f64)
                .sum::<f64>();
            if flows.first_pred_tick.is_none() {
                flows.first_pred_tick = Some(tick);
            }
        }
    }
}

struct ResidualMetrics {
    eu_0: f64,
    eu_1: f64,
    eu_final: f64,
    delta_t0: f64,
    delta_t1: f64,
    worst_abs_t1: f64,
    plants_field_gap: f64,
    ledger_effects: usize,
    flows: FlowCounts,
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
    let mut max_plants_gap = read_plants_gap(&adapter);
    let mut flows = FlowCounts::default();

    for t in 1..=run_ticks {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        adapter.step(&clock, &active, &mut causal, &mut rng);
        accumulate_tick_flows(&adapter, t, &mut flows, 0, 0);

        let eu_t = if t == 1 || t == run_ticks {
            let obs = adapter.observables();
            max_plants_gap = max_plants_gap.max(read_plants_gap(&adapter));
            obs_val(&obs, "live.closed_eu_total")
        } else {
            if t.is_multiple_of(PLANT_GAP_STRIDE) {
                max_plants_gap = max_plants_gap.max(read_plants_gap(&adapter));
            }
            snapshot_biomass_eu(&adapter)
        };
        if t == 1 {
            eu_1 = eu_t;
        }
        worst_abs_t1 = worst_abs_t1.max((eu_t - eu_1).abs());
        if t == run_ticks {
            eu_final = eu_t;
        }
    }

    ResidualMetrics {
        eu_0,
        eu_1,
        eu_final,
        delta_t0: eu_final - eu_0,
        delta_t1: eu_final - eu_1,
        worst_abs_t1,
        plants_field_gap: max_plants_gap,
        ledger_effects: causal.len(),
        flows,
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

fn assert_non_vacuous_policy(label: &str, pt: PolicyPoint, f: &FlowCounts) {
    if pt.reproduction == 1 {
        assert!(f.births >= 1, "{label}: reproduction=1 exercised 0 births");
    } else {
        assert_eq!(f.births, 0, "{label}: reproduction=0 must produce 0 births");
    }
    if pt.mortality == 1 {
        assert!(
            f.deaths >= 1 && f.first_death_tick.is_some(),
            "{label}: mortality=1 exercised 0 deaths"
        );
    } else {
        assert_eq!(f.deaths, 0, "{label}: mortality=0 must produce 0 deaths");
    }
    if pt.predator_income == 1 && pt.locomotion == 1 {
        assert!(
            f.pred_strikes >= 1 && f.pred_eu > 0.0,
            "{label}: k=1,l=1 exercised 0 predation transfers"
        );
    }
}

fn fmt_tick_opt(t: Option<u64>) -> String {
    match t {
        Some(v) => v.to_string(),
        None => "-".to_string(),
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

fn intervention_cases_for(horizon: u64) -> Vec<(&'static str, Vec<InterventionCommand>)> {
    let fire_early = 30.min(horizon / 2).max(1);
    let fire_mid = if horizon >= 1_000 { 120 } else { 60 };
    let dur = 45.min(horizon.saturating_sub(fire_early)).max(1);

    let single = vec![
        (
            "RainfallDelta",
            vec![cmd_with(
                1,
                InterventionKind::RainfallDelta,
                fire_early,
                dur,
                0.5,
            )],
        ),
        (
            "TemperatureDelta",
            vec![cmd_with(
                2,
                InterventionKind::TemperatureDelta,
                fire_early,
                dur,
                0.5,
            )],
        ),
        (
            "Deforest",
            vec![cmd_with(3, InterventionKind::Deforest, fire_mid, 1, 0.5)],
        ),
        (
            "AddNutrient",
            vec![cmd_with(4, InterventionKind::AddNutrient, fire_mid, 1, 1.0)],
        ),
        (
            "RemovePredators",
            vec![cmd_with(
                5,
                InterventionKind::RemovePredators,
                fire_mid,
                1,
                0.25,
            )],
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
                fire_mid,
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
                fire_mid,
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
                fire_mid,
                1,
                1.0,
            )],
        ),
    ];

    let combined = vec![
        cmd_with(11, InterventionKind::RainfallDelta, fire_early, dur, 0.5),
        cmd_with(12, InterventionKind::TemperatureDelta, fire_early, dur, 0.5),
        cmd_with(13, InterventionKind::Deforest, fire_mid, 1, 0.35),
        cmd_with(14, InterventionKind::AddNutrient, fire_mid, 1, 1.0),
        cmd_with(15, InterventionKind::RemovePredators, fire_mid, 1, 0.25),
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
            fire_mid,
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
            fire_mid,
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
            fire_mid,
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

struct DriftPairMeasurement {
    eu_0: f64,
    sel_worst: f64,
    drf_worst: f64,
    sel_flows: FlowCounts,
    drf_flows: FlowCounts,
    diag: DriftDiagnostics,
    drf_ledger_effects: usize,
    drf_final_checksum: u32,
    sel_final_checksum: u32,
}

fn run_drift_pair_case(
    label: &str,
    pt: PolicyPoint,
    reset_ticks: u64,
    interventions: Vec<InterventionCommand>,
    duration_ticks: u64,
) -> DriftPairMeasurement {
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

    let m_sel = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: format!("m4-{label}-sel"),
        name: label.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: sel_init.clone(),
        interventions: interventions.clone(),
        seeds: vec![SEED],
        duration_ticks,
        sample_period: duration_ticks,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let m_drf = ExperimentManifest {
        experiment_id: format!("m4-{label}-drf"),
        initial_conditions: drf_init.clone(),
        ..m_sel.clone()
    };

    let window_ticks = if reset_ticks == 0 {
        duration_ticks
    } else {
        reset_ticks
    };
    assert!(duration_ticks.is_multiple_of(window_ticks));
    let window_count = (duration_ticks / window_ticks) as usize;

    let mut sel_adapter = LiveExperimentAdapter::from_manifest(
        &m_sel.laws,
        &sel_init,
        &[],
        SEED,
        (16, 16),
        duration_ticks,
    )
    .expect("selected adapter builds");
    pt.apply_runtime_brain_policy(&mut sel_adapter);
    sel_adapter.install_turnover_recorder(duration_ticks);

    let obs0 = sel_adapter.observables();
    let eu_0 = obs_val(&obs0, "live.closed_eu_total")
        + obs_val(&obs0, "live.agent_count") * obs_val(&obs0, "live.mean_agent_energy");

    let sel_queue = InterventionQueue::new(interventions.clone());
    let drf_queue = InterventionQueue::new(interventions);
    let mut sel_rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut sel_clock = SimClock::new();
    let mut sel_ledger = CausalLedger::new();

    let mut drf_rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut drf_clock = SimClock::new();
    let mut drf_ledger = CausalLedger::new();
    let mut carried_drift_rng = derived_sim_rng(SEED, sim_stream::LIVE_DRIFT_SELECTION);
    let mut merged_diag = DriftDiagnostics::default();

    let mut sel_eu_1 = 0.0;
    let mut drf_eu_1 = 0.0;
    let mut sel_worst = 0.0f64;
    let mut drf_worst = 0.0f64;
    let mut sel_flows = FlowCounts::default();
    let mut drf_flows = FlowCounts::default();
    let mut drf_final_checksum = 0u32;

    for w in 0..window_count {
        let w_start = (w as u64) * window_ticks;
        let w_end = w_start + window_ticks;
        let snap = if w > 0 {
            Some(sel_adapter.snapshot())
        } else {
            None
        };

        for t_in_w in 1..=window_ticks {
            let t = w_start + t_in_w;
            let tick = sel_clock.advance();
            let active: Vec<&InterventionCommand> = sel_queue.active_at(tick).collect();
            sel_adapter.step(&sel_clock, &active, &mut sel_ledger, &mut sel_rng);
            accumulate_tick_flows(&sel_adapter, t, &mut sel_flows, 0, 0);

            let eu_t = if t == 1 || t == duration_ticks || t.is_multiple_of(PLANT_GAP_STRIDE) {
                sel_worst = sel_worst.max(read_plants_gap(&sel_adapter));
                obs_val(&sel_adapter.observables(), "live.closed_eu_total")
            } else {
                snapshot_biomass_eu(&sel_adapter)
            };
            if t == 1 {
                sel_eu_1 = eu_t;
            }
            sel_worst = sel_worst
                .max((eu_t - sel_eu_1).abs())
                .max((eu_t - eu_0).abs());
        }

        let full_sched = sel_adapter
            .take_turnover_schedule()
            .expect("recorder active on selected adapter");
        let win_sched = full_sched.slice_window(w_start as usize, w_end as usize);

        let mut drf_adapter = match snap {
            None => LiveExperimentAdapter::from_manifest_with_drift_schedule(
                &m_drf.laws,
                &drf_init,
                &[],
                SEED,
                m_drf.fingerprint(),
                registry.fingerprint(),
                &win_sched,
            )
            .expect("drift window 0 builds"),
            Some(ref s) => LiveExperimentAdapter::from_snapshot_with_drift_schedule(
                s,
                &m_drf.laws,
                &drf_init,
                m_drf.fingerprint(),
                registry.fingerprint(),
                &win_sched,
                carried_drift_rng,
            )
            .expect("drift snapshot window builds"),
        };
        pt.apply_runtime_brain_policy(&mut drf_adapter);

        let base_drf_births = drf_flows.births;
        let base_drf_deaths = drf_flows.deaths;
        for t_in_w in 1..=window_ticks {
            let t = w_start + t_in_w;
            let tick = drf_clock.advance();
            let active: Vec<&InterventionCommand> = drf_queue.active_at(tick).collect();
            drf_adapter.step(&drf_clock, &active, &mut drf_ledger, &mut drf_rng);
            accumulate_tick_flows(
                &drf_adapter,
                t,
                &mut drf_flows,
                base_drf_births,
                base_drf_deaths,
            );

            let eu_t = if t == 1 || t == duration_ticks || t.is_multiple_of(PLANT_GAP_STRIDE) {
                drf_worst = drf_worst.max(read_plants_gap(&drf_adapter));
                obs_val(&drf_adapter.observables(), "live.closed_eu_total")
            } else {
                snapshot_biomass_eu(&drf_adapter)
            };
            if t == 1 {
                drf_eu_1 = eu_t;
            }
            drf_worst = drf_worst
                .max((eu_t - drf_eu_1).abs())
                .max((eu_t - eu_0).abs());
        }

        carried_drift_rng = drf_adapter
            .drift_rng_clone()
            .expect("drift RNG present on drift adapter");
        if let Some(diag) = drf_adapter.drift_diagnostics() {
            merged_diag.merge_window(&diag);
        }
        if w + 1 == window_count {
            drf_final_checksum = drf_adapter.checksum();
        }
    }

    let sel_final_checksum = sel_adapter.checksum();
    assert!(
        sel_worst <= RESIDUAL_ABS_TOLERANCE_EU && drf_worst <= RESIDUAL_ABS_TOLERANCE_EU,
        "{label}: drift pair EU exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU:e} \
         (sel_worst={sel_worst:.3e}, drf_worst={drf_worst:.3e}, eu_0={eu_0:.6})"
    );
    assert!(
        sel_flows.births >= 1 && sel_flows.deaths >= 1,
        "{label}: selected arm had vacuous turnover (births={}, deaths={})",
        sel_flows.births,
        sel_flows.deaths
    );
    assert!(
        merged_diag.actual_births >= 1 && merged_diag.actual_deaths >= 1 && drf_flows.deaths >= 1,
        "{label}: drift arm replayed vacuous turnover (actual_births={}, actual_deaths={}, vitals_deaths={})",
        merged_diag.actual_births,
        merged_diag.actual_deaths,
        drf_flows.deaths
    );

    DriftPairMeasurement {
        eu_0,
        sel_worst,
        drf_worst,
        sel_flows,
        drf_flows,
        diag: merged_diag,
        drf_ledger_effects: drf_ledger.len(),
        drf_final_checksum,
        sel_final_checksum,
    }
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
    let started = Instant::now();
    let design = pairwise_covering_design();
    let mut global_worst = 0.0f64;

    for (idx, &pt) in design.iter().enumerate() {
        let label = pt.label();
        let m = measure_single_run(pt, &[], Vec::new(), POLICY_TICKS);
        assert_non_vacuous_policy(&label, pt, &m.flows);
        println!(
            "M4_PAIR[{idx:02}] {label:<24} eu_0={:.3} d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={}(s{},h{},d1={}) p={}({:.1})",
            m.eu_0,
            m.delta_t0,
            m.delta_t1,
            m.worst_abs_t1,
            m.plants_field_gap,
            m.flows.births,
            m.flows.deaths,
            m.flows.deaths_starv,
            m.flows.deaths_dehyd,
            fmt_tick_opt(m.flows.first_death_tick),
            m.flows.pred_strikes,
            m.flows.pred_eu,
        );
        global_worst = global_worst
            .max(m.delta_t0.abs())
            .max(m.delta_t1.abs())
            .max(m.worst_abs_t1)
            .max(m.plants_field_gap);
        assert_residual_ok(&label, &m);
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
    let started = Instant::now();
    let baseline = PolicyPoint::from_array([0, 0, 0, 0, 0, 0, 0]);
    let full_sel2 = PolicyPoint::from_array([1, 1, 1, 1, 1, 2, 1]);
    let mut global_worst = 0.0f64;
    let mut runs = 0usize;

    for (name, cmds) in intervention_cases_for(BASELINE_INTERVENTION_TICKS) {
        let label = format!("baseline/{name}");
        let m = measure_single_run(baseline, &[], cmds, BASELINE_INTERVENTION_TICKS);
        let min_effects = if name == "All8Combined" { 8 } else { 1 };
        assert!(
            m.ledger_effects >= min_effects,
            "{label}: expected >= {min_effects} causal ledger effect(s), got {}",
            m.ledger_effects
        );
        assert_non_vacuous_policy(&label, baseline, &m.flows);
        println!(
            "M4_INT {label:<34} eff={} d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={} p={}",
            m.ledger_effects,
            m.delta_t0,
            m.delta_t1,
            m.worst_abs_t1,
            m.plants_field_gap,
            m.flows.births,
            m.flows.deaths,
            m.flows.pred_strikes,
        );
        global_worst = global_worst
            .max(m.delta_t0.abs())
            .max(m.delta_t1.abs())
            .max(m.worst_abs_t1)
            .max(m.plants_field_gap);
        assert_residual_ok(&label, &m);
        runs += 1;
    }

    let sel2_focus = [
        "Deforest",
        "RemovePredators",
        "ToxicityPlume",
        "PathogenTransmission",
        "All8Combined",
    ];
    for (name, cmds) in intervention_cases_for(POLICY_TICKS) {
        if !sel2_focus.contains(&name) {
            continue;
        }
        let label = format!("sel2_full/{name}");
        let m = measure_single_run(full_sel2, &[], cmds, POLICY_TICKS);
        let min_effects = if name == "All8Combined" { 8 } else { 1 };
        assert!(
            m.ledger_effects >= min_effects,
            "{label}: expected >= {min_effects} causal ledger effect(s), got {}",
            m.ledger_effects
        );
        assert_non_vacuous_policy(&label, full_sel2, &m.flows);
        println!(
            "M4_INT {label:<34} eff={} d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={}(s{},h{},d1={}) p={}({:.1})",
            m.ledger_effects,
            m.delta_t0,
            m.delta_t1,
            m.worst_abs_t1,
            m.plants_field_gap,
            m.flows.births,
            m.flows.deaths,
            m.flows.deaths_starv,
            m.flows.deaths_dehyd,
            fmt_tick_opt(m.flows.first_death_tick),
            m.flows.pred_strikes,
            m.flows.pred_eu,
        );
        global_worst = global_worst
            .max(m.delta_t0.abs())
            .max(m.delta_t1.abs())
            .max(m.worst_abs_t1)
            .max(m.plants_field_gap);
        assert_residual_ok(&label, &m);
        runs += 1;
    }

    println!(
        "M4_INTERVENTIONS_SUMMARY runs={runs} elapsed_s={:.2} max_abs_residual={:.3e}",
        started.elapsed().as_secs_f64(),
        global_worst
    );
}

#[test]
fn drift_control_pairs_and_excluded_keys_conserve_closed_eu() {
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
            "k800_nores_shared",
            PolicyPoint::from_array([1, 1, 1, 0, 0, 0, 0]),
            DRIFT_RESET_K,
        ),
        (
            "k800_res30_evolved",
            PolicyPoint::from_array([1, 1, 1, 1, 1, 1, 1]),
            DRIFT_RESET_K,
        ),
    ];

    let mut shared_check_pair = None;
    for (label, pt, k) in drift_cases {
        let m = run_drift_pair_case(label, pt, k, Vec::new(), DRIFT_TICKS);
        if label == "k800_nores_shared" {
            shared_check_pair = Some((m.sel_final_checksum, m.drf_final_checksum, m.diag.clone()));
        }
        println!(
            "M4_DRIFT {label:<22} k={k:<4} eu_0={:.3} sel_w={:.3e} drf_w={:.3e} sel_b={} sel_d={}(s{},h{}) drf_b={} drf_d={}(o{}) pred=({},{})",
            m.eu_0,
            m.sel_worst,
            m.drf_worst,
            m.sel_flows.births,
            m.sel_flows.deaths,
            m.sel_flows.deaths_starv,
            m.sel_flows.deaths_dehyd,
            m.diag.actual_births,
            m.diag.actual_deaths,
            m.drf_flows.deaths_other,
            m.sel_flows.pred_strikes,
            m.drf_flows.pred_strikes,
        );
        global_worst = global_worst.max(m.sel_worst).max(m.drf_worst);
    }

    // Verify direct window-stepped driver matches `run_live_drift_pair` bit-for-bit on shared policy.
    let (sel_sum, drf_sum, ref_diag) = shared_check_pair.expect("k800_nores_shared measured");
    let parity_pt = PolicyPoint::from_array([1, 1, 1, 0, 0, 0, 0]);
    let registry = ObservableRegistry::live_default();
    let m_sel = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "m4-parity-sel".to_string(),
        name: "m4-parity".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: parity_pt.to_initial(&[]),
        interventions: Vec::new(),
        seeds: vec![SEED],
        duration_ticks: DRIFT_TICKS,
        sample_period: DRIFT_TICKS,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let m_drf = ExperimentManifest {
        experiment_id: "m4-parity-drf".to_string(),
        initial_conditions: parity_pt.to_initial(&[
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
            (LIVE_KEY_DRIFT_RESET_TICKS, DRIFT_RESET_K as f64),
        ]),
        ..m_sel.clone()
    };
    let api_pair =
        run_live_drift_pair(&m_sel, &m_drf, &registry, SEED).expect("run_live_drift_pair succeeds");
    assert_eq!(api_pair.selected.status, RunStatus::Completed);
    assert_eq!(api_pair.drift.status, RunStatus::Completed);
    assert_eq!(api_pair.selected.final_checksum, sel_sum);
    assert_eq!(api_pair.drift.final_checksum, drf_sum);
    assert_eq!(api_pair.diagnostics.actual_births, ref_diag.actual_births);
    assert_eq!(api_pair.diagnostics.actual_deaths, ref_diag.actual_deaths);

    let drift_full = PolicyPoint::from_array([1, 1, 1, 1, 1, 1, 1]);
    let (_, all8_cmds) = intervention_cases_for(DRIFT_TICKS)
        .into_iter()
        .find(|(n, _)| *n == "All8Combined")
        .expect("All8Combined present");
    let dlabel = format!("drift_k{DRIFT_RESET_K}/All8Combined");
    let dm = run_drift_pair_case(&dlabel, drift_full, DRIFT_RESET_K, all8_cmds, DRIFT_TICKS);
    assert!(
        dm.drf_ledger_effects >= 8 && dm.sel_flows.pred_strikes >= 1,
        "{dlabel}: expected >= 8 ledger effects and predation, got eff={} pred={}",
        dm.drf_ledger_effects,
        dm.sel_flows.pred_strikes
    );
    println!(
        "M4_DRIFT_INT {dlabel:<34} eff={} sel_w={:.3e} drf_w={:.3e} sel_b={} sel_d={} drf_b={} drf_d={}(o{}) p=({},{})",
        dm.drf_ledger_effects,
        dm.sel_worst,
        dm.drf_worst,
        dm.sel_flows.births,
        dm.sel_flows.deaths,
        dm.diag.actual_births,
        dm.diag.actual_deaths,
        dm.drf_flows.deaths_other,
        dm.sel_flows.pred_strikes,
        dm.drf_flows.pred_strikes,
    );
    global_worst = global_worst.max(dm.sel_worst).max(dm.drf_worst);

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
        POLICY_TICKS,
    );
    assert_non_vacuous_policy("excluded_guard", excluded_pt, &excl.flows);
    println!(
        "M4_EXCLUDED_GUARD d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={}(s{},h{},d1={}) p={}({:.1})",
        excl.delta_t0,
        excl.delta_t1,
        excl.worst_abs_t1,
        excl.plants_field_gap,
        excl.flows.births,
        excl.flows.deaths,
        excl.flows.deaths_starv,
        excl.flows.deaths_dehyd,
        fmt_tick_opt(excl.flows.first_death_tick),
        excl.flows.pred_strikes,
        excl.flows.pred_eu,
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
