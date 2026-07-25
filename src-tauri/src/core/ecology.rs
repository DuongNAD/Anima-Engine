//! Ecosystem dynamics primitives — the ecological "physics" of the world.
//!
//! Grounded in the standard population/energy-flow literature so most rates EMERGE from a few
//! physical parameters instead of being hand-tuned (see the project's ecology reference):
//!   - Metabolic Theory of Ecology (Brown et al. 2004): metabolic rate scales as
//!     `M^(3/4) · e^(−E/kT)` (Kleiber's law × an Arrhenius temperature term), so body mass is a
//!     single gene that sets metabolism, and warmth speeds every rate.
//!   - Holling (1959) functional responses: predation SATURATES (Type II) and, crucially,
//!     switches AWAY from rare prey (Type III) — the strongest single guard against prey
//!     extinction / boom-bust collapse.
//!   - Lindeman (1942): only ~10% of energy crosses each trophic link; the rest is lost. Here
//!     the "lost" fraction is recycled into a CLOSED biomass pool (the Bibites anti-collapse
//!     device), so total energy is conserved.
//!   - Whittaker (1975) NPP: per-biome net primary productivity sets each cell's carrying
//!     capacity for a logistic regrowth field.
//!   - Shannon / Simpson indices: emergent biodiversity diagnostics.
//!
//! Everything here is pure and allocation-free on the hot path (`ResourceField::step_regrowth`
//! mutates its pre-allocated buffers in place), so it is safe inside the zero-alloc tick loop.

use crate::core::terrain::BiomeType;

// ---- Metabolic Theory of Ecology ------------------------------------------------------

/// Boltzmann constant in eV/K.
pub const BOLTZMANN_EV: f32 = 8.617_333e-5;
/// Reference temperature (20 °C) at which the Arrhenius factor is 1.0.
pub const REF_TEMP_K: f32 = 293.15;
/// Activation energy of animal metabolism (Brown et al. 2004: 0.60–0.70 eV, mean ~0.65).
pub const E_ANIMAL_EV: f32 = 0.65;
/// Activation energy of photosynthesis — a weaker temperature dependence (~0.30 eV).
pub const E_PRODUCER_EV: f32 = 0.30;
/// Normalization so a reference animal's maintenance cost matches the legacy linear model.
pub const METABOLIC_NORM: f32 = 0.06;
/// Kleiber allometric exponent (mass^3/4). Exposed as a constant because it is contested for
/// some taxa (field data give ~2/3); 3/4 is the best single default.
pub const KLEIBER_EXP: f32 = 0.75;

/// Arrhenius/Boltzmann temperature factor, normalized to 1.0 at `REF_TEMP_K`.
/// `temp_c` is the organism temperature in °C. Warmer → faster metabolism (factor > 1).
#[inline]
pub fn boltzmann_factor(temp_c: f32, activation_ev: f32) -> f32 {
    let t_k = temp_c + 273.15;
    if t_k <= 0.0 {
        return 0.0;
    }
    ((activation_ev / BOLTZMANN_EV) * (1.0 / REF_TEMP_K - 1.0 / t_k)).exp()
}

/// Whole-organism maintenance metabolic rate: `i0 · M^(3/4) · e^(−E/kT)` (per Metabolic
/// Theory of Ecology). `mass` is total body mass, `temp_c` the body temperature in °C.
#[inline]
pub fn metabolic_rate(mass: f32, temp_c: f32, activation_ev: f32) -> f32 {
    let m = mass.max(0.0);
    METABOLIC_NORM * m.powf(KLEIBER_EXP) * boltzmann_factor(temp_c, activation_ev)
}

// ---- Whittaker NPP → per-cell carrying capacity ---------------------------------------

/// Net primary productivity per biome (Whittaker 1975 canonical dry-matter means,
/// g/m²/yr; aquatic/barren cells use representative values). Sets the resource ceiling.
#[inline]
pub fn biome_npp(biome: BiomeType) -> f32 {
    match biome {
        BiomeType::Rainforest => 2200.0,
        BiomeType::TemperateForest => 1200.0,
        BiomeType::BorealForest => 800.0,
        BiomeType::Grassland => 700.0,
        BiomeType::River => 500.0, // productive riparian margins
        BiomeType::Ocean => 250.0, // shelf phytoplankton
        BiomeType::Snow => 140.0,  // tundra-equivalent
        BiomeType::DeepOcean => 125.0,
        BiomeType::Desert => 90.0,
        BiomeType::Beach => 60.0,
        BiomeType::MountainRock => 50.0,
    }
}

