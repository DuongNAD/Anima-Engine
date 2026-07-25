//! # Canonical simulation rules — the M0 scientific contract, in code.
//!
//! This module is the single machine-checkable source of truth for the invariants every later
//! milestone (M1–M11) is allowed to depend on: the fixed time step, the world coordinate system,
//! the biome-taxonomy sizes, the conserved energy pool and the canonical unit of every state
//! variable. Its prose companion is `SIMULATION_RULES.md` at the repo root; when a constant here
//! changes, that document must change with it (and vice-versa).
//!
//! The M0 gate is: *do not build causal systems while units, conservation pools and the time
//! scale are still ambiguous.* The tests at the bottom enforce exactly that — the unit registry
//! and closed-energy conservation are machine-checked (scenario **S01**) and the cell↔world
//! coordinate round-trip is machine-checked (scenario **S03**). The biome taxonomy contract
//! (**S02**) lives beside the biome maps in [`crate::core::world_artifact`].
//!
//! Nothing here allocates in or hooks into the 60 Hz hot loop; it is pure constants and pure
//! functions so the contract is cheap to import and cheap to test.

use crate::core::resources::MapBounds;

// ---- Time -----------------------------------------------------------------------------------

// The time laws now live in `anima-domain` (G2 task 1) — the headless runner and the live world
// must agree on them or a manifest replayed in one does not describe the other. Re-exported rather
// than redeclared so this units table stays one document and the S01 checks below still read
// against the same names.
pub use anima_domain::laws::{
    SECONDS_PER_YEAR, TICKS_PER_EPOCH, TICKS_PER_YEAR, TICK_DT_SECONDS, TICK_HZ,
};

// ---- Space ----------------------------------------------------------------------------------

/// Default working resolution of the backend simulation grid, cells per side
/// (`MapSettings::default`, `core/terrain.rs`).
pub const DEFAULT_GRID_DIM: usize = 128;

/// Horizontal world-space bounds per axis, in world-units, from `MapBounds::default`
/// (`min.xz = -100`, `max.xz = +100` → a 200×200 unit square).
pub const WORLD_MIN_XZ: f32 = -100.0;
/// See [`WORLD_MIN_XZ`].
pub const WORLD_MAX_XZ: f32 = 100.0;

/// Vertical world-space bounds, in world-units: normalized elevation `[0,1]` maps onto `y = 0..10`.
pub const WORLD_MIN_Y: f32 = 0.0;
/// See [`WORLD_MIN_Y`].
pub const WORLD_MAX_Y: f32 = 10.0;

// ---- Biome taxonomy sizes -------------------------------------------------------------------

/// Canonical (new) biome taxonomy size — the frontend `Biome` enum in `worldGen.ts`.
pub const CANONICAL_BIOME_COUNT: u8 = 22;

/// Legacy biome taxonomy size — the backend `BiomeType` enum in `core/terrain.rs`.
pub const LEGACY_BIOME_COUNT: u8 = 11;

// ---- Coordinate transforms ------------------------------------------------------------------
//
// Four spaces, one convention (see COORDINATE_CONTRACT.md):
//   • cell     — integer (ix, iy), 0 ≤ ix < width, 0 ≤ iy < height; flat index i = iy*width + ix.
//   • uv       — normalized [0,1]²; a cell's *centre* is uv = ((ix+0.5)/width, (iy+0.5)/height).
//   • world    — backend sim space; x,z ∈ [WORLD_MIN_XZ, WORLD_MAX_XZ], y = elevation·WORLD_MAX_Y.
//   • render   — the 3D scene; a pure scale of `world`, defined frontend-side.
//
// The world→cell direction reproduces `TerrainMap::get_map_indices` exactly (floor bucketing),
// so a cell's centre always buckets back to that same cell — the property S03 checks.

/// Flat row-major index of cell `(ix, iy)` in a `width`-wide grid.
#[inline]
pub fn cell_index(ix: usize, iy: usize, width: usize) -> usize {
    iy * width + ix
}

/// Normalized centre `(u, v)` of cell `(ix, iy)` in a `width × height` grid.
#[inline]
pub fn cell_center_uv(ix: usize, iy: usize, width: usize, height: usize) -> (f32, f32) {
    (
        (ix as f32 + 0.5) / width as f32,
        (iy as f32 + 0.5) / height as f32,
    )
}

/// Bucket a normalized `(u, v)` (each expected in `[0,1]`) to its cell `(ix, iy)`. Uses the same
/// `floor(coord · dim)` rule as `TerrainMap::get_map_indices`, clamped to the last cell.
#[inline]
pub fn uv_to_cell(u: f32, v: f32, width: usize, height: usize) -> (usize, usize) {
    let ix = ((u * width as f32) as usize).min(width - 1);
    let iy = ((v * height as f32) as usize).min(height - 1);
    (ix, iy)
}

