//! Card SEL2-D (QD-SEL2): Lockstep neutral-drift control for live-world selection experiments.
//!
//! All 8 tests specified in §8C of the SEL2-D implementation design:
//! 1. Opt-in bit-identity: absent keys == explicit `0.0` == selected run carrying recorder.
//! 2. Exact lockstep birth/death count fidelity per sample when solvent.
//! 3. Trait-blind parent (`P_gen` decoupled from `P_pay`) and victim selection under an artificial
//!    50/50 light-rich vs heavy-bankrupt mass gradient (`mutation_rate = 0.0`).
//! 4. Closed-EU conservation within `RESIDUAL_ABS_TOLERANCE_EU` and zero scratch reallocation.
//! 5. RNG stream isolation (`sim_stream::LIVE_DRIFT_SELECTION = 4`) leaves `SimRng` untouched when
//!    only death counts vary on tick 1.
//! 6. Physical shortfall handling: never fabricates EU, counts extinct/insolvent/death misses,
//!    logs `CausalLedger` records, surfaces warnings, and keeps `RunStatus::Completed`.
//! 7. Channon periodic reset (`live.drift_reset_ticks > 0`): preserves `AgentBrain.genotype` &
//!    `learned` across snapshot boundaries (ADR-0003 D01) without Lamarckian inheritance, and
//!    maintains closed EU across reset boundaries.
//! 8. Multi-process checksum determinism (`K = 0` and `K > 0`), manifest validation guards,
//!    cause-split observable refusal, and `run_live_drift_ensemble` aggregation.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{AgentGeneration, AgentGenotype};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::drift_control::{
    DriftControlState, TickTurnover, TurnoverSchedule, DRIFT_SCRATCH_CAPACITY,
};
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::energy_ledger::{closed_total_eu, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, ExperimentModel, RunStatus};
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue};
use anima_engine_lib::core::live_experiment::{
    run_live_drift_ensemble, run_live_drift_pair, DriftPairReport, LiveExperimentAdapter,
    LiveWorldConfig, LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_DRIFT_CONTROL,
    LIVE_KEY_DRIFT_RESET_TICKS, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sim_clock::{SimClock, ECOLOGY_PERIOD};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::evolution::brain_genotype::BrainGenotype;
use rand::SeedableRng;
use std::process::Command;

const CHILD_ENV: &str = "ANIMA_SEL2D_CHILD";
const CHILD_RESET_ENV: &str = "ANIMA_SEL2D_CHILD_RESET";
const CHILD_MARKER: &str = "SEL2D_CHILD_CHECKSUMS";

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn shared_observable_ids() -> Vec<String> {
    LIVE_OBSERVABLE_IDS
        .iter()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS[..10].iter())
        .map(|s| (*s).to_string())
        .collect()
}

