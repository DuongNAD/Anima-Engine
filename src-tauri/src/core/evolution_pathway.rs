//! # Energy pathway & reference cohort population (AE3) — the headless selection slice.
//!
//! This module adds the missing middle of the AE causal chain: AE1–AE2.5 proved
//! `world law → spatial MU field → closed MU ledger`, but nothing in the world could *use* that
//! energy. Here an **inherited, costly** [`EnergyPathwayGenotype`] is developed once into a
//! [`DevelopedEnergyPathway`], carries runtime [`ExoticEnergyState`], and drives an opt-in
//! [`ReferencePopulation`] of aggregate cohorts through explicit reproduction/selection events.
//!
//! Three boundaries are load-bearing and deliberately not blurred:
//!
//! - **Genotype ≠ phenotype ≠ runtime.** Development happens once from the genotype
//!   ([`DevelopedEnergyPathway::develop`]); it never reads the environment, so a restored snapshot
//!   re-uses the materialized phenotype instead of re-developing it.
//! - **Performance is a mechanism result, never a fitness field.** A pathway pays its declared
//!   maintenance/opportunity cost every ecology firing whether or not any MU exists. It can only
//!   *gain* performance after an atomic field→storage uptake **and** a storage→dissipated spend, so
//!   `has_exotic` alone can never buy an advantage (ER03 / AE-S06 / AE-S07).
//! - **Frequency changes only at reproduction.** Sensing, uptake, cost, spend and performance
//!   accounting are forbidden from writing cohort counts or genotypes; only
//!   [`ReferencePopulation::reproduce`] may, and the recorded delta equals the resolved offspring
//!   composition (AE-S10).
//!
//! MU is not EU: nothing here touches the closed-EU trophic pools, and every MU that moves is booked
//! through the existing [`crate::core::exotic_energy`] transaction helpers so the MU ledger stays
//! closed (AE-S04/AE-S05).

use crate::core::exotic_energy::EnergySourceId;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

// ---- AE-301: the heritable pathway genotype --------------------------------------------------

/// A heritable energy-pathway trait set. The **legacy default is disabled and zero-cost**
/// ([`EnergyPathwayGenotype::legacy`]), which is what keeps an AE1–AE2.5 world bit-identical.
///
/// Every constructor normalizes its inputs: non-finite values collapse to the low bound and
/// out-of-range values clamp, so a genotype that exists is always [`is_bounded`](Self::is_bounded).
/// `expressed` is **strategy identity**, not a trait: neither mutation nor crossover may flip it, so
/// a legacy lineage can never mutate itself into a free pathway.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnergyPathwayGenotype {
    /// Which exotic source this pathway can use. Uptake from a field with a different source id is
    /// impossible (checked at the transaction seam), never silently coerced.
    pub source_id: EnergySourceId,
    /// Whether this genotype expresses a pathway at all. `false` is the legacy strategy.
    pub expressed: bool,
    /// How well the organism locates dense MU cells, in `[0, 1]`.
    pub sensing_affinity: f32,
    /// MU requested per individual per ecology firing, in `[0, MAX_UPTAKE_RATE]`.
    pub uptake_rate: f32,
    /// MU that one individual can hold, in `[0, MAX_STORAGE_CAPACITY]`.
    pub storage_capacity: f32,
    /// Fraction of spent MU that converts into reproductive performance, in `[0, 1]`.
    pub utilization_efficiency: f32,
    /// Share of storage that can be held without toxicity, in `[0, 1]`.
    pub tolerance: f32,
    /// Performance paid per individual per ecology firing while the pathway is expressed, in
    /// `[0, MAX_MAINTENANCE_COST]`. This is the "no universal benefit" term (ER05).
    pub maintenance_cost: f32,
    /// Share of the body budget diverted into pathway tissue, in `[0, 1]`. An opportunity cost: it
    /// raises the effective maintenance burden and buys storage/uptake surface.
    pub allocation: f32,
}

impl EnergyPathwayGenotype {
    pub const MAX_UPTAKE_RATE: f32 = 1.0;
    pub const MAX_STORAGE_CAPACITY: f32 = 10.0;
    pub const MAX_MAINTENANCE_COST: f32 = 1.0;

    /// The legacy strategy: no pathway, no cost, no storage. Also the [`Default`].
    pub fn legacy() -> Self {
        Self {
            source_id: EnergySourceId::new(""),
            expressed: false,
            sensing_affinity: 0.0,
            uptake_rate: 0.0,
            storage_capacity: 0.0,
            utilization_efficiency: 0.0,
            tolerance: 0.0,
            maintenance_cost: 0.0,
            allocation: 0.0,
        }
    }

    /// An expressed pathway genotype with every trait normalized into its declared range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_id: EnergySourceId,
        sensing_affinity: f32,
        uptake_rate: f32,
        storage_capacity: f32,
        utilization_efficiency: f32,
        tolerance: f32,
        maintenance_cost: f32,
        allocation: f32,
    ) -> Self {
        Self {
            source_id,
            expressed: true,
            sensing_affinity: norm(sensing_affinity, 1.0),
            uptake_rate: norm(uptake_rate, Self::MAX_UPTAKE_RATE),
            storage_capacity: norm(storage_capacity, Self::MAX_STORAGE_CAPACITY),
            utilization_efficiency: norm(utilization_efficiency, 1.0),
            tolerance: norm(tolerance, 1.0),
            maintenance_cost: norm(maintenance_cost, Self::MAX_MAINTENANCE_COST),
            allocation: norm(allocation, 1.0),
        }
    }

    /// Whether every trait is finite and inside its declared range.
    pub fn is_bounded(&self) -> bool {
        let checks = [
            (self.sensing_affinity, 1.0),
            (self.uptake_rate, Self::MAX_UPTAKE_RATE),
            (self.storage_capacity, Self::MAX_STORAGE_CAPACITY),
            (self.utilization_efficiency, 1.0),
            (self.tolerance, 1.0),
            (self.maintenance_cost, Self::MAX_MAINTENANCE_COST),
            (self.allocation, 1.0),
        ];
        checks
            .iter()
            .all(|(v, hi)| v.is_finite() && *v >= 0.0 && v <= hi)
    }

    /// A bounded, seeded mutant. `rate` scales the per-trait jitter and is itself normalized, so an
    /// absurd or non-finite rate cannot push a trait out of range. A **legacy genotype is returned
    /// unchanged**: strategy identity is not a mutable trait.
    pub fn mutate(&self, rng: &mut StdRng, rate: f32) -> Self {
        if !self.expressed {
            return Self::legacy();
        }
        let rate = norm(rate, 1.0);
        let mut jitter = |value: f32, hi: f32| -> f32 {
            // A symmetric relative step; `norm` re-clamps so the bound holds by construction.
            let step = rng.gen::<f32>() - 0.5;
            norm(value + step * rate * hi, hi)
        };
        Self {
            source_id: self.source_id.clone(),
            expressed: true,
            sensing_affinity: jitter(self.sensing_affinity, 1.0),
            uptake_rate: jitter(self.uptake_rate, Self::MAX_UPTAKE_RATE),
            storage_capacity: jitter(self.storage_capacity, Self::MAX_STORAGE_CAPACITY),
            utilization_efficiency: jitter(self.utilization_efficiency, 1.0),
            tolerance: jitter(self.tolerance, 1.0),
            maintenance_cost: jitter(self.maintenance_cost, Self::MAX_MAINTENANCE_COST),
            allocation: jitter(self.allocation, 1.0),
        }
    }

    /// Uniform per-trait crossover of two parents under an explicit RNG. Every trait is copied from
    /// one parent — nothing is invented or averaged — so the child is bounded whenever the parents
    /// are. The child is expressed only if **both** parents are (recombining two legacy genotypes
    /// can never manufacture a pathway).
    ///
    /// Returns `None` when two expressed parents target different source ids. Mixing source-specific
    /// traits while silently inheriting only one source id would create a biologically incoherent
    /// pathway, so callers must resolve compatibility before reproduction.
    pub fn crossover(a: &Self, b: &Self, rng: &mut StdRng) -> Option<Self> {
        if !(a.expressed && b.expressed) {
            return Some(Self::legacy());
        }
        if a.source_id != b.source_id {
            return None;
        }
        let mut pick = |x: f32, y: f32| if rng.gen::<bool>() { x } else { y };
        Some(Self {
            source_id: a.source_id.clone(),
            expressed: true,
            sensing_affinity: pick(a.sensing_affinity, b.sensing_affinity),
            uptake_rate: pick(a.uptake_rate, b.uptake_rate),
            storage_capacity: pick(a.storage_capacity, b.storage_capacity),
            utilization_efficiency: pick(a.utilization_efficiency, b.utilization_efficiency),
            tolerance: pick(a.tolerance, b.tolerance),
            maintenance_cost: pick(a.maintenance_cost, b.maintenance_cost),
            allocation: pick(a.allocation, b.allocation),
        })
    }
}

