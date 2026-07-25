//! # Dynamic world fields (M3) — climate, water and soil that change through mechanisms.
//!
//! Where the [World Artifact](crate::core::world_artifact) is the slow, static substrate, this is
//! the layer that moves every tick: air temperature, precipitation, surface & ground water, soil
//! moisture / nutrient / organic matter, erosion and turbidity — all as SoA `f32` grids in
//! pre-allocated buffers so the update loops **never allocate** (scenario S17, the project-wide
//! zero-heap-hot-loop rule).
//!
//! Interventions act on these fields THROUGH their mechanism, not by poking a final number: a heat
//! anomaly raises `air_temperature` only inside its region (S15); rainfall feeds the **conserved**
//! water budget precip → infiltration → evaporation → runoff, where total water only changes via
//! declared sources/sinks (S16, and the M3 gate); a dam blocks a cell's outflow so upstream water
//! backs up and downstream discharge falls (S19). This is where M2's `InterventionCommand`s finally
//! drive real physics instead of the reduced reference model.
//!
//! Everything here is deterministic (fixed iteration order, no `HashMap`) so a scenario over these
//! fields replays to the same checksum on the SAME machine. The one caveat to *cross-machine*
//! bit-equality is the `f32::sin` seasonal term in [`DynamicFields::step_climate`] (libm sin is not
//! guaranteed identical across platforms); every other operation is exact IEEE-754.
//!
//! Water reaches a finite equilibrium rather than growing without bound: at a dammed cell or interior
//! pit (no runoff outlet) evaporation is proportional to storage, so it is the implicit stabilizer.
//! (An explicit reservoir with upstream backpressure — where a full dam floods cells further upstream
//! and stores an impoundable pulse — is M8 flood/dam-removal work; the M3 dam is a single-cell hold.)

use crate::core::intervention::{InterventionCommand, InterventionKind, Region};
use crate::core::terrain::TerrainMap;
use serde::{Deserialize, Serialize};

// ---- Tuning constants ------------------------------------------------------------------------

/// Max groundwater a cell can hold (WV) before it is saturated.
pub const GROUND_CAPACITY: f32 = 2.0;
/// Surface water that infiltrates toward groundwater per tick (WV, capped by remaining room).
const INFIL_RATE: f32 = 0.15;
/// Base evaporation fraction per tick (scaled by temperature).
const EVAP_RATE: f32 = 0.03;
/// Surface water retained against runoff (WV); only the excess above this can run off.
const RUNOFF_RETENTION: f32 = 0.02;
/// Fraction of the runoff-eligible surface water that leaves a cell per tick.
const RUNOFF_FRACTION: f32 = 0.4;
/// Baseline precipitation input (WV/tick) before latitude and interventions.
const BASE_PRECIP: f32 = 0.10;
/// Fraction of soil organic matter mineralized to plant-available nitrogen per soil tick.
const SOIL_MINERALIZE: f32 = 0.01;
/// Max soil nitrogen a cell holds (NM).
pub const NITROGEN_CAP: f32 = 5.0;
/// Erosion sensitivity to rainfall × slope.
const EROSION_K: f32 = 2.0;
/// How strongly root cover resists erosion.
const ROOT_RESIST: f32 = 4.0;
const TURB_GAIN: f32 = 0.5;
const TURB_DECAY: f32 = 0.9;

/// `downstream` sentinel: this cell drains OFF the map (a boundary outlet) — its runoff becomes
/// discharge.
pub const OUTLET: usize = usize::MAX;
/// `downstream` sentinel: this cell has no lower neighbour (an interior pit) — its water is retained.
pub const RETAIN: usize = usize::MAX - 1;

// ---- The fields ------------------------------------------------------------------------------

