//! The experiment manifest (AE-103), its canonical encoding and fingerprint, and the AE-210
//! reference manifests the committed fixtures must match.

use super::*;

// ---- AE-210 reference fixtures ----------------------------------------------------------------

/// The three AE-210 reference manifests, built by the same constructors the engine uses.
///
/// These *define* what `tests/fixtures/experiments/*.json` must contain. Two consumers depend on
/// that being one definition rather than two:
///
/// - `ae210_fixtures_match_the_serializer` asserts the committed bytes still equal what this
///   produces, so a schema change cannot land with stale fixtures beside it;
/// - `examples/gen_ae_fixtures.rs` writes them, which is how you regenerate after an *intentional*
///   change.
///
/// This used to be the body of a `#[ignore]`d test — a writer that only ran when someone remembered
/// to type `--ignored`, guarding nothing in between. A test that writes into the repository is not a
/// regression test; the regression test is the comparison, and the writer is a tool.
///
/// The trio is deliberate: a baseline, a treatment differing only in the exotic regime (so the pair
/// is a clean AE-S08 control/treatment), and one that must be **rejected** by validation.
///
/// This is the **only** item the fixture work adds to the public API. The directory layout and the
/// trailing-newline convention deliberately stay in the two consumers: neither is a property of an
/// experiment manifest, and exporting them would widen the surface for no one's benefit.
pub fn ae210_reference_manifests() -> Vec<(&'static str, ExperimentManifest)> {
    use crate::core::exotic_energy::{ExoticEnergyLaw, ExoticInterventionKind};

    let base = ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "exp-1".into(),
        name: "baseline".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: 1337,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xDEAD_BEEF,
            sim_world_sha256: None,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ]),
        interventions: vec![],
        seeds: vec![1, 2, 3],
        duration_ticks: 6000,
        sample_period: 600,
        observable_ids: vec!["plants".into(), "herbivores".into()],
        exotic_interventions: Vec::new(),
    };

    let mut mana = base.clone();
    mana.experiment_id = "ae-mana-patchy".into();
    mana.name = "mana-patchy-renewable".into();
    mana.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
    // The observable list is deliberately IDENTICAL to the baseline's, so the two fixtures differ
    // only in the exotic regime (law + forcings). The treatment still *emits* the `exotic.*`
    // observables — emission is model-driven and `RunResult` attaches metadata for everything
    // emitted — so nothing is lost by not requesting them here.
    mana.exotic_interventions = vec![ExoticIntervention {
        id: 1,
        cause_id: 901,
        kind: ExoticInterventionKind::RemoveSource,
        region: Region::Global,
        start_tick: 3000,
        duration_ticks: 1200,
        amount: 0.25,
        curve: Curve::Step,
    }];

    // Invalid on purpose: a negative source rate must be rejected by the law validator.
    let mut invalid = mana.clone();
    invalid.experiment_id = "ae-invalid-negative-source".into();
    invalid.name = "invalid-negative-source".into();
    invalid.exotic_interventions.clear();
    if let Some(law) = invalid.laws.exotic_energy.as_mut() {
        law.source_rate = -0.05;
    }

    vec![
        ("baseline-no-exotic.json", base),
        ("mana-patchy-renewable.json", mana),
        ("invalid-negative-source.json", invalid),
    ]
}

// ---- Experiment manifest (AE-103) ------------------------------------------------------------

