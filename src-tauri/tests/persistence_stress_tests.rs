mod common;

use common::stall_detector::StallDetector;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::simulation_lifecycle::{
    SavedSimulationState, SerializedPheromoneGrid, SimulationEngine,
};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ---- Watchdog for `test_100_save_load_cycles` ---------------------------------------------------
//
// This test hung the CI Rust job twice in a row on 2026-07-26 (run 30190881586, both attempts): it
// was the last target to start, produced no result for ~83 minutes, and the job died at its
// `timeout-minutes: 90` cap. GitHub reports that as `cancelled`, so the run list made it look like
// somebody had stopped the build.
//
// The whole target takes **6.2 seconds** on a dev machine, so this is not a slow test that needs a
// bigger budget. Something blocks, and it blocks only on the `push`-to-main runs — three
// `pull_request` runs of the same code passed in about 8 minutes each. That smells like a race in
// engine start/stop rather than a systematic difference, and the standing suspect is the thread
// lifecycle debt in `STATE_OF_THE_PROJECT.md` §3.7: four threads spawned per `start`, no unified
// cancellation, and this loop does it 101 times.
//
// The watchdog does not fix that. It makes the failure **legible**: instead of 90 minutes of silence
// the log names the cycle and the phase that was in flight. Two runs were burned learning only the
// target name; the next one should hand over the line number.
//
// `process::exit` rather than a panic, deliberately: the hang is in another thread, and a panic on
// the watchdog thread would be swallowed while libtest kept waiting for the blocked one.
//
// ## The diagnostic needs `--nocapture` to be seen
//
// Measured, and it cost a wrong assumption first: libtest's output capture swallows `eprintln!` from
// the test thread **and** from any thread it spawned. I had assumed capture was thread-local and that
// a spawned watchdog would bypass it; a probe showed neither line appears without `--nocapture`, and
// both appear with it. Since the watchdog ends in `process::exit`, libtest never gets the chance to
// flush a captured buffer either.
//
// CI passes `--nocapture` for exactly this reason (`.github/workflows/ci.yml`, the `cargo test`
// step). Running this locally without it, the watchdog still saves you from a ninety-minute hang —
// the process dies in two minutes with exit code 101 — but it dies **silently**. Add `--nocapture`
// when you are the one chasing it.

/// Cycle currently in flight; `0` means the loop has not started yet, [`CYCLES_DONE`] means it
/// finished and the process is now expected to exit.
static WATCHDOG_CYCLE: AtomicU64 = AtomicU64::new(0);

/// Sentinel for "every cycle completed". Distinguishing this from "stuck on the last cycle" is not
/// cosmetic: the observed CI failure is that all 100 cycles **pass** and the binary then never
/// exits, so a watchdog that only watches cycle progress reports the wrong thing at the wrong time.
const CYCLES_DONE: u64 = u64::MAX;

/// How long the process may take to exit after the last cycle before that counts as the leak.
///
/// Two minutes, weighed both ways. Too short risks turning a green build red on a slow runner, which
/// is worse than the status quo for an intermittent fault. Too long gives back the whole point, which
/// is beating the 90-minute job cap. The target completes in ~6s locally and the entire 71-target
/// Rust job takes ~8 minutes on a passing CI run, so a two-minute exit is already pathological —
/// while still leaving a 45x margin under the cap.
const EXIT_GRACE_SECS: u64 = 120;
/// Which call inside the cycle is in flight — see [`PHASES`].
static WATCHDOG_PHASE: AtomicU64 = AtomicU64::new(0);

const PHASES: [&str; 6] = [
    "not started",
    "engine.start",
    "settle sleep",
    "save request + recv",
    "verify state",
    "engine.stop",
];

/// Ten times the observed CI budget for the whole target and roughly 200x the local runtime, so this
/// can only fire on a genuine stall — never on a slow runner.
const WATCHDOG_BUDGET_SECS: u64 = 300;

