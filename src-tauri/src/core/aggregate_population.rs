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
//! ## Dormant cohorts are suspended, not simulated
//!
//! `WORLD_DESIGN.md` §7 calls for cohorts to run aggregate ecology while dormant — logistic growth,
//! Holling grazing, density-dependent mortality. **This module does not do that yet.** A dormant
//! cohort is frozen: its energy and count do not change until something re-hydrates it.
//!
//! That is a real artifact and worth naming plainly: **time does not pass where nobody is looking.**
//! It is deferred rather than rushed because the two halves fail differently. Conservation here is
//! exact and provable; an aggregate ecology is a second model of the same system, and if it were
//! landed in the same commit a drifting residual would have two suspects instead of one. The
//! dynamics belong on a substrate whose conservation is already proven — which is what this is.
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

/// The dormant population of one chunk.
#[derive(Clone, Debug, Default)]
pub struct Cohort {
    /// How many dormant individuals the chunk holds.
    pub count: u32,
    /// Their total energy, in EU. Authoritative — this is animal energy that the census must see.
    pub energy: f64,
    /// Their total hydration. Not a conserved quantity, so it is pooled without ceremony.
    pub hydration: f64,
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
        energy: f64,
        hydration: f64,
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
        chunk.energy += energy;
        chunk.hydration += hydration;
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
    pub fn release(&mut self, index: usize) -> Option<(ArchivedIndividual, f64, f64)> {
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
        let share_energy = chunk.mean_energy().min(chunk.energy.max(0.0));
        let share_hydration = if chunk.count == 0 {
            0.0
        } else {
            (chunk.hydration / chunk.count as f64).min(chunk.hydration.max(0.0))
        };

        let individual = match pick {
            Some(i) => chunk.archive[i].clone(),
            None => chunk.archive.remove(0),
        };

        chunk.count -= 1;
        chunk.energy -= share_energy;
        chunk.hydration -= share_hydration;
        if chunk.count == 0 {
            // Rounding can leave a sliver behind. Reset `seen` so the next dormancy episode starts a
            // fresh reservoir rather than inheriting a denominator that would starve it of samples.
            chunk.seen = 0;
            chunk.archive.clear();
        }
        self.rehydrated += 1;
        Some((individual, share_energy, share_hydration))
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

        let (energy, hydration) = match world.get::<crate::ai::hrrl::HomeostaticState>(self.root) {
            Some(h) => (h.energy.max(0.0) as f64, h.hydration.max(0.0) as f64),
            None => (0.0, 0.0),
        };

        let absorbed = world
            .get_resource_mut::<DormantCohorts>()
            .map(|mut cohorts| {
                cohorts.absorb(
                    pos.x,
                    pos.z,
                    energy,
                    hydration,
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
        let (center, individual, energy, hydration, orphaned) = {
            let Some(mut cohorts) = world.get_resource_mut::<DormantCohorts>() else {
                return;
            };
            let center = cohorts.chunk_center(self.chunk);
            let Some((individual, energy, hydration)) = cohorts.release(self.chunk) else {
                return;
            };
            // A cohort that just emptied can be left holding a rounding sliver. It is EU, and the
            // census counts every cohort, so leaving it there would inflate the animal compartment
            // by a hair permanently — small, but a one-way ratchet, and invisible because the
            // residual would still look like noise. Drained at the one moment a cohort can empty.
            let orphaned = cohorts.drain_orphaned_energy();
            (center, individual, energy, hydration, orphaned)
        };

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
        assert!(!c.absorb(500.0, 0.0, 25.0, 10.0, individual(1)));
        assert_eq!(c.total_energy(), 0.0);
        assert_eq!(c.total_dormant(), 0);
    }

    #[test]
    fn energy_survives_a_round_trip_exactly() {
        let mut c = cohorts();
        for i in 0..5 {
            assert!(c.absorb(0.0, 0.0, 20.0, 5.0, individual(i)));
        }
        assert_eq!(c.total_energy(), 100.0);

        let mut returned = 0.0;
        while c.total_dormant() > 0 {
            let idx = c.chunk_index(0.0, 0.0).unwrap();
            let (_, e, _) = c.release(idx).unwrap();
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
            assert!(c.absorb(0.0, 0.0, e, 1.0, individual(i as u32)));
        }
        let idx = c.chunk_index(0.0, 0.0).unwrap();
        let (_, first, _) = c.release(idx).unwrap();
        assert!(
            (first - 61.5 / 4.0).abs() < 1e-12,
            "released {first}, expected the mean"
        );
        let mut total = first;
        while c.total_dormant() > 0 {
            total += c.release(idx).unwrap().1;
        }
        total += c.drain_orphaned_energy();
        assert!((total - 61.5).abs() < 1e-12, "total was {total}");
    }

    #[test]
    fn a_small_cohort_round_trips_its_genomes_exactly() {
        // The lossless regime: at or below the cap, dormancy loses nothing genetic.
        let mut c = cohorts();
        for i in 0..ARCHIVE_CAP as u32 {
            assert!(c.absorb(0.0, 0.0, 10.0, 1.0, individual(i)));
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
            assert!(c.absorb(0.0, 0.0, 10.0, 1.0, individual(i)));
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
            assert!(c.absorb(0.0, 0.0, 1.0, 1.0, individual(i)));
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
                c.absorb(0.0, 0.0, 1.0, 1.0, individual(i));
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
        assert!(c.absorb(-90.0, -90.0, 10.0, 1.0, individual(1)));
        assert!(c.absorb(90.0, 90.0, 10.0, 1.0, individual(2)));
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
            c.absorb(0.0, 0.0, 1.0, 1.0, individual(i));
        }
        while c.total_dormant() > 0 {
            c.release(idx).unwrap();
        }
        c.drain_orphaned_energy();

        c.absorb(0.0, 0.0, 1.0, 1.0, individual(777));
        let chunk = c.cohort(idx).unwrap();
        assert_eq!(chunk.archived(), 1);
        assert_eq!(chunk.archive[0].generation, 777);
    }
}
