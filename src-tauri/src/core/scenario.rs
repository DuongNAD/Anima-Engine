//! # Scenario runner + control/treatment harness (M2.4, M2.5).
//!
//! A [`Scenario`] is `seed + duration + a list of interventions`. [`run_scenario`] drives a
//! deterministic [`SimModel`] under the multi-rate [`SimClock`], applying the active interventions
//! each tick and recording the causal chain into a [`CausalLedger`], and returns a **state
//! checksum**. Running the same scenario twice yields the same checksum (scenario S13) — the proof
//! the engine has become a reproducible cause→effect world rather than a bag of subsystems.
//!
//! [`control_treatment`] runs the scenario with its interventions stripped (control) and with them
//! (treatment) under the *same seed*, then reports the per-observable delta (S14) — "what did this
//! intervention actually change, holding everything else fixed".
//!
//! The bundled [`ReferenceEcosystem`] is a small but real trophic cascade (precip → NPP → plants →
//! herbivores → predators, each relaxing toward a capacity set by the level above it, with
//! increasing lag downward). It is the deterministic vertical-slice substrate the M2 machinery is
//! demonstrated on; a heavier model (or the Bevy sim, once it is made deterministic) can implement
//! [`SimModel`] to become replayable in exactly the same way.

use crate::core::causal::{CausalLedger, CauseId, CAUSE_BACKGROUND};
use crate::core::intervention::{InterventionCommand, InterventionKind, InterventionQueue};
use crate::core::sim_clock::{SimClock, ECOLOGY_PERIOD, PLANT_PERIOD};
use crate::core::world_artifact::fnv1a_32;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// JSON representation for named scalar observations.
///
/// JSON has no native NaN or infinity values. Serde JSON otherwise emits those values as `null`,
/// which cannot deserialize back into `f64` and erases the value that explains a failed run.
/// Finite values remain ordinary JSON numbers; the three non-finite classes use explicit tags.
pub(crate) mod json_f64_pairs {
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum JsonFloat {
        Number(f64),
        Tag(String),
    }

    pub fn serialize<S>(values: &[(String, f64)], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let safe: Vec<(&str, JsonFloat)> = values
            .iter()
            .map(|(name, value)| {
                let encoded = if value.is_nan() {
                    JsonFloat::Tag("NaN".into())
                } else if *value == f64::INFINITY {
                    JsonFloat::Tag("+Infinity".into())
                } else if *value == f64::NEG_INFINITY {
                    JsonFloat::Tag("-Infinity".into())
                } else {
                    JsonFloat::Number(*value)
                };
                (name.as_str(), encoded)
            })
            .collect();
        safe.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<(String, f64)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<(String, JsonFloat)>::deserialize(deserializer)?
            .into_iter()
            .map(|(name, value)| {
                let decoded = match value {
                    JsonFloat::Number(value) => value,
                    JsonFloat::Tag(tag) => match tag.as_str() {
                        "NaN" => f64::NAN,
                        "+Infinity" => f64::INFINITY,
                        "-Infinity" => f64::NEG_INFINITY,
                        _ => {
                            return Err(D::Error::custom(format!(
                                "unknown non-finite float tag '{tag}'"
                            )))
                        }
                    },
                };
                Ok((name, decoded))
            })
            .collect()
    }
}

/// A deterministic simulation the scenario runner can drive. Implementations must be reproducible:
/// given the same initial state, tick sequence, interventions and RNG stream, they produce the same
/// checksum every run.
pub trait SimModel: Default {
    /// Advance one base tick under the `active` interventions, recording caused changes into
    /// `ledger`. Implementations decide which rate band(s) they update on via `clock`.
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    );

    /// A deterministic content fingerprint of the current state.
    fn checksum(&self) -> u32;

    /// Named scalar observables in a STABLE order, for control/treatment deltas and time series.
    fn observables(&self) -> Vec<(&'static str, f64)>;
}

// ---- Reference ecosystem ---------------------------------------------------------------------