fn live_manifest(
    id: &str,
    extra: &[(&str, f64)],
    seeds: Vec<u64>,
    duration_ticks: u64,
    sample_period: u64,
) -> ExperimentManifest {
    let mut pairs: Vec<(String, f64)> = [
        ("live.founders", 20.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 2.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.prey_cap", 160.0),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_string(), *v))
    .collect();
    for (k, v) in extra {
        if let Some(entry) = pairs.iter_mut().find(|(ek, _)| ek == k) {
            entry.1 = *v;
        } else {
            pairs.push(((*k).to_string(), *v));
        }
    }

    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: id.to_string(),
        name: id.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(pairs),
        interventions: Vec::new(),
        seeds,
        duration_ticks,
        sample_period,
        observable_ids: shared_observable_ids(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
        island: None,
    }
}

fn obs_val(sample: &[(String, f64)], name: &str) -> f64 {
    sample
        .iter()
        .find_map(|(k, v)| (k == name).then_some(*v))
        .unwrap_or_else(|| panic!("observable '{name}' missing from sample"))
}

fn step_adapter(
    adapter: &mut LiveExperimentAdapter,
    seed: u64,
    ticks: u64,
) -> (CausalLedger, Vec<Vec<(String, f64)>>) {
    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let queue = InterventionQueue::new(Vec::new());
    let mut out = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        let t = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(t).collect();
        adapter.step(&clock, &active, &mut ledger, &mut rng);
        out.push(adapter.observables());
    }
    (ledger, out)
}

fn world_closed_eu(adapter: &LiveExperimentAdapter) -> f64 {
    let mut world = adapter.world();
    let (plants, animals, detritus, dormant) = {
        let pool = world.resource::<EcosystemBiomass>();
        (pool.plants, pool.animals, pool.detritus, pool.dormant)
    };
    let _ = &mut world;
    closed_total_eu(plants, animals, detritus, dormant)
}

// ---- Test 1: Opt-in bit-identity ---------------------------------------------------------------

#[test]
fn absent_drift_key_and_explicit_zero_and_recorded_selected_run_are_bit_identical() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = ObservableRegistry::live_default();
    let seed = 1;
    let ticks = 3_000;
    let sample_period = 60;

    let m_absent = live_manifest("sel2d-t1-absent", &[], vec![seed], ticks, sample_period);
    let m_zero = live_manifest(
        "sel2d-t1-zero",
        &[
            (LIVE_KEY_DRIFT_CONTROL, 0.0),
            (LIVE_KEY_DRIFT_RESET_TICKS, 0.0),
        ],
        vec![seed],
        ticks,
        sample_period,
    );
    let m_drift = live_manifest(
        "sel2d-t1-drift",
        &[(LIVE_KEY_DRIFT_CONTROL, 1.0)],
        vec![seed],
        ticks,
        sample_period,
    );

    let r_absent =
        run_manifest_seed::<LiveExperimentAdapter>(&m_absent, &registry, seed, None, None);
    let r_zero = run_manifest_seed::<LiveExperimentAdapter>(&m_zero, &registry, seed, None, None);
    let pair = run_live_drift_pair(&m_absent, &m_drift, &registry, seed)
        .expect("paired selected->drift run must succeed");
    let r_recorded = &pair.selected;

    assert_eq!(
        r_absent.status,
        RunStatus::Completed,
        "{:?}",
        r_absent.warnings
    );
    assert_eq!(r_zero.status, RunStatus::Completed, "{:?}", r_zero.warnings);
    assert_eq!(
        r_recorded.status,
        RunStatus::Completed,
        "{:?}",
        r_recorded.warnings
    );

    println!(
        "SEL2D_T1 absent={:#010x} zero={:#010x} recorded={:#010x} samples={}",
        r_absent.final_checksum,
        r_zero.final_checksum,
        r_recorded.final_checksum,
        r_absent.series.len()
    );

    assert_eq!(
        r_absent.final_checksum, r_zero.final_checksum,
        "explicit 0.0 drift keys must match absent keys bit-for-bit"
    );
    assert_eq!(
        r_absent.final_checksum, r_recorded.final_checksum,
        "installing TurnoverRecorder must not alter the selected trajectory"
    );
    assert_eq!(r_absent.series.len(), r_zero.series.len());
    assert_eq!(r_absent.series.len(), r_recorded.series.len());

    let mut total_births = 0.0;
    let mut total_deaths = 0.0;
    for ((a, z), rec) in r_absent
        .series
        .iter()
        .zip(r_zero.series.iter())
        .zip(r_recorded.series.iter())
    {
        assert_eq!(a.tick, z.tick);
        assert_eq!(a.tick, rec.tick);
        assert_eq!(a.observables.len(), z.observables.len());
        assert_eq!(a.observables.len(), rec.observables.len());
        for ((k, va), ((kz, vz), (kr, vr))) in a
            .observables
            .iter()
            .zip(z.observables.iter().zip(rec.observables.iter()))
        {
            assert_eq!(k, kz);
            assert_eq!(k, kr);
            assert_eq!(
                va.to_bits(),
                vz.to_bits(),
                "sample {} @ tick {} differed between absent and explicit 0.0",
                k,
                a.tick
            );
            assert_eq!(
                va.to_bits(),
                vr.to_bits(),
                "sample {} @ tick {} differed between absent and recorded selected run",
                k,
                a.tick
            );
        }
        total_births += obs_val(&a.observables, "live.births");
        total_deaths += obs_val(&a.observables, "live.deaths");
    }
    assert!(
        total_births > 0.0 && total_deaths > 0.0,
        "test fixture must exercise both births ({total_births}) and deaths ({total_deaths})"
    );
}

// ---- Test 2: Exact lockstep birth/death fidelity -----------------------------------------------

