//! # Interventions (M2.2) — spatial + temporal commands with a cause id.
//!
//! An [`InterventionCommand`] is how the user (or a scenario) acts on the world. Unlike the legacy
//! `EnvironmentalEvent` — which pokes a final number directly (`food_count`, `temp_target`) — an
//! intervention is *declarative*: it names a `kind`, a `region`, a start tick, a duration, an
//! intensity and a ramp curve, plus the [`CauseId`] that every downstream effect will be traced to
//! (S12). The simulation reads active interventions each tick and applies them **through** its own
//! mechanisms, so effects propagate causally instead of teleporting to the final quantity
//! ("no magic effects", plan §2.2).
//!
//! Pure and Bevy-free so the scenario runner (S11/S13/S14) can drive it headlessly and
//! deterministically.

use crate::causal::CauseId;
use serde::{Deserialize, Serialize};

/// The spatial extent an intervention acts on. Cell coordinates are grid indices; `Radius` is in
/// world/grid units against a cell centre.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Region {
    /// The whole world.
    Global,
    /// A single grid cell.
    Cell { x: u32, y: u32 },
    /// An inclusive rectangle of grid cells.
    Rect {
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
    },
    /// A disc of radius `r` (grid units) centred at `(cx, cy)`.
    Radius { cx: f32, cy: f32, r: f32 },
}

impl Region {
    /// Whether grid cell `(x, y)` lies inside this region.
    pub fn contains(&self, x: u32, y: u32) -> bool {
        match *self {
            Region::Global => true,
            Region::Cell { x: cx, y: cy } => x == cx && y == cy,
            Region::Rect {
                min_x,
                min_y,
                max_x,
                max_y,
            } => x >= min_x && x <= max_x && y >= min_y && y <= max_y,
            Region::Radius { cx, cy, r } => {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                dx * dx + dy * dy <= r * r
            }
        }
    }
}

/// How intensity is shaped across the active window (`progress` runs 0→1 over the duration).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Curve {
    /// Full intensity for the whole window (a sustained forcing).
    Step,
    /// Ramps 0→1 across the window (a gradually building pressure).
    RampUp,
    /// Ramps 1→0 across the window (a decaying pulse).
    RampDown,
    /// 0→1→0 across the window (a transient spike peaking at the middle).
    Triangle,
}

impl Curve {
    /// Intensity multiplier in `[0, 1]` for a normalized `progress` in `[0, 1]`.
    pub fn factor(&self, progress: f32) -> f32 {
        let p = progress.clamp(0.0, 1.0);
        match self {
            Curve::Step => 1.0,
            Curve::RampUp => p,
            Curve::RampDown => 1.0 - p,
            Curve::Triangle => 1.0 - (2.0 * p - 1.0).abs(),
        }
    }
}

/// What an intervention does. Kinds mirror the plan's impact matrix (§6); more are added as the
/// abiotic/biotic systems land in M3–M8. The `f32` payload is interpreted per-kind via `intensity`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InterventionKind {
    /// Fractional change to precipitation (intensity 0.3 = −30% at full strength; sign in `signed`).
    RainfallDelta,
    /// Air-temperature shift; `intensity` is the shift magnitude in the CONSUMING field's units — a
    /// normalized [0,1] shift for the dynamic `air_temperature` field (`DynamicFields`), where 0.1
    /// spans a tenth of the whole temperature range. (Not °C: the M0 contract reserves °C for an
    /// organism's `body_temperature`, whereas the field temperature is normalized.)
    TemperatureDelta,
    /// Fractional canopy/root removal in the region (deforestation).
    Deforest,
    /// Fractional cull of predators in the region.
    RemovePredators,
    /// Nutrient/fertility boost in the region.
    AddNutrient,
}

/// A declarative intervention: apply `kind` at `intensity` (shaped by `curve`) to `region`, over
/// `[start_tick, start_tick + duration_ticks)`, attributed to `cause_id`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InterventionCommand {
    pub id: u32,
    pub cause_id: CauseId,
    pub kind: InterventionKind,
    pub region: Region,
    pub start_tick: u64,
    /// Length of the active window in base ticks; `0` means a single-tick (instantaneous) event.
    pub duration_ticks: u64,
    /// Base magnitude; meaning is per-`kind`. Combined with the `curve` factor and `signed`.
    pub intensity: f32,
    /// `true` if the intervention decreases the target (e.g. rainfall −30%), `false` if it increases.
    pub signed_negative: bool,
    pub curve: Curve,
    /// Whether the forcing is removed (world relaxes) once the window ends, vs a permanent change.
    pub reversible: bool,
}