/// World-space `(x, z)` of the centre of cell `(ix, iy)` for a grid covering `bounds`.
#[inline]
pub fn cell_center_to_world_xz(
    ix: usize,
    iy: usize,
    width: usize,
    height: usize,
    bounds: &MapBounds,
) -> (f32, f32) {
    let (u, v) = cell_center_uv(ix, iy, width, height);
    (
        bounds.min.x + u * (bounds.max.x - bounds.min.x),
        bounds.min.z + v * (bounds.max.z - bounds.min.z),
    )
}

/// Bucket a world-space `(x, z)` to its cell, or `None` if the point lies outside `bounds`. This
/// is the canonical inverse of [`cell_center_to_world_xz`] and matches
/// `TerrainMap::get_map_indices`.
#[inline]
pub fn world_xz_to_cell(
    x: f32,
    z: f32,
    width: usize,
    height: usize,
    bounds: &MapBounds,
) -> Option<(usize, usize)> {
    let x_range = bounds.max.x - bounds.min.x;
    let z_range = bounds.max.z - bounds.min.z;
    if x_range <= 0.0 || z_range <= 0.0 {
        return None;
    }
    let u = (x - bounds.min.x) / x_range;
    let v = (z - bounds.min.z) / z_range;
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }
    Some(uv_to_cell(u, v, width, height))
}

// ---- State variables, units and conservation -----------------------------------------------

/// How a state variable relates to the engine's conservation laws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conservation {
    /// Member of the closed energy/biomass ledger [`crate::core::ecology::EcosystemBiomass`]
    /// (`plants + animals + detritus` stays constant absent an explicit source/sink).
    ClosedEnergy,
    /// Declared here for a later milestone (M3 dynamic fields) but **not yet conserved** by the
    /// engine. Naming it now keeps the contract honest about what is and isn't tracked.
    DeferredM3,
    /// An observation/field that is not a conserved quantity (e.g. a normalized terrain field).
    None,
}

/// One canonical state variable: its name, unit, valid range and conservation role. This slice is
/// the machine side of the "bảng đơn vị" (units table) in `SIMULATION_RULES.md`; the S01 test
/// asserts it is total and internally consistent, so no later system can silently introduce an
/// undeclared quantity or an undeclared unit.
#[derive(Clone, Copy, Debug)]
pub struct StateVariable {
    /// Stable identifier used in code, docs and the units table.
    pub name: &'static str,
    /// Canonical unit string. `EU` = biomass-equivalent energy unit; `WU` = water-reserve unit.
    pub unit: &'static str,
    /// Inclusive lower bound of the valid range.
    pub min: f64,
    /// Inclusive upper bound of the valid range (`f64::INFINITY` if only bounded below).
    pub max: f64,
    /// Conservation role.
    pub conservation: Conservation,
}

/// The canonical registry of MVP state variables. Terrain fields are normalized `[0,1]`; the two
/// meanings of "temperature" are split explicitly (`field_temperature` normalized vs
/// `body_temperature` in °C) — resolving that ambiguity is a core reason M0 exists.
pub const STATE_VARIABLES: &[StateVariable] = &[
    StateVariable {
        name: "elevation",
        unit: "normalized [0,1] (→ y = value · 10 world-units)",
        min: 0.0,
        max: 1.0,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "moisture",
        unit: "normalized [0,1]",
        min: 0.0,
        max: 1.0,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "field_temperature",
        unit: "normalized [0,1] (cold→hot)",
        min: 0.0,
        max: 1.0,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "flow",
        unit: "normalized [0,1] (river flow accumulation)",
        min: 0.0,
        max: 1.0,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "body_temperature",
        unit: "°C",
        min: 30.0,
        max: 45.0,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "energy",
        unit: "EU (biomass-equivalent energy)",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::ClosedEnergy,
    },
    StateVariable {
        name: "hydration",
        unit: "WU (water reserve)",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::None,
    },
    StateVariable {
        name: "plants",
        unit: "EU (biomass-equivalent energy)",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::ClosedEnergy,
    },
    StateVariable {
        name: "animals",
        unit: "EU (biomass-equivalent energy)",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::ClosedEnergy,
    },
    StateVariable {
        name: "detritus",
        unit: "EU (biomass-equivalent energy)",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::ClosedEnergy,
    },
    // ---- Declared for M3, not yet conserved -------------------------------------------------
    StateVariable {
        name: "surface_water",
        unit: "WV (water volume) — deferred M3",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::DeferredM3,
    },
    StateVariable {
        name: "soil_nutrient",
        unit: "NM (nutrient mass) — deferred M3",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::DeferredM3,
    },
    StateVariable {
        name: "toxin",
        unit: "PM (pollutant mass) — deferred M3",
        min: 0.0,
        max: f64::INFINITY,
        conservation: Conservation::DeferredM3,
    },
];