#[test]
fn drift_replay_matches_per_tick_births_and_deaths_exactly_when_solvent() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = ObservableRegistry::live_default();
    let seed = 1;
    let m_sel = live_manifest("sel2d-t2-sel", &[], vec![seed], 1_500, 60);
    let m_drf = live_manifest(
        "sel2d-t2-drf",
        &[(LIVE_KEY_DRIFT_CONTROL, 1.0)],
        vec![seed],
        1_500,
        60,
    );

    let report: DriftPairReport =
        run_live_drift_pair(&m_sel, &m_drf, &registry, seed).expect("paired run must succeed");
    assert_eq!(report.selected.status, RunStatus::Completed);
    assert_eq!(report.drift.status, RunStatus::Completed);

    let diag = &report.diagnostics;
    println!(
        "SEL2D_T2 sched_b={} actual_b={} sched_d={} actual_d={} missed_b={} missed_d={} \
         sel_sum={:#010x} drf_sum={:#010x}",
        diag.scheduled_births,
        diag.actual_births,
        diag.scheduled_deaths,
        diag.actual_deaths,
        diag.missed_births(),
        diag.missed_deaths,
        report.selected.final_checksum,
        report.drift.final_checksum,
    );

    assert!(diag.scheduled_births > 0, "fixture must produce births");
    assert!(diag.scheduled_deaths > 0, "fixture must produce deaths");
    assert_eq!(
        diag.missed_births(),
        0,
        "expected zero missed births: {diag:?}"
    );
    assert_eq!(
        diag.missed_deaths, 0,
        "expected zero missed deaths: {diag:?}"
    );
    assert_eq!(diag.actual_births, diag.scheduled_births);
    assert_eq!(diag.actual_deaths, diag.scheduled_deaths);
    assert_ne!(
        report.selected.final_checksum, report.drift.final_checksum,
        "neutral-drift replay must diverge in state identity from selection"
    );

    assert_eq!(report.selected.series.len(), report.drift.series.len());
    for (s, d) in report
        .selected
        .series
        .iter()
        .zip(report.drift.series.iter())
    {
        assert_eq!(s.tick, d.tick);
        for key in ["live.births", "live.deaths", "live.agent_count"] {
            assert_eq!(
                obs_val(&s.observables, key).to_bits(),
                obs_val(&d.observables, key).to_bits(),
                "lockstep mismatch for {key} at tick {}",
                s.tick
            );
        }
    }
}

// ---- Test 3: Trait-blindness under artificial mass + energy gradient ---------------------------

