//! Land/water terrain walkability and shoreline navigation (Card LAND1; W1/W2 prototype).
//!
//! Opt-in via [`crate::core::resources::TerrainNavigation`]. When absent or `enabled == false`,
//! [`terrain_navigation_system`] returns immediately without mutating any component, keeping every
//! pre-LAND1 headless experiment bit-identical.
//!
//! When enabled:
//! - [`BiomeType::DeepOcean`] (`0`) and [`BiomeType::Ocean`] (`1`) are non-walkable;
//!   [`BiomeType::River`] (`3`) and all land biomes (`2`, `4..=10`) are walkable.
//! - After `integrate_physics_system`, any agent root whose wrapped `(x, z)` lies on a non-walkable
//!   cell is returned to its [`LastWalkablePosition`], its root velocity and `RigidBody.velocity`
//!   are zeroed, and all child segments (sharing `ParentAgent(root)`) are translated by the same
//!   delta and zeroed so joints never stretch across the shoreline.
//! - State is updated strictly in place on [`LastWalkablePosition`] with zero component structural
//!   changes and zero heap allocations per tick.

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::core::components::{Agent, LastWalkablePosition, ParentAgent, Position, Velocity};
use crate::core::resources::{MapBounds, TerrainNavigation};
use crate::core::terrain::{BiomeType, TerrainMap};
use crate::physics::dynamics::RigidBody;

/// Bounded deterministic retry budget when sampling a random walkable spawn point from `SimRng`.
pub const MAX_WALKABLE_RESAMPLE_TRIES: usize = 64;

/// Returns `true` if `biome` is walkable (`River` and land biomes are walkable; `DeepOcean` and
/// `Ocean` are not).
#[inline]
pub fn is_biome_walkable(biome: u8) -> bool {
    biome != BiomeType::DeepOcean as u8 && biome != BiomeType::Ocean as u8
}

/// Returns `true` if `biome` counts as water (`DeepOcean`, `Ocean`, or `River`) when placing
/// lakes along walkable shorelines in desktop genesis.
#[inline]
pub fn is_water_biome(biome: u8) -> bool {
    biome == BiomeType::DeepOcean as u8
        || biome == BiomeType::Ocean as u8
        || biome == BiomeType::River as u8
}

/// Wraps `(x, z)` onto `bounds` using the exact same `rem_euclid` arithmetic as
/// `wrap_coordinates_system`.
#[inline]
pub fn wrap_pos(mut pos: Vec3, bounds: &MapBounds) -> Vec3 {
    let x_range = bounds.max.x - bounds.min.x;
    let z_range = bounds.max.z - bounds.min.z;
    if x_range > 0.0 {
        pos.x = bounds.min.x + (pos.x - bounds.min.x).rem_euclid(x_range);
    }
    if z_range > 0.0 {
        pos.z = bounds.min.z + (pos.z - bounds.min.z).rem_euclid(z_range);
    }
    pos
}

/// World-space center `(x, 0.0, z)` of cell `(col, row)` in `tm` over `bounds`.
#[inline]
pub fn cell_center(tm: &TerrainMap, bounds: &MapBounds, col: usize, row: usize) -> Vec3 {
    let px = bounds.min.x + ((col as f32 + 0.5) / tm.width as f32) * (bounds.max.x - bounds.min.x);
    let pz = bounds.min.z + ((row as f32 + 0.5) / tm.height as f32) * (bounds.max.z - bounds.min.z);
    Vec3::new(px, 0.0, pz)
}

/// Returns `true` when `pos` (wrapped modulo `bounds`) maps inside `tm` onto a walkable cell.
#[inline]
pub fn is_pos_walkable(tm: &TerrainMap, bounds: &MapBounds, pos: Vec3) -> bool {
    let wrapped = wrap_pos(pos, bounds);
    let Some((col, row)) = tm.get_map_indices(wrapped, bounds) else {
        return false;
    };
    let idx = row * tm.width + col;
    tm.biomes.get(idx).copied().is_some_and(is_biome_walkable)
}

