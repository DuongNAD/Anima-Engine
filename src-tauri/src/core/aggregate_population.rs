//! The aggregate tier of simulation LOD: distant individuals become per-chunk population
//! statistics, and are re-hydrated as individuals when an observer comes back.
//!
//! [`crate::core::simulation_lod`] is tier one. It decides how *often* an agent thinks, and a `Cold`
//! agent still exists — it holds its brain, keeps its CPG parameters, and goes on moving and
//! metabolising. That buys CPU. It does not buy memory, and memory is what gate **EB-S12** found the
//! population ceiling to actually be: ~22.5 KiB per agent, of which the brain genome is the bulk.
//!
//! This module buys the memory. A dormant agent is **destroyed**: entity, components, brain and all.
//! What remains is a number in a chunk.
//!
//! **How much memory, and when.** Not unconditionally — the saving depends on how crowded the chunk
//! is, and the honest version is worth stating before the design rather than after:
//!
//! | Cohort size | What the archive keeps | Saving |
//! |---|---|---|
//! | ≤ [`ARCHIVE_CAP`] | every genome | the ECS body and the `learned` network — roughly a factor of two, no more |
//! | > [`ARCHIVE_CAP`] | [`ARCHIVE_CAP`] genomes | unbounded: dormant memory becomes O(chunks), not O(agents) |
//!
//! So a handful of agents wandering out of view costs about what it did. The ceiling only moves for
//! populations large enough that a chunk exceeds the cap, which is exactly the regime the
//! million-agent target lives in — and exactly the regime where dormancy is lossy. The two facts are
//! the same fact. Both rows are pinned by tests
//! (`below_the_cap_the_saving_is_the_body_and_the_learned_network_not_the_genome`,
//! `above_the_cap_the_archive_stops_growing_with_the_population`).
//!
//! # The two things that must not break
//!
//! ## 1. Energy is conserved, and it never leaves the animal compartment
//!
//! A dormant animal is still an animal. Its energy has not been eaten, respired or returned to
//! detritus — it has merely stopped being individually resolved. So there is **no fourth
//! compartment** and **no ledger transaction**: `Compartment::Animals` simply has two stores instead
//! of one, and [`ecosystem_census_system`](crate::core::environmental_systems::ecosystem_census_system)
//! sums both.
//!
//! That is the whole conservation story, and it is deliberately one line in one place. The
//! alternative — banking a dormant agent's reserve into detritus — conserves EU just as well and is
//! ecologically wrong: walking away from a herd would fertilise the world.
//!
//! ## 2. The move and the despawn share a sync point
//!
//! [`ReclaimAndDespawnAgentCommand`](crate::core::agent_systems::ReclaimAndDespawnAgentCommand)
//! documents an order-dependent leak that cost real debugging time: energy banked in a system but
//! despawned through `Commands` leaves a window in which a body is alive holding a reserve that has
//! already been counted somewhere else. Dehydration is exactly that shape, so it is a
//! [`Command`](bevy_ecs::system::Command) too — [`DehydrateAgentCommand`] — and nothing can observe
//! a reserve that has been absorbed but not yet destroyed.
//!
//! # What dormancy costs, stated rather than hidden
//!
//! ## Genetic diversity, bounded and counted
//!
//! A chunk keeps at most [`ARCHIVE_CAP`] genomes. Storing every dormant genome would defeat the
//! entire point — a million genomes at ~6 KiB is ~6 GB whether or not the entities exist — so the
//! cap is forced by arithmetic, not chosen for convenience.
//!
//! The archive is a **uniform random sample** (reservoir sampling, algorithm R) of the individuals
//! the chunk absorbed, not "the last few to arrive". That matters: an arrival-ordered archive would
//! make dormancy select on arrival time, which is an artifact with no biological meaning. A uniform
//! sample instead imposes a *bounded, describable* one:
//!
//! > **A dormant cohort has an effective population size of [`ARCHIVE_CAP`].** Drift runs faster
//! > there than in the live population, and diversity above the cap is gone permanently.
//!
//! [`DormantCohorts::genomes_dropped`] counts exactly how many genomes that cost, so the price is a
//! number a run can report rather than a caveat in a doc comment.
//!
//! ## The lossless regime, and where it ends
//!
//! A cohort that never held more than [`ARCHIVE_CAP`] individuals round-trips its genomes
//! **exactly** — release removes from the archive rather than sampling it, so the same genomes come
//! back. Above the cap it is lossy by construction. The boundary is not incidental, it is a test
//! (`a_small_cohort_round_trips_its_genomes_exactly`).
//!
//! Energy is pooled either way, so individual variation in reserves does not survive: everyone comes
//! back at the cohort mean. Exact in total, lossy per individual.
//!
//! # Time passes where nobody is looking
//!
//! [`dormant_cohort_ecology_system`] runs one tick of ecology for every cohort: metabolism, and
//! grazing for the herbivores among them. Without it a dormant region would be a freezer and an
//! observer's route through the world would decide which populations age.
//!
//! The governing constraint is not "model the ecology well" — it is **do not become a second,
//! different ecology**, because any divergence between the two models is the observer's attention
//! leaking into the biology. Two consequences, both load-bearing:
//!
//! - **Metabolism is measured, not modelled.** A cohort burns the rate its members were *observed*
//!   burning while they were live bodies, taken from `FeatureTracker`. A modelled maintenance-only
//!   rate would have been the obvious thing to write and would have made sleeping cheaper than being
//!   watched, so unobserved regions would quietly support larger populations.
//! - **No mortality.** A live agent at zero energy is not despawned — `update_agent_evaluation_system`
//!   simply stops counting it. So a starving cohort bottoms out at zero and keeps its members.
//!   Density-dependent death would look like richer ecology and would in fact be the observer
//!   choosing who dies.
//!
//! What is genuinely coarser, and one asymmetry that is genuinely still open:
//!
//! - A dormant herd is not resolved to a position inside its chunk, so it grazes the chunk's cells
//!   in proportion to what each holds rather than choosing among them. The giving-up-density
//!   dispersal that live herbivores show has no aggregate counterpart.
//! - **There is no aggregate predation.** In the live world `combat_system` moves EU from prey to
//!   predator and sheds the rest to detritus at the Lindeman efficiency, so a live food chain leaks
//!   energy downward. A dormant cohort pools predator and prey reserves into one number, so that
//!   transfer is a no-op and the Lindeman loss never happens — a chunk holding both classes
//!   conserves energy slightly *better* asleep than awake. The direction is known and the magnitude
//!   is bounded by the predation rate; it is left open rather than filled with an invented encounter
//!   model, because a wrong encounter rate would be a worse observer-dependence than the one it fixed.
//!   Cohorts of a single trophic class — the common case — are unaffected.
//!
//! ## The trap that aggregation sets
//!
//! Worth stating separately, because it ran perfectly, returned finite numbers and was wrong — the
//! failure mode `ADR-0003`'s hard rules describe.
//!
//! The obvious way to aggregate grazing is to hand [`herbivore_intake`](crate::core::ecology::herbivore_intake)
//! the chunk's **summed** standing resource and the cohort's summed appetite. It type-checks, it
//! conserves energy exactly, and every conservation test passes. But Holling Type II saturates in
//! the *density* it is given: a sum over sixty-odd cells sits far deeper into saturation than any
//! single cell a live agent stands on, so a dormant herd feeds better than a watched one. Left in,
//! unobserved regions would quietly support larger populations — the observer's attention as an
//! ecological variable, arriving through arithmetic rather than through a design decision.
//!
//! The functional response is per capita, so it is applied per capita: each dormant individual
//! grazes as though standing on an **average cell** of its chunk, and only then is it multiplied by
//! the herbivore count. `sleeping_is_not_cheaper_than_being_watched` is what caught it, and is what
//! holds it.
//!
//! # Not yet persisted — the one hard precondition
//!
//! [`SavedSimulationState`](crate::core::simulation_state::SavedSimulationState) is careful about
//! closed energy: it carries the detritus/plants/animals scalars, every resource-field cell and the
//! RNG's draw position, precisely so a save/load boundary neither creates nor destroys EU. It does
//! **not** carry [`DormantCohorts`].
//!
//! So saving a run with anything asleep silently deletes that population and its energy: the dormant
//! individuals are not in `agents`, their EU is in no scalar, and the reloaded world simply has less
//! of it. Nothing detects this, because a fresh baseline locks on the first census after the load.
//!
//! **Therefore: do not enable dormancy on a run that saves, until the cohorts are in the snapshot
//! envelope.** That is safe today only because nothing inserts the resource — the tier is off in
//! every shipped path, and the UI focus that would switch it on does not exist yet. Whoever wires
//! that focus up owns this line.
//!
//! # Default is off
//!
//! Every system here takes [`DormantCohorts`] as an `Option`, and the resource is not inserted by
//! default. Absent it, no agent is ever dehydrated and the census sums live agents exactly as
//! before. Allocation note: dehydration and re-hydration allocate (the archive, the command queue),
//! the same as spawn and despawn already do. They are transition events, not the per-tick loop the
//! `allocs == 0` rule governs.