/// The dynamic world fields on the sim grid. Static-ish substrate (`elevation`, `slope`,
/// `root_cover`, `downstream`, `dam`) is derived once from terrain; the rest evolves each tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DynamicFields {
    pub width: usize,
    pub height: usize,

    // Substrate (set once from terrain).
    pub elevation: Vec<f32>,
    pub slope: Vec<f32>,
    pub root_cover: Vec<f32>,
    /// Terrain moisture [0,1] from the world artifact — a driver for base precipitation.
    pub base_moisture: Vec<f32>,
    pub downstream: Vec<usize>,
    pub dam: Vec<bool>,

    // Climate.
    pub air_temperature: Vec<f32>, // normalized [0,1] (M0 field_temperature)
    pub precipitation: Vec<f32>,   // WV/tick input

    // Water storage.
    pub surface_water: Vec<f32>, // WV
    pub groundwater: Vec<f32>,   // WV
    pub soil_moisture: Vec<f32>, // [0,1] saturation (derived from groundwater)

    // Soil chemistry.
    pub soil_nitrogen: Vec<f32>, // NM (plant-available)
    pub soil_organic: Vec<f32>,  // NM (organic matter reservoir)

    // Erosion / water quality.
    pub erosion: Vec<f32>,   // per-tick erosion rate
    pub turbidity: Vec<f32>, // [0,1]

    // Pre-allocated scratch for synchronous runoff routing (zeroed each tick, never reallocated).
    inflow: Vec<f32>,

    // Conservation ledgers (f64), the machine side of the M3 gate.
    pub cum_precip: f64,
    pub cum_evap: f64,
    pub cum_discharge: f64,
    pub initial_water: f64,
    pub cum_nutrient_added: f64,
    pub cum_nutrient_leached: f64,
    pub cum_nutrient_uptake: f64,
    pub initial_nutrient: f64,
}

/// Erosion-resisting root cover per backend `BiomeType` index (0..=10): forests hold soil, desert
/// and rock do not.
fn root_cover_for_biome(b: u8) -> f32 {
    match b {
        0 | 1 => 0.0, // DeepOcean / Ocean
        2 => 0.1,     // Beach
        3 => 0.2,     // River
        4 => 0.5,     // Grassland
        5 => 0.9,     // TemperateForest
        6 => 0.8,     // BorealForest
        7 => 1.0,     // Rainforest
        8 => 0.05,    // Desert
        9 => 0.1,     // MountainRock
        10 => 0.0,    // Snow
        _ => 0.3,
    }
}

