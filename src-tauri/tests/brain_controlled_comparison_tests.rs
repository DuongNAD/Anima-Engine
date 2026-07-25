//! Step 6 of ADR-0003 — the seeded controlled comparison.
//!
//! Everything before this could be argued from the code. This is where the claim gets measured: with
//! the same seed and the same world, does turning evolved brains on actually produce behavioural
//! diversity, and does leaving them off leave the simulation as it was?
//!
//! ## Why this harness exists
//!
//! The live loop runs inference on a worker thread inside `SimulationEngine::start`, which needs the
//! Tauri runtime. So the tick is driven here directly: run the schedule, then pump the inference
//! channel through [`run_inference_batch`] — the **same function** the worker calls, not a
//! reimplementation. A stand-in that duplicated the logic would be measuring itself.
//!
//! ## What this does and does not establish
//!
//! It measures behaviour of the live ECS loop. It does **not** exercise the evolution thread, so
//! MAP-Elites archive coverage — the other half of EB-S11's original phrasing — stays out of reach
//! here and remains pending.

use anima_engine_lib::ai::cpg::{update_cpg_system, TimeStep};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::model::{run_inference_batch, BrainModel, InferenceScratch};
use anima_engine_lib::ai::pheromone::{agent_release_pheromone_system, PheromoneGrid};
use anima_engine_lib::core::agent_systems::{
    action_resolution_system, sensory_system, InferenceChannels, InferenceRequestBatch,
    InferenceResponseBatch, ACTION_SLOTS,
};
use anima_engine_lib::core::components::{ActionGates, AgentBrain};
use anima_engine_lib::core::ecs::{
    Agent, CognitiveState, FoodSpawnSettings, InertiaComponent, ParentAgent, Position, Predator,
    Prey, Rotation, SensoryBufferComponent, Velocity,
};
use anima_engine_lib::core::resources::{BrainPolicy, SimRng};
use anima_engine_lib::core::world_systems::{combat_system, detect_food_collisions_system};
use anima_engine_lib::evolution::brain_genotype::{action_index, BrainGenotype, EVOLVED_ARCH};
use anima_engine_lib::physics::dynamics::{integrate_physics_system, RigidBody};
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use glam::{Quat, Vec3};
use rand::rngs::StdRng;
use rand::SeedableRng;

const AGENTS: usize = 8;
const TICKS: usize = 40;

/// Serialises world construction across tests.
///
/// `init_world` reads the shared world artifact and the on-disk terrain cache, which several tests
/// building worlds at once would contend for. Model construction itself no longer needs protecting —
/// `BrainModel::new_seeded` supplies its own weights rather than drawing from Burn's process-wide
/// generator — but the filesystem side remains shared. Poison is recovered so a real assertion
/// failure reports as itself instead of cascading.
static BUILD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// How an agent's world-facing behaviour is summarised for comparison. Floats are compared bitwise
/// via `to_bits`, because "the trajectory is unchanged" means unchanged, not nearly unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Trace {
    position: [u32; 3],
    energy: u32,
    cpg: [u32; action_index::CPG_LEN],
    gates: Option<[u32; 3]>,
}

struct Sim {
    world: World,
    schedule: Schedule,
    req_rx: Receiver<InferenceRequestBatch>,
    res_tx: Sender<InferenceResponseBatch>,
    recycle_res_rx: Receiver<InferenceResponseBatch>,
    model: BrainModel,
    scratch: InferenceScratch,
    agents: Vec<Entity>,
}