/// Returns `true` when `(col, row)` is walkable and has at least one 8-connected water neighbor.
pub fn is_walkable_cell_next_to_water(tm: &TerrainMap, col: usize, row: usize) -> bool {
    if col >= tm.width || row >= tm.height || tm.biomes.len() < tm.width * tm.height {
        return false;
    }
    if !is_biome_walkable(tm.biomes[row * tm.width + col]) {
        return false;
    }
    let r_start = row.saturating_sub(1);
    let r_end = (row + 1).min(tm.height - 1);
    let c_start = col.saturating_sub(1);
    let c_end = (col + 1).min(tm.width - 1);
    for nr in r_start..=r_end {
        for nc in c_start..=c_end {
            if nc == col && nr == row {
                continue;
            }
            if is_water_biome(tm.biomes[nr * tm.width + nc]) {
                return true;
            }
        }
    }
    false
}

/// Returns `wrapped(pos)` unchanged (preserving `pos.y`) when already walkable, or the center of
/// the nearest walkable cell in `(col, row)` Euclidean distance with deterministic `(d2, row, col)`
/// tie-breaking. Returns `None` if `tm` has no walkable cells or `bounds` is degenerate.
pub fn snap_to_nearest_walkable(tm: &TerrainMap, bounds: &MapBounds, pos: Vec3) -> Option<Vec3> {
    if tm.width == 0 || tm.height == 0 || tm.biomes.len() < tm.width * tm.height {
        return None;
    }
    let wrapped = wrap_pos(pos, bounds);
    let (c0, r0) = tm.get_map_indices(wrapped, bounds)?;
    if is_biome_walkable(tm.biomes[r0 * tm.width + c0]) {
        return Some(wrapped);
    }

    let max_radius = r0.max(tm.height - 1 - r0).max(c0.max(tm.width - 1 - c0));
    let mut best: Option<(usize, usize, usize)> = None;

    for radius in 1..=max_radius {
        if let Some((best_d2, _, _)) = best {
            if radius * radius > best_d2 {
                break;
            }
        }
        let r_i = radius as isize;
        let c0_i = c0 as isize;
        let r0_i = r0 as isize;
        for dc in -r_i..=r_i {
            consider_walkable_cell(tm, c0, r0, c0_i + dc, r0_i - r_i, &mut best);
            consider_walkable_cell(tm, c0, r0, c0_i + dc, r0_i + r_i, &mut best);
        }
        for dr in (-r_i + 1)..=(r_i - 1) {
            consider_walkable_cell(tm, c0, r0, c0_i - r_i, r0_i + dr, &mut best);
            consider_walkable_cell(tm, c0, r0, c0_i + r_i, r0_i + dr, &mut best);
        }
    }

    best.map(|(_, row, col)| {
        let center = cell_center(tm, bounds, col, row);
        Vec3::new(center.x, pos.y, center.z)
    })
}

fn consider_walkable_cell(
    tm: &TerrainMap,
    c0: usize,
    r0: usize,
    c: isize,
    r: isize,
    best: &mut Option<(usize, usize, usize)>,
) {
    if c < 0 || r < 0 || c >= tm.width as isize || r >= tm.height as isize {
        return;
    }
    let col = c as usize;
    let row = r as usize;
    if !is_biome_walkable(tm.biomes[row * tm.width + col]) {
        return;
    }
    let dc = col.abs_diff(c0);
    let dr = row.abs_diff(r0);
    let d2 = dc * dc + dr * dr;
    let cand = (d2, row, col);
    if best.is_none_or(|b| cand < b) {
        *best = Some(cand);
    }
}

/// Draws up to `max_tries` candidate `(x, 0.0, z)` points inside `bounds` from `rng`, returning
/// the first one on a walkable cell or `None` if all attempts hit water.
pub fn sample_walkable_pos<R: rand::Rng + ?Sized>(
    rng: &mut R,
    tm: &TerrainMap,
    bounds: &MapBounds,
    max_tries: usize,
) -> Option<Vec3> {
    for _ in 0..max_tries {
        let x = rng.gen_range(bounds.min.x..bounds.max.x);
        let z = rng.gen_range(bounds.min.z..bounds.max.z);
        let pos = Vec3::new(x, 0.0, z);
        if is_pos_walkable(tm, bounds, pos) {
            return Some(pos);
        }
    }
    None
}

