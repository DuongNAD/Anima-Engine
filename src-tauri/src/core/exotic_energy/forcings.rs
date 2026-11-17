//! Runtime source forcings (AE-209): declared, time-bounded changes to an exotic source, each with
//! its `CauseId`.

use super::*;

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
