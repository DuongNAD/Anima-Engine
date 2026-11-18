//! Card S2 integration gates — observables for SEL2-P, E2R, and E3.
//!
//! 1. `live.mean_energy_integral`: cumulative running sum over every simulated tick of
//!    `live.mean_agent_energy` (`EU·tick`, `Aggregation::Instant`), including unsampled ticks,
//!    preserving the exact sum across disk save/load checkpoints (`K` -> disk -> `N - K`).
//! 2. `live.trait_mass_mean_west` and `live.trait_mass_mean_east`: E3 region-split mean body mass
//!    over `col < 128` (`Region::Rect{0,0,127,255}`) and `col >= 128` (`Region::Rect{128,0,255,255}`)
//!    of the 256x256 grid.
//! 3. Non-perturbation across every-tick, windowed, and never-sampled readers, plus finite
//!    empty-population behavior (`0.0` for regional mass means; `0.0` per-tick contribution to the
//!    integral).

use anima_engine_lib::core::agent_systems::AgentGenotype;
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{Agent, Position};
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::ecs::MapBounds;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{ExperimentModel, RunStatus};
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue, Region};
use anima_engine_lib::core::live_experiment::{
    live_observables, live_observables_peek, mass_statistics, run_live_drift_pair,
    LiveExperimentAdapter, LiveSnapshot, LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_DEHYDRATION_TICKS,
    LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_STARVE_TICKS, LIVE_KEY_TREES, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::With;
use rand::SeedableRng;

const SEED: u64 = 0x5253_0001;

fn s2_initials(
    founders: f64,
    mortality: bool,
    reproduction: bool,
    locomotion: bool,
) -> InitialConditionSet {
    let mut vals = vec![
        (LIVE_KEY_FOUNDERS.to_string(), founders),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.2),
        (LIVE_KEY_TREES.to_string(), 4.0),
        (LIVE_KEY_LAKES.to_string(), 2.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 40.0),
    ];
    if mortality {
        vals.push((LIVE_KEY_MORTALITY.to_string(), 1.0));
        vals.push((LIVE_KEY_STARVE_TICKS.to_string(), 5.0));
        vals.push((LIVE_KEY_DEHYDRATION_TICKS.to_string(), 5.0));
    }
    if reproduction {
        vals.push((LIVE_KEY_REPRODUCTION.to_string(), 1.0));
    }
    if locomotion {
        vals.push((LIVE_KEY_LOCOMOTION.to_string(), 1.0));
        vals.push((LIVE_KEY_DRIVE_FLOOR.to_string(), 0.5));
    }
    InitialConditionSet::new(vals)
}

fn value_of(obs: &[(String, f64)], id: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("{id} missing from emitted observables"))
}

fn through_disk(snap: &LiveSnapshot, label: &str) -> LiveSnapshot {
    static DISK_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = DISK_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("anima_s2_{}_{}_{}", std::process::id(), seq, label));
    std::fs::create_dir_all(&dir).expect("create temp checkpoint dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snap.saved.clone()).expect("seal snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write checkpoint");
    let back = snapshot::read(&path).expect("read checkpoint");
    assert_eq!(back.loaded_from_schema, snapshot::SCHEMA_VERSION);
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snap.config,
        seed: snap.seed,
    }
}