/// Scale from NPP (g/m²/yr) to a per-cell carrying capacity in sim energy units. Kept
/// deliberately moderate — the paradox of enrichment says over-rich cells drive boom-bust.
pub const NPP_TO_CAPACITY: f32 = 0.01;

/// Per-cell resource carrying capacity `R_max` derived from the cell's biome.
#[inline]
pub fn biome_carrying_capacity(biome: BiomeType) -> f32 {
    biome_npp(biome) * NPP_TO_CAPACITY
}

/// Discrete logistic regrowth of a resource toward its ceiling: `R + g·R·(1 − R/R_max)`.
/// A tiny seed keeps a fully-grazed cell (R = 0) from being permanently dead.
#[inline]
pub fn logistic_regrowth(r: f32, g: f32, r_max: f32) -> f32 {
    if r_max <= 0.0 {
        return 0.0;
    }
    let seed = if r < 1e-4 { 1e-3 * r_max } else { 0.0 };
    let next = r + seed + g * r * (1.0 - r / r_max);
    next.clamp(0.0, r_max)
}

// ---- Holling functional responses -----------------------------------------------------

/// Type II (hyperbolic disc equation): `a·N / (1 + a·h·N)`. Saturates at 1/h; destabilizing
/// at low prey density.
#[inline]
pub fn holling_type2(prey: f32, attack: f32, handling: f32) -> f32 {
    let n = prey.max(0.0);
    (attack * n) / (1.0 + attack * handling * n)
}

/// Type III (sigmoid): `a·N² / (1 + a·h·N²)`. Consumption is near-zero at low prey density
/// (predator switching / search image), so rare prey are protected — the recommended default
/// for anything that can go rare.
#[inline]
pub fn holling_type3(prey: f32, attack: f32, handling: f32) -> f32 {
    let n = prey.max(0.0);
    let n2 = n * n;
    (attack * n2) / (1.0 + attack * handling * n2)
}

/// Lindeman trophic transfer (assimilation) efficiency — the fraction of captured prey energy
/// that becomes predator energy. ~10% canonical; the remaining (1 − e) is recycled as detritus.
pub const LINDEMAN_EFFICIENCY: f32 = 0.30;

/// Herbivore grazing intake for one tick — a saturating (Type II) bite bounded by the standing
/// plant resource, how hungry the animal is, and a maximum bite rate. Grazing a depleted cell
/// yields little, which naturally disperses herbivores (giving-up density → moving refuges).
#[inline]
pub fn herbivore_intake(available: f32, hunger: f32, max_bite: f32) -> f32 {
    if available <= 0.0 || hunger <= 0.0 || max_bite <= 0.0 {
        return 0.0;
    }
    // Type II saturation on the standing resource, then clamp to hunger and the bite ceiling.
    let saturating = holling_type2(available, 1.0, 1.0 / max_bite.max(1e-3));
    saturating.min(available).min(hunger).min(max_bite)
}

/// Type III capture attack rate and handling time for one predator–prey contact.
pub const CAPTURE_ATTACK: f32 = 0.02;
pub const CAPTURE_HANDLING: f32 = 0.02;

/// Energy a predator captures from a prey in one contact. A Type III response means a healthy,
/// energetic prey is NOT drained to zero in a single strike, and a rare/weak (low-energy) prey
/// is barely touched — the rarity refuge that stops prey extinction. Bounded above by the prey's
/// energy and by how much the predator can actually use (its deficit ÷ assimilation efficiency,
/// so it never wastefully over-kills).
#[inline]
pub fn predation_capture(prey_energy: f32, predator_deficit: f32) -> f32 {
    if prey_energy <= 0.0 || predator_deficit <= 0.0 {
        return 0.0;
    }
    let raw = holling_type3(prey_energy, CAPTURE_ATTACK, CAPTURE_HANDLING);
    raw.min(prey_energy)
        .min(predator_deficit / LINDEMAN_EFFICIENCY.max(1e-3))
}

// ---- Ecological niche descriptors (MAP-Elites behavior space) --------------------------

