//! Regression tests for `FOUNDING-PREY`: `SimulationEngine::start` genesis must split
//! founders 70% Prey / 30% Predator using `FoundingPlan::is_prey(i)` at any configured
//! `ANIMA_FOUNDING_POPULATION`, while keeping default 10-founder genesis bit-identical.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::ecs::AgentClass;
use anima_engine_lib::core::engine::SimulationEngine;
use anima_engine_lib::core::resources::{FoundingPlan, DEFAULT_FOUNDING_POPULATION};
use anima_engine_lib::core::snapshot::checksum_bytes;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CHILD_ROLE_ENV: &str = "ANIMA_FOUNDING_PREY_CHILD";
const RESULT_MARKER: &str = "FOUNDING_PREY_RESULT";
const TEST_SEED_STR: &str = "42";
const READY_TIMEOUT: Duration = Duration::from_secs(15);

/// Canonical checksum of the default 10-founder genesis cohort (`ANIMA_DETERMINISTIC=1`,
/// `ANIMA_SEED=42`, `ANIMA_FOUNDING_POPULATION` unset), captured from pre-fix trunk (`89be4447`).
const EXPECTED_DEFAULT_10_FOUNDER_GENESIS_CHECKSUM: u32 = 1_737_656_444;

#[derive(Serialize)]
struct CanonicalFounder<'a> {
    index: usize,
    lineage_id: &'a str,
    class: AgentClass,
    start_pos_bits: [u32; 3],
    segment_nodes: usize,
    segment_edges: usize,
}

fn spawn_and_snapshot_child(
    test_fn: &str,
    founding_population: Option<&str>,
) -> (usize, usize, usize, u32, String) {
    let exe = std::env::current_exe().expect("current test binary path");
    let mut cmd = Command::new(exe);
    cmd.arg("--exact")
        .arg(test_fn)
        .arg("--nocapture")
        .env(CHILD_ROLE_ENV, "1")
        .env("ANIMA_DETERMINISTIC", "1")
        .env("ANIMA_SEED", TEST_SEED_STR)
        .env("ANIMA_USE_GPU", "0");
    match founding_population {
        Some(val) => {
            cmd.env("ANIMA_FOUNDING_POPULATION", val);
        }
        None => {
            cmd.env_remove("ANIMA_FOUNDING_POPULATION");
        }
    }

    let output = cmd
        .output()
        .expect("spawn founding_prey child test process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "child process for {test_fn} failed ({:?}):\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    let line = stdout
        .lines()
        .find(|l| l.starts_with(RESULT_MARKER))
        .unwrap_or_else(|| {
            panic!("missing {RESULT_MARKER} line in child output:\nstdout:\n{stdout}\nstderr:\n{stderr}")
        });

    let mut count = None;
    let mut prey = None;
    let mut predator = None;
    let mut checksum = None;
    let mut species = None;
    for token in line.split_whitespace().skip(1) {
        if let Some((k, v)) = token.split_once('=') {
            match k {
                "count" => count = v.parse::<usize>().ok(),
                "prey" => prey = v.parse::<usize>().ok(),
                "predator" => predator = v.parse::<usize>().ok(),
                "checksum" => checksum = v.parse::<u32>().ok(),
                "species" => species = Some(v.to_string()),
                _ => {}
            }
        }
    }

    (
        count.expect("count token"),
        prey.expect("prey token"),
        predator.expect("predator token"),
        checksum.expect("checksum token"),
        species.expect("species token"),
    )
}

fn run_child_and_print() {
    let plan = FoundingPlan::from_env();
    let engine = SimulationEngine::new();
    let evolution_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: HashMap::new(),
        grid_resolution: 50,
    }));

    engine.start::<tauri::test::MockRuntime>(
        None,
        evolution_settings,
        evolution_running,
        map_elites_grid,
    );

    let deadline = Instant::now() + READY_TIMEOUT;
    while (!engine.get_status().running || engine.get_status().tick_count < 1)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        engine.get_status().tick_count >= 1,
        "engine did not advance to tick 1 within {READY_TIMEOUT:?}"
    );

    let run_seed = engine
        .run_seed
        .read()
        .expect("run_seed read lock")
        .expect("started engine exposes run_seed");
    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .expect("sim thread accepts save request");
    let saved = rx
        .recv_timeout(READY_TIMEOUT)
        .expect("sim thread answers save request")
        .expect("live simulation saves cleanly");
    engine.stop();

    let founders_by_id: HashMap<&str, _> = saved
        .agents
        .iter()
        .filter(|a| a.generation == 0)
        .map(|a| (a.lineage_id.as_str(), a))
        .collect();
    assert_eq!(
        founders_by_id.len(),
        plan.count,
        "expected {} generation-0 founders, found {}",
        plan.count,
        founders_by_id.len()
    );

    let founder_ids = anima_engine_lib::core::determinism::RunIdentity::new(run_seed, "founder");
    let mut prey = 0usize;
    let mut predator = 0usize;
    let mut species = String::with_capacity(plan.count);
    let mut canonical = Vec::with_capacity(plan.count);

    for index in 0..plan.count {
        let id = founder_ids.next_id();
        let agent = founders_by_id
            .get(id.as_str())
            .unwrap_or_else(|| panic!("missing founder {index} with lineage_id {id}"));
        match agent.class {
            AgentClass::Prey => {
                prey += 1;
                species.push('P');
            }
            AgentClass::Predator => {
                predator += 1;
                species.push('R');
            }
        }
        let pos = agent.evaluation.start_position;
        canonical.push(CanonicalFounder {
            index,
            lineage_id: &agent.lineage_id,
            class: agent.class,
            start_pos_bits: [pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()],
            segment_nodes: agent.genotype.nodes.len(),
            segment_edges: agent.genotype.edges.len(),
        });
    }

    let json = serde_json::to_string(&canonical).expect("serialize canonical founders");
    let checksum = checksum_bytes(&json);
    println!(
        "{RESULT_MARKER} count={} prey={prey} predator={predator} checksum={checksum} species={species}",
        plan.count
    );
}