#[test]
fn reading_observables_every_tick_vs_windowed_vs_never_does_not_perturb_and_integrates_every_tick()
{
    const TICKS: u64 = 180;
    let laws = WorldLawSet::baseline();
    let initials = s2_initials(20.0, true, true, true);
    let queue = InterventionQueue::new(vec![]);

    let build = || {
        LiveExperimentAdapter::from_manifest(&laws, &initials, &[], SEED, (16, 16), TICKS)
            .expect("live adapter builds")
    };

    // Arm A: sampled (`observables()`) on EVERY tick; collect running sum of `mean_agent_energy`.
    let mut every_tick = build();
    let mut rng_a = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_a = SimClock::new();
    let mut ledger_a = CausalLedger::new();
    let mut expected_integral = 0.0f64;
    let mut checksums_a = Vec::new();
    for _ in 0..TICKS {
        let tick = clock_a.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        every_tick.step(&clock_a, &active, &mut ledger_a, &mut rng_a);
        let obs = every_tick.observables();
        expected_integral += value_of(&obs, "live.mean_agent_energy");
        if matches!(tick, 1 | 60 | 120 | 180) {
            checksums_a.push((tick, every_tick.checksum()));
        }
    }
    let obs_a = every_tick.observables_unsampled();

    // Arm B: sampled only at ticks 60, 120, 180; `observables_unsampled()` on all other ticks.
    let mut windowed = build();
    let mut rng_b = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_b = SimClock::new();
    let mut ledger_b = CausalLedger::new();
    let mut checksums_b = Vec::new();
    for _ in 0..TICKS {
        let tick = clock_b.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        windowed.step(&clock_b, &active, &mut ledger_b, &mut rng_b);
        let _obs = if tick.is_multiple_of(60) {
            windowed.observables()
        } else {
            windowed.observables_unsampled()
        };
        if matches!(tick, 1 | 60 | 120 | 180) {
            checksums_b.push((tick, windowed.checksum()));
        }
    }
    let obs_b = windowed.observables_unsampled();

    // Arm C: NEVER call observables during ticks 1..=180; only read once at tick 180.
    let mut never_read = build();
    let mut rng_c = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_c = SimClock::new();
    let mut ledger_c = CausalLedger::new();
    let mut checksums_c = Vec::new();
    for _ in 0..TICKS {
        let tick = clock_c.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        never_read.step(&clock_c, &active, &mut ledger_c, &mut rng_c);
        if matches!(tick, 1 | 60 | 120 | 180) {
            checksums_c.push((tick, never_read.checksum()));
        }
    }
    let obs_c = never_read.observables_unsampled();

    // Non-perturbation: every compared tick has identical checksum regardless of sampling cadence.
    assert_eq!(
        checksums_a, checksums_b,
        "windowed sampling changed world checksum"
    );
    assert_eq!(
        checksums_a, checksums_c,
        "never sampling changed world checksum"
    );

    // Integral must count EVERY simulated tick even in windowed and never-read arms.
    let int_a = value_of(&obs_a, "live.mean_energy_integral");
    let int_b = value_of(&obs_b, "live.mean_energy_integral");
    let int_c = value_of(&obs_c, "live.mean_energy_integral");
    assert!(
        expected_integral > 0.0,
        "integral must accumulate positive energy over 180 ticks"
    );
    assert_eq!(
        int_a.to_bits(),
        expected_integral.to_bits(),
        "live.mean_energy_integral must match running sum of live.mean_agent_energy"
    );
    assert_eq!(
        int_b.to_bits(),
        int_a.to_bits(),
        "windowed run missed unsampled ticks in live.mean_energy_integral"
    );
    assert_eq!(
        int_c.to_bits(),
        int_a.to_bits(),
        "unsampled run missed simulated ticks in live.mean_energy_integral"
    );
    assert_eq!(
        value_of(&obs_c, "live.trait_mass_mean_west").to_bits(),
        value_of(&obs_a, "live.trait_mass_mean_west").to_bits()
    );
    assert_eq!(
        value_of(&obs_c, "live.trait_mass_mean_east").to_bits(),
        value_of(&obs_a, "live.trait_mass_mean_east").to_bits()
    );
}

