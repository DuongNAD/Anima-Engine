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

// ---- Runtime source forcings (AE-209) --------------------------------------------------------

/// What a runtime exotic-source forcing does to the field.
///
/// These are **declared forcings on field state**, never edits to the [`ExoticEnergyLaw`]: a
/// `WorldLawSet` is immutable within a run (ER01), so "add a source at generation G" is an
/// intervention with a `CauseId`, not a silent law mutation. Every MU injected by `AddSource` or
/// `Pulse` is booked in the MU ledger. `RemoveSource` is different: it prevents part of the
/// renewable contribution from entering the field, so no stored MU is moved and the budget stays
/// closed through a lower `cum_sourced` value (ER04 / AE-S04).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExoticInterventionKind {
    /// Inject MU into the region each firing tick (a declared **source**).
    AddSource,
    /// Suppress the renewable source contribution in the region for this firing tick. This never
    /// drains stored MU and is not a dissipative sink.
    RemoveSource,
    /// A transient injection shaped by the curve across the window (a declared source that returns
    /// to zero); semantically `AddSource` with a non-`Step` curve, kept distinct so a scenario can
    /// state its intent and a reader can tell a pulse from a sustained source.
    Pulse,
}

/// A runtime forcing on the exotic field: apply `amount` MU (shaped by `curve`) to `region` over
/// `[start_tick, start_tick + effective_duration)`, attributed to `cause_id`.
///
/// Deliberately a **separate type** from [`crate::core::intervention::InterventionCommand`]: the
/// legacy command drives the trophic/abiotic model and its five kinds are matched exhaustively by
/// `core/scenario.rs`, which must stay untouched (AE-111). Reusing its `Region`/`Curve` keeps one
/// spatial/temporal vocabulary without widening the legacy enum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExoticIntervention {
    pub id: u32,
    pub cause_id: CauseId,
    pub kind: ExoticInterventionKind,
    pub region: Region,
    pub start_tick: u64,
    /// Length of the active window in base ticks; `0` means a single-tick (instantaneous) forcing.
    pub duration_ticks: u64,
    /// Magnitude in **MU per ecology firing, per affected cell**. Non-negative — the direction/role
    /// is carried by `kind`. Because exotic dynamics advance only on the ecology band, a window that
    /// contains no ecology firing tick is rejected by [`validate`](Self::validate).
    pub amount: f32,
    pub curve: Curve,
}

impl ExoticIntervention {
    /// The effective length of the active window (a `0` duration counts as one tick).
    pub fn effective_duration(&self) -> u64 {
        self.duration_ticks.max(1)
    }

    /// Whether this forcing is active at `tick`.
    pub fn active_at(&self, tick: u64) -> bool {
        tick >= self.start_tick && tick < self.start_tick + self.effective_duration()
    }

    /// The (non-negative) magnitude applied at `tick`, following the ramp curve; `0` outside the
    /// window.
    pub fn amount_at(&self, tick: u64) -> f32 {
        if !self.active_at(tick) {
            return 0.0;
        }
        let dur = self.effective_duration();
        let progress = if dur <= 1 {
            1.0
        } else {
            (tick - self.start_tick) as f32 / (dur - 1) as f32
        };
        self.amount * self.curve.factor(progress)
    }

    /// Whether this forcing affects grid cell `(x, y)`.
    pub fn affects(&self, x: u32, y: u32) -> bool {
        self.region.contains(x, y)
    }

    /// Validate the forcing for a run of `run_ticks` base ticks (tick domain `1..=run_ticks`).
    /// Mirrors the manifest-path rules used for legacy interventions: finite non-negative magnitude,
    /// valid geometry, no window overflow, and an active window that can actually fire.
    pub fn validate(&self, run_ticks: u64) -> Result<(), String> {
        if !self.amount.is_finite() {
            return Err(format!("exotic forcing {} amount is not finite", self.id));
        }
        if self.amount < 0.0 {
            return Err(format!(
                "exotic forcing {} amount must be >= 0 (direction is the kind, got {})",
                self.id, self.amount
            ));
        }
        match self.region {
            Region::Global | Region::Cell { .. } => {}
            Region::Rect {
                min_x,
                min_y,
                max_x,
                max_y,
            } => {
                if min_x > max_x || min_y > max_y {
                    return Err(format!(
                        "exotic forcing {} has inverted Rect bounds",
                        self.id
                    ));
                }
            }
            Region::Radius { cx, cy, r } => {
                if !(cx.is_finite() && cy.is_finite() && r.is_finite()) {
                    return Err(format!(
                        "exotic forcing {} has a non-finite Radius region",
                        self.id
                    ));
                }
                if r <= 0.0 {
                    return Err(format!(
                        "exotic forcing {} radius must be > 0 (got {r})",
                        self.id
                    ));
                }
            }
        }
        let end_exclusive = self
            .start_tick
            .checked_add(self.effective_duration())
            .ok_or_else(|| {
                format!(
                    "exotic forcing {} start_tick + duration overflows u64",
                    self.id
                )
            })?;
        if !(self.start_tick <= run_ticks && end_exclusive > 1) {
            return Err(format!(
                "exotic forcing {} can never fire within run ticks 1..={run_ticks}",
                self.id
            ));
        }
        // Cadence: exotic dynamics advance only on the ECOLOGY band, so a window that contains no
        // ecology firing tick would silently never apply. Reject it rather than accept a dead
        // forcing. The first firing at or after `start_tick` is the next multiple of the period.
        let period = ECOLOGY_PERIOD;
        let lo = self.start_tick.max(1);
        let first_firing = lo.div_ceil(period).saturating_mul(period);
        if first_firing >= end_exclusive || first_firing > run_ticks || first_firing == 0 {
            return Err(format!(
                "exotic forcing {} has no ecology-band firing tick (period {period}) inside its \
                 window [{}, {end_exclusive}) within run ticks 1..={run_ticks}; `amount` is MU per \
                 ecology firing per affected cell, so this forcing could never apply",
                self.id, self.start_tick
            ));
        }
        Ok(())
    }

    /// Validate this forcing's region against a concrete grid without constructing a field.
    ///
    /// `Cell` and `Rect` must be fully contained in the grid. `Radius` must have a finite,
    /// strictly-positive radius and an in-grid centre; the disc itself may overhang an edge and is
    /// clipped when applied.
    pub fn validate_grid_applicability(&self, width: usize, height: usize) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err(format!(
                "exotic forcing {} cannot target an empty {width}x{height} field",
                self.id
            ));
        }
        match self.region {
            Region::Global => Ok(()),
            Region::Cell { x, y } => {
                if (x as usize) < width && (y as usize) < height {
                    Ok(())
                } else {
                    Err(format!(
                        "exotic forcing {} targets cell ({x},{y}) outside the {width}x{height} field",
                        self.id
                    ))
                }
            }
            Region::Rect {
                min_x,
                min_y,
                max_x,
                max_y,
            } => {
                if min_x <= max_x
                    && min_y <= max_y
                    && (max_x as usize) < width
                    && (max_y as usize) < height
                {
                    Ok(())
                } else {
                    Err(format!(
                        "exotic forcing {} targets rect ({min_x},{min_y})..=({max_x},{max_y}) \
                         outside the {width}x{height} field; Rect regions must be fully contained",
                        self.id
                    ))
                }
            }
            Region::Radius { cx, cy, r } => {
                if !(cx.is_finite() && cy.is_finite() && r.is_finite()) || r <= 0.0 {
                    return Err(format!(
                        "exotic forcing {} has an invalid Radius region (cx={cx}, cy={cy}, r={r})",
                        self.id
                    ));
                }
                if cx < 0.0 || cy < 0.0 || cx >= width as f32 || cy >= height as f32 {
                    return Err(format!(
                        "exotic forcing {} has a Radius centre ({cx},{cy}) outside the \
                         {width}x{height} field",
                        self.id
                    ));
                }
                Ok(())
            }
        }
    }
}

