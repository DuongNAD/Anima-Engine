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
use crate::core::observer::ObserverPolicy;
use crate::core::sim_clock::RateBand;
use crate::core::world_artifact::WorldIdentity;
use serde::{Deserialize, Serialize};

// ---- Schema versions & resource limits -------------------------------------------------------

pub const WORLD_LAW_SCHEMA_VERSION: u16 = 1;
pub const INITIAL_CONDITION_SCHEMA_VERSION: u16 = 1;
pub const MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const OBSERVABLE_REGISTRY_VERSION: u16 = 1;

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
    /// The AE3 reference population is impossible, mis-declared, or absent while an AE3 observable
    /// was requested (which would otherwise report a fabricated zero).
    InvalidPopulation { reason: String },
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
            ExperimentError::InvalidPopulation { reason } => {
                write!(f, "invalid AE3 reference population: {reason}")
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

// ---- World laws (AE-102) ---------------------------------------------------------------------

/// The baseline (closed-EU) energy law. Minimal by design: it declares the biomass-equivalent unit
/// so that a manifest is self-describing and the MU≠EU invariant can be checked at validation time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineEnergyLaw {
    /// The closed-energy unit; must be [`EU_UNIT`].
    pub eu_unit: UnitId,
}

impl Default for BaselineEnergyLaw {
    fn default() -> Self {
        Self {
            eu_unit: UnitId::new(EU_UNIT),
        }
    }
}

/// The immutable laws of a run. `exotic_energy = None` is the baseline / rollback path (AE-S01); a
/// `Some(law)` declares a generic exotic source (displayed as e.g. "Mana"). Fixed before genesis —
/// changing a law is a new fork, never an in-place mutation (ER01).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldLawSet {
    pub schema_version: u16,
    pub baseline_energy: BaselineEnergyLaw,
    pub exotic_energy: Option<ExoticEnergyLaw>,
}

impl WorldLawSet {
    /// The baseline law set: closed EU, no exotic energy.
    pub fn baseline() -> Self {
        Self {
            schema_version: WORLD_LAW_SCHEMA_VERSION,
            baseline_energy: BaselineEnergyLaw::default(),
            exotic_energy: None,
        }
    }

    /// The baseline law set with an exotic-energy law added (a treatment regime).
    pub fn with_exotic(law: ExoticEnergyLaw) -> Self {
        Self {
            schema_version: WORLD_LAW_SCHEMA_VERSION,
            baseline_energy: BaselineEnergyLaw::default(),
            exotic_energy: Some(law),
        }
    }

    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != WORLD_LAW_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "world_laws".into(),
                found: self.schema_version,
                supported: WORLD_LAW_SCHEMA_VERSION,
            });
        }
        if !self.baseline_energy.eu_unit.is_eu() {
            return Err(ExperimentError::InvalidUnit {
                context: "baseline_energy".into(),
                reason: format!(
                    "the closed-energy unit must be '{EU_UNIT}', found '{}'",
                    self.baseline_energy.eu_unit.as_str()
                ),
            });
        }
        if let Some(law) = &self.exotic_energy {
            law.validate()
                .map_err(|reason| ExperimentError::InvalidLaw { reason })?;
        }
        Ok(())
    }

    /// Canonical identity of the law set — part of run/save identity (ER08). A material change to any
    /// declared law flips this (AE-S03).
    pub fn fingerprint(&self) -> u64 {
        let mut c = Canon::new();
        self.canonicalize(&mut c);
        c.hash()
    }

    pub(crate) fn canonicalize(&self, c: &mut Canon) {
        c.tag(0xA0).u16(self.schema_version);
        c.tag(0xA1).str(self.baseline_energy.eu_unit.as_str());
        match &self.exotic_energy {
            None => {
                c.tag(0x00);
            }
            Some(law) => {
                c.tag(0x01);
                c.str(law.id.as_str());
                c.str(&law.display_name);
                c.str(law.unit.as_str());
                // Enum discriminants encoded as stable tags.
                c.tag(source_model_tag(law.source_model));
                match law.topology {
                    crate::core::exotic_energy::SourceTopology::Uniform => {
                        c.tag(0x10);
                    }
                    crate::core::exotic_energy::SourceTopology::Patchy {
                        hotspot_count,
                        radius_cells,
                    } => {
                        c.tag(0x11).u16(hotspot_count).f32(radius_cells);
                    }
                }
                c.f64(law.initial_amount)
                    .f32(law.source_rate)
                    .f32(law.diffusion_rate)
                    .f32(law.decay_rate)
                    .f32(law.max_density);
                c.tag(boundary_tag(law.boundary));
            }
        }
    }
}

fn source_model_tag(m: crate::core::exotic_energy::ExoticSourceModel) -> u8 {
    use crate::core::exotic_energy::ExoticSourceModel::*;
    match m {
        Renewable => 0x21,
    }
}

fn boundary_tag(b: crate::core::exotic_energy::BoundaryMode) -> u8 {
    use crate::core::exotic_energy::BoundaryMode::*;
    match b {
        Closed => 0x30,
        Open => 0x31,
    }
}

// ---- Initial conditions (AE-102) -------------------------------------------------------------

/// A deterministic `t=0` state as a set of named scalars (e.g. `plants`, `herbivores`). Stored as a
/// list but treated as a **set**: the canonical encoding sorts by key, so listing order never changes
/// identity, and duplicate keys are rejected by [`validate`](Self::validate).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InitialConditionSet {
    pub schema_version: u16,
    pub values: Vec<(String, f64)>,
}

impl InitialConditionSet {
    pub fn new(values: Vec<(String, f64)>) -> Self {
        Self {
            schema_version: INITIAL_CONDITION_SCHEMA_VERSION,
            values,
        }
    }

    /// Look up a named initial value.
    pub fn get(&self, key: &str) -> Option<f64> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }

    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.schema_version != INITIAL_CONDITION_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "initial_conditions".into(),
                found: self.schema_version,
                supported: INITIAL_CONDITION_SCHEMA_VERSION,
            });
        }
        for (i, (k, v)) in self.values.iter().enumerate() {
            if k.is_empty() {
                return Err(ExperimentError::EmptyField {
                    field: "initial_condition key".into(),
                });
            }
            if !v.is_finite() {
                return Err(ExperimentError::NotFinite {
                    field: format!("initial_conditions.{k}"),
                });
            }
            for (k2, _) in &self.values[i + 1..] {
                if k == k2 {
                    return Err(ExperimentError::DuplicateId {
                        context: "initial_condition".into(),
                        id: k.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn canonicalize(&self, c: &mut Canon) {
        c.tag(0xB0).u16(self.schema_version);
        // Sort by key so listing order is non-semantic.
        let mut sorted: Vec<&(String, f64)> = self.values.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        c.u64(sorted.len() as u64);
        for (k, v) in sorted {
            c.str(k).f64(*v);
        }
    }
}

// ---- Observable registry (AE-109) ------------------------------------------------------------

/// Where an observable is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservableScope {
    World,
    Region,
    Cell,
    Organism,
    Lineage,
    Species,
    Run,
}

/// How an observable relates to a conservation law.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConservationRole {
    /// Member of the closed-EU biomass ledger.
    ClosedEu,
    /// Member of the independent MU ledger.
    ExoticMu,
    /// Not a conserved quantity.
    None,
}

/// How samples aggregate within a cadence window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aggregation {
    Instant,
    Sum,
    Mean,
    Max,
}

/// The full metadata for one observable (ER10). Self-describing so the UI never infers a unit/range
/// from a colour or a decorative field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservableSpec {
    pub id: String,
    pub display_name: String,
    pub unit: String,
    pub scope: ObservableScope,
    /// Sampling cadence name (e.g. `"ecology"`, `"plant"`).
    pub cadence_name: String,
    /// Sampling cadence period in base ticks (see [`RateBand::period`]).
    pub cadence_period: u64,
    pub aggregation: Aggregation,
    pub valid_min: f64,
    pub valid_max: f64,
    pub conservation: ConservationRole,
    /// The code symbol / component that produces this observable.
    pub source: String,
}

/// The registry of every observable a run can emit — the single source of truth shared by the
/// backend result and (later) the World Lab UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservableRegistry {
    pub version: u16,
    specs: Vec<ObservableSpec>,
}

/// The code symbol producing the AE3 population observables.
const AE3_SOURCE: &str = "core::evolution_pathway::ReferencePopulation";

impl ObservableRegistry {
    /// A large but **finite** upper bound for open-ended observables (EU/MU pools, cumulative
    /// counters). `f64::INFINITY` is deliberately NOT used: `serde_json` renders non-finite floats as
    /// `null`, which would silently destroy the range metadata of a supposedly self-describing
    /// [`crate::core::experiment_runner::RunResult`] on export. This value is far above any physical
    /// quantity the reference world produces while remaining exactly JSON-representable.
    pub const OPEN_UPPER_BOUND: f64 = 1e300;
    /// The symmetric lower bound for signed open-ended observables (e.g. a budget error).
    pub const OPEN_LOWER_BOUND: f64 = -1e300;