#[test]
fn mean_energy_integral_survives_disk_checkpoint_fork_without_midrun_sampling() {
    const N: u64 = 180;
    const K: u64 = 60;
    let laws = WorldLawSet::baseline();
    let initials = s2_initials(20.0, true, true, true);
    let queue = InterventionQueue::new(vec![]);

    // Uninterrupted run: N ticks without sampling mid-run.
    let mut reference =
        LiveExperimentAdapter::from_manifest(&laws, &initials, &[], SEED, (16, 16), N)
            .expect("reference builds");
    let mut rng_ref = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_ref = SimClock::new();
    let mut ledger_ref = CausalLedger::new();
    for _ in 0..N {
        let tick = clock_ref.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        reference.step(&clock_ref, &active, &mut ledger_ref, &mut rng_ref);
    }

    // Forked run: K ticks -> disk snapshot -> restore -> N - K ticks.
    let mut prefix = LiveExperimentAdapter::from_manifest(&laws, &initials, &[], SEED, (16, 16), N)
        .expect("prefix builds");
    let mut rng_fork = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut clock_prefix = SimClock::new();
    let mut ledger_fork = CausalLedger::new();
    for _ in 0..K {
        let tick = clock_prefix.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        prefix.step(&clock_prefix, &active, &mut ledger_fork, &mut rng_fork);
    }
    let disk_snap = through_disk(&prefix.snapshot(), "s2_integral");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&disk_snap).expect("resume succeeds");
    let mut clock_resumed = SimClock { tick: K };
    for _ in 0..(N - K) {
        let tick = clock_resumed.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        resumed.step(&clock_resumed, &active, &mut ledger_fork, &mut rng_fork);
    }

    assert_eq!(reference.checksum(), resumed.checksum());
    let obs_ref = reference.observables();
    let obs_res = resumed.observables();
    let int_ref = value_of(&obs_ref, "live.mean_energy_integral");
    let int_res = value_of(&obs_res, "live.mean_energy_integral");
    assert!(int_ref > 0.0);
    assert_eq!(
        int_ref.to_bits(),
        int_res.to_bits(),
        "live.mean_energy_integral diverged across disk save/load fork ({int_ref} vs {int_res})"
    );
    // All state-based observables (including S2 integral & west/east means) match bit-for-bit;
    // only windowed `VitalStatistics` counters (`live.births`, `live.deaths`, `live.mean_lifespan`,
    // `live.deaths_starvation`, `live.deaths_dehydration`) reflect `[0, N]` vs `[K, N]` because
    // neither arm sampled mid-run at tick `K`.
    assert_eq!(obs_ref.len(), obs_res.len());
    for ((k_ref, v_ref), (k_res, v_res)) in obs_ref.iter().zip(obs_res.iter()) {
        assert_eq!(k_ref, k_res);
        if matches!(
            k_ref.as_str(),
            "live.births"
                | "live.deaths"
                | "live.mean_lifespan"
                | "live.deaths_starvation"
                | "live.deaths_dehydration"
        ) {
            continue;
        }
        assert_eq!(
            v_ref.to_bits(),
            v_res.to_bits(),
            "{k_ref} diverged across disk checkpoint fork ({v_ref} vs {v_res})"
        );
    }
}

