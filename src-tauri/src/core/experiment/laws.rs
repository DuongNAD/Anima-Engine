//! World laws (AE-102): the immutable law set of a run and its canonical fingerprint, and the
//! deterministic initial conditions (`t = 0`).

use super::*;

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

pub(super) fn source_model_tag(m: crate::core::exotic_energy::ExoticSourceModel) -> u8 {
    use crate::core::exotic_energy::ExoticSourceModel::*;
    match m {
        Renewable => 0x21,
    }
}

pub(super) fn boundary_tag(b: crate::core::exotic_energy::BoundaryMode) -> u8 {
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