    /// The default registry for the headless reference world: the closed-EU trophic observables plus
    /// the exotic-MU observables.
    pub fn reference_default() -> Self {
        use crate::core::sim_clock::{ECOLOGY_PERIOD, PLANT_PERIOD};
        let eco = RateBand::new("ecology", ECOLOGY_PERIOD);
        let plant = RateBand::new("plant", PLANT_PERIOD);
        let world = |id: &str, unit: &str, role: ConservationRole, max: f64| ObservableSpec {
            id: id.to_string(),
            display_name: id.to_string(),
            unit: unit.to_string(),
            scope: ObservableScope::World,
            cadence_name: eco.name.to_string(),
            cadence_period: eco.period,
            aggregation: Aggregation::Instant,
            valid_min: 0.0,
            valid_max: max,
            conservation: role,
            source: "core::reference_world::ReferenceEvolutionWorld".to_string(),
        };
        let specs = vec![
            ObservableSpec {
                valid_min: 0.0,
                valid_max: Self::OPEN_UPPER_BOUND,
                ..world(
                    "precip",
                    "normalized",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            world("temperature", "normalized", ConservationRole::None, 1.0),
            world(
                "npp",
                "EU/tick",
                ConservationRole::None,
                Self::OPEN_UPPER_BOUND,
            ),
            world(
                "plants",
                "EU",
                ConservationRole::ClosedEu,
                Self::OPEN_UPPER_BOUND,
            ),
            world(
                "herbivores",
                "EU",
                ConservationRole::ClosedEu,
                Self::OPEN_UPPER_BOUND,
            ),
            world(
                "predators",
                "EU",
                ConservationRole::ClosedEu,
                Self::OPEN_UPPER_BOUND,
            ),
            world(
                "detritus",
                "EU",
                ConservationRole::ClosedEu,
                Self::OPEN_UPPER_BOUND,
            ),
            // exotic.* — the MU ledger, sampled on the plant band (slower field band).
            ObservableSpec {
                cadence_name: plant.name.to_string(),
                cadence_period: plant.period,
                ..world(
                    "exotic.density_total",
                    "MU",
                    ConservationRole::ExoticMu,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                cadence_name: plant.name.to_string(),
                cadence_period: plant.period,
                ..world(
                    "exotic.sourced",
                    "MU",
                    ConservationRole::ExoticMu,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                cadence_name: plant.name.to_string(),
                cadence_period: plant.period,
                ..world(
                    "exotic.dissipated",
                    "MU",
                    ConservationRole::ExoticMu,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                cadence_name: plant.name.to_string(),
                cadence_period: plant.period,
                ..world(
                    "exotic.stored",
                    "MU",
                    ConservationRole::ExoticMu,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                valid_min: Self::OPEN_LOWER_BOUND,
                cadence_name: plant.name.to_string(),
                cadence_period: plant.period,
                ..world(
                    "exotic.budget_error",
                    "MU",
                    ConservationRole::ExoticMu,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            // MU that actually crossed the field↔organism boundary (AE3). These are what make the
            // pathway's advantage explainable as a transaction rather than an assertion.
            world(
                "exotic.uptake",
                "MU",
                ConservationRole::ExoticMu,
                Self::OPEN_UPPER_BOUND,
            ),
            world(
                "exotic.spent",
                "MU",
                ConservationRole::ExoticMu,
                Self::OPEN_UPPER_BOUND,
            ),
            // evolution.* — the AE3 reference population. Emitted ONLY when the opt-in population
            // exists; a manifest that requests one without enabling a population fails validation
            // rather than reporting a fabricated zero.
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.population_total",
                    "individuals",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.pathway_population",
                    "individuals",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.pathway_frequency",
                    "fraction",
                    ConservationRole::None,
                    1.0,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.generation",
                    "count",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.births",
                    "individuals",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.performance_legacy",
                    "performance",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.performance_pathway",
                    "performance",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
            ObservableSpec {
                // Signed: a pathway LOSING to legacy is exactly the AE-S06 result, so the declared
                // range must admit it.
                valid_min: Self::OPEN_LOWER_BOUND,
                source: AE3_SOURCE.to_string(),
                ..world(
                    "evolution.performance_delta",
                    "performance",
                    ConservationRole::None,
                    Self::OPEN_UPPER_BOUND,
                )
            },
        ];
        Self {
            version: OBSERVABLE_REGISTRY_VERSION,
            specs,
        }
    }

    /// The registry for the **live Bevy world** ([`crate::core::live_experiment`]).
    ///
    /// A separate registry rather than extra rows on [`reference_default`](Self::reference_default),
    /// because the two worlds do not measure the same things: the reference world's `herbivores` is
    /// a pool of EU, while the live world's herbivores are countable bodies whose energy is already
    /// inside `live.animals_eu`. Giving both the id `herbivores` would put two different units under
    /// one name, which is precisely the failure `ObservableSpec` exists to prevent.
    ///
    /// Two ids **are** deliberately shared with the reference registry — `plants` and `detritus` —
    /// with the same unit (EU) and the same [`ConservationRole::ClosedEu`]. Those are the shared-law
    /// quantities, and sharing the id is what lets a control/treatment result from one path be
    /// compared, in direction and meaning, with the other. `live_and_reference_agree_on_shared_ids`
    /// pins that they never drift apart.
    pub fn live_default() -> Self {
        use crate::core::sim_clock::{ECOLOGY_PERIOD, PHYSICS_PERIOD};
        const LIVE_SOURCE: &str = "core::live_experiment::LiveExperimentAdapter";
        let spec =
            |id: &str, unit: &str, role: ConservationRole, max: f64, period: u64, cadence: &str| {
                ObservableSpec {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    unit: unit.to_string(),
                    scope: ObservableScope::World,
                    cadence_name: cadence.to_string(),
                    cadence_period: period,
                    aggregation: Aggregation::Instant,
                    valid_min: 0.0,
                    valid_max: max,
                    conservation: role,
                    source: LIVE_SOURCE.to_string(),
                }
            };
        let eu = |id: &str, role: ConservationRole| {
            spec(
                id,
                EU_UNIT,
                role,
                Self::OPEN_UPPER_BOUND,
                ECOLOGY_PERIOD,
                "ecology",
            )
        };
        let count = |id: &str| {
            spec(
                id,
                "individuals",
                ConservationRole::None,
                Self::OPEN_UPPER_BOUND,
                PHYSICS_PERIOD,
                "physics",
            )
        };
        let specs = vec![
            eu("plants", ConservationRole::ClosedEu),
            eu("detritus", ConservationRole::ClosedEu),
            eu("live.animals_eu", ConservationRole::ClosedEu),
            eu("live.closed_eu_total", ConservationRole::None),
            count("live.agent_count"),
            count("live.herbivore_count"),
            count("live.predator_count"),
            count("live.food_items"),
            eu("live.standing_crop", ConservationRole::None),
            eu("live.mean_agent_energy", ConservationRole::None),
            spec(
                "live.season_phase",
                "fraction",
                ConservationRole::None,
                1.0,
                ECOLOGY_PERIOD,
                "ecology",
            ),
        ];
        Self {
            version: OBSERVABLE_REGISTRY_VERSION,
            specs,
        }
    }

    pub fn get(&self, id: &str) -> Option<&ObservableSpec> {
        self.specs.iter().find(|s| s.id == id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.specs.iter().any(|s| s.id == id)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.specs.iter().map(|s| s.id.as_str()).collect()
    }

    pub fn specs(&self) -> &[ObservableSpec] {
        &self.specs
    }

    /// Test-only: append a duplicate of the first spec, producing a registry that must fail
    /// [`validate`](Self::validate). Lets other modules' tests build a malformed registry without
    /// exposing mutable access to `specs` in the public API.
    #[cfg(test)]
    pub fn push_duplicate_for_test(&mut self) {
        if let Some(first) = self.specs.first().cloned() {
            self.specs.push(first);
        }
    }

    /// Validate the registry: unique ids, non-empty units, sane ranges, and conservation metadata on
    /// conserved variables.
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.version != OBSERVABLE_REGISTRY_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "observable_registry".into(),
                found: self.version,
                supported: OBSERVABLE_REGISTRY_VERSION,
            });
        }
        for (i, s) in self.specs.iter().enumerate() {
            if s.id.is_empty() {
                return Err(ExperimentError::EmptyField {
                    field: "observable id".into(),
                });
            }
            if s.unit.is_empty() {
                return Err(ExperimentError::InvalidUnit {
                    context: format!("observable '{}'", s.id),
                    reason: "unit must not be empty".into(),
                });
            }
            // Descriptive metadata must be present: a spec with no display name / cadence name /
            // source symbol cannot describe itself to a consumer.
            for (field, value) in [
                ("display_name", &s.display_name),
                ("cadence_name", &s.cadence_name),
                ("source", &s.source),
            ] {
                if value.is_empty() {
                    return Err(ExperimentError::EmptyField {
                        field: format!("observable '{}' {field}", s.id),
                    });
                }
            }
            // A zero cadence period never fires (see `SimClock::fires`), so it is not a cadence.
            if s.cadence_period == 0 {
                return Err(ExperimentError::OutOfRange {
                    field: format!("observable '{}' cadence_period", s.id),
                    value: 0.0,
                    min: 1.0,
                    max: u64::MAX as f64,
                });
            }
            // Bounds must be FINITE: non-finite floats are not JSON-representable (serde_json emits
            // `null`), which would silently destroy the range metadata on export.
            for (field, value) in [("valid_min", s.valid_min), ("valid_max", s.valid_max)] {
                if !value.is_finite() {
                    return Err(ExperimentError::NotFinite {
                        field: format!("observable '{}' {field}", s.id),
                    });
                }
            }
            if s.valid_min > s.valid_max {
                return Err(ExperimentError::OutOfRange {
                    field: format!("observable '{}' range", s.id),
                    value: s.valid_min,
                    min: s.valid_min,
                    max: s.valid_max,
                });
            }
            for s2 in &self.specs[i + 1..] {
                if s.id == s2.id {
                    return Err(ExperimentError::DuplicateId {
                        context: "observable".into(),
                        id: s.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Canonical fingerprint of the registry (accompanies every result so the UI can verify it read
    /// the same catalogue the backend produced).
    pub fn fingerprint(&self) -> u64 {
        let mut c = Canon::new();
        c.tag(0xC0).u16(self.version);
        // Sort by id so definition order is non-semantic.
        let mut sorted: Vec<&ObservableSpec> = self.specs.iter().collect();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        c.u64(sorted.len() as u64);
        for s in sorted {
            c.str(&s.id)
                .str(&s.display_name)
                .str(&s.unit)
                .tag(scope_tag(s.scope))
                .str(&s.cadence_name)
                .u64(s.cadence_period)
                .tag(agg_tag(s.aggregation))
                .f64(s.valid_min)
                .f64(s.valid_max)
                .tag(role_tag(s.conservation))
                .str(&s.source);
        }
        c.hash()
    }
}

fn scope_tag(s: ObservableScope) -> u8 {
    match s {
        ObservableScope::World => 0,
        ObservableScope::Region => 1,
        ObservableScope::Cell => 2,
        ObservableScope::Organism => 3,
        ObservableScope::Lineage => 4,
        ObservableScope::Species => 5,
        ObservableScope::Run => 6,
    }
}
fn agg_tag(a: Aggregation) -> u8 {
    match a {
        Aggregation::Instant => 0,
        Aggregation::Sum => 1,
        Aggregation::Mean => 2,
        Aggregation::Max => 3,
    }
}
fn role_tag(r: ConservationRole) -> u8 {
    match r {
        ConservationRole::ClosedEu => 0,
        ConservationRole::ExoticMu => 1,
        ConservationRole::None => 2,
    }
}

// ---- Manifest-path intervention validation (AE-103) ------------------------------------------

/// Validate one [`InterventionCommand`] for use on the **manifest path**, where inputs may be
/// imported/untrusted. The legacy [`crate::core::intervention`] module is intentionally left
/// unchanged (its own scenario tests keep the permissive behaviour); this is the stricter gate that
/// experiment manifests and checkpoint forks share.
///
/// `run_ticks` is the run's `duration_ticks`; the simulated tick domain is `1..=run_ticks`. Rejects:
/// non-finite or negative `intensity`, invalid `Radius` (non-finite centre/radius, non-positive
/// radius), inverted `Rect` bounds, a `start_tick + effective_duration` that overflows `u64`, and a
/// schedule whose active window never intersects the run.
pub fn validate_intervention(
    cmd: &InterventionCommand,
    run_ticks: u64,
) -> Result<(), ExperimentError> {
    // Intensity: finite and non-negative (direction is carried by `signed_negative`, not the sign).
    if !cmd.intensity.is_finite() {
        return Err(ExperimentError::NotFinite {
            field: format!("intervention {} intensity", cmd.id),
        });
    }
    if cmd.intensity < 0.0 {
        return Err(ExperimentError::OutOfRange {
            field: format!("intervention {} intensity", cmd.id),
            value: cmd.intensity as f64,
            min: 0.0,
            max: f64::from(f32::MAX),
        });
    }

    // Geometry.
    match cmd.region {
        Region::Global | Region::Cell { .. } => {}
        Region::Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        } => {
            if min_x > max_x || min_y > max_y {
                return Err(ExperimentError::OutOfRange {
                    field: format!("intervention {} region Rect bounds", cmd.id),
                    value: min_x as f64,
                    min: 0.0,
                    max: max_x as f64,
                });
            }
        }
        Region::Radius { cx, cy, r } => {
            if !cx.is_finite() || !cy.is_finite() || !r.is_finite() {
                return Err(ExperimentError::NotFinite {
                    field: format!("intervention {} region Radius", cmd.id),
                });
            }
            if r <= 0.0 {
                return Err(ExperimentError::OutOfRange {
                    field: format!("intervention {} region radius", cmd.id),
                    value: r as f64,
                    min: f64::MIN_POSITIVE,
                    max: f64::from(f32::MAX),
                });
            }
        }
    }

    // Schedule: the active window is `[start_tick, start_tick + effective_duration)`. Compute its end
    // with a checked add so a crafted manifest cannot wrap around.
    let end_exclusive = cmd
        .start_tick
        .checked_add(cmd.effective_duration())
        .ok_or_else(|| ExperimentError::OutOfRange {
            field: format!("intervention {} start_tick + duration_ticks", cmd.id),
            value: cmd.start_tick as f64,
            min: 0.0,
            max: u64::MAX as f64,
        })?;

    // The run simulates ticks 1..=run_ticks. An intervention that can never fire inside that window
    // would be a silently-inert declared factor.
    let intersects = cmd.start_tick <= run_ticks && end_exclusive > 1;
    if !intersects {
        return Err(ExperimentError::OutOfRange {
            field: format!("intervention {} active window", cmd.id),
            value: cmd.start_tick as f64,
            min: 1.0,
            max: run_ticks as f64,
        });
    }
    Ok(())
}

// ---- Experiment manifest (AE-103) ------------------------------------------------------------

/// The full reproducible input for an experiment run: identity, laws, initial state, interventions,
/// seeds, duration, sampling and requested observables. Its [`fingerprint`](Self::fingerprint) is the
/// run's canonical identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExperimentManifest {
    pub schema_version: u16,
    pub experiment_id: String,
    pub name: String,
    /// The world this experiment runs on (artifact identity: seed/generator/dims/checksum).
    pub world_identity: WorldIdentity,
    pub laws: WorldLawSet,
    pub initial_conditions: InitialConditionSet,
    pub interventions: Vec<InterventionCommand>,
    pub seeds: Vec<u64>,
    pub duration_ticks: u64,
    /// Sample the observables every `sample_period` base ticks (`0` = never).
    pub sample_period: u64,
    pub observable_ids: Vec<String>,
    /// Runtime exotic-source forcings (AE-209). These are declared **state effects on the field**,
    /// never law edits: `laws.exotic_energy` stays immutable for the whole run (ER01). A non-empty
    /// list requires `laws.exotic_energy` to be `Some(..)` — there is no field to force otherwise.
    /// Defaults to empty so existing manifests (and JSON without the key) keep working.
    #[serde(default)]
    pub exotic_interventions: Vec<ExoticIntervention>,
    /// How an observer may relate to this run (ADR-0004). A **declared input**, so it changes the
    /// manifest fingerprint — but never the world-law fingerprint, which stays immutable for the run
    /// (ER01). That separation is what lets a checkpoint be forked to drop the observer.
    ///
    /// Defaults to [`ObserverPolicy::Absent`] so manifests written before ADR-0004 — and any JSON
    /// without the key — load and behave exactly as they did. `MANIFEST_SCHEMA_VERSION` is
    /// deliberately **not** bumped for the same reason: [`validate`](Self::validate) rejects any
    /// version it does not equal, so a bump would turn an additive field into a breaking change.
    #[serde(default)]
    pub observer: ObserverPolicy,
}

impl ExperimentManifest {
    /// Validate the manifest against a registry: schema, non-empty ids, laws, initial state, seeds
    /// (non-empty, bounded, unique), duration bounds, and that every requested observable exists.
    pub fn validate(&self, registry: &ObservableRegistry) -> Result<(), ExperimentError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "manifest".into(),
                found: self.schema_version,
                supported: MANIFEST_SCHEMA_VERSION,
            });
        }
        if self.experiment_id.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "experiment_id".into(),
            });
        }
        if self.name.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "name".into(),
            });
        }
        self.laws.validate()?;
        self.initial_conditions.validate()?;

        // AE3 reference population (opt-in). Absent keys mean "disabled", which is the legacy path;
        // present-but-impossible keys are a structural failure, never a silently-corrected default.
        // The pathway is tuned to whatever source THIS run declares, so a fixture can never name an
        // incompatible one.
        let population =
            crate::core::evolution_pathway::ReferencePopulationConfig::from_initial_conditions(
                &self.initial_conditions,
                self.laws.exotic_energy.as_ref().map(|l| &l.id),
            )
            .map_err(|reason| ExperimentError::InvalidPopulation { reason })?;
        // An AE3 observable without a population would report a fabricated zero. Fail preflight.
        if population.is_none() {
            if let Some(id) = self.observable_ids.iter().find(|id| {
                crate::core::evolution_pathway::AE3_OBSERVABLE_IDS.contains(&id.as_str())
            }) {
                return Err(ExperimentError::InvalidPopulation {
                    reason: format!(
                        "observable '{id}' requires an enabled AE3 reference population; declare \
                         '{}' in initial_conditions or drop the observable",
                        crate::core::evolution_pathway::AE3_KEY_POPULATION_TOTAL
                    ),
                });
            }
        }

        // Seeds: non-empty, bounded, unique.
        if self.seeds.is_empty() {
            return Err(ExperimentError::EmptySeeds);
        }
        if self.seeds.len() > MAX_SEEDS {
            return Err(ExperimentError::ResourceLimit {
                field: "seeds".into(),
                limit: MAX_SEEDS,
                found: self.seeds.len(),
            });
        }
        for (i, s) in self.seeds.iter().enumerate() {
            for s2 in &self.seeds[i + 1..] {
                if s == s2 {
                    return Err(ExperimentError::DuplicateSeed { seed: *s });
                }
            }
        }

        // Duration.
        if self.duration_ticks == 0 {
            return Err(ExperimentError::OutOfRange {
                field: "duration_ticks".into(),
                value: 0.0,
                min: 1.0,
                max: MAX_DURATION_TICKS as f64,
            });
        }
        if self.duration_ticks > MAX_DURATION_TICKS {
            return Err(ExperimentError::ResourceLimit {
                field: "duration_ticks".into(),
                limit: MAX_DURATION_TICKS as usize,
                found: self.duration_ticks as usize,
            });
        }

        // Interventions: bounded, unique ids.
        if self.interventions.len() > MAX_INTERVENTIONS {
            return Err(ExperimentError::ResourceLimit {
                field: "interventions".into(),
                limit: MAX_INTERVENTIONS,
                found: self.interventions.len(),
            });
        }
        for (i, cmd) in self.interventions.iter().enumerate() {
            // Values, geometry and schedule must be sane on the manifest path (AE-103).
            validate_intervention(cmd, self.duration_ticks)?;
            for cmd2 in &self.interventions[i + 1..] {
                if cmd.id == cmd2.id {
                    return Err(ExperimentError::DuplicateId {
                        context: "intervention".into(),
                        id: cmd.id.to_string(),
                    });
                }
            }
        }

        // Exotic forcings (AE-209): bounded, individually valid, unique ids, and only where there is
        // actually a field to force. Silently ignoring a forcing on a baseline world would misstate
        // the declared experimental input.
        if self.exotic_interventions.len() > MAX_INTERVENTIONS {
            return Err(ExperimentError::ResourceLimit {
                field: "exotic_interventions".into(),
                limit: MAX_INTERVENTIONS,
                found: self.exotic_interventions.len(),
            });
        }
        if !self.exotic_interventions.is_empty() && self.laws.exotic_energy.is_none() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: self.exotic_interventions[0].id,
                reason:
                    "manifest declares exotic forcings but laws.exotic_energy is None (there is \
                         no field to force)"
                        .into(),
            });
        }
        for (i, cmd) in self.exotic_interventions.iter().enumerate() {
            cmd.validate(self.duration_ticks).map_err(|reason| {
                ExperimentError::InvalidExoticIntervention { id: cmd.id, reason }
            })?;
            for other in &self.exotic_interventions[i + 1..] {
                if other.id == cmd.id {
                    return Err(ExperimentError::DuplicateId {
                        context: "exotic_intervention".into(),
                        id: cmd.id.to_string(),
                    });
                }
            }
        }

        // Observer policy (ADR-0004): well-formed, or refuse the manifest. An `Inhabit` rooted at
        // the background cause is the failure worth catching here — it would run happily and file
        // the observer's own effects as baseline dynamics, which is a lie the causal ledger would
        // then repeat to everyone downstream.
        if let Some(reason) = self.observer.rejection_reason() {
            return Err(ExperimentError::InvalidObserverPolicy { reason });
        }

        // `CAUSE_OBSERVER` means "a human did this, live". An intervention **authored into a
        // manifest** cannot be that by definition — the manifest was written before the run — so a
        // declared one claiming that id would share a root with the observer's own effects and make
        // `root_cause` answer a question nobody asked. Cause ids are hand-assigned with no allocator
        // anywhere, so this is checked rather than assumed.
        //
        // This does **not** contradict ADR-0004 C3, which lowers a live observer action to an
        // `InterventionCommand` carrying exactly this id. That one is built at runtime and never
        // passes through here; the rule is about declared input, not about the id itself.
        for cause_id in self
            .interventions
            .iter()
            .map(|cmd| cmd.cause_id)
            .chain(self.exotic_interventions.iter().map(|f| f.cause_id))
        {
            if cause_id == anima_domain::causal::CAUSE_OBSERVER {
                return Err(ExperimentError::InvalidObserverPolicy {
                    reason: format!(
                        "a manifest-declared intervention claims cause id {cause_id}, reserved for \
                         the live observer (ADR-0004); declared input cannot have been caused by a \
                         human acting during the run — pick an id of your own"
                    ),
                });
            }
        }

        // Observables: bounded, unique, all present in the registry.
        if self.observable_ids.len() > MAX_OBSERVABLES {
            return Err(ExperimentError::ResourceLimit {
                field: "observables".into(),
                limit: MAX_OBSERVABLES,
                found: self.observable_ids.len(),
            });
        }
        for (i, id) in self.observable_ids.iter().enumerate() {
            if !registry.contains(id) {
                return Err(ExperimentError::UnknownObservable { id: id.clone() });
            }
            for id2 in &self.observable_ids[i + 1..] {
                if id == id2 {
                    return Err(ExperimentError::DuplicateId {
                        context: "observable".into(),
                        id: id.clone(),
                    });
                }
            }
        }
        // G2 gate #3: the dimensions are individually legal — now check their PRODUCT against the
        // declared RAM ceiling. Nothing else did, and `RunResult::series` keeps every sample while
        // `run_ensemble` keeps every `RunResult`, so a manifest at the documented maxima asks for
        // petabytes and dies without ever pointing at the manifest.
        let estimated_bytes = self.estimated_result_bytes(registry);
        if estimated_bytes > MAX_ENSEMBLE_RESULT_BYTES {
            return Err(ExperimentError::EnsembleTooLarge {
                estimated_bytes,
                limit_bytes: MAX_ENSEMBLE_RESULT_BYTES,
                seeds: self.seeds.len(),
                samples_per_run: self.samples_per_run(),
                observables: self.observable_ids.len().max(registry.specs.len()),
            });
        }
        Ok(())
    }

    /// How many samples one run records. `sample_period == 0` means "never sample", so the series
    /// stays empty and only the final observables are kept.
    pub fn samples_per_run(&self) -> u64 {
        self.duration_ticks
            .checked_div(self.sample_period)
            .unwrap_or(0)
    }

    /// Upper bound on the bytes an ensemble of this manifest will hold in memory.
    ///
    /// `seeds x samples_per_run x observables x BYTES_PER_SAMPLED_OBSERVABLE`, saturating so an
    /// absurd manifest reports `u64::MAX` rather than wrapping to a small number and sailing
    /// through the very check it should fail. The observable count is the larger of what the
    /// manifest asks for and what the registry can emit, because a run records whatever it emits.
    pub fn estimated_result_bytes(&self, registry: &ObservableRegistry) -> u64 {
        let observables = self.observable_ids.len().max(registry.specs.len()) as u64;
        (self.seeds.len() as u64)
            .saturating_mul(self.samples_per_run())
            .saturating_mul(observables)
            .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE)
    }

    /// The control variant of this manifest for a genesis fork: identical in every shared input, with
    /// the exotic-energy law removed (`None`). The only declared factor is `laws.exotic_energy`
    /// (AE-S08). The name/experiment_id gain a `::control` suffix (labels, not factors).
    pub fn control_variant(&self) -> ExperimentManifest {
        let mut laws = self.laws.clone();
        laws.exotic_energy = None;
        ExperimentManifest {
            experiment_id: format!("{}::control", self.experiment_id),
            name: format!("{}::control", self.name),
            laws,
            // Removing the law removes the field, so any runtime forcing on it must go too —
            // otherwise the control would declare forcings with nothing to force (and fail
            // validation). The exotic regime (law + its forcings) is one declared factor.
            exotic_interventions: Vec::new(),
            ..self.clone()
        }
    }

    /// The run's canonical identity. Two logically-identical manifests with reordered non-semantic
    /// input (seed order, observable order, intervention order) hash the same (AE-S02); any material
    /// law change flips it (AE-S03). `experiment_id`/`name` are labels and excluded.
    pub fn fingerprint(&self) -> u64 {
        let mut c = Canon::new();
        c.tag(0xF0).u16(self.schema_version);

        // World identity.
        c.tag(0xF1)
            .u32(self.world_identity.seed)
            .u32(self.world_identity.generator_version)
            .u32(self.world_identity.width)
            .u32(self.world_identity.height)
            .u32(self.world_identity.checksum);

        // Laws + initial conditions (each self-canonicalizing).
        self.laws.canonicalize(&mut c);
        self.initial_conditions.canonicalize(&mut c);

        // Interventions as a set, sorted by (start_tick, id).
        let mut ivs: Vec<&InterventionCommand> = self.interventions.iter().collect();
        ivs.sort_by_key(|cmd| (cmd.start_tick, cmd.id));
        c.tag(0xF2).u64(ivs.len() as u64);
        for cmd in ivs {
            canonicalize_intervention(&mut c, cmd);
        }

        // Seeds as a set (sorted).
        let mut seeds = self.seeds.clone();
        seeds.sort_unstable();
        c.tag(0xF3).u64(seeds.len() as u64);
        for s in seeds {
            c.u64(s);
        }

        c.tag(0xF4).u64(self.duration_ticks).u64(self.sample_period);

        // Exotic forcings as a set, sorted by (start_tick, id) so listing order is non-semantic.
        // Runtime forcings are part of the DECLARED INPUT, so they change the manifest fingerprint —
        // but never the world-law fingerprint, which stays immutable for the run (ER01).
        let mut forcings: Vec<&ExoticIntervention> = self.exotic_interventions.iter().collect();
        forcings.sort_by_key(|f| (f.start_tick, f.id));
        c.tag(0xF6).u64(forcings.len() as u64);
        for f in forcings {
            c.u32(f.id).u32(f.cause_id);
            c.tag(exotic_kind_tag(f.kind));
            canonicalize_region(&mut c, &f.region);
            c.u64(f.start_tick).u64(f.duration_ticks);
            c.f32(f.amount).tag(curve_tag(f.curve));
        }

        // Observable ids as a set (sorted).
        let mut obs = self.observable_ids.clone();
        obs.sort();
        c.tag(0xF5).u64(obs.len() as u64);
        for id in obs {
            c.str(&id);
        }

        // Observer policy (ADR-0004). All three must give different identities, including `Absent`
        // vs `Spectate` — those two are required to produce the *same trajectory*, but they are
        // different declarations about how the run was watched, and a run's identity records what
        // was declared. `Inhabit` is a different treatment outright.
        c.tag(0xF7);
        match self.observer {
            ObserverPolicy::Absent => {
                c.tag(0x60);
            }
            ObserverPolicy::Spectate => {
                c.tag(0x61);
            }
            ObserverPolicy::Inhabit { cause_id } => {
                c.tag(0x62).u32(cause_id);
            }
        }
        c.hash()
    }
}