#[test]
fn founding_1000_agents_matches_founding_plan_split() {
    if std::env::var_os(CHILD_ROLE_ENV).is_some() {
        run_child_and_print();
        return;
    }

    let (count, prey, predator, _checksum, species) = spawn_and_snapshot_child(
        "founding_1000_agents_matches_founding_plan_split",
        Some("1000"),
    );
    let plan = FoundingPlan::parse("1000").expect("1000 is valid");
    let expected_prey = (0..plan.count).filter(|&i| plan.is_prey(i)).count();
    let expected_predator = plan.count - expected_prey;

    assert_eq!(count, 1000);
    assert_eq!(
        prey, expected_prey,
        "FOUNDING-PREY: expected {expected_prey} prey for 1,000 founders (FoundingPlan::is_prey), got {prey} prey / {predator} predators"
    );
    assert_eq!(predator, expected_predator);
    for (i, ch) in species.chars().enumerate() {
        let is_prey = ch == 'P';
        assert_eq!(
            is_prey,
            plan.is_prey(i),
            "founder index {i} species mismatch against FoundingPlan::is_prey"
        );
    }
}

#[test]
fn default_10_founder_genesis_keeps_species_per_index_and_checksum() {
    if std::env::var_os(CHILD_ROLE_ENV).is_some() {
        run_child_and_print();
        return;
    }

    let (count, prey, predator, checksum_a, species) = spawn_and_snapshot_child(
        "default_10_founder_genesis_keeps_species_per_index_and_checksum",
        None,
    );
    let (count_b, _prey_b, _pred_b, checksum_b, species_b) = spawn_and_snapshot_child(
        "default_10_founder_genesis_keeps_species_per_index_and_checksum",
        None,
    );

    assert_eq!(count, DEFAULT_FOUNDING_POPULATION);
    assert_eq!(count_b, DEFAULT_FOUNDING_POPULATION);
    assert_eq!(prey, 7);
    assert_eq!(predator, 3);
    assert_eq!(species, "PPPPPPPRRR");
    assert_eq!(species_b, species);
    assert_eq!(
        checksum_a, checksum_b,
        "default 10-founder genesis must be deterministic"
    );
    assert_eq!(
        checksum_a, EXPECTED_DEFAULT_10_FOUNDER_GENESIS_CHECKSUM,
        "default 10-founder genesis checksum changed (got {checksum_a})"
    );
}
