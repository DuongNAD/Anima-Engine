//! # Reference evolution world (AE1–AE3) — the headless vertical slice.
//!
//! [`ReferenceEvolutionWorld`] is the manifest-aware successor to
//! [`crate::core::scenario::ReferenceEcosystem`]. On the baseline path (`exotic_energy = None`) it is
//! that ecosystem *exactly* — same dynamics, same RNG draws, same checksum bits (AE-S01). When the
//! world laws declare a generic exotic-energy source it additionally carries a deterministic
//! [`ExoticEnergyField`] with a closed MU ledger, updated on a slow rate band and recorded into the
//! [`CausalLedger`] as a chain rooted at the world law (AE-207 / AE-S12).
//!
//! The exotic field stays **decoupled from the closed-EU dynamics**: it draws no ecology RNG and
//! never writes the trophic pools, so a control/treatment genesis fork keeps an *identical EU
//! trajectory* (AE-S05). AE3 optionally adds a two-cohort [`ReferencePopulation`] that can transact
//! MU and change inherited pathway frequency at reproduction. With no `ae3.` initial-condition key,
//! that population is absent and the AE1–AE2.5 path remains bit-identical.

use crate::core::causal::{CausalLedger, CauseId, EffectId, CAUSE_BACKGROUND};
use crate::core::evolution_pathway::{ReferencePopulation, ReferencePopulationConfig};
use crate::core::exotic_energy::{
    ExoticEnergyBudget, ExoticEnergyField, ExoticIntervention, ExoticInterventionKind,
    ExoticInterventionQueue,
};
use crate::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use crate::core::experiment_runner::ExperimentModel;
use crate::core::intervention::InterventionCommand;
use crate::core::scenario::{ReferenceEcosystem, SimModel};
use crate::core::sim_clock::{SimClock, ECOLOGY_PERIOD};
use crate::core::world_artifact::fnv1a_32;
use rand::rngs::StdRng;

/// The reserved [`CauseId`] for "the world law declared an exotic-energy source". Chosen well clear
/// of the small intervention cause ids used by scenarios so the two never collide.
pub const CAUSE_EXOTIC_WORLD_LAW: CauseId = 0xE0E0;

/// A reference world: the trophic ecosystem, an optional exotic-energy field with a closed MU
/// ledger, and an opt-in AE3 cohort population. The scalar `storage`/`spent_dissipated` seam is kept
/// for AE-205 transaction tests; real AE3 cohort storage/spend is folded into the same authoritative
/// budget by [`organism_storage_total`](Self::organism_storage_total).
#[derive(Clone, Debug)]
pub struct ReferenceEvolutionWorld {
    eco: ReferenceEcosystem,
    exotic: Option<ExoticEnergyField>,
    /// Display label from the law (e.g. "Mana"); kept for provenance/inspection only.
    exotic_display: Option<String>,
    /// Legacy AE-205 organism-storage test seam; AE3 cohort storage is held by `population`.
    storage: f64,
    /// Shared MU dissipation sink, including AE3 cohort metabolism and generation turnover.
    spent_dissipated: f64,
    /// The last field-total, so a causal record can carry the per-tick delta.
    last_field_total: f64,
    /// The previous exotic effect id, so records form a chain rooted at the world law.
    last_effect: Option<EffectId>,
    /// Declared runtime source forcings (AE-209). Immutable config: applied as their windows fire,
    /// never edited, and never able to change the [`WorldLawSet`].
    forcings: ExoticInterventionQueue,
    /// The opt-in AE3 reference population (AE-307). `None` — the default when a manifest declares
    /// no `ae3.` initial-condition key — is the AE1–AE2.5 path, bit-identical to before.
    population: Option<ReferencePopulation>,
    /// Tip of the current generation's mechanism chain (uptake → performance), so a reproduction
    /// event can be linked to the physiology that produced it.
    last_pathway_effect: Option<EffectId>,
}

impl ReferenceEvolutionWorld {
    /// Build the trophic ecosystem from the initial-condition set, defaulting any unset field to the
    /// [`ReferenceEcosystem::default`] value (so a baseline manifest reproduces the legacy world).
    fn eco_from_initial(initial: &InitialConditionSet) -> ReferenceEcosystem {
        let d = ReferenceEcosystem::default();
        ReferenceEcosystem {
            precip: initial.get("precip").unwrap_or(d.precip),
            temperature: initial.get("temperature").unwrap_or(d.temperature),
            npp: initial.get("npp").unwrap_or(d.npp),
            plants: initial.get("plants").unwrap_or(d.plants),
            herbivores: initial.get("herbivores").unwrap_or(d.herbivores),
            predators: initial.get("predators").unwrap_or(d.predators),
            detritus: initial.get("detritus").unwrap_or(d.detritus),
        }
    }

    /// Whether this world carries an exotic-energy field.
    pub fn has_exotic(&self) -> bool {
        self.exotic.is_some()
    }

    /// The exotic display label, if any.
    pub fn exotic_display(&self) -> Option<&str> {
        self.exotic_display.as_deref()
    }

    /// A read-only view of the exotic field, if present (for tests/inspection).
    pub fn exotic_field(&self) -> Option<&ExoticEnergyField> {
        self.exotic.as_ref()
    }

    /// A read-only view of the opt-in AE3 reference population, if this manifest enabled one.
    pub fn population(&self) -> Option<&ReferencePopulation> {
        self.population.as_ref()
    }

    /// All MU held in organism storage: the reference-model scalar plus the AE3 population's
    /// cohorts. This is the single value fed to the MU budget, so `exotic.stored` and the budget's
    /// `organism_storage` are the same number by construction.
    fn organism_storage_total(&self) -> f64 {
        self.storage + self.population.as_ref().map_or(0.0, |p| p.total_stored())
    }

    /// The single authoritative MU budget for this world: the field's ledger with the (test-double)
    /// storage-spend sink folded in. Both [`observables`](Self::observables) (`exotic.budget_error`)
    /// and [`exotic_budget`](Self::exotic_budget) read this same value, so the observable can never
    /// disagree with the result's budget (the seam is impossible to misuse inconsistently).
    /// Advance the AE3 population one ecology firing and record the mechanism chain.
    ///
    /// The causal contract here is deliberately strict: a performance effect roots at
    /// [`CAUSE_EXOTIC_WORLD_LAW`] **only** by descending from a real uptake effect. When no MU was
    /// taken (an absent source, an exhausted field, a blind or legacy strategy) the chain roots at
    /// [`CAUSE_BACKGROUND`] instead, so mere availability of a source can never be credited with a
    /// frequency change it did not mechanically produce.
    fn step_population(&mut self, tick: u64, ledger: &mut CausalLedger) {
        let Some(pop) = self.population.as_mut() else {
            return;
        };
        let before_uptake = pop.cum_uptake();

        // Steps 3–5: sense/uptake → pay cost and spend → derive performance.
        pop.step_physiology(self.exotic.as_mut(), &mut self.spent_dissipated);

        let took = pop.cum_uptake() - before_uptake;
        let cum_uptake = pop.cum_uptake();
        let perf_pathway = pop.pathway.mean_performance();
        let perf_legacy = pop.legacy.mean_performance();
        let boundary = pop.is_generation_boundary(tick);

        // The uptake transaction — the only bridge from the MU field into an organism.
        let uptake_effect = if took > 0.0 {
            Some(ledger.record(
                CAUSE_EXOTIC_WORLD_LAW,
                self.last_effect,
                tick,
                "exotic.uptake",
                cum_uptake,
                took,
                "pathway sensing and atomic uptake move MU from the field into organism storage",
            ))
        } else {
            None
        };

        // Measured performance. A parent's cause always wins in the ledger, so passing
        // CAUSE_BACKGROUND here is only load-bearing when there is no upstream uptake at all.
        let perf_parent = uptake_effect.or(self.last_pathway_effect);
        let perf_effect = ledger.record(
            CAUSE_BACKGROUND,
            perf_parent,
            tick,
            "evolution.performance_pathway",
            perf_pathway,
            perf_pathway - perf_legacy,
            "pathway performance = utilized MU minus maintenance/opportunity cost and toxicity",
        );
        self.last_pathway_effect = Some(perf_effect);

        // Step 6: the generation boundary — the only place composition may change.
        if boundary {
            let Some(pop) = self.population.as_mut() else {
                return;
            };
            let out = pop.reproduce(&mut self.spent_dissipated);
            let births_effect = ledger.record(
                CAUSE_BACKGROUND,
                self.last_pathway_effect,
                tick,
                "evolution.births",
                out.births,
                out.births,
                "reproduction: offspring shares follow each strategy's measured performance",
            );
            // Step 7: the frequency delta is recorded ONLY after offspring composition resolved.
            ledger.record(
                CAUSE_BACKGROUND,
                Some(births_effect),
                tick,
                "evolution.pathway_frequency",
                out.frequency_after,
                out.delta,
                "pathway frequency change equals the resolved offspring composition",
            );
            // Each generation starts a fresh mechanism chain so the ledger stays shallow; the field
            // chain it hangs from is unbroken, so the root cause is preserved.
            self.last_pathway_effect = None;
        }
    }