use crate::core::components::AgentClass;
use crate::evolution::brain_genotype::BrainGenotype;
use crate::evolution::genotype::MorphologyGenotype;
use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::Arc;

/// Chunks per side of the aggregate grid.
///
/// Far coarser than the terrain grid on purpose. Cohorts are pre-allocated one per chunk, so the
/// grid is a fixed memory cost paid whether or not anything is dormant: 32² chunks is ~100 KiB,
/// while one cohort per terrain cell at 256² would be ~7 MiB to hold mostly zeros. A chunk is also
/// the right granularity for the thing being modelled — a local population, not a square metre.
pub const AGGREGATE_GRID: usize = 32;

/// Genomes a single chunk may keep. See the module docs: this is the dormant cohort's effective
/// population size, and the reason dormancy is lossy at all.
pub const ARCHIVE_CAP: usize = 8;

/// Consecutive ticks an agent must be `Cold` before it is dehydrated.
///
/// Hysteresis, and not a small detail. Without it an agent that brushes the far edge of the warm
/// band loses its brain on the first tick it crosses, and a focus that pans back and forth would
/// grind the population's genetic diversity down to [`ARCHIVE_CAP`] per chunk in seconds — an
/// observer's camera movement driving evolution. Two seconds at 60 Hz.
pub const DORMANCY_DWELL_TICKS: u32 = 120;

/// Individuals re-hydrated per tick, across all chunks.
///
/// Re-hydration spawns a full body, which is expensive. Waking a chunk holding hundreds of dormant
/// individuals in one tick would stall the frame precisely when an observer arrived to look at it,
/// so the wake-up is spread out. The population comes back over a second or two rather than instantly.
pub const REHYDRATION_PER_TICK: usize = 4;

/// The dormancy RNG's stream id. Its own stream, never the ecology or evolution one, so a run with
/// dormancy enabled draws an identical sequence in every other system to one without it.
pub const DORMANCY_STREAM: u64 = 7;

/// Marks how long an agent has been continuously `Cold`.
///
/// Inserted when an agent first goes cold and removed when it warms, so it costs an archetype move
/// only on a tier transition rather than a component on every agent forever.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct DormancyWatch {
    pub cold_ticks: u32,
}

/// One archived individual: enough to build a body again, and nothing more.
#[derive(Clone, Debug)]
pub struct ArchivedIndividual {
    pub morphology: MorphologyGenotype,
    /// The brain the individual actually had.
    ///
    /// Invariant **D01** of ADR-0003 says restore and migration carry the brain they were given and
    /// never roll a new one. Re-hydration is a restore, so it carries an archived brain. It may be a
    /// *different individual's* archived brain once the cohort has exceeded [`ARCHIVE_CAP`] — that
    /// is the sampling loss above — but it is never freshly rolled.
    pub brain: Option<Arc<BrainGenotype>>,
    pub class: AgentClass,
    pub lineage_id: String,
    pub generation: u32,
}

impl ArchivedIndividual {
    /// Heap footprint of this archive entry, for the memory gate.
    pub fn heap_bytes(&self) -> usize {
        self.brain.as_ref().map_or(0, |b| b.heap_bytes()) + self.lineage_id.len()
    }
}

/// What one individual contributes to, and takes back from, a cohort's pooled ecology.
///
/// Pooled rather than kept per individual — that is the whole point of the tier — so everything
/// here is a quantity that sums meaningfully across a population.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DormantVitals {
    /// Reserve, in EU. The conserved quantity.
    pub energy: f64,
    /// Hydration. Not conserved, so it is pooled without ceremony.
    pub hydration: f64,
    /// The individual's satiation ceiling (`HomeostaticState::energy_target`). Summed, it is the
    /// cohort's appetite: what it would eat up to if food allowed.
    pub energy_cap: f64,
    /// EU the individual was burning per tick when it went to sleep. See
    /// [`Cohort::respire`] for why this is measured off the live run rather than modelled afresh.
    pub burn_per_tick: f64,
    /// Only herbivores graze, matching `herbivore_grazing_system`'s `With<Prey>` filter.
    pub is_prey: bool,
}

/// The dormant population of one chunk.
///
/// Every field is a **sum** over members, not a mean, so absorbing and releasing an individual is
/// an exact addition and subtraction rather than a re-derived average.
#[derive(Clone, Debug, Default)]
pub struct Cohort {
    /// How many dormant individuals the chunk holds.
    pub count: u32,
    /// How many of them are herbivores.
    pub prey_count: u32,
    /// Their total energy, in EU. Authoritative — this is animal energy that the census must see.
    pub energy: f64,
    /// Their total hydration. Not a conserved quantity, so it is pooled without ceremony.
    pub hydration: f64,
    /// Sum of members' satiation ceilings. The cohort stops eating here.
    pub energy_cap: f64,
    /// Sum of members' per-tick metabolic burn, in EU/tick.
    pub burn_rate: f64,
    /// How many individuals have been absorbed since the cohort last emptied. The reservoir
    /// sampler's denominator.
    seen: u64,
    archive: Vec<ArchivedIndividual>,
}

impl Cohort {
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Mean energy of a dormant individual, or zero for an empty cohort.
    pub fn mean_energy(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.energy / self.count as f64
        }
    }

    /// Burn one tick of metabolism, returning the EU to hand to detritus.
    ///
    /// The rate is not modelled — it is the rate the members were **measured** burning while they
    /// were still live bodies, summed. That is deliberate, and it is the single most important
    /// choice in the aggregate model. A dormant animal is asleep to the renderer, not to its own
    /// biology: if it were charged maintenance only, dormancy would be metabolically cheaper than
    /// being watched, unobserved regions would quietly support larger populations, and the
    /// observer's attention would be an ecological variable. Charging what they were actually
    /// paying keeps the two models on the same footing.
    ///
    /// Floors at zero and **never kills**. `update_agent_evaluation_system` shows a live agent at
    /// zero energy simply stops accumulating — it is not despawned — so a dormant one must not be
    /// either. Adding starvation mortality here would be a dynamic the live world does not have,
    /// which is the same observer-dependence in a different disguise.
    pub fn respire(&mut self) -> f64 {
        if self.count == 0 || self.burn_rate <= 0.0 || self.energy <= 0.0 {
            return 0.0;
        }
        let burned = self.burn_rate.min(self.energy);
        self.energy -= burned;
        burned
    }

    /// How much this cohort's herbivores would eat this tick if the field could supply it.
    ///
    /// Appetite is the shortfall against the members' summed satiation ceilings, scaled to the
    /// herbivore share — predators do not graze, exactly as in the live system.
    pub fn grazing_appetite(&self) -> f64 {
        if self.prey_count == 0 || self.count == 0 {
            return 0.0;
        }
        let hunger = (self.energy_cap - self.energy).max(0.0);
        hunger * self.prey_count as f64 / self.count as f64
    }

    /// Genomes currently archived.
    pub fn archived(&self) -> usize {
        self.archive.len()
    }

    /// Heap held by this cohort's archive.
    pub fn heap_bytes(&self) -> usize {
        self.archive.iter().map(|a| a.heap_bytes()).sum::<usize>()
            + self.archive.capacity() * std::mem::size_of::<ArchivedIndividual>()
    }
}