/// Look up a canonical state variable by name.
pub fn state_variable(name: &str) -> Option<&'static StateVariable> {
    STATE_VARIABLES.iter().find(|v| v.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ecology::EcosystemBiomass;

    // ---- S01: units + closed-energy conservation are machine-checked ------------------------

    #[test]
    fn s01_unit_registry_is_total_and_consistent() {
        assert!(!STATE_VARIABLES.is_empty());
        for v in STATE_VARIABLES {
            assert!(!v.name.is_empty(), "a state variable has no name");
            assert!(!v.unit.is_empty(), "state variable {} has no unit", v.name);
            assert!(
                v.min <= v.max,
                "state variable {} has min > max ({} > {})",
                v.name,
                v.min,
                v.max
            );
            // Normalized fields must declare exactly the [0,1] range.
            if v.unit.starts_with("normalized [0,1]") {
                assert_eq!(v.min, 0.0, "{} is normalized but min != 0", v.name);
                assert_eq!(v.max, 1.0, "{} is normalized but max != 1", v.name);
            }
        }
        // The three closed-energy compartments of `EcosystemBiomass` must all be declared.
        for pool in ["plants", "animals", "detritus"] {
            let sv = state_variable(pool).unwrap_or_else(|| panic!("{pool} missing from registry"));
            assert_eq!(sv.conservation, Conservation::ClosedEnergy);
        }
        // Names are unique.
        for (i, a) in STATE_VARIABLES.iter().enumerate() {
            for b in &STATE_VARIABLES[i + 1..] {
                assert_ne!(a.name, b.name, "duplicate state variable {}", a.name);
            }
        }
    }

    #[test]
    fn s01_closed_energy_ledger_conserves_under_transfer() {
        // The canonical energy rule: energy ≡ biomass-equivalent and moves plants → animals →
        // detritus → plants without being created or destroyed. Drive a transfer loop and assert
        // the total is invariant to within f64 rounding.
        let mut pool = EcosystemBiomass {
            detritus: 5.0,
            plants: 40.0,
            animals: 12.0,
        };
        let total0 = pool.total();
        for _ in 0..1000 {
            // graze: plants → animals
            let g = 0.10 * pool.plants;
            pool.plants -= g;
            pool.animals += g;
            // respire/die: animals → detritus
            let d = 0.08 * pool.animals;
            pool.animals -= d;
            pool.detritus += d;
            // regrow: detritus → plants
            let r = 0.05 * pool.detritus;
            pool.detritus -= r;
            pool.plants += r;
            assert!(
                (pool.total() - total0).abs() < 1e-9,
                "closed-energy total drifted: {} vs {}",
                pool.total(),
                total0
            );
        }
    }

    // ---- S03: cell ↔ world coordinate round-trip is machine-checked -------------------------

    #[test]
    fn s03_cell_center_round_trips_through_world() {
        let bounds = MapBounds::default();
        for &(w, h) in &[(16usize, 16usize), (128, 128), (200, 137)] {
            for iy in 0..h {
                for ix in 0..w {
                    let (x, z) = cell_center_to_world_xz(ix, iy, w, h, &bounds);
                    let back = world_xz_to_cell(x, z, w, h, &bounds)
                        .expect("cell centre must fall inside bounds");
                    assert_eq!(
                        back,
                        (ix, iy),
                        "cell ({ix},{iy}) at {w}x{h} did not round-trip (got {back:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn s03_out_of_bounds_world_point_has_no_cell() {
        let bounds = MapBounds::default();
        assert!(world_xz_to_cell(WORLD_MIN_XZ - 1.0, 0.0, 128, 128, &bounds).is_none());
        assert!(world_xz_to_cell(0.0, WORLD_MAX_XZ + 1.0, 128, 128, &bounds).is_none());
        // Corners are inclusive and resolve to the extreme cells.
        assert_eq!(
            world_xz_to_cell(WORLD_MIN_XZ, WORLD_MIN_XZ, 128, 128, &bounds),
            Some((0, 0))
        );
        assert_eq!(
            world_xz_to_cell(WORLD_MAX_XZ, WORLD_MAX_XZ, 128, 128, &bounds),
            Some((127, 127))
        );
    }

    #[test]
    fn time_constants_are_self_consistent() {
        assert!((TICK_DT_SECONDS - 1.0 / 60.0).abs() < 1e-12);
        assert_eq!(TICKS_PER_YEAR, (SECONDS_PER_YEAR * TICK_HZ) as u64);
    }
}