impl DynamicFields {
    /// Build the dynamic fields over a terrain map: slope and drainage directions from elevation,
    /// root cover from biome, groundwater seeded from terrain moisture, soil pools seeded uniformly.
    pub fn from_terrain(map: &TerrainMap) -> Self {
        let width = map.width;
        let height = map.height;
        let n = width * height;
        let elevation = map.elevations.clone();

        // Slope: the largest elevation drop to any of the 8 neighbours, clamped to [0,1].
        let mut slope = vec![0.0f32; n];
        // Drainage: steepest-descent neighbour, or an OUTLET (boundary) / RETAIN (interior pit).
        let mut downstream = vec![RETAIN; n];
        for row in 0..height {
            for col in 0..width {
                let i = row * width + col;
                let h = elevation[i];
                let on_boundary = row == 0 || col == 0 || row == height - 1 || col == width - 1;
                let mut max_drop = 0.0f32;
                let mut lowest = i;
                let mut lowest_h = h;
                for dr in -1i64..=1 {
                    for dc in -1i64..=1 {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let nr = row as i64 + dr;
                        let nc = col as i64 + dc;
                        if nr < 0 || nc < 0 || nr >= height as i64 || nc >= width as i64 {
                            continue;
                        }
                        let ni = nr as usize * width + nc as usize;
                        let nh = elevation[ni];
                        max_drop = max_drop.max(h - nh);
                        if nh < lowest_h {
                            lowest_h = nh;
                            lowest = ni;
                        }
                    }
                }
                slope[i] = max_drop.clamp(0.0, 1.0);
                downstream[i] = if lowest != i {
                    lowest // a strictly-lower neighbour exists → route there
                } else if on_boundary {
                    OUTLET // flat/pit on the edge → drains off the map
                } else {
                    RETAIN // interior pit → water pools here
                };
            }
        }

        let mut root_cover = vec![0.0f32; n];
        for i in 0..n.min(map.biomes.len()) {
            root_cover[i] = root_cover_for_biome(map.biomes[i]);
        }

        // Seed groundwater from terrain moisture (already normalized), surface water empty.
        let groundwater: Vec<f32> = (0..n)
            .map(|i| {
                (map.moistures.get(i).copied().unwrap_or(0.0) * GROUND_CAPACITY)
                    .min(GROUND_CAPACITY)
            })
            .collect();
        let surface_water = vec![0.0f32; n];
        let soil_moisture: Vec<f32> = groundwater
            .iter()
            .map(|g| (g / GROUND_CAPACITY).clamp(0.0, 1.0))
            .collect();
        let soil_nitrogen = vec![1.0f32; n];
        let soil_organic = vec![2.0f32; n];
        let base_moisture: Vec<f32> = (0..n)
            .map(|i| map.moistures.get(i).copied().unwrap_or(0.5).clamp(0.0, 1.0))
            .collect();

        let initial_water = surface_water.iter().map(|&v| v as f64).sum::<f64>()
            + groundwater.iter().map(|&v| v as f64).sum::<f64>();
        let initial_nutrient = soil_nitrogen.iter().map(|&v| v as f64).sum::<f64>()
            + soil_organic.iter().map(|&v| v as f64).sum::<f64>();

        Self {
            width,
            height,
            elevation,
            slope,
            root_cover,
            base_moisture,
            downstream,
            dam: vec![false; n],
            air_temperature: vec![0.5f32; n],
            precipitation: vec![0.0f32; n],
            surface_water,
            groundwater,
            soil_moisture,
            soil_nitrogen,
            soil_organic,
            erosion: vec![0.0f32; n],
            turbidity: vec![0.0f32; n],
            inflow: vec![0.0f32; n],
            cum_precip: 0.0,
            cum_evap: 0.0,
            cum_discharge: 0.0,
            initial_water,
            cum_nutrient_added: 0.0,
            cum_nutrient_leached: 0.0,
            cum_nutrient_uptake: 0.0,
            initial_nutrient,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.width * self.height
    }

    /// Place (or lift) a dam on grid cell `(x, y)`. A dammed cell does not release its runoff, so
    /// water backs up upstream and downstream discharge falls (S19).
    pub fn set_dam(&mut self, x: usize, y: usize, present: bool) {
        if x < self.width && y < self.height {
            self.dam[y * self.width + x] = present;
        }
    }

    // ---- M3.1 Climate + anomaly forcing (S15) ------------------------------------------------

    /// Recompute base air temperature and precipitation from latitude, elevation and season, then
    /// apply the active regional interventions: `TemperatureDelta` shifts temperature and
    /// `RainfallDelta` scales precipitation **only inside their region** — cells outside are left at
    /// the base value (S15). No allocation.
    pub fn step_climate(&mut self, tick: u64, season_phase: f32, active: &[&InterventionCommand]) {
        let season = 0.1 * season_phase.sin();
        let hspan = (self.height.max(2) - 1) as f32;
        for row in 0..self.height {
            // Polar-ness in [0,1]: 0 at the equator (mid-map), 1 at the poles (edges).
            let lat_polar = (2.0 * row as f32 / hspan - 1.0).abs();
            for col in 0..self.width {
                let i = row * self.width + col;
                self.air_temperature[i] =
                    (0.7 - 0.45 * lat_polar - 0.2 * self.elevation[i] + season).clamp(0.0, 1.0);
                // Base rainfall rises toward the equator, scales with terrain moisture, and thins
                // with elevation (a coarse rain-shadow proxy). Orographic/wind structure is post-MVP.
                let latitude = 0.4 + 0.6 * (1.0 - lat_polar);
                let moisture = 0.5 + self.base_moisture[i];
                let altitude = 1.0 - 0.3 * self.elevation[i];
                self.precipitation[i] = (BASE_PRECIP * latitude * moisture * altitude).max(0.0);
            }
        }

        for cmd in active {
            if !cmd.active_at(tick) {
                continue;
            }
            let s = cmd.signed_intensity_at(tick);
            match cmd.kind {
                InterventionKind::TemperatureDelta => {
                    self.apply_region(&cmd.region, |t| (t + s).clamp(0.0, 1.0), Field::Temperature)
                }
                InterventionKind::RainfallDelta => {
                    self.apply_region(&cmd.region, |p| (p * (1.0 + s)).max(0.0), Field::Precip)
                }
                _ => {}
            }
        }
    }

    /// Apply `f` to every cell of `field` inside `region` (allocation-free).
    fn apply_region(&mut self, region: &Region, f: impl Fn(f32) -> f32, field: Field) {
        for row in 0..self.height {
            for col in 0..self.width {
                if region.contains(col as u32, row as u32) {
                    let i = row * self.width + col;
                    match field {
                        Field::Temperature => self.air_temperature[i] = f(self.air_temperature[i]),
                        Field::Precip => self.precipitation[i] = f(self.precipitation[i]),
                    }
                }
            }
        }
    }

    // ---- M3.2 Water budget (S16) -------------------------------------------------------------

    /// Advance the water cycle one tick: precipitation input → infiltration (surface→ground) →
    /// evaporation (output) → synchronous runoff routing (to the steepest-descent neighbour, off-map
    /// outlets counted as discharge, dammed cells held). Every source and sink is tracked in the
    /// conservation ledger so [`water_budget_residual`](Self::water_budget_residual) stays near zero
    /// (S16). Allocation-free.
    pub fn step_water(&mut self) {
        let n = self.len();

        // 1) Precipitation input (track the ACTUAL storage increase, so f32 rounding cannot leak).
        for i in 0..n {
            let before = self.surface_water[i];
            self.surface_water[i] += self.precipitation[i];
            self.cum_precip += (self.surface_water[i] - before) as f64;
        }

        // 2) Infiltration: surface → groundwater. Move the ACTUAL amount that left `surface` (not the
        //    nominal `want`), so the pair transfers mass with no source/sink rounding asymmetry.
        for i in 0..n {
            let room = (GROUND_CAPACITY - self.groundwater[i]).max(0.0);
            let want = INFIL_RATE.min(self.surface_water[i]).min(room);
            if want > 0.0 {
                let before = self.surface_water[i];
                self.surface_water[i] -= want;
                let moved = before - self.surface_water[i];
                self.groundwater[i] += moved;
            }
        }

        // 3) Evaporation (output), warmer cells evaporate more.
        for i in 0..n {
            let temp_factor = 0.5 + self.air_temperature[i];
            let target =
                EVAP_RATE * temp_factor * (self.surface_water[i] + 0.1 * self.groundwater[i]);
            let from_surface = target.min(self.surface_water[i]);
            let s_before = self.surface_water[i];
            self.surface_water[i] -= from_surface;
            let removed_s = s_before - self.surface_water[i];
            let from_ground = (target - removed_s).max(0.0).min(self.groundwater[i]);
            let g_before = self.groundwater[i];
            self.groundwater[i] -= from_ground;
            let removed_g = g_before - self.groundwater[i];
            self.cum_evap += (removed_s + removed_g) as f64;
        }

        // 4) Runoff routing — synchronous (scatter into `inflow`, then apply), so order-independent.
        self.inflow.fill(0.0);
        let mut discharge = 0.0f64;
        for i in 0..n {
            if self.dam[i] {
                continue; // a dam holds this cell's water → it backs up
            }
            let dest = self.downstream[i];
            if dest == RETAIN {
                continue;
            }
            let excess = (self.surface_water[i] - RUNOFF_RETENTION).max(0.0);
            let out = excess * RUNOFF_FRACTION;
            if out <= 0.0 {
                continue;
            }
            // Credit the ledger / downstream with the ACTUAL amount removed (not nominal `out`), so an
            // OUTLET cell at a fixed-operand steady state cannot accumulate a same-signed rounding
            // error every tick (a systematic discharge drift).
            let before = self.surface_water[i];
            self.surface_water[i] -= out;
            let moved = before - self.surface_water[i];
            if dest == OUTLET {
                discharge += moved as f64; // leaves the map
            } else {
                self.inflow[dest] += moved;
            }
        }
        for i in 0..n {
            self.surface_water[i] += self.inflow[i];
        }
        self.cum_discharge += discharge;

        // 5) Derived soil moisture.
        for i in 0..n {
            self.soil_moisture[i] = (self.groundwater[i] / GROUND_CAPACITY).clamp(0.0, 1.0);
        }
    }

    /// Total stored water (surface + ground), WV.
    pub fn total_water(&self) -> f64 {
        self.surface_water.iter().map(|&v| v as f64).sum::<f64>()
            + self.groundwater.iter().map(|&v| v as f64).sum::<f64>()
    }

    /// The water-budget residual: `stored − (initial + precip − evap − discharge)`. Zero means
    /// perfect conservation; the M3 gate keeps `|residual|` under a small tolerance (S16).
    pub fn water_budget_residual(&self) -> f64 {
        self.total_water()
            - (self.initial_water + self.cum_precip - self.cum_evap - self.cum_discharge)
    }

    // ---- M3.3 Soil (S17) ---------------------------------------------------------------------

    /// Advance soil chemistry: mineralize organic matter into plant-available nitrogen (a conserving
    /// transfer — only up to the remaining nitrogen room, so nothing is discarded at the cap), then
    /// apply active `AddNutrient` interventions. Nutrient is conserved: the full intervention input
    /// is a declared SOURCE (`cum_nutrient_added`) and anything the soil cannot hold is a declared
    /// SINK (`cum_nutrient_leached`), so the budget never silently gains or loses (the M3 gate). All
    /// fields stay bounded; allocation-free.
    pub fn step_soil(&mut self, tick: u64, active: &[&InterventionCommand]) {
        let n = self.len();
        for i in 0..n {
            // Transfer organic → nitrogen, but never beyond what the nitrogen pool can hold, so the
            // clamp can never destroy mass.
            let room = (NITROGEN_CAP - self.soil_nitrogen[i]).max(0.0);
            let m = (SOIL_MINERALIZE * self.soil_organic[i]).min(room);
            if m > 0.0 {
                let before = self.soil_organic[i];
                self.soil_organic[i] -= m;
                let moved = before - self.soil_organic[i]; // exact amount that left organic
                self.soil_nitrogen[i] += moved;
            }
        }
        for cmd in active {
            if !cmd.active_at(tick) || cmd.kind != InterventionKind::AddNutrient {
                continue;
            }
            let add = cmd.signed_intensity_at(tick).abs();
            if add <= 0.0 {
                continue;
            }
            for row in 0..self.height {
                for col in 0..self.width {
                    if cmd.region.contains(col as u32, row as u32) {
                        let i = row * self.width + col;
                        let before = self.soil_nitrogen[i];
                        let room = (NITROGEN_CAP - before).max(0.0);
                        let absorbed = add.min(room);
                        self.soil_nitrogen[i] = before + absorbed;
                        self.cum_nutrient_added += add as f64; // full input = declared source
                        self.cum_nutrient_leached += (add - absorbed) as f64; // excess = declared sink
                    }
                }
            }
        }
    }

    /// Total soil nutrient (nitrogen + organic), NM.
    pub fn total_nutrient(&self) -> f64 {
        self.soil_nitrogen.iter().map(|&v| v as f64).sum::<f64>()
            + self.soil_organic.iter().map(|&v| v as f64).sum::<f64>()
    }

    /// The nutrient-budget residual: `stored − (initial + added − leached − uptake)`. Zero means the
    /// nutrient pool only changed through declared sources/sinks (the M3 gate); kept under tolerance.
    pub fn nutrient_budget_residual(&self) -> f64 {
        self.total_nutrient()
            - (self.initial_nutrient + self.cum_nutrient_added
                - self.cum_nutrient_leached
                - self.cum_nutrient_uptake)
    }

    // ---- M3.4 Erosion / turbidity (S18) ------------------------------------------------------

    /// Recompute per-cell erosion — rising with rainfall and slope, falling with root cover — and
    /// feed the (decaying) turbidity field. Allocation-free.
    pub fn step_erosion(&mut self) {
        let n = self.len();
        for i in 0..n {
            let e = EROSION_K * self.precipitation[i] * self.slope[i]
                / (1.0 + ROOT_RESIST * self.root_cover[i]);
            self.erosion[i] = e;
            self.turbidity[i] = (self.turbidity[i] * TURB_DECAY + e * TURB_GAIN).clamp(0.0, 1.0);
        }
    }
}

/// Which climate field a regional intervention writes.
#[derive(Clone, Copy)]
enum Field {
    Temperature,
    Precip,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::intervention::{Curve, InterventionCommand, Region};
    use crate::core::terrain::{MapSettings, TerrainMap};