/// The full reproducible input for an experiment run: identity, laws, initial state, interventions,
/// seeds, duration, sampling and requested observables. Its [`fingerprint`](Self::fingerprint) is the
/// run's canonical identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentManifest {
    pub schema_version: u16,
    pub experiment_id: String,
    pub name: String,
    /// The world this experiment runs on (artifact identity: seed/generator/dims/checksum).
    pub world_identity: WorldIdentity,
    pub laws: WorldLawSet,
    pub initial_conditions: InitialConditionSet,
    pub interventions: Vec<InterventionCommand>,
    pub seeds: Vec<u64>,
    pub duration_ticks: u64,
    /// Sample the observables every `sample_period` base ticks (`0` = never).
    pub sample_period: u64,
    pub observable_ids: Vec<String>,
    /// Runtime exotic-source forcings (AE-209). These are declared **state effects on the field**,
    /// never law edits: `laws.exotic_energy` stays immutable for the whole run (ER01). A non-empty
    /// list requires `laws.exotic_energy` to be `Some(..)` — there is no field to force otherwise.
    /// Defaults to empty so existing manifests (and JSON without the key) keep working.
    #[serde(default)]
    pub exotic_interventions: Vec<ExoticIntervention>,
    /// How an observer may relate to this run (ADR-0004). A **declared input**, so it changes the
    /// manifest fingerprint — but never the world-law fingerprint, which stays immutable for the run
    /// (ER01). That separation is what lets a checkpoint be forked to drop the observer.
    ///
    /// Defaults to [`ObserverPolicy::Absent`] so manifests written before ADR-0004 — and any JSON
    /// without the key — load and behave exactly as they did. `MANIFEST_SCHEMA_VERSION` is
    /// deliberately **not** bumped for the same reason: [`validate`](Self::validate) rejects any
    /// version it does not equal, so a bump would turn an additive field into a breaking change.
    #[serde(default)]
    pub observer: ObserverPolicy,
}