impl Default for EnergyPathwayGenotype {
    fn default() -> Self {
        Self::legacy()
    }
}

/// Clamp `v` into `[0, hi]`, mapping any non-finite input to `0.0`.
fn norm(v: f32, hi: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(0.0, hi)
}

// ---- AE-303: one-time development ------------------------------------------------------------

/// The materialized pathway an organism is born with. Produced **once** from the genotype by
/// [`develop`](Self::develop) — it reads no environment, so restore/migration re-uses a serialized
/// phenotype rather than re-developing (the Creature Development Contract boundary, applied here to
/// the reference cohort model).
#[derive(Clone, Debug, PartialEq)]
pub struct DevelopedEnergyPathway {
    pub source_id: EnergySourceId,
    pub expressed: bool,
    /// Effective sensing reach, in `[0, 1]`.
    pub sensor_range: f64,
    /// Effective uptake surface (MU requested per individual per ecology firing).
    pub uptake_surface: f64,
    /// MU one individual can store.
    pub storage_capacity: f64,
    /// Fraction of spent MU that becomes reproductive performance.
    pub utilization_efficiency: f64,
    /// Share of storage tolerated before toxicity accrues.
    pub tolerance: f64,
    /// Performance paid per individual per ecology firing, including the allocation opportunity
    /// cost. Always `>= genotype.maintenance_cost`.
    pub maintenance_cost: f64,
}

impl DevelopedEnergyPathway {
    /// How strongly `allocation` converts into extra tissue (and extra upkeep).
    const ALLOCATION_GAIN: f64 = 0.5;

    /// Materialize the phenotype from the genotype. Pure and total.
    pub fn develop(g: &EnergyPathwayGenotype) -> Self {
        if !g.expressed {
            return Self {
                source_id: g.source_id.clone(),
                expressed: false,
                sensor_range: 0.0,
                uptake_surface: 0.0,
                storage_capacity: 0.0,
                utilization_efficiency: 0.0,
                tolerance: 0.0,
                maintenance_cost: 0.0,
            };
        }
        let alloc = g.allocation as f64;
        // Allocation buys surface and storage, and is paid for in upkeep — the trade-off is
        // structural, not a tunable bonus.
        let tissue = 1.0 + Self::ALLOCATION_GAIN * alloc;
        Self {
            source_id: g.source_id.clone(),
            expressed: true,
            sensor_range: g.sensing_affinity as f64,
            uptake_surface: g.uptake_rate as f64 * tissue,
            storage_capacity: g.storage_capacity as f64 * tissue,
            utilization_efficiency: g.utilization_efficiency as f64,
            tolerance: g.tolerance as f64,
            maintenance_cost: g.maintenance_cost as f64 * tissue,
        }
    }
}

// ---- AE-304: runtime state -------------------------------------------------------------------

/// Per-cohort runtime exotic state. Storage is real MU held out of the field; it is part of the MU
/// budget's `organism_storage` slot, so it can never be minted or lost silently.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExoticEnergyState {
    /// MU currently held by this cohort (aggregate, not per individual).
    pub stored_mu: f64,
    /// MU taken from the field on the last ecology firing.
    pub last_uptake_mu: f64,
    /// MU spent from storage on the last ecology firing (booked as dissipated).
    pub last_spent_mu: f64,
    /// Accumulated overload from holding MU above the tolerated share of capacity.
    pub toxicity_load: f64,
}

// ---- AE-305/306: the cohort and its transactional physiology ---------------------------------

/// One inherited strategy as an aggregate cohort: a genotype, the phenotype developed from it once,
/// its runtime MU state, and how many individuals carry it.
///
/// The physiology is three ordered, separately-testable steps — [`uptake`](Self::uptake),
/// [`metabolize`](Self::metabolize), [`update_performance`](Self::update_performance) — and **none
/// of them may write `count` or `genotype`**. That is what makes AE-S10 checkable: only
/// [`ReferencePopulation::reproduce`] changes composition.
#[derive(Clone, Debug, PartialEq)]
pub struct PathwayCohort {
    pub genotype: EnergyPathwayGenotype,
    pub developed: DevelopedEnergyPathway,
    pub state: ExoticEnergyState,
    /// Individuals carrying this strategy.
    pub count: f64,
    /// Performance measured on the most recent ecology firing.
    pub last_performance: f64,
    /// Running sum/count of performance since the last reproduction (what selection reads).
    perf_accum: f64,
    perf_samples: u64,
    /// Mean performance of the last completed generation, so the observable is defined immediately
    /// after a reproduction event resets the accumulator.
    last_mean_performance: f64,
}

impl PathwayCohort {
    /// Reproductive performance bought by one efficiently-utilized MU.
    pub const PERFORMANCE_PER_MU: f64 = 10.0;
    /// Performance lost per unit of accumulated toxicity, per individual.
    pub const TOXICITY_PENALTY: f64 = 0.5;
    /// Fraction of the toxicity load cleared each ecology firing.
    const TOXICITY_CLEARANCE: f64 = 0.25;
    /// Share of *currently stored* MU burned each ecology firing. Metabolic demand is deliberately
    /// proportional to the reserve rather than to the uptake surface: that makes storage a genuine
    /// multi-firing buffer (it settles at roughly `intake / SPEND_FRACTION`) instead of a slot that
    /// drains to empty on every tick, so `storage_capacity` is a trait that can actually matter.
    const SPEND_FRACTION: f64 = 0.5;

    /// Build a cohort, developing its phenotype **once** here (the reference model's birth path).
    pub fn new(genotype: EnergyPathwayGenotype, count: f64) -> Self {
        let developed = DevelopedEnergyPathway::develop(&genotype);
        Self {
            genotype,
            developed,
            state: ExoticEnergyState::default(),
            count: finite_non_negative(count),
            last_performance: 0.0,
            perf_accum: 0.0,
            perf_samples: 0,
            last_mean_performance: 0.0,
        }
    }

    /// Total MU this cohort can hold.
    pub fn storage_capacity_total(&self) -> f64 {
        self.developed.storage_capacity * self.count
    }

    /// Mean performance since the last reproduction — the quantity selection acts on. Falls back to
    /// the previous generation's mean in the window right after a reproduction event.
    pub fn mean_performance(&self) -> f64 {
        if self.perf_samples == 0 {
            self.last_mean_performance
        } else {
            self.perf_accum / self.perf_samples as f64
        }
    }