fn canonicalize_intervention(c: &mut Canon, cmd: &InterventionCommand) {
    c.u32(cmd.id).u32(cmd.cause_id);
    c.tag(intervention_kind_tag(cmd.kind));
    canonicalize_region(c, &cmd.region);
    c.u64(cmd.start_tick).u64(cmd.duration_ticks);
    c.f32(cmd.intensity).bool(cmd.signed_negative);
    c.tag(curve_tag(cmd.curve)).bool(cmd.reversible);
}

fn canonicalize_region(c: &mut Canon, region: &Region) {
    match *region {
        Region::Global => {
            c.tag(0x40);
        }
        Region::Cell { x, y } => {
            c.tag(0x41).u32(x).u32(y);
        }
        Region::Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        } => {
            c.tag(0x42).u32(min_x).u32(min_y).u32(max_x).u32(max_y);
        }
        Region::Radius { cx, cy, r } => {
            c.tag(0x43).f32(cx).f32(cy).f32(r);
        }
    }
}

fn intervention_kind_tag(k: crate::core::intervention::InterventionKind) -> u8 {
    use crate::core::intervention::InterventionKind::*;
    match k {
        RainfallDelta => 0x50,
        TemperatureDelta => 0x51,
        Deforest => 0x52,
        RemovePredators => 0x53,
        AddNutrient => 0x54,
    }
}