/// A validated, deterministically-ordered set of runtime forcings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExoticInterventionQueue {
    commands: Vec<ExoticIntervention>,
}

impl ExoticInterventionQueue {
    /// Validate every member for a `run_ticks`-long run, reject duplicate ids, and store them in a
    /// stable `(start_tick, id)` order so application is deterministic regardless of input order.
    pub fn new(mut commands: Vec<ExoticIntervention>, run_ticks: u64) -> Result<Self, String> {
        for (i, c) in commands.iter().enumerate() {
            c.validate(run_ticks)?;
            for other in &commands[i + 1..] {
                if other.id == c.id {
                    return Err(format!("duplicate exotic forcing id {}", c.id));
                }
            }
        }
        commands.sort_by_key(|c| (c.start_tick, c.id));
        Ok(Self { commands })
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn all(&self) -> &[ExoticIntervention] {
        &self.commands
    }

    /// The forcings active at `tick`, in deterministic order.
    pub fn active_at(&self, tick: u64) -> impl Iterator<Item = &ExoticIntervention> {
        self.commands.iter().filter(move |c| c.active_at(tick))
    }
}

// ---- Budget ledger (AE-204) ------------------------------------------------------------------

/// An audit of where every MU is. The closure equation (design §"Dynamic exotic field and budget"):
///
/// ```text
/// initial + sourced  =  field + organism_storage + dissipated + exported  ± tolerance
/// ```
///
/// [`balance_error`](Self::balance_error) is the left minus the right; a well-behaved subsystem keeps
/// it within the documented numeric tolerance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExoticEnergyBudget {
    pub unit: UnitId,
    /// MU present at genesis.
    pub initial: f64,
    /// MU injected by the source over the run.
    pub sourced: f64,
    /// MU currently held in the spatial field.
    pub field: f64,
    /// MU currently held in organism storage (a reference-model scalar in this slice).
    pub organism_storage: f64,
    /// MU removed by decay / spend (a declared sink).
    pub dissipated: f64,
    /// MU that left the modelled system (e.g. off-grid at an open boundary).
    pub exported: f64,
}

impl ExoticEnergyBudget {
    /// `(initial + sourced) − (field + storage + dissipated + exported)`. Zero means perfect MU
    /// conservation.
    pub fn balance_error(&self) -> f64 {
        (self.initial + self.sourced)
            - (self.field + self.organism_storage + self.dissipated + self.exported)
    }

    /// A scale for a *relative* balance check: the total MU that has ever entered the system.
    pub fn throughput(&self) -> f64 {
        (self.initial + self.sourced).max(1.0)
    }
}

// ---- The field (AE-203) ----------------------------------------------------------------------

/// A pre-allocated SoA exotic-energy field on the sim grid. `density` is the live MU per cell;
/// `next` is a reused scratch buffer for the diffusion double-buffer; `source_mask` is the fixed
/// per-cell source multiplier (uniform → all 1, patchy → hotspot bumps). The cumulative f64 counters
/// are the MU ledger — the machine side of AE-S04.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExoticEnergyField {
    pub source_id: EnergySourceId,
    pub unit: UnitId,
    pub width: usize,
    pub height: usize,
    pub density: Vec<f32>,
    /// Reused diffusion scratch (never reallocated after construction).
    next: Vec<f32>,
    /// Fixed per-cell source multiplier (preallocated once).
    source_mask: Vec<f32>,
    /// Per-tick source-suppression request accumulated from active `RemoveSource` forcings
    /// (preallocated once; zeroed by [`begin_tick_suppression`](Self::begin_tick_suppression)).
    #[serde(default)]
    suppress: Vec<f32>,
    pub source_rate: f32,
    pub diffusion_rate: f32,
    pub decay_rate: f32,
    pub max_density: f32,
    pub boundary: BoundaryMode,
    pub source_model: ExoticSourceModel,

    // ---- MU ledger (f64) --------------------------------------------------------------------
    pub initial: f64,
    pub cum_sourced: f64,
    pub cum_dissipated: f64,
    pub cum_exported: f64,
    /// Cumulative MU that the base renewable source **would have** injected but did not, because a
    /// `RemoveSource` forcing suppressed it. This is a **counterfactual diagnostic**, not a ledger
    /// slot: suppressed MU never existed, so it does **not** appear in the balance equation (it shows
    /// up simply as a lower `cum_sourced`).
    #[serde(default)]
    pub cum_source_suppressed: f64,
}

impl ExoticEnergyField {
    /// Build the field from a law over a `width × height` grid, using `seed` for deterministic
    /// hotspot placement (never `thread_rng`). Allocates once here; the per-tick [`step`](Self::step)
    /// never allocates.
    ///
    /// Returns `Err` if `initial_amount` cannot be placed **without clamping** — i.e. the declared MU
    /// exceeds what the topology + `max_density` + grid can hold. This is deliberate: silently
    /// clamping would change the run's declared initial condition (recording less MU than asked). The
    /// realized field total therefore always equals the declared `initial_amount` up to f32
    /// representation rounding (a few ULP), never a capacity truncation.
    pub fn from_law(
        law: &ExoticEnergyLaw,
        width: usize,
        height: usize,
        seed: u64,
    ) -> Result<Self, String> {
        // This constructor is public, so it re-validates the law defensively rather than trusting the
        // caller to have run `ExoticEnergyLaw::validate` first.
        law.validate()?;
        // Zero dimensions are REJECTED, never silently coerced to 1 (that would quietly change the
        // declared world size); an overflowing grid is rejected instead of wrapping.
        if width == 0 || height == 0 {
            return Err(format!(
                "exotic field grid must be non-empty (got {width}x{height})"
            ));
        }
        let (w, h) = (width, height);
        let n = w
            .checked_mul(h)
            .ok_or_else(|| format!("exotic field grid {w}x{h} overflows addressable size"))?;

        // Source mask: uniform = 1 everywhere; patchy = a raised-cosine bump around deterministic
        // hotspot centres derived from the seed + source id.
        let mut source_mask = vec![0.0f32; n];
        match law.topology {
            SourceTopology::Uniform => {
                source_mask.iter_mut().for_each(|m| *m = 1.0);
            }
            SourceTopology::Patchy {
                hotspot_count,
                radius_cells,
            } => {
                let stream = seed ^ fnv1a_64_str(law.id.as_str());
                let mut rng = StdRng::seed_from_u64(stream);
                let r = radius_cells.max(0.5);
                for _ in 0..hotspot_count.max(1) {
                    let cx = rng.gen_range(0..w) as f32;
                    let cy = rng.gen_range(0..h) as f32;
                    for row in 0..h {
                        for col in 0..w {
                            let dx = col as f32 - cx;
                            let dy = row as f32 - cy;
                            let d2 = dx * dx + dy * dy;
                            if d2 <= r * r {
                                // Raised-cosine falloff to 0 at the radius.
                                let d = d2.sqrt() / r;
                                let bump = 0.5 * (1.0 + (std::f32::consts::PI * d).cos());
                                let i = row * w + col;
                                source_mask[i] = (source_mask[i] + bump).min(1.0);
                            }
                        }
                    }
                }
                // A degenerate mask (no cell covered) would make the source inert; guarantee at least
                // one active cell so `initial_amount` has somewhere to live.
                if source_mask.iter().all(|&m| m == 0.0) {
                    source_mask[0] = 1.0;
                }
            }
        }

        // Feasibility: the initial amount is distributed proportionally to the source mask, so the
        // most-loaded cell receives `initial_amount · max_mask / mask_sum`. If that would exceed
        // `max_density`, the amount cannot be placed without clamping — reject rather than silently
        // shrink the declared initial condition.
        let mask_sum: f64 = source_mask.iter().map(|&m| m as f64).sum();
        let max_mask = source_mask.iter().cloned().fold(0.0f32, f32::max) as f64;
        if law.initial_amount > 0.0 {
            if mask_sum <= 0.0 || max_mask <= 0.0 {
                return Err("exotic field has no active cells to hold initial_amount".into());
            }
            let feasible_max = law.max_density as f64 * mask_sum / max_mask;
            // Allow a hair of slack for the boundary case where the peak cell lands exactly at
            // max_density (f32 rounding), but reject anything genuinely over capacity.
            if law.initial_amount > feasible_max * (1.0 + 1e-6) {
                return Err(format!(
                    "initial_amount {} exceeds field capacity {:.6} (topology/max_density {} over {}x{} grid); \
                     refusing to silently clamp",
                    law.initial_amount, feasible_max, law.max_density, w, h
                ));
            }
        }

        // Distribute the (feasible) initial amount proportionally to the source mask. The `.min` is a
        // pure ULP guard on the peak cell — feasibility above guarantees it removes only f32 rounding,
        // never real MU — so the realized total equals `initial_amount` up to f32 rounding.
        let mut density = vec![0.0f32; n];
        if mask_sum > 0.0 && law.initial_amount > 0.0 {
            for i in 0..n {
                let share = law.initial_amount * (source_mask[i] as f64 / mask_sum);
                density[i] = (share as f32).min(law.max_density);
            }
        }
        let initial = density.iter().map(|&v| v as f64).sum::<f64>();

        Ok(Self {
            source_id: law.id.clone(),
            unit: law.unit.clone(),
            width: w,
            height: h,
            density,
            next: vec![0.0f32; n],
            source_mask,
            suppress: vec![0.0f32; n],
            source_rate: law.source_rate,
            diffusion_rate: law.diffusion_rate,
            decay_rate: law.decay_rate,
            max_density: law.max_density,
            boundary: law.boundary,
            source_model: law.source_model,
            initial,
            cum_sourced: 0.0,
            cum_dissipated: 0.0,
            cum_exported: 0.0,
            cum_source_suppressed: 0.0,
        })
    }