    /// **Step 3 — sense and atomically take MU from the field into storage.**
    ///
    /// Returns the MU actually moved, which is exactly the field's decrease (the transaction goes
    /// through [`crate::core::exotic_energy::ExoticEnergyField::uptake`], so it is ledger-exact
    /// across the f32 field / f64 storage boundary). Allocation-free: it scans cells in index order
    /// and stops as soon as the sensed request is satisfied.
    ///
    /// Returns `0.0` — touching nothing — for a legacy cohort, a blind one (`sensor_range == 0`),
    /// an empty one, or one whose `source_id` does not match the field's.
    pub fn uptake(&mut self, field: &mut crate::core::exotic_energy::ExoticEnergyField) -> f64 {
        self.state.last_uptake_mu = 0.0;
        if !self.developed.expressed || self.count <= 0.0 {
            return 0.0;
        }
        // A pathway is tuned to ONE source. A different field is not a weaker meal, it is no meal.
        if self.developed.source_id != field.source_id {
            return 0.0;
        }
        let mut remaining =
            self.developed.uptake_surface * self.developed.sensor_range * self.count;
        if !(remaining.is_finite() && remaining > 0.0) {
            return 0.0;
        }
        let capacity = self.storage_capacity_total();
        let room = (capacity - self.state.stored_mu).max(0.0);
        remaining = remaining.min(room);
        if !(remaining.is_finite() && remaining > 0.0) {
            return 0.0;
        }

        let mut moved = 0.0;
        for idx in 0..field.density.len() {
            if remaining <= 0.0 {
                break;
            }
            let got = field.uptake(idx, remaining, &mut self.state.stored_mu, capacity);
            moved += got;
            remaining -= got;
        }
        self.state.last_uptake_mu = moved;

        // Holding more than the tolerated share of capacity accrues an overload the organism pays
        // for in performance — the ER05 toxicity term, not a hidden fitness knob.
        let tolerated = capacity * self.developed.tolerance;
        let excess = (self.state.stored_mu - tolerated).max(0.0);
        if excess > 0.0 {
            self.state.toxicity_load += excess;
        }
        moved
    }

    /// **Step 4 — pay the metabolic demand by atomically spending stored MU.**
    ///
    /// Spends up to the cohort's metabolic demand from storage, booking every MU into `dissipated`
    /// via [`crate::core::exotic_energy::spend_storage`]. Returns the MU actually spent (`0.0` when
    /// storage is empty — which is exactly why an absent source yields no benefit).
    pub fn metabolize(&mut self, dissipated: &mut f64) -> f64 {
        self.state.last_spent_mu = 0.0;
        // Toxicity clears slowly whether or not there is anything to spend.
        self.state.toxicity_load *= 1.0 - Self::TOXICITY_CLEARANCE;
        if !self.developed.expressed || self.count <= 0.0 {
            return 0.0;
        }
        let demand = Self::SPEND_FRACTION * self.state.stored_mu;
        if !(demand.is_finite() && demand > 0.0) {
            return 0.0;
        }
        let spent = crate::core::exotic_energy::spend_storage(
            &mut self.state.stored_mu,
            demand,
            dissipated,
        );
        self.state.last_spent_mu = spent;
        spent
    }

    /// **Step 5 — derive this firing's performance from the mechanism outputs.**
    ///
    /// `base` is the shared baseline reproductive performance both strategies start from. The
    /// pathway's contribution is `efficiency × spent-MU-per-individual`; its cost is the developed
    /// maintenance/opportunity burden plus any toxicity. The result is always finite and
    /// non-negative, and is accumulated for the next selection event.
    ///
    /// Note it reads `last_spent_mu` — **not** `expressed` — so presence of a pathway can never buy
    /// performance without a completed uptake→spend transaction.
    pub fn update_performance(&mut self, base: f64) -> f64 {
        let base = finite_non_negative(base);
        let per_capita = |total: f64| {
            if self.count > 0.0 {
                total / self.count
            } else {
                0.0
            }
        };
        let gain = self.developed.utilization_efficiency
            * per_capita(self.state.last_spent_mu)
            * Self::PERFORMANCE_PER_MU;
        let toxicity = Self::TOXICITY_PENALTY * per_capita(self.state.toxicity_load);
        let perf = base + gain - self.developed.maintenance_cost - toxicity;
        let perf = if perf.is_finite() { perf.max(0.0) } else { 0.0 };
        self.last_performance = perf;
        self.perf_accum += perf;
        self.perf_samples += 1;
        perf
    }

    /// Close the current selection window: freeze the mean and reset the accumulator. Called only
    /// from a reproduction event.
    fn close_performance_window(&mut self) {
        if self.perf_samples > 0 {
            self.last_mean_performance = self.perf_accum / self.perf_samples as f64;
        }
        self.perf_accum = 0.0;
        self.perf_samples = 0;
    }
}

/// Map any non-finite or negative input to `0.0`.
fn finite_non_negative(v: f64) -> f64 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

// ---- AE-307: the opt-in reference population --------------------------------------------------

/// Declared `t=0` configuration of the reference population. Validated structurally: an *enabled*
/// population that cannot exist is an error, never a silently-corrected default.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferencePopulationConfig {
    /// Individuals at genesis (must be `> 0` — this is the opt-in switch).
    pub total: f64,
    /// Fixed carrying capacity; must be `>= total`.
    pub capacity: f64,
    /// Share of `total` carrying the pathway strategy, in `[0, 1]`.
    pub pathway_fraction: f64,
    /// Base ticks between reproduction events. Must be a positive multiple of
    /// [`ECOLOGY_PERIOD`](crate::core::sim_clock::ECOLOGY_PERIOD) so a generation boundary always
    /// coincides with a physiology firing.
    pub generation_ticks: u64,
    /// Baseline reproductive performance shared by both strategies.
    pub base_performance: f64,
    /// Bounded per-generation trait jitter.
    pub mutation_rate: f32,
    /// The pathway strategy's genotype. Must be expressed — a "pathway cohort" carrying the legacy
    /// genotype would make the whole comparison meaningless.
    pub genotype: EnergyPathwayGenotype,
}

impl ReferencePopulationConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [
            ("total", self.total),
            ("capacity", self.capacity),
            ("pathway_fraction", self.pathway_fraction),
            ("base_performance", self.base_performance),
        ] {
            if !v.is_finite() {
                return Err(format!("ae3 population {name} must be finite (got {v})"));
            }
            if v < 0.0 {
                return Err(format!("ae3 population {name} must be >= 0 (got {v})"));
            }
        }
        if self.total <= 0.0 {
            return Err("ae3 population total must be > 0 when the population is enabled".into());
        }
        if self.capacity < self.total {
            return Err(format!(
                "ae3 population capacity {} must be >= total {}",
                self.capacity, self.total
            ));
        }
        if self.pathway_fraction > 1.0 {
            return Err(format!(
                "ae3 pathway_fraction must be within [0, 1] (got {})",
                self.pathway_fraction
            ));
        }
        if self.generation_ticks == 0 {
            return Err("ae3 generation_ticks must be > 0 (a zero cadence never fires)".into());
        }
        if !self
            .generation_ticks
            .is_multiple_of(crate::core::sim_clock::ECOLOGY_PERIOD)
        {
            return Err(format!(
                "ae3 generation_ticks {} must be a multiple of the ecology period {}, otherwise a \
                 generation boundary would never coincide with a physiology firing",
                self.generation_ticks,
                crate::core::sim_clock::ECOLOGY_PERIOD
            ));
        }
        if !self.mutation_rate.is_finite() || self.mutation_rate < 0.0 {
            return Err(format!(
                "ae3 mutation_rate must be finite and >= 0 (got {})",
                self.mutation_rate
            ));
        }
        if !self.genotype.expressed {
            return Err("ae3 pathway genotype must express a pathway".into());
        }
        if !self.genotype.is_bounded() {
            return Err("ae3 pathway genotype has an out-of-range trait".into());
        }
        Ok(())
    }
}

// ---- The AE3 initial-condition seam ----------------------------------------------------------
//
// The AE3 reference population is configured through **version-1 `InitialConditionSet` scalar
// keys**, not through a new world-law schema: no schema version moves, and every existing fixture
// keeps working untouched because absence of these keys means "population disabled".