#[test]
fn west_and_east_trait_mass_means_match_256_grid_rect_split_and_boundary_columns() {
    let initials = s2_initials(4.0, false, false, false);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("adapter builds");

    let (width, height, min_x, max_x, min_z, max_z) = {
        let world = adapter.world();
        let field = world
            .get_resource::<ResourceField>()
            .expect("ResourceField");
        let bounds = world.get_resource::<MapBounds>().expect("MapBounds");
        assert_eq!(field.width, 256);
        assert_eq!(field.height, 256);
        assert!((field.min_x - bounds.min.x).abs() < 1e-6);
        assert!((field.max_x - bounds.max.x).abs() < 1e-6);
        (
            field.width,
            field.height,
            field.min_x,
            field.max_x,
            field.min_z,
            field.max_z,
        )
    };

    let span_x = max_x - min_x;
    let mid_z = 0.5 * (min_z + max_z);
    // Exact boundary columns on the 256x256 grid:
    // - West half = E3 Rect{0, 0, 127, 255}: test interior col 32 and boundary col 127.
    // - East half = E3 Rect{128, 0, 255, 255}: test boundary col 128 (`u = 0.5`) and col 220.
    let placements: [(f32, f32, bool); 4] = [
        (min_x + (32.5 / width as f32) * span_x, 2.0, true),
        (min_x + (127.5 / width as f32) * span_x, 4.0, true),
        (min_x + (128.0 / width as f32) * span_x, 6.0, false),
        (min_x + (220.5 / width as f32) * span_x, 10.0, false),
    ];

    let west_rect = Region::Rect {
        min_x: 0,
        min_y: 0,
        max_x: 127,
        max_y: (height - 1) as u32,
    };
    let east_rect = Region::Rect {
        min_x: 128,
        min_y: 0,
        max_x: (width - 1) as u32,
        max_y: (height - 1) as u32,
    };

    {
        let mut world = adapter.world();
        {
            let field = world
                .get_resource::<ResourceField>()
                .expect("ResourceField");
            for &(x, _, is_west) in &placements {
                let idx = field
                    .cell_index(x, mid_z)
                    .expect("test x within ResourceField bounds");
                let col = (idx % field.width) as u32;
                let row = (idx / field.width) as u32;
                assert_eq!(
                    west_rect.contains(col, row),
                    is_west,
                    "west rect mismatch for col {col}"
                );
                assert_eq!(
                    east_rect.contains(col, row),
                    !is_west,
                    "east rect mismatch for col {col}"
                );
            }
        }
        let mut q = world.query_filtered::<(&mut Position, &mut AgentGenotype), With<Agent>>();
        let mut i = 0usize;
        for (mut pos, mut geno) in q.iter_mut(&mut world) {
            let (x, target_mass, _) = placements[i];
            pos.0.x = x;
            pos.0.z = mid_z;
            // All three nodes are reachable from root 0, so total_mass == sum of node masses.
            geno.0.nodes[0].mass = target_mass;
            for n in geno.0.nodes.iter_mut().skip(1) {
                n.mass = 0.0;
            }
            assert!((geno.0.total_mass() - target_mass).abs() < 1e-6);
            i += 1;
        }
        assert_eq!(i, placements.len());
    }

    let obs = {
        let mut world = adapter.world();
        live_observables(&mut world)
    };
    let expected_west = mass_statistics(&[2.0f32, 4.0])[0];
    let expected_east = mass_statistics(&[6.0f32, 10.0])[0];
    let expected_all = mass_statistics(&[2.0f32, 4.0, 6.0, 10.0])[0];
    assert_eq!(
        value_of(&obs, "live.trait_mass_mean").to_bits(),
        expected_all.to_bits()
    );
    assert_eq!(
        value_of(&obs, "live.trait_mass_mean_west").to_bits(),
        expected_west.to_bits()
    );
    assert_eq!(
        value_of(&obs, "live.trait_mass_mean_east").to_bits(),
        expected_east.to_bits()
    );

    // Empty one side (move every agent to the East half) -> West emits finite `0.0`.
    {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<&mut Position, With<Agent>>();
        for mut pos in q.iter_mut(&mut world) {
            pos.0.x = min_x + (192.5 / width as f32) * span_x;
        }
    }
    let obs_east_only = {
        let mut world = adapter.world();
        live_observables_peek(&mut world)
    };
    assert_eq!(value_of(&obs_east_only, "live.trait_mass_mean_west"), 0.0);
    assert_eq!(
        value_of(&obs_east_only, "live.trait_mass_mean_east").to_bits(),
        expected_all.to_bits()
    );
}

