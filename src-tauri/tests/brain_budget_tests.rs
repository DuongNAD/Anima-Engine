//! **EB-S03** (zero allocation on the tick path) and **EB-S12** (per-agent and population memory).
//!
//! These are the two gates that decide whether evolved brains can reach the scale the project aims
//! at. ADR-0003 names per-agent memory as its central risk; leaving it as an estimate would be the
//! easiest way to discover the problem only once a run is large enough to hurt.
//!
//! Both are written to fail on a **silent regression**, not merely to record a number today.

mod common;

use anima_engine_lib::ai::model::{run_inference_batch, BrainModel, InferenceScratch};
use anima_engine_lib::core::agent_systems::{AgentInferenceRequest, AgentInferenceResponse};
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::evolution::brain_genotype::{
    action_index, ArchSpec, BrainGenotype, LearnScratch, EVOLVED_ARCH,
};
use bevy_ecs::prelude::Entity;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::any::Any;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

/// The allocator is process-global and its counter is not per-test, so the suites that measure it
/// must not run concurrently — the same interference that makes the terrain allocation test flaky.
type PanicPayload = Box<dyn Any + Send + 'static>;
type ContractResult = Result<(), PanicPayload>;

/// Disarms the process-wide counter during unwinding and stops it exactly once on success.
#[must_use = "dropping the guard immediately closes the allocation measurement window"]
struct AllocationTrackingGuard {
    active: bool,
}

impl AllocationTrackingGuard {
    fn start() -> Self {
        ALLOCATOR.start_tracking();
        Self { active: true }
    }

    #[must_use = "the measured allocation count must be asserted"]
    fn stop(mut self) -> usize {
        self.active = false;
        ALLOCATOR.stop_tracking()
    }
}

impl Drop for AllocationTrackingGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = ALLOCATOR.stop_tracking();
        }
    }
}

/// Report every failed contract, then preserve libtest's non-zero result by resuming the first one.
fn finish_contracts<const N: usize>(results: [(&str, ContractResult); N]) {
    let mut first_failure = None;
    for (name, result) in results {
        if let Err(payload) = result {
            eprintln!("contract failed: {name}");
            if first_failure.is_none() {
                first_failure = Some(payload);
            }
        }
    }
    if let Some(payload) = first_failure {
        std::panic::resume_unwind(payload);
    }
}

const AGENTS: usize = 64;

fn brain(seed: u64) -> BrainGenotype {
    BrainGenotype::random(EVOLVED_ARCH, &mut StdRng::seed_from_u64(seed)).unwrap()
}

fn requests(count: usize, with_brains: bool) -> Vec<AgentInferenceRequest> {
    (0..count)
        .map(|i| AgentInferenceRequest {
            entity: Entity::from_raw(i as u32),
            sensory_input: [0.3 + i as f32 * 0.001; 15],
            request_id: i as u64,
            brain: with_brains.then(|| std::sync::Arc::new(brain(i as u64))),
        })
        .collect()
}

// --- EB-S03: allocation on the tick path ---------------------------------------------------------

/// The four EB-S03 measurements run as one contract, and the reason is not style.
///
/// A mutex can serialise test *bodies*, but libtest still gives each `#[test]` its own thread and
/// spawning those threads allocates outside the lock. The allocator counts the whole process, so a
/// sibling's start-up landing inside a measured window is counted as if the code under test had
/// allocated. Under the load of a full desktop suite this made
/// `a_learning_step_allocates_nothing` claim four allocations in a function that makes none.
///
/// The aggregate test at the end is the only `#[test]` in this binary, which removes sibling-test
/// startup noise while preserving process-wide coverage of any delegated worker-thread work.
fn eb_s03_allocation_on_the_tick_path() {
    finish_contracts([
        (
            "evolved inference",
            std::panic::catch_unwind(evolved_inference_allocates_nothing_per_tick),
        ),
        (
            "learning step",
            std::panic::catch_unwind(a_learning_step_allocates_nothing),
        ),
        (
            "learned-network install",
            std::panic::catch_unwind(installing_a_learned_network_costs_one_allocation),
        ),
        (
            "shared model",
            std::panic::catch_unwind(the_shared_model_path_is_not_allocation_free_and_never_was),
        ),
    ]);
}