/// Reference scales that normalize raw ecological traits into a comparable [0, 1] MAP-Elites
/// behavior space, so a fixed grid resolution bins both axes sensibly.
pub const MASS_REFERENCE: f32 = 20.0;
pub const FORAGING_REFERENCE: f32 = 200.0;

/// Two ecological niche axes for MAP-Elites (Hutchinson's hypervolume): body mass (the
/// Metabolic-Theory master trait) and foraging range (distance roamed = niche breadth), each
/// normalized to [0, 1]. Illuminating this space rewards ECOLOGICAL diversity — small roamers
/// vs large homebodies, generalists vs specialists — instead of a single locomotion optimum,
/// and lets predator/prey arms races (Red Queen) spread the archive rather than collapse it.
#[inline]
pub fn ecological_descriptors(body_mass: f32, foraging_range: f32) -> [f32; 2] {
    [
        (body_mass / MASS_REFERENCE).clamp(0.0, 1.0),
        (foraging_range / FORAGING_REFERENCE).clamp(0.0, 1.0),
    ]
}

// ---- Seasonal fertility driver --------------------------------------------------------

/// Amplitude of the seasonal fertility swing around the mean of 1.0.
pub const SEASON_AMPLITUDE: f32 = 0.5;

/// Global fertility multiplier as a slow sine over the season phase (radians): summer boosts
/// plant regrowth, winter suppresses it. The resulting boom-and-bust in the resource field is
/// exactly the kind of periodic disturbance that keeps predator-prey cycles going and (at
/// intermediate frequency) sustains diversity. Clamped at 0 so deep winter can pause growth.
#[inline]
pub fn seasonal_fertility(phase: f32) -> f32 {
    (1.0 + SEASON_AMPLITUDE * phase.sin()).max(0.0)
}

/// A slowly-advancing season phase (radians). One field, advanced by `rate·dt` each tick.
#[derive(Resource, Clone, Copy, Debug)]
pub struct SeasonClock {
    pub phase: f32,
    pub rate: f32,
}

impl Default for SeasonClock {
    fn default() -> Self {
        // ~one full year every 100 seconds of sim time at 60 FPS.
        Self {
            phase: 0.0,
            rate: std::f32::consts::TAU / 100.0,
        }
    }
}

impl SeasonClock {
    /// Advance the phase and return the current fertility multiplier.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.phase = (self.phase + self.rate * dt) % std::f32::consts::TAU;
        seasonal_fertility(self.phase)
    }
}

/// Character-displacement / Red-Queen signal: how far apart two guilds (e.g. prey vs
/// predators) sit on a shared trait axis, normalized to [0, 1] by the same reference the niche
/// space uses. A rising value means the guilds are diverging morphologically — the coevolution
/// arms race pushing them into separate niches instead of competing head-on.
#[inline]
pub fn niche_divergence(guild_a_mass: f32, guild_b_mass: f32) -> f32 {
    ((guild_a_mass - guild_b_mass).abs() / MASS_REFERENCE).clamp(0.0, 1.0)
}

// ---- Biodiversity diagnostics ---------------------------------------------------------

/// Shannon–Wiener index `H = −Σ pᵢ ln pᵢ` over species abundances. Higher = more diverse.
pub fn shannon_index(abundances: &[u32]) -> f32 {
    let total: u32 = abundances.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let total_f = total as f32;
    let mut h = 0.0;
    for &n in abundances {
        if n > 0 {
            let p = n as f32 / total_f;
            h -= p * p.ln();
        }
    }
    h
}

/// Gini–Simpson index `1 − Σ pᵢ²` — the probability two random individuals are different
/// species. 0 = monoculture, → 1 = highly even/diverse.
pub fn simpson_index(abundances: &[u32]) -> f32 {
    let total: u32 = abundances.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let total_f = total as f32;
    let sum_sq: f32 = abundances
        .iter()
        .map(|&n| {
            let p = n as f32 / total_f;
            p * p
        })
        .sum();
    1.0 - sum_sq
}

// ---- Per-cell NPP resource field (SoA, zero-alloc regrowth) ----------------------------

use bevy_ecs::prelude::Resource;

