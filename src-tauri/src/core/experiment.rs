//! # Experiment manifest & identity (AE1) — a headless evolution lab from the M2 core.
//!
//! This module turns the M2 scenario runner into a reproducible *experiment*: a versioned
//! [`WorldLawSet`] (the immutable laws of a run, including an optional generic exotic-energy source),
//! a deterministic [`InitialConditionSet`] (the `t=0` state), and an [`ExperimentManifest`] bundling
//! artifact identity, laws, initial conditions, interventions, seeds, duration, sampling and the
//! declared [`ObservableRegistry`].
//!
//! The centrepiece is **canonical identity**: [`WorldLawSet::fingerprint`] and
//! [`ExperimentManifest::fingerprint`] hash a *canonical byte encoding* (fixed field order, sets
//! sorted, floats hashed by IEEE-754 bits) — never map iteration order or `Debug` formatting — so two
//! logically-identical manifests with reordered non-semantic input share a fingerprint (AE-S02),
//! while any material world-law change flips it (AE-S03). [`FactorDiff`] then enforces that a
//! control/treatment pair differs *only* in declared factors (AE-S08).
//!
//! Nothing here runs the simulation; that is [`crate::core::experiment_runner`]. Nothing here mixes
//! MU and EU; the exotic types live in [`crate::core::exotic_energy`] and are unit-checked (MU ≠ EU).

use crate::core::exotic_energy::{ExoticEnergyLaw, ExoticIntervention, UnitId, EU_UNIT};
use crate::core::intervention::{Curve, InterventionCommand, Region};
pub use crate::core::island::{GenesisPhase, IslandId, IslandManifest, RealTimeEcologyProfile};
pub use crate::core::observer::ObserverPolicy;
use crate::core::sim_clock::RateBand;
pub use crate::core::world_artifact::WorldIdentity;
use serde::{Deserialize, Serialize};

mod factor_diff;
mod interventions;
mod laws;
mod manifest;
mod observables;

pub use factor_diff::*;
pub use interventions::*;
pub use laws::*;
pub use manifest::*;
pub use observables::*;

// ---- Schema versions & resource limits -------------------------------------------------------

pub const WORLD_LAW_SCHEMA_VERSION: u16 = 1;
pub const INITIAL_CONDITION_SCHEMA_VERSION: u16 = 1;
pub const MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const OBSERVABLE_REGISTRY_VERSION: u16 = 2;

/// Import limits so an imported manifest cannot exhaust resources (design §Security/privacy).
pub const MAX_SEEDS: usize = 1024;
pub const MAX_OBSERVABLES: usize = 4096;
pub const MAX_INTERVENTIONS: usize = 4096;
pub const MAX_DURATION_TICKS: u64 = 100_000_000;

/// Declared RAM ceiling for one ensemble, in bytes (G2 gate #3).
///
/// The individual limits above bound each dimension on its own, but nothing bounded their
/// **product** — and `RunResult::series` holds every sample in memory while `run_ensemble` holds
/// every `RunResult`. A manifest at the documented maxima (1024 seeds x 100M ticks x 4096
/// observables) is therefore not merely slow: it asks for petabytes and the process dies with no
/// explanation that points at the manifest.
///
/// 2 GiB is the declared ceiling. It is a *policy* number, not a measurement of any particular
/// machine, which is why it is stated here rather than discovered at runtime: an experiment that
/// does not fit is refused up front with the estimate and the limit, so the operator can lower
/// `sample_period`, seeds or duration deliberately instead of finding out by OOM.
pub const MAX_ENSEMBLE_RESULT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Bytes charged per sampled observable. One `(String, f64)` pair: 24 bytes of `String` header, a
/// short name on the heap, and the `f64`. Deliberately a round over-estimate — the budget exists to
/// refuse the catastrophic case, and under-charging would defeat it.
pub const BYTES_PER_SAMPLED_OBSERVABLE: u64 = 64;

/// Conservative retained bytes per causal effect: the record itself, two `String` headers and
/// their typical heap payloads. The budget is a refusal boundary, so rounding upward is safer than
/// pretending the append-only ledger is free.
pub const BYTES_PER_CAUSAL_EFFECT: u64 = 256;

/// One ordinary forcing can root the reference world's abiotic-to-predator chain. The live adapter
/// records fewer effects, so this remains an upper bound for both built-in experiment models.
const MAX_EFFECTS_PER_INTERVENTION_FIRING: u64 = 7;

/// An enabled AE3 population can record uptake, performance, births and a frequency change on one
/// ecology firing. Not every firing emits all four; budgeting the maximum keeps the bound simple.
const MAX_AE3_EFFECTS_PER_ECOLOGY_FIRING: u64 = 4;

// ---- Structured errors (AE-101) --------------------------------------------------------------