/// Per-chunk dormant populations, and the policy for entering and leaving dormancy.
///
/// Insert this resource to switch the aggregate tier on. Absent it, nothing here runs.
#[derive(Resource)]
pub struct DormantCohorts {
    chunks: Vec<Cohort>,
    grid: usize,
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    /// See [`DORMANCY_DWELL_TICKS`].
    pub dwell_ticks: u32,
    /// See [`ARCHIVE_CAP`].
    pub archive_cap: usize,
    /// See [`REHYDRATION_PER_TICK`].
    pub rehydrate_per_tick: usize,
    rng: rand_chacha::ChaCha12Rng,
    dehydrated: u64,
    rehydrated: u64,
    genomes_dropped: u64,
    /// EU grazed out of the field that no cohort had room for, awaiting transfer to detritus.
    spilled: f64,
}

impl DormantCohorts {
    /// Build a grid spanning the given world bounds, seeded from the run seed.
    pub fn new(run_seed: u64, min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Self {
        use rand::SeedableRng;
        // Same mixing constant as `derived_rng`, but producing the ChaCha the resource owns rather
        // than a `StdRng`, so dormancy's draw position can be snapshotted with the rest of the world.
        let mixed = run_seed ^ DORMANCY_STREAM.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        Self {
            chunks: vec![Cohort::default(); AGGREGATE_GRID * AGGREGATE_GRID],
            grid: AGGREGATE_GRID,
            min_x,
            min_z,
            max_x,
            max_z,
            dwell_ticks: DORMANCY_DWELL_TICKS,
            archive_cap: ARCHIVE_CAP,
            rehydrate_per_tick: REHYDRATION_PER_TICK,
            rng: rand_chacha::ChaCha12Rng::seed_from_u64(mixed),
            dehydrated: 0,
            rehydrated: 0,
            genomes_dropped: 0,
            spilled: 0.0,
        }
    }

    /// Build one matching the world's map bounds.
    pub fn from_bounds(run_seed: u64, bounds: &crate::core::resources::MapBounds) -> Self {
        Self::new(
            run_seed,
            bounds.min.x,
            bounds.min.z,
            bounds.max.x,
            bounds.max.z,
        )
    }

    /// Chunk holding a world position, or `None` outside the grid.
    #[inline]
    pub fn chunk_index(&self, x: f32, z: f32) -> Option<usize> {
        if self.grid == 0 || self.max_x <= self.min_x || self.max_z <= self.min_z {
            return None;
        }
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        if x < self.min_x || x > self.max_x || z < self.min_z || z > self.max_z {
            return None;
        }
        let u = (x - self.min_x) / (self.max_x - self.min_x);
        let v = (z - self.min_z) / (self.max_z - self.min_z);
        let col = ((u * self.grid as f32) as usize).min(self.grid - 1);
        let row = ((v * self.grid as f32) as usize).min(self.grid - 1);
        Some(row * self.grid + col)
    }

    /// World-space centre of a chunk. Re-hydration tiers a whole chunk by this point, and spawns
    /// its individuals here — an individual's own position is not archived, because a position per
    /// dormant agent is the kind of per-individual storage this tier exists to stop keeping.
    pub fn chunk_center(&self, index: usize) -> Vec3 {
        let row = index / self.grid;
        let col = index % self.grid;
        let w = (self.max_x - self.min_x) / self.grid as f32;
        let h = (self.max_z - self.min_z) / self.grid as f32;
        Vec3::new(
            self.min_x + (col as f32 + 0.5) * w,
            0.0,
            self.min_z + (row as f32 + 0.5) * h,
        )
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn cohort(&self, index: usize) -> Option<&Cohort> {
        self.chunks.get(index)
    }

    /// Total dormant energy, in EU. **This is the number the census must add to `pool.animals`.**
    pub fn total_energy(&self) -> f64 {
        self.chunks.iter().map(|c| c.energy).sum()
    }

    /// How many individuals are currently dormant.
    pub fn total_dormant(&self) -> u64 {
        self.chunks.iter().map(|c| c.count as u64).sum()
    }

    /// Cumulative individuals absorbed into dormancy.
    pub fn dehydrated(&self) -> u64 {
        self.dehydrated
    }

    /// Cumulative individuals brought back as bodies.
    pub fn rehydrated(&self) -> u64 {
        self.rehydrated
    }

    /// Genomes destroyed because a cohort was already at [`Self::archive_cap`].
    ///
    /// The diversity cost of dormancy, as a number rather than a caveat. A run that reports a large
    /// figure here has been trading evolutionary history for memory, and should either raise the cap
    /// or keep more of the world hot.
    pub fn genomes_dropped(&self) -> u64 {
        self.genomes_dropped
    }

    /// Heap held by every archive, for the memory gate.
    pub fn archive_heap_bytes(&self) -> usize {
        self.chunks.iter().map(|c| c.heap_bytes()).sum()
    }

    /// Take an individual into dormancy: its energy and hydration join the pool, and its genome
    /// enters the chunk's reservoir sample.
    ///
    /// Returns `false` if the position is off-grid, in which case the caller must **not** destroy
    /// the agent — nothing was absorbed and doing so would delete its energy.
    pub fn absorb(
        &mut self,
        x: f32,
        z: f32,
        vitals: DormantVitals,
        individual: ArchivedIndividual,
    ) -> bool {
        let Some(idx) = self.chunk_index(x, z) else {
            return false;
        };
        let cap = self.archive_cap;
        // Borrow the RNG out first: the reservoir decision needs it while `chunk` is borrowed.
        let roll = {
            use rand::Rng;
            let seen = self.chunks[idx].seen + 1;
            if self.chunks[idx].archive.len() < cap {
                None
            } else {
                // Algorithm R: the (seen)-th item survives with probability cap/seen, displacing a
                // uniformly chosen incumbent. The result is a uniform sample of everything absorbed.
                let keep = self.rng.gen_range(0..seen);
                if keep < cap as u64 {
                    Some(keep as usize)
                } else {
                    None
                }
            }
        };

        let chunk = &mut self.chunks[idx];
        chunk.count += 1;
        if vitals.is_prey {
            chunk.prey_count += 1;
        }
        chunk.energy += vitals.energy;
        chunk.hydration += vitals.hydration;
        chunk.energy_cap += vitals.energy_cap;
        chunk.burn_rate += vitals.burn_per_tick;
        chunk.seen += 1;

        if chunk.archive.len() < cap {
            chunk.archive.push(individual);
        } else if let Some(slot) = roll {
            chunk.archive[slot] = individual;
            self.genomes_dropped += 1;
        } else {
            self.genomes_dropped += 1;
        }
        self.dehydrated += 1;
        true
    }

    /// Bring one individual back out of a chunk, if it holds any.
    ///
    /// Returns the archived individual to rebuild plus its share of the pooled energy and hydration
    /// (the cohort mean). While the cohort is small enough to be fully archived the entry is
    /// **removed**, so a cohort at or below [`Self::archive_cap`] round-trips its genomes exactly;
    /// a larger one is sampled with replacement, and the individuals that come back are clones drawn
    /// from the survivors of the reservoir.
    pub fn release(&mut self, index: usize) -> Option<(ArchivedIndividual, DormantVitals)> {
        let pick = {
            let chunk = self.chunks.get(index)?;
            if chunk.count == 0 || chunk.archive.is_empty() {
                return None;
            }
            if chunk.count as usize <= chunk.archive.len() {
                // Fully archived: hand back distinct individuals, oldest slot first, so the
                // round trip is deterministic and exact.
                None
            } else {
                use rand::Rng;
                Some(self.rng.gen_range(0..self.chunks[index].archive.len()))
            }
        };

        let chunk = &mut self.chunks[index];
        // Clamp the share to what the pool actually holds, and debit by exactly the clamped figure.
        // Handing out `mean` and debiting `max(0, energy - mean)` would look identical almost
        // always, and on the tick where rounding puts `mean` a hair above `energy` it would hand
        // out EU the cohort did not have — a one-way creation, which is precisely the shape of the
        // drift `step_regrowth_gated_strided` was rewritten to kill. What is given is what is taken.
        let n = chunk.count as f64;
        let share = |total: f64| (total / n).min(total.max(0.0));
        let vitals = DormantVitals {
            energy: share(chunk.energy),
            hydration: share(chunk.hydration),
            energy_cap: share(chunk.energy_cap),
            burn_per_tick: share(chunk.burn_rate),
            // Whether *this* individual grazes is its own archived trait, not a share of the pool.
            is_prey: matches!(
                pick.map_or_else(
                    || chunk.archive.first().map(|a| a.class),
                    |i| chunk.archive.get(i).map(|a| a.class),
                ),
                Some(AgentClass::Prey)
            ),
        };

        let individual = match pick {
            Some(i) => chunk.archive[i].clone(),
            None => chunk.archive.remove(0),
        };

        chunk.count -= 1;
        if vitals.is_prey {
            chunk.prey_count = chunk.prey_count.saturating_sub(1);
        }
        chunk.energy -= vitals.energy;
        chunk.hydration -= vitals.hydration;
        chunk.energy_cap -= vitals.energy_cap;
        chunk.burn_rate -= vitals.burn_per_tick;
        if chunk.count == 0 {
            // Rounding can leave a sliver behind. Reset `seen` so the next dormancy episode starts a
            // fresh reservoir rather than inheriting a denominator that would starve it of samples.
            chunk.seen = 0;
            chunk.prey_count = 0;
            chunk.energy_cap = 0.0;
            chunk.burn_rate = 0.0;
            chunk.archive.clear();
        }
        self.rehydrated += 1;
        Some((individual, vitals))
    }

    /// Energy left in an emptied cohort, to be returned to detritus by the caller.
    ///
    /// `count` reaching zero with a non-zero pool is a rounding remainder, not a bug — but it is EU,
    /// and EU that no body can embody is detritus. Draining it here keeps the census exact instead of
    /// leaving a permanent phantom in the animal compartment.
    pub fn drain_orphaned_energy(&mut self) -> f64 {
        let mut drained = 0.0;
        for chunk in self.chunks.iter_mut() {
            if chunk.count == 0 && chunk.energy != 0.0 {
                drained += chunk.energy;
                chunk.energy = 0.0;
                chunk.hydration = 0.0;
            }
        }
        drained
    }

    /// Burn one tick of metabolism across every cohort. Returns the total EU to hand to detritus.
    ///
    /// Allocation-free: a scan over the pre-allocated chunk vector, arithmetic only.
    pub fn respire_all(&mut self) -> f64 {
        let mut respired = 0.0;
        for chunk in self.chunks.iter_mut() {
            respired += chunk.respire();
        }
        respired
    }

    /// Graze every cohort's herbivores against the resource field beneath their chunk.
    ///
    /// Returns the EU actually moved out of the field and into the cohorts, which the caller
    /// subtracts from `plants` — the same contract `herbivore_grazing_system` honours, and for the
    /// same reason: `plants` is carried incrementally, so whatever leaves the field has to be
    /// reported rather than re-summed.
    ///
    /// Conservation is by measurement on both sides. The amount removed is read back from the
    /// stored `f32` cells in `f64`, and exactly that is credited to the cohort — so the rounding in
    /// `cell -= taken` cannot leak, which is the lesson `herbivore_grazing_system` already carries.
    ///
    /// Allocation-free: it walks the cells of occupied chunks in place.
    pub fn graze_all(&mut self, field: &mut crate::core::ecology::ResourceField, dt: f32) -> f64 {
        if dt <= 0.0 || field.width == 0 || field.height == 0 {
            return 0.0;
        }
        let mut grazed_total = 0.0f64;
        for i in 0..self.chunks.len() {
            let appetite = self.chunks[i].grazing_appetite();
            if appetite <= 0.0 {
                continue;
            }
            let prey = self.chunks[i].prey_count as f64;
            let Some((c0, c1, r0, r1)) = self.chunk_cells(i, field) else {
                continue;
            };

            let mut standing = 0.0f64;
            for r in r0..=r1 {
                for c in c0..=c1 {
                    standing += field.r[r * field.width + c].max(0.0) as f64;
                }
            }
            if standing <= 0.0 {
                continue;
            }

            // The functional response is applied **per capita, against the mean cell density**, and
            // only then multiplied up. Feeding `herbivore_intake` the chunk's summed resource was
            // the obvious way to write this and is wrong in a way that runs perfectly: Holling
            // Type II saturates in the *density* it is given, so a sum over sixty-odd cells sits far
            // deeper into saturation than any cell a live agent ever stands on, and a dormant herd
            // eats better than a watched one. That is observer-dependent ecology, which is the one
            // thing this whole tier is arranged against — and it is what
            // `sleeping_is_not_cheaper_than_being_watched` caught.
            //
            // Each dormant individual therefore grazes as though standing on an average cell of its
            // chunk, which reproduces the live per-agent response exactly when the field is uniform.
            let cells = ((r1 - r0 + 1) * (c1 - c0 + 1)) as f64;
            let mean_density = standing / cells;
            let per_capita_bite = crate::core::ecology::herbivore_intake(
                mean_density as f32,
                (appetite / prey) as f32,
                // The same ceiling `herbivore_grazing_system` gives one agent.
                8.0 * dt,
            ) as f64;
            let bite = (per_capita_bite * prey).min(standing);
            if bite <= 0.0 {
                continue;
            }

            // Take it proportionally to what each cell holds, so a depleted cell is not driven
            // negative and a rich one carries the load.
            let fraction = (bite / standing).min(1.0) as f32;
            let mut removed = 0.0f64;
            for r in r0..=r1 {
                for c in c0..=c1 {
                    let idx = r * field.width + c;
                    let before = field.r[idx];
                    if before <= 0.0 {
                        continue;
                    }
                    field.r[idx] = (before - before * fraction).max(0.0);
                    removed += (before - field.r[idx]) as f64;
                }
            }
            // Credit exactly what the field lost, and never past the cohort's satiation ceiling.
            let headroom = (self.chunks[i].energy_cap - self.chunks[i].energy).max(0.0);
            let landed = removed.min(headroom);
            self.chunks[i].energy += landed;
            grazed_total += removed;
            // Anything the cohort could not hold has still left the field, so it becomes detritus
            // rather than evaporating. Reported through the same return value the caller splits.
            self.spilled += removed - landed;
        }
        grazed_total
    }

    /// EU that left the resource field but no cohort could hold, awaiting transfer to detritus.
    ///
    /// Taken rather than read, so it cannot be banked twice.
    pub fn take_spilled(&mut self) -> f64 {
        std::mem::take(&mut self.spilled)
    }

    /// Inclusive `(col0, col1, row0, row1)` of the resource-field cells under a chunk.
    ///
    /// Derived from the chunk's world rectangle through the field's own bounds rather than by
    /// assuming the two grids divide evenly, so a resource field built over different extents still
    /// maps correctly instead of silently grazing the wrong cells.
    fn chunk_cells(
        &self,
        index: usize,
        field: &crate::core::ecology::ResourceField,
    ) -> Option<(usize, usize, usize, usize)> {
        if field.max_x <= field.min_x || field.max_z <= field.min_z {
            return None;
        }
        let row = index / self.grid;
        let col = index % self.grid;
        let w = (self.max_x - self.min_x) / self.grid as f32;
        let h = (self.max_z - self.min_z) / self.grid as f32;
        let x0 = self.min_x + col as f32 * w;
        let z0 = self.min_z + row as f32 * h;

        let to_col = |x: f32| {
            let u = ((x - field.min_x) / (field.max_x - field.min_x)).clamp(0.0, 1.0);
            ((u * field.width as f32) as usize).min(field.width - 1)
        };
        let to_row = |z: f32| {
            let v = ((z - field.min_z) / (field.max_z - field.min_z)).clamp(0.0, 1.0);
            ((v * field.height as f32) as usize).min(field.height - 1)
        };
        // Nudge inside the far edge so a chunk does not claim the first cell of its neighbour.
        let c0 = to_col(x0);
        let c1 = to_col(x0 + w - w * 1e-3).max(c0);
        let r0 = to_row(z0);
        let r1 = to_row(z0 + h - h * 1e-3).max(r0);
        Some((c0, c1, r0, r1))
    }

    /// Chunk indices holding dormant individuals whose centre is within `radius` of `center`.
    ///
    /// Writes into a caller-owned buffer so the per-tick wake-up scan allocates nothing.
    pub fn wakeable_into(&self, center: Vec3, radius: f32, out: &mut Vec<usize>) {
        out.clear();
        if !radius.is_finite() {
            return;
        }
        for (i, chunk) in self.chunks.iter().enumerate() {
            if chunk.count > 0 && self.chunk_center(i).distance(center) <= radius {
                out.push(i);
            }
        }
    }
}

// ---- ECS integration -------------------------------------------------------------------

/// Absorb an agent into its chunk's cohort and destroy it, as one indivisible step.
///
/// A `Command` for the same reason
/// [`ReclaimAndDespawnAgentCommand`](crate::core::agent_systems::ReclaimAndDespawnAgentCommand) is
/// one. Absorbing the reserve in a system but despawning through `Commands` would leave the agent
/// alive for the rest of the schedule holding energy the cohort had already counted, and
/// `ecosystem_census_system` sums both stores — so the world would gain one reserve of EU per
/// dehydration, at a rate depending on system ordering. Doing both here closes the window.
///
/// If the agent's position has no chunk, **nothing happens at all**: it is not absorbed and not
/// destroyed. Destroying it would delete its energy outright.
pub struct DehydrateAgentCommand {
    /// The agent's root entity. Its segments are found by `ParentAgent` and go with it.
    pub root: Entity,
}

impl bevy_ecs::system::Command for DehydrateAgentCommand {
    fn apply(self, world: &mut World) {
        let Some(pos) = world
            .get::<crate::core::components::Position>(self.root)
            .map(|p| p.0)
        else {
            return;
        };
        let Some(genotype) = world
            .get::<crate::core::agent_systems::AgentGenotype>(self.root)
            .map(|g| g.0.clone())
        else {
            // No morphology means nothing could be rebuilt, so dormancy would be a one-way
            // deletion. Leave the agent alone rather than lose it.
            return;
        };

        // ADR-0003, no Lamarck: `learned` is runtime state that dies with the individual and is
        // never written back into the genome. Dormancy destroys the body, so what it learned goes
        // with it — carrying `learned` across would be inheritance of acquired characteristics by
        // the back door, and it is also the half of the memory this tier exists to reclaim.
        let brain = world
            .get::<crate::core::components::AgentBrain>(self.root)
            .map(|b| b.genotype.clone());

        let class = if world
            .get::<crate::core::components::Predator>(self.root)
            .is_some()
        {
            AgentClass::Predator
        } else {
            AgentClass::Prey
        };
        let lineage_id = world
            .get::<crate::core::agent_systems::AgentLineageId>(self.root)
            .map(|l| l.0.clone())
            .unwrap_or_default();
        let generation = world
            .get::<crate::core::agent_systems::AgentGeneration>(self.root)
            .map(|g| g.0)
            .unwrap_or(0);

        // The metabolic rate the cohort will burn on this individual's behalf is *measured*, not
        // modelled: `FeatureTracker` accumulates `total_cost * dt` every tick, so the mean per tick
        // is what the individual was actually paying — locomotion, cognition and all — up to the
        // moment it fell asleep. See `Cohort::respire` for why a modelled maintenance-only rate
        // would have made dormancy metabolically cheaper than being watched.
        //
        // `apply_staggered_evolution_system` resets the tracker at each epoch boundary, so this is
        // the mean over the current epoch rather than the whole life — which is the more useful of
        // the two, being closer to what the individual is doing now.
        //
        // An agent with no tracker history contributes nothing. Every live spawn path attaches one
        // (genesis, `SpawnGenotypeCommand`, restore), so in practice this is the freshly-reset case,
        // and it under-charges rather than over-charges: it cannot manufacture a metabolic cost the
        // live world never levied.
        let burn_per_tick = world
            .get::<crate::core::components::FeatureTracker>(self.root)
            .filter(|t| t.tick_count > 0)
            .map(|t| (t.cumulative_energy_decay as f64 / t.tick_count as f64).max(0.0))
            .unwrap_or(0.0);

        let vitals = match world.get::<crate::ai::hrrl::HomeostaticState>(self.root) {
            Some(h) => DormantVitals {
                energy: h.energy.max(0.0) as f64,
                hydration: h.hydration.max(0.0) as f64,
                energy_cap: h.energy_target.max(0.0) as f64,
                burn_per_tick,
                is_prey: class == AgentClass::Prey,
            },
            None => DormantVitals {
                is_prey: class == AgentClass::Prey,
                ..Default::default()
            },
        };

        let absorbed = world
            .get_resource_mut::<DormantCohorts>()
            .map(|mut cohorts| {
                cohorts.absorb(
                    pos.x,
                    pos.z,
                    vitals,
                    ArchivedIndividual {
                        morphology: genotype,
                        brain,
                        class,
                        lineage_id,
                        generation,
                    },
                )
            })
            .unwrap_or(false);

        if !absorbed {
            return;
        }

        let mut doomed: Vec<Entity> = Vec::new();
        let mut q = world.query::<(Entity, &crate::core::components::ParentAgent)>();
        for (entity, parent) in q.iter(world) {
            if parent.0 == self.root {
                doomed.push(entity);
            }
        }
        for entity in doomed {
            if let Some(e) = world.get_entity_mut(entity) {
                e.despawn();
            }
        }
        if let Some(e) = world.get_entity_mut(self.root) {
            e.despawn();
        }
    }
}

/// Rebuild one individual from a chunk's cohort.
///
/// Deliberately not a call into
/// [`SpawnGenotypeCommand`](crate::core::agent_systems::SpawnGenotypeCommand): that path funds the
/// new body out of **detritus**, which is right for evolutionary replacement (D06 — the predecessor's
/// reserve went there) and wrong here. A dormant individual's energy never left the animal
/// compartment, so re-hydration must draw it from the cohort. Routing through detritus would work
/// out conserved and be ecologically false, and would fail outright whenever the pool was empty —
/// an observer walking back to a herd would find it starved by the act of looking.
pub struct RehydrateCommand {
    pub chunk: usize,
}

impl bevy_ecs::system::Command for RehydrateCommand {
    fn apply(self, world: &mut World) {
        let (center, individual, vitals, orphaned) = {
            let Some(mut cohorts) = world.get_resource_mut::<DormantCohorts>() else {
                return;
            };
            let center = cohorts.chunk_center(self.chunk);
            let Some((individual, vitals)) = cohorts.release(self.chunk) else {
                return;
            };
            // A cohort that just emptied can be left holding a rounding sliver. It is EU, and the
            // census counts every cohort, so leaving it there would inflate the animal compartment
            // by a hair permanently — small, but a one-way ratchet, and invisible because the
            // residual would still look like noise. Drained at the one moment a cohort can empty.
            let orphaned = cohorts.drain_orphaned_energy();
            (center, individual, vitals, orphaned)
        };
        let (energy, hydration) = (vitals.energy, vitals.hydration);

        let entity = crate::evolution::genotype::decode_genotype(
            world,
            &individual.morphology,
            center,
            glam::Quat::IDENTITY,
        );
        world.entity_mut(entity).insert((
            crate::core::agent_systems::AgentGenotype(individual.morphology),
            crate::core::agent_systems::AgentEvaluation {
                start_position: center,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: center,
            },
            crate::core::components::FeatureTracker::default(),
            crate::core::agent_systems::AgentLineageId(individual.lineage_id),
            crate::core::agent_systems::AgentGeneration(individual.generation),
        ));
        match individual.class {
            AgentClass::Predator => {
                world
                    .entity_mut(entity)
                    .insert(crate::core::components::Predator);
            }
            AgentClass::Prey => {
                world
                    .entity_mut(entity)
                    .insert(crate::core::components::Prey);
            }
        }

        // The brain is the archived one. Invariant D01: restore paths carry the brain they were
        // given and never roll a new one. Above `ARCHIVE_CAP` it may be a clone of a different
        // dormant individual's brain — that is the sampling loss the module docs describe — but it
        // is never freshly random, so a re-hydrated population is drawn from the lineages that
        // actually went to sleep there.
        if let Some(genotype) = individual.brain {
            world
                .entity_mut(entity)
                .insert(crate::core::components::AgentBrain::from_arc(genotype));
        }

        // `decode_genotype` hands every new body a flat starting reserve out of nowhere. Zero it and
        // fund from the cohort share, exactly as the replacement path does with detritus.
        let unlanded = match world.get_mut::<crate::ai::hrrl::HomeostaticState>(entity) {
            Some(mut homeo) => {
                let cap = homeo.energy_target;
                homeo.energy = 0.0;
                let landed = crate::core::energy_ledger::credit_reserve(
                    &mut homeo.energy,
                    energy as f32,
                    cap,
                );
                homeo.hydration = (hydration as f32).min(homeo.hydration_target);
                energy - landed
            }
            // A body with no homeostatic state cannot hold energy at all, so the whole share is
            // unembodied rather than silently dropped.
            None => energy,
        };

        // Whatever the reserve could not hold — it saturated at its cap, or the `f32` add rounded —
        // is EU that no body embodies. That is the definition of detritus. Measured rather than
        // assumed, so the census stays exact.
        let unlanded = unlanded + orphaned;
        if unlanded > 0.0 {
            if let Some(mut pool) =
                world.get_resource_mut::<crate::core::ecology::EcosystemBiomass>()
            {
                pool.detritus += unlanded;
            }
        }
    }
}

/// Dehydrate agents that have been `Cold` for [`DormantCohorts::dwell_ticks`] consecutive ticks.
///
/// Does nothing without a [`DormantCohorts`] resource, and nothing without an enabled
/// [`LodFocus`](crate::core::simulation_lod::LodFocus) — a disabled focus tiers everything `Hot`, so
/// the default configuration never dehydrates anything.
pub fn dehydrate_cold_agents_system(
    mut commands: Commands,
    cohorts: Option<Res<DormantCohorts>>,
    focus: Option<Res<crate::core::simulation_lod::LodFocus>>,
    bands: Option<Res<crate::core::simulation_lod::LodBands>>,
    mut query: Query<
        (
            Entity,
            &crate::core::components::Position,
            Option<&mut DormancyWatch>,
        ),
        With<crate::core::components::Agent>,
    >,
) {
    let Some(cohorts) = cohorts else {
        return;
    };
    let dwell = cohorts.dwell_ticks;
    for (entity, pos, watch) in query.iter_mut() {
        let tier = crate::core::simulation_lod::tier_at(pos.0, focus.as_deref(), bands.as_deref());
        if tier != crate::core::simulation_lod::LodTier::Cold {
            // Warmed up again: the dwell counter resets by removing the component, so a boundary
            // agent that dips in and out never accumulates toward dormancy.
            if watch.is_some() {
                commands.entity(entity).remove::<DormancyWatch>();
            }
            continue;
        }
        match watch {
            Some(mut w) => {
                w.cold_ticks = w.cold_ticks.saturating_add(1);
                if w.cold_ticks >= dwell {
                    commands.add(DehydrateAgentCommand { root: entity });
                }
            }
            None => {
                commands
                    .entity(entity)
                    .insert(DormancyWatch { cold_ticks: 1 });
            }
        }
    }
}

/// Run one tick of ecology for the dormant cohorts: they burn metabolism, and their herbivores
/// graze the resource field under their chunks.
///
/// This is the aggregate half of the tier — the second model of the same ecology — and it exists so
/// that **time passes where nobody is looking**. Without it a dormant region is a freezer, and an
/// observer's route through the world decides which populations age.
///
/// Two rules govern every line of it, and both are about *not* being a second ecology:
///
/// - **No dynamic the live world does not have.** A live agent at zero energy is not despawned
///   (`update_agent_evaluation_system` merely stops counting it), so a dormant cohort does not
///   starve to death either. Adding aggregate mortality would look like richer ecology and would in
///   fact be the observer deciding who dies.
/// - **The same rates, not similar ones.** Metabolism is what the members were measured burning
///   while live; grazing uses `herbivore_intake` and the same `8.0 * dt` bite ceiling as
///   `herbivore_grazing_system`, summed over the cohort's herbivores.
///
/// Ordered exactly where its live counterparts sit: after live grazing, before regrowth, so both
/// consumers draw on the same standing field before it grows back. Inert without a
/// [`DormantCohorts`] resource. Allocation-free.
pub fn dormant_cohort_ecology_system(
    cohorts: Option<ResMut<DormantCohorts>>,
    field: Option<ResMut<crate::core::ecology::ResourceField>>,
    biomass: Option<ResMut<crate::core::ecology::EcosystemBiomass>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let Some(mut cohorts) = cohorts else {
        return;
    };
    if cohorts.total_dormant() == 0 {
        return;
    }

    // Metabolism first, so a cohort's appetite this tick reflects what it has just burned — the
    // same order the live schedule runs metabolic decay and grazing in.
    let respired = cohorts.respire_all();

    let grazed = match field {
        Some(mut field) => cohorts.graze_all(&mut field, time_step.0),
        None => 0.0,
    };
    let spilled = cohorts.take_spilled();

    if let Some(mut pool) = biomass {
        // Respired energy leaves the animals and becomes free detritus, exactly as
        // `metabolic_decay_system` does for live bodies.
        pool.detritus += respired;
        // Grazing moves EU out of the standing field. `plants` is carried incrementally rather than
        // re-summed each tick, so what left has to be reported here or the mirror drifts from the
        // store it describes.
        pool.plants -= grazed;
        // Whatever left the field but no cohort had room for is not lost — it is detritus.
        pool.detritus += spilled;
    }
}

/// Bring dormant individuals back where the focus has returned.
///
/// Wakes at most [`DormantCohorts::rehydrate_per_tick`] individuals per tick — see
/// [`REHYDRATION_PER_TICK`] for why the wake-up is spread out rather than instant.
pub fn rehydrate_wakeable_chunks_system(
    mut commands: Commands,
    cohorts: Option<Res<DormantCohorts>>,
    focus: Option<Res<crate::core::simulation_lod::LodFocus>>,
    bands: Option<Res<crate::core::simulation_lod::LodBands>>,
    mut scratch: Local<Vec<usize>>,
) {
    let Some(cohorts) = cohorts else {
        return;
    };
    let Some(focus) = focus else {
        return;
    };
    if !focus.enabled {
        return;
    }
    let radius = bands
        .map(|b| b.hot_radius)
        .unwrap_or_else(|| crate::core::simulation_lod::LodBands::default().hot_radius);
    cohorts.wakeable_into(focus.center, radius, &mut scratch);

    // The budget counts *individuals*, not chunks: one command per chunk per tick would make a
    // crowded chunk take hundreds of ticks to wake while the setting claimed otherwise. Chunks are
    // served in index order and each is emptied before the next is started, so a crowded chunk is
    // served first and fully — acceptable because the budget is small and only chunks the observer
    // has actually arrived at are ever in this list.
    let mut budget = cohorts.rehydrate_per_tick;
    for &chunk in scratch.iter() {
        if budget == 0 {
            break;
        }
        let waking = cohorts
            .cohort(chunk)
            .map_or(0, |c| c.count as usize)
            .min(budget);
        for _ in 0..waking {
            commands.add(RehydrateCommand { chunk });
        }
        budget -= waking;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cohorts() -> DormantCohorts {
        DormantCohorts::new(42, -100.0, -100.0, 100.0, 100.0)
    }

    /// A prey's vitals: `energy` and `hydration`, satiated at twice its reserve and burning nothing,
    /// so the cohort arithmetic under test is not perturbed by metabolism it did not ask for.
    fn vitals(energy: f64, hydration: f64) -> DormantVitals {
        DormantVitals {
            energy,
            hydration,
            energy_cap: energy * 2.0,
            burn_per_tick: 0.0,
            is_prey: true,
        }
    }

    fn individual(tag: u32) -> ArchivedIndividual {
        ArchivedIndividual {
            morphology: MorphologyGenotype::default(),
            brain: None,
            class: AgentClass::Prey,
            lineage_id: format!("lin-{tag}"),
            generation: tag,
        }
    }

    #[test]
    fn chunk_lookup_covers_the_bounds_and_rejects_outside() {
        let c = cohorts();
        assert_eq!(c.chunk_index(-100.0, -100.0), Some(0));
        assert_eq!(
            c.chunk_index(100.0, 100.0),
            Some(AGGREGATE_GRID * AGGREGATE_GRID - 1)
        );
        assert_eq!(c.chunk_index(-101.0, 0.0), None);
        assert_eq!(c.chunk_index(f32::NAN, 0.0), None);
    }

    #[test]
    fn absorbing_off_grid_refuses_rather_than_swallowing_the_energy() {
        // The caller destroys the agent only if this returns true. Returning true for a position
        // with no chunk would delete an agent whose energy went nowhere.
        let mut c = cohorts();
        assert!(!c.absorb(500.0, 0.0, vitals(25.0, 10.0), individual(1)));
        assert_eq!(c.total_energy(), 0.0);
        assert_eq!(c.total_dormant(), 0);
    }

    #[test]
    fn energy_survives_a_round_trip_exactly() {
        let mut c = cohorts();
        for i in 0..5 {
            assert!(c.absorb(0.0, 0.0, vitals(20.0, 5.0), individual(i)));
        }
        assert_eq!(c.total_energy(), 100.0);

        let mut returned = 0.0;
        while c.total_dormant() > 0 {
            let idx = c.chunk_index(0.0, 0.0).unwrap();
            let (_, v) = c.release(idx).unwrap();
            let e = v.energy;
            returned += e;
        }
        returned += c.drain_orphaned_energy();
        assert!(
            (returned - 100.0).abs() < 1e-12,
            "round trip returned {returned} of 100 EU"
        );
        assert_eq!(c.total_energy(), 0.0);
    }

    #[test]
    fn unequal_reserves_come_back_pooled_but_the_total_is_exact() {
        // Individual variation does not survive dormancy — everyone returns at the mean. What must
        // survive is the sum.
        let mut c = cohorts();
        for (i, e) in [1.0, 50.0, 3.0, 7.5].into_iter().enumerate() {
            assert!(c.absorb(0.0, 0.0, vitals(e, 1.0), individual(i as u32)));
        }
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        let (_, v) = c.release(idx).unwrap();
        let first = v.energy;
        assert!(
            (first - 61.5 / 4.0).abs() < 1e-12,
            "released {first}, expected the mean"
        );
        let mut total = first;
        while c.total_dormant() > 0 {
            total += c.release(idx).unwrap().1.energy;
        }
        total += c.drain_orphaned_energy();
        assert!((total - 61.5).abs() < 1e-12, "total was {total}");
    }

    #[test]
    fn a_small_cohort_round_trips_its_genomes_exactly() {
        // The lossless regime: at or below the cap, dormancy loses nothing genetic.
        let mut c = cohorts();
        for i in 0..ARCHIVE_CAP as u32 {
            assert!(c.absorb(0.0, 0.0, vitals(10.0, 1.0), individual(i)));
        }
        assert_eq!(c.genomes_dropped(), 0);

        let idx = c.chunk_index(0.0, 0.0).unwrap();
        let mut back: Vec<u32> = Vec::new();
        while c.total_dormant() > 0 {
            back.push(c.release(idx).unwrap().0.generation);
        }
        back.sort_unstable();
        assert_eq!(back, (0..ARCHIVE_CAP as u32).collect::<Vec<_>>());
    }

    #[test]
    fn beyond_the_cap_the_loss_is_counted_not_silent() {
        let mut c = cohorts();
        let n = ARCHIVE_CAP as u32 + 50;
        for i in 0..n {
            assert!(c.absorb(0.0, 0.0, vitals(10.0, 1.0), individual(i)));
        }
        assert_eq!(
            c.genomes_dropped(),
            50,
            "every genome past the cap must be accounted for"
        );
        assert_eq!(
            c.cohort(c.chunk_index(0.0, 0.0).unwrap())
                .unwrap()
                .archived(),
            ARCHIVE_CAP
        );
        // The count and the energy are unaffected by the genetic loss — 58 individuals are dormant.
        assert_eq!(c.total_dormant(), n as u64);
        assert_eq!(c.total_energy(), n as f64 * 10.0);
    }

    #[test]
    fn the_archive_is_a_sample_of_everything_not_the_last_arrivals() {
        // An arrival-ordered archive would make dormancy select on arrival time, which means
        // nothing biologically. Reservoir sampling must keep some early arrivals.
        let mut c = cohorts();
        for i in 0..500u32 {
            assert!(c.absorb(0.0, 0.0, vitals(1.0, 1.0), individual(i)));
        }
        let chunk = c.cohort(c.chunk_index(0.0, 0.0).unwrap()).unwrap();
        let kept: Vec<u32> = chunk.archive.iter().map(|a| a.generation).collect();
        assert_eq!(kept.len(), ARCHIVE_CAP);
        assert!(
            kept.iter().any(|&g| g < 250),
            "no early arrival survived: {kept:?} — this is the last-N bug, not a sample"
        );
    }

    #[test]
    fn dormancy_draws_are_reproducible_from_the_run_seed() {
        let archive_of = |seed: u64| {
            let mut c = DormantCohorts::new(seed, -100.0, -100.0, 100.0, 100.0);
            for i in 0..200u32 {
                c.absorb(0.0, 0.0, vitals(1.0, 1.0), individual(i));
            }
            let idx = c.chunk_index(0.0, 0.0).unwrap();
            c.cohort(idx)
                .unwrap()
                .archive
                .iter()
                .map(|a| a.generation)
                .collect::<Vec<_>>()
        };
        assert_eq!(archive_of(1234), archive_of(1234), "same seed, same sample");
        assert_ne!(
            archive_of(1234),
            archive_of(9999),
            "a different seed must actually change the draw"
        );
    }

    #[test]
    fn an_empty_cohort_releases_nothing() {
        let mut c = cohorts();
        assert!(c.release(0).is_none());
        assert!(
            c.release(999_999).is_none(),
            "an out-of-range chunk is None, not a panic"
        );
    }

    #[test]
    fn wakeable_finds_only_occupied_chunks_in_range() {
        let mut c = cohorts();
        assert!(c.absorb(-90.0, -90.0, vitals(10.0, 1.0), individual(1)));
        assert!(c.absorb(90.0, 90.0, vitals(10.0, 1.0), individual(2)));
        let mut out = Vec::new();

        c.wakeable_into(Vec3::new(-90.0, 0.0, -90.0), 20.0, &mut out);
        assert_eq!(out.len(), 1, "only the near corner is in range");

        c.wakeable_into(Vec3::ZERO, 1000.0, &mut out);
        assert_eq!(out.len(), 2, "a wide radius reaches both");

        c.wakeable_into(Vec3::ZERO, 1.0, &mut out);
        assert!(out.is_empty(), "nothing is that close to the origin");
    }

    #[test]
    fn an_emptied_cohort_starts_a_fresh_reservoir() {
        // `seen` must reset, or a chunk that has been through one heavy dormancy episode would
        // reject almost every genome of the next one — the cap would silently become "the first
        // cohort ever to sleep here".
        let mut c = cohorts();
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        for i in 0..100u32 {
            c.absorb(0.0, 0.0, vitals(1.0, 1.0), individual(i));
        }
        while c.total_dormant() > 0 {
            c.release(idx).unwrap();
        }
        c.drain_orphaned_energy();

        c.absorb(0.0, 0.0, vitals(1.0, 1.0), individual(777));
        let chunk = c.cohort(idx).unwrap();
        assert_eq!(chunk.archived(), 1);
        assert_eq!(chunk.archive[0].generation, 777);
    } // ---- Aggregate ecology ---------------------------------------------------------------

    fn burning(energy: f64, burn: f64, is_prey: bool) -> DormantVitals {
        DormantVitals {
            energy,
            hydration: 0.0,
            energy_cap: energy * 2.0,
            burn_per_tick: burn,
            is_prey,
        }
    }

    fn uniform_field(side: usize, per_cell: f32) -> crate::core::ecology::ResourceField {
        let n = side * side;
        crate::core::ecology::ResourceField {
            width: side,
            height: side,
            min_x: -100.0,
            min_z: -100.0,
            max_x: 100.0,
            max_z: 100.0,
            r: vec![per_cell; n],
            r_max: vec![per_cell; n],
            growth_rate: 0.0,
        }
    }

    #[test]
    fn respiration_burns_the_measured_rate_and_hands_it_over_exactly() {
        let mut c = cohorts();
        c.absorb(0.0, 0.0, burning(100.0, 0.25, true), individual(1));
        c.absorb(0.0, 0.0, burning(100.0, 0.75, true), individual(2));

        // The cohort burns the SUM of what its members were measured burning while alive.
        let burned = c.respire_all();
        assert!((burned - 1.0).abs() < 1e-12, "burned {burned}");
        assert!((c.total_energy() - 199.0).abs() < 1e-12);
    }

    #[test]
    fn a_starving_cohort_bottoms_out_at_zero_and_keeps_its_members() {
        // Matching the live world, where `update_agent_evaluation_system` stops counting an agent at
        // zero energy but does not despawn it. Aggregate mortality would be a dynamic the live model
        // does not have — the observer deciding who dies.
        let mut c = cohorts();
        c.absorb(0.0, 0.0, burning(1.0, 10.0, true), individual(1));

        let first = c.respire_all();
        assert!(
            (first - 1.0).abs() < 1e-12,
            "it can only burn what it holds"
        );
        assert_eq!(c.total_energy(), 0.0);
        assert_eq!(c.total_dormant(), 1, "starvation must not kill");

        assert_eq!(c.respire_all(), 0.0, "an empty pool burns nothing further");
        assert_eq!(c.total_dormant(), 1);
    }

    #[test]
    fn a_cohort_with_no_measured_burn_rate_costs_nothing() {
        // A freshly spawned agent has no `FeatureTracker` history, so it contributes no rate. That
        // under-charges rather than over-charges, which is the safe direction: it cannot manufacture
        // a metabolic cost the live world never levied.
        let mut c = cohorts();
        c.absorb(0.0, 0.0, burning(50.0, 0.0, true), individual(1));
        assert_eq!(c.respire_all(), 0.0);
        assert_eq!(c.total_energy(), 50.0);
    }

    #[test]
    fn only_herbivores_contribute_appetite() {
        let mut c = cohorts();
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        c.absorb(0.0, 0.0, burning(10.0, 0.0, false), individual(1));
        c.absorb(0.0, 0.0, burning(10.0, 0.0, false), individual(2));
        assert_eq!(
            c.cohort(idx).unwrap().grazing_appetite(),
            0.0,
            "predators do not graze, matching herbivore_grazing_system's With<Prey> filter"
        );

        c.absorb(0.0, 0.0, burning(10.0, 0.0, true), individual(3));
        // Hunger is (cap 60 - energy 30) = 30, scaled by the herbivore share of 1 in 3.
        let appetite = c.cohort(idx).unwrap().grazing_appetite();
        assert!((appetite - 10.0).abs() < 1e-12, "appetite was {appetite}");
    }

    #[test]
    fn a_satiated_cohort_does_not_graze() {
        let mut c = cohorts();
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        c.absorb(
            0.0,
            0.0,
            DormantVitals {
                energy: 40.0,
                hydration: 0.0,
                energy_cap: 40.0,
                burn_per_tick: 0.0,
                is_prey: true,
            },
            individual(1),
        );
        assert_eq!(c.cohort(idx).unwrap().grazing_appetite(), 0.0);
    }

    #[test]
    fn grazing_takes_from_the_field_exactly_what_the_cohort_gains() {
        let mut c = cohorts();
        c.absorb(0.0, 0.0, burning(10.0, 0.0, true), individual(1));
        let mut field = uniform_field(64, 5.0);

        let before_field = field.total_biomass();
        let before_cohort = c.total_energy();
        let grazed = c.graze_all(&mut field, 1.0 / 60.0);
        let spilled = c.take_spilled();

        assert!(grazed > 0.0, "a hungry cohort over a full field should eat");
        let field_lost = before_field - field.total_biomass();
        assert!(
            (field_lost - grazed).abs() < 1e-9,
            "the field lost {field_lost} but grazing reported {grazed}"
        );
        let cohort_gained = c.total_energy() - before_cohort;
        assert!(
            (cohort_gained + spilled - grazed).abs() < 1e-9,
            "gained {cohort_gained} plus spilled {spilled} should equal grazed {grazed}"
        );
    }

    #[test]
    fn grazing_only_touches_the_cells_under_its_own_chunk() {
        let mut c = cohorts();
        // Put the herd in the far corner, well away from the origin.
        c.absorb(-95.0, -95.0, burning(1.0, 0.0, true), individual(1));
        let mut field = uniform_field(64, 5.0);
        c.graze_all(&mut field, 1.0 / 60.0);

        assert!(field.r[0] < 5.0, "the chunk's own cells should be grazed");
        assert_eq!(
            field.r[64 * 64 - 1],
            5.0,
            "a cohort must not graze the other side of the world"
        );
    }

    #[test]
    fn grazing_an_empty_field_takes_nothing_and_spills_nothing() {
        let mut c = cohorts();
        c.absorb(0.0, 0.0, burning(1.0, 0.0, true), individual(1));
        let mut field = uniform_field(16, 0.0);
        assert_eq!(c.graze_all(&mut field, 1.0 / 60.0), 0.0);
        assert_eq!(c.take_spilled(), 0.0);
        assert_eq!(c.total_energy(), 1.0);
    }

    #[test]
    fn spilled_energy_is_banked_once_and_only_once() {
        let mut c = cohorts();
        assert_eq!(c.take_spilled(), 0.0);
        c.spilled = 3.5;
        assert_eq!(c.take_spilled(), 3.5);
        assert_eq!(c.take_spilled(), 0.0, "banking it twice would create EU");
    }

    #[test]
    fn releasing_hands_back_a_share_of_the_burn_rate_and_the_cap() {
        // The pooled quantities have to leave with the individual, or a cohort that has been through
        // a sleep/wake cycle would keep charging metabolism for members it no longer holds — a slow
        // energy sink that would look exactly like ordinary attrition.
        let mut c = cohorts();
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        c.absorb(0.0, 0.0, burning(10.0, 0.5, true), individual(1));
        c.absorb(0.0, 0.0, burning(10.0, 0.5, true), individual(2));

        let (_, v) = c.release(idx).unwrap();
        assert!((v.burn_per_tick - 0.5).abs() < 1e-12);
        assert!((v.energy_cap - 20.0).abs() < 1e-12);
        assert!(v.is_prey);

        let chunk = c.cohort(idx).unwrap();
        assert!(
            (chunk.burn_rate - 0.5).abs() < 1e-12,
            "the cohort keeps exactly one member's rate"
        );
        assert_eq!(chunk.prey_count, 1);
    }

    #[test]
    fn an_emptied_cohort_carries_no_leftover_rate_into_its_next_episode() {
        let mut c = cohorts();
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        c.absorb(0.0, 0.0, burning(10.0, 0.5, true), individual(1));
        c.release(idx).unwrap();

        let chunk = c.cohort(idx).unwrap();
        assert_eq!(chunk.burn_rate, 0.0);
        assert_eq!(chunk.energy_cap, 0.0);
        assert_eq!(chunk.prey_count, 0);
        assert_eq!(c.respire_all(), 0.0, "an empty grid burns nothing");
    }
}