/// A small deterministic trophic cascade used to demonstrate the M2 machinery. Each level relaxes
/// toward a carrying capacity set by the level above it; relaxation rates decrease down the chain so
/// predators respond more slowly than herbivores (the plan's "predator giảm trễ hơn").
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReferenceEcosystem {
    pub precip: f64,
    /// Normalized air temperature in `[0, 1]` (matches M0's `field_temperature`), optimal at 0.5.
    pub temperature: f64,
    pub npp: f64,
    pub plants: f64,
    pub herbivores: f64,
    pub predators: f64,
    pub detritus: f64,
}

impl Default for ReferenceEcosystem {
    fn default() -> Self {
        // Start at the equilibrium of the undisturbed chain (precip = 1) so the control run is flat
        // and any drift in a treatment run is attributable to the intervention.
        Self {
            precip: 1.0,
            temperature: 0.5,
            npp: 1.0,
            plants: 100.0,
            herbivores: 40.0,
            predators: 8.0,
            detritus: 0.0,
        }
    }
}

impl ReferenceEcosystem {
    // Carrying-capacity couplings and relaxation rates (per ecology tick).
    const PLANT_CAP_PER_NPP: f64 = 100.0;
    const HERB_CAP_PER_PLANT: f64 = 0.4;
    const PRED_CAP_PER_HERB: f64 = 0.2;
    const NPP_RATE: f64 = 0.30;
    const PLANT_RATE: f64 = 0.20;
    const HERB_RATE: f64 = 0.10;
    const PRED_RATE: f64 = 0.05;
    /// Top-down control: each predator suppresses this much herbivore carrying capacity, so removing
    /// predators releases the prey (the plan's "loại bỏ predator → prey overshoot", §6).
    const PREDATION_PRESSURE: f64 = 0.5;
    /// Temperature stress: NPP falls by this fraction per unit deviation of temperature from optimal.
    const TEMP_OPTIMAL: f64 = 0.5;
    const TEMP_STRESS: f64 = 0.8;
    /// Detritus mineralized back toward the soil each PLANT-band tick (0.2 Hz decomposition, §2.5).
    const DECOMP_RATE: f64 = 0.1;
}

impl SimModel for ReferenceEcosystem {
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        let tick = clock.tick;

        // Slow decomposition runs on the PLANT band (0.2 Hz, plan §2.5): detritus mineralizes back
        // toward the soil. This is a genuine SECOND rate band driven by the same clock as the 1 Hz
        // ecology dynamics below — the multi-rate scheduler (S10) driving real work at two rates.
        if clock.band_fires(PLANT_PERIOD) {
            self.detritus -= Self::DECOMP_RATE * self.detritus;
            self.detritus = self.detritus.max(0.0);
        }

        // The trophic pools update on the ECOLOGY band (1 Hz); the faster physics band would move
        // agents but not these aggregates.
        if !clock.band_fires(ECOLOGY_PERIOD) {
            return;
        }

        // Read the active interventions affecting this (single-region) world's handle cell (0,0):
        // RainfallDelta forces precipitation; TemperatureDelta shifts air temperature; AddNutrient /
        // Deforest raise/lower plant carrying capacity (fertility vs canopy loss); RemovePredators
        // culls predators (which, via top-down control, releases the prey).
        // NOTE: if several interventions co-drive the SAME target their factors all combine, but only
        // the last in deterministic (start_tick, id) order is recorded as the cause — full multi-cause
        // attribution is future work.
        let mut precip_factor = 1.0_f64;
        let mut precip_cause: Option<CauseId> = None;
        let mut cap_factor = 1.0_f64;
        let mut cap_cause: Option<CauseId> = None;
        let mut temp_shift = 0.0_f64;
        let mut temp_cause: Option<CauseId> = None;
        let mut pred_cull = 0.0_f64;
        let mut cull_cause: Option<CauseId> = None;
        for cmd in active {
            if !cmd.affects(0, 0) {
                continue;
            }
            match cmd.kind {
                InterventionKind::RainfallDelta => {
                    precip_factor *= 1.0 + cmd.signed_intensity_at(tick) as f64;
                    precip_cause = Some(cmd.cause_id);
                }
                InterventionKind::TemperatureDelta => {
                    temp_shift += cmd.signed_intensity_at(tick) as f64;
                    temp_cause = Some(cmd.cause_id);
                }
                InterventionKind::AddNutrient => {
                    cap_factor += cmd.signed_intensity_at(tick).abs() as f64;
                    cap_cause = Some(cmd.cause_id);
                }
                InterventionKind::Deforest => {
                    cap_factor -= cmd.signed_intensity_at(tick).abs() as f64;
                    cap_cause = Some(cmd.cause_id);
                }
                InterventionKind::RemovePredators => {
                    pred_cull = (pred_cull + cmd.signed_intensity_at(tick).abs() as f64).min(1.0);
                    cull_cause = Some(cmd.cause_id);
                }
            }
        }