/// Every way validation can fail, as structured data (never a bare string) so callers — and the
/// eventual World Lab UI — can react per-case instead of scraping a message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExperimentError {
    /// The manifest is individually legal but its dimensions multiply out past
    /// [`MAX_ENSEMBLE_RESULT_BYTES`]. Carries both numbers so the message can say by how much.
    EnsembleTooLarge {
        estimated_bytes: u64,
        limit_bytes: u64,
        seeds: usize,
        samples_per_run: u64,
        observables: usize,
    },
    /// A schema version the current build does not understand (never silently defaulted).
    UnsupportedSchemaVersion {
        component: String,
        found: u16,
        supported: u16,
    },
    /// A unit string that is empty, wrong, or illegally the closed-EU unit for exotic energy.
    InvalidUnit { context: String, reason: String },
    /// A numeric field outside its valid range.
    OutOfRange {
        field: String,
        value: f64,
        min: f64,
        max: f64,
    },
    /// A required name/id was empty.
    EmptyField { field: String },
    /// The seed list was empty.
    EmptySeeds,
    /// The same seed appears twice in the ensemble.
    DuplicateSeed { seed: u64 },
    /// The same id (observable / intervention) appears twice.
    DuplicateId { context: String, id: String },
    /// A referenced observable is not in the registry.
    UnknownObservable { id: String },
    /// A control/treatment difference at a path that was not declared in the [`FactorDiff`] allowlist.
    UndeclaredFactorDifference { path: String },
    /// A resource-limit ceiling was exceeded.
    ResourceLimit {
        field: String,
        limit: usize,
        found: usize,
    },
    /// The exotic-energy law itself is invalid (wraps its reason).
    InvalidLaw { reason: String },
    /// The exotic-energy field could not be constructed from the law + grid (e.g. the declared
    /// `initial_amount` exceeds field capacity), wrapping the reason.
    FieldConstruction { reason: String },
    /// A referenced registry is itself invalid (wraps the underlying error rendered as text).
    InvalidRegistry { reason: String },
    /// A runtime exotic-source forcing (AE-209) is malformed or cannot apply to this run.
    InvalidExoticIntervention { id: u32, reason: String },
    /// The declared observer policy (ADR-0004) is malformed — e.g. an `Inhabit` that roots its
    /// effects at the background cause, which would file a human's doing as baseline dynamics.
    InvalidObserverPolicy { reason: String },
    /// A run was requested for a seed that is not in the manifest's declared seed set.
    SeedNotInManifest { seed: u64 },
    /// A checkpoint-fork treatment intervention could never be applied in the post-fork window
    /// `(fork_tick, duration_ticks]` (e.g. it starts in the shared prefix or after the run ends), so
    /// declaring it as the fork's factor would be misleading.
    InapplicableIntervention { id: u32, reason: String },
    /// The shared prefix of a checkpoint fork failed before reaching a valid checkpoint at
    /// `fork_tick`, so no branch can be continued from it.
    CheckpointPrefixFailed { tick: u64, reason: String },
    /// A non-finite value where a finite one is required.
    NotFinite { field: String },
    /// Finite samples produced an aggregate statistic that cannot be represented as a finite `f64`.
    ///
    /// Reporting this explicitly prevents `serde_json` from silently encoding NaN or infinity as
    /// `null`, which would make a scientific artifact look complete while losing its value.
    AggregateNotFinite {
        observable: String,
        statistic: String,
    },
    /// The AE3 reference population is impossible, mis-declared, or absent while an AE3 observable
    /// was requested (which would otherwise report a fabricated zero).
    InvalidPopulation { reason: String },
    /// Feature ecological-selection (O-5): a manifest requested an observable whose producing
    /// structure does not exist in this run — e.g. death observables on a run that declares no
    /// death rule. Reporting a zero would look like a measurement; failing here does not.
    InvalidObservableRequest { id: String, reason: String },
    /// Shared-island declaration (`ADR-0014`) violates guest/observer/phase invariants.
    InvalidIslandConfig { reason: String },
    /// Experiments must never mix organism genesis phases (`QD-GENESIS` / `ADR-0014`).
    MixedGenesisPhase {
        control: Option<GenesisPhase>,
        treatment: Option<GenesisPhase>,
    },
}