/// A grid of standing plant resource, one cell per terrain cell, regrowing logistically toward
/// its biome-set ceiling. Stored SoA in pre-allocated buffers so `step_regrowth` never
/// allocates. Grazing subtracts from `r`; the world→cell mapping matches the terrain grid.
#[derive(Resource, Clone, Debug)]
pub struct ResourceField {
    pub width: usize,
    pub height: usize,
    /// World-space bounds the grid spans (X/Z), mirroring `MapBounds`.
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
    /// Current standing resource per cell.
    pub r: Vec<f32>,
    /// Carrying capacity per cell (from biome NPP).
    pub r_max: Vec<f32>,
    /// Intrinsic regrowth rate.
    pub growth_rate: f32,
}

impl ResourceField {
    /// Build from a terrain biome grid, starting each cell full at its capacity.
    pub fn from_biomes(
        biomes: &[u8],
        width: usize,
        height: usize,
        min_x: f32,
        min_z: f32,
        max_x: f32,
        max_z: f32,
        growth_rate: f32,
    ) -> Self {
        let n = width * height;
        let mut r_max = vec![0.0f32; n];
        for i in 0..n.min(biomes.len()) {
            let biome = biome_from_u8(biomes[i]);
            r_max[i] = biome_carrying_capacity(biome);
        }
        let r = r_max.clone();
        Self {
            width,
            height,
            min_x,
            min_z,
            max_x,
            max_z,
            r,
            r_max,
            growth_rate,
        }
    }

    /// Advance every cell one logistic step, scaled by `dt`. Allocation-free (in-place).
    /// `fertility` is the global seasonal fertility multiplier (0 pauses growth entirely).
    pub fn step_regrowth(&mut self, dt: f32, fertility: f32) {
        let g = self.growth_rate * dt * fertility.max(0.0);
        for i in 0..self.r.len() {
            self.r[i] = logistic_regrowth(self.r[i], g, self.r_max[i]);
        }
    }

    /// How many ticks it takes to sweep the whole resource field once.
    ///
    /// Raising the world to 256² quadrupled the cell count, and the regrowth path was costing four
    /// full passes per tick (two inside the gated step, two more for the `total_biomass()` sandwich
    /// that measured what actually grew). Measured at ~4.2 ms/tick — a quarter of a 60 FPS frame
    /// budget for a background field.
    ///
    /// Striding turns that into two passes over a quarter of the cells, and the exact accounting
    /// now happens inside the loop so the sandwich is gone entirely: 4·N cell-visits become 0.5·N.
    /// The cost of resolution is paid back without giving the resolution up.
    pub const REGROWTH_STRIDE: usize = 4;

    /// Biomass-gated regrowth over every cell. See [`Self::step_regrowth_gated_strided`].
    pub fn step_regrowth_gated(&mut self, dt: f32, fertility: f32, budget: f64) -> f64 {
        self.step_regrowth_gated_strided(dt, fertility, budget, 0, 1)
    }

    /// Biomass-gated regrowth (the Bibites closed-system rule): plants can only grow by drawing
    /// free energy from the detritus `budget`. Returns the biomass **actually** added, which the
    /// caller subtracts from detritus, so total energy is conserved — plants cannot appear from
    /// nothing. Two in-place passes over the visited cells, allocation-free.
    ///
    /// ## Exact accounting, measured in the loop
    ///
    /// The returned figure is `new_cell - old_cell` read back from the stored `f32`, not the
    /// `delta * scale` that was asked for. Those differ: `cell += applied` rounds, and over a long
    /// run the difference is a one-way drift rather than noise — it showed up as a real EU sink in
    /// the G1.1 conservation gate. Measuring inside the loop gets the same exactness a
    /// `total_biomass()` sandwich around the call would, without the two extra full passes.
    ///
    /// ## Striding
    ///
    /// Only cells where `i % stride == phase` are visited, and the caller is expected to pass
    /// `dt * stride` so each cell still advances at the same rate — it is simply updated once every
    /// `stride` ticks instead of every tick. Regrowth is a slow logistic process, so one step of
    /// `n·dt` and `n` steps of `dt` agree to first order; the error is `O((n·dt)²)` against a rate
    /// of ~0.02/s, i.e. far below the conservation tolerance.
    ///
    /// One behavioural note: the detritus `budget` offered is the whole pool, and with a stride only
    /// a fraction of the cells compete for it on any given tick. That changes which cells win when
    /// detritus is scarce, but not how much is drawn in total — conservation is unaffected.
    pub fn step_regrowth_gated_strided(
        &mut self,
        dt: f32,
        fertility: f32,
        budget: f64,
        phase: usize,
        stride: usize,
    ) -> f64 {
        let stride = stride.max(1);
        let phase = if stride == 1 { 0 } else { phase % stride };
        let g = self.growth_rate * dt * fertility.max(0.0);
        if g <= 0.0 || budget <= 0.0 {
            return 0.0;
        }
        // Pass 1: total desired (positive) growth this step, over the visited cells.
        let mut desired = 0.0f64;
        let mut i = phase;
        while i < self.r.len() {
            let delta = logistic_regrowth(self.r[i], g, self.r_max[i]) - self.r[i];
            if delta > 0.0 {
                desired += delta as f64;
            }
            i += stride;
        }
        if desired <= 0.0 {
            return 0.0;
        }
        // Pass 2: apply, scaled down if the detritus budget cannot cover the full demand.
        let scale = (budget / desired).min(1.0) as f32;
        let mut grown = 0.0f64;
        let mut i = phase;
        while i < self.r.len() {
            let delta = logistic_regrowth(self.r[i], g, self.r_max[i]) - self.r[i];
            if delta > 0.0 {
                let before = self.r[i];
                self.r[i] += delta * scale;
                grown += (self.r[i] - before) as f64;
            }
            i += stride;
        }
        grown
    }

