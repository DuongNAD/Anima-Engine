//! N-ary Causal Branch Tree (Feature 6).
//!
//! Provides arbitrary counterfactual branching tree execution, single-pass checkpoint
//! memoization across prefix timelines, and multi-dimensional trajectory divergence metrics.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::core::build_identity::build_id;
use crate::core::causal::CausalLedger;
use crate::core::exotic_energy::ExoticIntervention;
use crate::core::experiment::{
    ExperimentError, ExperimentManifest, ObservableRegistry, ObservableSpec, MAX_DURATION_TICKS,
};
use crate::core::experiment_runner::{
    ExperimentModel, RunProvenance, RunResult, RunStatus,
};
use crate::core::intervention::{InterventionCommand, InterventionQueue};
use crate::core::scenario::StateSample;
use crate::core::sim_clock::SimClock;

pub const BRANCH_TREE_SCHEMA_VERSION: u16 = 1;
pub const MAX_BRANCH_NODES: usize = 1024;

pub type BranchNodeId = u64;

/// Deterministic 64-bit seed derivation function.
pub fn derived_seed(base_seed: u64, stream_id: u64) -> u64 {
    let mut h = base_seed.wrapping_mul(0x517cc1b727220a95) ^ stream_id.wrapping_mul(0x9e3779b97f4a7c15);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^= h >> 33;
    h
}

/// Strategy for initializing the deterministic random number generator for a branch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum BranchSeedStrategy {
    /// Seamlessly continue from the parent's RNG stream at the fork tick (exact cloned state).
    InheritStream,
    /// Derive a distinct deterministic seed using `derived_seed(parent_seed, stream_id ^ (fork_tick << 32))`.
    DerivedStream { stream_id: u64 },
    /// Explicitly provide a fixed seed for the branch.
    ExplicitSeed { seed: u64 },
}

impl Default for BranchSeedStrategy {
    fn default() -> Self {
        Self::InheritStream
    }
}

/// Specification of an individual branch node in the manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BranchNodeSpecification {
    pub id: BranchNodeId,
    /// `None` indicates the root branch.
    pub parent_id: Option<BranchNodeId>,
    /// Tick at which this branch forks from its parent (0 for root).
    #[serde(default)]
    pub fork_tick: u64,
    /// Target duration / end tick for this branch.
    pub duration_ticks: u64,
    /// Causal interventions attached to this branch starting at or after `fork_tick`.
    #[serde(default)]
    pub interventions: Vec<InterventionCommand>,
    /// Runtime exotic energy forcings attached to this branch.
    #[serde(default)]
    pub exotic_interventions: Vec<ExoticIntervention>,
    /// Seed strategy for this branch.
    #[serde(default)]
    pub seed_strategy: BranchSeedStrategy,
    /// Descriptive human-readable label or hypothesis.
    #[serde(default)]
    pub label: String,
}

/// Declaration of a full N-ary Causal Branch Tree Experiment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchTreeManifest {
    pub schema_version: u16,
    pub tree_id: String,
    pub name: String,
    /// Base experiment manifest defining world laws, initial conditions, and default grid/seed.
    pub base_manifest: ExperimentManifest,
    /// List of all branch node specifications forming the tree.
    pub nodes: Vec<BranchNodeSpecification>,
    /// Optional pairs of branch IDs to compute trajectory divergence analysis on.
    #[serde(default)]
    pub comparisons: Vec<(BranchNodeId, BranchNodeId)>,
}