        let new_precip = precip_factor;
        let precip_delta = new_precip - self.precip;
        self.precip = new_precip;
        // Only record a caused chain while an intervention is actively driving precipitation; the
        // undisturbed baseline is not "explained" by any cause.
        let precip_effect = precip_cause.map(|cause| {
            ledger.record(
                cause,
                None,
                tick,
                "precip",
                self.precip,
                precip_delta,
                "rainfall intervention forcing precipitation",
            )
        });

        // 2) Air temperature shifts toward optimal ± the active TemperatureDelta forcing.
        let new_temp = (Self::TEMP_OPTIMAL + temp_shift).clamp(0.0, 1.0);
        let temp_delta = new_temp - self.temperature;
        self.temperature = new_temp;
        let temp_effect = temp_cause.map(|cause| {
            ledger.record(
                cause,
                None,
                tick,
                "temperature",
                self.temperature,
                temp_delta,
                "temperature intervention forcing air temperature",
            )
        });

        // The NPP chain is rooted at whichever abiotic driver is active (rainfall or temperature).
        let abiotic_effect = precip_effect.or(temp_effect);

        // 3) NPP relaxes toward precipitation, scaled by a temperature-stress factor (productivity
        //    falls as temperature departs from optimal), with tiny seeded process noise (so the seed
        //    matters for replay while control/treatment under one seed share the same noise).
        let noise = 1.0 + 0.02 * (rng.gen::<f64>() - 0.5);
        let temp_stress =
            (1.0 - Self::TEMP_STRESS * (self.temperature - Self::TEMP_OPTIMAL).abs()).max(0.0);
        let npp_target = self.precip * temp_stress * noise;
        let npp_before = self.npp;
        self.npp += Self::NPP_RATE * (npp_target - self.npp);
        self.npp = self.npp.max(0.0);
        let npp_effect = abiotic_effect.map(|p| {
            ledger.record(
                CAUSE_BACKGROUND,
                Some(p),
                tick,
                "npp",
                self.npp,
                self.npp - npp_before,
                "precipitation and temperature drive net primary productivity",
            )
        });

        // 3) Plants relax toward a capacity set by NPP and scaled by fertility/deforestation.
        let plant_cap = Self::PLANT_CAP_PER_NPP * self.npp * cap_factor.max(0.0);
        let plants_before = self.plants;
        self.plants += Self::PLANT_RATE * (plant_cap - self.plants);
        self.plants = self.plants.max(0.0);
        // Attribute the plant change to the rainfall chain if one is driving; otherwise, if a
        // fertility/deforestation intervention is driving capacity directly, root a new chain at it.
        let plants_effect = if let Some(np) = npp_effect {
            Some(ledger.record(
                CAUSE_BACKGROUND,
                Some(np),
                tick,
                "plants",
                self.plants,
                self.plants - plants_before,
                "NPP and fertility set plant carrying capacity",
            ))
        } else {
            cap_cause.map(|cause| {
                ledger.record(
                    cause,
                    None,
                    tick,
                    "plants",
                    self.plants,
                    self.plants - plants_before,
                    "fertility / deforestation changes plant carrying capacity",
                )
            })
        };

        // 4) Herbivores relax toward a capacity set by plant biomass MINUS top-down predation
        //    pressure — so culling predators (below) releases the prey.
        let herb_cap = (Self::HERB_CAP_PER_PLANT * self.plants
            - Self::PREDATION_PRESSURE * self.predators)
            .max(0.0);
        let herb_before = self.herbivores;
        self.herbivores += Self::HERB_RATE * (herb_cap - self.herbivores);
        self.herbivores = self.herbivores.max(0.0);
        let herb_effect = plants_effect.map(|p| {
            ledger.record(
                CAUSE_BACKGROUND,
                Some(p),
                tick,
                "herbivores",
                self.herbivores,
                self.herbivores - herb_before,
                "forage availability limits herbivores",
            )
        });