#[test]
fn drift_parent_and_victim_selection_is_trait_blind_under_artificial_mass_gradient() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let initial_sel = InitialConditionSet::new(vec![
        ("live.founders".into(), 20.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.locomotion".into(), 0.0),
        ("live.mutation_rate".into(), 0.0),
        (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
    ]);

    let mut total_deaths = 0usize;
    let mut heavy_deaths = 0usize;
    let mut total_births = 0usize;
    let mut heavy_births = 0usize;

    for seed in 1..=20u64 {
        let schedule = TurnoverSchedule {
            seed,
            source_manifest_fingerprint: 0,
            duration_ticks: 1,
            ticks: vec![TickTurnover {
                prey_deaths: 6,
                pred_deaths: 0,
                prey_births: 6,
                pred_births: 0,
            }],
        };
        let mut adapter = LiveExperimentAdapter::from_manifest_with_drift_schedule(
            &WorldLawSet::baseline(),
            &initial_sel,
            &[],
            seed,
            0,
            0,
            &schedule,
        )
        .expect("drift world builds");

        {
            let mut world = adapter.world();
            let mut entities: Vec<bevy_ecs::prelude::Entity> = {
                let mut q = world
                    .query_filtered::<bevy_ecs::prelude::Entity, bevy_ecs::prelude::With<Agent>>();
                q.iter(&world).collect()
            };
            entities.sort_by_key(|e| e.index());
            assert_eq!(entities.len(), 20);

            for (idx, entity) in entities.into_iter().enumerate() {
                let is_heavy = idx >= 10;
                let target_mass = if is_heavy { 9.0f32 } else { 0.6f32 };
                if let Some(mut geno) = world.get_mut::<AgentGenotype>(entity) {
                    let cur = geno.0.total_mass().max(1e-6);
                    let scale = target_mass / cur;
                    for node in &mut geno.0.nodes {
                        node.mass *= scale;
                    }
                }
                if let Some(mut homeo) = world.get_mut::<HomeostaticState>(entity) {
                    if is_heavy {
                        // Heavy founders are completely insolvent (0.0 EU) and far past the
                        // starvation grace: both a survival-of-starved leak and a fecundity-energy
                        // filter leak would drive heavy birth/death shares away from 0.50!
                        homeo.energy = 0.0;
                        homeo.starving_ticks = 999;
                    } else {
                        homeo.energy = 200.0;
                        homeo.starving_ticks = 0;
                    }
                }
            }
        }

        adapter.run_schedule_once();
        let diag = adapter.drift_diagnostics().expect("diag present");
        assert_eq!(diag.missed_deaths, 0);
        assert_eq!(diag.missed_births(), 0);

        let mut world = adapter.world();
        let mut surviving_heavy = 0usize;
        let mut surviving_light = 0usize;
        let mut q = world
            .query_filtered::<(&AgentGenotype, &AgentGeneration), bevy_ecs::prelude::With<Agent>>();
        for (geno, age_gen) in q.iter(&world) {
            let m = geno.0.total_mass();
            let heavy = m > 4.0;
            if age_gen.0 == 0 {
                if heavy {
                    surviving_heavy += 1;
                } else {
                    surviving_light += 1;
                }
            } else {
                total_births += 1;
                if heavy {
                    heavy_births += 1;
                }
            }
        }
        let d_heavy = 10 - surviving_heavy;
        let d_light = 10 - surviving_light;
        total_deaths += d_heavy + d_light;
        heavy_deaths += d_heavy;
    }

    let heavy_death_share = heavy_deaths as f64 / total_deaths as f64;
    let heavy_birth_share = heavy_births as f64 / total_births as f64;
    println!(
        "SEL2D_T3 seeds=20 deaths={heavy_deaths}/{total_deaths} ({heavy_death_share:.3}) \
         births_from_bankrupt_heavy={heavy_births}/{total_births} ({heavy_birth_share:.3})"
    );
    assert_eq!(total_deaths, 120);
    assert_eq!(total_births, 120);
    assert!(
        (0.35..=0.65).contains(&heavy_death_share),
        "heavy death share {heavy_death_share:.3} deviated from uniform 0.50"
    );
    assert!(
        (0.35..=0.65).contains(&heavy_birth_share),
        "heavy birth share {heavy_birth_share:.3} deviated from uniform 0.50 (decoupled P_gen/P_pay broken?)"
    );
}

// ---- Test 4: Closed-EU conservation + zero buffer reallocation ---------------------------------

#[test]
fn drift_mode_preserves_closed_eu_within_ledger_tolerance() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let seed = 3;
    let ticks = 1_500;
    let initial_sel = InitialConditionSet::new(vec![
        ("live.founders".into(), 20.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.trees".into(), 8.0),
        ("live.lakes".into(), 2.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.prey_cap".into(), 160.0),
    ]);
    let mut initial_drf_pairs = initial_sel.values.clone();
    initial_drf_pairs.push((LIVE_KEY_DRIFT_CONTROL.into(), 1.0));
    let initial_drf = InitialConditionSet::new(initial_drf_pairs);

    let mut sel = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial_sel,
        &[],
        seed,
        (16, 16),
        ticks,
    )
    .expect("selected builds");
    sel.install_turnover_recorder(ticks);
    let _ = step_adapter(&mut sel, seed, ticks);
    let schedule = sel.take_turnover_schedule().expect("schedule");

    let mut drf = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &initial_drf,
        &[],
        seed,
        0,
        0,
        &schedule,
    )
    .expect("drift builds");

    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let queue = InterventionQueue::new(Vec::new());

    // Tick 1 settles genesis/regrowth baseline, matching live_turnover_tests & plant_reserve_tests.
    clock.advance();
    let active: Vec<&InterventionCommand> = queue.active_at(1).collect();
    drf.step(&clock, &active, &mut ledger, &mut rng);
    let start_eu = obs_val(&drf.observables(), "live.closed_eu_total");

    let mut worst_abs = 0.0f64;
    let mut worst_rel = 0.0f64;
    for _ in 1..ticks {
        let t = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(t).collect();
        drf.step(&clock, &active, &mut ledger, &mut rng);
        let eu = obs_val(&drf.observables(), "live.closed_eu_total");
        let abs_diff = (eu - start_eu).abs();
        let rel_diff = abs_diff / start_eu.abs().max(1.0);
        worst_abs = worst_abs.max(abs_diff);
        worst_rel = worst_rel.max(rel_diff);
        assert!(
            abs_diff <= RESIDUAL_ABS_TOLERANCE_EU,
            "closed-EU drifted by {abs_diff} at tick {t} (tolerance {RESIDUAL_ABS_TOLERANCE_EU})"
        );
    }

    let diag = drf.drift_diagnostics().expect("diag");
    let caps = {
        let world = drf.world();
        world.resource::<DriftControlState>().scratch_capacities()
    };
    println!(
        "SEL2D_T4 ticks={ticks} actual_deaths={} actual_births={} worst_abs_eu={worst_abs:e} \
         worst_rel_eu={worst_rel:e} scratch_caps={caps:?}",
        diag.actual_deaths, diag.actual_births
    );
    assert!(diag.actual_deaths > 0 && diag.actual_births > 0);
    assert!(worst_rel < 1e-6, "relative drift {worst_rel:e} >= 1e-6");
    assert_eq!(
        caps,
        (
            DRIFT_SCRATCH_CAPACITY,
            DRIFT_SCRATCH_CAPACITY,
            DRIFT_SCRATCH_CAPACITY
        ),
        "per-tick scratch buffers must never reallocate during warm ticks"
    );
}

// ---- Test 5: Drift RNG stream isolation from SimRng --------------------------------------------

