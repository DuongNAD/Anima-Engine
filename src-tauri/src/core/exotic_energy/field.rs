//! The exotic-energy field (AE-203): the grid that sources fill and organisms draw from.

use super::*;

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
pub(super) fn fnv1a_64_str(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in s.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}
