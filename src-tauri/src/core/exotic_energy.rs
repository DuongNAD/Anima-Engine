//! # Exotic energy (AE2) — a generic, budgeted spatial energy substrate.
//!
//! The core engine never speaks of "mana": it speaks of a generic [`ExoticEnergyLaw`] carrying an
//! opaque [`EnergySourceId`] and a display-only `display_name` (e.g. `"Mana"`). This keeps the same
//! machinery reusable for geothermal vents, radiation, chemosynthesis or a fantasy field — the
//! distinction the ADR-0002 decision rests on.
//!
//! Two hard invariants live here:
//!
//! - **MU is not EU.** An exotic law's [`UnitId`] must never be the biomass-equivalent energy unit
//!   (`EU`). The [`ExoticEnergyField`] and [`ExoticEnergyBudget`] keep an independent **MU ledger**;
//!   they can move MU around but never mint biomass, so the closed-EU contract in
//!   [`crate::core::sim_rules`] is untouched.
//! - **Disabled by default.** `exotic_energy = None` in a [`crate::core::experiment::WorldLawSet`]
//!   is the baseline-compatibility and rollback path: no field, no cost, no RNG draw.
//!
//! The field is a pre-allocated SoA double buffer (mirroring [`crate::core::dynamic_fields`]) whose
//! update loop performs **zero heap allocation** after construction, and it draws only from a
//! declared seed at construction time — never `thread_rng()` — so a run replays to the same MU
//! ledger every time.

use crate::core::causal::CauseId;
use crate::core::intervention::{Curve, Region};
use crate::core::sim_clock::ECOLOGY_PERIOD;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

mod budget;
mod field;
mod forcings;

pub use budget::*;
pub use field::*;
pub use forcings::*;

// ---- Typed identifiers (AE-201) --------------------------------------------------------------

// The unit vocabulary moved into `anima-domain` (G2 task 1): both engines must mean the same thing
// by "EU", and a vocabulary each defined for itself is the second source of truth G2 removes.
pub use anima_domain::units::{EnergySourceId, UnitId, EU_UNIT, MU_UNIT};

// ---- World-law data (AE-202 schema; behaviour validated below) -------------------------------

/// How an exotic source replenishes. The MVP has exactly one source model, `Renewable` (a steady
/// per-tick source rate with decay). There is **deliberately no `Disabled` variant**: the sole
/// baseline / "off" path is `exotic_energy = None` on the [`crate::core::experiment::WorldLawSet`],
/// which allocates no field, seeds no RNG and adds no cost. A `Some(ExoticEnergyLaw)` is therefore
/// *always* a live source and can never be mistaken for the baseline. `Finite` and `Pulsed` from the
/// explanation are deferred behind a future ADR and intentionally absent so no unsupported variant
/// can be silently mishandled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExoticSourceModel {
    /// A steady renewable source: each ecology tick injects `source_rate · mask` MU, bounded by
    /// `max_density`, offset by `decay_rate` dissipation.
    Renewable,
}

/// Spatial layout of the source. `Uniform` spreads the initial amount and source evenly; `Patchy`
/// concentrates it into `hotspot_count` deterministic hotspots of `radius_cells`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SourceTopology {
    Uniform,
    Patchy {
        hotspot_count: u16,
        radius_cells: f32,
    },
}

/// How the field treats MU that diffuses past the grid edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryMode {
    /// Reflecting edges — no MU leaves the grid (diffusion conserves the field total exactly).
    Closed,
    /// Absorbing edges — MU that would diffuse off-grid is booked as `dissipated` (a declared sink).
    Open,
}

/// The definition of an exotic energy source in a [`crate::core::experiment::WorldLawSet`]. This is a
/// *law of the run*, fixed before genesis — not a trait of any organism. `display_name` is the only
/// user-facing label; the core keys everything off `id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExoticEnergyLaw {
    pub id: EnergySourceId,
    /// Scenario/UI display label, e.g. `"Mana"`. Never used as a key.
    pub display_name: String,
    pub unit: UnitId,
    pub source_model: ExoticSourceModel,
    pub topology: SourceTopology,
    /// Initial total MU distributed across the field at genesis.
    pub initial_amount: f64,
    /// MU injected per ecology tick at full source strength (per unit source mask).
    pub source_rate: f32,
    /// Diffusion coefficient per ecology tick; must be in `[0, 0.25]` for an explicit-Euler
    /// 5-point stencil to stay stable.
    pub diffusion_rate: f32,
    /// Fraction of a cell's MU that dissipates per ecology tick.
    pub decay_rate: f32,
    /// Per-cell density ceiling (MU).
    pub max_density: f32,
    pub boundary: BoundaryMode,
}