    /// Cell index for a world (x, z), or None if outside the grid.
    #[inline]
    pub fn cell_index(&self, x: f32, z: f32) -> Option<usize> {
        if self.width == 0
            || self.height == 0
            || self.max_x <= self.min_x
            || self.max_z <= self.min_z
        {
            return None;
        }
        if x < self.min_x || x > self.max_x || z < self.min_z || z > self.max_z {
            return None;
        }
        let u = (x - self.min_x) / (self.max_x - self.min_x);
        let v = (z - self.min_z) / (self.max_z - self.min_z);
        let col = ((u * self.width as f32) as usize).min(self.width - 1);
        let row = ((v * self.height as f32) as usize).min(self.height - 1);
        Some(row * self.width + col)
    }

    /// Graze up to `desired` energy from the cell at (x, z), returning what was actually taken
    /// (bounded by the standing resource). Mutates the field in place.
    pub fn graze(&mut self, x: f32, z: f32, desired: f32) -> f32 {
        if desired <= 0.0 {
            return 0.0;
        }
        if let Some(i) = self.cell_index(x, z) {
            let taken = desired.min(self.r[i]);
            self.r[i] -= taken;
            taken
        } else {
            0.0
        }
    }

    /// Total standing plant biomass across the field.
    pub fn total_biomass(&self) -> f64 {
        self.r.iter().map(|&v| v as f64).sum()
    }
}

/// Map a raw biome byte back to `BiomeType` (unknown values fall back to barren rock).
#[inline]
pub fn biome_from_u8(b: u8) -> BiomeType {
    match b {
        0 => BiomeType::DeepOcean,
        1 => BiomeType::Ocean,
        2 => BiomeType::Beach,
        3 => BiomeType::River,
        4 => BiomeType::Grassland,
        5 => BiomeType::TemperateForest,
        6 => BiomeType::BorealForest,
        7 => BiomeType::Rainforest,
        8 => BiomeType::Desert,
        9 => BiomeType::MountainRock,
        10 => BiomeType::Snow,
        _ => BiomeType::MountainRock,
    }
}

// ---- Closed biomass pool (conservation / anti-collapse) --------------------------------