impl BranchTreeManifest {
    /// Validate topology, tick bounds, and intervention applicability across the tree.
    pub fn validate(&self, registry: &ObservableRegistry) -> Result<(), ExperimentError> {
        if self.schema_version != BRANCH_TREE_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "branch_tree_manifest".into(),
                found: self.schema_version,
                supported: BRANCH_TREE_SCHEMA_VERSION,
            });
        }
        if self.tree_id.trim().is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "tree_id".into(),
            });
        }
        if self.nodes.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "branch tree nodes".into(),
            });
        }
        if self.nodes.len() > MAX_BRANCH_NODES {
            return Err(ExperimentError::ResourceLimit {
                field: "branch tree nodes".into(),
                limit: MAX_BRANCH_NODES,
                found: self.nodes.len(),
            });
        }

        self.base_manifest.validate(registry)?;

        let mut node_map: HashMap<BranchNodeId, &BranchNodeSpecification> = HashMap::new();
        let mut root_count = 0usize;
        let mut root_id = None;

        for node in &self.nodes {
            if node_map.insert(node.id, node).is_some() {
                return Err(ExperimentError::DuplicateId {
                    context: "branch node".into(),
                    id: node.id.to_string(),
                });
            }
            if node.parent_id.is_none() {
                root_count += 1;
                root_id = Some(node.id);
                if node.fork_tick != 0 {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("root branch node {} fork_tick must be 0", node.id),
                        value: node.fork_tick as f64,
                        min: 0.0,
                        max: 0.0,
                    });
                }
            }
            if node.duration_ticks > MAX_DURATION_TICKS || node.duration_ticks == 0 {
                return Err(ExperimentError::OutOfRange {
                    field: format!("branch node {} duration_ticks", node.id),
                    value: node.duration_ticks as f64,
                    min: 1.0,
                    max: MAX_DURATION_TICKS as f64,
                });
            }
            if node.fork_tick >= node.duration_ticks {
                return Err(ExperimentError::OutOfRange {
                    field: format!("branch node {} fork_tick must be < duration_ticks", node.id),
                    value: node.fork_tick as f64,
                    min: 0.0,
                    max: (node.duration_ticks.saturating_sub(1)) as f64,
                });
            }

            // Validate interventions
            for cmd in &node.interventions {
                let end_exclusive = cmd
                    .start_tick
                    .checked_add(cmd.effective_duration())
                    .ok_or_else(|| ExperimentError::InvalidLaw {
                        reason: format!(
                            "intervention {} start_tick + duration overflows u64",
                            cmd.id
                        ),
                    })?;

                let first_sim_tick = if node.parent_id.is_some() {
                    node.fork_tick + 1
                } else {
                    1
                };

                if cmd.start_tick < node.fork_tick {
                    return Err(ExperimentError::InapplicableIntervention {
                        id: cmd.id,
                        reason: format!(
                            "intervention {} start_tick {} is before branch {} fork_tick {}; it belongs to the parent prefix",
                            cmd.id, cmd.start_tick, node.id, node.fork_tick
                        ),
                    });
                }

                if cmd.start_tick > node.duration_ticks || end_exclusive <= first_sim_tick {
                    return Err(ExperimentError::InapplicableIntervention {
                        id: cmd.id,
                        reason: format!(
                            "intervention {} active window [{}, {}) has no active ticks in branch {} execution window [{}, {}]",
                            cmd.id, cmd.start_tick, end_exclusive, node.id, first_sim_tick, node.duration_ticks
                        ),
                    });
                }
            }
        }

        if root_count != 1 {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "branch tree must have exactly one root node (parent_id = None), found {root_count}"
                ),
            });
        }

        // Verify DAG tree structure (no cycles, every parent exists, topological connectivity)
        let root_id = root_id.unwrap();
        let mut in_degrees: HashMap<BranchNodeId, usize> = HashMap::new();
        let mut adj: HashMap<BranchNodeId, Vec<BranchNodeId>> = HashMap::new();

        for &id in node_map.keys() {
            in_degrees.insert(id, 0);
            adj.insert(id, Vec::new());
        }

        for node in &self.nodes {
            if let Some(pid) = node.parent_id {
                let parent = node_map.get(&pid).ok_or_else(|| ExperimentError::InvalidLaw {
                    reason: format!(
                        "branch node {} references non-existent parent_id {}",
                        node.id, pid
                    ),
                })?;

                if node.fork_tick > parent.duration_ticks {
                    return Err(ExperimentError::OutOfRange {
                        field: format!(
                            "branch node {} fork_tick {} exceeds parent {} duration_ticks {}",
                            node.id, node.fork_tick, parent.id, parent.duration_ticks
                        ),
                        value: node.fork_tick as f64,
                        min: 0.0,
                        max: parent.duration_ticks as f64,
                    });
                }

                if node.fork_tick < parent.fork_tick {
                    return Err(ExperimentError::OutOfRange {
                        field: format!(
                            "branch node {} fork_tick {} is earlier than parent {} fork_tick {}",
                            node.id, node.fork_tick, parent.id, parent.fork_tick
                        ),
                        value: node.fork_tick as f64,
                        min: parent.fork_tick as f64,
                        max: parent.duration_ticks as f64,
                    });
                }

                adj.get_mut(&pid).unwrap().push(node.id);
                *in_degrees.get_mut(&node.id).unwrap() += 1;
            }
        }

        // Topological Kahn's check
        let mut queue = VecDeque::new();
        queue.push_back(root_id);
        let mut visited_count = 0usize;

        while let Some(u) = queue.pop_front() {
            visited_count += 1;
            for &v in &adj[&u] {
                let deg = in_degrees.get_mut(&v).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(v);
                }
            }
        }

        if visited_count != self.nodes.len() {
            return Err(ExperimentError::InvalidLaw {
                reason: "branch tree topology contains cycles or disconnected components".into(),
            });
        }

        // Validate comparison IDs
        for &(a, b) in &self.comparisons {
            if !node_map.contains_key(&a) || !node_map.contains_key(&b) {
                return Err(ExperimentError::InvalidLaw {
                    reason: format!(
                        "comparison refers to unknown branch node ({a}, {b})"
                    ),
                });
            }
        }

        Ok(())
    }

    /// Compute topological node execution order.
    pub fn topological_order(&self) -> Result<Vec<BranchNodeSpecification>, ExperimentError> {
        let mut node_map: HashMap<BranchNodeId, &BranchNodeSpecification> = HashMap::new();
        let mut in_degrees: HashMap<BranchNodeId, usize> = HashMap::new();
        let mut adj: HashMap<BranchNodeId, Vec<BranchNodeId>> = HashMap::new();
        let mut root_id = None;

        for node in &self.nodes {
            node_map.insert(node.id, node);
            in_degrees.insert(node.id, 0);
            adj.insert(node.id, Vec::new());
            if node.parent_id.is_none() {
                root_id = Some(node.id);
            }
        }

        for node in &self.nodes {
            if let Some(pid) = node.parent_id {
                adj.get_mut(&pid).unwrap().push(node.id);
                *in_degrees.get_mut(&node.id).unwrap() += 1;
            }
        }

        let mut order = Vec::with_capacity(self.nodes.len());
        let mut queue = VecDeque::new();
        if let Some(r) = root_id {
            queue.push_back(r);
        }

        while let Some(u) = queue.pop_front() {
            order.push(node_map[&u].clone());
            // Sort children deterministically by id
            let mut children = adj[&u].clone();
            children.sort_unstable();
            for v in children {
                let deg = in_degrees.get_mut(&v).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(v);
                }
            }
        }

        Ok(order)
    }
}