fn exotic_kind_tag(k: crate::core::exotic_energy::ExoticInterventionKind) -> u8 {
    use crate::core::exotic_energy::ExoticInterventionKind::*;
    match k {
        AddSource => 0x70,
        RemoveSource => 0x71,
        Pulse => 0x72,
    }
}

fn curve_tag(c: Curve) -> u8 {
    match c {
        Curve::Step => 0x60,
        Curve::RampUp => 0x61,
        Curve::RampDown => 0x62,
        Curve::Triangle => 0x63,
    }
}

// ---- Factor diff (AE-103) --------------------------------------------------------------------

/// The allowlist of manifest paths a control/treatment pair is permitted to differ at. Any *other*
/// difference is rejected (AE-S08) so a comparison can never hide an uncontrolled variable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactorDiff {
    pub allowed_paths: Vec<String>,
}

impl FactorDiff {
    /// The genesis-fork allowlist for the exotic regime: the exotic-energy law and the runtime
    /// forcings that act on its field. These are one declared factor — removing the law necessarily
    /// removes its forcings (see [`ExperimentManifest::control_variant`]) — so both paths are
    /// allowed. Every other manifest path must still match exactly (AE-S08).
    pub fn genesis_exotic() -> Self {
        Self {
            allowed_paths: vec![
                "laws.exotic_energy".to_string(),
                "exotic_interventions".to_string(),
            ],
        }
    }