    fn tiny_terrain() -> TerrainMap {
        // A small deterministic terrain to build fields from.
        TerrainMap::generate(&MapSettings {
            width: 16,
            height: 16,
            ..MapSettings::default()
        })
    }

    fn temp_anomaly(region: Region) -> InterventionCommand {
        InterventionCommand {
            id: 1,
            cause_id: 1,
            kind: InterventionKind::TemperatureDelta,
            region,
            start_tick: 0,
            duration_ticks: 1000,
            intensity: 0.3,
            signed_negative: false, // warming
            curve: Curve::Step,
            reversible: true,
        }
    }

    #[test]
    fn s15_heat_anomaly_is_confined_to_its_region() {
        let terrain = tiny_terrain();
        // Baseline climate with no anomaly.
        let mut base = DynamicFields::from_terrain(&terrain);
        base.step_climate(10, 0.0, &[]);

        // Same climate with a heat anomaly over a rectangle.
        let region = Region::Rect {
            min_x: 4,
            min_y: 4,
            max_x: 7,
            max_y: 7,
        };
        let cmd = temp_anomaly(region);
        let mut hot = DynamicFields::from_terrain(&terrain);
        hot.step_climate(10, 0.0, &[&cmd]);

        for row in 0..hot.height {
            for col in 0..hot.width {
                let i = row * hot.width + col;
                if region.contains(col as u32, row as u32) {
                    // Magnitude, not just sign: the applied shift is exactly the intensity (0.3),
                    // clamped to [0,1] — pinning the normalized-units contract.
                    assert_eq!(
                        hot.air_temperature[i],
                        (base.air_temperature[i] + 0.3).clamp(0.0, 1.0),
                        "temp inside anomaly must shift by exactly the intensity at ({col},{row})"
                    );
                } else {
                    assert_eq!(
                        hot.air_temperature[i], base.air_temperature[i],
                        "temp OUTSIDE anomaly must be unchanged at ({col},{row})"
                    );
                }
            }
        }
    }