/// Build a world with `AGENTS` agents. `evolved` turns on per-agent brains; `install_gates` controls
/// whether the `ActionGates` component exists at all, which is how the "before the gates were added"
/// state is reconstructed.
fn build(seed: u64, evolved: bool, install_gates: bool) -> Sim {
    let _lock = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = anima_engine_lib::core::ecs::init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(SimRng::from_seed(seed));
    world.insert_resource(PheromoneGrid::new(0.05, 0.02));
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(BrainPolicy {
        evolved,
        arch: EVOLVED_ARCH,
        ..Default::default()
    });

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    for _ in 0..16 {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(64),
        });
        let _ = recycle_res_tx.send(InferenceResponseBatch {
            responses: Vec::with_capacity(64),
        });
    }
    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });

    // Brains are drawn from a stream of their own so the founding population is identical whether or
    // not the rest of the world has consumed draws — otherwise "same seed" would silently mean
    // "same seed and same number of prior draws".
    let mut brain_rng = StdRng::seed_from_u64(seed ^ 0xB4A1);

    let mut agents = Vec::new();
    for i in 0..AGENTS {
        let pos = Vec3::new((i as f32) * 3.0 - 10.0, 0.0, (i % 3) as f32 * 2.0);
        let entity = world
            .spawn((
                Agent,
                Position(pos),
                Rotation(Quat::IDENTITY),
                Velocity(Vec3::ZERO),
                RigidBody {
                    mass: 1.0,
                    velocity: Vec3::ZERO,
                    force: Vec3::ZERO,
                },
                HomeostaticState {
                    energy: 80.0,
                    energy_target: 100.0,
                    hydration: 80.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
                CognitiveState::Ready,
                InertiaComponent::default(),
                SensoryBufferComponent::default(),
            ))
            .id();
        world.entity_mut(entity).insert(ParentAgent(entity));
        if i % 4 == 3 {
            world.entity_mut(entity).insert(Predator);
        } else {
            world.entity_mut(entity).insert(Prey);
        }
        if install_gates {
            world.entity_mut(entity).insert(ActionGates::default());
        }
        if evolved {
            let genotype = BrainGenotype::random(EVOLVED_ARCH, &mut brain_rng).unwrap();
            world
                .entity_mut(entity)
                .insert(AgentBrain::from_genotype(genotype));
        }
        agents.push(entity);
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((
        sensory_system,
        action_resolution_system,
        update_cpg_system.after(action_resolution_system),
        integrate_physics_system.after(update_cpg_system),
        agent_release_pheromone_system.after(integrate_physics_system),
        detect_food_collisions_system.after(integrate_physics_system),
        combat_system.after(integrate_physics_system),
    ));

    Sim {
        world,
        schedule,
        req_rx,
        res_tx,
        recycle_res_rx,
        model: BrainModel::new_seeded(15, 64, action_index::CPG_LEN, seed),
        scratch: InferenceScratch::with_capacity(64),
        agents,
    }
}

impl Sim {
    fn tick(&mut self) {
        self.schedule.run(&mut self.world);

        // Stand in for the worker thread by calling exactly what it calls.
        while let Ok(req_batch) = self.req_rx.try_recv() {
            let mut res_batch =
                self.recycle_res_rx
                    .try_recv()
                    .unwrap_or_else(|_| InferenceResponseBatch {
                        responses: Vec::with_capacity(64),
                    });
            run_inference_batch(
                &self.model,
                &req_batch.requests,
                &mut res_batch.responses,
                &mut self.scratch,
            );
            let _ = self.res_tx.send(res_batch);
        }
    }

    fn run(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    fn traces(&self) -> Vec<Trace> {
        self.agents
            .iter()
            .map(|&e| {
                let entity = self.world.entity(e);
                let pos = entity.get::<Position>().unwrap().0;
                let homeo = entity.get::<HomeostaticState>().unwrap();
                let inertia = entity.get::<InertiaComponent>().unwrap();
                Trace {
                    position: [pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()],
                    energy: homeo.energy.to_bits(),
                    cpg: [
                        inertia.cpg_parameters[0].to_bits(),
                        inertia.cpg_parameters[1].to_bits(),
                        inertia.cpg_parameters[2].to_bits(),
                        inertia.cpg_parameters[3].to_bits(),
                    ],
                    gates: entity.get::<ActionGates>().map(|g| {
                        [
                            g.pheromone_emit.to_bits(),
                            g.attack_intent.to_bits(),
                            g.feed_intent.to_bits(),
                        ]
                    }),
                }
            })
            .collect()
    }

    /// Every agent's policy applied to one identical observation.
    ///
    /// This is the measurement that matters. Comparing agents' *positions* would show divergence
    /// even under a single shared brain, because they stand in different places and therefore see
    /// different things. Holding the observation fixed isolates the policy itself.
    fn policy_responses(&mut self, probe: [f32; 15]) -> Vec<[f32; ACTION_SLOTS]> {
        use anima_engine_lib::core::agent_systems::AgentInferenceRequest;

        let requests: Vec<AgentInferenceRequest> = self
            .agents
            .iter()
            .enumerate()
            .map(|(i, &e)| AgentInferenceRequest {
                entity: e,
                sensory_input: probe,
                request_id: i as u64,
                brain: self
                    .world
                    .entity(e)
                    .get::<AgentBrain>()
                    .map(|b| std::sync::Arc::clone(&b.genotype)),
            })
            .collect();

        let mut responses = Vec::new();
        run_inference_batch(&self.model, &requests, &mut responses, &mut self.scratch);
        responses.into_iter().map(|r| r.actions).collect()
    }
}

fn distinct(rows: &[[f32; ACTION_SLOTS]]) -> usize {
    let mut keys: Vec<Vec<u32>> = rows
        .iter()
        .map(|r| r.iter().map(|v| v.to_bits()).collect())
        .collect();
    keys.sort();
    keys.dedup();
    keys.len()
}

// --- the baseline is undisturbed ----------------------------------------------------------------

#[test]
fn the_run_is_reproducible_under_one_seed() {
    // Everything below compares two runs, so this has to hold first or none of it means anything.
    let mut a = build(7, false, true);
    let mut b = build(7, false, true);
    a.run(TICKS);
    b.run(TICKS);
    assert_eq!(a.traces(), b.traces());
}

#[test]
fn installing_the_gates_changed_nothing_with_them_open() {
    // EB-S05 over a whole run rather than a single system call. `install_gates = false` reconstructs
    // the component layout as it was before ADR-0003 step 4; with the gates open the two must agree
    // bit-for-bit on position, energy and locomotion across every tick.
    let mut without = build(9, false, false);
    let mut with = build(9, false, true);
    without.run(TICKS);
    with.run(TICKS);

    let strip = |t: Vec<Trace>| -> Vec<Trace> {
        t.into_iter()
            .map(|mut x| {
                x.gates = None;
                x
            })
            .collect()
    };
    assert_eq!(strip(with.traces()), strip(without.traces()));
}

#[test]
fn with_brains_off_every_agent_shares_one_policy() {
    // The baseline's defining property, and the thing ADR-0003 exists to change: given the same
    // observation, every agent must answer identically, because there is only one brain.
    let mut sim = build(11, false, true);
    sim.run(TICKS);

    let responses = sim.policy_responses([0.37; 15]);
    assert_eq!(responses.len(), AGENTS);
    assert_eq!(
        distinct(&responses),
        1,
        "a shared model cannot produce more than one policy"
    );
}

#[test]
fn with_brains_off_no_agent_ever_gains_one() {
    let mut sim = build(13, false, true);
    sim.run(TICKS);
    assert!(sim
        .agents
        .iter()
        .all(|&e| sim.world.entity(e).get::<AgentBrain>().is_none()));
}

#[test]
fn with_brains_off_no_gate_ever_closes() {
    // The shared path fills the gate slots with the open default every tick, so no agent may drift
    // shut. A closed gate here would mean the legacy path had started suppressing ecology.
    let mut sim = build(15, false, true);
    sim.run(TICKS);
    for &e in &sim.agents {
        let gates = sim.world.entity(e).get::<ActionGates>().copied().unwrap();
        assert_eq!(gates, ActionGates::OPEN, "a legacy agent's gate moved");
    }
}

// --- turning it on produces diversity -----------------------------------------------------------

#[test]
fn with_brains_on_the_population_holds_many_policies() {
    // EB-S11's core claim. Same observation, different answers — the measurement that was impossible
    // to make before this ADR, because the quantity did not exist.
    let mut sim = build(11, true, true);
    sim.run(TICKS);

    let responses = sim.policy_responses([0.37; 15]);
    assert_eq!(
        distinct(&responses),
        AGENTS,
        "every agent carries its own genome, so every policy should differ"
    );
}

#[test]
fn evolved_agents_disagree_about_ecology_not_just_gait() {
    // Diversity confined to the CPG block would leave the engine where it started: agents that walk
    // differently but eat, hunt and signal identically. The gates are the part that matters.
    let mut sim = build(17, true, true);
    sim.run(TICKS);
    let responses = sim.policy_responses([0.42; 15]);

    let gate_rows: Vec<[f32; ACTION_SLOTS]> = responses
        .iter()
        .map(|r| {
            let mut only_gates = [0.0f32; ACTION_SLOTS];
            only_gates[action_index::CPG_LEN..].copy_from_slice(&r[action_index::CPG_LEN..]);
            only_gates
        })
        .collect();

    assert!(
        distinct(&gate_rows) > 1,
        "brains differed only in locomotion; the ecological channel is not being used"
    );
}

#[test]
fn evolved_gates_actually_reach_the_agents() {
    // The policies differ — but do the differences land on the components the ecology reads? At
    // least one agent must end the run holding gates that are not the open default.
    let mut sim = build(19, true, true);
    sim.run(TICKS);

    let moved = sim
        .agents
        .iter()
        .filter(|&&e| {
            sim.world.entity(e).get::<ActionGates>().copied().unwrap() != ActionGates::OPEN
        })
        .count();
    assert!(
        moved > 0,
        "no evolved agent's gates ever moved off the open default"
    );
}

#[test]
fn evolved_runs_are_reproducible_too() {
    // Diversity must come from the genomes, not from nondeterminism. Same seed, same population,
    // same trajectory — otherwise "we observed diversity" is unfalsifiable.
    let mut a = build(23, true, true);
    let mut b = build(23, true, true);
    a.run(TICKS);
    b.run(TICKS);
    assert_eq!(a.traces(), b.traces());
}

#[test]
fn a_different_seed_founds_a_different_population() {
    let mut a = build(29, true, true);
    let mut b = build(31, true, true);
    a.run(TICKS);
    b.run(TICKS);
    assert_ne!(a.traces(), b.traces());
}

#[test]
fn turning_brains_on_changes_the_run() {
    // The flag has to actually do something. If this passed with equal traces, everything above
    // would be measuring an inert feature.
    let mut off = build(37, false, true);
    let mut on = build(37, true, true);
    off.run(TICKS);
    on.run(TICKS);
    assert_ne!(on.traces(), off.traces());
}