#[test]
fn drift_rng_stream_does_not_displace_sim_rng_when_no_births_differ() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let initial_drf = InitialConditionSet::new(vec![
        ("live.founders".into(), 12.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
    ]);
    let seed = 7;

    let build_with = |prey_deaths: u16| -> LiveExperimentAdapter {
        let sched = TurnoverSchedule {
            seed,
            source_manifest_fingerprint: 0,
            duration_ticks: 1,
            ticks: vec![TickTurnover {
                prey_deaths,
                pred_deaths: 0,
                prey_births: 0,
                pred_births: 0,
            }],
        };
        LiveExperimentAdapter::from_manifest_with_drift_schedule(
            &WorldLawSet::baseline(),
            &initial_drf,
            &[],
            seed,
            0,
            0,
            &sched,
        )
        .expect("drift adapter builds")
    };

    let mut w_zero = build_with(0);
    let mut w_five = build_with(5);
    w_zero.run_schedule_once();
    w_five.run_schedule_once();

    let (sim_pos_zero, drf_pos_zero, alive_zero) = {
        let mut world = w_zero.world();
        let sim_pos = world.resource::<SimRng>().stream_pos();
        let drf_pos = world.resource::<DriftControlState>().rng_stream_pos();
        let mut q = world.query::<&Agent>();
        let alive = q.iter(&world).count();
        (sim_pos, drf_pos, alive)
    };
    let (sim_pos_five, drf_pos_five, alive_five) = {
        let mut world = w_five.world();
        let sim_pos = world.resource::<SimRng>().stream_pos();
        let drf_pos = world.resource::<DriftControlState>().rng_stream_pos();
        let mut q = world.query::<&Agent>();
        let alive = q.iter(&world).count();
        (sim_pos, drf_pos, alive)
    };

    println!(
        "SEL2D_T5 zero_deaths(alive={alive_zero}, sim_pos={sim_pos_zero}, drift_pos={drf_pos_zero}) \
         five_deaths(alive={alive_five}, sim_pos={sim_pos_five}, drift_pos={drf_pos_five})"
    );
    assert_eq!(alive_zero, 12);
    assert_eq!(alive_five, 7);
    assert_eq!(
        sim_pos_zero, sim_pos_five,
        "random drift deaths must consume zero draws from SimRng"
    );
    assert_eq!(drf_pos_zero, 0);
    assert!(
        drf_pos_five > 0,
        "random drift deaths must draw from LIVE_DRIFT_SELECTION stream"
    );
}

// ---- Test 6: Shortfall accounting, CausalLedger, warnings, and RunStatus::Completed ------------