    #[test]
    fn s16_water_budget_is_conserved() {
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        for tick in 1..=200u64 {
            f.step_climate(tick, 0.0, &[]);
            f.step_water();
            // Conservation must hold at every step, not just the end.
            let residual = f.water_budget_residual();
            let scale = (f.initial_water + f.cum_precip).max(1.0);
            assert!(
                residual.abs() / scale < 1e-4,
                "water budget drifted at tick {tick}: residual {residual}, scale {scale}"
            );
        }
        // The cycle actually moved water (precip fell, some evaporated/discharged).
        assert!(f.cum_precip > 0.0);
        assert!(f.cum_evap + f.cum_discharge > 0.0);
    }

    #[test]
    fn s18_erosion_rises_with_rainfall_and_slope_and_falls_with_roots() {
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        let n = f.width * f.height;
        // Neutralize the substrate, then vary one factor at a time across three cells.
        for i in 0..n {
            f.precipitation[i] = 0.1;
            f.slope[i] = 0.3;
            f.root_cover[i] = 0.2;
        }
        // Cell A: baseline. B: more rain. C: steeper. D: more root cover.
        f.precipitation[1] = 0.2; // B
        f.slope[2] = 0.6; // C
        f.root_cover[3] = 0.9; // D
        f.step_erosion();
        assert!(f.erosion[1] > f.erosion[0], "more rain → more erosion");
        assert!(f.erosion[2] > f.erosion[0], "steeper → more erosion");
        assert!(
            f.erosion[3] < f.erosion[0],
            "more root cover → less erosion"
        );
    }