#[test]
fn zero_living_agents_at_genesis_and_after_extinction_emit_finite_values_without_sentinels() {
    let laws = WorldLawSet::baseline();

    // Case A: zero founders from tick 0 -> all emitted values finite, S2 observables are 0.0.
    let empty_init = s2_initials(0.0, true, false, false);
    let mut empty =
        LiveExperimentAdapter::from_manifest(&laws, &empty_init, &[], SEED, (16, 16), 10)
            .expect("empty-founder live world builds");
    for _ in 0..10 {
        empty.run_schedule_once();
        let obs = {
            let mut w = empty.world();
            live_observables(&mut w)
        };
        assert!(obs.iter().all(|(_, v)| v.is_finite()));
        assert_eq!(value_of(&obs, "live.agent_count"), 0.0);
        assert_eq!(value_of(&obs, "live.mean_energy_integral"), 0.0);
        assert_eq!(value_of(&obs, "live.trait_mass_mean_west"), 0.0);
        assert_eq!(value_of(&obs, "live.trait_mass_mean_east"), 0.0);
    }

    // Case B: extinction mid-run -> pre-extinction positive integral is preserved finitely when
    // population drops to zero (adds 0.0 on extinct ticks), while regional mass means become 0.0.
    let doomed_init = InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 6.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
        (LIVE_KEY_MORTALITY.to_string(), 1.0),
        (LIVE_KEY_STARVE_TICKS.to_string(), 100.0),
        (LIVE_KEY_DEHYDRATION_TICKS.to_string(), 1.0),
    ]);
    let mut doomed =
        LiveExperimentAdapter::from_manifest(&laws, &doomed_init, &[], SEED, (16, 16), 10)
            .expect("mortality live world builds");
    doomed.run_schedule_once();
    let pre_obs = {
        let mut w = doomed.world();
        live_observables(&mut w)
    };
    let pre_integral = value_of(&pre_obs, "live.mean_energy_integral");
    assert!(pre_integral > 0.0 && pre_integral.is_finite());

    // Dehydrate all founders to 0 on the next tick.
    {
        use anima_engine_lib::ai::hrrl::HomeostaticState;
        let mut w = doomed.world();
        let mut q = w.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for mut h in q.iter_mut(&mut w) {
            h.hydration = 0.0;
            h.dehydrating_ticks = Some(1);
        }
    }
    doomed.run_schedule_once();
    let extinct_obs_1 = {
        let mut w = doomed.world();
        live_observables(&mut w)
    };
    assert_eq!(value_of(&extinct_obs_1, "live.agent_count"), 0.0);
    let integral_at_extinction = value_of(&extinct_obs_1, "live.mean_energy_integral");
    assert!(integral_at_extinction >= pre_integral && integral_at_extinction.is_finite());
    assert_eq!(value_of(&extinct_obs_1, "live.trait_mass_mean_west"), 0.0);
    assert_eq!(value_of(&extinct_obs_1, "live.trait_mass_mean_east"), 0.0);

    // Subsequent extinct tick adds 0.0 (no sentinel, stays finite and unchanged).
    doomed.run_schedule_once();
    let extinct_obs_2 = {
        let mut w = doomed.world();
        live_observables(&mut w)
    };
    assert!(extinct_obs_2.iter().all(|(_, v)| v.is_finite()));
    assert_eq!(
        value_of(&extinct_obs_2, "live.mean_energy_integral").to_bits(),
        integral_at_extinction.to_bits()
    );
    assert_eq!(value_of(&extinct_obs_2, "live.trait_mass_mean_west"), 0.0);
    assert_eq!(value_of(&extinct_obs_2, "live.trait_mass_mean_east"), 0.0);
}

#[test]
fn paired_drift_run_emits_s2_observables_in_both_selected_and_drift_arms() {
    let registry = ObservableRegistry::live_default();
    let mut observable_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS[..10].iter())
        .chain(
            [
                "live.mean_energy_integral",
                "live.trait_mass_mean_west",
                "live.trait_mass_mean_east",
            ]
            .iter(),
        )
        .map(|s| (*s).to_string())
        .collect();
    observable_ids.sort();
    observable_ids.dedup();

    let sel_manifest = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "s2-drift-sel".to_string(),
        name: "s2-drift-sel".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: s2_initials(12.0, true, true, true),
        interventions: vec![],
        seeds: vec![SEED],
        duration_ticks: 120,
        sample_period: 60,
        observable_ids: observable_ids.clone(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    };
    let mut drf_vals = sel_manifest.initial_conditions.values.clone();
    drf_vals.push((LIVE_KEY_DRIFT_CONTROL.to_string(), 1.0));
    let drf_manifest = ExperimentManifest {
        experiment_id: "s2-drift-drf".to_string(),
        name: "s2-drift-drf".to_string(),
        initial_conditions: InitialConditionSet::new(drf_vals),
        observable_ids,
        ..sel_manifest.clone()
    };

    let pair = run_live_drift_pair(&sel_manifest, &drf_manifest, &registry, SEED)
        .expect("paired drift pair succeeds with S2 observables");
    assert_eq!(pair.selected.status, RunStatus::Completed);
    assert_eq!(pair.drift.status, RunStatus::Completed);
    for id in [
        "live.mean_energy_integral",
        "live.trait_mass_mean_west",
        "live.trait_mass_mean_east",
    ] {
        let sel_v = value_of(&pair.selected.final_observables, id);
        let drf_v = value_of(&pair.drift.final_observables, id);
        let delta = pair
            .window_net_selection_deltas
            .get(id)
            .unwrap_or_else(|| panic!("{id} missing from window_net_selection_deltas"));
        assert!(sel_v.is_finite() && drf_v.is_finite() && delta.is_finite());
    }
    assert!(
        value_of(
            &pair.selected.final_observables,
            "live.mean_energy_integral"
        ) > 0.0,
        "selected arm must accumulate positive energy integral"
    );
}