    #[inline]
    fn n(&self) -> usize {
        self.width * self.height
    }

    /// Total MU currently in the field.
    pub fn total(&self) -> f64 {
        self.density.iter().map(|&v| v as f64).sum()
    }

    /// Advance the field one ecology tick: **source → decay → diffusion**, tracking every MU into the
    /// ledger. Allocation-free (uses the preallocated `next` scratch and `source_mask`). Deterministic
    /// (pure arithmetic, no RNG).
    pub fn step(&mut self) {
        let n = self.n();

        // 1) Source injection. `Renewable` is the only source model (a `Some(law)` is always a live
        //    source — the baseline is `exotic_energy = None`), so injection always applies. The ACTUAL
        //    increase is booked as `sourced` so a cell already at the ceiling contributes nothing (no
        //    phantom MU).
        //    A `RemoveSource` forcing SUPPRESSES this contribution (capped at it) instead of draining
        //    stored MU: the suppressed amount is simply never sourced, so `cum_sourced` is lower and
        //    `cum_source_suppressed` records the counterfactual.
        if self.source_rate > 0.0 {
            for i in 0..n {
                let intended = self.source_rate * self.source_mask[i];
                if intended <= 0.0 {
                    continue;
                }
                let headroom = (self.max_density - self.density[i]).max(0.0);
                let actual_base_contribution = intended.min(headroom);
                let suppressed = self.suppress[i].min(actual_base_contribution).max(0.0);
                if suppressed > 0.0 {
                    self.cum_source_suppressed += suppressed as f64;
                }
                let add = actual_base_contribution - suppressed;
                if add <= 0.0 {
                    continue;
                }
                let before = self.density[i];
                self.density[i] = before + add;
                self.cum_sourced += (self.density[i] - before) as f64;
            }
        }
        // Suppression is a one-firing input. Consume it here as a defence against a caller
        // accidentally carrying a previous tick's request into the next field step.
        self.suppress.fill(0.0);

        // 2) Decay: a fraction of each cell dissipates (a declared sink).
        if self.decay_rate > 0.0 {
            for i in 0..n {
                let lost = self.decay_rate * self.density[i];
                if lost > 0.0 {
                    self.density[i] -= lost;
                    self.cum_dissipated += lost as f64;
                }
            }
        }

        // 3) Diffusion: explicit 5-point Laplacian into `next`. Closed boundary reflects (a missing
        //    neighbour contributes no flux across the edge → MU conserved); Open boundary treats an
        //    off-grid neighbour as a 0-density sink and books the net outflux as `exported`. The
        //    exported amount is measured as the exact total-MU difference so the ledger closes to f64
        //    precision regardless of per-cell rounding.
        if self.diffusion_rate > 0.0 {
            let d = self.diffusion_rate;
            let w = self.width;
            let h = self.height;
            let open = matches!(self.boundary, BoundaryMode::Open);
            for row in 0..h {
                for col in 0..w {
                    let i = row * w + col;
                    let c = self.density[i];
                    let mut acc = 0.0f32; // sum of (neighbour - c) over 4 neighbours
                    let mut edge_missing = 0i32;
                    if col > 0 {
                        acc += self.density[i - 1] - c;
                    } else {
                        edge_missing += 1;
                    }
                    if col + 1 < w {
                        acc += self.density[i + 1] - c;
                    } else {
                        edge_missing += 1;
                    }
                    if row > 0 {
                        acc += self.density[i - w] - c;
                    } else {
                        edge_missing += 1;
                    }
                    if row + 1 < h {
                        acc += self.density[i + w] - c;
                    } else {
                        edge_missing += 1;
                    }
                    if open {
                        // Each missing neighbour acts as a 0-density sink: adds (0 - c) to the flux.
                        self.next[i] = c + d * (acc - c * edge_missing as f32);
                    } else {
                        self.next[i] = c + d * acc;
                    }
                }
            }
            if open {
                let before: f64 = self.density.iter().map(|&v| v as f64).sum();
                self.density.copy_from_slice(&self.next);
                let after: f64 = self.density.iter().map(|&v| v as f64).sum();
                self.cum_exported += (before - after).max(0.0);
            } else {
                self.density.copy_from_slice(&self.next);
            }
        }
    }

    /// Apply a runtime source forcing (AE-209) to the field at `tick`, returning the **actual** MU
    /// moved (always non-negative). This is a *state* effect on the field, never a law change: the
    /// [`ExoticEnergyLaw`] and its fingerprint are untouched (ER01).
    ///
    /// Every injected MU is booked so the ledger stays closed (AE-S04):
    /// - [`ExoticInterventionKind::AddSource`] / [`ExoticInterventionKind::Pulse`] credit
    ///   `cum_sourced` by the actual increase (a cell already at `max_density` contributes nothing);
    /// - [`ExoticInterventionKind::RemoveSource`] is intentionally a no-op here because it is source
    ///   suppression, handled by [`add_source_suppression`](Self::add_source_suppression), not field
    ///   movement.
    ///
    /// Allocation-free and deterministic (pure arithmetic over the region, no RNG).
    pub fn apply_forcing(&mut self, cmd: &ExoticIntervention, tick: u64) -> f64 {
        // Only external INJECTIONS move MU here. `RemoveSource` is a source *suppression*, applied
        // via [`add_source_suppression`](Self::add_source_suppression) — routing it here must be a
        // no-op so the old drain semantics can never be reintroduced by accident.
        if matches!(cmd.kind, ExoticInterventionKind::RemoveSource) {
            return 0.0;
        }
        let per_cell = cmd.amount_at(tick);
        if !per_cell.is_finite() || per_cell <= 0.0 {
            return 0.0;
        }
        let mut moved = 0.0f64;
        for row in 0..self.height {
            for col in 0..self.width {
                if !cmd.affects(col as u32, row as u32) {
                    continue;
                }
                let i = row * self.width + col;
                let before = self.density[i];
                let after = (before + per_cell).min(self.max_density);
                let delta = (after - before) as f64;
                if delta <= 0.0 {
                    continue;
                }
                self.density[i] = after;
                moved += delta;
            }
        }
        if moved > 0.0 {
            self.cum_sourced += moved;
        }
        moved
    }