    #[test]
    fn s19_dam_raises_upstream_storage_and_cuts_downstream_discharge() {
        // A west→east elevation ramp so water drains toward the east edge (an outlet).
        let w = 12usize;
        let h = 3usize;
        let mut map = TerrainMap::generate(&MapSettings {
            width: w,
            height: h,
            ..MapSettings::default()
        });
        map.width = w;
        map.height = h;
        map.elevations = (0..w * h)
            .map(|i| {
                let col = i % w;
                1.0 - col as f32 / (w - 1) as f32 // high in the west, low in the east
            })
            .collect();
        map.moistures = vec![0.0; w * h];
        map.biomes = vec![4u8; w * h]; // grassland
        map.temperatures = vec![0.5; w * h];
        map.flows = vec![0.0; w * h];

        let run = |dam: bool| {
            let mut f = DynamicFields::from_terrain(&map);
            if dam {
                for row in 0..h {
                    f.set_dam(w / 2, row, true); // dam across the middle column
                }
            }
            // Rain uniformly and let it drain for a while.
            for _ in 0..150 {
                for i in 0..w * h {
                    f.precipitation[i] = 0.1;
                }
                f.step_water();
            }
            // Impounded water = the dam column and west of it. In this MVP the dam is a single-cell
            // HOLD (no upstream backpressure), so the impoundment concentrates in the dammed column;
            // a true reservoir that floods progressively further upstream is M8 flood/dam-removal work.
            let upstream: f64 = (0..w * h)
                .filter(|&i| i % w <= w / 2)
                .map(|i| f.surface_water[i] as f64)
                .sum();
            (upstream, f.cum_discharge)
        };

        let (up_free, discharge_free) = run(false);
        let (up_dam, discharge_dam) = run(true);
        assert!(
            up_dam > up_free,
            "dam should raise upstream storage: {up_dam} vs {up_free}"
        );
        assert!(
            discharge_dam < discharge_free,
            "dam should cut downstream discharge: {discharge_dam} vs {discharge_free}"
        );
    }