/// Runtime execution status of a branch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum BranchExecutionStatus {
    Pending,
    Completed,
    Failed { tick: u64, reason: String },
}

/// Executed state of a branch node in the tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchNodeResult {
    pub spec: BranchNodeSpecification,
    pub status: BranchExecutionStatus,
    pub resolved_seed: u64,
    pub run_result: Option<RunResult>,
    pub children: Vec<BranchNodeId>,
}

/// Comprehensive report of the executed Branch Tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchTreeReport {
    pub tree_id: String,
    pub name: String,
    pub root_id: BranchNodeId,
    pub manifest_fingerprint: u64,
    pub nodes: HashMap<BranchNodeId, BranchNodeResult>,
    pub divergences: Vec<PairwiseBranchDivergence>,
}

impl BranchTreeReport {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// -----------------------------------------------------------------------------
// Trajectory Divergence Metrics
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObservableDivergence {
    pub observable_id: String,
    pub mean_delta: f64,
    pub max_abs_delta: f64,
    pub rmsd: f64,
    pub relative_divergence: f64,
    pub dtw_distance: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BiomassPhaseDivergence {
    pub mean_distance: f64,
    pub max_distance: f64,
    pub final_distance: f64,
    pub trajectory_length_a: f64,
    pub trajectory_length_b: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PairwiseBranchDivergence {
    pub branch_a_id: BranchNodeId,
    pub branch_b_id: BranchNodeId,
    pub common_start_tick: u64,
    pub common_end_tick: u64,
    pub time_to_divergence: Option<u64>,
    pub phase_space: BiomassPhaseDivergence,
    pub observables: Vec<ObservableDivergence>,
}

/// Compute Dynamic Time Warping (DTW) distance between two scalar trajectories.
pub fn dynamic_time_warping(series_a: &[f64], series_b: &[f64]) -> f64 {
    let n = series_a.len();
    let m = series_b.len();
    if n == 0 || m == 0 {
        return 0.0;
    }

    let mut dtw = vec![vec![f64::INFINITY; m + 1]; n + 1];
    dtw[0][0] = 0.0;

    for i in 1..=n {
        for j in 1..=m {
            let cost = (series_a[i - 1] - series_b[j - 1]).powi(2);
            let prev = dtw[i - 1][j].min(dtw[i][j - 1]).min(dtw[i - 1][j - 1]);
            dtw[i][j] = cost + prev;
        }
    }

    dtw[n][m].sqrt()
}

/// Compute pairwise trajectory divergence between two completed branch runs.
pub fn compute_pairwise_divergence(
    branch_a_id: BranchNodeId,
    run_a: &RunResult,
    branch_b_id: BranchNodeId,
    run_b: &RunResult,
    fork_tick: u64,
) -> PairwiseBranchDivergence {
    let mut a_samples: HashMap<u64, HashMap<String, f64>> = HashMap::new();
    for sample in &run_a.series {
        let obs_map: HashMap<String, f64> = sample.observables.iter().cloned().collect();
        a_samples.insert(sample.tick, obs_map);
    }

    let mut common_ticks: Vec<u64> = run_b
        .series
        .iter()
        .map(|s| s.tick)
        .filter(|t| a_samples.contains_key(t) && *t >= fork_tick)
        .collect();
    common_ticks.sort_unstable();

    let common_start_tick = common_ticks.first().copied().unwrap_or(fork_tick);
    let common_end_tick = common_ticks.last().copied().unwrap_or(fork_tick);

    // Phase space compartments: plants, herbivores, predators, detritus
    let mut phase_distances = Vec::new();
    let mut traj_len_a = 0.0;
    let mut traj_len_b = 0.0;
    let mut prev_pt_a: Option<[f64; 4]> = None;
    let mut prev_pt_b: Option<[f64; 4]> = None;
    let mut time_to_divergence = None;

    let get_compartments = |map: &HashMap<String, f64>| -> [f64; 4] {
        let p = map.get("plants").copied().or_else(|| map.get("live.standing_crop").copied()).unwrap_or(0.0);
        let h = map.get("herbivores").copied().or_else(|| map.get("live.herbivore_count").copied()).unwrap_or(0.0);
        let c = map.get("predators").copied().or_else(|| map.get("live.predator_count").copied()).unwrap_or(0.0);
        let d = map.get("detritus").copied().unwrap_or(0.0);
        [p, h, c, d]
    };

    for &t in &common_ticks {
        let vals_a = &a_samples[&t];
        let vals_b_sample = run_b.series.iter().find(|s| s.tick == t).unwrap();
        let vals_b: HashMap<String, f64> = vals_b_sample.observables.iter().cloned().collect();

        let pt_a = get_compartments(vals_a);
        let pt_b = get_compartments(&vals_b);

        let dist_sq: f64 = pt_a.iter().zip(pt_b.iter()).map(|(a, b)| (a - b).powi(2)).sum();
        let dist = dist_sq.sqrt();
        phase_distances.push(dist);

        if time_to_divergence.is_none() && dist > 1.0 && t > fork_tick {
            time_to_divergence = Some(t);
        }

        if let Some(prev) = prev_pt_a {
            let step: f64 = pt_a.iter().zip(prev.iter()).map(|(a, b)| (a - b).powi(2)).sum();
            traj_len_a += step.sqrt();
        }
        if let Some(prev) = prev_pt_b {
            let step: f64 = pt_b.iter().zip(prev.iter()).map(|(a, b)| (a - b).powi(2)).sum();
            traj_len_b += step.sqrt();
        }
        prev_pt_a = Some(pt_a);
        prev_pt_b = Some(pt_b);
    }

    let phase_mean = if phase_distances.is_empty() {
        0.0
    } else {
        phase_distances.iter().sum::<f64>() / phase_distances.len() as f64
    };
    let phase_max = phase_distances.iter().copied().fold(0.0, f64::max);
    let phase_final = phase_distances.last().copied().unwrap_or(0.0);

    let phase_space = BiomassPhaseDivergence {
        mean_distance: phase_mean,
        max_distance: phase_max,
        final_distance: phase_final,
        trajectory_length_a: traj_len_a,
        trajectory_length_b: traj_len_b,
    };

    // Per-observable divergence metrics
    let mut all_obs_ids: HashSet<String> = HashSet::new();
    for sample in &run_a.series {
        all_obs_ids.extend(sample.observables.iter().map(|(n, _)| n.clone()));
    }
    for sample in &run_b.series {
        all_obs_ids.extend(sample.observables.iter().map(|(n, _)| n.clone()));
    }

    let mut obs_keys: Vec<String> = all_obs_ids.into_iter().collect();
    obs_keys.sort();

    let mut observables = Vec::new();

    for obs in obs_keys {
        let mut series_a = Vec::new();
        let mut series_b = Vec::new();
        let mut deltas = Vec::new();

        for &t in &common_ticks {
            let vals_a = &a_samples[&t];
            let vals_b_sample = run_b.series.iter().find(|s| s.tick == t).unwrap();
            let vals_b: HashMap<String, f64> = vals_b_sample.observables.iter().cloned().collect();

            if let (Some(&ya), Some(&yb)) = (vals_a.get(&obs), vals_b.get(&obs)) {
                series_a.push(ya);
                series_b.push(yb);
                deltas.push(ya - yb);
            }
        }

        if deltas.is_empty() {
            continue;
        }

        let m = deltas.len() as f64;
        let mean_delta = deltas.iter().sum::<f64>() / m;
        let max_abs_delta = deltas.iter().map(|d| d.abs()).fold(0.0, f64::max);
        let rmsd = (deltas.iter().map(|d| d.powi(2)).sum::<f64>() / m).sqrt();

        // Compute baseline variance of series_b
        let b_mean = series_b.iter().sum::<f64>() / m;
        let b_var = series_b.iter().map(|v| (v - b_mean).powi(2)).sum::<f64>() / m;
        let b_std = b_var.sqrt();
        let relative_divergence = rmsd / (b_std + 1e-6);

        let dtw_dist = if series_a.len() > 1 && series_a.len() <= 1000 {
            Some(dynamic_time_warping(&series_a, &series_b))
        } else {
            None
        };

        observables.push(ObservableDivergence {
            observable_id: obs,
            mean_delta,
            max_abs_delta,
            rmsd,
            relative_divergence,
            dtw_distance: dtw_dist,
        });
    }

    PairwiseBranchDivergence {
        branch_a_id,
        branch_b_id,
        common_start_tick,
        common_end_tick,
        time_to_divergence,
        phase_space,
        observables,
    }
}

// -----------------------------------------------------------------------------
// Checkpoint Memoized Execution Engine
// -----------------------------------------------------------------------------

/// Checkpoint cache record holding full state snapshot and deterministic stream position.
struct CheckpointEntry<S> {
    snapshot: S,
    rng: StdRng,
    clock: SimClock,
    resolved_seed: u64,
}

fn specs_for_emitted(
    registry: &ObservableRegistry,
    series: &[StateSample],
    final_observables: &[(String, f64)],
) -> (Vec<ObservableSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();
    let names = series
        .iter()
        .flat_map(|s| s.observables.iter().map(|(n, _)| n))
        .chain(final_observables.iter().map(|(n, _)| n));
    for name in names {
        if !seen.insert(name.clone()) {
            continue;
        }
        match registry.get(name) {
            Some(s) => specs.push(s.clone()),
            None => warnings.push(format!("emitted observable '{name}' has no registry spec")),
        }
    }
    (specs, warnings)
}

/// Execute an N-ary Causal Branch Tree using single-pass checkpoint memoization.
pub fn run_branch_tree<M: ExperimentModel>(
    manifest: &BranchTreeManifest,
    registry: &ObservableRegistry,
) -> Result<BranchTreeReport, ExperimentError> {
    manifest.validate(registry)?;

    let nodes_order = manifest.topological_order()?;
    let root_id = nodes_order[0].id;
    let base_seed = manifest.base_manifest.seeds.first().copied().unwrap_or(1);
    let sample_period = manifest.base_manifest.sample_period;
    let grid = (16usize, 16usize);

    // Map children for checkpoint capture points
    let mut child_fork_points: HashMap<BranchNodeId, HashSet<u64>> = HashMap::new();
    let mut children_map: HashMap<BranchNodeId, Vec<BranchNodeId>> = HashMap::new();

    for node in &manifest.nodes {
        child_fork_points.insert(node.id, HashSet::new());
        children_map.insert(node.id, Vec::new());
    }

    for node in &manifest.nodes {
        if let Some(pid) = node.parent_id {
            child_fork_points.get_mut(&pid).unwrap().insert(node.fork_tick);
            children_map.get_mut(&pid).unwrap().push(node.id);
        }
    }

    // Memoization cache: (node_id, tick) -> CheckpointEntry
    let mut checkpoint_cache: HashMap<(BranchNodeId, u64), CheckpointEntry<M::Snapshot>> = HashMap::new();
    let mut node_results: HashMap<BranchNodeId, BranchNodeResult> = HashMap::new();

    for spec in nodes_order {
        let node_id = spec.id;
        let fork_ticks = child_fork_points[&node_id].clone();

        // 1. Initialize model and RNG stream
        let (mut model, mut rng, mut clock, start_tick, resolved_seed, mut ledger, initial_observables) = match spec.parent_id {
            None => {
                // Root node
                let model = M::from_manifest(
                    &manifest.base_manifest.laws,
                    &manifest.base_manifest.initial_conditions,
                    &manifest.base_manifest.exotic_interventions,
                    base_seed,
                    grid,
                    spec.duration_ticks,
                )?;
                let rng = StdRng::seed_from_u64(base_seed);
                let clock = SimClock::default();
                let obs = model.observables();
                (model, rng, clock, 0u64, base_seed, CausalLedger::new(), obs)
            }
            Some(parent_id) => {
                // Resume from parent checkpoint
                let cache_key = (parent_id, spec.fork_tick);
                let entry = checkpoint_cache.get(&cache_key).ok_or_else(|| {
                    ExperimentError::InvalidLaw {
                        reason: format!(
                            "missing memoized checkpoint for parent {} at tick {}",
                            parent_id, spec.fork_tick
                        ),
                    }
                })?;

                let mut model = M::from_snapshot(&entry.snapshot)?;
                if !spec.exotic_interventions.is_empty() {
                    let mut combined_forcings = manifest.base_manifest.exotic_interventions.clone();
                    combined_forcings.extend(spec.exotic_interventions.clone());
                    model.reconfigure_forcings(&combined_forcings, spec.duration_ticks)?;
                }

                let (rng, resolved_seed) = match spec.seed_strategy {
                    BranchSeedStrategy::InheritStream => (entry.rng.clone(), entry.resolved_seed),
                    BranchSeedStrategy::DerivedStream { stream_id } => {
                        let s = derived_seed(entry.resolved_seed, stream_id ^ (spec.fork_tick << 32));
                        (StdRng::seed_from_u64(s), s)
                    }
                    BranchSeedStrategy::ExplicitSeed { seed } => {
                        (StdRng::seed_from_u64(seed), seed)
                    }
                };

                let clock = entry.clock.clone();
                let obs = model.observables();
                (model, rng, clock, spec.fork_tick, resolved_seed, CausalLedger::new(), obs)
            }
        };

        // Capture initial checkpoint if any child forks at start_tick (genesis fork at 0 or co-temporal child fork)
        if fork_ticks.contains(&start_tick) {
            let snapshot = model.snapshot();
            checkpoint_cache.insert(
                (node_id, start_tick),
                CheckpointEntry {
                    snapshot,
                    rng: rng.clone(),
                    clock: clock.clone(),
                    resolved_seed,
                },
            );
        }

        // Combine base interventions and node-specific interventions
        let mut combined_interventions = manifest.base_manifest.interventions.clone();
        combined_interventions.extend(spec.interventions.clone());
        let queue = InterventionQueue::new(combined_interventions);

        let mut series = Vec::new();
        if sample_period > 0 {
            series.push(StateSample {
                tick: start_tick,
                observables: initial_observables.clone(),
            });
        }

        let execution_status = BranchExecutionStatus::Completed;

        // 2. Step simulation
        for tick in (start_tick + 1)..=spec.duration_ticks {
            clock.tick = tick;
            let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();

            model.step(&clock, &active, &mut ledger, &mut rng);

            if sample_period > 0 && clock.band_fires(sample_period) {
                series.push(StateSample {
                    tick,
                    observables: model.observables(),
                });
            }

            // Capture intermediate checkpoint if any child forks at this tick
            if fork_ticks.contains(&tick) {
                let snapshot = model.snapshot();
                checkpoint_cache.insert(
                    (node_id, tick),
                    CheckpointEntry {
                        snapshot,
                        rng: rng.clone(),
                        clock: clock.clone(),
                        resolved_seed,
                    },
                );
            }
        }

        let final_checksum = model.checksum();
        let final_observables = model.observables();
        let exotic_budget = model.exotic_budget();

        let (observable_specs, warnings) = specs_for_emitted(registry, &series, &final_observables);

        let provenance = RunProvenance {
            experiment_id: manifest.tree_id.clone(),
            run_id: format!("{}#node_{}", manifest.tree_id, node_id),
            parent_run_id: spec.parent_id.map(|pid| format!("{}#node_{}", manifest.tree_id, pid)),
            fork_tick: if spec.parent_id.is_some() { Some(spec.fork_tick) } else { None },
            seed: resolved_seed,
            manifest_fingerprint: manifest.base_manifest.fingerprint(),
            law_fingerprint: manifest.base_manifest.laws.fingerprint(),
            registry_fingerprint: registry.fingerprint(),
            model_version: M::model_version().to_string(),
            build_id: build_id(),
        };

        let run_result = RunResult {
            provenance,
            status: RunStatus::Completed,
            final_checksum,
            final_observables,
            series,
            ledger,
            exotic_budget,
            observable_specs,
            warnings,
        };

        let mut children = children_map[&node_id].clone();
        children.sort_unstable();

        node_results.insert(
            node_id,
            BranchNodeResult {
                spec,
                status: execution_status,
                resolved_seed,
                run_result: Some(run_result),
                children,
            },
        );
    }

    // 3. Compute pairwise trajectory divergence comparisons
    let mut divergences = Vec::new();
    for &(id_a, id_b) in &manifest.comparisons {
        if let (Some(res_a), Some(res_b)) = (node_results.get(&id_a), node_results.get(&id_b)) {
            if let (Some(run_a), Some(run_b)) = (&res_a.run_result, &res_b.run_result) {
                let fork_tick = res_a.spec.fork_tick.max(res_b.spec.fork_tick);
                let div = compute_pairwise_divergence(id_a, run_a, id_b, run_b, fork_tick);
                divergences.push(div);
            }
        }
    }

    Ok(BranchTreeReport {
        tree_id: manifest.tree_id.clone(),
        name: manifest.name.clone(),
        root_id,
        manifest_fingerprint: manifest.base_manifest.fingerprint(),
        nodes: node_results,
        divergences,
    })
}