/// The budget, overridable by `ANIMA_STRESS_WATCHDOG_SECS`.
///
/// Read-only, and read once. `STATE_OF_THE_PROJECT.md` §4 warns that tests touching the environment
/// need a shared mutex — that applies to tests that *set* variables and race each other. Nothing in
/// this binary writes one.
fn watchdog_budget() -> Duration {
    let secs = std::env::var("ANIMA_STRESS_WATCHDOG_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(WATCHDOG_BUDGET_SECS);
    Duration::from_secs(secs)
}

fn phase(p: u64) {
    WATCHDOG_PHASE.store(p, Ordering::SeqCst);
}

/// Kill the process with diagnostics if the cycle loop stops making progress.
fn spawn_watchdog() {
    let budget = watchdog_budget();
    let tick = Duration::from_secs(1).min(budget);
    let exit_grace = Duration::from_secs(EXIT_GRACE_SECS).min(budget);
    thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut detector = StallDetector::new(budget);
        let mut done_detector = StallDetector::new(exit_grace);
        loop {
            thread::sleep(tick);
            let cycle = WATCHDOG_CYCLE.load(Ordering::SeqCst);

            // The observed CI failure lives here, not above: every cycle passes and the binary then
            // fails to exit, because `engine.stop()` does not reclaim the threads each `start`
            // spawned. Reproduced locally 2026-07-26 — two runs in three left a live process behind
            // with 79 threads, and it held the test executable hard enough to fail the next link
            // with LNK1104.
            if cycle == CYCLES_DONE {
                if !done_detector.observe(CYCLES_DONE, tick) {
                    continue;
                }
                eprintln!(
                    "\n=== persistence_stress_tests watchdog ===\n\
                     All 100 cycles passed, and this binary has not exited {}s later.\n\
                     That is the leak, not slowness: the target completes in ~6s.\n\
                     `engine.stop()` is not reclaiming the threads each `start` spawns, so the \
                     process outlives its own tests — which on CI hangs `cargo test` until the \
                     job's 90-minute cap (run 30190881586, both attempts).\n\
                     Root cause is the thread lifecycle debt in STATE_OF_THE_PROJECT.md §3.7.\n\
                     Failing loudly here instead of hanging.\n\
                     =========================================",
                    done_detector.stalled_for().as_secs(),
                );
                thread::sleep(Duration::from_millis(200));
                std::process::exit(101);
            }

            if !detector.observe(cycle, tick) {
                continue;
            }
            let phase = WATCHDOG_PHASE.load(Ordering::SeqCst) as usize;
            eprintln!(
                "\n=== persistence_stress_tests watchdog ===\n\
                 test_100_save_load_cycles made no progress for {}s.\n\
                 stalled in cycle {} of 100, at: {}\n\
                 total elapsed: {}s. The whole target takes ~6s locally.\n\
                 =========================================",
                detector.stalled_for().as_secs(),
                cycle,
                PHASES.get(phase).unwrap_or(&"unknown"),
                started.elapsed().as_secs(),
            );
            // Flush ordering matters: libtest captures stderr, so give it a moment to drain before
            // the process disappears.
            thread::sleep(Duration::from_millis(200));
            std::process::exit(101);
        }
    });
}

#[test]
fn test_load_zero_agents() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let engine = SimulationEngine::new();
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    // Construct a SavedSimulationState with 0 agents
    let state = SavedSimulationState {
        tick_count: 500,
        active_environment_event: anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable,
        food_spawn_settings: anima_engine_lib::core::ecs::FoodSpawnSettings {
            max_food_count: 15,
            default_energy: 25.0,
            default_hydration: 15.0,
        },
        map_bounds: anima_engine_lib::core::ecs::MapBounds {
            min: glam::Vec3::new(-50.0, 0.0, -50.0),
            max: glam::Vec3::new(50.0, 10.0, 50.0),
        },
        epoch_manager: anima_engine_lib::core::ecs::EpochManager {
            ticks_per_epoch: 500,
            current_epoch_ticks: 120,
            current_epoch: 3,
        },
        pheromone_grid: SerializedPheromoneGrid {
            values: vec![0.0; 16384],
            diffusion_rate: 0.12,
            decay_rate: 0.04,
        },
        foods: vec![
            anima_engine_lib::core::simulation_lifecycle::SerializedFood {
                position: glam::Vec3::new(1.0, 0.0, 2.0),
                energy_value: 30.0,
                hydration_value: 20.0,
            },
        ],
        agents: vec![], // Zero agents
        evolution_settings: EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        },
        map_elites_grid: MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        },
        chronicle_history: vec![],
        lineage_nodes: vec![],
        lineage_relations: vec![],
        lakes: vec![],
        trees: vec![],
        world_identity: Default::default(),
        // Everything not named above (closed-energy state, RNG stream position, season clock)
        // takes its zero value, which every restore path reads as "nothing was saved here".
        ..anima_engine_lib::core::simulation_state::empty_saved_state_for_tests()
    };

    // Put into pending load state
    *engine.pending_load_state.lock().unwrap() = Some(state);

    // Start simulation
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Wait a short moment for ticks to happen
    thread::sleep(Duration::from_millis(200));

    // Verify it is running and tick count increased
    let status = engine.get_status();
    assert!(status.running);
    assert!(status.tick_count > 500);

    // Verify agents telemetry has length 0
    let states = engine.agent_states.read().unwrap();
    assert_eq!(
        states.len(),
        0,
        "Agent states should be empty when loading state with 0 agents"
    );

    engine.stop();
}