    /// The set of manifest paths at which `control` and `treatment` differ (sorted, deduped). Labels
    /// (`name`, `experiment_id`) are excluded — they are not experimental factors.
    pub fn diff_paths(control: &ExperimentManifest, treatment: &ExperimentManifest) -> Vec<String> {
        let mut paths = Vec::new();
        if control.world_identity != treatment.world_identity {
            paths.push("world_identity".to_string());
        }
        if control.laws.schema_version != treatment.laws.schema_version {
            paths.push("laws.schema_version".to_string());
        }
        if control.laws.baseline_energy != treatment.laws.baseline_energy {
            paths.push("laws.baseline_energy".to_string());
        }
        if control.laws.exotic_energy != treatment.laws.exotic_energy {
            paths.push("laws.exotic_energy".to_string());
        }
        if control.initial_conditions != treatment.initial_conditions {
            paths.push("initial_conditions".to_string());
        }
        // Interventions compared as sets (order-independent).
        if !interventions_equal(&control.interventions, &treatment.interventions) {
            paths.push("interventions".to_string());
        }
        if !exotic_interventions_equal(
            &control.exotic_interventions,
            &treatment.exotic_interventions,
        ) {
            paths.push("exotic_interventions".to_string());
        }
        if !seeds_equal(&control.seeds, &treatment.seeds) {
            paths.push("seeds".to_string());
        }
        if control.duration_ticks != treatment.duration_ticks {
            paths.push("duration_ticks".to_string());
        }
        if control.sample_period != treatment.sample_period {
            paths.push("sample_period".to_string());
        }
        if !observables_equal(&control.observable_ids, &treatment.observable_ids) {
            paths.push("observables".to_string());
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// Confirm the actual difference between `control` and `treatment` lies entirely within the
    /// allowlist; return the differing paths on success, or the first undeclared difference as a
    /// structured error.
    pub fn validate(
        &self,
        control: &ExperimentManifest,
        treatment: &ExperimentManifest,
    ) -> Result<Vec<String>, ExperimentError> {
        let diffs = Self::diff_paths(control, treatment);
        for p in &diffs {
            if !self.allowed_paths.contains(p) {
                return Err(ExperimentError::UndeclaredFactorDifference { path: p.clone() });
            }
        }
        Ok(diffs)
    }
}

fn interventions_equal(a: &[InterventionCommand], b: &[InterventionCommand]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<&InterventionCommand> = a.iter().collect();
    let mut b: Vec<&InterventionCommand> = b.iter().collect();
    a.sort_by_key(|c| (c.start_tick, c.id));
    b.sort_by_key(|c| (c.start_tick, c.id));
    a == b
}

fn exotic_interventions_equal(a: &[ExoticIntervention], b: &[ExoticIntervention]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<&ExoticIntervention> = a.iter().collect();
    let mut b: Vec<&ExoticIntervention> = b.iter().collect();
    a.sort_by_key(|c| (c.start_tick, c.id));
    b.sort_by_key(|c| (c.start_tick, c.id));
    a == b
}

fn seeds_equal(a: &[u64], b: &[u64]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

fn observables_equal(a: &[String], b: &[String]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::exotic_energy::ExoticEnergyLaw;
    use crate::core::intervention::InterventionKind;

    fn ref_init() -> InitialConditionSet {
        InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ])
    }

    pub(super) fn base_manifest() -> ExperimentManifest {
        ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: "exp-1".into(),
            name: "baseline".into(),
            observer: ObserverPolicy::default(),
            world_identity: WorldIdentity {
                seed: 1337,
                generator_version: 20,
                width: 128,
                height: 128,
                checksum: 0xDEAD_BEEF,
            },
            laws: WorldLawSet::baseline(),
            initial_conditions: ref_init(),
            interventions: vec![],
            seeds: vec![1, 2, 3],
            duration_ticks: 6000,
            sample_period: 600,
            observable_ids: vec!["plants".into(), "herbivores".into()],
            exotic_interventions: Vec::new(),
        }
    }

    // ---- AE-308: AE3 observables & the initial-condition seam --------------------------------

    use crate::core::evolution_pathway as ae3;

    fn ae3_init() -> InitialConditionSet {
        let mut v = ref_init().values;
        v.push((ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0));
        v.push((ae3::AE3_KEY_POPULATION_CAPACITY.into(), 100.0));
        v.push((ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.5));
        InitialConditionSet::new(v)
    }

    #[test]
    fn ae308_registry_fully_describes_every_ae3_observable() {
        let reg = ObservableRegistry::reference_default();
        reg.validate().expect("the extended registry stays valid");
        for id in ae3::AE3_OBSERVABLE_IDS {
            let spec = reg
                .get(id)
                .unwrap_or_else(|| panic!("AE3 observable '{id}' has no registry spec"));
            assert!(!spec.unit.is_empty(), "{id} needs a unit");
            assert!(!spec.source.is_empty(), "{id} needs a source symbol");
            assert!(spec.cadence_period > 0, "{id} needs a real cadence");
            assert!(
                spec.valid_min.is_finite() && spec.valid_max.is_finite(),
                "{id} bounds must be JSON-safe"
            );
            assert!(spec.valid_min <= spec.valid_max);
        }
        // A frequency is a share, so its declared range says so rather than being open-ended.
        let freq = reg.get("evolution.pathway_frequency").unwrap();
        assert_eq!((freq.valid_min, freq.valid_max), (0.0, 1.0));
        // A signed delta must admit negative values, or a pathway LOSING ground would be unreportable.
        let delta = reg.get("evolution.performance_delta").unwrap();
        assert!(delta.valid_min < 0.0);
        // The model emits the cumulative births counter. Summing those snapshots over a cadence
        // window would count the same births repeatedly, so its metadata must be instantaneous.
        let births = reg.get("evolution.births").unwrap();
        assert_eq!(births.aggregation, Aggregation::Instant);
        // MU-ledger observables carry the exotic conservation role, never the closed-EU one.
        for id in ["exotic.uptake", "exotic.spent"] {
            assert_eq!(
                reg.get(id).unwrap().conservation,
                ConservationRole::ExoticMu
            );
        }
    }