/// Prefix every AE3 reference-fixture key shares.
pub const AE3_KEY_PREFIX: &str = "ae3.";
/// Individuals at genesis. **Presence of this key is what enables the population.**
pub const AE3_KEY_POPULATION_TOTAL: &str = "ae3.population_total";
/// Fixed carrying capacity (default: `population_total`).
pub const AE3_KEY_POPULATION_CAPACITY: &str = "ae3.population_capacity";
/// Initial share carrying the pathway strategy (default: 0.5).
pub const AE3_KEY_PATHWAY_FRACTION: &str = "ae3.pathway_fraction";
/// Base ticks between reproduction events; must be a positive whole multiple of the ecology period
/// (default: 600).
pub const AE3_KEY_GENERATION_TICKS: &str = "ae3.generation_ticks";
/// Baseline reproductive performance shared by both strategies (default: 1.0).
pub const AE3_KEY_BASE_PERFORMANCE: &str = "ae3.base_performance";
/// Bounded per-generation trait jitter (default: 0.0, i.e. no drift unless declared).
pub const AE3_KEY_MUTATION_RATE: &str = "ae3.mutation_rate";
/// Pathway genotype: sensing affinity (default: 0.8).
pub const AE3_KEY_PATHWAY_SENSING: &str = "ae3.pathway_sensing";
/// Pathway genotype: uptake rate (default: 0.05).
pub const AE3_KEY_PATHWAY_UPTAKE: &str = "ae3.pathway_uptake";
/// Pathway genotype: storage capacity per individual (default: 0.5).
pub const AE3_KEY_PATHWAY_STORAGE: &str = "ae3.pathway_storage";
/// Pathway genotype: utilization efficiency (default: 0.6).
pub const AE3_KEY_PATHWAY_EFFICIENCY: &str = "ae3.pathway_efficiency";
/// Pathway genotype: tolerated share of storage before toxicity (default: 0.5).
pub const AE3_KEY_PATHWAY_TOLERANCE: &str = "ae3.pathway_tolerance";
/// Pathway genotype: **maintenance cost** — the factorial's cost factor (default: 0.01).
pub const AE3_KEY_PATHWAY_MAINTENANCE: &str = "ae3.pathway_maintenance";
/// Pathway genotype: body-budget allocation to pathway tissue (default: 0.2).
pub const AE3_KEY_PATHWAY_ALLOCATION: &str = "ae3.pathway_allocation";

/// Every accepted AE3 initial-condition key. Any other `ae3.` key is a typo or an unsupported
/// input and is **rejected**, never silently ignored.
pub const AE3_INITIAL_CONDITION_KEYS: [&str; 13] = [
    AE3_KEY_POPULATION_TOTAL,
    AE3_KEY_POPULATION_CAPACITY,
    AE3_KEY_PATHWAY_FRACTION,
    AE3_KEY_GENERATION_TICKS,
    AE3_KEY_BASE_PERFORMANCE,
    AE3_KEY_MUTATION_RATE,
    AE3_KEY_PATHWAY_SENSING,
    AE3_KEY_PATHWAY_UPTAKE,
    AE3_KEY_PATHWAY_STORAGE,
    AE3_KEY_PATHWAY_EFFICIENCY,
    AE3_KEY_PATHWAY_TOLERANCE,
    AE3_KEY_PATHWAY_MAINTENANCE,
    AE3_KEY_PATHWAY_ALLOCATION,
];

/// The observable ids this slice adds. A manifest requesting any of them must enable a valid
/// population, or validation fails rather than returning a fabricated zero.
pub const AE3_OBSERVABLE_IDS: [&str; 10] = [
    "evolution.population_total",
    "evolution.pathway_population",
    "evolution.pathway_frequency",
    "evolution.generation",
    "evolution.births",
    "evolution.performance_legacy",
    "evolution.performance_pathway",
    "evolution.performance_delta",
    "exotic.uptake",
    "exotic.spent",
];

/// The source id a pathway is tuned to when the run declares **no** exotic law. It matches no field
/// (a baseline world has none at all), so the pathway pays its cost and gains nothing.
pub const AE3_ABSENT_SOURCE_ID: &str = "ae3.absent_source";

impl ReferencePopulationConfig {
    /// Read the AE3 reference-population configuration out of an [`InitialConditionSet`].
    ///
    /// Returns `Ok(None)` when **no** `ae3.` key is present — the legacy path, which must stay
    /// bit-identical. Returns `Err` for an enabled-but-impossible population, an unknown `ae3.` key,
    /// or `ae3.` keys with no declared population total (a declared input that could never act).
    ///
    /// `source` is the **active exotic law's** id, so the pathway is always tuned to the source the
    /// run actually declares; a fixture cannot name an incompatible source. In a source-absent world
    /// it falls back to [`AE3_ABSENT_SOURCE_ID`].
    pub fn from_initial_conditions(
        initial: &crate::core::experiment::InitialConditionSet,
        source: Option<&EnergySourceId>,
    ) -> Result<Option<Self>, String> {
        let declared: Vec<&str> = initial
            .values
            .iter()
            .map(|(k, _)| k.as_str())
            .filter(|k| k.starts_with(AE3_KEY_PREFIX))
            .collect();
        if declared.is_empty() {
            return Ok(None);
        }
        if let Some(unknown) = declared
            .iter()
            .find(|k| !AE3_INITIAL_CONDITION_KEYS.contains(k))
        {
            return Err(format!(
                "unknown AE3 initial-condition key '{unknown}'; accepted keys are {:?}",
                AE3_INITIAL_CONDITION_KEYS
            ));
        }
        let Some(total) = initial.get(AE3_KEY_POPULATION_TOTAL) else {
            return Err(format!(
                "AE3 initial conditions {declared:?} were declared without '{AE3_KEY_POPULATION_TOTAL}', \
                 so the population would stay disabled and the declared input could never take effect"
            ));
        };
        let get = |key: &str, default: f64| initial.get(key).unwrap_or(default);

        // The cadence is a tick count, so a fractional value is a declaration error rather than
        // something to round silently.
        let raw_ticks = get(AE3_KEY_GENERATION_TICKS, 600.0);
        if !raw_ticks.is_finite() || raw_ticks < 0.0 || raw_ticks.fract() != 0.0 {
            return Err(format!(
                "'{AE3_KEY_GENERATION_TICKS}' must be a whole non-negative tick count (got {raw_ticks})"
            ));
        }
        let source_id = source
            .cloned()
            .unwrap_or_else(|| EnergySourceId::new(AE3_ABSENT_SOURCE_ID));

        let config = Self {
            total,
            capacity: get(AE3_KEY_POPULATION_CAPACITY, total),
            pathway_fraction: get(AE3_KEY_PATHWAY_FRACTION, 0.5),
            generation_ticks: raw_ticks as u64,
            base_performance: get(AE3_KEY_BASE_PERFORMANCE, 1.0),
            mutation_rate: get(AE3_KEY_MUTATION_RATE, 0.0) as f32,
            genotype: EnergyPathwayGenotype::new(
                source_id,
                get(AE3_KEY_PATHWAY_SENSING, 0.8) as f32,
                get(AE3_KEY_PATHWAY_UPTAKE, 0.05) as f32,
                get(AE3_KEY_PATHWAY_STORAGE, 0.5) as f32,
                get(AE3_KEY_PATHWAY_EFFICIENCY, 0.6) as f32,
                get(AE3_KEY_PATHWAY_TOLERANCE, 0.5) as f32,
                get(AE3_KEY_PATHWAY_MAINTENANCE, 0.01) as f32,
                get(AE3_KEY_PATHWAY_ALLOCATION, 0.2) as f32,
            ),
        };
        config.validate()?;
        Ok(Some(config))
    }
}