impl ExperimentManifest {
    /// Validate the manifest against a registry: schema, non-empty ids, laws, initial state, seeds
    /// (non-empty, bounded, unique), duration bounds, and that every requested observable exists.
    pub fn validate(&self, registry: &ObservableRegistry) -> Result<(), ExperimentError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "manifest".into(),
                found: self.schema_version,
                supported: MANIFEST_SCHEMA_VERSION,
            });
        }
        if self.experiment_id.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "experiment_id".into(),
            });
        }
        if self.name.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "name".into(),
            });
        }
        self.laws.validate()?;
        self.initial_conditions.validate()?;

        // AE3 reference population (opt-in). Absent keys mean "disabled", which is the legacy path;
        // present-but-impossible keys are a structural failure, never a silently-corrected default.
        // The pathway is tuned to whatever source THIS run declares, so a fixture can never name an
        // incompatible one.
        let population =
            crate::core::evolution_pathway::ReferencePopulationConfig::from_initial_conditions(
                &self.initial_conditions,
                self.laws.exotic_energy.as_ref().map(|l| &l.id),
            )
            .map_err(|reason| ExperimentError::InvalidPopulation { reason })?;
        // An AE3 observable without a population would report a fabricated zero. Fail preflight.
        if population.is_none() {
            if let Some(id) = self.observable_ids.iter().find(|id| {
                crate::core::evolution_pathway::AE3_OBSERVABLE_IDS.contains(&id.as_str())
            }) {
                return Err(ExperimentError::InvalidPopulation {
                    reason: format!(
                        "observable '{id}' requires an enabled AE3 reference population; declare \
                         '{}' in initial_conditions or drop the observable",
                        crate::core::evolution_pathway::AE3_KEY_POPULATION_TOTAL
                    ),
                });
            }
        }

        // Seeds: non-empty, bounded, unique.
        if self.seeds.is_empty() {
            return Err(ExperimentError::EmptySeeds);
        }
        if self.seeds.len() > MAX_SEEDS {
            return Err(ExperimentError::ResourceLimit {
                field: "seeds".into(),
                limit: MAX_SEEDS,
                found: self.seeds.len(),
            });
        }
        for (i, s) in self.seeds.iter().enumerate() {
            for s2 in &self.seeds[i + 1..] {
                if s == s2 {
                    return Err(ExperimentError::DuplicateSeed { seed: *s });
                }
            }
        }

        // Duration.
        if self.duration_ticks == 0 {
            return Err(ExperimentError::OutOfRange {
                field: "duration_ticks".into(),
                value: 0.0,
                min: 1.0,
                max: MAX_DURATION_TICKS as f64,
            });
        }
        if self.duration_ticks > MAX_DURATION_TICKS {
            return Err(ExperimentError::ResourceLimit {
                field: "duration_ticks".into(),
                limit: MAX_DURATION_TICKS as usize,
                found: self.duration_ticks as usize,
            });
        }

        // Interventions: bounded, unique ids.
        if self.interventions.len() > MAX_INTERVENTIONS {
            return Err(ExperimentError::ResourceLimit {
                field: "interventions".into(),
                limit: MAX_INTERVENTIONS,
                found: self.interventions.len(),
            });
        }
        for (i, cmd) in self.interventions.iter().enumerate() {
            // Values, geometry and schedule must be sane on the manifest path (AE-103).
            validate_intervention(cmd, self.duration_ticks)?;
            for cmd2 in &self.interventions[i + 1..] {
                if cmd.id == cmd2.id {
                    return Err(ExperimentError::DuplicateId {
                        context: "intervention".into(),
                        id: cmd.id.to_string(),
                    });
                }
            }
        }

        // Exotic forcings (AE-209): bounded, individually valid, unique ids, and only where there is
        // actually a field to force. Silently ignoring a forcing on a baseline world would misstate
        // the declared experimental input.
        if self.exotic_interventions.len() > MAX_INTERVENTIONS {
            return Err(ExperimentError::ResourceLimit {
                field: "exotic_interventions".into(),
                limit: MAX_INTERVENTIONS,
                found: self.exotic_interventions.len(),
            });
        }
        if !self.exotic_interventions.is_empty() && self.laws.exotic_energy.is_none() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: self.exotic_interventions[0].id,
                reason:
                    "manifest declares exotic forcings but laws.exotic_energy is None (there is \
                         no field to force)"
                        .into(),
            });
        }
        for (i, cmd) in self.exotic_interventions.iter().enumerate() {
            cmd.validate(self.duration_ticks).map_err(|reason| {
                ExperimentError::InvalidExoticIntervention { id: cmd.id, reason }
            })?;
            for other in &self.exotic_interventions[i + 1..] {
                if other.id == cmd.id {
                    return Err(ExperimentError::DuplicateId {
                        context: "exotic_intervention".into(),
                        id: cmd.id.to_string(),
                    });
                }
            }
        }

        // Observer policy (ADR-0004): well-formed, or refuse the manifest. An `Inhabit` rooted at
        // the background cause is the failure worth catching here — it would run happily and file
        // the observer's own effects as baseline dynamics, which is a lie the causal ledger would
        // then repeat to everyone downstream.
        if let Some(reason) = self.observer.rejection_reason() {
            return Err(ExperimentError::InvalidObserverPolicy { reason });
        }

        // `CAUSE_OBSERVER` means "a human did this, live". An intervention **authored into a
        // manifest** cannot be that by definition — the manifest was written before the run — so a
        // declared one claiming that id would share a root with the observer's own effects and make
        // `root_cause` answer a question nobody asked. Cause ids are hand-assigned with no allocator
        // anywhere, so this is checked rather than assumed.
        //
        // This does **not** contradict ADR-0004 C3, which lowers a live observer action to an
        // `InterventionCommand` carrying exactly this id. That one is built at runtime and never
        // passes through here; the rule is about declared input, not about the id itself.
        for cause_id in self
            .interventions
            .iter()
            .map(|cmd| cmd.cause_id)
            .chain(self.exotic_interventions.iter().map(|f| f.cause_id))
        {
            if cause_id == anima_domain::causal::CAUSE_OBSERVER {
                return Err(ExperimentError::InvalidObserverPolicy {
                    reason: format!(
                        "a manifest-declared intervention claims cause id {cause_id}, reserved for \
                         the live observer (ADR-0004); declared input cannot have been caused by a \
                         human acting during the run — pick an id of your own"
                    ),
                });
            }
        }

        // Observables: bounded, unique, all present in the registry.
        if self.observable_ids.len() > MAX_OBSERVABLES {
            return Err(ExperimentError::ResourceLimit {
                field: "observables".into(),
                limit: MAX_OBSERVABLES,
                found: self.observable_ids.len(),
            });
        }
        for (i, id) in self.observable_ids.iter().enumerate() {
            if !registry.contains(id) {
                return Err(ExperimentError::UnknownObservable { id: id.clone() });
            }
            for id2 in &self.observable_ids[i + 1..] {
                if id == id2 {
                    return Err(ExperimentError::DuplicateId {
                        context: "observable".into(),
                        id: id.clone(),
                    });
                }
            }
        }
        // G2 gate #3: the dimensions are individually legal — now check their PRODUCT against the
        // declared RAM ceiling. Nothing else did, and `RunResult::series` keeps every sample while
        // `run_ensemble` keeps every `RunResult`, so a manifest at the documented maxima asks for
        // petabytes and dies without ever pointing at the manifest.
        let estimated_bytes = self.estimated_result_bytes(registry);
        if estimated_bytes > MAX_ENSEMBLE_RESULT_BYTES {
            return Err(ExperimentError::EnsembleTooLarge {
                estimated_bytes,
                limit_bytes: MAX_ENSEMBLE_RESULT_BYTES,
                seeds: self.seeds.len(),
                samples_per_run: self.samples_per_run(),
                observables: self.observable_ids.len().max(registry.specs.len()),
            });
        }
        Ok(())
    }

    /// How many samples one run records. `sample_period == 0` means "never sample", so the series
    /// stays empty and only the final observables are kept.
    pub fn samples_per_run(&self) -> u64 {
        self.duration_ticks
            .checked_div(self.sample_period)
            .unwrap_or(0)
    }

    /// Upper bound on the bytes an ensemble of this manifest will hold in memory.
    ///
    /// Includes both sampled series and the append-only causal ledger. Every product saturates so
    /// an absurd manifest reports `u64::MAX` rather than wrapping to a small number and sailing
    /// through the very check it should fail. The observable count is the larger of what the
    /// manifest asks for and what the registry can emit, because a run records whatever it emits.
    pub fn estimated_result_bytes(&self, registry: &ObservableRegistry) -> u64 {
        let observables = self.observable_ids.len().max(registry.specs.len()) as u64;
        let series_per_run = self
            .samples_per_run()
            .saturating_mul(observables)
            .saturating_mul(BYTES_PER_SAMPLED_OBSERVABLE);
        let ledger_per_run = self
            .estimated_ledger_records_per_run()
            .saturating_mul(BYTES_PER_CAUSAL_EFFECT);
        (self.seeds.len() as u64).saturating_mul(series_per_run.saturating_add(ledger_per_run))
    }

    /// Upper bound on causal records retained by one built-in-model run.
    ///
    /// Ordinary and exotic intervention windows are charged only for ecology-band firings they can
    /// actually cover. The exotic world law and AE3 population are persistent causal producers, so
    /// they are charged for every ecology firing. This intentionally over-counts overlapping
    /// ordinary chains; an up-front RAM ceiling must not under-count the hostile case.
    pub fn estimated_ledger_records_per_run(&self) -> u64 {
        use crate::core::evolution_pathway::AE3_KEY_POPULATION_TOTAL;
        use crate::core::sim_clock::ECOLOGY_PERIOD;

        let ordinary = self.interventions.iter().fold(0_u64, |total, command| {
            total.saturating_add(
                ecology_firings_in_window(
                    command.start_tick,
                    command.effective_duration(),
                    self.duration_ticks,
                    ECOLOGY_PERIOD,
                )
                .saturating_mul(MAX_EFFECTS_PER_INTERVENTION_FIRING),
            )
        });

        let persistent_firings = self.duration_ticks / ECOLOGY_PERIOD;
        let exotic_law = if self.laws.exotic_energy.is_some() {
            persistent_firings
        } else {
            0
        };
        let exotic_forcings = self
            .exotic_interventions
            .iter()
            .fold(0_u64, |total, command| {
                total.saturating_add(ecology_firings_in_window(
                    command.start_tick,
                    command.effective_duration(),
                    self.duration_ticks,
                    ECOLOGY_PERIOD,
                ))
            });
        let ae3 = if self
            .initial_conditions
            .get(AE3_KEY_POPULATION_TOTAL)
            .is_some()
        {
            persistent_firings.saturating_mul(MAX_AE3_EFFECTS_PER_ECOLOGY_FIRING)
        } else {
            0
        };

        ordinary
            .saturating_add(exotic_law)
            .saturating_add(exotic_forcings)
            .saturating_add(ae3)
    }

    /// The control variant of this manifest for a genesis fork: identical in every shared input, with
    /// the exotic-energy law removed (`None`). The only declared factor is `laws.exotic_energy`
    /// (AE-S08). The name/experiment_id gain a `::control` suffix (labels, not factors).
    pub fn control_variant(&self) -> ExperimentManifest {
        let mut laws = self.laws.clone();
        laws.exotic_energy = None;
        ExperimentManifest {
            experiment_id: format!("{}::control", self.experiment_id),
            name: format!("{}::control", self.name),
            laws,
            // Removing the law removes the field, so any runtime forcing on it must go too —
            // otherwise the control would declare forcings with nothing to force (and fail
            // validation). The exotic regime (law + its forcings) is one declared factor.
            exotic_interventions: Vec::new(),
            ..self.clone()
        }
    }

    /// The run's canonical identity. Two logically-identical manifests with reordered non-semantic
    /// input (seed order, observable order, intervention order) hash the same (AE-S02); any material
    /// law change flips it (AE-S03). `experiment_id`/`name` are labels and excluded.
    pub fn fingerprint(&self) -> u64 {
        let mut c = Canon::new();
        c.tag(0xF0).u16(self.schema_version);

        // World identity.
        c.tag(0xF1)
            .u32(self.world_identity.seed)
            .u32(self.world_identity.generator_version)
            .u32(self.world_identity.width)
            .u32(self.world_identity.height)
            .u32(self.world_identity.checksum);
        if let Some(sha) = self.world_identity.sim_world_sha256 {
            c.tag(0xF8);
            for chunk in sha.as_chunks::<8>().0 {
                c.u64(u64::from_le_bytes(*chunk));
            }
        }

        // Laws + initial conditions (each self-canonicalizing).
        self.laws.canonicalize(&mut c);
        self.initial_conditions.canonicalize(&mut c);

        // Interventions as a set, sorted by (start_tick, id).
        let mut ivs: Vec<&InterventionCommand> = self.interventions.iter().collect();
        ivs.sort_by_key(|cmd| (cmd.start_tick, cmd.id));
        c.tag(0xF2).u64(ivs.len() as u64);
        for cmd in ivs {
            canonicalize_intervention(&mut c, cmd);
        }

        // Seeds as a set (sorted).
        let mut seeds = self.seeds.clone();
        seeds.sort_unstable();
        c.tag(0xF3).u64(seeds.len() as u64);
        for s in seeds {
            c.u64(s);
        }

        c.tag(0xF4).u64(self.duration_ticks).u64(self.sample_period);

        // Exotic forcings as a set, sorted by (start_tick, id) so listing order is non-semantic.
        // Runtime forcings are part of the DECLARED INPUT, so they change the manifest fingerprint —
        // but never the world-law fingerprint, which stays immutable for the run (ER01).
        let mut forcings: Vec<&ExoticIntervention> = self.exotic_interventions.iter().collect();
        forcings.sort_by_key(|f| (f.start_tick, f.id));
        c.tag(0xF6).u64(forcings.len() as u64);
        for f in forcings {
            c.u32(f.id).u32(f.cause_id);
            c.tag(exotic_kind_tag(f.kind));
            canonicalize_region(&mut c, &f.region);
            c.u64(f.start_tick).u64(f.duration_ticks);
            c.f32(f.amount).tag(curve_tag(f.curve));
        }

        // Observable ids as a set (sorted).
        let mut obs = self.observable_ids.clone();
        obs.sort();
        c.tag(0xF5).u64(obs.len() as u64);
        for id in obs {
            c.str(&id);
        }

        // Observer policy (ADR-0004). All three must give different identities, including `Absent`
        // vs `Spectate` — those two are required to produce the *same trajectory*, but they are
        // different declarations about how the run was watched, and a run's identity records what
        // was declared. `Inhabit` is a different treatment outright.
        c.tag(0xF7);
        match self.observer {
            ObserverPolicy::Absent => {
                c.tag(0x60);
            }
            ObserverPolicy::Spectate => {
                c.tag(0x61);
            }
            ObserverPolicy::Inhabit { cause_id } => {
                c.tag(0x62).u32(cause_id);
            }
        }
        c.hash()
    }
}