        // 5) Predators relax toward a capacity proportional to herbivores (slowest → lags most).
        let pred_cap = Self::PRED_CAP_PER_HERB * self.herbivores;
        let pred_before = self.predators;
        self.predators += Self::PRED_RATE * (pred_cap - self.predators);
        self.predators = self.predators.max(0.0);
        if let Some(p) = herb_effect {
            ledger.record(
                CAUSE_BACKGROUND,
                Some(p),
                tick,
                "predators",
                self.predators,
                self.predators - pred_before,
                "prey availability limits predators",
            );
        }

        // A RemovePredators intervention culls a fraction of the standing predators each active tick,
        // recorded as its own effect rooted at its own cause. Fewer predators → less predation
        // pressure → the prey overshoots on following ticks (the plan's trophic cascade, §6).
        if pred_cull > 0.0 {
            let before_cull = self.predators;
            self.predators *= (1.0 - pred_cull).max(0.0);
            if let Some(cause) = cull_cause {
                ledger.record(
                    cause,
                    None,
                    tick,
                    "predators",
                    self.predators,
                    self.predators - before_cull,
                    "predator removal intervention",
                );
            }
        }

        // Detritus accumulates the biomass shed as any level declines (a simple recycling sink).
        let shed =
            (herb_before - self.herbivores).max(0.0) + (pred_before - self.predators).max(0.0);
        self.detritus += shed;
    }

    fn checksum(&self) -> u32 {
        // Hash the raw IEEE-754 bits (not a milli-quantization) so the fingerprint keeps FULL f64
        // precision: every seed's trajectory divergence — however small — changes the checksum, and
        // NaN / out-of-range values stay distinct instead of collapsing to a collision. The math is
        // pure deterministic f64, so the same seed still yields identical bits → identical checksum.
        // FNV-1a is reused so the whole engine speaks one hash.
        let fields = [
            self.precip,
            self.temperature,
            self.npp,
            self.plants,
            self.herbivores,
            self.predators,
            self.detritus,
        ];
        let mut bytes = Vec::with_capacity(fields.len() * 8);
        for f in fields {
            bytes.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        fnv1a_32(&bytes)
    }

    fn observables(&self) -> Vec<(&'static str, f64)> {
        vec![
            ("precip", self.precip),
            ("temperature", self.temperature),
            ("npp", self.npp),
            ("plants", self.plants),
            ("herbivores", self.herbivores),
            ("predators", self.predators),
            ("detritus", self.detritus),
        ]
    }
}

// ---- Scenario, runner, results ---------------------------------------------------------------

/// A reproducible experiment: a seed, a run length, an optional series-sampling cadence, and the
/// interventions to apply.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    pub duration_ticks: u64,
    /// Sample the observables every `sample_period` base ticks into the result series (`0` = never).
    pub sample_period: u64,
    pub interventions: Vec<InterventionCommand>,
}

impl Scenario {
    /// The same scenario with every intervention removed — the control condition for S14.
    pub fn control_variant(&self) -> Scenario {
        Scenario {
            name: format!("{}::control", self.name),
            interventions: Vec::new(),
            ..self.clone()
        }
    }
}

/// One sampled point in a run's time series.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StateSample {
    pub tick: u64,
    #[serde(with = "json_f64_pairs")]
    pub observables: Vec<(String, f64)>,
}

/// The outcome of running a scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub seed: u64,
    pub final_checksum: u32,
    #[serde(with = "json_f64_pairs")]
    pub final_observables: Vec<(String, f64)>,
    pub ledger: CausalLedger,
    pub series: Vec<StateSample>,
}

/// Run a scenario against model `M` and return its result (checksum + causal ledger + time series).
/// Deterministic: identical `Scenario` → identical `ScenarioResult` (S13).
pub fn run_scenario<M: SimModel>(scenario: &Scenario) -> ScenarioResult {
    let mut model = M::default();
    let mut rng = StdRng::seed_from_u64(scenario.seed);
    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    let queue = InterventionQueue::new(scenario.interventions.clone());
    let mut series = Vec::new();

    for _ in 0..scenario.duration_ticks {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        model.step(&clock, &active, &mut ledger, &mut rng);
        if scenario.sample_period != 0 && tick.is_multiple_of(scenario.sample_period) {
            series.push(StateSample {
                tick,
                observables: model
                    .observables()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
            });
        }
    }

    ScenarioResult {
        scenario_name: scenario.name.clone(),
        seed: scenario.seed,
        final_checksum: model.checksum(),
        final_observables: model
            .observables()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        ledger,
        series,
    }
}