/// What one reproduction event resolved. The frequency delta is **derived from** the offspring
/// counts, never written independently, which is what makes AE-S10 provable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReproductionOutcome {
    pub generation: u64,
    pub births: f64,
    pub legacy_offspring: f64,
    pub pathway_offspring: f64,
    pub frequency_before: f64,
    pub frequency_after: f64,
    pub delta: f64,
    /// Mean performance each strategy took into this selection event.
    pub legacy_performance: f64,
    pub pathway_performance: f64,
}

/// A fixed-capacity, two-strategy headless population. Reproduction is **evolutionary replacement**
/// (the whole cohort turns over at a generation boundary), matching the engine's current epoch
/// model rather than pretending to be biological birth/death.
///
/// Its RNG is a **separate deterministic stream** derived from the manifest seed, so population
/// variation never consumes or reorders the legacy ecology RNG — which is exactly why the AE-S01
/// baseline stays bit-identical.
#[derive(Clone, Debug)]
pub struct ReferencePopulation {
    pub legacy: PathwayCohort,
    pub pathway: PathwayCohort,
    pub capacity: f64,
    pub generation: u64,
    /// Cumulative offspring produced across all reproduction events.
    pub births: f64,
    pub base_performance: f64,
    pub mutation_rate: f32,
    pub generation_ticks: u64,
    /// Frequency change recorded by the most recent reproduction event.
    pub last_frequency_delta: f64,
    rng: StdRng,
    cum_uptake: f64,
    cum_spent: f64,
}

impl ReferencePopulation {
    /// Domain separator for the population's RNG stream, so it cannot collide with the field's
    /// hotspot stream or the ecology stream that share the same manifest seed.
    const RNG_DOMAIN: u64 = 0xA3E3_0000_5EED_0001;
    /// Bounded seeded jitter on the offspring share, so an ensemble has real between-seed variance
    /// instead of a degenerate zero-variance effect.
    const REPRODUCTION_JITTER: f64 = 0.02;

    /// Build the population from a validated config and the run's manifest seed.
    pub fn new(config: &ReferencePopulationConfig, seed: u64) -> Result<Self, String> {
        config.validate()?;
        let pathway_count = config.total * config.pathway_fraction;
        let legacy_count = config.total - pathway_count;
        Ok(Self {
            legacy: PathwayCohort::new(EnergyPathwayGenotype::legacy(), legacy_count),
            pathway: PathwayCohort::new(config.genotype.clone(), pathway_count),
            capacity: config.capacity,
            generation: 0,
            births: 0.0,
            base_performance: config.base_performance,
            mutation_rate: config.mutation_rate,
            generation_ticks: config.generation_ticks,
            last_frequency_delta: 0.0,
            rng: StdRng::seed_from_u64(seed ^ Self::RNG_DOMAIN),
            cum_uptake: 0.0,
            cum_spent: 0.0,
        })
    }

    pub fn total(&self) -> f64 {
        self.legacy.count + self.pathway.count
    }

