//! `FactorDiff` (AE-103 / AE-S08): the guard that a control/treatment pair differs only in declared
//! factors.

use super::*;

// ---- Factor diff (AE-103) --------------------------------------------------------------------

/// The allowlist of manifest paths a control/treatment pair is permitted to differ at. Any *other*
/// difference is rejected (AE-S08) so a comparison can never hide an uncontrolled variable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactorDiff {
    pub allowed_paths: Vec<String>,
}

impl FactorDiff {
    /// The genesis-fork allowlist for the exotic regime: the exotic-energy law and the runtime
    /// forcings that act on its field. These are one declared factor — removing the law necessarily
    /// removes its forcings (see [`ExperimentManifest::control_variant`]) — so both paths are
    /// allowed. Every other manifest path must still match exactly (AE-S08).
    pub fn genesis_exotic() -> Self {
        Self {
            allowed_paths: vec![
                "laws.exotic_energy".to_string(),
                "exotic_interventions".to_string(),
            ],
        }
    }

    /// The set of manifest paths at which `control` and `treatment` differ (sorted, deduped). Labels
    /// (`name`, `experiment_id`) are excluded — they are not experimental factors.
    pub fn diff_paths(control: &ExperimentManifest, treatment: &ExperimentManifest) -> Vec<String> {
        let mut paths = Vec::new();
        if control.observer != treatment.observer {
            paths.push("observer".to_string());
        }
        if control.island != treatment.island {
            paths.push("island".to_string());
        }
        if control.world_identity != treatment.world_identity {
            paths.push("world_identity".to_string());
        }
        if control.laws.schema_version != treatment.laws.schema_version {
            paths.push("laws.schema_version".to_string());
        }
        if control.laws.baseline_energy != treatment.laws.baseline_energy {
            paths.push("laws.baseline_energy".to_string());
        }
        if control.laws.exotic_energy != treatment.laws.exotic_energy {
            paths.push("laws.exotic_energy".to_string());
        }
        if control.initial_conditions != treatment.initial_conditions {
            paths.push("initial_conditions".to_string());
        }
        // Interventions compared as sets (order-independent).
        if !interventions_equal(&control.interventions, &treatment.interventions) {
            paths.push("interventions".to_string());
        }
        if !exotic_interventions_equal(
            &control.exotic_interventions,
            &treatment.exotic_interventions,
        ) {
            paths.push("exotic_interventions".to_string());
        }
        if !seeds_equal(&control.seeds, &treatment.seeds) {
            paths.push("seeds".to_string());
        }
        if control.duration_ticks != treatment.duration_ticks {
            paths.push("duration_ticks".to_string());
        }
        if control.sample_period != treatment.sample_period {
            paths.push("sample_period".to_string());
        }
        if !observables_equal(&control.observable_ids, &treatment.observable_ids) {
            paths.push("observables".to_string());
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// Confirm the actual difference between `control` and `treatment` lies entirely within the
    /// allowlist; return the differing paths on success, or the first undeclared difference as a
    /// structured error.
    pub fn validate(
        &self,
        control: &ExperimentManifest,
        treatment: &ExperimentManifest,
    ) -> Result<Vec<String>, ExperimentError> {
        let diffs = Self::diff_paths(control, treatment);
        for p in &diffs {
            if !self.allowed_paths.contains(p) {
                return Err(ExperimentError::UndeclaredFactorDifference { path: p.clone() });
            }
        }
        Ok(diffs)
    }
}

pub(super) fn interventions_equal(a: &[InterventionCommand], b: &[InterventionCommand]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<&InterventionCommand> = a.iter().collect();
    let mut b: Vec<&InterventionCommand> = b.iter().collect();
    a.sort_by_key(|c| (c.start_tick, c.id));
    b.sort_by_key(|c| (c.start_tick, c.id));
    a == b
}

pub(super) fn exotic_interventions_equal(
    a: &[ExoticIntervention],
    b: &[ExoticIntervention],
) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<&ExoticIntervention> = a.iter().collect();
    let mut b: Vec<&ExoticIntervention> = b.iter().collect();
    a.sort_by_key(|c| (c.start_tick, c.id));
    b.sort_by_key(|c| (c.start_tick, c.id));
    a == b
}

pub(super) fn seeds_equal(a: &[u64], b: &[u64]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

pub(super) fn observables_equal(a: &[String], b: &[String]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}
