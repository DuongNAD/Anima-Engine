//! Control/treatment forks: from genesis (AE-106 / AE-S08) and from a checkpoint (AE-107 / AE-S09).

use super::*;

// ---- Genesis fork (AE-106 / AE-S08) ----------------------------------------------------------

/// A control/treatment pair from one treatment manifest, with the declared factor difference and the
/// per-observable final delta.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForkReport {
    pub control: RunResult,
    pub treatment: RunResult,
    /// The manifest paths that actually differ (⊆ the allowlist).
    pub declared_factors: Vec<String>,
    pub delta: Vec<TargetDelta>,
}

impl ForkReport {
    /// The final delta (treatment − control) for a named observable, if present in both.
    pub fn delta_of(&self, target: &str) -> Option<f64> {
        self.delta
            .iter()
            .find(|d| d.target == target)
            .map(|d| d.delta)
    }
}

/// Compare only observables measured on both sides.
///
/// A missing control observation is not a measured zero. Treatment-only channels remain available
/// in the treatment [`RunResult`], but cannot support a control/treatment delta.
pub(super) fn matched_final_deltas(control: &RunResult, treatment: &RunResult) -> Vec<TargetDelta> {
    treatment
        .final_observables
        .iter()
        .filter_map(|(target, treatment_final)| {
            control.observable(target).map(|control_final| TargetDelta {
                target: target.clone(),
                control_final,
                treatment_final: *treatment_final,
                delta: treatment_final - control_final,
            })
        })
        .collect()
}

/// Run a genesis fork: the `treatment` manifest and its [`control_variant`](ExperimentManifest::control_variant)
/// under the same (first) seed, after checking their difference lies entirely within `allowed`
/// (AE-S08). Both runs are genesis runs (no parent); only the declared factor differs.
pub fn genesis_fork<M: ExperimentModel>(
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    allowed: &FactorDiff,
) -> Result<ForkReport, ExperimentError> {
    // Preflight: the registry itself must be valid BEFORE any manifest check, model construction or
    // RNG work — a malformed catalogue would otherwise silently shape both runs' result metadata.
    registry.validate()?;
    treatment.validate(registry)?;
    let control = treatment.control_variant();
    control.validate(registry)?;
    let declared_factors = allowed.validate(&control, treatment)?;

    let seed = treatment.seeds[0];
    let control_res = run_manifest_seed::<M>(&control, registry, seed, None, None);
    let treatment_res = run_manifest_seed::<M>(treatment, registry, seed, None, None);

    // Per-observable final delta (treatment − control), matched by name.
    let delta = matched_final_deltas(&control_res, &treatment_res);

    Ok(ForkReport {
        control: control_res,
        treatment: treatment_res,
        declared_factors,
        delta,
    })
}

// ---- Checkpoint fork (AE-107 / AE-S09) -------------------------------------------------------

/// A mid-run checkpoint fork: a shared prefix over ticks `1..=fork_tick`, then two branches continued
/// from the **same in-memory snapshot** (captured *after* `fork_tick` was processed) — `control` (base
/// interventions) and `treatment` (base + extra interventions), each processing ticks
/// `fork_tick+1 ..= duration_ticks`. Both branches carry `parent_run_id = prefix` and the same
/// `fork_tick`, and start from byte-identical post-`fork_tick` state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointForkReport {
    /// The shared history run over ticks `1..=fork_tick` (the checkpoint is its end state).
    pub prefix: RunResult,
    pub control: RunResult,
    pub treatment: RunResult,
    pub fork_tick: u64,
    /// The treatment-only interventions applied after the fork, **fully structured** (empty if none).
    /// This is the authoritative, lossless declaration of what differs between the branches — the
    /// human-readable [`declared_factors`](Self::declared_factors) strings are a convenience summary
    /// and must never be parsed.
    pub treatment_extra: Vec<InterventionCommand>,
    /// The treatment-only **exotic source forcings** (AE-209) applied after the fork, fully
    /// structured (empty if none). Like `treatment_extra`, this is the authoritative declaration —
    /// `declared_factors` is only a display summary.
    #[serde(default)]
    pub treatment_extra_exotic: Vec<ExoticIntervention>,
    /// A human-readable summary of `treatment_extra` / `treatment_extra_exotic`. Display only;
    /// never machine-parsed.
    pub declared_factors: Vec<String>,
    pub delta: Vec<TargetDelta>,
}