/// Constrains agent roots to walkable terrain cells when [`TerrainNavigation`] is enabled.
///
/// Registered in [`crate::core::simulation_schedule::build_tick_schedule`] right after
/// `integrate_physics_system` (before `wrap_coordinates_system`), so torus wrapping and shoreline
/// pushback compose cleanly in zero heap allocations.
#[allow(clippy::type_complexity)]
pub fn terrain_navigation_system(
    policy: Option<Res<TerrainNavigation>>,
    terrain_map: Option<Res<TerrainMap>>,
    bounds: Option<Res<MapBounds>>,
    mut root_query: Query<
        (
            &mut Position,
            &mut Velocity,
            &mut RigidBody,
            &mut LastWalkablePosition,
        ),
        With<Agent>,
    >,
    mut segment_query: Query<
        (&ParentAgent, &mut Position, &mut Velocity, &mut RigidBody),
        Without<Agent>,
    >,
) {
    if !policy.as_deref().is_some_and(|p| p.enabled) {
        return;
    }
    let (Some(tm), Some(bounds)) = (terrain_map.as_deref(), bounds.as_deref()) else {
        return;
    };

    let mut any_reverted = false;
    for (mut pos, mut vel, mut body, mut last) in root_query.iter_mut() {
        let wrapped = wrap_pos(pos.0, bounds);
        if is_pos_walkable(tm, bounds, wrapped) {
            last.pos = wrapped;
            last.reverted_this_tick = false;
        } else {
            let target = if is_pos_walkable(tm, bounds, last.pos) {
                wrap_pos(last.pos, bounds)
            } else {
                snap_to_nearest_walkable(tm, bounds, wrapped).unwrap_or(last.pos)
            };
            let delta = target - pos.0;
            pos.0 = target;
            vel.0 = Vec3::ZERO;
            body.velocity = Vec3::ZERO;
            last.pos = target;
            last.revert_delta = delta;
            last.reverted_this_tick = true;
            any_reverted = true;
        }
    }

    if !any_reverted {
        return;
    }

    for (parent, mut seg_pos, mut seg_vel, mut seg_body) in segment_query.iter_mut() {
        if let Ok((_, _, _, last)) = root_query.get(parent.0) {
            if last.reverted_this_tick {
                seg_pos.0 += last.revert_delta;
                seg_vel.0 = Vec3::ZERO;
                seg_body.velocity = Vec3::ZERO;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ecs::init_world;
    use crate::evolution::genotype::{
        decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
    };

    fn test_bounds() -> MapBounds {
        MapBounds {
            min: Vec3::new(-100.0, 0.0, -100.0),
            max: Vec3::new(100.0, 10.0, 100.0),
        }
    }

    fn grid_terrain(width: usize, height: usize, fill: BiomeType) -> TerrainMap {
        let len = width * height;
        TerrainMap {
            width,
            height,
            elevations: vec![0.5; len],
            moistures: vec![0.5; len],
            temperatures: vec![0.5; len],
            biomes: vec![fill as u8; len],
            flows: vec![0.0; len],
            pois: Vec::new(),
        }
    }

    fn two_segment_genotype() -> MorphologyGenotype {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode {
            id: 0,
            length: 1.0,
            radius: 0.2,
            mass: 1.5,
        });
        g.add_node(MorphologyNode {
            id: 1,
            length: 1.0,
            radius: 0.2,
            mass: 1.0,
        });
        g.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: 1,
            joint_anchor: Vec3::new(1.0, 0.0, 0.0),
            joint_axis: Vec3::new(0.0, 0.0, 1.0),
        });
        g
    }

    #[test]
    fn biome_walkability_classifies_all_biomes() {
        assert!(!is_biome_walkable(BiomeType::DeepOcean as u8));
        assert!(!is_biome_walkable(BiomeType::Ocean as u8));
        for biome in [
            BiomeType::Beach,
            BiomeType::River,
            BiomeType::Grassland,
            BiomeType::TemperateForest,
            BiomeType::BorealForest,
            BiomeType::Rainforest,
            BiomeType::Desert,
            BiomeType::MountainRock,
            BiomeType::Snow,
        ] {
            assert!(is_biome_walkable(biome as u8), "{biome:?} must be walkable");
        }
    }

    #[test]
    fn shoreline_reversion_moves_whole_body_and_respects_wrap_seam() {
        let bounds = test_bounds();
        // 4x4 grid over [-100, 100]^2: each cell is 50 units wide.
        // Column 3 at x in [50, 100) is Grassland; Column 0 at x in [-100, -50) is Ocean.
        let mut tm = grid_terrain(4, 4, BiomeType::Ocean);
        for row in 0..4 {
            tm.biomes[row * 4 + 3] = BiomeType::Grassland as u8;
        }

        let mut world = init_world();
        world.insert_resource(bounds);
        world.insert_resource(tm);
        world.insert_resource(TerrainNavigation { enabled: true });

        let start = Vec3::new(98.0, 0.0, 0.0);
        let root = decode_genotype(
            &mut world,
            &two_segment_genotype(),
            start,
            glam::Quat::IDENTITY,
        );

        let mut sched = Schedule::default();
        sched.add_systems(terrain_navigation_system);

        // Simulate one post-physics tick moving +4.0 in X across +100.0 seam (wraps to -98.0 Ocean).
        let child = {
            let mut q = world.query_filtered::<Entity, (With<ParentAgent>, Without<Agent>)>();
            q.iter(&world).next().expect("child segment exists")
        };
        let child_start = world.get::<Position>(child).expect("child pos").0;
        let expected_offset = child_start - start;

        world.get_mut::<Position>(root).unwrap().0 = Vec3::new(102.0, 0.0, 0.0);
        world.get_mut::<Velocity>(root).unwrap().0 = Vec3::new(4.0, 0.0, 0.0);
        world.get_mut::<RigidBody>(root).unwrap().velocity = Vec3::new(4.0, 0.0, 0.0);
        world.get_mut::<Position>(child).unwrap().0 = child_start + Vec3::new(4.0, 0.0, 0.0);
        world.get_mut::<Velocity>(child).unwrap().0 = Vec3::new(4.0, 0.0, 0.0);
        world.get_mut::<RigidBody>(child).unwrap().velocity = Vec3::new(4.0, 0.0, 0.0);

        sched.run(&mut world);

        assert_eq!(world.get::<Position>(root).unwrap().0, start);
        assert_eq!(world.get::<Velocity>(root).unwrap().0, Vec3::ZERO);
        assert_eq!(world.get::<RigidBody>(root).unwrap().velocity, Vec3::ZERO);
        assert_eq!(
            world.get::<Position>(child).unwrap().0 - world.get::<Position>(root).unwrap().0,
            expected_offset,
            "child segment must move with root on shoreline reversion"
        );
        assert_eq!(world.get::<Velocity>(child).unwrap().0, Vec3::ZERO);
        assert_eq!(world.get::<RigidBody>(child).unwrap().velocity, Vec3::ZERO);
    }

    #[test]
    fn snap_and_water_adjacency_work_deterministically() {
        let bounds = test_bounds();
        let mut tm = grid_terrain(4, 4, BiomeType::DeepOcean);
        tm.biomes[2 * 4 + 2] = BiomeType::Grassland as u8;
        tm.biomes[3 * 4 + 3] = BiomeType::TemperateForest as u8;

        assert!(!is_walkable_cell_next_to_water(&tm, 0, 0));
        assert!(is_walkable_cell_next_to_water(&tm, 2, 2));
        // (3,3) still borders (2,3) and (3,2) which are DeepOcean:
        assert!(is_walkable_cell_next_to_water(&tm, 3, 3));

        // Walkable cell keeps exact coordinate
        let exact_land = cell_center(&tm, &bounds, 2, 2);
        let nudged_land = exact_land + Vec3::new(3.0, 1.25, -2.0);
        assert_eq!(
            snap_to_nearest_walkable(&tm, &bounds, nudged_land),
            Some(nudged_land)
        );

        // Ocean point snaps to nearest walkable cell center preserving y
        let ocean_pt = cell_center(&tm, &bounds, 1, 2) + Vec3::new(0.0, 2.5, 0.0);
        let snapped = snap_to_nearest_walkable(&tm, &bounds, ocean_pt).expect("finds land");
        assert_eq!(snapped, Vec3::new(exact_land.x, 2.5, exact_land.z));
        assert_eq!(
            tm.get_map_indices(snapped, &bounds),
            Some((2, 2)),
            "snapped center must bucket back to (2, 2)"
        );

        // All-ocean returns None cleanly for both snap and sample
        let all_ocean = grid_terrain(4, 4, BiomeType::Ocean);
        assert_eq!(
            snap_to_nearest_walkable(&all_ocean, &bounds, Vec3::ZERO),
            None
        );
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        assert_eq!(sample_walkable_pos(&mut rng, &all_ocean, &bounds, 16), None);
    }
}