    #[test]
    fn s20_save_load_preserves_fields_and_budget() {
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        let nutrient_cmd = InterventionCommand {
            id: 2,
            cause_id: 2,
            kind: InterventionKind::AddNutrient,
            region: Region::Global,
            start_tick: 0,
            duration_ticks: 1000,
            intensity: 0.2,
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        };
        for tick in 1..=50u64 {
            f.step_climate(tick, 0.0, &[]);
            f.step_water();
            f.step_soil(tick, &[&nutrient_cmd]);
            f.step_erosion();
        }

        let json = serde_json::to_string(&f).expect("serialize dynamic fields");
        let loaded: DynamicFields = serde_json::from_str(&json).expect("deserialize");

        // Per-cell fields survive the round-trip exactly.
        assert_eq!(loaded.surface_water, f.surface_water);
        assert_eq!(loaded.groundwater, f.groundwater);
        assert_eq!(loaded.soil_nitrogen, f.soil_nitrogen);
        // The f64 conservation ledgers survive to within serde_json's float precision — its default
        // parser is accurate to ±1 ULP (the `float_roundtrip` feature is not enabled), so an
        // accumulated budget is preserved to ~1e-13 relative, not bit-for-bit.
        assert!((loaded.cum_precip - f.cum_precip).abs() < 1e-6);
        assert!((loaded.cum_nutrient_added - f.cum_nutrient_added).abs() < 1e-6);
        assert!((loaded.water_budget_residual() - f.water_budget_residual()).abs() < 1e-6);

        // The nutrient source was tracked, and the loaded field keeps stepping conservatively.
        assert!(f.cum_nutrient_added > 0.0);
        let mut cont = loaded;
        for tick in 51..=60u64 {
            cont.step_climate(tick, 0.0, &[]);
            cont.step_water();
        }
        let scale = (cont.initial_water + cont.cum_precip).max(1.0);
        assert!(cont.water_budget_residual().abs() / scale < 1e-4);
    }

    #[test]
    fn s17_soil_fields_stay_bounded() {
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        let cmd = InterventionCommand {
            id: 3,
            cause_id: 3,
            kind: InterventionKind::AddNutrient,
            region: Region::Global,
            start_tick: 0,
            duration_ticks: 10000,
            intensity: 1.0, // hammer it to test the clamp
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        };
        for tick in 1..=500u64 {
            f.step_soil(tick, &[&cmd]);
        }
        for i in 0..f.width * f.height {
            assert!(
                (0.0..=NITROGEN_CAP).contains(&f.soil_nitrogen[i]),
                "nitrogen out of bounds: {}",
                f.soil_nitrogen[i]
            );
            assert!(f.soil_organic[i] >= 0.0);
        }
    }