    /// Check that a forcing's region is valid for **this** grid.
    ///
    /// - `Global` always applies.
    /// - `Cell` / `Rect` must be fully contained in the grid.
    /// - `Radius` must have a finite, strictly-positive radius and an **in-grid centre**. A disc whose
    ///   centre is in-grid but which overhangs an edge is accepted and **clipped** to the grid — only
    ///   the in-grid cells are affected.
    pub fn validate_region_applicable(&self, cmd: &ExoticIntervention) -> Result<(), String> {
        cmd.validate_grid_applicability(self.width, self.height)
    }

    /// Clear the per-tick suppression buffer. Call once per ecology firing tick **before** any
    /// [`add_source_suppression`](Self::add_source_suppression) and before [`step`](Self::step).
    /// Allocation-free.
    pub fn begin_tick_suppression(&mut self) {
        self.suppress.fill(0.0);
    }

    /// Accumulate a `RemoveSource` forcing's requested suppression for this tick, returning the
    /// **effective** MU this command actually suppresses (its incremental contribution after the
    /// joint cap at the base source contribution that could enter the cell's remaining headroom).
    /// Apply external `AddSource` / `Pulse` injections first, then accumulate suppressions.
    /// Overlapping suppression commands are credited in queue order, so attribution is deterministic
    /// and the total can never exceed the realizable renewable contribution.
    ///
    /// Returns `0.0` for a non-`RemoveSource` command or outside the command's window. Never touches
    /// stored MU. Allocation-free.
    pub fn add_source_suppression(&mut self, cmd: &ExoticIntervention, tick: u64) -> f64 {
        if !matches!(cmd.kind, ExoticInterventionKind::RemoveSource) {
            return 0.0;
        }
        let per_cell = cmd.amount_at(tick);
        if !per_cell.is_finite() || per_cell <= 0.0 || self.source_rate <= 0.0 {
            return 0.0;
        }
        let mut effective = 0.0f64;
        for row in 0..self.height {
            for col in 0..self.width {
                if !cmd.affects(col as u32, row as u32) {
                    continue;
                }
                let i = row * self.width + col;
                let intended = self.source_rate * self.source_mask[i];
                let headroom = (self.max_density - self.density[i]).max(0.0);
                let cap = intended.min(headroom);
                if cap <= 0.0 {
                    continue;
                }
                let before_eff = self.suppress[i].min(cap);
                let after_eff = (before_eff + per_cell).min(cap);
                self.suppress[i] = after_eff;
                effective += (after_eff - before_eff) as f64;
            }
        }
        effective
    }

    /// Withdraw up to `requested` MU from cell `idx` into `storage` (bounded by `capacity`). Returns
    /// the amount actually transferred, which is the **field's true f32 decrease** — storage is
    /// credited exactly that, never the pre-rounding `requested`. This keeps the transaction
    /// ledger-exact across the f32 field / f64 storage boundary: `field_delta == storage_gain` to the
    /// bit, so repeated fractional uptakes never leak or mint MU (the atomic uptake of AE-205).
    pub fn uptake(&mut self, idx: usize, requested: f64, storage: &mut f64, capacity: f64) -> f64 {
        // Reject invalid inputs BEFORE touching any state. A plain `requested <= 0.0` guard is not
        // enough: `f64::min` returns the NON-NaN operand, so a NaN request would survive the clamps
        // and move real MU. Every numeric input — including the CURRENT storage and the cell's own
        // density — must be finite and non-negative, or the transaction is a no-op returning 0.
        if idx >= self.density.len() {
            return 0.0;
        }
        let cell = self.density[idx] as f64;
        if !(requested.is_finite()
            && capacity.is_finite()
            && storage.is_finite()
            && cell.is_finite())
        {
            return 0.0;
        }
        if requested <= 0.0 || capacity < 0.0 || *storage < 0.0 || cell <= 0.0 {
            return 0.0;
        }
        let room = (capacity - *storage).max(0.0);
        let want = requested.min(cell).min(room);
        if !(want.is_finite() && want > 0.0) {
            return 0.0;
        }
        // Perform the withdrawal in f32 (the field's precision) and measure the ACTUAL decrease, so
        // the amount credited to storage is exactly what left the field — not the f64 request.
        let before = self.density[idx];
        let after = (before - want as f32).max(0.0);
        let actual = (before - after) as f64;
        self.density[idx] = after;
        *storage += actual;
        actual
    }

    /// The current [`ExoticEnergyBudget`] given an organism-storage total. Closes the MU equation.
    pub fn budget(&self, organism_storage: f64) -> ExoticEnergyBudget {
        ExoticEnergyBudget {
            unit: self.unit.clone(),
            initial: self.initial,
            sourced: self.cum_sourced,
            field: self.total(),
            organism_storage,
            dissipated: self.cum_dissipated,
            exported: self.cum_exported,
        }
    }
}

/// Spend `amount` MU from `storage`, booking it as `dissipated`. Returns the amount spent (bounded by
/// what is stored). The atomic spend transaction of AE-205, keeping the MU ledger closed.
pub fn spend_storage(storage: &mut f64, amount: f64, dissipated: &mut f64) -> f64 {
    // As in [`ExoticEnergyField::uptake`], a bare `amount <= 0.0` guard is insufficient: a NaN
    // `amount` passes it and `NaN.min(*storage)` yields `*storage`, draining the entire balance. Both
    // the request AND the two ledger slots must be finite and non-negative, or this is a no-op.
    if !(amount.is_finite() && storage.is_finite() && dissipated.is_finite()) {
        return 0.0;
    }
    if amount <= 0.0 || *storage <= 0.0 || *dissipated < 0.0 {
        return 0.0;
    }
    let spent = amount.min(*storage);
    if !(spent.is_finite() && spent > 0.0) {
        return 0.0;
    }
    *storage -= spent;
    *dissipated += spent;
    spent
}