#[test]
fn shortfall_when_population_cannot_follow_schedule_is_counted_and_warned_never_silent() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = ObservableRegistry::live_default();
    let seed = 11;

    // 6A: Deterministic schedule shortfall exercise (insolvent births -> overshoot deaths -> extinct births).
    let initial_drf = InitialConditionSet::new(vec![
        ("live.founders".into(), 4.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.trees".into(), 0.0),
        ("live.lakes".into(), 0.0),
        ("live.food_cap".into(), 0.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
    ]);
    let mut ticks_vec = vec![TickTurnover::default(); ECOLOGY_PERIOD as usize];
    ticks_vec[0].prey_births = 3; // Tick 1: all 4 agents insolvent -> 3 missed_births_insolvent
    ticks_vec[1].prey_deaths = 6; // Tick 2: only 4 alive -> 4 actual, 2 missed_deaths
    ticks_vec[2].prey_births = 2; // Tick 3: 0 alive -> 2 missed_births_extinct
    let schedule = TurnoverSchedule {
        seed,
        source_manifest_fingerprint: 0,
        duration_ticks: ECOLOGY_PERIOD,
        ticks: ticks_vec,
    };

    let mut drf = LiveExperimentAdapter::from_manifest_with_drift_schedule(
        &WorldLawSet::baseline(),
        &initial_drf,
        &[],
        seed,
        0,
        0,
        &schedule,
    )
    .expect("shortfall fixture builds");

    // Transfer all founder energy to detritus before tick 1 so the guild is 100% insolvent while
    // keeping closed total EU invariant (no energy created or destroyed).
    {
        let mut world = drf.world();
        let mut drained = 0.0f64;
        let mut q = world.query_filtered::<&mut HomeostaticState, bevy_ecs::prelude::With<Agent>>();
        for mut h in q.iter_mut(&mut world) {
            drained += h.energy as f64;
            h.energy = 0.0;
        }
        world.resource_mut::<EcosystemBiomass>().detritus += drained;
    }

    let eu_before = world_closed_eu(&drf);
    let (ledger, _) = step_adapter(&mut drf, seed, ECOLOGY_PERIOD);
    let eu_after = world_closed_eu(&drf);

    let diag = drf.drift_diagnostics().expect("diagnostics present");
    let shortfall_birth_records: Vec<_> = ledger
        .all()
        .iter()
        .filter(|r| r.target == "live.drift_shortfall_births")
        .collect();
    let shortfall_death_records: Vec<_> = ledger
        .all()
        .iter()
        .filter(|r| r.target == "live.drift_shortfall_deaths")
        .collect();

    println!(
        "SEL2D_T6A first_div={:?} missed_insolvent={} missed_extinct={} missed_deaths={} \
         actual_deaths={} ledger_b={} ledger_d={} eu_delta={:e}",
        diag.first_divergence_tick,
        diag.missed_births_insolvent,
        diag.missed_births_extinct,
        diag.missed_deaths,
        diag.actual_deaths,
        shortfall_birth_records.len(),
        shortfall_death_records.len(),
        (eu_after - eu_before).abs()
    );

    assert_eq!(diag.first_divergence_tick, Some(1));
    assert_eq!(diag.scheduled_births, 5);
    assert_eq!(diag.actual_births, 0);
    assert_eq!(diag.missed_births_insolvent, 3);
    assert_eq!(diag.missed_births_extinct, 2);
    assert_eq!(diag.ticks_with_missed_births, 2);
    assert_eq!(diag.scheduled_deaths, 6);
    assert_eq!(diag.actual_deaths, 4);
    assert_eq!(diag.missed_deaths, 2);
    assert_eq!(diag.ticks_with_missed_deaths, 1);
    assert!(
        (eu_after - eu_before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "shortfall must never invent EU: delta={}",
        eu_after - eu_before
    );
    assert!(
        !shortfall_birth_records.is_empty(),
        "CausalLedger must record birth shortfalls"
    );
    assert!(
        !shortfall_death_records.is_empty(),
        "CausalLedger must record death shortfalls"
    );
    let remaining_agents = {
        let mut world = drf.world();
        let mut q = world.query::<&Agent>();
        q.iter(&world).count()
    };
    assert_eq!(
        remaining_agents, 0,
        "clamped deaths must cleanly despawn every living agent without panicking"
    );

    // 6B: Verify run_live_drift_pair end-to-end warns while preserving RunStatus::Completed.
    let sel_m = live_manifest(
        "sel2d-t6-sel",
        &[
            ("live.founders", 30.0),
            ("live.trees", 0.0),
            ("live.lakes", 0.0),
            ("live.food_cap", 0.0),
            ("live.starve_ticks", 5.0),
            ("live.dehydration_ticks", 5.0),
            ("live.resource_capacity_scale", 0.01),
        ],
        vec![seed],
        1_500,
        100,
    );
    let mut drf_m = sel_m.clone();
    drf_m.experiment_id = "sel2d-t6-drf".into();
    drf_m
        .initial_conditions
        .values
        .push((LIVE_KEY_DRIFT_CONTROL.into(), 1.0));
    let pair = run_live_drift_pair(&sel_m, &drf_m, &registry, seed)
        .expect("harsh shortfall pair builds cleanly");
    assert_eq!(
        pair.drift.status,
        RunStatus::Completed,
        "drift shortfall must keep RunStatus::Completed to avoid survivorship bias"
    );
    if pair.diagnostics.has_shortfall() {
        assert!(
            !pair.drift.warnings.is_empty(),
            "RunResult::warnings must surface drift shortfall"
        );
    }
    println!(
        "SEL2D_T6B pair_status={:?} sched_d={} missed_d={} warnings={:?}",
        pair.drift.status,
        pair.diagnostics.scheduled_deaths,
        pair.diagnostics.missed_deaths,
        pair.drift.warnings
    );
}

// ---- Test 7: Channon periodic reset preserves evolved brains (D01, No-Lamarck) & EU -----------

#[test]
fn periodic_reset_preserves_evolved_brains_without_lamarck_and_conserves_eu() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = ObservableRegistry::live_default();
    let seed = 5;

    // 7A: Rolling 200-tick Channon windows over 600 ticks with evolved brains enabled.
    let m_sel = live_manifest(
        "sel2d-t7-sel",
        &[("live.founders", 16.0), ("live.evolved_brains", 1.0)],
        vec![seed],
        600,
        100,
    );
    let m_drf = live_manifest(
        "sel2d-t7-drf",
        &[
            ("live.founders", 16.0),
            ("live.evolved_brains", 1.0),
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
            (LIVE_KEY_DRIFT_RESET_TICKS, 200.0),
        ],
        vec![seed],
        600,
        100,
    );

    let pair = run_live_drift_pair(&m_sel, &m_drf, &registry, seed)
        .expect("Channon periodic-reset pair must succeed");
    assert_eq!(pair.selected.status, RunStatus::Completed);
    assert_eq!(pair.drift.status, RunStatus::Completed);
    assert_eq!(pair.window_count, 3);
    assert_eq!(pair.reset_period_ticks, 200);
    assert!(!pair.window_net_selection_deltas.is_empty());

    let baseline_eu = obs_val(&pair.selected.series[0].observables, "live.closed_eu_total");
    let worst_window_eu_drift = pair
        .drift
        .series
        .iter()
        .map(|s| (obs_val(&s.observables, "live.closed_eu_total") - baseline_eu).abs())
        .fold(0.0f64, f64::max);
    println!(
        "SEL2D_T7A windows={} samples={} worst_eu_drift={worst_window_eu_drift:e} \
         mass_net_delta={:?}",
        pair.window_count,
        pair.drift.series.len(),
        pair.window_net_selection_deltas.get("live.trait_mass_mean")
    );
    assert!(
        worst_window_eu_drift <= RESIDUAL_ABS_TOLERANCE_EU,
        "Channon resets leaked EU: {worst_window_eu_drift}"
    );

    // 7B: Direct ADR-0003 D01 (verbatim restore of genotype + learned) and No-Lamarck check across
    // `from_snapshot_with_drift_schedule`.
    let sel_init = InitialConditionSet::new(vec![
        ("live.founders".into(), 6.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.evolved_brains".into(), 1.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.mutation_rate".into(), 0.0),
    ]);
    let drf_init = InitialConditionSet::new(vec![
        ("live.founders".into(), 6.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.evolved_brains".into(), 1.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.mutation_rate".into(), 0.0),
        (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
        (LIVE_KEY_DRIFT_RESET_TICKS.into(), 1.0),
    ]);

    let mut sel_adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &sel_init,
        &[],
        seed,
        (16, 16),
        2,
    )
    .expect("evolved brain selected builds");
    sel_adapter.run_schedule_once();

    // Record each pre-snapshot agent's genotype and install distinct sentinel `learned` weights before snapshot.
    let sentinel_weight = 42.25f32;
    let mut expected_genotypes: Vec<Vec<f32>> = Vec::new();
    {
        let mut world = sel_adapter.world();
        let mut q = world.query_filtered::<&mut AgentBrain, bevy_ecs::prelude::With<Agent>>();
        for mut brain in q.iter_mut(&mut world) {
            expected_genotypes.push(brain.genotype.weights.clone());
            let mut learned: BrainGenotype = (*brain.genotype).clone();
            learned.weights.fill(sentinel_weight);
            brain.set_learned(learned);
        }
    }
    let pre_snapshot_count = expected_genotypes.len();
    assert!(pre_snapshot_count >= 6);

    let snap = sel_adapter.snapshot();
    let win_schedule = TurnoverSchedule {
        seed,
        source_manifest_fingerprint: 0,
        duration_ticks: 1,
        ticks: vec![TickTurnover {
            prey_deaths: 0,
            pred_deaths: 0,
            prey_births: 2,
            pred_births: 0,
        }],
    };
    let mut drf_window = LiveExperimentAdapter::from_snapshot_with_drift_schedule(
        &snap,
        &WorldLawSet::baseline(),
        &drf_init,
        0,
        0,
        &win_schedule,
        SimRng::from_seed(seed),
    )
    .expect("snapshot reset drift window builds");

    // Verify D01 immediately after restore: both `genotype` and sentinel `learned` preserved.
    {
        let mut world = drf_window.world();
        let mut q = world.query_filtered::<&AgentBrain, bevy_ecs::prelude::With<Agent>>();
        let mut restored_count = 0usize;
        for brain in q.iter(&world) {
            assert!(expected_genotypes.contains(&brain.genotype.weights));
            let learned = brain
                .learned
                .as_ref()
                .expect("D01: snapshot restore must preserve learned brain weights");
            assert!(learned.weights.iter().all(|&w| w == sentinel_weight));
            restored_count += 1;
        }
        assert_eq!(restored_count, pre_snapshot_count);
    }

    drf_window.run_schedule_once();

    // Verify No-Lamarck on newborn offspring spawned in the reset drift window (the only agents
    // without sentinel `learned` weights are the 2 newborns just spawned by the replay window).
    {
        let mut world = drf_window.world();
        let mut q = world
            .query_filtered::<(&AgentGeneration, &AgentBrain), bevy_ecs::prelude::With<Agent>>();
        let mut preserved_learned_count = 0usize;
        let mut newborn_without_learned = 0usize;
        for (gen_comp, brain) in q.iter(&world) {
            if brain.learned.is_some() {
                preserved_learned_count += 1;
            } else {
                assert!(gen_comp.0 >= 1);
                newborn_without_learned += 1;
                assert!(
                    expected_genotypes.contains(&brain.genotype.weights),
                    "newborn brain must come strictly from parent's heritable genotype, never learned weights"
                );
                assert!(
                    !brain.genotype.weights.iter().all(|&w| w == sentinel_weight),
                    "Lamarckian leakage detected: newborn inherited sentinel learned weights"
                );
            }
        }
        assert_eq!(preserved_learned_count, pre_snapshot_count);
        assert_eq!(
            newborn_without_learned, 2,
            "expected 2 newborns with learned=None in window"
        );
    }
}

// ---- Test 8: Multi-process determinism + guards + ensemble -------------------------------------

#[test]
fn sel2d_drift_child_entry_point() {
    let Ok(mode) = std::env::var(CHILD_ENV) else {
        return;
    };
    let reset_ticks: u64 = std::env::var(CHILD_RESET_ENV)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let registry = ObservableRegistry::live_default();
    let seed = 2;
    let m_sel = live_manifest("sel2d-child-sel", &[], vec![seed], 600, 150);
    let m_drf = live_manifest(
        "sel2d-child-drf",
        &[
            (LIVE_KEY_DRIFT_CONTROL, 1.0),
            (LIVE_KEY_DRIFT_RESET_TICKS, reset_ticks as f64),
        ],
        vec![seed],
        600,
        150,
    );
    let pair = run_live_drift_pair(&m_sel, &m_drf, &registry, seed).expect("child drift pair runs");
    assert_eq!(mode, "1");
    println!(
        "{CHILD_MARKER} {:#010x} {:#010x}",
        pair.selected.final_checksum, pair.drift.final_checksum
    );
}

fn spawn_drift_child(reset_ticks: u64) -> (u32, u32) {
    let out = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "sel2d_drift_child_entry_point", "--nocapture"])
        .env(CHILD_ENV, "1")
        .env(CHILD_RESET_ENV, reset_ticks.to_string())
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn sel2d child");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "child failed:\n{stdout}\n{stderr}");
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(CHILD_MARKER))
        .unwrap_or_else(|| panic!("missing marker in child output:\n{stdout}\n{stderr}"));
    let parts: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(parts.len(), 3, "unexpected child marker line: {line}");
    let parse_hex = |tok: &str| u32::from_str_radix(tok.trim_start_matches("0x"), 16).expect("hex");
    (parse_hex(parts[1]), parse_hex(parts[2]))
}