    /// Share of the population carrying the pathway strategy, always in `[0, 1]`.
    pub fn pathway_frequency(&self) -> f64 {
        let total = self.total();
        if total > 0.0 {
            (self.pathway.count / total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// MU currently held in organism storage across both cohorts (the budget's `organism_storage`).
    pub fn total_stored(&self) -> f64 {
        self.legacy.state.stored_mu + self.pathway.state.stored_mu
    }

    pub fn cum_uptake(&self) -> f64 {
        self.cum_uptake
    }

    pub fn cum_spent(&self) -> f64 {
        self.cum_spent
    }

    /// Whether `tick` is a generation boundary.
    pub fn is_generation_boundary(&self, tick: u64) -> bool {
        self.generation_ticks != 0 && tick.is_multiple_of(self.generation_ticks)
    }

    /// **Steps 3–5 for one ecology firing:** sense/uptake, pay cost and spend, derive performance.
    ///
    /// Explicitly may NOT change counts, genotypes, generation or births — see
    /// [`reproduce`](Self::reproduce). `field` is `None` in a source-absent world, where the pathway
    /// still pays its full cost.
    pub fn step_physiology(
        &mut self,
        field: Option<&mut crate::core::exotic_energy::ExoticEnergyField>,
        dissipated: &mut f64,
    ) {
        if let Some(field) = field {
            // Legacy cohorts are unexpressed, so this is a no-op for them; it is called anyway so
            // the ordering is uniform and a future expressed legacy variant cannot be forgotten.
            self.cum_uptake += self.legacy.uptake(field);
            self.cum_uptake += self.pathway.uptake(field);
        } else {
            self.legacy.state.last_uptake_mu = 0.0;
            self.pathway.state.last_uptake_mu = 0.0;
        }
        self.cum_spent += self.legacy.metabolize(dissipated);
        self.cum_spent += self.pathway.metabolize(dissipated);
        self.legacy.update_performance(self.base_performance);
        self.pathway.update_performance(self.base_performance);
    }

    /// **Step 6 — the generation boundary: the only place composition may change.**
    ///
    /// Parent contribution is `count × mean measured performance`, so a strategy that performed
    /// better *through the mechanism* leaves more offspring. Parental MU storage is released into
    /// `dissipated` (evolutionary replacement turns the whole cohort over), keeping the MU ledger
    /// closed. Offspring genotypes are the parents' with bounded seeded mutation, and each is
    /// re-developed once — strategy identity is never mutated.
    pub fn reproduce(&mut self, dissipated: &mut f64) -> ReproductionOutcome {
        let frequency_before = self.pathway_frequency();
        self.legacy.close_performance_window();
        self.pathway.close_performance_window();
        let w_legacy = self.legacy.mean_performance().max(0.0);
        let w_pathway = self.pathway.mean_performance().max(0.0);

        // Release parental storage as a declared sink before the cohort turns over.
        for cohort in [&mut self.legacy, &mut self.pathway] {
            let held = cohort.state.stored_mu;
            let released = crate::core::exotic_energy::spend_storage(
                &mut cohort.state.stored_mu,
                held,
                dissipated,
            );
            self.cum_spent += released;
            cohort.state.stored_mu = 0.0;
            cohort.state.toxicity_load = 0.0;
            cohort.state.last_uptake_mu = 0.0;
            cohort.state.last_spent_mu = 0.0;
        }

        let weight_legacy = self.legacy.count * w_legacy;
        let weight_pathway = self.pathway.count * w_pathway;
        let total_weight = weight_legacy + weight_pathway;
        let new_total = self.total().min(self.capacity);

        // Draw the jitter unconditionally so the RNG stream advances the same way whatever the
        // selection outcome — a treatment must never change the draw ORDER (ER07).
        let jitter = (self.rng.gen::<f64>() - 0.5) * 2.0 * Self::REPRODUCTION_JITTER;

        let (legacy_offspring, pathway_offspring) =
            if total_weight > 0.0 && new_total > 0.0 && total_weight.is_finite() {
                let share = ((weight_pathway / total_weight) + jitter).clamp(0.0, 1.0);
                let pathway_offspring = new_total * share;
                (new_total - pathway_offspring, pathway_offspring)
            } else {
                // Total reproductive failure: nothing is born and composition is untouched rather
                // than fabricated.
                (self.legacy.count, self.pathway.count)
            };
        let births = if total_weight > 0.0 && new_total > 0.0 {
            new_total
        } else {
            0.0
        };

        // Offspring inherit their parent strategy with bounded seeded variation, re-developed once.
        let legacy_genotype = self
            .legacy
            .genotype
            .mutate(&mut self.rng, self.mutation_rate);
        let pathway_genotype = self
            .pathway
            .genotype
            .mutate(&mut self.rng, self.mutation_rate);
        self.legacy.genotype = legacy_genotype;
        self.legacy.developed = DevelopedEnergyPathway::develop(&self.legacy.genotype);
        self.pathway.genotype = pathway_genotype;
        self.pathway.developed = DevelopedEnergyPathway::develop(&self.pathway.genotype);

        self.legacy.count = legacy_offspring;
        self.pathway.count = pathway_offspring;
        self.births += births;
        self.generation += 1;

        let frequency_after = self.pathway_frequency();
        let delta = frequency_after - frequency_before;
        self.last_frequency_delta = delta;

        ReproductionOutcome {
            generation: self.generation,
            births,
            legacy_offspring,
            pathway_offspring,
            frequency_before,
            frequency_after,
            delta,
            legacy_performance: w_legacy,
            pathway_performance: w_pathway,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    // ---- AE-301: heritable pathway genotype, legacy default disabled & zero-cost -------------

    #[test]
    fn ae301_legacy_genotype_is_disabled_and_zero_cost() {
        let legacy = EnergyPathwayGenotype::legacy();
        assert!(
            !legacy.expressed,
            "legacy default must not express a pathway"
        );
        assert_eq!(legacy.maintenance_cost, 0.0);
        assert_eq!(legacy.allocation, 0.0);
        assert_eq!(legacy.uptake_rate, 0.0);
        assert_eq!(legacy.storage_capacity, 0.0);
        assert_eq!(legacy.utilization_efficiency, 0.0);
        assert_eq!(legacy.sensing_affinity, 0.0);
        // The legacy genotype is also the `Default`, so a struct-update literal cannot smuggle a
        // cost in by accident.
        assert_eq!(EnergyPathwayGenotype::default(), legacy);
    }

    #[test]
    fn ae301_genotype_normalizes_non_finite_and_out_of_range_inputs() {
        let g = EnergyPathwayGenotype::new(
            EnergySourceId::new("arcane_flux"),
            f32::NAN,          // sensing
            f32::INFINITY,     // uptake
            -5.0,              // storage capacity
            2.0,               // efficiency (over 1)
            f32::NEG_INFINITY, // tolerance
            -1.0,              // maintenance cost
            17.0,              // allocation (over 1)
        );
        for (name, v) in [
            ("sensing_affinity", g.sensing_affinity),
            ("uptake_rate", g.uptake_rate),
            ("storage_capacity", g.storage_capacity),
            ("utilization_efficiency", g.utilization_efficiency),
            ("tolerance", g.tolerance),
            ("maintenance_cost", g.maintenance_cost),
            ("allocation", g.allocation),
        ] {
            assert!(v.is_finite(), "{name} must be finite, got {v}");
            assert!(v >= 0.0, "{name} must be >= 0, got {v}");
        }
        assert!(g.utilization_efficiency <= 1.0);
        assert!(g.sensing_affinity <= 1.0);
        assert!(g.tolerance <= 1.0);
        assert!(g.allocation <= 1.0);
        assert!(g.uptake_rate <= EnergyPathwayGenotype::MAX_UPTAKE_RATE);
        assert!(g.storage_capacity <= EnergyPathwayGenotype::MAX_STORAGE_CAPACITY);
        assert!(g.maintenance_cost <= EnergyPathwayGenotype::MAX_MAINTENANCE_COST);
        assert!(g.expressed, "a constructed pathway genotype is expressed");
    }

    #[test]
    fn ae301_genotype_serde_round_trips_without_trait_or_source_loss() {
        let genotype = sample_pathway();
        let json = serde_json::to_string(&genotype).expect("genotype must serialize");
        let restored: EnergyPathwayGenotype =
            serde_json::from_str(&json).expect("genotype must deserialize");
        assert_eq!(restored, genotype);
    }

    // ---- AE-302: seeded, bounded mutation & crossover ----------------------------------------

    fn sample_pathway() -> EnergyPathwayGenotype {
        EnergyPathwayGenotype::new(
            EnergySourceId::new("arcane_flux"),
            0.8,
            0.05,
            0.5,
            0.6,
            0.5,
            0.01,
            0.2,
        )
    }

    #[test]
    fn ae302_mutation_is_replay_deterministic_and_bounded() {
        let parent = sample_pathway();
        let mut a = StdRng::seed_from_u64(7);
        let mut b = StdRng::seed_from_u64(7);
        let ma = parent.mutate(&mut a, 0.5);
        let mb = parent.mutate(&mut b, 0.5);
        assert_eq!(ma, mb, "same seed must reproduce the same mutant");
        assert_ne!(ma, parent, "a 0.5 mutation rate must actually vary traits");

        // Bounds hold even under an absurd (normalized) mutation rate, over many draws.
        let mut rng = StdRng::seed_from_u64(99);
        let mut g = parent.clone();
        for _ in 0..2000 {
            g = g.mutate(&mut rng, f32::INFINITY);
            assert!(g.is_bounded(), "mutation escaped its bounds: {g:?}");
        }
        // Mutation never flips the strategy identity: an expressed pathway stays expressed and a
        // legacy genotype can never mutate itself into a free pathway.
        assert!(g.expressed);
        let mut legacy = EnergyPathwayGenotype::legacy();
        for _ in 0..500 {
            legacy = legacy.mutate(&mut rng, 1.0);
            assert!(!legacy.expressed);
            assert_eq!(legacy, EnergyPathwayGenotype::legacy());
        }
    }

    #[test]
    fn ae302_crossover_is_replay_deterministic_and_bounded() {
        let a = sample_pathway();
        let b = EnergyPathwayGenotype::new(
            EnergySourceId::new("arcane_flux"),
            0.1,
            0.9,
            9.0,
            0.05,
            0.9,
            0.4,
            0.9,
        );
        let mut r1 = StdRng::seed_from_u64(11);
        let mut r2 = StdRng::seed_from_u64(11);
        let c1 = EnergyPathwayGenotype::crossover(&a, &b, &mut r1)
            .expect("matching source ids may recombine");
        let c2 = EnergyPathwayGenotype::crossover(&a, &b, &mut r2)
            .expect("matching source ids may recombine");
        assert_eq!(c1, c2, "same seed must reproduce the same recombinant");
        assert!(c1.is_bounded());
        // Every trait comes from one of the two parents (uniform crossover, no invention).
        assert!(
            c1.sensing_affinity == a.sensing_affinity || c1.sensing_affinity == b.sensing_affinity
        );
        assert!(c1.uptake_rate == a.uptake_rate || c1.uptake_rate == b.uptake_rate);
        assert!(
            c1.maintenance_cost == a.maintenance_cost || c1.maintenance_cost == b.maintenance_cost
        );
        // Crossing two legacy genotypes can never produce an expressed pathway.
        let mut r3 = StdRng::seed_from_u64(3);
        let legacy = EnergyPathwayGenotype::crossover(
            &EnergyPathwayGenotype::legacy(),
            &EnergyPathwayGenotype::legacy(),
            &mut r3,
        )
        .expect("two legacy genotypes are compatible");
        assert_eq!(legacy, EnergyPathwayGenotype::legacy());
    }

    #[test]
    fn ae302_crossover_rejects_incompatible_source_ids() {
        let a = sample_pathway();
        let mut b = sample_pathway();
        b.source_id = EnergySourceId::new("geothermal_vent");
        let mut rng = StdRng::seed_from_u64(11);

        assert!(
            EnergyPathwayGenotype::crossover(&a, &b, &mut rng).is_none(),
            "traits tuned to different energy sources must not be recombined"
        );
    }

    // ---- AE-303: one-time development ---------------------------------------------------------

    #[test]
    fn ae303_development_materializes_capacities_once_from_the_genotype() {
        let g = sample_pathway();
        let d = DevelopedEnergyPathway::develop(&g);
        assert_eq!(d.source_id, g.source_id);
        assert!(d.storage_capacity > 0.0);
        assert!(d.uptake_surface > 0.0);
        assert!(d.maintenance_cost >= g.maintenance_cost as f64);
        // Development is a pure function of the genotype: repeating it changes nothing (restore and
        // migration re-use a materialized phenotype rather than re-developing from environment).
        assert_eq!(DevelopedEnergyPathway::develop(&g), d);

        // A legacy genotype develops into a zero-capacity, zero-cost phenotype.
        let legacy = DevelopedEnergyPathway::develop(&EnergyPathwayGenotype::legacy());
        assert_eq!(legacy.storage_capacity, 0.0);
        assert_eq!(legacy.uptake_surface, 0.0);
        assert_eq!(legacy.maintenance_cost, 0.0);
        assert!(!legacy.expressed);
    }

    // ---- AE-304: runtime exotic state ---------------------------------------------------------

    #[test]
    fn ae304_runtime_state_starts_empty_and_records_transactions() {
        let s = ExoticEnergyState::default();
        assert_eq!(s.stored_mu, 0.0);
        assert_eq!(s.last_uptake_mu, 0.0);
        assert_eq!(s.last_spent_mu, 0.0);
        assert_eq!(s.toxicity_load, 0.0);
    }

    // ---- AE-305/306: transactional physiology, explicit cost, finite performance --------------

    use crate::core::exotic_energy::{ExoticEnergyField, ExoticEnergyLaw};

    fn rich_field() -> ExoticEnergyField {
        ExoticEnergyField::from_law(&ExoticEnergyLaw::mana_uniform(400.0), 8, 8, 1).unwrap()
    }

    /// A pathway cohort with a real uptake/efficiency and a positive cost.
    fn pathway_cohort(count: f64) -> PathwayCohort {
        PathwayCohort::new(sample_pathway(), count)
    }

    #[test]
    fn ae305_uptake_is_ledger_exact_between_field_and_storage() {
        let mut field = rich_field();
        let mut c = pathway_cohort(50.0);
        let before_field = field.total();
        let moved = c.uptake(&mut field);
        let after_field = field.total();

        assert!(moved > 0.0, "a sensing, expressed pathway must take MU");
        assert!(
            ((before_field - after_field) - moved).abs() < 1e-9,
            "field decrease {} must equal storage gain {moved}",
            before_field - after_field
        );
        assert_eq!(c.state.stored_mu, moved);
        assert_eq!(c.state.last_uptake_mu, moved);
        // Storage is capped by the developed capacity — repeated uptake never exceeds it.
        for _ in 0..50 {
            c.uptake(&mut field);
        }
        assert!(
            c.state.stored_mu <= c.storage_capacity_total() + 1e-9,
            "stored {} exceeded capacity {}",
            c.state.stored_mu,
            c.storage_capacity_total()
        );
    }

    #[test]
    fn ae305_uptake_is_impossible_without_expression_sensing_or_a_matching_source() {
        let mut field = rich_field();

        // Legacy strategy: no pathway, no uptake, no state change.
        let mut legacy = PathwayCohort::new(EnergyPathwayGenotype::legacy(), 50.0);
        assert_eq!(legacy.uptake(&mut field), 0.0);
        assert_eq!(legacy.state.stored_mu, 0.0);

        // Expressed but blind (sensing_affinity = 0): the mechanism is off, so nothing moves.
        let mut blind = PathwayCohort::new(
            EnergyPathwayGenotype::new(
                EnergySourceId::new("arcane_flux"),
                0.0,
                0.05,
                0.5,
                0.6,
                0.5,
                0.01,
                0.2,
            ),
            50.0,
        );
        assert_eq!(blind.uptake(&mut field), 0.0);

        // A pathway tuned to a DIFFERENT source cannot feed on this field (no silent coercion).
        let mut wrong_source = PathwayCohort::new(
            EnergyPathwayGenotype::new(
                EnergySourceId::new("geothermal_vent"),
                0.9,
                0.05,
                0.5,
                0.6,
                0.5,
                0.01,
                0.2,
            ),
            50.0,
        );
        let before = field.total();
        assert_eq!(wrong_source.uptake(&mut field), 0.0);
        assert_eq!(field.total(), before);
    }

    #[test]
    fn ae305_spend_books_mu_as_dissipated_and_keeps_the_ledger_closed() {
        let mut field = rich_field();
        let initial = field.total();
        let mut c = pathway_cohort(50.0);
        let mut dissipated = 0.0;

        let taken = c.uptake(&mut field);
        let spent = c.metabolize(&mut dissipated);
        assert!(spent > 0.0, "stored MU must actually be spent");
        assert_eq!(dissipated, spent, "every spent MU is booked as a sink");
        assert_eq!(c.state.last_spent_mu, spent);
        assert!((c.state.stored_mu - (taken - spent)).abs() < 1e-12);

        // initial == field + storage + dissipated (nothing minted, nothing lost).
        let closure = field.total() + c.state.stored_mu + dissipated;
        assert!(
            (initial - closure).abs() / initial.max(1.0) < 1e-9,
            "MU ledger did not close: initial {initial} vs {closure}"
        );
    }

    #[test]
    fn ae306_maintenance_cost_is_paid_even_when_no_mu_exists() {
        // AE-S06 at the cohort level: with no field at all, a costly pathway performs strictly
        // WORSE than legacy. Cost is unconditional; benefit is not.
        let base = 1.0;
        let mut dissipated = 0.0;

        let mut legacy = PathwayCohort::new(EnergyPathwayGenotype::legacy(), 50.0);
        legacy.metabolize(&mut dissipated);
        let legacy_perf = legacy.update_performance(base);

        let mut costly = pathway_cohort(50.0);
        costly.metabolize(&mut dissipated);
        let costly_perf = costly.update_performance(base);

        assert_eq!(legacy_perf, base, "legacy pays nothing and gains nothing");
        assert!(
            costly_perf < legacy_perf,
            "a costly pathway with no source must be a net loss: {costly_perf} vs {legacy_perf}"
        );
        assert_eq!(dissipated, 0.0, "no MU existed, so none was spent");
    }

    #[test]
    fn ae307_performance_gain_requires_a_real_spend_not_mere_source_presence() {
        // AE-S07 at the cohort level: the benefit is produced by the uptake→spend transaction, so
        // disabling uptake OR utilization removes it even though the field is identical.
        let base = 1.0;
        let run = |g: EnergyPathwayGenotype| -> (f64, f64) {
            let mut field = rich_field();
            let mut dissipated = 0.0;
            let mut c = PathwayCohort::new(g, 50.0);
            c.uptake(&mut field);
            let spent = c.metabolize(&mut dissipated);
            (spent, c.update_performance(base))
        };

        let (spent_full, perf_full) = run(sample_pathway());
        assert!(spent_full > 0.0);
        assert!(
            perf_full > base,
            "an efficient pathway on a rich field must beat the base rate"
        );

        // Uptake disabled (rate 0): nothing is taken, nothing is spent, cost still paid.
        let mut no_uptake = sample_pathway();
        no_uptake.uptake_rate = 0.0;
        let (spent_none, perf_none) = run(no_uptake);
        assert_eq!(spent_none, 0.0);
        assert!(perf_none < base, "cost remains without any uptake");

        // Utilization disabled (efficiency 0): MU is still taken and spent, but converts to nothing.
        let mut no_use = sample_pathway();
        no_use.utilization_efficiency = 0.0;
        let (spent_wasted, perf_wasted) = run(no_use);
        assert!(spent_wasted > 0.0, "MU still moves through the transaction");
        assert!(
            perf_wasted < perf_full,
            "zero efficiency must remove the advantage: {perf_wasted} vs {perf_full}"
        );
        assert!(perf_wasted < base, "and leave only the cost");
    }

    #[test]
    fn ae306_performance_is_finite_and_non_negative_under_hostile_inputs() {
        let mut c = pathway_cohort(50.0);
        for base in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 0.0] {
            let p = c.update_performance(base);
            assert!(p.is_finite(), "performance must be finite for base {base}");
            assert!(p >= 0.0, "performance must be non-negative for base {base}");
        }
        // An empty cohort cannot divide by zero into a NaN performance.
        let mut empty = pathway_cohort(0.0);
        let p = empty.update_performance(1.0);
        assert!(p.is_finite() && p >= 0.0);
    }

    // ---- AE-307: opt-in population, generations, reproduction-only frequency change -----------

    use crate::core::sim_clock::ECOLOGY_PERIOD;

    fn base_config() -> ReferencePopulationConfig {
        ReferencePopulationConfig {
            total: 100.0,
            capacity: 100.0,
            pathway_fraction: 0.5,
            generation_ticks: ECOLOGY_PERIOD * 10,
            base_performance: 1.0,
            mutation_rate: 0.02,
            genotype: sample_pathway(),
        }
    }

    #[test]
    fn ae307_population_config_rejects_impossible_states_structurally() {
        assert!(base_config().validate().is_ok());

        let bad = |mutate: fn(&mut ReferencePopulationConfig)| {
            let mut c = base_config();
            mutate(&mut c);
            c.validate()
        };
        assert!(bad(|c| c.total = f64::NAN).is_err(), "NaN total");
        assert!(bad(|c| c.total = -1.0).is_err(), "negative total");
        assert!(
            bad(|c| c.total = 0.0).is_err(),
            "an enabled population needs individuals"
        );
        assert!(bad(|c| c.capacity = -5.0).is_err(), "negative capacity");
        assert!(bad(|c| c.capacity = 10.0).is_err(), "capacity below total");
        assert!(bad(|c| c.pathway_fraction = 1.5).is_err(), "fraction > 1");
        assert!(bad(|c| c.pathway_fraction = -0.1).is_err(), "fraction < 0");
        assert!(
            bad(|c| c.pathway_fraction = f64::NAN).is_err(),
            "NaN fraction"
        );
        assert!(
            bad(|c| c.generation_ticks = 0).is_err(),
            "a zero cadence never fires"
        );
        assert!(
            bad(|c| c.generation_ticks = ECOLOGY_PERIOD + 1).is_err(),
            "a cadence off the ecology band would never coincide with a physiology firing"
        );
        assert!(
            bad(|c| c.base_performance = f64::INFINITY).is_err(),
            "non-finite base"
        );
        assert!(bad(|c| c.base_performance = -1.0).is_err(), "negative base");
        assert!(
            bad(|c| c.mutation_rate = f32::NAN).is_err(),
            "NaN mutation rate"
        );
        assert!(
            bad(|c| c.genotype = EnergyPathwayGenotype::legacy()).is_err(),
            "the pathway strategy must actually express a pathway"
        );
    }

    #[test]
    fn ae310_physiology_ticks_cannot_change_frequency_generation_or_genotype() {
        // AE-S10, negative half: sensing, uptake, spend, cost and performance accounting are all
        // forbidden from touching composition. Only reproduction may.
        let mut field = rich_field();
        let mut pop = ReferencePopulation::new(&base_config(), 4242).unwrap();
        let mut dissipated = 0.0;

        let freq0 = pop.pathway_frequency();
        let gen0 = pop.generation;
        let g0 = pop.pathway.genotype.clone();
        let counts0 = (pop.legacy.count, pop.pathway.count);

        for _ in 0..9 {
            pop.step_physiology(Some(&mut field), &mut dissipated);
        }

        assert!(pop.cum_uptake() > 0.0, "the mechanism actually ran");
        assert!(pop.cum_spent() > 0.0);
        assert_eq!(pop.pathway_frequency(), freq0, "frequency must not drift");
        assert_eq!(pop.generation, gen0, "generation must not advance");
        assert_eq!(pop.pathway.genotype, g0, "genotype must not be rewritten");
        assert_eq!((pop.legacy.count, pop.pathway.count), counts0);
        assert_eq!(pop.births, 0.0, "no births without a reproduction event");
    }

    #[test]
    fn ae310_reproduction_delta_equals_the_resolved_offspring_composition() {
        // AE-S10, positive half: a reproduction event is the ONLY place frequency moves, and the
        // recorded delta is exactly what the offspring composition says.
        let mut field = rich_field();
        let mut pop = ReferencePopulation::new(&base_config(), 4242).unwrap();
        let mut dissipated = 0.0;
        for _ in 0..9 {
            pop.step_physiology(Some(&mut field), &mut dissipated);
        }

        let before = pop.pathway_frequency();
        let out = pop.reproduce(&mut dissipated);

        assert_eq!(out.generation, 1);
        assert!(out.births > 0.0, "a reproduction event produces offspring");
        assert_eq!(pop.births, out.births);
        assert_eq!(out.frequency_before, before);
        assert_eq!(out.frequency_after, pop.pathway_frequency());
        assert!(
            (out.delta - (out.frequency_after - out.frequency_before)).abs() < 1e-12,
            "the recorded delta must be the resolved change"
        );
        // The delta is literally the offspring composition, not an independently-written number.
        let resolved = out.pathway_offspring / (out.legacy_offspring + out.pathway_offspring);
        assert!((resolved - out.frequency_after).abs() < 1e-12);
        assert!((out.legacy_offspring + out.pathway_offspring - out.births).abs() < 1e-12);
        assert!(pop.pathway_frequency() >= 0.0 && pop.pathway_frequency() <= 1.0);
    }

    #[test]
    fn ae307_reproduction_releases_stored_mu_so_the_ledger_stays_closed() {
        let mut field = rich_field();
        let initial = field.total();
        let mut pop = ReferencePopulation::new(&base_config(), 7).unwrap();
        let mut dissipated = 0.0;
        for _ in 0..9 {
            pop.step_physiology(Some(&mut field), &mut dissipated);
        }
        assert!(pop.total_stored() > 0.0, "MU is held before the boundary");
        pop.reproduce(&mut dissipated);
        assert_eq!(
            pop.total_stored(),
            0.0,
            "generational replacement releases parental storage"
        );
        let closure = field.total() + pop.total_stored() + dissipated;
        assert!(
            (initial - closure).abs() / initial.max(1.0) < 1e-9,
            "MU ledger did not close across a reproduction event: {initial} vs {closure}"
        );
    }

    #[test]
    fn ae302_population_variation_replays_deterministically_from_its_own_seed() {
        let run = |seed: u64| {
            let mut field = rich_field();
            let mut pop = ReferencePopulation::new(&base_config(), seed).unwrap();
            let mut dissipated = 0.0;
            let mut freqs = Vec::new();
            for _ in 0..3 {
                for _ in 0..9 {
                    pop.step_physiology(Some(&mut field), &mut dissipated);
                }
                pop.reproduce(&mut dissipated);
                freqs.push(pop.pathway_frequency());
            }
            (freqs, pop.pathway.genotype.clone())
        };
        assert_eq!(run(2026), run(2026), "same seed must replay exactly");
        assert_ne!(
            run(2026).0,
            run(9001).0,
            "a different seed must give a different trajectory"
        );
    }

    #[test]
    fn ae306_absent_source_drives_a_costly_pathway_down_not_up() {
        // AE-S06 at population level: with NO field at all, the costly strategy must lose ground.
        let mut pop = ReferencePopulation::new(&base_config(), 55).unwrap();
        let mut dissipated = 0.0;
        let start = pop.pathway_frequency();
        for _ in 0..12 {
            for _ in 0..9 {
                pop.step_physiology(None, &mut dissipated);
            }
            pop.reproduce(&mut dissipated);
        }
        assert!(
            pop.pathway_frequency() < start,
            "a costly pathway with no source must not increase: {} vs {start}",
            pop.pathway_frequency()
        );
        assert_eq!(pop.cum_uptake(), 0.0, "no source means no uptake");
        assert_eq!(dissipated, 0.0, "and nothing to dissipate");
    }
}