/// Count multiples of `period` in the intersection of the run's `1..=run_ticks` clock and an
/// intervention's half-open `[start_tick, start_tick + duration)` window.
pub(super) fn ecology_firings_in_window(
    start_tick: u64,
    duration: u64,
    run_ticks: u64,
    period: u64,
) -> u64 {
    if period == 0 || duration == 0 || run_ticks == 0 {
        return 0;
    }
    let lo = start_tick.max(1);
    let end_exclusive = start_tick
        .saturating_add(duration)
        .min(run_ticks.saturating_add(1));
    if lo >= end_exclusive {
        return 0;
    }
    let first = lo.div_ceil(period).saturating_mul(period);
    if first >= end_exclusive {
        0
    } else {
        (end_exclusive - 1 - first) / period + 1
    }
}

pub(super) fn canonicalize_intervention(c: &mut Canon, cmd: &InterventionCommand) {
    c.u32(cmd.id).u32(cmd.cause_id);
    canonicalize_intervention_kind(c, &cmd.kind);
    canonicalize_region(c, &cmd.region);
    c.u64(cmd.start_tick).u64(cmd.duration_ticks);
    c.f32(cmd.intensity).bool(cmd.signed_negative);
    c.tag(curve_tag(cmd.curve)).bool(cmd.reversible);
}