/// FNV-1a 64 over a string — used to derive a per-source deterministic RNG stream at construction.
fn fnv1a_64_str(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in s.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- AE-202 law validation --------------------------------------------------------------

    #[test]
    fn law_rejects_eu_unit_and_bad_ranges() {
        // MU is not EU: a law claiming the EU unit is rejected.
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.unit = UnitId::new(EU_UNIT);
        assert!(law.validate().unwrap_err().contains("EU"));

        // Negative source rate.
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.source_rate = -0.1;
        assert!(law.validate().is_err());

        // Unstable diffusion.
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.diffusion_rate = 0.9;
        assert!(law.validate().unwrap_err().contains("diffusion"));

        // Zero hotspots in a patchy law.
        let mut law = ExoticEnergyLaw::mana_patchy(100.0, 3);
        law.topology = SourceTopology::Patchy {
            hotspot_count: 0,
            radius_cells: 2.0,
        };
        assert!(law.validate().is_err());

        // A well-formed law passes.
        assert!(ExoticEnergyLaw::mana_uniform(100.0).validate().is_ok());
        assert!(ExoticEnergyLaw::mana_patchy(100.0, 4).validate().is_ok());
    }

    fn build(law: &ExoticEnergyLaw, w: usize, h: usize, seed: u64) -> ExoticEnergyField {
        ExoticEnergyField::from_law(law, w, h, seed).expect("feasible field")
    }

    // ---- AE-203 field init ------------------------------------------------------------------

    #[test]
    fn uniform_field_has_the_declared_initial_total_and_is_bounded() {
        let law = ExoticEnergyLaw::mana_uniform(160.0);
        let f = build(&law, 8, 8, 1);
        // Uniform: every cell equal, total ~= initial_amount.
        assert!((f.total() - 160.0).abs() < 1e-3, "total was {}", f.total());
        assert!(f
            .density
            .iter()
            .all(|&v| (0.0..=law.max_density).contains(&v)));
        // All equal (uniform).
        let first = f.density[0];
        assert!(f.density.iter().all(|&v| (v - first).abs() < 1e-4));
    }

    #[test]
    fn patchy_field_is_deterministic_by_seed_and_bounded() {
        let law = ExoticEnergyLaw::mana_patchy(200.0, 5);
        let a = build(&law, 16, 16, 42);
        let b = build(&law, 16, 16, 42);
        // Same seed → identical field (no thread_rng anywhere).
        assert_eq!(a.density, b.density);
        // A different seed places hotspots differently.
        let c = build(&law, 16, 16, 43);
        assert_ne!(a.density, c.density);
        // Bounded and non-negative.
        assert!(a
            .density
            .iter()
            .all(|&v| (0.0..=law.max_density).contains(&v)));
        // Concentrated: not every cell is equal.
        let first = a.density[0];
        assert!(a.density.iter().any(|&v| (v - first).abs() > 1e-4));
    }

    #[test]
    fn defect_c_law_rejects_empty_display_name() {
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.display_name = String::new();
        assert!(
            law.validate().is_err(),
            "a law with no display name cannot be presented to a user"
        );
    }

    #[test]
    fn defect_c_from_law_validates_defensively_and_rejects_bad_grids() {
        // `from_law` is public, so it must not boot on an invalid law even if a caller skipped
        // `validate()` (defence in depth).
        let mut bad = ExoticEnergyLaw::mana_uniform(10.0);
        bad.unit = UnitId::new(EU_UNIT); // MU is not EU
        assert!(ExoticEnergyField::from_law(&bad, 8, 8, 1).is_err());

        let mut bad = ExoticEnergyLaw::mana_uniform(10.0);
        bad.diffusion_rate = 0.9; // unstable
        assert!(ExoticEnergyField::from_law(&bad, 8, 8, 1).is_err());

        // Zero dimensions must be REJECTED, not silently coerced to a 1x1 grid (which would
        // quietly change the declared world size).
        let good = ExoticEnergyLaw::mana_uniform(10.0);
        assert!(ExoticEnergyField::from_law(&good, 0, 8, 1).is_err());
        assert!(ExoticEnergyField::from_law(&good, 8, 0, 1).is_err());

        // An overflowing grid is rejected rather than panicking / wrapping.
        assert!(ExoticEnergyField::from_law(&good, usize::MAX, 2, 1).is_err());

        // A sane grid still builds.
        assert!(ExoticEnergyField::from_law(&good, 8, 8, 1).is_ok());
    }

    #[test]
    fn over_capacity_initial_amount_is_rejected_not_silently_clamped() {
        // A uniform 4x4 field with max_density 10 holds at most 16*10 = 160 MU. Asking for 400 MU
        // used to be silently clamped (recording a smaller `initial`); it must now be a hard error so
        // the declared initial condition is never quietly changed.
        let mut law = ExoticEnergyLaw::mana_uniform(400.0);
        law.max_density = 10.0;
        let err = ExoticEnergyField::from_law(&law, 4, 4, 1).unwrap_err();
        assert!(err.contains("capacity"), "unexpected error: {err}");

        // At/under capacity is accepted, and the realized total matches the declared amount within
        // f32 rounding (no capacity truncation).
        let mut law = ExoticEnergyLaw::mana_uniform(160.0);
        law.max_density = 10.0;
        let f =
            ExoticEnergyField::from_law(&law, 4, 4, 1).expect("exactly at capacity is feasible");
        assert!((f.total() - 160.0).abs() / 160.0 < 1e-5);
        assert!((f.initial - 160.0).abs() / 160.0 < 1e-5);

        // Patchy: a single hotspot concentrates MU, so a modest amount can already exceed a cell cap.
        let mut law = ExoticEnergyLaw::mana_patchy(500.0, 1);
        law.max_density = 1.0;
        assert!(ExoticEnergyField::from_law(&law, 8, 8, 5).is_err());
    }

    // ---- AE-S04 conservation ----------------------------------------------------------------

    #[test]
    fn closed_diffusion_conserves_mu() {
        // Pure diffusion (no source, no decay), closed boundary → total MU invariant.
        let mut law = ExoticEnergyLaw::mana_patchy(300.0, 6);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.2;
        law.boundary = BoundaryMode::Closed;
        let mut f = build(&law, 24, 24, 7);
        let total0 = f.total();
        for _ in 0..500 {
            f.step();
            assert!(
                (f.total() - total0).abs() / total0 < 1e-4,
                "closed diffusion drifted: {} vs {}",
                f.total(),
                total0
            );
        }
    }

    #[test]
    fn source_decay_diffusion_closes_the_mu_budget() {
        let law = ExoticEnergyLaw::mana_patchy(150.0, 4);
        let mut f = build(&law, 20, 20, 11);
        for _ in 0..800 {
            f.step();
            let b = f.budget(0.0);
            assert!(
                b.balance_error().abs() / b.throughput() < 1e-4,
                "MU budget drifted: error {}, throughput {}",
                b.balance_error(),
                b.throughput()
            );
        }
        // The source and decay actually moved MU.
        let b = f.budget(0.0);
        assert!(b.sourced > 0.0, "renewable source should inject MU");
        assert!(b.dissipated > 0.0, "decay should dissipate MU");
    }

    #[test]
    fn open_boundary_exports_mu_as_a_declared_sink() {
        let mut law = ExoticEnergyLaw::mana_uniform(200.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.2;
        law.boundary = BoundaryMode::Open;
        let mut f = build(&law, 12, 12, 3);
        for _ in 0..300 {
            f.step();
            let b = f.budget(0.0);
            assert!(
                b.balance_error().abs() / b.throughput() < 1e-4,
                "open budget must still close via `exported`"
            );
        }
        // Some MU genuinely left the grid.
        assert!(f.cum_exported > 0.0, "open boundary should export MU");
        assert!(f.total() < 200.0);
    }

    // ---- AE-205 transactions ----------------------------------------------------------------

    #[test]
    fn uptake_and_spend_conserve_mu_end_to_end() {
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let mut storage = 0.0f64;
        let mut dissipated_via_spend = 0.0f64;

        // Uptake: field → storage, exactly conserving (field total + storage is invariant to the bit).
        let field_before = f.total();
        let moved = f.uptake(0, 5.0, &mut storage, 1000.0);
        assert!(moved > 0.0);
        assert_eq!(
            f.total() + storage,
            field_before,
            "uptake must conserve field+storage exactly"
        );

        // Spend: storage → dissipated, exactly conserving.
        let stored_before = storage;
        let spent = spend_storage(&mut storage, 2.0, &mut dissipated_via_spend);
        assert!((spent - 2.0).abs() < 1e-9);
        assert!((storage + dissipated_via_spend - stored_before).abs() < 1e-9);

        // Full ledger closes: initial == field + storage + spent-dissipated.
        let b = f.budget(storage);
        let closed = b.initial - (b.field + b.organism_storage) - dissipated_via_spend;
        assert!(
            closed.abs() < 1e-9,
            "end-to-end MU ledger drifted: {closed}"
        );
    }

    #[test]
    fn fractional_and_repeated_uptake_is_ledger_exact() {
        // Awkward, non-representable request amounts drawn repeatedly from the same cell. Storage must
        // gain EXACTLY the field's total decrease (the f32 field delta), never the f64 request — so
        // the running invariant `field_delta == storage` holds to the field's f32 granularity.
        let mut law = ExoticEnergyLaw::mana_uniform(120.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        law.max_density = 100.0; // one cell holds plenty
        let mut f = build(&law, 4, 4, 1);
        let mut storage = 0.0f64;

        let field_before = f.total();
        // Repeated withdrawals of an awkward, non-f32-representable fraction from cell 5.
        for _ in 0..37 {
            f.uptake(5, 0.111_357_9, &mut storage, 1_000_000.0);
        }
        // Documented tolerance: the field is f32, so its total is represented to ~1e-6 relative; the
        // conservation invariant holds to that granularity (and much tighter than the f64 request sum
        // the buggy version would have credited).
        let field_delta = field_before - f.total();
        assert!(
            (field_delta - storage).abs() <= 1e-4,
            "uptake ledger drifted: field lost {field_delta}, storage gained {storage}"
        );
        // The uptake returns the ACTUAL field delta, not the request: 37 × 0.1113579 ≈ 4.12, but the
        // f32 field can only shed representable amounts, so storage ≈ field_delta (proven above), and
        // storage never exceeds the field's real decrease.
        assert!(storage <= field_delta + 1e-9);
    }

    // ---- AE-209 (M1): exotic-source intervention schema -------------------------------------

    fn forcing(id: u32, kind: ExoticInterventionKind, start: u64, dur: u64) -> ExoticIntervention {
        ExoticIntervention {
            id,
            cause_id: 900 + id,
            kind,
            region: Region::Global,
            start_tick: start,
            duration_ticks: dur,
            amount: 1.0,
            curve: Curve::Step,
        }
    }

    #[test]
    fn ae209_m1_forcing_validates_structurally() {
        // Well-formed forcings of each kind pass.
        for kind in [
            ExoticInterventionKind::AddSource,
            ExoticInterventionKind::RemoveSource,
            ExoticInterventionKind::Pulse,
        ] {
            assert!(forcing(1, kind, 10, 100).validate(6000).is_ok(), "{kind:?}");
        }

        // Non-finite / negative amount is rejected (magnitude only; direction is the `kind`).
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
        c.amount = f32::NAN;
        assert!(c.validate(6000).is_err());
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
        c.amount = -1.0;
        assert!(c.validate(6000).is_err());
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
        c.amount = f32::INFINITY;
        assert!(c.validate(6000).is_err());

        // Invalid geometry.
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
        c.region = Region::Radius {
            cx: 0.0,
            cy: 0.0,
            r: -1.0,
        };
        assert!(c.validate(6000).is_err());
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
        c.region = Region::Rect {
            min_x: 9,
            min_y: 0,
            max_x: 1,
            max_y: 1,
        };
        assert!(c.validate(6000).is_err());

        // Overflowing window and a window that can never fire in 1..=run_ticks.
        assert!(
            forcing(1, ExoticInterventionKind::Pulse, u64::MAX - 1, u64::MAX)
                .validate(6000)
                .is_err()
        );
        assert!(forcing(1, ExoticInterventionKind::Pulse, 900_000, 10)
            .validate(6000)
            .is_err());
    }

    #[test]
    fn ae209_m1_queue_is_deterministic_and_rejects_duplicate_ids() {
        // Windows must straddle an ecology firing tick (60) to be valid — see the D3 cadence rule.
        let a = forcing(2, ExoticInterventionKind::AddSource, 30, 50); // [30, 80) ∋ 60
        let b = forcing(1, ExoticInterventionKind::Pulse, 30, 50);
        // Constructed out of order; the queue yields a stable (start_tick, id) order.
        let q =
            ExoticInterventionQueue::new(vec![a.clone(), b.clone()], 6000).expect("valid queue");
        let ids: Vec<u32> = q.active_at(60).map(|c| c.id).collect();
        assert_eq!(ids, vec![1, 2]);
        assert!(q.active_at(29).next().is_none());
        assert!(q.active_at(80).next().is_none());

        // Duplicate ids are rejected structurally.
        let dup = ExoticInterventionQueue::new(vec![a.clone(), a.clone()], 6000);
        assert!(dup.is_err());

        // An invalid member is rejected by the queue constructor too.
        let mut bad = b.clone();
        bad.amount = f32::NAN;
        assert!(ExoticInterventionQueue::new(vec![bad], 6000).is_err());
    }

    #[test]
    fn ae209_m1_forcing_serde_round_trips() {
        let c = forcing(7, ExoticInterventionKind::RemoveSource, 100, 500);
        let json = serde_json::to_string(&c).expect("serializes");
        let back: ExoticIntervention = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, c);
    }

    // ---- AE-209 (M2): budgeted field forcing ------------------------------------------------

    #[test]
    fn ae209_m2_add_source_forcing_injects_and_books_mu() {
        let mut law = ExoticEnergyLaw::mana_uniform(50.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 8, 8, 1);
        let before_total = f.total();
        let before_sourced = f.cum_sourced;

        let c = forcing(1, ExoticInterventionKind::AddSource, 10, 1);
        let moved = f.apply_forcing(&c, 10);

        assert!(moved > 0.0, "an AddSource forcing must inject MU");
        // The field gained exactly what was booked as sourced.
        assert!((f.total() - before_total - moved).abs() < 1e-6);
        assert!((f.cum_sourced - before_sourced - moved).abs() < 1e-9);
        // The MU ledger still closes.
        let b = f.budget(0.0);
        assert!(b.balance_error().abs() / b.throughput() < 1e-4);
    }

    // NOTE: the two former tests here — `ae209_m2_remove_source_forcing_withdraws_and_books_as_
    // dissipated` and `ae209_m2_removal_cannot_drive_density_negative_and_stays_closed` — encoded the
    // ORIGINAL (defective) drain semantics, in which `RemoveSource` debited stored MU and credited
    // `cum_dissipated`. That contract was corrected in the audit pass: `RemoveSource` now suppresses
    // the base renewable source and never touches stored MU. Those tests are superseded by the `d2_*`
    // suite above and were deleted rather than adjusted, so the old behaviour cannot be re-asserted.

    #[test]
    fn ae209_m2_forcing_is_region_scoped_and_bounded_by_max_density() {
        let mut law = ExoticEnergyLaw::mana_uniform(16.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        law.max_density = 2.0;
        let mut f = build(&law, 4, 4, 1);

        // Region-scoped: only cell (0,0) may change.
        let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 1);
        c.region = Region::Cell { x: 0, y: 0 };
        c.amount = 1_000.0; // hammer the ceiling
        let before = f.density.clone();
        let moved = f.apply_forcing(&c, 10);

        for (i, (&now, &was)) in f.density.iter().zip(before.iter()).enumerate() {
            if i == 0 {
                assert!(now > was, "the targeted cell must gain");
            } else {
                assert_eq!(
                    now, was,
                    "cell {i} is outside the region and must not change"
                );
            }
        }
        // Bounded by max_density, and only the ACTUAL increase is booked.
        assert!(f.density[0] <= law.max_density + 1e-6);
        assert!((f.cum_sourced - moved).abs() < 1e-9);
        let b = f.budget(0.0);
        assert!(b.balance_error().abs() / b.throughput() < 1e-4);
    }

    #[test]
    fn ae209_m2_inactive_or_zero_forcing_is_a_noop() {
        let mut law = ExoticEnergyLaw::mana_uniform(20.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let before = f.density.clone();
        let before_ledger = (f.cum_sourced, f.cum_dissipated);

        let c = forcing(1, ExoticInterventionKind::AddSource, 100, 10);
        // Outside its window.
        assert_eq!(f.apply_forcing(&c, 5), 0.0);
        // Zero magnitude inside the window.
        let mut zero = forcing(2, ExoticInterventionKind::AddSource, 100, 10);
        zero.amount = 0.0;
        assert_eq!(f.apply_forcing(&zero, 100), 0.0);

        assert_eq!(f.density, before);
        assert_eq!((f.cum_sourced, f.cum_dissipated), before_ledger);
    }

    #[test]
    fn ae209_m2_pulse_curve_shapes_the_injection_over_its_window() {
        let mut law = ExoticEnergyLaw::mana_uniform(10.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        law.max_density = 1e6; // never clamp, so we measure the curve itself
        let mut f = build(&law, 4, 4, 1);

        let mut c = forcing(1, ExoticInterventionKind::Pulse, 10, 5);
        c.curve = Curve::Triangle;
        c.amount = 4.0;
        // Triangle peaks mid-window and is ~0 at the edges.
        let at_start = c.amount_at(10);
        let at_mid = c.amount_at(12);
        assert!(at_mid > at_start, "a triangle pulse peaks in the middle");

        let injected_start = f.apply_forcing(&c, 10);
        let injected_mid = f.apply_forcing(&c, 12);
        assert!(injected_mid > injected_start);
        let b = f.budget(0.0);
        assert!(b.balance_error().abs() / b.throughput() < 1e-4);
    }

    // ---- AUDIT D3: cadence + spatial validation ----------------------------------------------

    #[test]
    fn d3_forcing_window_must_contain_an_ecology_firing_tick() {
        use crate::core::sim_clock::ECOLOGY_PERIOD;
        assert_eq!(ECOLOGY_PERIOD, 60, "this test encodes the ecology cadence");

        // A one-tick pulse at a NON-ecology tick can never apply — exotic dynamics only fire on the
        // ecology band — so it must be rejected rather than silently ignored.
        let c = forcing(1, ExoticInterventionKind::Pulse, 61, 1);
        assert!(
            c.validate(6000).is_err(),
            "a 1-tick pulse off the ecology band must be rejected"
        );

        // Exactly on an ecology tick is accepted.
        assert!(forcing(1, ExoticInterventionKind::Pulse, 60, 1)
            .validate(6000)
            .is_ok());

        // A window that straddles an ecology tick is accepted...
        assert!(forcing(1, ExoticInterventionKind::AddSource, 59, 2)
            .validate(6000)
            .is_ok());
        // ...but a window that sits strictly between two firings is not.
        assert!(forcing(1, ExoticInterventionKind::AddSource, 61, 59)
            .validate(6000)
            .is_err());
        // [61, 121) contains 120 → accepted (boundary exactness).
        assert!(forcing(1, ExoticInterventionKind::AddSource, 61, 60)
            .validate(6000)
            .is_ok());

        // A window whose only ecology tick lies beyond the run's duration is rejected: [5990, 6010)
        // fires at 6000, which a 5999-tick run never reaches.
        assert!(forcing(1, ExoticInterventionKind::AddSource, 5_990, 20)
            .validate(5_999)
            .is_err());
        // The same window IS valid in a 6000-tick run, where 6000 is the last firing.
        assert!(forcing(1, ExoticInterventionKind::AddSource, 5_990, 20)
            .validate(6_000)
            .is_ok());
        assert!(forcing(1, ExoticInterventionKind::AddSource, 6_000, 1)
            .validate(6_000)
            .is_ok());
    }

    #[test]
    fn d3_grid_applicability_is_validated_against_the_field() {
        let law = ExoticEnergyLaw::mana_uniform(10.0);
        let f = build(&law, 16, 16, 1);

        // In-grid Cell / Rect are fine.
        let mut ok = forcing(1, ExoticInterventionKind::AddSource, 60, 1);
        ok.region = Region::Cell { x: 15, y: 15 };
        assert!(f.validate_region_applicable(&ok).is_ok());
        ok.region = Region::Rect {
            min_x: 0,
            min_y: 0,
            max_x: 15,
            max_y: 15,
        };
        assert!(f.validate_region_applicable(&ok).is_ok());

        // Out-of-grid Cell / Rect are rejected structurally. Rect must be fully contained, not merely
        // overlap the field.
        let mut bad = forcing(2, ExoticInterventionKind::AddSource, 60, 1);
        bad.region = Region::Cell { x: 16, y: 0 };
        assert!(f.validate_region_applicable(&bad).is_err());
        bad.region = Region::Rect {
            min_x: 16,
            min_y: 16,
            max_x: 20,
            max_y: 20,
        };
        assert!(f.validate_region_applicable(&bad).is_err());
        bad.region = Region::Rect {
            min_x: 15,
            min_y: 15,
            max_x: 16,
            max_y: 16,
        };
        assert!(
            f.validate_region_applicable(&bad).is_err(),
            "a partially overflowing Rect must not be silently clipped"
        );

        // A Radius centred off-grid is rejected; one centred in-grid is accepted even if the disc
        // crosses an edge (documented: it is clipped to the grid).
        let mut r = forcing(3, ExoticInterventionKind::AddSource, 60, 1);
        r.region = Region::Radius {
            cx: 100.0,
            cy: 100.0,
            r: 2.0,
        };
        assert!(f.validate_region_applicable(&r).is_err());
        r.region = Region::Radius {
            cx: 0.0,
            cy: 0.0,
            r: 5.0,
        };
        assert!(
            f.validate_region_applicable(&r).is_ok(),
            "an in-grid centre whose disc overhangs the edge is clipped, not rejected"
        );

        // Global always applies.
        let g = forcing(4, ExoticInterventionKind::AddSource, 60, 1);
        assert!(f.validate_region_applicable(&g).is_ok());
    }

    // ---- AUDIT D2: RemoveSource suppresses the SOURCE, it does not drain the field ------------

    #[test]
    fn d2_remove_source_never_drains_existing_mu_when_there_is_no_source() {
        // With source_rate = 0 there is nothing to suppress, so a RemoveSource forcing must be a
        // complete no-op on stored MU. (The old semantics drained the field — a sink, not a source
        // removal.)
        let mut law = ExoticEnergyLaw::mana_uniform(80.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 8, 8, 1);
        let before = f.density.clone();
        let before_total = f.total();

        let c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
        f.begin_tick_suppression();
        let effective = f.add_source_suppression(&c, 60);
        f.step();

        assert_eq!(effective, 0.0, "nothing to suppress when source_rate == 0");
        assert_eq!(f.density, before, "existing MU must NOT be drained");
        assert_eq!(f.total(), before_total);
        assert_eq!(f.cum_dissipated, 0.0, "removal must not credit dissipation");
        assert_eq!(f.cum_source_suppressed, 0.0);
    }

    #[test]
    fn d2_remove_source_only_avoids_source_injection_and_lowers_cum_sourced() {
        // With a live source, the ONLY difference a RemoveSource makes is the avoided injection.
        let mut law = ExoticEnergyLaw::mana_uniform(20.0);
        law.source_rate = 0.4;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        law.max_density = 1e6; // never clamp, so the comparison is exact

        // Unsuppressed reference tick.
        let mut plain = build(&law, 4, 4, 1);
        plain.begin_tick_suppression();
        plain.step();

        // Same tick with a full suppression.
        let mut supp = build(&law, 4, 4, 1);
        let mut c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
        c.amount = 0.4; // exactly the per-cell source contribution
        supp.begin_tick_suppression();
        let effective = supp.add_source_suppression(&c, 60);
        supp.step();

        let cells = (4 * 4) as f64;
        assert!(
            (effective - 0.4 * cells).abs() < 1e-4,
            "effective = {effective}"
        );
        // The suppressed field gained nothing this tick; the plain one gained the full source.
        assert!((plain.cum_sourced - 0.4 * cells).abs() < 1e-4);
        assert_eq!(supp.cum_sourced, 0.0, "suppression lowers cum_sourced");
        assert_eq!(supp.cum_dissipated, 0.0, "and never credits dissipation");
        assert!((supp.cum_source_suppressed - 0.4 * cells).abs() < 1e-4);
        // Existing MU is untouched: the suppressed field still holds its initial amount.
        assert!((supp.total() - 20.0).abs() < 1e-3);
    }

    #[test]
    fn d2_suppression_is_capped_at_the_source_contribution() {
        // A removal far larger than the source can only cancel the source — it can never go further
        // and start eating stored MU.
        let mut law = ExoticEnergyLaw::mana_uniform(30.0);
        law.source_rate = 0.1;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let before_total = f.total();

        let mut c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
        c.amount = 1_000_000.0;
        f.begin_tick_suppression();
        let effective = f.add_source_suppression(&c, 60);
        f.step();

        let cells = 16.0;
        assert!(
            (effective - 0.1 * cells).abs() < 1e-4,
            "capped at the source contribution, got {effective}"
        );
        assert_eq!(f.cum_sourced, 0.0);
        assert_eq!(f.cum_dissipated, 0.0);
        assert!(
            (f.total() - before_total).abs() < 1e-4,
            "stored MU is untouched by an over-large suppression"
        );

        // A saturated field has no realizable renewable contribution. Suppression must therefore be
        // zero rather than claiming that MU "would have entered" a cell with no headroom.
        let mut saturated_law = ExoticEnergyLaw::mana_uniform(160.0);
        saturated_law.source_rate = 0.1;
        saturated_law.decay_rate = 0.0;
        saturated_law.diffusion_rate = 0.0;
        let mut saturated = build(&saturated_law, 4, 4, 1);
        saturated.begin_tick_suppression();
        assert_eq!(saturated.add_source_suppression(&c, 60), 0.0);
        saturated.step();
        assert_eq!(saturated.cum_sourced, 0.0);
        assert_eq!(saturated.cum_source_suppressed, 0.0);
    }

    #[test]
    fn d2_overlapping_suppressions_are_deterministic_and_jointly_capped() {
        let mut law = ExoticEnergyLaw::mana_uniform(10.0);
        law.source_rate = 0.3;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);

        let mut a = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
        a.amount = 0.2;
        let mut b = forcing(2, ExoticInterventionKind::RemoveSource, 60, 1);
        b.amount = 0.2; // together 0.4 > the 0.3 source

        f.begin_tick_suppression();
        let ea = f.add_source_suppression(&a, 60);
        let eb = f.add_source_suppression(&b, 60);
        f.step();

        let cells = 16.0;
        // Each command is credited only what it actually contributed; the total is capped.
        assert!((ea - 0.2 * cells).abs() < 1e-4, "first gets its full share");
        assert!((eb - 0.1 * cells).abs() < 1e-4, "second only the remainder");
        assert!(
            (ea + eb - 0.3 * cells).abs() < 1e-4,
            "jointly capped at the source"
        );
        assert_eq!(f.cum_sourced, 0.0);
        assert!((f.cum_source_suppressed - 0.3 * cells).abs() < 1e-4);
    }

    #[test]
    fn d2_add_and_pulse_remain_external_injections() {
        // Add/Pulse are unchanged: they inject MU and credit cum_sourced.
        let mut law = ExoticEnergyLaw::mana_uniform(10.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let before = f.total();
        let c = forcing(1, ExoticInterventionKind::AddSource, 60, 1);
        let moved = f.apply_forcing(&c, 60);
        assert!(moved > 0.0);
        assert!((f.total() - before - moved).abs() < 1e-6);
        assert!((f.cum_sourced - moved).abs() < 1e-9);
        assert_eq!(f.cum_dissipated, 0.0);
    }

    #[test]
    fn d2_apply_forcing_rejects_remove_source() {
        // RemoveSource is no longer an `apply_forcing` movement; routing it there must be a no-op so
        // the drain semantics cannot be reintroduced by accident.
        let mut law = ExoticEnergyLaw::mana_uniform(10.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let before = f.density.clone();
        let c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
        assert_eq!(f.apply_forcing(&c, 60), 0.0);
        assert_eq!(f.density, before);
    }

    // ---- DEFECT P3: transaction helpers reject non-finite / negative inputs ------------------

    #[test]
    fn p3_uptake_rejects_non_finite_and_negative_inputs_without_mutating() {
        let mut law = ExoticEnergyLaw::mana_uniform(100.0);
        law.source_rate = 0.0;
        law.decay_rate = 0.0;
        law.diffusion_rate = 0.0;
        let mut f = build(&law, 4, 4, 1);
        let before_field = f.density.clone();
        let before_total = f.total();

        // A NaN request must NOT transfer anything. (`f64::min` returns the NON-NaN operand, so a
        // naive `requested <= 0.0` guard lets NaN through and moves real MU.)
        let mut storage = 0.0f64;
        assert_eq!(f.uptake(0, f64::NAN, &mut storage, 1000.0), 0.0);
        assert_eq!(storage, 0.0, "NaN request must not credit storage");
        assert_eq!(
            f.density, before_field,
            "NaN request must not touch the field"
        );

        // Infinite request.
        assert_eq!(f.uptake(0, f64::INFINITY, &mut storage, 1000.0), 0.0);
        assert_eq!(storage, 0.0);
        assert_eq!(f.density, before_field);

        // Negative request.
        assert_eq!(f.uptake(0, -5.0, &mut storage, 1000.0), 0.0);
        assert_eq!(f.density, before_field);

        // Non-finite / negative capacity.
        for cap in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(f.uptake(0, 1.0, &mut storage, cap), 0.0, "capacity {cap}");
            assert_eq!(f.density, before_field);
            assert_eq!(storage, 0.0);
        }

        // A corrupt CURRENT storage value must not be used to compute a transfer.
        let mut bad_storage = f64::NAN;
        assert_eq!(f.uptake(0, 1.0, &mut bad_storage, 1000.0), 0.0);
        assert!(bad_storage.is_nan(), "corrupt storage is left untouched");
        assert_eq!(f.density, before_field);

        // The field total is completely unchanged by every rejected transaction.
        assert_eq!(f.total(), before_total);

        // A valid request still works.
        let mut ok_storage = 0.0f64;
        assert!(f.uptake(0, 1.0, &mut ok_storage, 1000.0) > 0.0);
    }

    #[test]
    fn p3_spend_storage_rejects_non_finite_and_negative_inputs_without_mutating() {
        // A NaN amount must not drain storage (naive `amount <= 0.0` lets NaN through, and
        // `NaN.min(storage)` yields `storage` — spending the whole balance).
        let mut storage = 10.0f64;
        let mut dissipated = 0.0f64;
        assert_eq!(spend_storage(&mut storage, f64::NAN, &mut dissipated), 0.0);
        assert_eq!(storage, 10.0, "NaN amount must not drain storage");
        assert_eq!(dissipated, 0.0, "NaN amount must not credit the sink");

        // Infinite amount.
        assert_eq!(
            spend_storage(&mut storage, f64::INFINITY, &mut dissipated),
            0.0
        );
        assert_eq!(storage, 10.0);
        assert_eq!(dissipated, 0.0);

        // Negative amount.
        assert_eq!(spend_storage(&mut storage, -1.0, &mut dissipated), 0.0);
        assert_eq!(storage, 10.0);
        assert_eq!(dissipated, 0.0);

        // A corrupt CURRENT storage or dissipated ledger must not be spent from / credited into.
        let mut bad_storage = f64::NAN;
        let mut ok_sink = 0.0f64;
        assert_eq!(spend_storage(&mut bad_storage, 1.0, &mut ok_sink), 0.0);
        assert!(bad_storage.is_nan());
        assert_eq!(ok_sink, 0.0);

        let mut ok_storage = 10.0f64;
        let mut bad_sink = f64::NAN;
        assert_eq!(spend_storage(&mut ok_storage, 1.0, &mut bad_sink), 0.0);
        assert_eq!(
            ok_storage, 10.0,
            "storage must not be debited into a corrupt sink"
        );
        assert!(bad_sink.is_nan());

        // A valid spend still works and conserves.
        let mut s = 10.0f64;
        let mut d = 0.0f64;
        assert_eq!(spend_storage(&mut s, 4.0, &mut d), 4.0);
        assert_eq!(s, 6.0);
        assert_eq!(d, 4.0);
    }

    #[test]
    fn deliberate_leak_is_detected_by_the_budget() {
        let law = ExoticEnergyLaw::mana_uniform(100.0);
        let mut f = build(&law, 4, 4, 1);
        // Corrupt the field WITHOUT booking the loss — a leak the audit must catch.
        f.density[0] -= 1e-2;
        let b = f.budget(0.0);
        assert!(
            b.balance_error().abs() > 1e-3,
            "a 1e-2 unbooked leak must be visible in the budget error"
        );
    }
}