    fn current_budget(&self) -> Option<ExoticEnergyBudget> {
        self.exotic.as_ref().map(|f| {
            let mut b = f.budget(self.organism_storage_total());
            b.dissipated += self.spent_dissipated;
            b
        })
    }
}

impl ExperimentModel for ReferenceEvolutionWorld {
    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        // Defence in depth: the runner validates the manifest, but a directly-constructed model still
        // rejects an invalid law rather than booting on it.
        laws.validate()?;
        // Same for the forcings: validate + order them here, and refuse forcings with no field to act
        // on rather than silently ignoring a declared input.
        if !forcings.is_empty() && laws.exotic_energy.is_none() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: forcings[0].id,
                reason: "exotic forcings declared but laws.exotic_energy is None".into(),
            });
        }
        let forcing_queue =
            ExoticInterventionQueue::new(forcings.to_vec(), run_ticks).map_err(|reason| {
                ExperimentError::InvalidExoticIntervention {
                    id: forcings.first().map(|f| f.id).unwrap_or(0),
                    reason,
                }
            })?;
        let eco = Self::eco_from_initial(initial);
        let (exotic, exotic_display) = match &laws.exotic_energy {
            None => (None, None),
            Some(law) => {
                // Propagate an over-capacity / infeasible field as a structured error rather than
                // silently clamping the declared initial MU (see `ExoticEnergyField::from_law`).
                let field = ExoticEnergyField::from_law(law, grid.0, grid.1, seed)
                    .map_err(|reason| ExperimentError::FieldConstruction { reason })?;
                (Some(field), Some(law.display_name.clone()))
            }
        };
        // Grid applicability (D3): a forcing whose region cannot address any cell of THIS field is a
        // dead declared input — reject it structurally rather than let it silently do nothing.
        if let Some(field) = &exotic {
            for cmd in forcings {
                field.validate_region_applicable(cmd).map_err(|reason| {
                    ExperimentError::InvalidExoticIntervention { id: cmd.id, reason }
                })?;
            }
        }
        // AE3 opt-in population. The pathway genotype is tuned to whatever source THIS run declares,
        // so a fixture can never name a source incompatible with the active law; with no law it gets
        // the absent-source id, which matches no field.
        let population = match ReferencePopulationConfig::from_initial_conditions(
            initial,
            laws.exotic_energy.as_ref().map(|l| &l.id),
        )
        .map_err(|reason| ExperimentError::InvalidPopulation { reason })?
        {
            None => None,
            Some(config) => Some(
                ReferencePopulation::new(&config, seed)
                    .map_err(|reason| ExperimentError::InvalidPopulation { reason })?,
            ),
        };

        let last_field_total = exotic.as_ref().map(|f| f.total()).unwrap_or(0.0);
        Ok(Self {
            eco,
            exotic,
            exotic_display,
            storage: 0.0,
            spent_dissipated: 0.0,
            last_field_total,
            last_effect: None,
            forcings: forcing_queue,
            population,
            last_pathway_effect: None,
        })
    }

    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    ) {
        // 1) The EU dynamics — byte-identical to the legacy reference ecosystem (same RNG draws).
        self.eco.step(clock, active, ledger, rng);

        // 2) The exotic field advances on the ecology band. It draws NO RNG and does not feed back
        //    into the trophic pools, so the EU trajectory is unchanged by the field's presence
        //    (AE-S05). Its changes are recorded as a chain rooted at the world-law cause (AE-207).
        if let Some(field) = &mut self.exotic {
            if clock.band_fires(ECOLOGY_PERIOD) {
                // 2a) Declared runtime source forcings (AE-209) resolve BEFORE the field's own
                //     source/decay/diffusion, so a forcing at tick T shapes that same tick.
                //     Each is a *state* effect with its own CauseId — the world law is never touched.
                //
                //     Two distinct semantics, deliberately NOT conflated:
                //       • Add/Pulse   — inject MU (external source), recorded on `exotic.forcing`.
                //       • RemoveSource — suppress the base renewable source for this tick, recorded
                //         on `exotic.source_suppressed` as a positive **counterfactual**. No MU
                //         leaves the field, so nothing is recorded as movement.
                field.begin_tick_suppression();
                let field_was_empty = field.total() <= f64::EPSILON;
                let mut sole_positive_forcing_effect = None;
                let mut positive_forcing_count = 0u32;
                // Apply external injections first so suppression is capped against the renewable
                // source contribution that can still enter the cell's remaining headroom.
                for cmd in self
                    .forcings
                    .active_at(clock.tick)
                    .filter(|cmd| !matches!(cmd.kind, ExoticInterventionKind::RemoveSource))
                {
                    let moved = field.apply_forcing(cmd, clock.tick);
                    if moved > 0.0 {
                        let forcing_effect = ledger.record(
                            cmd.cause_id,
                            None,
                            clock.tick,
                            "exotic.forcing",
                            field.total(),
                            moved,
                            if matches!(cmd.kind, ExoticInterventionKind::Pulse) {
                                "declared runtime pulse injects MU into the field"
                            } else {
                                "declared runtime forcing adds MU to the field"
                            },
                        );
                        positive_forcing_count += 1;
                        sole_positive_forcing_effect = if positive_forcing_count == 1 {
                            Some(forcing_effect)
                        } else {
                            None
                        };
                    }
                }
                let mut pending_suppressed = 0.0;
                for cmd in self
                    .forcings
                    .active_at(clock.tick)
                    .filter(|cmd| matches!(cmd.kind, ExoticInterventionKind::RemoveSource))
                {
                    let suppressed = field.add_source_suppression(cmd, clock.tick);
                    if suppressed > 0.0 {
                        pending_suppressed += suppressed;
                        ledger.record(
                            cmd.cause_id,
                            None,
                            clock.tick,
                            "exotic.source_suppressed",
                            field.cum_source_suppressed + pending_suppressed,
                            suppressed,
                            "declared runtime forcing suppresses the base renewable source \
                             (no stored MU is removed)",
                        );
                    }
                }

                // 2b) Re-baseline BEFORE the field's own dynamics so the world-law effect below
                //     describes ONLY source/decay/diffusion. Without this, MU injected by a forcing
                //     would be counted a second time inside the world-law delta (double attribution).
                self.last_field_total = field.total();

                field.step();
                let total = field.total();
                let delta = total - self.last_field_total;
                // The current ledger supports one immediate parent. Attribute field provenance to
                // a forcing only in the exact case where doing so is unambiguous: the field was
                // empty, the world law cannot replenish it, and exactly one forcing injected MU.
                // Mixed-origin fields conservatively retain the existing world-law/field chain.
                let sole_forcing_parent =
                    if field_was_empty && field.source_rate == 0.0 && positive_forcing_count == 1 {
                        sole_positive_forcing_effect
                    } else {
                        None
                    };
                let effect = ledger.record(
                    CAUSE_EXOTIC_WORLD_LAW,
                    sole_forcing_parent.or(self.last_effect),
                    clock.tick,
                    "exotic.density_total",
                    total,
                    delta,
                    if sole_forcing_parent.is_some() {
                        "field dynamics propagate MU whose sole effective origin is the declared forcing"
                    } else if self.last_effect.is_none() {
                        "world-law exotic source establishes the MU field"
                    } else {
                        "exotic source / decay / diffusion update the MU field"
                    },
                );
                self.last_effect = Some(effect);
                self.last_field_total = total;
            }
        }

        // 3) The AE3 reference population, on the same ecology band and strictly AFTER the field has
        //    been updated, so a cohort senses this tick's MU. The ordering is the goal's update
        //    order: sense/uptake → cost + spend → performance → (at a boundary) reproduction.
        //
        //    It draws from its OWN deterministic RNG stream, never `rng` — that is what keeps the
        //    legacy ecology draw order (and therefore the AE-S01 baseline checksum) untouched.
        if clock.band_fires(ECOLOGY_PERIOD) && self.population.is_some() {
            self.step_population(clock.tick, ledger);
        }
    }

    fn checksum(&self) -> u32 {
        match (&self.exotic, &self.population) {
            // Baseline: reproduce the legacy checksum bits EXACTLY (AE-S01). Only a world with
            // neither an exotic field nor an AE3 population can take this path.
            (None, None) => self.eco.checksum(),
            // Otherwise fold the EU state, the MU ledger and the population into one deterministic
            // fingerprint. The AE1–AE2.5 (field, no population) layout is preserved exactly so a
            // pre-AE3 treatment run keeps its checksum bits.
            (exotic, population) => {
                let mut fields = vec![
                    self.eco.precip,
                    self.eco.temperature,
                    self.eco.npp,
                    self.eco.plants,
                    self.eco.herbivores,
                    self.eco.predators,
                    self.eco.detritus,
                ];
                if let Some(field) = exotic {
                    fields.extend_from_slice(&[
                        field.total(),
                        field.cum_sourced,
                        field.cum_dissipated,
                        field.cum_exported,
                        self.storage,
                        self.spent_dissipated,
                    ]);
                }
                if let Some(pop) = population {
                    let g = &pop.pathway.genotype;
                    fields.extend_from_slice(&[
                        pop.generation as f64,
                        pop.births,
                        pop.legacy.count,
                        pop.pathway.count,
                        pop.legacy.state.stored_mu,
                        pop.pathway.state.stored_mu,
                        pop.cum_uptake(),
                        pop.cum_spent(),
                        pop.last_frequency_delta,
                        g.sensing_affinity as f64,
                        g.uptake_rate as f64,
                        g.storage_capacity as f64,
                        g.utilization_efficiency as f64,
                        g.tolerance as f64,
                        g.maintenance_cost as f64,
                        g.allocation as f64,
                    ]);
                }
                let mut bytes = Vec::with_capacity(fields.len() * 8);
                for f in fields {
                    bytes.extend_from_slice(&f.to_bits().to_le_bytes());
                }
                fnv1a_32(&bytes)
            }
        }
    }

    fn observables(&self) -> Vec<(String, f64)> {
        let mut out: Vec<(String, f64)> = self
            .eco
            .observables()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        if let (Some(field), Some(budget)) = (&self.exotic, self.current_budget()) {
            // `exotic.dissipated` and `exotic.budget_error` come from the SAME authoritative budget as
            // `exotic_budget()`, so the two can never diverge.
            out.push(("exotic.density_total".into(), field.total()));
            out.push(("exotic.sourced".into(), budget.sourced));
            out.push(("exotic.dissipated".into(), budget.dissipated));
            out.push(("exotic.stored".into(), budget.organism_storage));
            out.push(("exotic.budget_error".into(), budget.balance_error()));
        }
        // AE3 observables exist only when the opt-in population does — a disabled population emits
        // nothing rather than a fabricated zero that would read as "measured".
        if let Some(pop) = &self.population {
            let legacy = pop.legacy.mean_performance();
            let pathway = pop.pathway.mean_performance();
            out.push(("evolution.population_total".into(), pop.total()));
            out.push(("evolution.pathway_population".into(), pop.pathway.count));
            out.push((
                "evolution.pathway_frequency".into(),
                pop.pathway_frequency(),
            ));
            out.push(("evolution.generation".into(), pop.generation as f64));
            out.push(("evolution.births".into(), pop.births));
            out.push(("evolution.performance_legacy".into(), legacy));
            out.push(("evolution.performance_pathway".into(), pathway));
            out.push(("evolution.performance_delta".into(), pathway - legacy));
            out.push(("exotic.uptake".into(), pop.cum_uptake()));
            out.push(("exotic.spent".into(), pop.cum_spent()));
        }
        out
    }

    fn exotic_budget(&self) -> Option<ExoticEnergyBudget> {
        self.current_budget()
    }

    fn reconfigure_forcings(
        &mut self,
        forcings: &[ExoticIntervention],
        run_ticks: u64,
    ) -> Result<(), ExperimentError> {
        if !forcings.is_empty() && self.exotic.is_none() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: forcings[0].id,
                reason: "exotic forcings declared but this world has no exotic field".into(),
            });
        }
        if let Some(field) = &self.exotic {
            for cmd in forcings {
                field.validate_region_applicable(cmd).map_err(|reason| {
                    ExperimentError::InvalidExoticIntervention { id: cmd.id, reason }
                })?;
            }
        }
        self.forcings =
            ExoticInterventionQueue::new(forcings.to_vec(), run_ticks).map_err(|reason| {
                ExperimentError::InvalidExoticIntervention {
                    id: forcings.first().map(|f| f.id).unwrap_or(0),
                    reason,
                }
            })?;
        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        // The reference world is cheap and fully `Clone`; the snapshot is a deep copy of its state
        // (EU pools + MU field/ledger + storage). It is NOT serialized to disk (that is AE4
        // persistence, out of scope) — it lives in memory for a mid-run checkpoint fork.
        self.clone()
    }

    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        // Resume from a checkpoint: restore the full state but start a FRESH causal chain for the
        // post-fork segment (the pre-fork ledger belongs to the parent run; its effect ids do not
        // exist in this branch's new ledger). `last_field_total` is kept so the first post-fork
        // causal delta is measured against the checkpoint, not zero.
        // The population — counts, genotypes, developed phenotypes, MU storage AND its private RNG
        // stream — rides along in the clone, so a control continuation reproduces the uninterrupted
        // run bit-for-bit. Only ledger-local effect ids are cleared: they index the PARENT run's
        // ledger and do not exist in this branch's fresh one.
        let mut restored = snapshot.clone();
        restored.last_effect = None;
        restored.last_pathway_effect = None;
        Ok(restored)
    }

    type Snapshot = ReferenceEvolutionWorld;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::exotic_energy::ExoticEnergyLaw;
    use crate::core::experiment::{
        ExperimentManifest, FactorDiff, ObservableRegistry, WorldLawSet, MANIFEST_SCHEMA_VERSION,
    };
    use crate::core::experiment_runner::{genesis_fork, run_manifest_seed};
    use crate::core::intervention::{Curve, Region};
    use crate::core::world_artifact::WorldIdentity;

    fn ref_init() -> InitialConditionSet {
        InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("npp".into(), 1.0),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ])
    }

    fn treatment_manifest() -> ExperimentManifest {
        ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: "mana-slice".into(),
            name: "renewable-patchy-mana".into(),
            observer: crate::core::observer::ObserverPolicy::default(),
            world_identity: WorldIdentity::default(),
            laws: WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(200.0, 5)),
            initial_conditions: ref_init(),
            interventions: vec![],
            seeds: vec![2026],
            duration_ticks: 6000,
            sample_period: 600,
            observable_ids: vec![
                "plants".into(),
                "herbivores".into(),
                "predators".into(),
                "exotic.density_total".into(),
                "exotic.budget_error".into(),
            ],
            exotic_interventions: Vec::new(),
        }
    }

    // ---- AE3: population wiring, observables and the full causal chain -----------------------

    use crate::core::causal::CAUSE_BACKGROUND;
    use crate::core::evolution_pathway as ae3;

    /// The reference initial conditions plus the AE3 opt-in population keys.
    fn ae3_init(maintenance: f64) -> InitialConditionSet {
        let mut v = ref_init().values;
        v.push((ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0));
        v.push((ae3::AE3_KEY_POPULATION_CAPACITY.into(), 100.0));
        v.push((ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.5));
        v.push((ae3::AE3_KEY_GENERATION_TICKS.into(), 600.0));
        v.push((ae3::AE3_KEY_PATHWAY_MAINTENANCE.into(), maintenance));
        InitialConditionSet::new(v)
    }

    /// A source-present manifest whose world also carries the AE3 reference population.
    fn ae3_manifest(maintenance: f64) -> ExperimentManifest {
        let mut m = treatment_manifest();
        m.experiment_id = "ae3-slice".into();
        m.initial_conditions = ae3_init(maintenance);
        m.observable_ids = vec![
            "plants".into(),
            "herbivores".into(),
            "predators".into(),
            "exotic.density_total".into(),
            "exotic.budget_error".into(),
            "exotic.uptake".into(),
            "exotic.spent".into(),
            "evolution.pathway_frequency".into(),
            "evolution.performance_delta".into(),
            "evolution.generation".into(),
            "evolution.births".into(),
        ];
        m
    }

    #[test]
    fn ae3_s01_a_world_without_ae3_keys_stays_bit_identical_to_the_legacy_baseline() {
        // AE-S01 regression for this slice: no AE3 initial keys + exotic None ⇒ no population, no
        // AE3 observables, and the legacy checksum bits unchanged.
        let world = ReferenceEvolutionWorld::from_manifest(
            &WorldLawSet::baseline(),
            &ref_init(),
            &[],
            1,
            (16, 16),
            6000,
        )
        .unwrap();
        assert!(world.population().is_none(), "AE3 is opt-in and stays off");
        assert_eq!(world.checksum(), world.eco.checksum());
        assert!(world
            .observables()
            .iter()
            .all(|(k, _)| !k.starts_with("evolution.")));
    }

    #[test]
    fn ae308_population_observables_are_emitted_only_when_the_population_exists() {
        let reg = ObservableRegistry::reference_default();

        // Enabled: every declared AE3 observable is present and self-consistent.
        let m = ae3_manifest(0.01);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed(), "{:?}", res.status);
        assert!(
            res.warnings.is_empty(),
            "every emitted AE3 observable must have a registry spec: {:?}",
            res.warnings
        );
        let total = res.observable("evolution.population_total").unwrap();
        let bearers = res.observable("evolution.pathway_population").unwrap();
        let freq = res.observable("evolution.pathway_frequency").unwrap();
        assert!(total > 0.0);
        assert!(
            (freq - bearers / total).abs() < 1e-12,
            "frequency must be derived, not asserted"
        );
        assert!(res.observable("evolution.generation").unwrap() > 0.0);
        assert!(res.observable("evolution.births").unwrap() > 0.0);

        // Disabled: no AE3 observable is fabricated.
        let plain = treatment_manifest();
        let pres =
            run_manifest_seed::<ReferenceEvolutionWorld>(&plain, &reg, plain.seeds[0], None, None);
        assert!(pres
            .final_observables
            .iter()
            .all(|(k, _)| !k.starts_with("evolution.")
                && k != "exotic.uptake"
                && k != "exotic.spent"));
    }

    #[test]
    fn ae309_s12_pathway_frequency_traces_back_to_the_exotic_world_law() {
        // AE-S12: the FINAL frequency effect must trace, through reproduction and performance and
        // the uptake transaction, to the world-law cause that created the MU in the first place.
        let reg = ObservableRegistry::reference_default();
        let m = ae3_manifest(0.01);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed());

        let effect_of = |target: &str| -> Option<&crate::core::causal::EffectRecord> {
            res.ledger.all().iter().rev().find(|e| e.target == target)
        };
        let freq = effect_of("evolution.pathway_frequency").expect("a frequency effect exists");
        assert_eq!(
            res.ledger.root_cause(freq.effect_id),
            Some(CAUSE_EXOTIC_WORLD_LAW),
            "the frequency change must root at the exotic world law"
        );

        // And the chain really passes through each mechanism link, in order.
        let chain = res.ledger.trace_to_root(freq.effect_id);
        let targets: Vec<&str> = chain
            .iter()
            .filter_map(|id| res.ledger.get(*id))
            .map(|e| e.target.as_str())
            .collect();
        for link in [
            "evolution.pathway_frequency",
            "evolution.births",
            "evolution.performance_pathway",
            "exotic.uptake",
            "exotic.density_total",
        ] {
            assert!(
                targets.contains(&link),
                "the causal chain is missing '{link}': {targets:?}"
            );
        }
        // Ordering: frequency is downstream of births, which is downstream of performance.
        let pos = |t: &str| targets.iter().position(|x| *x == t).unwrap();
        assert!(pos("evolution.pathway_frequency") < pos("evolution.births"));
        assert!(pos("evolution.births") < pos("evolution.performance_pathway"));
        assert!(pos("evolution.performance_pathway") < pos("exotic.uptake"));
    }

    #[test]
    fn ae309_s12_pathway_frequency_traces_to_a_sole_effective_forcing() {
        // Isolate forcing provenance: the declared law provides no initial or renewable MU, so the
        // one AddSource command is the only possible origin of every unit the pathway can consume.
        let reg = ObservableRegistry::reference_default();
        let mut m = ae3_manifest(0.01);
        let mut inert_law = ExoticEnergyLaw::mana_uniform(0.0);
        inert_law.source_rate = 0.0;
        inert_law.diffusion_rate = 0.0;
        inert_law.decay_rate = 0.0;
        m.laws = WorldLawSet::with_exotic(inert_law);
        m.duration_ticks = 600;
        m.sample_period = 60;
        let forcing = add_forcing(77, 60, 600, 0.2);
        let forcing_cause = forcing.cause_id;
        m.exotic_interventions = vec![forcing];

        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed(), "{:?}", res.status);
        assert!(res.observable("exotic.uptake").unwrap() > 0.0);

        let frequency = res
            .ledger
            .all()
            .iter()
            .rev()
            .find(|effect| effect.target == "evolution.pathway_frequency")
            .expect("the generation boundary must record a frequency effect");
        assert_eq!(
            res.ledger.root_cause(frequency.effect_id),
            Some(forcing_cause),
            "when forcing is the sole effective MU origin, selection must trace to that forcing"
        );
        let targets: Vec<&str> = res
            .ledger
            .trace_to_root(frequency.effect_id)
            .iter()
            .filter_map(|id| res.ledger.get(*id))
            .map(|effect| effect.target.as_str())
            .collect();
        for link in [
            "evolution.pathway_frequency",
            "evolution.births",
            "evolution.performance_pathway",
            "exotic.uptake",
            "exotic.density_total",
            "exotic.forcing",
        ] {
            assert!(
                targets.contains(&link),
                "forcing causal chain is missing '{link}': {targets:?}"
            );
        }
    }

    #[test]
    fn ae309_absent_source_cost_roots_at_background_not_a_fabricated_mana_cause() {
        // With no exotic law there is no Mana to blame: the pathway's cost is background dynamics.
        let reg = ObservableRegistry::reference_default();
        let mut m = ae3_manifest(0.05);
        m = m.control_variant();
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed(), "{:?}", res.status);

        let freq = res
            .ledger
            .all()
            .iter()
            .rev()
            .find(|e| e.target == "evolution.pathway_frequency")
            .expect("selection still happens without a source");
        assert_eq!(
            res.ledger.root_cause(freq.effect_id),
            Some(CAUSE_BACKGROUND),
            "an absent-source world must not invent an exotic cause"
        );
        assert!(
            !res.ledger
                .all()
                .iter()
                .any(|e| e.cause_id == CAUSE_EXOTIC_WORLD_LAW),
            "no world-law exotic effect may exist without an exotic law"
        );
        assert_eq!(res.observable("exotic.uptake"), Some(0.0));
    }

    #[test]
    fn ae3_s04_s05_organism_uptake_keeps_mu_closed_and_leaves_eu_byte_identical() {
        let reg = ObservableRegistry::reference_default();
        let m = ae3_manifest(0.01);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed());

        // AE-S04: the MU ledger still closes once organisms hold and burn MU.
        let b = res
            .exotic_budget
            .clone()
            .expect("treatment has an MU budget");
        assert!(
            res.observable("exotic.uptake").unwrap() > 0.0,
            "MU really moved"
        );
        assert!(
            res.observable("exotic.spent").unwrap() > 0.0,
            "and was really spent"
        );
        assert!(
            b.balance_error().abs() / b.throughput() < 1e-4,
            "MU budget must close with organism storage: error {} over throughput {}",
            b.balance_error(),
            b.throughput()
        );
        // `exotic.stored` is the organism storage the budget actually used — they cannot disagree.
        assert_eq!(res.observable("exotic.stored").unwrap(), b.organism_storage);

        // AE-S05: the closed-EU pools are byte-identical to the same run without a population.
        let mut no_pop = ae3_manifest(0.01);
        no_pop.initial_conditions = ref_init();
        no_pop
            .observable_ids
            .retain(|id| !ae3::AE3_OBSERVABLE_IDS.contains(&id.as_str()));
        let eu = run_manifest_seed::<ReferenceEvolutionWorld>(
            &no_pop,
            &reg,
            no_pop.seeds[0],
            None,
            None,
        );
        for name in ["plants", "herbivores", "predators", "detritus", "npp"] {
            assert_eq!(
                res.observable(name),
                eu.observable(name),
                "{name} must be untouched by exotic physiology (MU is not EU)"
            );
        }
    }

    #[test]
    fn ae3_s02_a_population_run_replays_deterministically() {
        let reg = ObservableRegistry::reference_default();
        let m = ae3_manifest(0.01);
        let a = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        let b = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert_eq!(a.final_checksum, b.final_checksum);
        assert_eq!(a.final_observables, b.final_observables);
        assert_eq!(a.ledger.len(), b.ledger.len());
        // The population is part of run identity: a different composition is a different checksum.
        let mut other = ae3_manifest(0.01);
        let mut v = other.initial_conditions.values.clone();
        v.retain(|(k, _)| k != ae3::AE3_KEY_PATHWAY_FRACTION);
        v.push((ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.25));
        other.initial_conditions = InitialConditionSet::new(v);
        let c =
            run_manifest_seed::<ReferenceEvolutionWorld>(&other, &reg, other.seeds[0], None, None);
        assert_ne!(a.final_checksum, c.final_checksum);
    }

    #[test]
    fn ae3_checkpoint_restore_preserves_population_and_rng_state() {
        // A control continuation from the checkpoint must equal an uninterrupted run bit-for-bit,
        // which is only true if the population counts, genotypes, storage AND its private RNG
        // stream all survive the snapshot.
        let reg = ObservableRegistry::reference_default();
        let m = ae3_manifest(0.01);
        let uninterrupted =
            run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        let fork = crate::core::experiment_runner::checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            m.seeds[0],
            3000,
            &[],
        )
        .expect("fork validates");
        assert_eq!(
            fork.control.final_checksum, uninterrupted.final_checksum,
            "a control continuation must reproduce the uninterrupted run exactly"
        );
        assert_eq!(
            fork.control.final_observables,
            uninterrupted.final_observables
        );
        assert_eq!(
            fork.control.observable("evolution.generation"),
            uninterrupted.observable("evolution.generation")
        );
    }

    // ---- AE-310 / AE-S14: the 2×2 factorial and paired multi-seed evidence -------------------

    /// One factorial cell: exotic source present or absent × pathway maintenance cost.
    fn factorial_cell(source_present: bool, maintenance: f64) -> ExperimentManifest {
        let mut m = ae3_manifest(maintenance);
        m.experiment_id = format!("ae3-factorial-{source_present}-{maintenance}");
        if !source_present {
            m = m.control_variant();
        }
        m
    }

    fn run_cell(
        source_present: bool,
        maintenance: f64,
    ) -> crate::core::experiment_runner::RunResult {
        let reg = ObservableRegistry::reference_default();
        let m = factorial_cell(source_present, maintenance);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed(), "{:?}", res.status);
        res
    }

    #[test]
    fn ae310_factorial_absent_source_never_gives_a_pathway_a_free_advantage() {
        // Row 1 — absent × zero cost: the two strategies are mechanically identical, so frequency
        // must only drift, never trend. This is the factorial's control cell.
        let control = run_cell(false, 0.0);
        let f = control.observable("evolution.pathway_frequency").unwrap();
        assert!(
            (f - 0.5).abs() < 0.15,
            "a cost-free pathway in a source-free world must not trend: {f}"
        );
        assert_eq!(control.observable("exotic.uptake"), Some(0.0));
        assert_eq!(
            control.observable("evolution.performance_delta"),
            Some(0.0),
            "identical mechanisms must measure identical performance"
        );

        // Row 2 — absent × positive cost: the pathway pays and gains nothing, so it must LOSE.
        let costly = run_cell(false, 0.05);
        let fc = costly.observable("evolution.pathway_frequency").unwrap();
        assert!(
            fc < 0.5,
            "a costly pathway with no source must decline, got {fc}"
        );
        assert!(
            costly.observable("evolution.performance_delta").unwrap() < 0.0,
            "its reproductive performance must be lower than legacy"
        );
        assert!(
            fc < f,
            "and it must do worse than the zero-cost control: {fc} vs {f}"
        );
    }

    #[test]
    fn ae310_factorial_present_source_advantage_flows_through_the_transaction() {
        // Row 3 — present × zero/low cost: MU is taken and burned, and only then does performance
        // move. Row 4 — present × positive cost: the benefit must overcome the cost, and the
        // reported result is the actual trade-off, not an assumed win.
        let cheap = run_cell(true, 0.0);
        let dear = run_cell(true, 0.05);

        for (label, res) in [("zero-cost", &cheap), ("positive-cost", &dear)] {
            assert!(
                res.observable("exotic.uptake").unwrap() > 0.0,
                "{label}: MU must actually be taken"
            );
            assert!(
                res.observable("exotic.spent").unwrap() > 0.0,
                "{label}: and actually spent — presence alone buys nothing"
            );
            assert!(
                res.observable("evolution.performance_delta").unwrap() > 0.0,
                "{label}: the transaction must raise measured performance"
            );
            assert!(
                res.observable("evolution.pathway_frequency").unwrap() > 0.5,
                "{label}: selection must then raise the pathway's frequency"
            );
        }

        // The trade-off is real and correctly signed: paying more for the same benefit does worse.
        let f_cheap = cheap.observable("evolution.pathway_frequency").unwrap();
        let f_dear = dear.observable("evolution.pathway_frequency").unwrap();
        assert!(
            f_dear < f_cheap,
            "a costlier pathway must end lower than a cheap one: {f_dear} vs {f_cheap}"
        );

        // And the source is what makes the difference: same cost, no source ⇒ opposite outcome.
        let no_source = run_cell(false, 0.05);
        assert!(
            no_source.observable("evolution.pathway_frequency").unwrap() < 0.5 && f_dear > 0.5,
            "the exotic source is the factor that flips the sign of selection"
        );
    }

    #[test]
    fn ae_s14_ae3_paired_multi_seed_reports_a_finite_effect_and_interval() {
        // AE-S14 for the AE3 slice: a SAME-SEED paired ensemble over the declared factor
        // (laws.exotic_energy), reporting paired effect, interval and d_z, and preserving every
        // requested pair. Five seeds is a small ensemble — enough to report an effect with an
        // interval, NOT enough to claim statistical confidence.
        let reg = ObservableRegistry::reference_default();
        let mut treatment = ae3_manifest(0.02);
        treatment.seeds = vec![2026, 2027, 2028, 2029, 2030];

        let report =
            crate::core::experiment_runner::run_paired_ensemble::<ReferenceEvolutionWorld>(
                &treatment,
                &reg,
                &FactorDiff::genesis_exotic(),
            )
            .expect("the paired ensemble validates");

        assert_eq!(
            report.declared_factors,
            vec!["laws.exotic_energy".to_string()]
        );
        assert_eq!(report.seed_order, treatment.seeds);
        assert_eq!(report.pairs.len(), 5, "every requested pair is preserved");
        assert_eq!(report.complete_pairs(), 5);
        assert_eq!(report.incomplete_pairs(), 0);
        // Both sides ran the same seeds in the same order — that is what makes the deltas paired.
        assert!(report
            .pairs
            .iter()
            .all(|p| p.control.provenance.seed == p.treatment.provenance.seed));
        // The law fingerprint is the declared difference; the registry is shared.
        assert_ne!(
            report.control_law_fingerprint,
            report.treatment_law_fingerprint
        );

        let effect = report
            .effect_of("evolution.pathway_frequency")
            .expect("the frequency effect is reported");
        assert_eq!(effect.n_requested, 5);
        assert_eq!(effect.n_complete_pairs, 5);
        for (name, v) in [
            ("paired_mean_delta", effect.paired_mean_delta),
            ("paired_sd", effect.paired_sd),
            ("paired_se", effect.paired_se),
            ("ci95_low", effect.ci95_low),
            ("ci95_high", effect.ci95_high),
            ("paired_dz", effect.paired_dz),
        ] {
            let v = v.unwrap_or_else(|| panic!("{name} must be defined for 5 complete pairs"));
            assert!(v.is_finite(), "{name} must be finite, got {v}");
        }
        let delta = effect.paired_mean_delta.unwrap();
        assert!(
            delta > 0.0 && effect.ci95_low.unwrap() > 0.0,
            "a usable source must raise pathway frequency across the ensemble: delta {delta}, \
             interval [{:?}, {:?}]",
            effect.ci95_low,
            effect.ci95_high
        );

        // AE-S05 holds across the whole ensemble: the EU pools show no paired effect at all.
        for name in ["plants", "herbivores", "predators", "detritus"] {
            let eu = report
                .effect_of(name)
                .expect("EU observables are paired too");
            assert_eq!(
                eu.paired_mean_delta,
                Some(0.0),
                "{name} must show exactly zero paired effect (MU is not EU)"
            );
        }
    }

    #[test]
    fn baseline_world_is_bit_identical_to_reference_ecosystem() {
        // With exotic None, the world's checksum equals the inner ecosystem's — the AE-S01 guarantee
        // at the model level, independent of the runner.
        let world = ReferenceEvolutionWorld::from_manifest(
            &WorldLawSet::baseline(),
            &ref_init(),
            &[],
            1,
            (16, 16),
            6000,
        )
        .unwrap();
        assert!(!world.has_exotic());
        assert!(world.exotic_budget().is_none());
        assert_eq!(world.checksum(), world.eco.checksum());
    }

    #[test]
    fn exotic_none_is_the_only_baseline_path() {
        // Task-4: `exotic_energy = None` is the sole baseline — no field, no exotic observables, no MU
        // budget. There is no `Disabled` source model, so a `Some(law)` world can never masquerade as
        // the baseline: it always carries a live field and exotic observables.
        let baseline = ReferenceEvolutionWorld::from_manifest(
            &WorldLawSet::baseline(),
            &ref_init(),
            &[],
            1,
            (16, 16),
            6000,
        )
        .unwrap();
        assert!(
            baseline.exotic_field().is_none(),
            "no field is allocated on the baseline"
        );
        assert!(baseline
            .observables()
            .iter()
            .all(|(k, _)| !k.starts_with("exotic.")));

        let treated = ReferenceEvolutionWorld::from_manifest(
            &WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(200.0, 5)),
            &ref_init(),
            &[],
            1,
            (16, 16),
            6000,
        )
        .unwrap();
        assert!(
            treated.exotic_field().is_some(),
            "a Some(law) world always has a live field"
        );
        assert!(treated
            .observables()
            .iter()
            .any(|(k, _)| k.starts_with("exotic.")));
        assert!(treated.exotic_budget().is_some());
    }

    #[test]
    fn over_capacity_law_fails_construction_with_structured_error() {
        // Task-2: an infeasible initial_amount surfaces as a structured ExperimentError, never a
        // silently-shrunk initial condition.
        let mut law = ExoticEnergyLaw::mana_uniform(1_000_000.0);
        law.max_density = 1.0;
        let err = ReferenceEvolutionWorld::from_manifest(
            &WorldLawSet::with_exotic(law),
            &ref_init(),
            &[],
            1,
            (16, 16),
            6000,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::core::experiment::ExperimentError::FieldConstruction { .. }
        ));
    }

    // ---- AE-209 (M3): forcings wired through manifest → model → causal ledger ----------------

    fn removal_forcing(id: u32, start: u64, dur: u64) -> ExoticIntervention {
        ExoticIntervention {
            id,
            cause_id: 900 + id,
            kind: ExoticInterventionKind::RemoveSource,
            region: Region::Global,
            start_tick: start,
            duration_ticks: dur,
            amount: 0.5,
            curve: Curve::Step,
        }
    }

    // ---- AUDIT D4: no double attribution between forcing and world-law causes ----------------

    fn add_forcing(id: u32, start: u64, dur: u64, amount: f32) -> ExoticIntervention {
        ExoticIntervention {
            id,
            cause_id: 900 + id,
            kind: ExoticInterventionKind::AddSource,
            region: Region::Global,
            start_tick: start,
            duration_ticks: dur,
            amount,
            curve: Curve::Step,
        }
    }

    #[test]
    fn d4_add_forcing_movement_is_not_also_counted_in_the_world_law_delta() {
        // The forcing injects MU under its OWN cause. The world-law effect recorded in the same tick
        // must describe only the field's own source/decay/diffusion — not the injected MU as well.
        // (The previous code computed the world-law delta as `total - last_field_total` *after*
        // applying the forcing, so the injection was attributed twice.)
        let reg = ObservableRegistry::reference_default();
        let mut m = treatment_manifest();
        m.duration_ticks = 300;
        m.sample_period = 60;
        let f = add_forcing(1, 120, 1, 2.0);
        let cause = f.cause_id;
        m.exotic_interventions = vec![f];

        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed());

        // The forcing tick (120) has both a forcing effect and a world-law effect.
        let forcing_delta: f64 = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.tick == 120 && e.target == "exotic.forcing")
            .map(|e| e.delta)
            .sum();
        let law_delta_at_forcing: f64 = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.tick == 120 && e.target == "exotic.density_total")
            .map(|e| e.delta)
            .sum();
        assert!(forcing_delta > 0.0, "the forcing injected MU");

        // The world-law delta at that tick must match an UNFORCED run's world-law delta at the same
        // tick — i.e. it excludes the injection entirely. (Source/decay/diffusion depend on density,
        // so they are not bit-identical; the point is that the injection is not folded in.)
        let mut plain = treatment_manifest();
        plain.duration_ticks = 300;
        plain.sample_period = 60;
        let plain_res =
            run_manifest_seed::<ReferenceEvolutionWorld>(&plain, &reg, plain.seeds[0], None, None);
        let plain_law_delta: f64 = plain_res
            .ledger
            .all()
            .iter()
            .filter(|e| e.tick == 120 && e.target == "exotic.density_total")
            .map(|e| e.delta)
            .sum();

        assert!(
            (law_delta_at_forcing - plain_law_delta).abs() < forcing_delta * 0.5,
            "world-law delta {law_delta_at_forcing} looks like it absorbed the forcing \
             {forcing_delta} (unforced baseline was {plain_law_delta})"
        );

        // Attribution roots are distinct and correct.
        let forcing_roots: Vec<u32> = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.target == "exotic.forcing")
            .map(|e| e.cause_id)
            .collect();
        assert!(forcing_roots.iter().all(|c| *c == cause));
        assert!(res
            .ledger
            .all()
            .iter()
            .filter(|e| e.target == "exotic.density_total")
            .all(|e| e.cause_id == CAUSE_EXOTIC_WORLD_LAW));
    }

    #[test]
    fn d4_remove_source_is_attributed_as_suppression_without_pretending_mu_moved() {
        // A RemoveSource must be attributable, but it must NOT claim MU left the field: it records a
        // counterfactual suppression on its own target, and the field's stored MU is never debited.
        let reg = ObservableRegistry::reference_default();
        let mut m = treatment_manifest();
        m.duration_ticks = 600;
        m.sample_period = 60;
        let mut first = removal_forcing(1, 120, 300);
        first.amount = 0.005;
        let mut second = removal_forcing(2, 120, 300);
        second.amount = 0.005;
        let causes = [first.cause_id, second.cause_id];
        m.exotic_interventions = vec![first, second];

        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed());

        let supp: Vec<&crate::core::causal::EffectRecord> = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.target == "exotic.source_suppressed")
            .collect();
        assert!(!supp.is_empty(), "suppression must be recorded");
        assert!(supp.iter().all(|e| causes.contains(&e.cause_id)));
        // Suppression is reported as a positive counterfactual amount, and there is NO
        // `exotic.forcing` movement record for a RemoveSource (no MU moved).
        assert!(supp.iter().all(|e| e.delta > 0.0));
        let at_first_tick: Vec<_> = supp.iter().filter(|e| e.tick == 120).copied().collect();
        assert_eq!(at_first_tick.len(), 2);
        assert_eq!(at_first_tick[0].cause_id, causes[0]);
        assert_eq!(at_first_tick[1].cause_id, causes[1]);
        assert!(
            (at_first_tick[1].quantity - (at_first_tick[0].quantity + at_first_tick[1].delta))
                .abs()
                < 1e-9,
            "causal values must accumulate monotonically across overlapping suppressions"
        );
        assert!(
            !res.ledger
                .all()
                .iter()
                .any(|e| e.target == "exotic.forcing" && causes.contains(&e.cause_id)),
            "a RemoveSource must not record MU movement"
        );

        // The MU ledger closes and nothing was dissipated by the suppression.
        let b = res.exotic_budget.clone().expect("budget");
        assert!(b.balance_error().abs() / b.throughput() < 1e-4);
    }

    #[test]
    fn ae209_m3_forcing_changes_field_but_never_the_world_law() {
        let reg = ObservableRegistry::reference_default();
        let base = treatment_manifest();
        let mut forced = treatment_manifest();
        forced.exotic_interventions = vec![removal_forcing(1, 1200, 3000)];

        let law_fp_before = base.laws.fingerprint();
        let law_fp_after = forced.laws.fingerprint();
        assert_eq!(
            law_fp_before, law_fp_after,
            "a runtime forcing must NOT change the world-law fingerprint (laws are immutable)"
        );
        // The manifest fingerprint DOES change — the declared input differs.
        assert_ne!(base.fingerprint(), forced.fingerprint());

        let plain =
            run_manifest_seed::<ReferenceEvolutionWorld>(&base, &reg, base.seeds[0], None, None);
        let with_forcing = run_manifest_seed::<ReferenceEvolutionWorld>(
            &forced,
            &reg,
            forced.seeds[0],
            None,
            None,
        );
        assert!(plain.status.is_completed() && with_forcing.status.is_completed());

        // Provenance keeps the law fingerprint identical across both runs.
        assert_eq!(
            plain.provenance.law_fingerprint,
            with_forcing.provenance.law_fingerprint
        );

        // The removal forcing actually lowered the field.
        let plain_density = plain.observable("exotic.density_total").unwrap();
        let forced_density = with_forcing.observable("exotic.density_total").unwrap();
        assert!(
            forced_density < plain_density,
            "a RemoveSource forcing must lower the MU field: {forced_density} vs {plain_density}"
        );
    }

    #[test]
    fn ae209_m3_forcing_keeps_mu_closed_and_eu_untouched() {
        let reg = ObservableRegistry::reference_default();
        let base = treatment_manifest();
        let mut forced = treatment_manifest();
        forced.exotic_interventions = vec![removal_forcing(1, 1200, 3000)];

        let plain =
            run_manifest_seed::<ReferenceEvolutionWorld>(&base, &reg, base.seeds[0], None, None);
        let with_forcing = run_manifest_seed::<ReferenceEvolutionWorld>(
            &forced,
            &reg,
            forced.seeds[0],
            None,
            None,
        );

        // AE-S04: the MU ledger still closes with the forcing active.
        let b = with_forcing
            .exotic_budget
            .clone()
            .expect("treatment has a budget");
        assert!(
            b.balance_error().abs() / b.throughput() < 1e-4,
            "MU budget must close under a forcing: error {}",
            b.balance_error()
        );
        assert!(b.dissipated > 0.0, "the removal must be booked as a sink");

        // AE-S05: the closed-EU pools are byte-identical — an exotic forcing never touches biomass.
        for name in ["plants", "herbivores", "predators", "detritus", "npp"] {
            let a = plain.observable(name).unwrap();
            let c = with_forcing.observable(name).unwrap();
            assert_eq!(a, c, "{name} must be unchanged by an exotic forcing");
        }
    }

    #[test]
    fn ae209_m3_forcing_is_recorded_in_the_causal_ledger_under_its_own_cause() {
        let reg = ObservableRegistry::reference_default();
        let mut forced = treatment_manifest();
        let f = removal_forcing(1, 1200, 3000);
        let cause = f.cause_id;
        forced.exotic_interventions = vec![f];

        let res = run_manifest_seed::<ReferenceEvolutionWorld>(
            &forced,
            &reg,
            forced.seeds[0],
            None,
            None,
        );
        // A RemoveSource is recorded as a source SUPPRESSION (a counterfactual), not as MU movement —
        // see the D4/D2 contract. Its target is therefore `exotic.source_suppressed`.
        let forcing_effects: Vec<&crate::core::causal::EffectRecord> = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.target == "exotic.source_suppressed")
            .collect();
        assert!(
            !forcing_effects.is_empty(),
            "a forcing must leave causal records"
        );
        // Rooted at the FORCING's own cause id — distinct from the world-law cause (AE-S12).
        assert!(forcing_effects.iter().all(|e| e.cause_id == cause));
        assert_ne!(cause, CAUSE_EXOTIC_WORLD_LAW);
    }

    #[test]
    fn ae209_m3_forced_run_replays_deterministically() {
        let reg = ObservableRegistry::reference_default();
        let mut forced = treatment_manifest();
        forced.exotic_interventions = vec![removal_forcing(1, 1200, 3000)];
        let a = run_manifest_seed::<ReferenceEvolutionWorld>(
            &forced,
            &reg,
            forced.seeds[0],
            None,
            None,
        );
        let b = run_manifest_seed::<ReferenceEvolutionWorld>(
            &forced,
            &reg,
            forced.seeds[0],
            None,
            None,
        );
        assert_eq!(a.final_checksum, b.final_checksum);
        assert_eq!(a.final_observables, b.final_observables);
        assert_eq!(a.ledger.len(), b.ledger.len());
    }

    #[test]
    fn ae209_m3_invalid_forcing_fails_the_manifest_structurally() {
        let reg = ObservableRegistry::reference_default();
        let mut bad = treatment_manifest();
        let mut f = removal_forcing(1, 1200, 3000);
        f.amount = f32::NAN;
        bad.exotic_interventions = vec![f];
        assert!(bad.validate(&reg).is_err());

        // Duplicate forcing ids are rejected too.
        let mut dup = treatment_manifest();
        dup.exotic_interventions = vec![removal_forcing(1, 1200, 10), removal_forcing(1, 1300, 10)];
        assert!(dup.validate(&reg).is_err());

        // A forcing declared on a baseline (no exotic law) world is rejected — there is no field to
        // force, so silently ignoring it would be a lie about the declared input.
        let mut no_law = treatment_manifest();
        no_law.laws = WorldLawSet::baseline();
        no_law.exotic_interventions = vec![removal_forcing(1, 1200, 10)];
        assert!(no_law.validate(&reg).is_err());
    }

    #[test]
    fn ae_m4_treatment_produces_a_field_and_a_closed_mu_ledger() {
        let reg = ObservableRegistry::reference_default();
        let m = treatment_manifest();
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, m.seeds[0], None, None);
        assert!(res.status.is_completed());

        // A measurable spatial field exists.
        let density = res.observable("exotic.density_total").unwrap();
        assert!(density > 0.0, "treatment must have a measurable MU field");

        // The MU ledger closes within a generous local test tolerance (the final policy value is
        // AE-006, deferred to the user).
        let budget = res.exotic_budget.expect("treatment has an MU budget");
        assert!(
            budget.balance_error().abs() / budget.throughput() < 1e-4,
            "MU budget must close: error {}, throughput {}",
            budget.balance_error(),
            budget.throughput()
        );
        assert!(budget.sourced > 0.0, "renewable source injected MU");

        // The causal ledger carries an exotic chain rooted at the world law (AE-S12 partial).
        let last = (res.ledger.len() - 1) as u32;
        // Find the last exotic effect and confirm its root cause is the world law.
        let exotic_effects: Vec<u32> = res
            .ledger
            .all()
            .iter()
            .filter(|e| e.target == "exotic.density_total")
            .map(|e| e.effect_id)
            .collect();
        assert!(!exotic_effects.is_empty(), "field changes are recorded");
        let tip = *exotic_effects.last().unwrap();
        assert_eq!(
            res.ledger.root_cause(tip),
            Some(CAUSE_EXOTIC_WORLD_LAW),
            "the exotic field chain roots at the world-law cause"
        );
        let _ = last;
    }

    #[test]
    fn ae_m4_genesis_fork_isolates_the_exotic_factor_and_leaves_eu_unchanged() {
        let reg = ObservableRegistry::reference_default();
        let treatment = treatment_manifest();
        let report = genesis_fork::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("fork validates");

        // The ONLY declared factor is the exotic law (AE-S08).
        assert_eq!(
            report.declared_factors,
            vec!["laws.exotic_energy".to_string()]
        );

        // The EU trajectory is IDENTICAL between control and treatment — the exotic field changed no
        // biomass (AE-S05). Closed-EU pools do not jump because of exotic bookkeeping.
        for name in ["plants", "herbivores", "predators", "detritus", "npp"] {
            let d = report.delta_of(name).unwrap();
            assert!(
                d.abs() < 1e-12,
                "{name} EU delta must be ~0 (exotic must not touch biomass), was {d}"
            );
        }

        // The control has no MU field; the treatment has a positive one.
        assert!(report.control.exotic_budget.is_none());
        assert!(report.control.observable("exotic.density_total").is_none());
        assert!(report.treatment.observable("exotic.density_total").unwrap() > 0.0);

        // The control's checksum equals the treatment's inner EU state — provable because the EU
        // observables all match and the control checksum is the pure-EU fingerprint.
        assert!(report.control.status.is_completed());
        assert!(report.treatment.status.is_completed());
    }

    #[test]
    fn ae_m4_either_branch_replays_deterministically() {
        let reg = ObservableRegistry::reference_default();
        let treatment = treatment_manifest();
        let a = run_manifest_seed::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            treatment.seeds[0],
            None,
            None,
        );
        let b = run_manifest_seed::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            treatment.seeds[0],
            None,
            None,
        );
        assert_eq!(a.final_checksum, b.final_checksum);
        assert_eq!(a.final_observables, b.final_observables);
        assert_eq!(a.ledger.len(), b.ledger.len());

        // The control branch is likewise deterministic.
        let control = treatment.control_variant();
        let ca = run_manifest_seed::<ReferenceEvolutionWorld>(
            &control,
            &reg,
            control.seeds[0],
            None,
            None,
        );
        let cb = run_manifest_seed::<ReferenceEvolutionWorld>(
            &control,
            &reg,
            control.seeds[0],
            None,
            None,
        );
        assert_eq!(ca.final_checksum, cb.final_checksum);
    }
}