impl std::fmt::Display for ExperimentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExperimentError::EnsembleTooLarge {
                estimated_bytes,
                limit_bytes,
                seeds,
                samples_per_run,
                observables,
            } => write!(
                f,
                "ensemble would hold about {estimated_bytes} bytes in memory, over the declared \n                 ceiling of {limit_bytes} ({seeds} seeds x {samples_per_run} samples x \n                 {observables} observables); lower sample_period, seeds or duration_ticks"
            ),
            ExperimentError::UnsupportedSchemaVersion {
                component,
                found,
                supported,
            } => write!(
                f,
                "unsupported {component} schema version {found} (this build supports {supported})"
            ),
            ExperimentError::InvalidUnit { context, reason } => {
                write!(f, "invalid unit in {context}: {reason}")
            }
            ExperimentError::OutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} = {value} is out of range [{min}, {max}]"),
            ExperimentError::EmptyField { field } => write!(f, "{field} must not be empty"),
            ExperimentError::EmptySeeds => write!(f, "the seed list must not be empty"),
            ExperimentError::DuplicateSeed { seed } => write!(f, "duplicate seed {seed}"),
            ExperimentError::DuplicateId { context, id } => {
                write!(f, "duplicate {context} id '{id}'")
            }
            ExperimentError::UnknownObservable { id } => {
                write!(f, "observable '{id}' is not in the registry")
            }
            ExperimentError::UndeclaredFactorDifference { path } => write!(
                f,
                "control and treatment differ at undeclared factor '{path}'"
            ),
            ExperimentError::ResourceLimit {
                field,
                limit,
                found,
            } => write!(f, "{field} = {found} exceeds the limit of {limit}"),
            ExperimentError::InvalidLaw { reason } => write!(f, "invalid world law: {reason}"),
            ExperimentError::FieldConstruction { reason } => {
                write!(f, "exotic field construction failed: {reason}")
            }
            ExperimentError::InvalidRegistry { reason } => {
                write!(f, "invalid observable registry: {reason}")
            }
            ExperimentError::InvalidExoticIntervention { id, reason } => {
                write!(f, "invalid exotic forcing {id}: {reason}")
            }
            ExperimentError::InvalidObserverPolicy { reason } => {
                write!(f, "invalid observer policy: {reason}")
            }
            ExperimentError::SeedNotInManifest { seed } => {
                write!(f, "seed {seed} is not in the manifest's seed set")
            }
            ExperimentError::InapplicableIntervention { id, reason } => {
                write!(
                    f,
                    "treatment intervention {id} cannot apply post-fork: {reason}"
                )
            }
            ExperimentError::CheckpointPrefixFailed { tick, reason } => {
                write!(f, "checkpoint prefix failed at tick {tick}: {reason}")
            }
            ExperimentError::NotFinite { field } => write!(f, "{field} must be finite"),
            ExperimentError::AggregateNotFinite {
                observable,
                statistic,
            } => write!(
                f,
                "aggregate {statistic} for observable '{observable}' is not representable as a finite f64"
            ),
            ExperimentError::InvalidPopulation { reason } => {
                write!(f, "invalid AE3 reference population: {reason}")
            }
            ExperimentError::InvalidObservableRequest { id, reason } => write!(
                f,
                "observable '{id}' cannot be produced by this run: {reason}"
            ),
            ExperimentError::InvalidIslandConfig { reason } => {
                write!(f, "invalid shared-island configuration: {reason}")
            }
            ExperimentError::MixedGenesisPhase { control, treatment } => {
                let fmt_phase = |p: Option<GenesisPhase>| match p {
                    Some(phase) => phase.code(),
                    None => "none",
                };
                write!(
                    f,
                    "experiments must never mix genesis phases (control: {}, treatment: {})",
                    fmt_phase(*control),
                    fmt_phase(*treatment)
                )
            }
        }
    }
}

impl std::error::Error for ExperimentError {}

// ---- Canonical encoder -----------------------------------------------------------------------

/// FNV-1a 64-bit over a byte slice — the same hash family as the World Artifact's 32-bit checksum,
/// widened to 64 bits for manifest fingerprints (design uses `u64`).
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// A canonical byte accumulator: every value is appended in a fixed, explicit order with a
/// domain-separating tag so structurally different inputs cannot collide (e.g. `None` vs an empty
/// list). Floats are written by their IEEE-754 bits so the encoding is exact and stable, never via
/// `Debug`/`Display`. Callers must always sort set-like collections before feeding them in, so that
/// non-semantic input order does not change the hash.
#[derive(Default)]
pub struct Canon {
    buf: Vec<u8>,
}

impl Canon {
    pub fn new() -> Self {
        Self::default()
    }
    /// A domain-separation tag (a discriminant / section marker).
    pub fn tag(&mut self, t: u8) -> &mut Self {
        self.buf.push(t);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.buf.push(v as u8);
        self
    }
    /// A length-prefixed string (length prefix prevents `"ab"+"c"` colliding with `"a"+"bc"`).
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.u64(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
        self
    }
    pub fn hash(&self) -> u64 {
        fnv1a_64(&self.buf)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ensemble_budget_tests;

#[cfg(test)]
mod observer_policy_manifest_tests;
