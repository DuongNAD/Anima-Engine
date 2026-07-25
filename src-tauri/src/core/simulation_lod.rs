//! Simulation level of detail — how much thinking an agent gets, by where it is.
//!
//! Brain inference is the dominant per-tick cost, and it is paid for every agent whether or not
//! anything is watching. That is what stands between the engine and the population sizes it aims at:
//! ADR-0003 gate EB-S12 measured ~46,500 resident agents per GiB of weights, and the map research
//! then showed resolution binds before memory does. Neither number matters if every agent must think
//! sixty times a second.
//!
//! The standard answer in the simulation-LOD literature is to run a detailed model where it is being
//! observed and a cheaper one elsewhere, choosing the level per agent rather than globally. This is
//! the first half of that: three tiers that decide **how often an agent thinks**.
//!
//! | Tier | Distance | Inference |
//! |---|---|---|
//! | `Hot` | inside `hot_radius` | every tick, exactly as before |
//! | `Warm` | out to `warm_radius` | once every `warm_interval` ticks |
//! | `Cold` | beyond that | never |
//!
//! ## What this does not do yet
//!
//! A `Cold` agent still exists: it holds its brain, keeps its last CPG parameters, and goes on
//! moving, eating and metabolising. So this buys **CPU, not memory** — the weights stay resident.
//! Reclaiming memory needs the aggregate tier, where distant individuals are replaced by per-cell
//! population statistics and re-hydrated on approach. That is a second model of the same ecology and
//! it has to preserve closed energy across every transition, so it is deliberately not bundled here.
//!
//! ## Default is off
//!
//! [`LodFocus::enabled`] is `false` unless something sets a focus, and a disabled focus puts every
//! agent in `Hot`. An unconfigured run is therefore indistinguishable from one without this module
//! at all — held by `without_a_focus_every_agent_thinks_every_tick` and
//! `a_disabled_focus_changes_nothing` in `tests/simulation_lod_tests.rs`.

use bevy_ecs::prelude::Resource;
use glam::Vec3;

/// How much detail an agent is being simulated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodTier {
    Hot,
    Warm,
    Cold,
}

/// Where detail is centred — the observer, in world units.
///
/// Headless runs have no camera, so this stays disabled and everything is `Hot`. A UI can set it to
/// the view position later; nothing here depends on that existing.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LodFocus {
    pub enabled: bool,
    pub center: Vec3,
}

impl Default for LodFocus {
    fn default() -> Self {
        Self {
            enabled: false,
            center: Vec3::ZERO,
        }
    }
}

impl LodFocus {
    pub fn at(center: Vec3) -> Self {
        Self {
            enabled: true,
            center,
        }
    }
}

/// Tier boundaries and how often a `Warm` agent thinks.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LodBands {
    pub hot_radius: f32,
    pub warm_radius: f32,
    /// Ticks between inferences for a `Warm` agent. `1` makes `Warm` behave like `Hot`.
    pub warm_interval: u32,
}

impl Default for LodBands {
    fn default() -> Self {
        // Sized against the world's 200-unit extent: a quarter of it stays fully alive, half of it
        // thinks at a reduced rate, and the far quarter coasts. Deliberately generous — the point of
        // the first version is to prove the mechanism, not to squeeze it.
        Self {
            hot_radius: 50.0,
            warm_radius: 100.0,
            warm_interval: 8,
        }
    }
}

impl LodBands {
    /// Tier for an agent `distance` away from the focus.
    ///
    /// Boundaries are inclusive at the near side, so an agent exactly on `hot_radius` is still
    /// `Hot`. Non-finite distances read as `Cold`: a position that has gone to NaN is not something
    /// to spend inference on, and treating it as `Hot` would hide the problem behind a busy agent.
    pub fn tier(&self, distance: f32) -> LodTier {
        if !distance.is_finite() {
            return LodTier::Cold;
        }
        if distance <= self.hot_radius {
            LodTier::Hot
        } else if distance <= self.warm_radius.max(self.hot_radius) {
            LodTier::Warm
        } else {
            LodTier::Cold
        }
    }
}

/// Whether an agent in `tier` should run inference on this tick.
///
/// `Warm` agents are spread across the interval by entity index rather than all firing on the same
/// tick. Without that, a warm band of a thousand agents would go quiet for seven ticks and then
/// submit a thousand requests at once — the same total work, arriving as a spike that the frame
/// budget feels and the average hides. Staggering is the same trick the resource field uses.
///
/// Deterministic by construction: entity index and tick number, no clock and no RNG.
pub fn should_infer(tier: LodTier, entity_index: u32, tick: u64, warm_interval: u32) -> bool {
    match tier {
        LodTier::Hot => true,
        LodTier::Cold => false,
        LodTier::Warm => {
            let interval = warm_interval.max(1) as u64;
            (tick + entity_index as u64).is_multiple_of(interval)
        }
    }
}

/// Tier for an agent at `position`, or `Hot` when no focus is set.
///
/// The fallback matters: every caller takes the focus as an `Option`, and a world that never
/// configured one must behave exactly as it did before this module existed.
pub fn tier_at(position: Vec3, focus: Option<&LodFocus>, bands: Option<&LodBands>) -> LodTier {
    match focus {
        Some(f) if f.enabled => {
            let bands = bands.copied().unwrap_or_default();
            bands.tier(position.distance(f.center))
        }
        _ => LodTier::Hot,
    }
}

