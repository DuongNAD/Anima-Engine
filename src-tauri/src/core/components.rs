use crate::evolution::genotype::MorphologyGenotype;
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};

#[derive(Component, Clone, Debug)]
pub struct Agent;

#[derive(Component, Clone, Copy, Debug)]
pub struct Predator;

#[derive(Component, Clone, Copy, Debug)]
pub struct Prey;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentClass {
    Predator,
    Prey,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitEntityType {
    Food,
    Predator,
    Prey,
    Obstacle,
    None,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct RaycastTelemetry {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub hit_distance: f32,
    pub hit_entity_type: HitEntityType,
    pub agent_id: u32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct CombatEvent {
    pub predator_id: u32,
    pub prey_id: u32,
    pub damage: f32,
    pub energy_transferred: f32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
// ts-rs does not read serde's rename_all, so it is repeated here. If these two ever disagree the
// generated TypeScript stops matching the wire format, which is the exact failure G1.4 exists to
// prevent — keep them in step.
#[ts(rename_all = "lowercase")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Predator,
    Prey,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Food {
    pub energy_value: f32,
    pub hydration_value: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Position(pub Vec3);

#[derive(Component, Clone, Copy, Debug)]
pub struct Rotation(pub Quat);

#[derive(Component, Clone, Copy, Debug)]
pub struct Velocity(pub Vec3);

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FeatureTracker {
    pub cumulative_distance: f32,
    pub cumulative_energy_decay: f32,
    pub tick_count: u32,
}

impl Default for FeatureTracker {
    fn default() -> Self {
        Self {
            cumulative_distance: 0.0,
            cumulative_energy_decay: 0.0,
            tick_count: 0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Segment {
    pub id: u32,
    pub length: f32,
    pub radius: f32,
    pub mass: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ParentLink(pub Entity);

#[derive(Component, Clone, Debug)]
pub struct ChildrenLinks(pub Vec<Entity>);

#[derive(Component, Clone, Copy, Debug)]
pub struct JointAxis(pub glam::Vec3);

#[derive(Component, Clone, Copy, Debug)]
pub struct ParentAgent(pub Entity);

#[derive(Component, Clone, Copy, Debug)]
pub struct SegmentJointForce(pub f32);

#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentParentLineageIds(pub Vec<String>);

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lake {
    pub current_water: f32,
    pub max_water: f32,
    pub replenishment_rate: f32,
}

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tree {
    pub current_fruit: f32,
    pub max_fruit: f32,
    pub fruit_growth_rate: f32,
    pub time_since_last_drop: f32,
    pub seed_drop_cooldown: f32,
    pub seed_spread_radius: f32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EnvironmentalElement {
    #[serde(rename = "type")]
    pub element_type: String, // "lake" | "tree"
    pub x: f32,
    pub y: f32, // Maps to Bevy's z coordinate
    pub radius: f32,
    pub resources: f32, // Maps to current water / current fruit
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EnvironmentalState {
    pub elements: Vec<EnvironmentalElement>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AgentMigrationData {
    pub genotype: MorphologyGenotype,
    pub homeostatic_state: crate::ai::hrrl::HomeostaticState,
    pub position: glam::Vec3,
    pub velocity: glam::Vec3,
    pub lineage_id: String,
    pub generation: u32,
    pub agent_class: AgentClass,
    pub parent_ids: Vec<String>,
    pub evaluation: Option<crate::core::engine::AgentEvaluation>,
    pub feature_tracker: Option<FeatureTracker>,
    pub last_transition_state: Option<crate::ai::hrrl::LastTransitionState>,
    #[serde(default)]
    pub source_port: u16,
    /// Travels with the individual. A migrating agent keeps the brain it had — invariant D02 says
    /// migration moves *the same creature*, and a creature that forgets everything on crossing a
    /// shard boundary is not the same creature. `None` is a legacy agent on the shared model.
    #[serde(default)]
    pub brain: Option<AgentBrain>,
}

#[derive(Clone, Debug)]
pub struct OutboundMigration {
    pub target_port: u16,
    pub data: AgentMigrationData,
    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ShardingConfig {
    pub local_port: u16,
    pub left_target_port: Option<u16>,
    pub right_target_port: Option<u16>,
}

#[derive(
    Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default,
)]
pub enum CognitiveState {
    #[default]
    Ready,
    PendingInference(u64),
    Cooldown,
}

#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InertiaComponent {
    pub target_velocity: Vec3,
    pub cpg_parameters: [f32; 4],
    pub ticks_pending: u32,
    pub ticks_elapsed: u32,
    pub decay_rate: f32,
}

impl Default for InertiaComponent {
    fn default() -> Self {
        Self {
            target_velocity: Vec3::ZERO,
            cpg_parameters: [1.0, 0.0, 1.0, 0.0],
            ticks_pending: 0,
            ticks_elapsed: 0,
            decay_rate: 0.0,
        }
    }
}

/// An action fires when its intent reaches this. Deterministic on purpose: a probabilistic gate
/// would need the RNG, and coupling ecological outcomes to draw order is exactly what
/// [`crate::core::resources::SimRng`] exists to avoid.
pub const ACTION_GATE_THRESHOLD: f32 = 0.5;

/// Per-agent control over ecological actions that are otherwise unconditional.
///
/// Today an agent's brain emits four CPG parameters and nothing else, so pheromone release, combat
/// and feeding all happen automatically on proximity — the brain is a gait controller, not a
/// decision-maker. Two agents cannot differ on "hunt or flee" because no channel exists to say it.
/// ADR-0003 decision 4 opens those channels.
///
/// **This step only installs the valves.** Every field defaults to fully open, reproducing today's
/// behaviour exactly; nothing writes to this component yet. Wiring it to evolved brain outputs comes
/// with `brain_genotype = Some(..)`. A missing component is read as fully open too, so saves and
/// worlds written before this existed behave identically (invariant D09).
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActionGates {
    /// Multiplies `PheromoneReleaser.strength`. Continuous rather than thresholded, because emission
    /// strength is already a continuous quantity: `1.0` is today's constant trail, `0.0` is silence.
    pub pheromone_emit: f32,
    /// Intent to engage prey within reach.
    pub attack_intent: f32,
    /// Intent to take energy from food within reach.
    pub feed_intent: f32,
}

impl Default for ActionGates {
    fn default() -> Self {
        Self {
            pheromone_emit: 1.0,
            attack_intent: 1.0,
            feed_intent: 1.0,
        }
    }
}

impl ActionGates {
    /// Reading for an agent that has no gates component — fully open, i.e. legacy behaviour.
    pub const OPEN: ActionGates = ActionGates {
        pheromone_emit: 1.0,
        attack_intent: 1.0,
        feed_intent: 1.0,
    };

    /// `None` means "no gates installed", which must read as fully open rather than fully shut —
    /// a missing component must never silently disable an agent's ecology.
    pub fn of(gates: Option<&ActionGates>) -> ActionGates {
        gates.copied().unwrap_or(ActionGates::OPEN)
    }

    pub fn attacks(&self) -> bool {
        self.attack_intent >= ACTION_GATE_THRESHOLD
    }

    pub fn feeds(&self) -> bool {
        self.feed_intent >= ACTION_GATE_THRESHOLD
    }

    /// Emission multiplier, clamped so a wild brain output cannot inject unbounded pheromone or
    /// subtract from the field.
    pub fn pheromone_scale(&self) -> f32 {
        self.pheromone_emit.clamp(0.0, 1.0)
    }
}

/// An agent's own brain: the heritable genome, plus whatever it has learned since birth.
///
/// Presence of this component is what distinguishes an evolved-brain agent from a legacy one. An
/// agent without it falls back to the single shared [`crate::ai::model::BrainModel`], which is the
/// rollback path ADR-0003 decision 7 requires — the same shape as `exotic_energy = None` in ADR-0002.
///
/// The two weight sets are kept apart deliberately. `genotype` is what reproduction copies;
/// `learned` is runtime state that dies with the individual. Writing learned weights back into the
/// genome would be Lamarckian inheritance, which ADR-0003 decision 2 rules out — the Baldwin effect
/// works by evolving *the capacity to learn*, not by inheriting what was learned.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentBrain {
    /// Behind an `Arc` so the per-tick inference request can carry the genome as a refcount bump
    /// rather than copying ~23 KiB of weights per agent per tick, which would break the
    /// zero-allocation rule the tick path is held to.
    pub genotype: std::sync::Arc<crate::evolution::brain_genotype::BrainGenotype>,
    /// The brain as it stands after lifetime learning. `None` means no learning has happened and the
    /// genome is the live network — every agent, unless lifetime learning is switched on. Saved so a
    /// restored or migrated individual does not forget what it knew (D02).
    ///
    /// Held as a whole genotype behind an `Arc`, not a bare weight vector, so [`Self::live`] can hand
    /// inference a shareable network whichever branch it takes. Learning replaces this `Arc` rather
    /// than mutating through it: an update allocates once, which is why learning is throttled to an
    /// interval rather than run every tick.
    #[serde(default)]
    pub learned: Option<std::sync::Arc<crate::evolution::brain_genotype::BrainGenotype>>,
}

impl AgentBrain {
    pub fn from_genotype(genotype: crate::evolution::brain_genotype::BrainGenotype) -> Self {
        Self {
            genotype: std::sync::Arc::new(genotype),
            learned: None,
        }
    }

    /// The network inference should actually use: what the individual has learned, else its genome.
    pub fn live(&self) -> &std::sync::Arc<crate::evolution::brain_genotype::BrainGenotype> {
        self.learned.as_ref().unwrap_or(&self.genotype)
    }

    /// The weights inference should actually use.
    pub fn live_weights(&self) -> &[f32] {
        &self.live().weights
    }

    /// Install the result of a lifetime-learning step.
    ///
    /// Takes the whole updated network rather than mutating in place: the previous one may still be
    /// referenced by an in-flight inference request, and tearing weights out from under it would
    /// make an agent's action depend on thread timing.
    pub fn set_learned(&mut self, learned: crate::evolution::brain_genotype::BrainGenotype) {
        self.learned = Some(std::sync::Arc::new(learned));
    }

    /// Energy per second this brain costs to keep running, at `cost_per_1k` per 1,000 parameters.
    ///
    /// Charged against the *genome's* size, not the learned network's: learning does not grow the
    /// brain, so it must not raise the bill. Returns `0.0` for a non-finite or negative rate rather
    /// than producing a nonsensical charge that would then have to be reconciled against the energy
    /// ledger.
    pub fn metabolic_cost(&self, cost_per_1k: f32) -> f32 {
        if !cost_per_1k.is_finite() || cost_per_1k <= 0.0 {
            return 0.0;
        }
        (self.genotype.arch.param_count() as f32 / 1000.0) * cost_per_1k
    }

    /// Heap bytes this agent's brain occupies: its genome, plus the learned network when it has one.
    ///
    /// An agent that has learned carries **two** networks, so switching lifetime learning on roughly
    /// doubles the per-agent cost. Gate **EB-S12** publishes both figures rather than leaving the
    /// second one to be discovered at scale.
    pub fn heap_bytes(&self) -> usize {
        self.genotype.heap_bytes() + self.learned.as_ref().map_or(0, |l| l.heap_bytes())
    }

    /// Reject a brain whose learned network no longer matches its genome's architecture — a mismatch
    /// means a save was written by a build with a different layout, and running it would silently
    /// produce noise rather than behaviour.
    pub fn validate(&self) -> Result<(), crate::evolution::brain_genotype::BrainGenotypeError> {
        self.genotype.validate()?;
        if let Some(learned) = &self.learned {
            learned.validate()?;
            if learned.arch != self.genotype.arch {
                return Err(
                    crate::evolution::brain_genotype::BrainGenotypeError::InvalidArch(learned.arch),
                );
            }
        }
        Ok(())
    }
}

#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SensoryBufferComponent {
    pub buffer: Vec<f32>,
}

impl Default for SensoryBufferComponent {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(15),
        }
    }
}