impl ExoticEnergyLaw {
    /// The maximum stable diffusion coefficient for an explicit 5-point Laplacian.
    pub const MAX_STABLE_DIFFUSION: f32 = 0.25;

    /// A minimal renewable-patchy "Mana" law over a `hotspot_count`-hotspot field, for the reference
    /// vertical slice and fixtures.
    pub fn mana_patchy(initial_amount: f64, hotspot_count: u16) -> Self {
        Self {
            id: EnergySourceId::new("arcane_flux"),
            display_name: "Mana".into(),
            unit: UnitId::new(MU_UNIT),
            source_model: ExoticSourceModel::Renewable,
            topology: SourceTopology::Patchy {
                hotspot_count,
                radius_cells: 2.5,
            },
            initial_amount,
            source_rate: 0.05,
            diffusion_rate: 0.10,
            decay_rate: 0.01,
            max_density: 10.0,
            boundary: BoundaryMode::Closed,
        }
    }

    /// A uniform renewable "Mana" law spreading `initial_amount` MU evenly.
    pub fn mana_uniform(initial_amount: f64) -> Self {
        Self {
            id: EnergySourceId::new("arcane_flux"),
            display_name: "Mana".into(),
            unit: UnitId::new(MU_UNIT),
            source_model: ExoticSourceModel::Renewable,
            topology: SourceTopology::Uniform,
            initial_amount,
            source_rate: 0.02,
            diffusion_rate: 0.08,
            decay_rate: 0.005,
            max_density: 10.0,
            boundary: BoundaryMode::Closed,
        }
    }

    /// Validate the law's numeric ranges and unit. Returns a human-readable reason on failure; the
    /// manifest validator wraps this into a structured [`crate::core::experiment::ExperimentError`].
    pub fn validate(&self) -> Result<(), String> {
        if self.id.as_str().is_empty() {
            return Err("exotic energy law has an empty source id".into());
        }
        if self.display_name.is_empty() {
            return Err("exotic energy law has an empty display_name".into());
        }
        if self.unit.is_eu() {
            return Err(format!(
                "exotic energy unit must not be the closed-EU unit '{EU_UNIT}' (MU is not EU)"
            ));
        }
        if self.unit.as_str().is_empty() {
            return Err("exotic energy law has an empty unit".into());
        }
        for (name, v) in [
            ("source_rate", self.source_rate),
            ("diffusion_rate", self.diffusion_rate),
            ("decay_rate", self.decay_rate),
            ("max_density", self.max_density),
        ] {
            if !v.is_finite() {
                return Err(format!("exotic energy {name} is not finite ({v})"));
            }
            if v < 0.0 {
                return Err(format!("exotic energy {name} must be >= 0 (got {v})"));
            }
        }
        if !self.initial_amount.is_finite() || self.initial_amount < 0.0 {
            return Err(format!(
                "exotic energy initial_amount must be finite and >= 0 (got {})",
                self.initial_amount
            ));
        }
        if self.diffusion_rate > Self::MAX_STABLE_DIFFUSION {
            return Err(format!(
                "exotic energy diffusion_rate {} exceeds the stable limit {}",
                self.diffusion_rate,
                Self::MAX_STABLE_DIFFUSION
            ));
        }
        if self.decay_rate > 1.0 {
            return Err(format!(
                "exotic energy decay_rate {} exceeds 1.0",
                self.decay_rate
            ));
        }
        if self.max_density <= 0.0 {
            return Err(format!(
                "exotic energy max_density must be > 0 (got {})",
                self.max_density
            ));
        }
        if let SourceTopology::Patchy {
            hotspot_count,
            radius_cells,
        } = self.topology
        {
            if hotspot_count == 0 {
                return Err("patchy topology needs at least one hotspot".into());
            }
            if !(radius_cells.is_finite() && radius_cells > 0.0) {
                return Err(format!(
                    "patchy hotspot radius must be finite and > 0 (got {radius_cells})"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