// The closed-energy compartments moved to `anima-domain` (G2 task 1) — both engines must share one
// definition of the conserved ledger. Re-exported so every `ecology::EcosystemBiomass` path keeps
// working.
pub use anima_domain::energy::EcosystemBiomass;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boltzmann_is_unity_at_reference_and_rises_with_heat() {
        assert!((boltzmann_factor(20.0, E_ANIMAL_EV) - 1.0).abs() < 1e-4);
        assert!(boltzmann_factor(37.0, E_ANIMAL_EV) > 1.0); // body heat speeds metabolism
        assert!(boltzmann_factor(0.0, E_ANIMAL_EV) < 1.0); // cold slows it
    }

    #[test]
    fn q10_is_biologically_plausible() {
        // Q10: rate ratio per 10 °C. MTE with E≈0.65 eV predicts ~2.2–2.6 near room temp.
        let q10 = boltzmann_factor(30.0, E_ANIMAL_EV) / boltzmann_factor(20.0, E_ANIMAL_EV);
        assert!(q10 > 2.0 && q10 < 3.0, "Q10 was {q10}");
    }

    #[test]
    fn metabolism_scales_sublinearly_with_mass() {
        // Kleiber: doubling mass raises metabolism by 2^0.75 ≈ 1.68, NOT 2×.
        let m1 = metabolic_rate(4.0, 20.0, E_ANIMAL_EV);
        let m2 = metabolic_rate(8.0, 20.0, E_ANIMAL_EV);
        let ratio = m2 / m1;
        assert!(
            (ratio - 2.0f32.powf(0.75)).abs() < 1e-3,
            "ratio was {ratio}"
        );
        // A big animal spends less energy PER GRAM than a small one (mass-specific ∝ M^−1/4).
        assert!(m2 / 8.0 < m1 / 4.0);
    }

    #[test]
    fn producers_are_less_temperature_sensitive_than_animals() {
        let animal = boltzmann_factor(35.0, E_ANIMAL_EV);
        let plant = boltzmann_factor(35.0, E_PRODUCER_EV);
        assert!(animal > plant); // 0.65 eV responds more steeply than 0.30 eV
    }

    #[test]
    fn logistic_regrowth_converges_to_capacity() {
        let mut r = 0.5;
        for _ in 0..2000 {
            r = logistic_regrowth(r, 0.1, 10.0);
        }
        assert!((r - 10.0).abs() < 1e-2, "r converged to {r}");
    }

    #[test]
    fn logistic_regrowth_recovers_from_zero() {
        // A fully-grazed cell must not stay dead forever (seed term).
        let mut r = 0.0;
        for _ in 0..5000 {
            r = logistic_regrowth(r, 0.2, 5.0);
        }
        assert!(r > 4.9, "r recovered to only {r}");
    }

    #[test]
    fn holling_type3_protects_rare_prey() {
        // At low prey density, Type III consumes far less (per prey) than Type II — this is
        // the rarity refuge that prevents prey extinction.
        let low = 0.2;
        let per_prey_t2 = holling_type2(low, 1.0, 0.5) / low;
        let per_prey_t3 = holling_type3(low, 1.0, 0.5) / low;
        assert!(
            per_prey_t3 < per_prey_t2 * 0.5,
            "t3 {per_prey_t3} vs t2 {per_prey_t2}"
        );
    }

    #[test]
    fn holling_responses_saturate() {
        let hi = holling_type2(1e6, 1.0, 0.5);
        assert!((hi - 1.0 / 0.5).abs() < 1e-2); // saturates at 1/h
        assert!(holling_type3(1e6, 1.0, 0.5) <= 1.0 / 0.5 + 1e-2);
    }

    #[test]
    fn npp_orders_biomes_correctly() {
        assert!(
            biome_carrying_capacity(BiomeType::Rainforest)
                > biome_carrying_capacity(BiomeType::Grassland)
        );
        assert!(
            biome_carrying_capacity(BiomeType::Grassland)
                > biome_carrying_capacity(BiomeType::Desert)
        );
        assert!(
            biome_carrying_capacity(BiomeType::Desert)
                > biome_carrying_capacity(BiomeType::MountainRock) - 1.0
        );
    }

    #[test]
    fn niche_divergence_rises_as_guilds_separate() {
        // Identical guild masses → no displacement.
        assert!(niche_divergence(6.0, 6.0).abs() < 1e-6);
        // Predators evolving heavier than prey → the axes separate.
        let small = niche_divergence(6.0, 8.0);
        let large = niche_divergence(4.0, 16.0);
        assert!(large > small && small > 0.0);
        // Symmetric and clamped.
        assert_eq!(niche_divergence(4.0, 16.0), niche_divergence(16.0, 4.0));
        assert_eq!(niche_divergence(0.0, 1000.0), 1.0);
    }

    #[test]
    fn seasonal_fertility_oscillates_around_one() {
        assert!((seasonal_fertility(0.0) - 1.0).abs() < 1e-6);
        let summer = seasonal_fertility(std::f32::consts::FRAC_PI_2);
        let winter = seasonal_fertility(3.0 * std::f32::consts::FRAC_PI_2);
        assert!(summer > 1.0 && (0.0..1.0).contains(&winter));
        // A full clock cycle returns to the start; the mean stays ~1.0.
        let mut clock = SeasonClock {
            phase: 0.0,
            rate: 0.1,
        };
        let mut sum = 0.0f32;
        let steps = 1000;
        for _ in 0..steps {
            sum += clock.tick(1.0);
        }
        assert!(
            (sum / steps as f32 - 1.0).abs() < 0.05,
            "mean fertility off"
        );
    }

    #[test]
    fn ecological_descriptors_normalize_and_order() {
        let [m, f] = ecological_descriptors(10.0, 100.0);
        assert!((m - 0.5).abs() < 1e-6 && (f - 0.5).abs() < 1e-6);
        // Heavier / wider-ranging organisms sit higher on each axis, clamped to 1.0.
        let [m2, f2] = ecological_descriptors(1000.0, 1000.0);
        assert_eq!([m2, f2], [1.0, 1.0]);
        let [m0, f0] = ecological_descriptors(0.0, 0.0);
        assert_eq!([m0, f0], [0.0, 0.0]);
        // The two axes are independent (a heavy homebody differs from a light wanderer).
        assert_ne!(
            ecological_descriptors(18.0, 10.0),
            ecological_descriptors(2.0, 180.0)
        );
    }

    #[test]
    fn shannon_and_simpson_peak_for_even_communities() {
        let even = [25u32, 25, 25, 25];
        let skewed = [97u32, 1, 1, 1];
        assert!(shannon_index(&even) > shannon_index(&skewed));
        assert!(simpson_index(&even) > simpson_index(&skewed));
        // Even 4-species Shannon = ln 4.
        assert!((shannon_index(&even) - 4.0f32.ln()).abs() < 1e-4);
        assert_eq!(shannon_index(&[10]), 0.0); // a monoculture has zero diversity
    }

    #[test]
    fn resource_field_maps_and_grazes() {
        // 2×2 grid over [0,10]×[0,10]: rainforest, desert / grassland, boreal.
        let biomes = [
            BiomeType::Rainforest as u8,
            BiomeType::Desert as u8,
            BiomeType::Grassland as u8,
            BiomeType::BorealForest as u8,
        ];
        let mut field = ResourceField::from_biomes(&biomes, 2, 2, 0.0, 0.0, 10.0, 10.0, 0.1);
        // Rainforest cell (top-left) holds far more than desert (top-right).
        let rf = field.cell_index(1.0, 1.0).unwrap();
        let ds = field.cell_index(9.0, 1.0).unwrap();
        assert!(field.r[rf] > field.r[ds]);
        // Grazing subtracts and is bounded by the standing resource.
        let before = field.r[rf];
        let taken = field.graze(1.0, 1.0, 5.0);
        assert!(taken > 0.0 && taken <= before);
        assert!((field.r[rf] - (before - taken)).abs() < 1e-5);
        // Out-of-bounds grazing takes nothing.
        assert_eq!(field.graze(999.0, 999.0, 5.0), 0.0);
    }

    #[test]
    fn resource_field_regrowth_is_bounded_and_allocation_free_in_shape() {
        let biomes = [BiomeType::Grassland as u8; 16];
        let mut field = ResourceField::from_biomes(&biomes, 4, 4, -1.0, -1.0, 1.0, 1.0, 0.1);
        // Drain every cell, then confirm regrowth climbs back toward capacity without exceeding it.
        for v in field.r.iter_mut() {
            *v = 0.0;
        }
        let cap = field.r_max[0];
        for _ in 0..3000 {
            field.step_regrowth(1.0, 1.0);
            for &v in &field.r {
                assert!(v <= cap + 1e-3 && v >= 0.0);
            }
        }
        assert!(field.r[0] > cap * 0.9);
        // Zero fertility (deep winter) halts growth.
        for v in field.r.iter_mut() {
            *v = 1.0;
        }
        field.step_regrowth(1.0, 0.0);
        assert!((field.r[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn predation_spares_rare_prey_but_feeds_on_abundance() {
        // A healthy prey (energy 60) loses a real bite but is NOT zeroed in one contact.
        let big = predation_capture(60.0, 100.0);
        assert!(
            big > 0.0 && big < 60.0,
            "captured {big} from a healthy prey"
        );
        // A nearly-dead prey (energy 2) is barely touched — the Type III rarity refuge.
        let small = predation_capture(2.0, 100.0);
        assert!(
            small / 2.0 < big / 60.0,
            "rare prey per-energy loss not lower"
        );
        // Predator never captures more than its deficit warrants (÷ efficiency).
        let capped = predation_capture(1e6, 3.0);
        assert!(capped <= 3.0 / LINDEMAN_EFFICIENCY + 1e-3);
        assert_eq!(predation_capture(0.0, 100.0), 0.0);
    }

    #[test]
    fn gated_regrowth_never_exceeds_detritus_budget() {
        let biomes = [BiomeType::Rainforest as u8; 16];
        let mut field = ResourceField::from_biomes(&biomes, 4, 4, 0.0, 0.0, 4.0, 4.0, 0.2);
        for v in field.r.iter_mut() {
            *v = 0.1; // heavily grazed → strong regrowth demand
        }
        // Starve it: a tiny budget caps how much can regrow this step.
        let consumed = field.step_regrowth_gated(1.0, 1.0, 0.05);
        assert!(
            consumed <= 0.05 + 1e-6,
            "consumed {consumed} exceeded budget"
        );
        assert!(consumed > 0.0);
        // Zero budget → no growth at all (plants can't appear from nothing).
        let before: f64 = field.total_biomass();
        let c0 = field.step_regrowth_gated(1.0, 1.0, 0.0);
        assert_eq!(c0, 0.0);
        assert!((field.total_biomass() - before).abs() < 1e-6);
    }

    #[test]
    fn herbivore_intake_saturates_and_disperses_on_depletion() {
        // A rich cell gives a near-full bite; a nearly-empty cell gives almost nothing —
        // the giving-up-density dispersal signal.
        let rich = herbivore_intake(50.0, 100.0, 2.0);
        let poor = herbivore_intake(0.1, 100.0, 2.0);
        assert!(rich > poor * 5.0, "rich {rich} vs poor {poor}");
        assert!(rich <= 2.0 + 1e-4); // never exceeds the bite ceiling
        assert!(herbivore_intake(50.0, 0.5, 2.0) <= 0.5 + 1e-4); // clamped to hunger
        assert_eq!(herbivore_intake(0.0, 100.0, 2.0), 0.0);
    }

    #[test]
    fn full_trophic_cycle_conserves_energy() {
        // Plants → herbivore (graze) → detritus (metabolism/predation) → plants (gated
        // regrowth). Simulate every transfer on the pool and assert the total is invariant.
        let biomes = [BiomeType::Grassland as u8; 4];
        let mut field = ResourceField::from_biomes(&biomes, 2, 2, 0.0, 0.0, 2.0, 2.0, 0.15);
        let mut pool = EcosystemBiomass {
            detritus: 5.0,
            plants: field.total_biomass(),
            animals: 20.0,
        };
        let total0 = pool.total();
        for _ in 0..50 {
            // Herbivore grazes cell (0,0): plants → animals.
            let bite = herbivore_intake(field.r[0], 100.0, 1.0);
            let taken = field.graze(0.5, 0.5, bite);
            pool.animals += taken as f64;
            // Metabolism burns some animal energy → detritus.
            let burned = 0.5f64.min(pool.animals);
            pool.animals -= burned;
            pool.detritus += burned;
            // Plants regrow, gated by detritus.
            let consumed = field.step_regrowth_gated(1.0, 1.0, pool.detritus);
            pool.detritus -= consumed;
            // Keep the plant compartment in sync with the field.
            pool.plants = field.total_biomass();
            assert!(
                (pool.total() - total0).abs() < 1e-6,
                "energy drifted to {}",
                pool.total()
            );
        }
    }

    #[test]
    fn biomass_pool_conserves_on_transfer() {
        // Simulate a predation event: prey loses `captured`, predator gains `e·captured`,
        // detritus receives the rest — total energy unchanged.
        let mut pool = EcosystemBiomass {
            detritus: 0.0,
            plants: 100.0,
            animals: 50.0,
        };
        let before = pool.total();
        let captured = 10.0f64;
        let assimilated = captured * LINDEMAN_EFFICIENCY as f64;
        // prey (animal) → predator (animal): net animal change is −captured + assimilated.
        pool.animals += -captured + assimilated;
        pool.detritus += captured - assimilated; // unassimilated → detritus
        assert!((pool.total() - before).abs() < 1e-9, "energy not conserved");
    }
}