pub(super) fn canonicalize_region(c: &mut Canon, region: &Region) {
    match *region {
        Region::Global => {
            c.tag(0x40);
        }
        Region::Cell { x, y } => {
            c.tag(0x41).u32(x).u32(y);
        }
        Region::Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        } => {
            c.tag(0x42).u32(min_x).u32(min_y).u32(max_x).u32(max_y);
        }
        Region::Radius { cx, cy, r } => {
            c.tag(0x43).f32(cx).f32(cy).f32(r);
        }
    }
}

pub(super) fn canonicalize_intervention_kind(
    c: &mut Canon,
    k: &crate::core::intervention::InterventionKind,
) {
    use crate::core::intervention::{InterventionKind::*, TraitMod};
    match k {
        RainfallDelta => {
            c.tag(0x50);
        }
        TemperatureDelta => {
            c.tag(0x51);
        }
        Deforest => {
            c.tag(0x52);
        }
        RemovePredators => {
            c.tag(0x53);
        }
        AddNutrient => {
            c.tag(0x54);
        }
        GeneticInjection {
            target_species,
            trait_modifications,
            count,
        } => {
            c.tag(0x55);
            match target_species {
                Some(sp) => {
                    c.tag(0x01).u32(*sp);
                }
                None => {
                    c.tag(0x00);
                }
            }
            c.u64(*count as u64).u64(trait_modifications.len() as u64);
            for tm in trait_modifications {
                match tm {
                    TraitMod::ScaleMorphology(f) => {
                        c.tag(0x01).f32(*f);
                    }
                    TraitMod::MassMultiplier(f) => {
                        c.tag(0x02).f32(*f);
                    }
                    TraitMod::MetabolicEfficiencyDelta(f) => {
                        c.tag(0x03).f32(*f);
                    }
                    TraitMod::SensoryRangeMultiplier(f) => {
                        c.tag(0x04).f32(*f);
                    }
                    TraitMod::BrainParameterShift {
                        layer_index,
                        bias_delta,
                    } => {
                        c.tag(0x05).u32(*layer_index).f32(*bias_delta);
                    }
                }
            }
        }
        ToxicityPlume {
            center_x,
            center_y,
            radius,
            toxicity_level,
            decay_rate,
        } => {
            c.tag(0x56)
                .f32(*center_x)
                .f32(*center_y)
                .f32(*radius)
                .f32(*toxicity_level)
                .f32(*decay_rate);
        }
        PathogenTransmission {
            pathogen_id,
            transmission_rate,
            mortality_multiplier,
        } => {
            c.tag(0x57)
                .u32(*pathogen_id)
                .f32(*transmission_rate)
                .f32(*mortality_multiplier);
        }
    }
}

pub(super) fn exotic_kind_tag(k: crate::core::exotic_energy::ExoticInterventionKind) -> u8 {
    use crate::core::exotic_energy::ExoticInterventionKind::*;
    match k {
        AddSource => 0x70,
        RemoveSource => 0x71,
        Pulse => 0x72,
    }
}

pub(super) fn curve_tag(c: Curve) -> u8 {
    match c {
        Curve::Step => 0x60,
        Curve::RampUp => 0x61,
        Curve::RampDown => 0x62,
        Curve::Triangle => 0x63,
    }
}