impl InterventionCommand {
    /// The effective length of the active window (a `0` duration counts as one tick).
    pub fn effective_duration(&self) -> u64 {
        self.duration_ticks.max(1)
    }

    /// Whether this intervention is active at `tick`: `start <= tick < start + effective_duration`.
    pub fn active_at(&self, tick: u64) -> bool {
        tick >= self.start_tick && tick < self.start_tick + self.effective_duration()
    }

    /// The signed applied magnitude at `tick` (0 outside the active window), following the ramp
    /// curve. Positive means "increase the target", negative means "decrease".
    pub fn signed_intensity_at(&self, tick: u64) -> f32 {
        if !self.active_at(tick) {
            return 0.0;
        }
        let dur = self.effective_duration();
        // progress at the LAST active tick is 1.0; a single-tick event is full Step-equivalent.
        let progress = if dur <= 1 {
            1.0
        } else {
            (tick - self.start_tick) as f32 / (dur - 1) as f32
        };
        let mag = self.intensity * self.curve.factor(progress);
        if self.signed_negative {
            -mag
        } else {
            mag
        }
    }

    /// Whether this intervention affects grid cell `(x, y)`.
    pub fn affects(&self, x: u32, y: u32) -> bool {
        self.region.contains(x, y)
    }
}

/// A set of scheduled interventions. Yields the ones active at a given tick in stable id order.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InterventionQueue {
    pub commands: Vec<InterventionCommand>,
}

impl InterventionQueue {
    pub fn new(mut commands: Vec<InterventionCommand>) -> Self {
        // Stable, deterministic application order regardless of how the scenario listed them.
        commands.sort_by_key(|c| (c.start_tick, c.id));
        Self { commands }
    }

    /// The interventions active at `tick`, in deterministic order.
    pub fn active_at(&self, tick: u64) -> impl Iterator<Item = &InterventionCommand> {
        self.commands.iter().filter(move |c| c.active_at(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd() -> InterventionCommand {
        InterventionCommand {
            id: 1,
            cause_id: 5,
            kind: InterventionKind::RainfallDelta,
            region: Region::Rect {
                min_x: 2,
                min_y: 2,
                max_x: 4,
                max_y: 4,
            },
            start_tick: 100,
            duration_ticks: 50,
            intensity: 0.3,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        }
    }

    #[test]
    fn s11_applies_only_within_window_and_region() {
        let c = cmd();
        // Temporal window [100, 150).
        assert!(!c.active_at(99));
        assert!(c.active_at(100));
        assert!(c.active_at(149));
        assert!(!c.active_at(150));
        assert_eq!(c.signed_intensity_at(99), 0.0);
        assert_eq!(c.signed_intensity_at(125), -0.3); // Step → full magnitude, negative sign

        // Spatial region: inside the rect vs outside.
        assert!(c.affects(3, 3));
        assert!(!c.affects(0, 0));
        assert!(!c.affects(5, 3));
    }

    #[test]
    fn s11_ramp_curves_shape_intensity() {
        let mut c = cmd();
        c.signed_negative = false;
        c.curve = Curve::RampUp;
        assert!((c.signed_intensity_at(100) - 0.0).abs() < 1e-6); // progress 0
        assert!((c.signed_intensity_at(149) - 0.3).abs() < 1e-6); // progress 1
        c.curve = Curve::Triangle;
        // Peak at the middle tick (progress 0.5) → full intensity.
        let mid = 100 + (c.effective_duration() - 1) / 2;
        assert!(c.signed_intensity_at(mid) > 0.29);
    }

    #[test]
    fn s11_queue_yields_active_in_order() {
        let mut a = cmd();
        a.id = 2;
        a.start_tick = 10;
        a.duration_ticks = 5;
        let mut b = cmd();
        b.id = 1;
        b.start_tick = 10;
        b.duration_ticks = 5;
        // Constructed out of order; queue sorts by (start_tick, id).
        let q = InterventionQueue::new(vec![a, b]);
        let active: Vec<u32> = q.active_at(12).map(|c| c.id).collect();
        assert_eq!(active, vec![1, 2]);
        assert!(q.active_at(20).next().is_none());
    }

    #[test]
    fn instantaneous_event_is_one_tick() {
        let mut c = cmd();
        c.duration_ticks = 0;
        assert_eq!(c.effective_duration(), 1);
        assert!(c.active_at(100));
        assert!(!c.active_at(101));
        assert_eq!(c.signed_intensity_at(100), -0.3);
    }
}