fn evolved_inference_allocates_nothing_per_tick() {
    eprintln!("phase: evolved inference, steady state");

    let model = BrainModel::new_seeded(15, 64, action_index::CPG_LEN, 1);
    let reqs = requests(AGENTS, true);
    let mut scratch = InferenceScratch::with_capacity(AGENTS);
    let mut responses: Vec<AgentInferenceResponse> = Vec::with_capacity(AGENTS);

    // Warm-up establishes every buffer's capacity, exactly as the first tick of a run does. What is
    // being measured is the steady state, not one-off setup.
    run_inference_batch(&model, &reqs, &mut responses, &mut scratch);

    let tracking_guard = AllocationTrackingGuard::start();
    for _ in 0..8 {
        run_inference_batch(&model, &reqs, &mut responses, &mut scratch);
    }
    let allocs = tracking_guard.stop();

    assert_eq!(
        allocs, 0,
        "per-agent inference must not allocate on the tick path"
    );
    assert_eq!(responses.len(), AGENTS);
}

fn a_learning_step_allocates_nothing() {
    eprintln!("phase: learn_step gradient update");

    let mut genotype = brain(9);
    let state = [0.4f32; 15];
    let action = [0.6f32; action_index::CPG_LEN];
    let mut scratch = LearnScratch::default();

    anima_engine_lib::evolution::brain_genotype::learn_step(
        &mut genotype,
        &state,
        &action,
        0.5,
        0.2,
        0.99,
        1e-3,
        &mut scratch,
    )
    .unwrap();

    let tracking_guard = AllocationTrackingGuard::start();
    for _ in 0..16 {
        anima_engine_lib::evolution::brain_genotype::learn_step(
            &mut genotype,
            &state,
            &action,
            0.5,
            0.2,
            0.99,
            1e-3,
            &mut scratch,
        )
        .unwrap();
    }
    let allocs = tracking_guard.stop();

    assert_eq!(
        allocs, 0,
        "the gradient step itself must be allocation-free; the cost is installing the result"
    );
}

fn installing_a_learned_network_costs_one_allocation() {
    // The documented exception. Learning replaces the whole network so an in-flight inference
    // request is never mutated underneath, and that replacement allocates once. This pins the cost
    // at *one* — if it ever became one per weight or per tick, that would be the regression.
    eprintln!("phase: installing a learned network");

    let mut agent = AgentBrain::from_genotype(brain(11));
    let updated = (*agent.genotype).clone();

    let tracking_guard = AllocationTrackingGuard::start();
    agent.set_learned(updated);
    let allocs = tracking_guard.stop();

    assert_eq!(
        allocs, 1,
        "installing a learned network should cost exactly one Arc allocation"
    );
}

fn the_shared_model_path_is_not_allocation_free_and_never_was() {
    // Reported rather than asserted to zero. The Burn batch clones its input buffer and builds a
    // tensor, which allocates — that predates ADR-0003 and is a property of routing through Burn.
    // The point of measuring it is that "evolved inference allocates nothing" above is a real
    // improvement over the legacy path, not an accident of how the test is written.
    eprintln!("phase: shared Burn model path");

    let model = BrainModel::new_seeded(15, 64, action_index::CPG_LEN, 2);
    let reqs = requests(AGENTS, false);
    let mut scratch = InferenceScratch::with_capacity(AGENTS);
    let mut responses = Vec::with_capacity(AGENTS);
    run_inference_batch(&model, &reqs, &mut responses, &mut scratch);

    let tracking_guard = AllocationTrackingGuard::start();
    run_inference_batch(&model, &reqs, &mut responses, &mut scratch);
    let shared_allocs = tracking_guard.stop();

    assert!(
        shared_allocs > 0,
        "if the Burn path ever becomes allocation-free, this note is stale and should be revisited"
    );
}

// --- EB-S12: memory budget -----------------------------------------------------------------------