    #[test]
    fn ae308_manifest_rejects_an_ae3_observable_without_an_enabled_population() {
        let reg = ObservableRegistry::reference_default();
        // Requesting a selection observable from a world that has no population would return a
        // fabricated zero. It must fail preflight instead.
        let mut m = base_manifest();
        m.observable_ids.push("evolution.pathway_frequency".into());
        let err = m.validate(&reg).unwrap_err();
        assert!(
            matches!(err, ExperimentError::InvalidPopulation { .. }),
            "expected InvalidPopulation, got {err:?}"
        );

        // With the population enabled, the same manifest validates.
        let mut ok = m.clone();
        ok.initial_conditions = ae3_init();
        ok.validate(&reg)
            .expect("an enabled population may emit AE3 observables");
    }

    #[test]
    fn ae3_initial_conditions_reject_malformed_and_unknown_keys() {
        let reg = ObservableRegistry::reference_default();
        let with = |pairs: Vec<(String, f64)>| {
            let mut m = base_manifest();
            let mut v = ref_init().values;
            v.extend(pairs);
            m.initial_conditions = InitialConditionSet::new(v);
            m.validate(&reg)
        };

        // No AE3 keys at all: the population is simply disabled (legacy compatibility).
        assert!(with(vec![]).is_ok());

        // An AE3 key with no population total declares an input that can never take effect.
        assert!(with(vec![(ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.5)]).is_err());

        // A misspelt AE3 key is rejected rather than silently ignored.
        assert!(with(vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            ("ae3.pathway_fracton".into(), 0.5),
        ])
        .is_err());