#[test]
fn test_load_corrupted_json() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Per-process. `corrupted_state.json` in the shared system temp directory is a name any other
    // process could also be writing, and this test asserts on the exact bytes it wrote back.
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("corrupted_state_{}.json", std::process::id()));

    // Write invalid JSON content
    std::fs::write(&file_path, "{ corrupted_json_missing_brackets: ").unwrap();

    let read_result = std::fs::read_to_string(&file_path);
    assert!(read_result.is_ok());
    let json_str = read_result.unwrap();

    // Deserialize should fail
    let deserialized = serde_json::from_str::<SavedSimulationState>(&json_str);
    assert!(
        deserialized.is_err(),
        "Expected parsing error for corrupted JSON"
    );

    // Cleanup
    let _ = std::fs::remove_file(file_path);
}

#[test]
fn test_load_missing_file() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let missing_path = "non_existent_file_persistence_999.json";

    // File read should fail with NotFound
    let read_result = std::fs::read_to_string(missing_path);
    assert!(read_result.is_err());
    let err = read_result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn test_100_save_load_cycles() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let engine = SimulationEngine::new();
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    // Start simulation initially
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Let it run to spawn initial 10 agents and food
    thread::sleep(Duration::from_millis(150));

    // Save initial state
    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .expect("Failed to send initial save request");
    let mut current_state = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Timeout waiting for initial save")
        .expect("initial save must not be refused");

    assert!(current_state.tick_count > 0);
    assert_eq!(
        current_state.agents.len(),
        10,
        "Should spawn 10 agents by default"
    );

    engine.stop();

    // Armed before tracking starts, on purpose: spawning a thread allocates, and `common/allocator.rs`
    // documents at length how a thread starting mid-measurement gets counted against the code under
    // test. Nothing asserts on the count here today, but the next person to add an assertion should
    // not have to discover this.
    //
    // See the watchdog comment at the top of this file: two CI runs died at the 90-minute job cap
    // inside this loop and the log named only the target. This makes the next one name the cycle.
    spawn_watchdog();

    // Track initial allocations in hot-loop tick to establish a baseline
    ALLOCATOR.start_tracking();

    for cycle in 1..=100u64 {
        WATCHDOG_CYCLE.store(cycle, Ordering::SeqCst);
        // Coarse progress in the log too, so a stall is visible while it is happening rather than
        // only in the watchdog's post-mortem.
        if cycle % 10 == 1 {
            eprintln!("save/load cycle {cycle}/100");
        }

        // Load the saved state from previous cycle
        *engine.pending_load_state.lock().unwrap() = Some(current_state.clone());

        // Start engine
        phase(1);
        engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&evo_settings),
            Arc::clone(&evo_running),
            Arc::clone(&map_elites_grid),
        );

        // Run briefly to allow simulation ticks to execute
        phase(2);
        thread::sleep(Duration::from_millis(15));

        // Save state at the end of this run
        phase(3);
        let (tx_cycle, rx_cycle) = std::sync::mpsc::channel();
        engine
            .save_request_tx
            .send(tx_cycle)
            .unwrap_or_else(|e| panic!("Failed to send save request in cycle {}: {:?}", cycle, e));

        let new_state = rx_cycle
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|e| panic!("Timeout/error receiving save in cycle {}: {:?}", cycle, e))
            .unwrap_or_else(|why| panic!("save refused in cycle {}: {}", cycle, why));

        // Verify that the data is not glitched/NaN
        phase(4);
        for agent in &new_state.agents {
            assert!(
                agent.root_position.x.is_finite(),
                "NaN/Inf coordinate in cycle {}",
                cycle
            );
            assert!(
                agent.root_position.y.is_finite(),
                "NaN/Inf coordinate in cycle {}",
                cycle
            );
            assert!(
                agent.root_position.z.is_finite(),
                "NaN/Inf coordinate in cycle {}",
                cycle
            );
            for segment in &agent.segments {
                assert!(
                    segment.position.x.is_finite(),
                    "NaN/Inf segment coordinate in cycle {}",
                    cycle
                );
                assert!(
                    segment.position.y.is_finite(),
                    "NaN/Inf segment coordinate in cycle {}",
                    cycle
                );
                assert!(
                    segment.position.z.is_finite(),
                    "NaN/Inf segment coordinate in cycle {}",
                    cycle
                );
            }
        }

        assert!(new_state.tick_count >= current_state.tick_count);
        current_state = new_state;

        // Stop the engine to finish the cycle
        phase(5);
        engine.stop();
    }

    eprintln!("all 100 save/load cycles completed");
    // Hands the watchdog over to its second job: from here the only thing left is for this process
    // to exit, and failing to do that is the bug that hung CI twice.
    WATCHDOG_CYCLE.store(CYCLES_DONE, Ordering::SeqCst);
    let _allocations = ALLOCATOR.stop_tracking();

    // We expect some allocations because starting/stopping the engine, serializing to state,
    // and thread initialization/joining allocates memory. But it should not lock, panic,
    // or leak unbounded memory resources (thread handles and channels are fully joined and dropped).
    // Let's assert that the engine is stopped successfully.
    let status = engine.get_status();
    assert!(!status.running);
}