#[test]
fn drift_pair_reproduces_identical_checksums_across_separate_processes() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 8A: Multi-process checksum agreement for both unreset (K=0) and Channon (K=150).
    let unreset: Vec<(u32, u32)> = (0..3).map(|_| spawn_drift_child(0)).collect();
    let windowed: Vec<(u32, u32)> = (0..3).map(|_| spawn_drift_child(150)).collect();
    println!("SEL2D_T8_PROCS unreset={unreset:#x?} windowed_k150={windowed:#x?}");
    assert!(
        unreset.iter().all(|&p| p == unreset[0]),
        "unreset multi-process checksum disagreement: {unreset:#x?}"
    );
    assert!(
        windowed.iter().all(|&p| p == windowed[0]),
        "windowed multi-process checksum disagreement: {windowed:#x?}"
    );

    // 8B: Manifest & adapter guards.
    let drift_without_schedule = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(vec![
            ("live.mortality".into(), 1.0),
            ("live.reproduction".into(), 1.0),
            (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
        ]),
        &[],
        1,
        (16, 16),
        10,
    );
    assert!(
        matches!(
            drift_without_schedule,
            Err(ExperimentError::InvalidLaw { .. })
        ),
        "bare ExperimentModel::from_manifest must reject live.drift_control = 1.0"
    );

    assert!(matches!(
        LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
            LIVE_KEY_DRIFT_CONTROL.into(),
            1.0
        )])),
        Err(ExperimentError::InvalidLaw { .. })
    ));
    assert!(matches!(
        LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
            LIVE_KEY_DRIFT_RESET_TICKS.into(),
            100.0
        )])),
        Err(ExperimentError::InvalidLaw { .. })
    ));

    let drift_cfg = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        (LIVE_KEY_DRIFT_CONTROL.into(), 1.0),
    ]))
    .expect("valid drift config");
    for cause_id in ["live.deaths_starvation", "live.deaths_dehydration"] {
        assert!(matches!(
            drift_cfg.ensure_observables_supported(&[cause_id.to_string()]),
            Err(ExperimentError::InvalidObservableRequest { .. })
        ));
    }

    // 8C: Multi-seed paired ensemble reuse via summarize_seed_pairs.
    let registry = ObservableRegistry::live_default();
    let m_sel = live_manifest("sel2d-ens-sel", &[], vec![1, 2], 400, 100);
    let m_drf = live_manifest(
        "sel2d-ens-drf",
        &[(LIVE_KEY_DRIFT_CONTROL, 1.0)],
        vec![1, 2],
        400,
        100,
    );
    let report = run_live_drift_ensemble(&m_sel, &m_drf, &registry)
        .expect("paired drift ensemble must succeed");
    assert_eq!(report.ensemble.complete_pairs(), 2);
    assert_eq!(
        report.ensemble.treatment_only,
        vec![
            "live.deaths_dehydration".to_string(),
            "live.deaths_starvation".to_string()
        ]
    );
    let mass_effect = report
        .ensemble
        .effect_of("live.trait_mass_mean")
        .expect("live.trait_mass_mean effect present");
    assert_eq!(mass_effect.n_complete_pairs, 2);
}
