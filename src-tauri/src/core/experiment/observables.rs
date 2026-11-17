//! The observable registry (AE-109): what a run may emit, under which scope, aggregation and
//! conservation role.

use super::*;

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
    pub(super) specs: Vec<ObservableSpec>,
}

/// The code symbol producing the AE3 population observables.
pub(super) const AE3_SOURCE: &str = "core::evolution_pathway::ReferencePopulation";

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
            eu("live.dormant_eu", ConservationRole::ClosedEu),
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
            // ---- Feature ecological-selection, branch O -------------------------------------------
            // Distributional and turnover observables: the quantities a frozen population keeps
            // constant and an evolving one does not. Each is emitted only when its producing
            // structure exists — see `LiveWorldConfig::ensure_observables_supported`, which
            // refuses death observables on runs that declare no death rule rather than
            // reporting fabricated zeros.
            eu("live.trait_mass_mean", ConservationRole::None),
            eu("live.trait_mass_var", ConservationRole::None),
            eu("live.trait_mass_p10", ConservationRole::None),
            eu("live.trait_mass_p90", ConservationRole::None),
            eu("live.shannon", ConservationRole::None),
            eu("live.simpson", ConservationRole::None),
            count("live.births"),
            count("live.deaths"),
            eu("live.mean_lifespan", ConservationRole::None),
            count("live.archive_coverage"),
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

pub(super) fn scope_tag(s: ObservableScope) -> u8 {
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
pub(super) fn agg_tag(a: Aggregation) -> u8 {
    match a {
        Aggregation::Instant => 0,
        Aggregation::Sum => 1,
        Aggregation::Mean => 2,
        Aggregation::Max => 3,
    }
}
pub(super) fn role_tag(r: ConservationRole) -> u8 {
    match r {
        ConservationRole::ClosedEu => 0,
        ConservationRole::ExoticMu => 1,
        ConservationRole::None => 2,
    }
}