/// Published per-agent ceiling for an evolved brain, in bytes.
///
/// `EVOLVED_ARCH` is 15→64→64→{8,1} = 5,769 `f32` = 23,076 bytes. The budget is that figure rounded
/// up, so a change to the architecture that inflates the per-agent cost has to be a deliberate edit
/// here rather than something noticed later at scale.
const BRAIN_BUDGET_BYTES: usize = 24 * 1024;

/// Ceiling for an agent that has also learned: it carries genome **and** learned network.
const LEARNING_BRAIN_BUDGET_BYTES: usize = 48 * 1024;

fn a_brains_memory_stays_within_the_published_budget() {
    // This allocates its fixtures, which is why the aggregate runs it after every EB-S03 window.
    let genome = brain(3);
    assert_eq!(
        genome.heap_bytes(),
        EVOLVED_ARCH.param_count() * 4,
        "heap accounting must track the weight vector"
    );
    assert!(
        genome.heap_bytes() <= BRAIN_BUDGET_BYTES,
        "an evolved brain costs {} bytes, over the {BRAIN_BUDGET_BYTES}-byte budget",
        genome.heap_bytes()
    );

    let mut agent = AgentBrain::from_genotype(genome);
    let before = agent.heap_bytes();
    agent.set_learned((*agent.genotype).clone());

    assert_eq!(
        agent.heap_bytes(),
        before * 2,
        "a learning agent carries two networks — the cost of the Baldwin half"
    );
    assert!(agent.heap_bytes() <= LEARNING_BRAIN_BUDGET_BYTES);
}

fn the_population_cost_is_stated_not_discovered() {
    // ADR-0003 records that per-agent weights are what stands between this design and the project's
    // scale target. The arithmetic is pinned here so the claim in the ADR cannot drift away from the
    // code: at ~22.5 KiB each, a million agents would need ~21 GiB of weights alone, which is why
    // Simulation-LOD is a precondition for scale rather than an optimisation.
    let per_agent = brain(5).heap_bytes();

    let for_population = |n: usize| n * per_agent;
    assert!(
        for_population(1_000) < 32 * 1024 * 1024,
        "a thousand agents should still fit comfortably"
    );
    assert!(
        for_population(1_000_000) > 16 * 1024 * 1024 * 1024_usize,
        "if a million agents ever fit in 16 GiB, the ADR's scaling note needs rewriting"
    );

    // The resident population Simulation-LOD would have to hold to stay inside a 1 GiB weight budget.
    let residents_per_gib = (1024 * 1024 * 1024) / per_agent;
    assert!(
        (40_000..60_000).contains(&residents_per_gib),
        "resident budget moved to {residents_per_gib} agents per GiB; update the ADR"
    );
}

fn a_smaller_architecture_is_the_lever_that_actually_moves_the_budget() {
    // The mitigations ADR-0003 lists are: fewer hidden units, quantisation, or sharing weights along
    // a lineage. Only the first is available today, so this records what it buys — halving the
    // hidden width is roughly a quarter of the memory, because the trunk-to-trunk matrix dominates.
    let full = ArchSpec::new(15, 64, action_index::COUNT).param_count();
    let half = ArchSpec::new(15, 32, action_index::COUNT).param_count();

    let ratio = full as f32 / half as f32;
    assert!(
        (3.0..4.5).contains(&ratio),
        "halving hidden width changed the parameter ratio to {ratio}; the memory note needs review"
    );
}

/// Run allocation contracts before memory fixtures allocate, with one libtest thread for the whole
/// binary. Every contract executes on a red suite; helper names are intentionally not independently
/// filterable. Multi-failure reporting depends on Cargo's test profile retaining unwind panics.
#[test]
fn brain_budget_contracts() {
    finish_contracts([
        (
            "EB-S03 tick-path allocation",
            std::panic::catch_unwind(eb_s03_allocation_on_the_tick_path),
        ),
        (
            "per-brain memory ceiling",
            std::panic::catch_unwind(a_brains_memory_stays_within_the_published_budget),
        ),
        (
            "population memory statement",
            std::panic::catch_unwind(the_population_cost_is_stated_not_discovered),
        ),
        (
            "smaller architecture leverage",
            std::panic::catch_unwind(
                a_smaller_architecture_is_the_lever_that_actually_moves_the_budget,
            ),
        ),
    ]);
}