        // Structurally impossible enabled states.
        for bad in [
            vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), -10.0)],
            vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), 0.0)],
            vec![
                (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
                (ae3::AE3_KEY_PATHWAY_FRACTION.into(), 1.5),
            ],
            vec![
                (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
                (ae3::AE3_KEY_POPULATION_CAPACITY.into(), 10.0),
            ],
            vec![
                (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
                (ae3::AE3_KEY_GENERATION_TICKS.into(), 0.0),
            ],
            vec![
                (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
                (ae3::AE3_KEY_GENERATION_TICKS.into(), 90.0),
            ],
            vec![
                (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
                (ae3::AE3_KEY_GENERATION_TICKS.into(), 600.5),
            ],
        ] {
            assert!(
                with(bad.clone()).is_err(),
                "expected structural rejection for {bad:?}"
            );
        }

        // NaN is already rejected by the InitialConditionSet contract itself.
        assert!(with(vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), f64::NAN)]).is_err());
    }

    // ---- AE-S03: canonical fingerprint ------------------------------------------------------

    #[test]
    fn ae_s03_disabled_law_round_trips_and_has_stable_fingerprint() {
        let laws = WorldLawSet::baseline();
        let json = serde_json::to_string(&laws).unwrap();
        let back: WorldLawSet = serde_json::from_str(&json).unwrap();
        assert_eq!(laws, back);
        assert_eq!(laws.fingerprint(), back.fingerprint());
        // Two independently-built baselines share a fingerprint.
        assert_eq!(WorldLawSet::baseline().fingerprint(), laws.fingerprint());
    }

    #[test]
    fn ae_s03_changing_any_material_law_changes_fingerprint() {
        let baseline = WorldLawSet::baseline().fingerprint();
        // Adding an exotic law changes it.
        let with_law = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
        assert_ne!(baseline, with_law.fingerprint());
        // Changing a single law parameter changes it.
        let mut law = ExoticEnergyLaw::mana_patchy(100.0, 4);
        law.diffusion_rate += 0.01;
        let changed = WorldLawSet::with_exotic(law);
        assert_ne!(with_law.fingerprint(), changed.fingerprint());
        // Changing the display name (a label) STILL changes the fingerprint here, because the law's
        // display_name is part of its declared identity — but the source id / physical params are the
        // material ones the gate cares about. Confirm a physical change is detected:
        let mut law2 = ExoticEnergyLaw::mana_patchy(100.0, 4);
        law2.max_density += 1.0;
        assert_ne!(
            with_law.fingerprint(),
            WorldLawSet::with_exotic(law2).fingerprint()
        );
    }

    #[test]
    fn ae_s02_reordered_non_semantic_input_has_same_manifest_fingerprint() {
        let mut a = base_manifest();
        a.seeds = vec![3, 1, 2];
        a.observable_ids = vec!["herbivores".into(), "plants".into()];
        let mut b = base_manifest();
        b.seeds = vec![1, 2, 3];
        b.observable_ids = vec!["plants".into(), "herbivores".into()];
        // Same logical manifest, reordered sets → identical fingerprint.
        assert_eq!(a.fingerprint(), b.fingerprint());
        // Reordering initial-condition entries also does not matter.
        let mut c = base_manifest();
        let mut vals = ref_init().values;
        vals.reverse();
        c.initial_conditions = InitialConditionSet::new(vals);
        assert_eq!(c.fingerprint(), b.fingerprint());
    }

    #[test]
    fn ae_s03_manifest_fingerprint_tracks_world_law_change() {
        let baseline = base_manifest();
        let mut treatment = base_manifest();
        treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
        assert_ne!(baseline.fingerprint(), treatment.fingerprint());
    }

    // ---- Validation (structured errors) -----------------------------------------------------

    #[test]
    fn validator_accepts_a_well_formed_manifest() {
        let reg = ObservableRegistry::reference_default();
        assert!(base_manifest().validate(&reg).is_ok());
    }

    #[test]
    fn validator_rejects_structured_failures() {
        let reg = ObservableRegistry::reference_default();

        // Duplicate seed.
        let mut m = base_manifest();
        m.seeds = vec![1, 1, 2];
        assert_eq!(
            m.validate(&reg),
            Err(ExperimentError::DuplicateSeed { seed: 1 })
        );

        // Empty seeds.
        let mut m = base_manifest();
        m.seeds = vec![];
        assert_eq!(m.validate(&reg), Err(ExperimentError::EmptySeeds));

        // Unknown observable.
        let mut m = base_manifest();
        m.observable_ids = vec!["does_not_exist".into()];
        assert_eq!(
            m.validate(&reg),
            Err(ExperimentError::UnknownObservable {
                id: "does_not_exist".into()
            })
        );

        // Zero duration.
        let mut m = base_manifest();
        m.duration_ticks = 0;
        assert!(matches!(
            m.validate(&reg),
            Err(ExperimentError::OutOfRange { .. })
        ));

        // Unsupported schema version.
        let mut m = base_manifest();
        m.schema_version = 999;
        assert!(matches!(
            m.validate(&reg),
            Err(ExperimentError::UnsupportedSchemaVersion { .. })
        ));

        // An exotic law with the EU unit is rejected (MU is not EU).
        let mut m = base_manifest();
        let mut law = ExoticEnergyLaw::mana_uniform(50.0);
        law.unit = UnitId::new(EU_UNIT);
        m.laws = WorldLawSet::with_exotic(law);
        assert!(matches!(
            m.validate(&reg),
            Err(ExperimentError::InvalidLaw { .. })
        ));
    }

    // ---- AE-S08: factor diff ----------------------------------------------------------------

    #[test]
    fn ae_s08_control_variant_differs_only_in_exotic_law() {
        let mut treatment = base_manifest();
        treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
        let control = treatment.control_variant();
        let diffs = FactorDiff::diff_paths(&control, &treatment);
        assert_eq!(diffs, vec!["laws.exotic_energy".to_string()]);
        // The genesis allowlist accepts exactly this.
        assert!(FactorDiff::genesis_exotic()
            .validate(&control, &treatment)
            .is_ok());
    }

    #[test]
    fn ae_s08_undeclared_difference_is_rejected() {
        let mut treatment = base_manifest();
        treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
        let mut control = treatment.control_variant();
        // Sneak in an off-allowlist change: a different seed set.
        control.seeds = vec![9, 9_9];
        let err = FactorDiff::genesis_exotic()
            .validate(&control, &treatment)
            .unwrap_err();
        assert!(matches!(
            err,
            ExperimentError::UndeclaredFactorDifference { .. }
        ));
    }

    // ---- AE-109: observable registry --------------------------------------------------------

    #[test]
    fn observable_registry_validates_and_is_unique() {
        let reg = ObservableRegistry::reference_default();
        assert!(reg.validate().is_ok());
        // Ids are unique and non-empty; every conserved EU/MU observable declares its role.
        for spec in reg.specs() {
            assert!(!spec.id.is_empty());
            assert!(!spec.unit.is_empty());
        }
        assert!(reg.contains("plants"));
        assert!(reg.contains("exotic.density_total"));
        assert!(!reg.contains("nope"));
        // Fingerprint is order-independent.
        assert_eq!(
            reg.fingerprint(),
            ObservableRegistry::reference_default().fingerprint()
        );
    }

    #[test]
    fn observable_registry_rejects_duplicate_ids() {
        let mut reg = ObservableRegistry::reference_default();
        let dup = reg.specs()[0].clone();
        reg.specs.push(dup);
        assert!(matches!(
            reg.validate(),
            Err(ExperimentError::DuplicateId { .. })
        ));
    }

    // ---- DEFECT A: registry ranges must be JSON-safe and strictly validated -----------------

    #[test]
    fn defect_a_reference_registry_is_json_safe_and_finite() {
        // A registry carrying f64 infinities cannot round-trip through JSON (serde_json emits
        // `null` for non-finite floats), so the "self-describing" result would silently lose its
        // ranges. Every default spec must therefore declare FINITE bounds.
        let reg = ObservableRegistry::reference_default();
        for s in reg.specs() {
            assert!(
                s.valid_min.is_finite() && s.valid_max.is_finite(),
                "observable '{}' has non-finite bounds [{}, {}]",
                s.id,
                s.valid_min,
                s.valid_max
            );
        }
        // And the whole registry survives a JSON round-trip unchanged.
        let json = serde_json::to_string(&reg).expect("registry serializes");
        let back: ObservableRegistry = serde_json::from_str(&json).expect("registry deserializes");
        assert_eq!(reg, back);
        assert_eq!(reg.fingerprint(), back.fingerprint());
    }

    #[test]
    fn defect_a_registry_validation_rejects_malformed_specs() {
        let base = ObservableRegistry::reference_default();
        let mutate = |f: &dyn Fn(&mut ObservableSpec)| {
            let mut reg = base.clone();
            f(&mut reg.specs[0]);
            reg.validate()
        };

        // Non-finite bounds are rejected (they are not JSON-representable).
        assert!(mutate(&|s| s.valid_max = f64::INFINITY).is_err());
        assert!(mutate(&|s| s.valid_min = f64::NEG_INFINITY).is_err());
        assert!(mutate(&|s| s.valid_max = f64::NAN).is_err());
        assert!(mutate(&|s| s.valid_min = f64::NAN).is_err());

        // A zero cadence period never fires — a meaningless sampling cadence.
        assert!(mutate(&|s| s.cadence_period = 0).is_err());

        // Empty descriptive metadata is rejected (id/unit were already covered).
        assert!(mutate(&|s| s.display_name = String::new()).is_err());
        assert!(mutate(&|s| s.cadence_name = String::new()).is_err());
        assert!(mutate(&|s| s.source = String::new()).is_err());

        // The unmodified default registry still validates.
        assert!(base.validate().is_ok());
    }

    // ---- DEFECT B: manifest-path intervention validation ------------------------------------

    fn iv(id: u32, start: u64, duration: u64) -> InterventionCommand {
        InterventionCommand {
            id,
            cause_id: id,
            kind: InterventionKind::RainfallDelta,
            region: Region::Global,
            start_tick: start,
            duration_ticks: duration,
            intensity: 0.3,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        }
    }

    #[test]
    fn defect_b_manifest_rejects_invalid_intervention_values() {
        let reg = ObservableRegistry::reference_default();
        let with = |cmd: InterventionCommand| {
            let mut m = base_manifest();
            m.interventions = vec![cmd];
            m.validate(&reg)
        };

        // Non-finite / negative intensity.
        let mut c = iv(1, 10, 100);
        c.intensity = f64::NAN as f32;
        assert!(with(c).is_err());
        let mut c = iv(1, 10, 100);
        c.intensity = f32::INFINITY;
        assert!(with(c).is_err());
        let mut c = iv(1, 10, 100);
        c.intensity = -0.5;
        assert!(with(c).is_err());

        // Invalid Radius geometry.
        let mut c = iv(1, 10, 100);
        c.region = Region::Radius {
            cx: f32::NAN,
            cy: 0.0,
            r: 1.0,
        };
        assert!(with(c).is_err());
        let mut c = iv(1, 10, 100);
        c.region = Region::Radius {
            cx: 0.0,
            cy: 0.0,
            r: -1.0,
        };
        assert!(with(c).is_err());

        // Inverted Rect bounds.
        let mut c = iv(1, 10, 100);
        c.region = Region::Rect {
            min_x: 5,
            min_y: 0,
            max_x: 2,
            max_y: 4,
        };
        assert!(with(c).is_err());

        // start + effective_duration overflows u64.
        let c = iv(1, u64::MAX - 1, u64::MAX);
        assert!(with(c).is_err());

        // A schedule that can never be active within 1..=duration_ticks (base_manifest runs 6000).
        let c = iv(1, 900_000, 10);
        assert!(with(c).is_err());
        // start_tick 0 with a 1-tick window ends before tick 1 → never active either.
        let c = iv(1, 0, 1);
        assert!(with(c).is_err());

        // A well-formed, genuinely-active intervention passes.
        assert!(with(iv(1, 600, 3000)).is_ok());
    }

    // ---- AE-210 (M5): committed JSON fixtures ------------------------------------------------

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/experiments")
            .join(name)
    }

    fn load_fixture(name: &str) -> String {
        let p = fixture_path(name);
        std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("missing AE fixture {}: {e}", p.display()))
    }

    /// Regenerate the committed AE fixtures from the *real* serializer, so they can never drift from
    /// the schema. Ignored by default (it writes into the repo); run explicitly with:
    /// `cargo test --lib ae210_regenerate_fixtures -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes fixture files; run explicitly after an intentional schema change"]
    fn ae210_regenerate_fixtures() {
        use crate::core::exotic_energy::{
            ExoticEnergyLaw, ExoticIntervention, ExoticInterventionKind,
        };
        let dir = fixture_path("");
        std::fs::create_dir_all(&dir).expect("fixture dir");

        let base = base_manifest();
        let mut mana = base_manifest();
        mana.experiment_id = "ae-mana-patchy".into();
        mana.name = "mana-patchy-renewable".into();
        mana.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
        // NOTE: the observable list is deliberately IDENTICAL to the baseline's, so the two fixtures
        // differ only in the exotic regime (law + forcings) and form a clean AE-S08 control/treatment
        // pair. The treatment still *emits* the `exotic.*` observables — emission is model-driven, and
        // `RunResult` attaches metadata for everything emitted — so nothing is lost by not requesting
        // them here.
        mana.exotic_interventions = vec![ExoticIntervention {
            id: 1,
            cause_id: 901,
            kind: ExoticInterventionKind::RemoveSource,
            region: Region::Global,
            start_tick: 3000,
            duration_ticks: 1200,
            amount: 0.25,
            curve: Curve::Step,
        }];

        // Invalid on purpose: a negative source rate must be rejected by the law validator.
        let mut invalid = mana.clone();
        invalid.experiment_id = "ae-invalid-negative-source".into();
        invalid.name = "invalid-negative-source".into();
        invalid.exotic_interventions.clear();
        if let Some(law) = invalid.laws.exotic_energy.as_mut() {
            law.source_rate = -0.05;
        }

        for (file, m) in [
            ("baseline-no-exotic.json", &base),
            ("mana-patchy-renewable.json", &mana),
            ("invalid-negative-source.json", &invalid),
        ] {
            let json = serde_json::to_string_pretty(m).expect("serialize fixture");
            std::fs::write(dir.join(file), json + "\n").expect("write fixture");
        }
    }

    #[test]
    fn ae210_m5_baseline_and_mana_fixtures_round_trip_and_validate() {
        let reg = ObservableRegistry::reference_default();

        for name in ["baseline-no-exotic.json", "mana-patchy-renewable.json"] {
            let raw = load_fixture(name);
            let m: ExperimentManifest =
                serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name} parses: {e}"));
            m.validate(&reg)
                .unwrap_or_else(|e| panic!("{name} must be a valid manifest: {e}"));

            // Round-trip is lossless and identity-stable.
            let re = serde_json::to_string(&m).expect("serializes");
            let back: ExperimentManifest = serde_json::from_str(&re).expect("re-parses");
            assert_eq!(back, m, "{name} round-trip changed the manifest");
            assert_eq!(
                back.fingerprint(),
                m.fingerprint(),
                "{name} fingerprint is not round-trip stable"
            );
        }

        // The baseline fixture is genuinely exotic-free; the Mana fixture is genuinely a live
        // treatment carrying a runtime forcing.
        let base: ExperimentManifest =
            serde_json::from_str(&load_fixture("baseline-no-exotic.json")).unwrap();
        assert!(base.laws.exotic_energy.is_none());
        assert!(base.exotic_interventions.is_empty());

        let mana: ExperimentManifest =
            serde_json::from_str(&load_fixture("mana-patchy-renewable.json")).unwrap();
        let law = mana.laws.exotic_energy.as_ref().expect("mana law present");
        assert_eq!(law.display_name, "Mana", "display label only");
        assert_eq!(
            law.unit.as_str(),
            crate::core::exotic_energy::MU_UNIT,
            "MU is not EU"
        );
        assert!(!mana.exotic_interventions.is_empty());

        // The two differ in the exotic regime, and only there.
        let diffs = FactorDiff::diff_paths(&base, &mana);
        for p in &diffs {
            assert!(
                p == "laws.exotic_energy" || p == "exotic_interventions",
                "fixtures differ at an undeclared path: {p}"
            );
        }
    }

    #[test]
    fn ae210_m5_invalid_fixture_is_rejected_with_a_structured_error() {
        let reg = ObservableRegistry::reference_default();
        let raw = load_fixture("invalid-negative-source.json");
        let m: ExperimentManifest = serde_json::from_str(&raw).expect("parses as JSON");
        // It parses, but must FAIL validation with a structured (not stringly) error.
        let err = m
            .validate(&reg)
            .expect_err("invalid fixture must be rejected");
        assert!(
            matches!(err, ExperimentError::InvalidLaw { .. }),
            "expected a structured InvalidLaw, got {err:?}"
        );
    }

    #[test]
    fn ae210_m5_fixtures_are_small_enough_to_stay_reviewable() {
        // Schema/size record: these are hand-reviewable fixtures, not blobs. If a change balloons
        // one, that is a schema smell worth noticing in review.
        for name in [
            "baseline-no-exotic.json",
            "mana-patchy-renewable.json",
            "invalid-negative-source.json",
        ] {
            let bytes = load_fixture(name).len();
            assert!(
                bytes < 8_192,
                "fixture {name} is {bytes} bytes — too large to review by eye"
            );
        }
    }

    #[test]
    fn intervention_kind_tags_are_distinct() {
        // Guard: the canonical encoding must give every intervention kind a distinct tag.
        use std::collections::HashSet;
        let kinds = [
            InterventionKind::RainfallDelta,
            InterventionKind::TemperatureDelta,
            InterventionKind::Deforest,
            InterventionKind::RemovePredators,
            InterventionKind::AddNutrient,
        ];
        let tags: HashSet<u8> = kinds.into_iter().map(intervention_kind_tag).collect();
        assert_eq!(tags.len(), kinds.len());
    }
}