impl CheckpointForkReport {
    pub fn delta_of(&self, target: &str) -> Option<f64> {
        self.delta
            .iter()
            .find(|d| d.target == target)
            .map(|d| d.delta)
    }
}

/// Run a checkpoint fork (AE-107).
///
/// **Exact tick semantics.** The shared prefix processes ticks `1..=fork_tick` (the clock advances
/// exactly `fork_tick` times); the snapshot and the live RNG are captured **after** `fork_tick` has
/// been processed. Each branch then continues, processing ticks `fork_tick+1 ..= duration_ticks`. So
/// the first post-fork tick a branch can act on is `fork_tick + 1`, and a `treatment_extra`
/// intervention can only affect the treatment branch if its start lies in the half-open-below window
/// `(fork_tick, duration_ticks]` — i.e. `fork_tick < start_tick <= duration_ticks`. An intervention
/// starting at or before `fork_tick` (it belongs to the shared prefix, which used only the base
/// interventions) or after `duration_ticks` (the run has ended) is **rejected** rather than declared
/// as an unapplied factor.
///
/// Because the snapshot + a cloned RNG fully capture the state, a `control` branch continued with
/// identical inputs reproduces an uninterrupted run **bit-for-bit** (AE-S09), and both branches are
/// byte-identical up to the fork; only `treatment_extra` makes them diverge afterward. This never
/// re-simulates the prefix per branch. If the shared prefix fails before reaching `fork_tick`, a
/// structured [`ExperimentError::CheckpointPrefixFailed`] is returned (no branch is continued from a
/// partial state).
pub fn checkpoint_fork<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    fork_tick: u64,
    treatment_extra: &[InterventionCommand],
) -> Result<CheckpointForkReport, ExperimentError> {
    checkpoint_fork_with_exotic::<M>(manifest, registry, seed, fork_tick, treatment_extra, &[])
}