/// Per-observable difference between a treatment run and its control.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TargetDelta {
    pub target: String,
    pub control_final: f64,
    pub treatment_final: f64,
    pub delta: f64,
}

/// The automatic delta report comparing a treatment run against its control (S14).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeltaReport {
    pub per_target: Vec<TargetDelta>,
}

impl DeltaReport {
    fn compute(control: &ScenarioResult, treatment: &ScenarioResult) -> Self {
        let per_target = treatment
            .final_observables
            .iter()
            .map(|(target, treat_v)| {
                let ctrl_v = control
                    .final_observables
                    .iter()
                    .find(|(t, _)| t == target)
                    .map(|(_, v)| *v)
                    .unwrap_or(0.0);
                TargetDelta {
                    target: target.clone(),
                    control_final: ctrl_v,
                    treatment_final: *treat_v,
                    delta: treat_v - ctrl_v,
                }
            })
            .collect();
        DeltaReport { per_target }
    }

    /// The delta for a named observable, if present.
    pub fn delta_of(&self, target: &str) -> Option<f64> {
        self.per_target
            .iter()
            .find(|d| d.target == target)
            .map(|d| d.delta)
    }
}

/// Run `scenario` as a control (interventions stripped) and a treatment (as given) under the SAME
/// seed, and return both results plus the automatic delta report (S14).
pub fn control_treatment<M: SimModel>(
    scenario: &Scenario,
) -> (ScenarioResult, ScenarioResult, DeltaReport) {
    let control = run_scenario::<M>(&scenario.control_variant());
    let treatment = run_scenario::<M>(scenario);
    let report = DeltaReport::compute(&control, &treatment);
    (control, treatment, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::intervention::{Curve, Region};

    #[test]
    fn observable_json_preserves_every_non_finite_class_without_null() {
        let sample = StateSample {
            tick: 7,
            observables: vec![
                ("finite".into(), 1.25),
                ("nan".into(), f64::NAN),
                ("positive".into(), f64::INFINITY),
                ("negative".into(), f64::NEG_INFINITY),
            ],
        };

        let json = serde_json::to_string(&sample).expect("sample serializes");
        assert!(!json.contains("null"), "{json}");
        assert!(json.contains("\"NaN\""), "{json}");
        assert!(json.contains("\"+Infinity\""), "{json}");
        assert!(json.contains("\"-Infinity\""), "{json}");

        let back: StateSample = serde_json::from_str(&json).expect("sample deserializes");
        assert_eq!(back.observables[0], ("finite".into(), 1.25));
        assert!(back.observables[1].1.is_nan());
        assert_eq!(back.observables[2].1, f64::INFINITY);
        assert_eq!(back.observables[3].1, f64::NEG_INFINITY);
    }

    fn drought_scenario(seed: u64) -> Scenario {
        // A sustained −30% rainfall drought that begins early and holds through the end of the run,
        // so the final state reflects the drought equilibrium (a reversible pulse that ended earlier
        // would show recovery instead). start=600, duration=6000 → active [600, 6600) covers tick 6000.
        let drought = InterventionCommand {
            id: 1,
            cause_id: 1,
            kind: InterventionKind::RainfallDelta,
            region: Region::Global,
            start_tick: 600,
            duration_ticks: 6000,
            intensity: 0.3,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        };
        Scenario {
            name: "drought".into(),
            seed,
            duration_ticks: 6000,
            sample_period: 600,
            interventions: vec![drought],
        }
    }

    #[test]
    fn s13_same_scenario_replays_identically() {
        let scn = drought_scenario(42);
        let a = run_scenario::<ReferenceEcosystem>(&scn);
        let b = run_scenario::<ReferenceEcosystem>(&scn);
        // Same scenario → byte-identical outcome (the M2 gate).
        assert_eq!(a.final_checksum, b.final_checksum);
        assert_eq!(a.ledger.len(), b.ledger.len());
        assert_eq!(a.series, b.series);
        assert_eq!(a.final_observables, b.final_observables);
    }

    #[test]
    fn s13_distinct_seeds_yield_distinct_runs() {
        // The run genuinely consumes the RNG stream: a spread of seeds all produce distinct
        // checksums. The raw-f64-bit fingerprint captures even sub-milli noise divergence, so this
        // is a guaranteed property — not a lucky single-pair (42 vs 43) that quantization could hide.
        use std::collections::HashSet;
        let checksums: HashSet<u32> = (100u64..116)
            .map(|s| run_scenario::<ReferenceEcosystem>(&drought_scenario(s)).final_checksum)
            .collect();
        assert_eq!(
            checksums.len(),
            16,
            "16 distinct seeds must give 16 distinct checksums"
        );
    }

    #[test]
    fn s14_no_intervention_yields_zero_delta() {
        let mut scn = drought_scenario(7);
        scn.interventions.clear();
        let (control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        // Control and treatment are the SAME run (same seed, no interventions) → identical.
        assert_eq!(control.final_checksum, treatment.final_checksum);
        for d in &report.per_target {
            assert!(d.delta.abs() < 1e-9, "{} delta should be ~0", d.target);
        }
    }

    #[test]
    fn s14_drought_cascades_down_the_food_web() {
        let scn = drought_scenario(7);
        let (control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);

        // The intervention actually changed the world.
        assert_ne!(control.final_checksum, treatment.final_checksum);

        // Rainfall ↓ propagates down every trophic level (each delta negative).
        for target in ["precip", "npp", "plants", "herbivores", "predators"] {
            let d = report.delta_of(target).expect("target present");
            assert!(d < 0.0, "{target} should fall under drought, delta was {d}");
        }

        // The delta is exactly treatment_final − control_final.
        let herb = report
            .per_target
            .iter()
            .find(|d| d.target == "herbivores")
            .unwrap();
        assert!((herb.delta - (herb.treatment_final - herb.control_final)).abs() < 1e-9);

        // The treatment ledger explains the change: every effect traces back to the drought cause.
        assert!(!treatment.ledger.is_empty());
        let last = (treatment.ledger.len() - 1) as u32;
        assert_eq!(treatment.ledger.root_cause(last), Some(1));
        // The control run has nothing to explain.
        assert!(control.ledger.is_empty());
    }

    #[test]
    fn scenario_serde_roundtrips() {
        let scn = drought_scenario(1);
        let json = serde_json::to_string(&scn).expect("serialize");
        let back: Scenario = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.duration_ticks, scn.duration_ticks);
        assert_eq!(back.interventions.len(), 1);
        // A scenario re-run from its deserialized form reproduces the same checksum.
        let a = run_scenario::<ReferenceEcosystem>(&scn);
        let b = run_scenario::<ReferenceEcosystem>(&back);
        assert_eq!(a.final_checksum, b.final_checksum);
    }

    fn base_cmd(kind: InterventionKind) -> InterventionCommand {
        InterventionCommand {
            id: 1,
            cause_id: 1,
            kind,
            region: Region::Global,
            start_tick: 600,
            duration_ticks: 6000,
            intensity: 0.5,
            signed_negative: false,
            curve: Curve::Step,
            reversible: true,
        }
    }

    fn one_intervention(seed: u64, cmd: InterventionCommand) -> Scenario {
        Scenario {
            name: "one".into(),
            seed,
            duration_ticks: 6000,
            sample_period: 600,
            interventions: vec![cmd],
        }
    }

    #[test]
    fn s14_off_target_intervention_is_a_noop_proving_lockstep() {
        // A rainfall intervention on a cell that is NOT this world's handle (0,0) must leave the run
        // byte-identical to the stripped control. This pins two things at once: (a) region scoping,
        // and (b) that interventions never change the RNG draw count — the ecology noise is drawn
        // once per firing tick regardless of interventions — so S14 deltas isolate only real effects.
        let mut cmd = base_cmd(InterventionKind::RainfallDelta);
        cmd.region = Region::Cell { x: 5, y: 5 };
        let scn = one_intervention(7, cmd);
        let (control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        assert_eq!(control.final_checksum, treatment.final_checksum);
        assert!(
            treatment.ledger.is_empty(),
            "an off-target intervention records nothing"
        );
        for d in &report.per_target {
            assert!(
                d.delta.abs() < 1e-12,
                "{} delta must be exactly 0",
                d.target
            );
        }
    }

    #[test]
    fn s14_add_nutrient_lifts_the_food_web() {
        // A positive intervention (fertility ↑) must cascade UP the trophic levels, and the model
        // stays finite/stable over the whole run (the "positive-intervention stability" question).
        let scn = one_intervention(7, base_cmd(InterventionKind::AddNutrient));
        let (_control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        for target in ["plants", "herbivores", "predators"] {
            let d = report.delta_of(target).expect("target present");
            assert!(
                d > 0.0,
                "{target} should rise with added nutrient, delta was {d}"
            );
        }
        // Fertility does not touch precipitation or NPP directly.
        assert!(report.delta_of("precip").unwrap().abs() < 1e-12);
        assert!(report.delta_of("npp").unwrap().abs() < 1e-12);
        // All treatment observables are finite (no blow-up / NaN).
        assert!(treatment
            .final_observables
            .iter()
            .all(|(_, v)| v.is_finite()));
        // The change is explained: the plant→herbivore→predator chain roots at the nutrient cause.
        let last = (treatment.ledger.len() - 1) as u32;
        assert_eq!(treatment.ledger.root_cause(last), Some(1));
    }

    #[test]
    fn s14_deforest_in_a_region_with_a_ramp_curve_cascades_down() {
        // Exercises a non-Global region (Rect covering the handle cell) AND a RampUp curve
        // end-to-end — coverage the drought scenario (Global + Step) never touches.
        let mut cmd = base_cmd(InterventionKind::Deforest);
        cmd.region = Region::Rect {
            min_x: 0,
            min_y: 0,
            max_x: 2,
            max_y: 2,
        };
        cmd.curve = Curve::RampUp;
        cmd.intensity = 0.4;
        let scn = one_intervention(3, cmd);
        let (_control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        for target in ["plants", "herbivores", "predators"] {
            let d = report.delta_of(target).expect("target present");
            assert!(
                d < 0.0,
                "{target} should fall with deforestation, delta was {d}"
            );
        }
        assert!(treatment
            .final_observables
            .iter()
            .all(|(_, v)| v.is_finite()));
    }

    #[test]
    fn s14_remove_predators_releases_the_prey() {
        // Culling predators must drop predators AND lift herbivores (top-down release / prey
        // overshoot, plan §6) — a MIXED-sign trophic cascade, not a one-directional decline.
        let mut cmd = base_cmd(InterventionKind::RemovePredators);
        cmd.intensity = 0.8;
        let scn = one_intervention(11, cmd);
        let (_control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        assert!(
            report.delta_of("predators").unwrap() < 0.0,
            "predators should fall"
        );
        assert!(
            report.delta_of("herbivores").unwrap() > 0.0,
            "prey should overshoot once predators are removed"
        );
        assert!(treatment
            .final_observables
            .iter()
            .all(|(_, v)| v.is_finite()));
        // The predator change is explained by the removal cause.
        let last = (treatment.ledger.len() - 1) as u32;
        assert_eq!(treatment.ledger.root_cause(last), Some(1));
    }

    #[test]
    fn s14_warming_stresses_productivity_down_the_chain() {
        // A warming intervention raises temperature and stresses NPP, cascading down every level —
        // exercising the 5th intervention kind (TemperatureDelta) end-to-end.
        let mut cmd = base_cmd(InterventionKind::TemperatureDelta);
        cmd.intensity = 0.3;
        cmd.signed_negative = false; // warming
        let scn = one_intervention(5, cmd);
        let (_control, treatment, report) = control_treatment::<ReferenceEcosystem>(&scn);
        assert!(
            report.delta_of("temperature").unwrap() > 0.0,
            "temperature should rise"
        );
        for target in ["npp", "plants", "herbivores", "predators"] {
            let d = report.delta_of(target).expect("target present");
            assert!(
                d < 0.0,
                "{target} should fall under heat stress, delta was {d}"
            );
        }
        assert!(treatment
            .final_observables
            .iter()
            .all(|(_, v)| v.is_finite()));
    }
}