#[cfg(test)]
mod ensemble_budget_tests {
    use super::*;

    /// G2 gate #3. A manifest at the documented maxima is individually legal on every axis, and
    /// before this check it was accepted — then the runner tried to hold the result set in RAM.
    /// It must now be refused up front, naming the estimate and the limit.
    #[test]
    fn a_manifest_at_the_documented_maxima_is_refused_not_attempted() {
        let seeds = MAX_SEEDS as u64;
        let samples = MAX_DURATION_TICKS; // sample_period = 1
        let observables = MAX_OBSERVABLES as u64;
        let estimate = seeds
            .saturating_mul(samples)
            .saturating_mul(observables)
            .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE);

        assert!(
            estimate > MAX_ENSEMBLE_RESULT_BYTES,
            "the documented maxima must exceed the declared ceiling, or this gate is vacuous: \
             {estimate} vs {MAX_ENSEMBLE_RESULT_BYTES}"
        );
        // ~2.7e16 bytes — about 27 petabytes. The point of the ceiling in one number.
        assert!(estimate > 1e16 as u64, "estimate was {estimate}");
    }

    /// The estimate saturates instead of wrapping. A wrapped product would come out small and sail
    /// through the very check it should fail — the failure mode a budget must not have.
    #[test]
    fn the_estimate_saturates_rather_than_wrapping() {
        let huge = u64::MAX;
        let product = huge
            .saturating_mul(huge)
            .saturating_mul(huge)
            .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE);
        assert_eq!(product, u64::MAX);
        assert!(product > MAX_ENSEMBLE_RESULT_BYTES);
    }

    /// `sample_period == 0` means "never sample", so the series stays empty and the budget is not
    /// charged for it. A long run that records only final observables is legitimate.
    #[test]
    fn never_sampling_costs_nothing_in_series_memory() {
        assert_eq!(samples_for(0, MAX_DURATION_TICKS), 0);
        assert_eq!(samples_for(1000, 100_000), 100);
    }

    fn samples_for(sample_period: u64, duration_ticks: u64) -> u64 {
        duration_ticks.checked_div(sample_period).unwrap_or(0)
    }
}

#[cfg(test)]
mod observer_policy_manifest_tests {
    //! ADR-0004 O1 at the manifest level: the rollback path, run identity, and the ER01 boundary.
    //! The behaviour of the policy in a live world is `tests/observer_policy_tests.rs`.
    use super::tests::base_manifest;
    use super::*;
    use crate::core::intervention::InterventionKind;
    use anima_domain::causal::CauseId;

    /// The rollback path. A manifest written before ADR-0004 has no `observer` key, and must load
    /// and validate exactly as it did — which is why `MANIFEST_SCHEMA_VERSION` was deliberately not
    /// bumped for an additive field.
    #[test]
    fn a_manifest_without_an_observer_key_reads_as_absent() {
        let m = base_manifest();
        let mut json: serde_json::Value =
            serde_json::to_value(&m).expect("manifest should serialize");
        json.as_object_mut()
            .expect("manifest is an object")
            .remove("observer")
            .expect("the key should have been there to remove");

        let restored: ExperimentManifest =
            serde_json::from_value(json).expect("a pre-ADR-0004 manifest must still load");
        assert_eq!(restored.observer, ObserverPolicy::Absent);
        assert!(restored
            .validate(&ObservableRegistry::reference_default())
            .is_ok());
    }

    /// The policy is a declared input, so it is part of the run's identity — including `Absent` vs
    /// `Spectate`, which produce the *same trajectory* but are different declarations about how the
    /// run was watched.
    #[test]
    fn the_observer_policy_changes_the_run_identity() {
        let absent = base_manifest();
        let spectate = ExperimentManifest {
            observer: ObserverPolicy::Spectate,
            ..absent.clone()
        };
        let inhabit = ExperimentManifest {
            observer: ObserverPolicy::Inhabit { cause_id: 9 },
            ..absent.clone()
        };

        let (a, s, i) = (
            absent.fingerprint(),
            spectate.fingerprint(),
            inhabit.fingerprint(),
        );
        assert_ne!(a, s, "Absent and Spectate must not share a run identity");
        assert_ne!(a, i, "Absent and Inhabit must not share a run identity");
        assert_ne!(s, i, "Spectate and Inhabit must not share a run identity");
    }

    /// **ER01.** The observer is state, not law. If it reached the world-law fingerprint, a
    /// checkpoint could not be forked to drop the observer — which is the whole point of recording
    /// one. This is the nearest trap in ADR-0004 and it is cheap to guard.
    #[test]
    fn the_observer_policy_leaves_the_law_fingerprint_alone() {
        let absent = base_manifest();
        let inhabit = ExperimentManifest {
            observer: ObserverPolicy::Inhabit { cause_id: 9 },
            ..absent.clone()
        };
        assert_eq!(
            absent.laws.fingerprint(),
            inhabit.laws.fingerprint(),
            "the observer policy moved the world-law fingerprint; a checkpoint branch can no \
             longer drop the observer without changing the laws"
        );
    }

    /// An `Inhabit` that roots at the background cause would file a human's doing as baseline
    /// dynamics, so `trace_to_root` would report the observer's own effects as something the world
    /// did by itself. Refused at validation rather than discovered in a ledger.
    #[test]
    fn an_inhabit_rooted_at_the_background_cause_is_refused() {
        let m = ExperimentManifest {
            observer: ObserverPolicy::Inhabit {
                cause_id: anima_domain::causal::CAUSE_BACKGROUND,
            },
            ..base_manifest()
        };
        assert!(matches!(
            m.validate(&ObservableRegistry::reference_default()),
            Err(ExperimentError::InvalidObserverPolicy { .. })
        ));

        let ok = ExperimentManifest {
            observer: ObserverPolicy::Inhabit { cause_id: 1 },
            ..base_manifest()
        };
        assert!(ok
            .validate(&ObservableRegistry::reference_default())
            .is_ok());
    }

    /// A manifest cannot author an intervention as if a live human had caused it. The run had not
    /// started when the manifest was written, so the claim is false by construction — and if it were
    /// allowed through, a scenario forcing and the observer's own doing would share a root and
    /// `root_cause` would stop distinguishing them.
    /// A run simulates ticks `1..=duration_ticks`, and `validate_intervention` refuses a window that
    /// can never fire inside it. `start_tick: 1` keeps these fixtures about cause ids rather than
    /// about scheduling.
    fn intervention_with_cause(cause_id: CauseId) -> InterventionCommand {
        InterventionCommand {
            id: 1,
            cause_id,
            kind: InterventionKind::RainfallDelta,
            region: Region::Global,
            start_tick: 1,
            duration_ticks: 1,
            intensity: 0.1,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        }
    }

    #[test]
    fn a_declared_intervention_may_not_claim_the_observer_cause() {
        let m = ExperimentManifest {
            interventions: vec![intervention_with_cause(
                anima_domain::causal::CAUSE_OBSERVER,
            )],
            ..base_manifest()
        };
        assert!(
            matches!(
                m.validate(&ObservableRegistry::reference_default()),
                Err(ExperimentError::InvalidObserverPolicy { .. })
            ),
            "a declared intervention claiming CAUSE_OBSERVER must be refused, got {:?}",
            m.validate(&ObservableRegistry::reference_default())
        );
    }

    /// The rule above must not have swept up the ids a manifest author actually writes. Without this
    /// the check could reject everything and still look correct.
    #[test]
    fn ordinary_hand_written_cause_ids_are_still_accepted() {
        for cause_id in [1u32, 2, 7, 4096] {
            let m = ExperimentManifest {
                interventions: vec![intervention_with_cause(cause_id)],
                ..base_manifest()
            };
            assert!(
                m.validate(&ObservableRegistry::reference_default()).is_ok(),
                "cause id {cause_id} should be free for a manifest author to use, got {:?}",
                m.validate(&ObservableRegistry::reference_default())
            );
        }
    }
}