/// Run a checkpoint fork that may additionally apply **treatment-only exotic source forcings**
/// (AE-209) after the fork — the "add / remove / pulse the source at generation G" experiment.
///
/// This is a superset of [`checkpoint_fork`], which delegates here with no exotic extras, so existing
/// callers are unaffected.
///
/// On top of the legacy checks, every exotic extra must (all validated **before** the model, RNG or
/// snapshot are built): be structurally valid and grid-applicable, be uniquely identified within the
/// extras and against the manifest's base exotic forcings, act on a world that actually has an exotic
/// field, and have an **ecology firing tick strictly after `fork_tick` and within `duration_ticks`**
/// — otherwise it could never apply post-fork and declaring it would be misleading.
///
/// The treatment branch is reconfigured from the checkpoint with the *effective* forcing set
/// (base + extras) while the control keeps the base set; the prefix is never re-simulated.
pub fn checkpoint_fork_with_exotic<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    fork_tick: u64,
    treatment_extra: &[InterventionCommand],
    treatment_extra_exotic: &[ExoticIntervention],
) -> Result<CheckpointForkReport, ExperimentError> {
    registry.validate()?;
    manifest.validate(registry)?;
    if !manifest.seeds.contains(&seed) {
        return Err(ExperimentError::SeedNotInManifest { seed });
    }
    if fork_tick == 0 || fork_tick >= manifest.duration_ticks {
        return Err(ExperimentError::OutOfRange {
            field: "fork_tick".into(),
            value: fork_tick as f64,
            min: 1.0,
            max: (manifest.duration_ticks.saturating_sub(1)) as f64,
        });
    }
    // The combined base + extra intervention set must respect the manifest ceiling.
    let combined = manifest
        .interventions
        .len()
        .saturating_add(treatment_extra.len());
    if combined > MAX_INTERVENTIONS {
        return Err(ExperimentError::ResourceLimit {
            field: "interventions (base + treatment_extra)".into(),
            limit: MAX_INTERVENTIONS,
            found: combined,
        });
    }
    // Every treatment-only intervention must be individually valid (same manifest-path helper the
    // manifest itself uses), uniquely identified within the extras AND against the base set, and able
    // to affect a post-fork tick in `(fork_tick, duration_ticks]`; otherwise declaring it as the
    // fork's factor is misleading.
    for (i, cmd) in treatment_extra.iter().enumerate() {
        // Fork-window checks run FIRST so the checkpoint-specific `InapplicableIntervention` (which
        // explains *why* it cannot apply to this fork) wins over the generic manifest-path
        // "never active in the run" error, which would otherwise mask it for a late start_tick.
        if cmd.start_tick <= fork_tick {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "start_tick {} is at or before fork_tick {fork_tick}; it belongs to the shared \
                     prefix and would never be applied in the branch",
                    cmd.start_tick
                ),
            });
        }
        if cmd.start_tick > manifest.duration_ticks {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "start_tick {} is after duration_ticks {}; it would never be applied",
                    cmd.start_tick, manifest.duration_ticks
                ),
            });
        }
        validate_intervention(cmd, manifest.duration_ticks)?;
        if manifest.interventions.iter().any(|b| b.id == cmd.id) {
            return Err(ExperimentError::DuplicateId {
                context: "intervention (treatment_extra collides with a base intervention)".into(),
                id: cmd.id.to_string(),
            });
        }
        for other in &treatment_extra[i + 1..] {
            if other.id == cmd.id {
                return Err(ExperimentError::DuplicateId {
                    context: "intervention (duplicate within treatment_extra)".into(),
                    id: cmd.id.to_string(),
                });
            }
        }
    }

    // Exotic treatment extras (AE-209 checkpoint channel): validated fully BEFORE any model/RNG or
    // snapshot work.
    if !treatment_extra_exotic.is_empty() && manifest.laws.exotic_energy.is_none() {
        return Err(ExperimentError::InvalidExoticIntervention {
            id: treatment_extra_exotic[0].id,
            reason: "exotic treatment extras declared but laws.exotic_energy is None".into(),
        });
    }
    let combined_exotic = manifest
        .exotic_interventions
        .len()
        .saturating_add(treatment_extra_exotic.len());
    if combined_exotic > MAX_INTERVENTIONS {
        return Err(ExperimentError::ResourceLimit {
            field: "exotic_interventions (base + treatment_extra_exotic)".into(),
            limit: MAX_INTERVENTIONS,
            found: combined_exotic,
        });
    }
    for (i, cmd) in treatment_extra_exotic.iter().enumerate() {
        // Post-fork applicability: there must be an ecology firing STRICTLY after `fork_tick` and
        // within the run, inside the command's own window.
        let end_exclusive = cmd
            .start_tick
            .checked_add(cmd.effective_duration())
            .ok_or_else(|| ExperimentError::InvalidExoticIntervention {
                id: cmd.id,
                reason: "start_tick + duration overflows u64".into(),
            })?;
        let lo = cmd.start_tick.max(fork_tick + 1);
        let first_firing = lo.div_ceil(ECOLOGY_PERIOD).saturating_mul(ECOLOGY_PERIOD);
        if first_firing >= end_exclusive
            || first_firing > manifest.duration_ticks
            || first_firing <= fork_tick
        {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "exotic forcing has no ecology firing (period {ECOLOGY_PERIOD}) strictly after \
                     fork_tick {fork_tick} inside its window [{}, {end_exclusive}) and within \
                     duration_ticks {}",
                    cmd.start_tick, manifest.duration_ticks
                ),
            });
        }
        cmd.validate(manifest.duration_ticks)
            .map_err(|reason| ExperimentError::InvalidExoticIntervention { id: cmd.id, reason })?;
        if manifest.exotic_interventions.iter().any(|b| b.id == cmd.id) {
            return Err(ExperimentError::DuplicateId {
                context: "exotic_intervention (treatment extra collides with a base forcing)"
                    .into(),
                id: cmd.id.to_string(),
            });
        }
        for other in &treatment_extra_exotic[i + 1..] {
            if other.id == cmd.id {
                return Err(ExperimentError::DuplicateId {
                    context: "exotic_intervention (duplicate within treatment_extra_exotic)".into(),
                    id: cmd.id.to_string(),
                });
            }
        }
    }

    // The **effective treatment input** is the base manifest with the extras appended. It is
    // validated in its own right (so the combined set must itself be a legal manifest) and it —
    // not the base manifest — is what the treatment branch's provenance fingerprints, so a
    // treatment run is independently addressable and replayable. With no extras it is byte-identical
    // to the base manifest, so both branches then share one fingerprint.
    let effective_treatment = {
        let mut m = manifest.clone();
        m.interventions.extend_from_slice(treatment_extra);
        m.exotic_interventions
            .extend_from_slice(treatment_extra_exotic);
        m
    };
    effective_treatment.validate(registry)?;
    // The reference grid is fixed and known before any model or RNG exists, so reject a spatially
    // inapplicable base/extra forcing here. This keeps checkpoint preflight model-independent:
    // even a custom `ExperimentModel` cannot silently accept an out-of-grid treatment factor.
    for cmd in &effective_treatment.exotic_interventions {
        cmd.validate_grid_applicability(REFERENCE_FIELD_DIM, REFERENCE_FIELD_DIM)
            .map_err(|reason| ExperimentError::InvalidExoticIntervention { id: cmd.id, reason })?;
    }

    let mfp = manifest.fingerprint();
    let treatment_fp = effective_treatment.fingerprint();
    let registry_fingerprint = registry.fingerprint();
    let model_version = M::model_version().to_string();
    let build_id = build_id();
    let base_id = run_identity_base(
        &manifest.experiment_id,
        mfp,
        registry_fingerprint,
        &model_version,
        &build_id,
        seed,
    );
    let treatment_base_id = run_identity_base(
        &manifest.experiment_id,
        treatment_fp,
        registry_fingerprint,
        &model_version,
        &build_id,
        seed,
    );
    let prefix_id = format!("{base_id}#prefix@{fork_tick}");
    // `fingerprint` selects which manifest's identity a branch carries: the prefix and control run
    // under the base manifest, the treatment under the effective treatment manifest.
    let make_prov = |run_id: String,
                     parent: Option<String>,
                     fork: Option<u64>,
                     fingerprint: u64|
     -> RunProvenance {
        RunProvenance {
            experiment_id: manifest.experiment_id.clone(),
            run_id,
            parent_run_id: parent,
            fork_tick: fork,
            seed,
            manifest_fingerprint: fingerprint,
            // The world laws are identical across branches (a checkpoint fork never changes a law —
            // that would be a genesis fork), so the law fingerprint is shared.
            law_fingerprint: manifest.laws.fingerprint(),
            registry_fingerprint,
            model_version: model_version.clone(),
            build_id: build_id.clone(),
            fitness_rule: declared_fitness_rule(&manifest.initial_conditions),
            terrain_source: None,
            world_identity: None,
        }
    };

    let grid = (REFERENCE_FIELD_DIM, REFERENCE_FIELD_DIM);
    let base_queue = InterventionQueue::new(manifest.interventions.clone());

    // Shared prefix: process ticks 1..=fork_tick (the clock advances exactly `fork_tick` times).
    let mut model = M::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        seed,
        grid,
        manifest.duration_ticks,
    )?;
    let parent_terrain_source = model.terrain_source().map(str::to_owned);
    let parent_world_identity = model.world_identity();
    let stamp_parent_terrain = |mut p: RunProvenance| -> RunProvenance {
        p.terrain_source = parent_terrain_source.clone();
        p.world_identity = parent_world_identity;
        p
    };
    let mut rng = StdRng::seed_from_u64(seed);
    let mut clock = SimClock::new();
    let mut prefix_ledger = CausalLedger::new();
    let (prefix_series, prefix_failure) = drive(
        &mut model,
        &mut rng,
        &mut clock,
        &mut prefix_ledger,
        &base_queue,
        fork_tick,
        manifest.sample_period,
    );

    // If the prefix diverged before reaching the checkpoint, the clock did NOT reach `fork_tick`, so
    // `remaining` would be wrong and the branches would resume from a partial/failed state. Fail
    // structurally instead of continuing.
    if let Some(failure) = prefix_failure {
        return Err(ExperimentError::CheckpointPrefixFailed {
            tick: failure.tick,
            reason: failure.reason,
        });
    }

    // Capture the checkpoint: model state + the live RNG (cloned, so no RNG serialization is needed
    // and both branches share the exact same stream from the fork).
    let snapshot = model.snapshot();
    let rng_at_fork = rng.clone();
    let clock_at_fork = clock; // SimClock is Copy
    let remaining = manifest.duration_ticks - fork_tick;

    // The prefix reached the checkpoint successfully (any failure returned above).
    let prefix = assemble_result(
        stamp_parent_terrain(make_prov(prefix_id.clone(), None, None, mfp)),
        &model,
        registry,
        manifest.laws.exotic_energy.as_ref().map(|law| &law.unit),
        prefix_series,
        prefix_ledger,
        clock_at_fork.tick,
        None,
    );

    // Continue a branch from the checkpoint under `interventions`, stamping it with the manifest
    // fingerprint that actually describes that branch's input.
    let continue_branch = |run_id: String,
                           interventions: Vec<InterventionCommand>,
                           forcings: &[ExoticIntervention],
                           fingerprint: u64| {
        let mut branch_model = M::from_snapshot(&snapshot)?;
        // Each branch gets its OWN declared forcing set: the control keeps the base forcings,
        // the treatment gets base + exotic extras. The snapshot itself is shared and untouched.
        branch_model.reconfigure_forcings(forcings, manifest.duration_ticks)?;
        let mut branch_rng = rng_at_fork.clone();
        let mut branch_clock = clock_at_fork;
        let mut branch_ledger = CausalLedger::new();
        let queue = InterventionQueue::new(interventions);
        let (series, failure) = drive(
            &mut branch_model,
            &mut branch_rng,
            &mut branch_clock,
            &mut branch_ledger,
            &queue,
            remaining,
            manifest.sample_period,
        );
        Ok::<RunResult, ExperimentError>(finish_drive(
            stamp_parent_terrain(make_prov(
                run_id,
                Some(prefix_id.clone()),
                Some(fork_tick),
                fingerprint,
            )),
            &branch_model,
            registry,
            manifest.laws.exotic_energy.as_ref().map(|law| &law.unit),
            series,
            branch_ledger,
            branch_clock.tick,
            failure,
        ))
    };

    let control = continue_branch(
        format!("{base_id}#control@{fork_tick}"),
        manifest.interventions.clone(),
        &manifest.exotic_interventions,
        mfp,
    )?;
    let treatment = continue_branch(
        format!("{treatment_base_id}#treatment@{fork_tick}"),
        effective_treatment.interventions.clone(),
        &effective_treatment.exotic_interventions,
        treatment_fp,
    )?;

    let declared_factors: Vec<String> = treatment_extra
        .iter()
        .map(|c| format!("intervention:{:?}@{}", c.kind, c.start_tick))
        .chain(
            treatment_extra_exotic
                .iter()
                .map(|c| format!("exotic:{:?}@{}", c.kind, c.start_tick)),
        )
        .collect();

    let delta = matched_final_deltas(&control, &treatment);

    Ok(CheckpointForkReport {
        prefix,
        control,
        treatment,
        fork_tick,
        treatment_extra: treatment_extra.to_vec(),
        treatment_extra_exotic: treatment_extra_exotic.to_vec(),
        declared_factors,
        delta,
    })
}