/// The LOD inputs a system needs, as one parameter.
///
/// Bundled rather than taken as three separate arguments because Bevy caps a system at sixteen
/// parameters and `sensory_system` was already close to it — and because the three are one concept,
/// so splitting them across a signature only made the call site noisier.
#[derive(bevy_ecs::system::SystemParam)]
pub struct LodGate<'w, 's> {
    focus: Option<bevy_ecs::system::Res<'w, LodFocus>>,
    bands: Option<bevy_ecs::system::Res<'w, LodBands>>,
    tick: bevy_ecs::system::Local<'s, u64>,
}

impl LodGate<'_, '_> {
    /// Advance the tick counter and take a copy of the configuration for this run.
    ///
    /// Snapshotting once per tick rather than reading the resources per agent keeps the decision
    /// consistent across the whole population: a focus that moved mid-loop would otherwise tier the
    /// first half of the agents against one centre and the second half against another.
    pub fn begin_tick(&mut self) -> LodSnapshot {
        *self.tick = self.tick.wrapping_add(1);
        LodSnapshot {
            focus: self.focus.as_deref().copied().unwrap_or_default(),
            bands: self.bands.as_deref().copied().unwrap_or_default(),
            tick: *self.tick,
        }
    }
}

/// One tick's LOD decision inputs, detached from the ECS.
#[derive(Clone, Copy, Debug)]
pub struct LodSnapshot {
    pub focus: LodFocus,
    pub bands: LodBands,
    pub tick: u64,
}

impl LodSnapshot {
    pub fn tier(&self, position: Vec3) -> LodTier {
        tier_at(position, Some(&self.focus), Some(&self.bands))
    }

    /// Whether the agent at `position` should run inference this tick.
    pub fn should_think(&self, position: Vec3, entity_index: u32) -> bool {
        should_infer(
            self.tier(position),
            entity_index,
            self.tick,
            self.bands.warm_interval,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands() -> LodBands {
        LodBands {
            hot_radius: 10.0,
            warm_radius: 20.0,
            warm_interval: 4,
        }
    }

    #[test]
    fn tiers_follow_distance() {
        let b = bands();
        assert_eq!(b.tier(0.0), LodTier::Hot);
        assert_eq!(b.tier(10.0), LodTier::Hot, "the near boundary is inclusive");
        assert_eq!(b.tier(10.001), LodTier::Warm);
        assert_eq!(b.tier(20.0), LodTier::Warm);
        assert_eq!(b.tier(20.001), LodTier::Cold);
        assert_eq!(b.tier(1e9), LodTier::Cold);
    }

    #[test]
    fn a_broken_position_reads_as_cold() {
        // An agent whose position has gone non-finite is a bug to find, not a thing to keep
        // thinking. Treating it as `Hot` would keep it busy and hide the fault.
        assert_eq!(bands().tier(f32::NAN), LodTier::Cold);
        assert_eq!(bands().tier(f32::INFINITY), LodTier::Cold);
    }

    #[test]
    fn inverted_bands_do_not_produce_a_warm_gap() {
        // If someone configures `warm_radius` below `hot_radius`, the warm band is empty rather
        // than inverted — no distance should fall through into an undefined state.
        let b = LodBands {
            hot_radius: 30.0,
            warm_radius: 5.0,
            warm_interval: 4,
        };
        assert_eq!(b.tier(20.0), LodTier::Hot);
        assert_eq!(b.tier(31.0), LodTier::Cold);
    }

    #[test]
    fn hot_always_thinks_and_cold_never_does() {
        for tick in 0..32u64 {
            assert!(should_infer(LodTier::Hot, 7, tick, 4));
            assert!(!should_infer(LodTier::Cold, 7, tick, 4));
        }
    }

    #[test]
    fn warm_thinks_at_the_configured_rate() {
        let hits = (0..64u64)
            .filter(|&t| should_infer(LodTier::Warm, 0, t, 4))
            .count();
        assert_eq!(hits, 16, "a warm agent should think once every 4 ticks");
    }

    #[test]
    fn warm_agents_are_spread_across_the_interval() {
        // The property that keeps the saving real: a warm band must not fire all at once. With four
        // agents and an interval of four, exactly one should think on any given tick.
        for tick in 0..16u64 {
            let firing = (0..4u32)
                .filter(|&e| should_infer(LodTier::Warm, e, tick, 4))
                .count();
            assert_eq!(
                firing, 1,
                "tick {tick} had {firing} of 4 warm agents thinking"
            );
        }
    }

    #[test]
    fn an_interval_of_zero_is_treated_as_every_tick() {
        // Guards against a divide-by-zero and against a misconfiguration silently freezing agents.
        for tick in 0..8u64 {
            assert!(should_infer(LodTier::Warm, 3, tick, 0));
        }
    }

    #[test]
    fn no_focus_means_everything_is_hot() {
        let far = Vec3::new(10_000.0, 0.0, 10_000.0);
        assert_eq!(tier_at(far, None, None), LodTier::Hot);
        assert_eq!(
            tier_at(far, Some(&LodFocus::default()), Some(&bands())),
            LodTier::Hot,
            "a disabled focus must not start tiering agents"
        );
    }

    #[test]
    fn a_focus_tiers_relative_to_its_centre_not_the_origin() {
        let focus = LodFocus::at(Vec3::new(100.0, 0.0, 0.0));
        let b = bands();
        // Right next to the focus but far from the origin.
        assert_eq!(
            tier_at(Vec3::new(105.0, 0.0, 0.0), Some(&focus), Some(&b)),
            LodTier::Hot
        );
        // At the origin, which a naive distance-from-zero test would have called `Hot`.
        assert_eq!(tier_at(Vec3::ZERO, Some(&focus), Some(&b)), LodTier::Cold);
    }
}
