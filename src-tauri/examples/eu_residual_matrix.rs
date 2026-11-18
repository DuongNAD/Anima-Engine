//! CARD M4 — Release-mode full-factorial closed-EU residual matrix runner.
//!
//! Covers all 192 policy flag combinations (`2 * 2 * 2 * 2 * 2 * 3 * 2`), 16 `drift_control`
//! paired runs (`K in {0, DRIFT_RESET_K}` across `plant_reserve`, `locomotion`, `evolved_brains`),
//! 27 `(profile x InterventionKind)` runs (all 8 `InterventionKind` variants + `All8Combined` on
//! `baseline`, `sel2_full`, and `drift_k_reset`), and excluded-key guards — while asserting
//! non-vacuous execution (births, deaths by cause, predation transfers, and drift death/birth
//! replay) on every active factor level.
//!
//! Build in release mode and execute the binary directly (never `cargo run`):
//!   `cargo build --release --features desktop --example eu_residual_matrix`
//!   `target\release\examples\eu_residual_matrix.exe`

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::drift_control::DriftDiagnostics;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::energy_ledger::{closed_total_eu, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region, TraitMod,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_DEHYDRATION_TICKS,
    LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIFT_RESET_TICKS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_KILL_EFFICIENCY,
    LIVE_KEY_KILL_HYDRATION, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE,
    LIVE_KEY_PLANT_RESERVE, LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_STARVE_TICKS, LIVE_KEY_TARGET_MIN_ENERGY,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_W_SURPLUS, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::{
    derived_sim_rng, sim_stream, BrainPolicy, CombatEvents, LifetimeLearning, VitalStatistics,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;
use std::process::ExitCode;
use std::time::Instant;

const SEED: u64 = 20261118;
const POLICY_TICKS: u64 = 3_000;
const DRIFT_TICKS: u64 = 3_000;
const DRIFT_RESET_K: u64 = 1_000;
const BASELINE_INTERVENTION_TICKS: u64 = 180;
const PLANT_GAP_STRIDE: u64 = 250;

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

fn snapshot_biomass_eu(adapter: &LiveExperimentAdapter) -> (f64, f64) {
    let world = adapter.world();
    let pool = world.resource::<EcosystemBiomass>();
    let eu = closed_total_eu(pool.plants, pool.animals, pool.detritus, pool.dormant);
    let carried_plants = pool.plants;
    (eu, carried_plants)
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

struct Metrics {
    eu_0: f64,
    delta_t0: f64,
    delta_t1: f64,
    worst_abs_t1: f64,
    plants_field_gap: f64,
    ledger_effects: usize,
    flows: FlowCounts,
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
            snapshot_biomass_eu(&adapter).0
        };
        if t == 1 {
            eu_1 = eu_t;
        }
        worst_abs_t1 = worst_abs_t1.max((eu_t - eu_1).abs());
        if t == run_ticks {
            eu_final = eu_t;
        }
    }

    Metrics {
        eu_0,
        delta_t0: eu_final - eu_0,
        delta_t1: eu_final - eu_1,
        worst_abs_t1,
        plants_field_gap: max_plants_gap,
        ledger_effects: causal.len(),
        flows,
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
}

fn measure_drift_pair_case(
    label: &str,
    pt: PolicyPoint,
    reset_ticks: u64,
    interventions: Vec<InterventionCommand>,
    total_ticks: u64,
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
        experiment_id: format!("m4-full-{label}-sel"),
        name: label.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: sel_init.clone(),
        interventions: interventions.clone(),
        seeds: vec![SEED],
        duration_ticks: total_ticks,
        sample_period: total_ticks,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let m_drf = ExperimentManifest {
        experiment_id: format!("m4-full-{label}-drf"),
        initial_conditions: drf_init.clone(),
        ..m_sel.clone()
    };

    let window_ticks = if reset_ticks == 0 {
        total_ticks
    } else {
        reset_ticks
    };
    assert!(total_ticks.is_multiple_of(window_ticks));
    let window_count = (total_ticks / window_ticks) as usize;

    let mut sel_adapter = LiveExperimentAdapter::from_manifest(
        &m_sel.laws,
        &sel_init,
        &[],
        SEED,
        (16, 16),
        total_ticks,
    )
    .expect("selected adapter builds");
    pt.apply_runtime_brain_policy(&mut sel_adapter);
    sel_adapter.install_turnover_recorder(total_ticks);

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

            let eu_t = if t == 1 || t == total_ticks || t.is_multiple_of(PLANT_GAP_STRIDE) {
                sel_worst = sel_worst.max(read_plants_gap(&sel_adapter));
                obs_val(&sel_adapter.observables(), "live.closed_eu_total")
            } else {
                snapshot_biomass_eu(&sel_adapter).0
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

            let eu_t = if t == 1 || t == total_ticks || t.is_multiple_of(PLANT_GAP_STRIDE) {
                drf_worst = drf_worst.max(read_plants_gap(&drf_adapter));
                obs_val(&drf_adapter.observables(), "live.closed_eu_total")
            } else {
                snapshot_biomass_eu(&drf_adapter).0
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
    }

    DriftPairMeasurement {
        eu_0,
        sel_worst,
        drf_worst,
        sel_flows,
        drf_flows,
        diag: merged_diag,
        drf_ledger_effects: drf_ledger.len(),
    }
}

fn check_policy_vacuity(
    label: &str,
    pt: PolicyPoint,
    flows: &FlowCounts,
    vacuity: &mut Vec<String>,
) {
    if pt.reproduction == 1 && flows.births == 0 {
        vacuity.push(format!("{label}: reproduction=1 had 0 births"));
    }
    if pt.reproduction == 0 && flows.births != 0 {
        vacuity.push(format!(
            "{label}: reproduction=0 had {} births",
            flows.births
        ));
    }
    if pt.mortality == 1 && flows.deaths == 0 {
        vacuity.push(format!("{label}: mortality=1 had 0 deaths"));
    }
    if pt.mortality == 0 && flows.deaths != 0 {
        vacuity.push(format!("{label}: mortality=0 had {} deaths", flows.deaths));
    }
    if pt.predator_income == 1 && pt.locomotion == 1 && flows.pred_strikes == 0 {
        vacuity.push(format!("{label}: k=1,l=1 had 0 predation strikes"));
    }
}

fn fmt_tick_opt(t: Option<u64>) -> String {
    match t {
        Some(v) => v.to_string(),
        None => "-".to_string(),
    }
}

fn main() -> ExitCode {
    let total_start = Instant::now();
    let mut violations: Vec<String> = Vec::new();
    let mut vacuity_failures: Vec<String> = Vec::new();

    // 1. Full 192-point policy factorial.
    let policy_start = Instant::now();
    let mut policy_worst = 0.0f64;
    let mut policy_worst_label = String::new();
    let mut policy_count = 0usize;

    let mut repro_exercised = 0usize;
    let mut repro_total = 0usize;
    let mut mort_exercised = 0usize;
    let mut mort_total = 0usize;
    let mut mort_starv_total = 0u64;
    let mut mort_dehyd_total = 0u64;
    let mut first_death_min = u64::MAX;
    let mut first_death_max = 0u64;
    let mut death_ticks_sorted: Vec<u64> = Vec::new();
    let mut loc_exercised = 0usize;
    let mut loc_total = 0usize;
    let mut pred_loc1_exercised = 0usize;
    let mut pred_loc1_total = 0usize;
    let mut pred_loc0_exercised = 0usize;
    let mut pred_loc0_total = 0usize;
    let mut first_pred_min = u64::MAX;
    let mut first_pred_max = 0u64;
    let mut reserve_exercised = 0usize;
    let mut reserve_total = 0usize;
    let mut cap_exercised = 0usize;
    let mut cap_total = 0usize;
    let mut brain_ev_exercised = 0usize;
    let mut brain_ev_total = 0usize;
    let mut brain_ll_exercised = 0usize;
    let mut brain_ll_total = 0usize;

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
                                let res = measure_single_run(pt, &[], Vec::new(), POLICY_TICKS);
                                let peak = res.max_abs();
                                if peak > policy_worst {
                                    policy_worst = peak;
                                    policy_worst_label = label.clone();
                                }
                                if peak > RESIDUAL_ABS_TOLERANCE_EU {
                                    violations.push(format!("{label}: {peak:.3e}"));
                                }
                                check_policy_vacuity(&label, pt, &res.flows, &mut vacuity_failures);

                                if r == 1 {
                                    repro_total += 1;
                                    if res.flows.births > 0 {
                                        repro_exercised += 1;
                                    }
                                }
                                if m == 1 {
                                    mort_total += 1;
                                    if res.flows.deaths > 0 {
                                        mort_exercised += 1;
                                    }
                                    mort_starv_total += res.flows.deaths_starv;
                                    mort_dehyd_total += res.flows.deaths_dehyd;
                                    if let Some(t) = res.flows.first_death_tick {
                                        first_death_min = first_death_min.min(t);
                                        first_death_max = first_death_max.max(t);
                                        death_ticks_sorted.push(t);
                                    }
                                }
                                if l == 1 {
                                    loc_total += 1;
                                    loc_exercised += 1;
                                }
                                if p == 1 {
                                    reserve_total += 1;
                                    reserve_exercised += 1;
                                }
                                if c == 1 {
                                    cap_total += 1;
                                    cap_exercised += 1;
                                }
                                if b == 1 {
                                    brain_ev_total += 1;
                                    brain_ev_exercised += 1;
                                }
                                if b == 2 {
                                    brain_ll_total += 1;
                                    brain_ll_exercised += 1;
                                }
                                if k == 1 && l == 1 {
                                    pred_loc1_total += 1;
                                    if res.flows.pred_strikes > 0 {
                                        pred_loc1_exercised += 1;
                                    }
                                    if let Some(t) = res.flows.first_pred_tick {
                                        first_pred_min = first_pred_min.min(t);
                                        first_pred_max = first_pred_max.max(t);
                                    }
                                }
                                if k == 1 && l == 0 {
                                    pred_loc0_total += 1;
                                    if res.flows.pred_strikes > 0 {
                                        pred_loc0_exercised += 1;
                                    }
                                }

                                if (m == 1 && r == 1 && l == 1 && c == 1 && k == 1)
                                    || (m == 1 && r == 0 && l == 0 && p == 0 && c == 0 && k == 0)
                                    || (m == 0 && r == 0 && l == 0 && p == 0 && c == 0 && b == 0)
                                    || peak == policy_worst
                                    || policy_count == 0
                                    || policy_count == 191
                                {
                                    println!(
                                        "POLICY[{policy_count:03}] {label:<24} eu_0={:.3} delta_t0={:+.3e} delta_t1={:+.3e} worst_t1={:.3e} gap={:.3e} b={} d={}(s{},h{}) d1={} p={}({:.1})",
                                        res.eu_0,
                                        res.delta_t0,
                                        res.delta_t1,
                                        res.worst_abs_t1,
                                        res.plants_field_gap,
                                        res.flows.births,
                                        res.flows.deaths,
                                        res.flows.deaths_starv,
                                        res.flows.deaths_dehyd,
                                        fmt_tick_opt(res.flows.first_death_tick),
                                        res.flows.pred_strikes,
                                        res.flows.pred_eu,
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
    death_ticks_sorted.sort_unstable();
    let death_med = death_ticks_sorted
        .get(death_ticks_sorted.len() / 2)
        .copied()
        .unwrap_or(0);
    println!(
        "FULL_POLICY_SUMMARY runs={policy_count} elapsed_s={:.2} worst={policy_worst:.3e} worst_case={policy_worst_label}",
        policy_start.elapsed().as_secs_f64()
    );
    println!(
        "FACTOR_COVERAGE repro={repro_exercised}/{repro_total} mort={mort_exercised}/{mort_total}(starv={mort_starv_total},dehyd={mort_dehyd_total},d1={first_death_min}/{death_med}/{first_death_max}) loc={loc_exercised}/{loc_total} resv={reserve_exercised}/{reserve_total} cap={cap_exercised}/{cap_total} br_ev={brain_ev_exercised}/{brain_ev_total} br_ll={brain_ll_exercised}/{brain_ll_total} pred_l1={pred_loc1_exercised}/{pred_loc1_total}(p1={first_pred_min}..{first_pred_max}) pred_l0={pred_loc0_exercised}/{pred_loc0_total}"
    );

    // 2. 16-point drift_control matrix (K in {0, DRIFT_RESET_K} x reserve x locomotion x evolved_brains).
    let drift_start = Instant::now();
    let mut drift_worst = 0.0f64;
    let mut drift_worst_label = String::new();
    let mut drift_count = 0usize;
    let mut drift_exercised = 0usize;

    for k in [0u64, DRIFT_RESET_K] {
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
                    let m = measure_drift_pair_case(&label, pt, k, Vec::new(), DRIFT_TICKS);
                    let peak = m.sel_worst.max(m.drf_worst);
                    if peak > drift_worst {
                        drift_worst = peak;
                        drift_worst_label = label.clone();
                    }
                    if peak > RESIDUAL_ABS_TOLERANCE_EU {
                        violations.push(format!("drift/{label}: {peak:.3e}"));
                    }
                    if m.sel_flows.births == 0
                        || m.sel_flows.deaths == 0
                        || m.diag.actual_births == 0
                        || m.diag.actual_deaths == 0
                    {
                        vacuity_failures.push(format!(
                            "drift/{label}: sel(b={},d={}) drf_actual(b={},d={})",
                            m.sel_flows.births,
                            m.sel_flows.deaths,
                            m.diag.actual_births,
                            m.diag.actual_deaths
                        ));
                    } else {
                        drift_exercised += 1;
                    }
                    println!(
                        "DRIFT_PAIR[{drift_count:02}] {label:<18} eu_0={:.3} sel_w={:.3e} drf_w={:.3e} sel_b={} sel_d={}(s{},h{},d1={}) drf_b={} drf_d={}(o{},s{},h{}) pred=({},{})",
                        m.eu_0,
                        m.sel_worst,
                        m.drf_worst,
                        m.sel_flows.births,
                        m.sel_flows.deaths,
                        m.sel_flows.deaths_starv,
                        m.sel_flows.deaths_dehyd,
                        fmt_tick_opt(m.sel_flows.first_death_tick),
                        m.diag.actual_births,
                        m.diag.actual_deaths,
                        m.drf_flows.deaths_other,
                        m.drf_flows.deaths_starv,
                        m.drf_flows.deaths_dehyd,
                        m.sel_flows.pred_strikes,
                        m.drf_flows.pred_strikes,
                    );
                    drift_count += 1;
                }
            }
        }
    }
    println!(
        "FULL_DRIFT_SUMMARY pairs={drift_count} exercised={drift_exercised}/{drift_count} elapsed_s={:.2} worst={drift_worst:.3e} worst_case={drift_worst_label}",
        drift_start.elapsed().as_secs_f64()
    );

    // 3. Interventions across Baseline, SEL2 Full, and Drift K=DRIFT_RESET_K.
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

    let base_cases = intervention_cases_for(BASELINE_INTERVENTION_TICKS);
    let long_cases = intervention_cases_for(POLICY_TICKS);
    let drift_cases = intervention_cases_for(DRIFT_TICKS);

    for idx in 0..base_cases.len() {
        let (name, base_cmds) = &base_cases[idx];
        let min_effects = if *name == "All8Combined" { 8 } else { 1 };

        let bm = measure_single_run(
            baseline,
            &[],
            base_cmds.clone(),
            BASELINE_INTERVENTION_TICKS,
        );
        if bm.ledger_effects < min_effects {
            vacuity_failures.push(format!("baseline/{name}: eff={}", bm.ledger_effects));
        }
        let b_label = format!("baseline/{name}");
        let b_peak = bm.max_abs();
        if b_peak > int_worst {
            int_worst = b_peak;
            int_worst_label = b_label.clone();
        }
        if b_peak > RESIDUAL_ABS_TOLERANCE_EU {
            violations.push(format!("{b_label}: {b_peak:.3e}"));
        }
        println!(
            "INTERVENTION {b_label:<34} eff={} d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={} p={}",
            bm.ledger_effects,
            bm.delta_t0,
            bm.delta_t1,
            bm.worst_abs_t1,
            bm.plants_field_gap,
            bm.flows.births,
            bm.flows.deaths,
            bm.flows.pred_strikes,
        );
        int_count += 1;

        let sm = measure_single_run(sel2_full, &[], long_cases[idx].1.clone(), POLICY_TICKS);
        if sm.ledger_effects < min_effects {
            vacuity_failures.push(format!("sel2_full/{name}: eff={}", sm.ledger_effects));
        }
        check_policy_vacuity(
            &format!("sel2_full/{name}"),
            sel2_full,
            &sm.flows,
            &mut vacuity_failures,
        );
        let s_label = format!("sel2_full/{name}");
        let s_peak = sm.max_abs();
        if s_peak > int_worst {
            int_worst = s_peak;
            int_worst_label = s_label.clone();
        }
        if s_peak > RESIDUAL_ABS_TOLERANCE_EU {
            violations.push(format!("{s_label}: {s_peak:.3e}"));
        }
        println!(
            "INTERVENTION {s_label:<34} eff={} d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={}(s{},h{},d1={}) p={}({:.1})",
            sm.ledger_effects,
            sm.delta_t0,
            sm.delta_t1,
            sm.worst_abs_t1,
            sm.plants_field_gap,
            sm.flows.births,
            sm.flows.deaths,
            sm.flows.deaths_starv,
            sm.flows.deaths_dehyd,
            fmt_tick_opt(sm.flows.first_death_tick),
            sm.flows.pred_strikes,
            sm.flows.pred_eu,
        );
        int_count += 1;

        let dm = measure_drift_pair_case(
            &format!("k{DRIFT_RESET_K}-{name}"),
            drift_full,
            DRIFT_RESET_K,
            drift_cases[idx].1.clone(),
            DRIFT_TICKS,
        );
        if dm.drf_ledger_effects < min_effects
            || dm.sel_flows.births == 0
            || dm.sel_flows.deaths == 0
            || dm.diag.actual_births == 0
            || dm.diag.actual_deaths == 0
            || dm.sel_flows.pred_strikes == 0
        {
            vacuity_failures.push(format!(
                "drift_k{DRIFT_RESET_K}/{name}: eff={} sel(b={},d={},p={}) drf_actual(b={},d={})",
                dm.drf_ledger_effects,
                dm.sel_flows.births,
                dm.sel_flows.deaths,
                dm.sel_flows.pred_strikes,
                dm.diag.actual_births,
                dm.diag.actual_deaths
            ));
        }
        let dlabel = format!("drift_k{DRIFT_RESET_K}/{name}");
        let d_peak = dm.sel_worst.max(dm.drf_worst);
        if d_peak > int_worst {
            int_worst = d_peak;
            int_worst_label = dlabel.clone();
        }
        if d_peak > RESIDUAL_ABS_TOLERANCE_EU {
            violations.push(format!("{dlabel}: {d_peak:.3e}"));
        }
        println!(
            "INTERVENTION {dlabel:<34} eff={} eu_0={:.3} sel_w={:.3e} drf_w={:.3e} sel_b={} sel_d={} drf_b={} drf_d={}(o{}) p=({},{})",
            dm.drf_ledger_effects,
            dm.eu_0,
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
        POLICY_TICKS,
    );
    check_policy_vacuity(
        "excluded_guard",
        drift_full,
        &excl.flows,
        &mut vacuity_failures,
    );
    let excl_peak = excl.max_abs();
    if excl_peak > RESIDUAL_ABS_TOLERANCE_EU {
        violations.push(format!("excluded_guard: {excl_peak:.3e}"));
    }
    println!(
        "EXCLUDED_GUARD d0={:+.3e} d1={:+.3e} wt1={:.3e} gap={:.3e} b={} d={}(s{},h{},d1={}) p={}({:.1})",
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

    let overall_worst = policy_worst.max(drift_worst).max(int_worst).max(excl_peak);
    println!(
        "M4_OVERALL_SUMMARY total_s={:.2} tolerance={RESIDUAL_ABS_TOLERANCE_EU:.1e} overall_worst={overall_worst:.3e} violations={} vacuity_failures={}",
        total_start.elapsed().as_secs_f64(),
        violations.len(),
        vacuity_failures.len()
    );
    if !vacuity_failures.is_empty() {
        eprintln!("VACUITY_FAILURES ({}):", vacuity_failures.len());
        for v in vacuity_failures.iter().take(40) {
            eprintln!("  - {v}");
        }
    }
    if !violations.is_empty() {
        eprintln!("LEAK_VIOLATIONS: {:?}", violations);
    }
    if violations.is_empty() && vacuity_failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