    #[test]
    fn s17_nutrient_budget_is_conserved_including_leaching() {
        // Hammer AddNutrient so the soil saturates and excess must LEACH (a declared sink) rather
        // than silently vanish at the cap — the nutrient half of the M3 conservation gate.
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        let cmd = InterventionCommand {
            id: 4,
            cause_id: 4,
            kind: InterventionKind::AddNutrient,
            region: Region::Global,
            start_tick: 0,
            duration_ticks: 10000,
            intensity: 0.5,
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        };
        for tick in 1..=300u64 {
            f.step_soil(tick, &[&cmd]);
            let residual = f.nutrient_budget_residual();
            let scale = (f.initial_nutrient + f.cum_nutrient_added).max(1.0);
            assert!(
                residual.abs() / scale < 1e-4,
                "nutrient budget drifted at tick {tick}: residual {residual}, scale {scale}"
            );
        }
        // Saturation actually forced some nutrient to leach (proving the sink is exercised, not that
        // the clamp silently swallowed it).
        assert!(
            f.cum_nutrient_leached > 0.0,
            "a saturated soil should leach excess nutrient"
        );
    }

    #[test]
    fn s15_rainfall_change_is_confined_to_its_region() {
        // The precipitation analogue of the S15 heat-anomaly confinement test.
        let terrain = tiny_terrain();
        let mut base = DynamicFields::from_terrain(&terrain);
        base.step_climate(5, 0.0, &[]);
        let region = Region::Rect {
            min_x: 2,
            min_y: 2,
            max_x: 5,
            max_y: 5,
        };
        let rain = InterventionCommand {
            id: 5,
            cause_id: 5,
            kind: InterventionKind::RainfallDelta,
            region,
            start_tick: 0,
            duration_ticks: 1000,
            intensity: 0.3,
            signed_negative: true, // −30%
            curve: Curve::Step,
            reversible: true,
        };
        let mut dry = DynamicFields::from_terrain(&terrain);
        dry.step_climate(5, 0.0, &[&rain]);
        for row in 0..dry.height {
            for col in 0..dry.width {
                let i = row * dry.width + col;
                if region.contains(col as u32, row as u32) {
                    assert!(
                        (dry.precipitation[i] - base.precipitation[i] * 0.7).abs() < 1e-6,
                        "rainfall inside region must scale by (1+s) at ({col},{row})"
                    );
                } else {
                    assert_eq!(
                        dry.precipitation[i], base.precipitation[i],
                        "rainfall OUTSIDE region must be unchanged at ({col},{row})"
                    );
                }
            }
        }
    }

    #[test]
    fn s16_combined_update_conserves_water_and_nutrient_under_interventions() {
        // The M3 gate over the REAL §4.4 update order (climate → water → soil → erosion) with a
        // rainfall AND a nutrient intervention active simultaneously — both budgets conserved every
        // tick, not just per-subsystem in isolation.
        let terrain = tiny_terrain();
        let mut f = DynamicFields::from_terrain(&terrain);
        let rain = InterventionCommand {
            id: 6,
            cause_id: 6,
            kind: InterventionKind::RainfallDelta,
            region: Region::Global,
            start_tick: 0,
            duration_ticks: 1000,
            intensity: 0.3,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        };
        let nutrient = InterventionCommand {
            id: 7,
            cause_id: 7,
            kind: InterventionKind::AddNutrient,
            region: Region::Global,
            start_tick: 0,
            duration_ticks: 1000,
            intensity: 0.4,
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        };
        let active: Vec<&InterventionCommand> = vec![&rain, &nutrient];
        for tick in 1..=200u64 {
            f.step_climate(tick, tick as f32 * 0.01, &active);
            f.step_water();
            f.step_soil(tick, &active);
            f.step_erosion();
            let w = f.water_budget_residual();
            let ws = (f.initial_water + f.cum_precip).max(1.0);
            assert!(
                w.abs() / ws < 1e-4,
                "water budget drifted at tick {tick}: {w}"
            );
            let nu = f.nutrient_budget_residual();
            let ns = (f.initial_nutrient + f.cum_nutrient_added).max(1.0);
            assert!(
                nu.abs() / ns < 1e-4,
                "nutrient budget drifted at tick {tick}: {nu}"
            );
        }
        assert!(f.cum_precip > 0.0 && f.cum_nutrient_added > 0.0);
    }
}
