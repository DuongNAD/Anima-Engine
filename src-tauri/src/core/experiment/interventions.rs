//! Manifest-path intervention validation (AE-103).

use super::*;

// ---- Manifest-path intervention validation (AE-103) ------------------------------------------

/// Validate one [`InterventionCommand`] for use on the **manifest path**, where inputs may be
/// imported/untrusted. The legacy [`crate::core::intervention`] module is intentionally left
/// unchanged (its own scenario tests keep the permissive behaviour); this is the stricter gate that
/// experiment manifests and checkpoint forks share.
///
/// `run_ticks` is the run's `duration_ticks`; the simulated tick domain is `1..=run_ticks`. Rejects:
/// non-finite or negative `intensity`, invalid `Radius` (non-finite centre/radius, non-positive
/// radius), inverted `Rect` bounds, a `start_tick + effective_duration` that overflows `u64`, and a
/// schedule whose active window never intersects the run.
pub fn validate_intervention(
    cmd: &InterventionCommand,
    run_ticks: u64,
) -> Result<(), ExperimentError> {
    // Intensity: finite and non-negative (direction is carried by `signed_negative`, not the sign).
    if !cmd.intensity.is_finite() {
        return Err(ExperimentError::NotFinite {
            field: format!("intervention {} intensity", cmd.id),
        });
    }
    if cmd.intensity < 0.0 {
        return Err(ExperimentError::OutOfRange {
            field: format!("intervention {} intensity", cmd.id),
            value: cmd.intensity as f64,
            min: 0.0,
            max: f64::from(f32::MAX),
        });
    }

    // Geometry.
    match cmd.region {
        Region::Global | Region::Cell { .. } => {}
        Region::Rect {
            min_x,
            min_y,
            max_x,
            max_y,
        } => {
            if min_x > max_x || min_y > max_y {
                return Err(ExperimentError::OutOfRange {
                    field: format!("intervention {} region Rect bounds", cmd.id),
                    value: min_x as f64,
                    min: 0.0,
                    max: max_x as f64,
                });
            }
        }
        Region::Radius { cx, cy, r } => {
            if !cx.is_finite() || !cy.is_finite() || !r.is_finite() {
                return Err(ExperimentError::NotFinite {
                    field: format!("intervention {} region Radius", cmd.id),
                });
            }
            if r <= 0.0 {
                return Err(ExperimentError::OutOfRange {
                    field: format!("intervention {} region radius", cmd.id),
                    value: r as f64,
                    min: f64::MIN_POSITIVE,
                    max: f64::from(f32::MAX),
                });
            }
        }
    }

    // Schedule: the active window is `[start_tick, start_tick + effective_duration)`. Compute its end
    // with a checked add so a crafted manifest cannot wrap around.
    let end_exclusive = cmd
        .start_tick
        .checked_add(cmd.effective_duration())
        .ok_or_else(|| ExperimentError::OutOfRange {
            field: format!("intervention {} start_tick + duration_ticks", cmd.id),
            value: cmd.start_tick as f64,
            min: 0.0,
            max: u64::MAX as f64,
        })?;

    // The run simulates ticks 1..=run_ticks. An intervention that can never fire inside that window
    // would be a silently-inert declared factor.
    let intersects = cmd.start_tick <= run_ticks && end_exclusive > 1;
    if !intersects {
        return Err(ExperimentError::OutOfRange {
            field: format!("intervention {} active window", cmd.id),
            value: cmd.start_tick as f64,
            min: 1.0,
            max: run_ticks as f64,
        });
    }

    // Specific variant validation.
    match &cmd.kind {
        crate::core::intervention::InterventionKind::GeneticInjection {
            trait_modifications,
            ..
        } => {
            for tm in trait_modifications {
                match tm {
                    crate::core::intervention::TraitMod::ScaleMorphology(f)
                    | crate::core::intervention::TraitMod::MassMultiplier(f)
                    | crate::core::intervention::TraitMod::MetabolicEfficiencyDelta(f)
                    | crate::core::intervention::TraitMod::SensoryRangeMultiplier(f) => {
                        if !f.is_finite() {
                            return Err(ExperimentError::NotFinite {
                                field: format!("intervention {} trait modification", cmd.id),
                            });
                        }
                    }
                    crate::core::intervention::TraitMod::BrainParameterShift {
                        bias_delta, ..
                    } => {
                        if !bias_delta.is_finite() {
                            return Err(ExperimentError::NotFinite {
                                field: format!("intervention {} brain bias delta", cmd.id),
                            });
                        }
                    }
                }
            }
        }
        crate::core::intervention::InterventionKind::ToxicityPlume {
            center_x,
            center_y,
            radius,
            toxicity_level,
            decay_rate,
        } => {
            if !center_x.is_finite()
                || !center_y.is_finite()
                || !radius.is_finite()
                || !toxicity_level.is_finite()
                || !decay_rate.is_finite()
            {
                return Err(ExperimentError::NotFinite {
                    field: format!("intervention {} toxicity plume parameters", cmd.id),
                });
            }
            if *radius <= 0.0 {
                return Err(ExperimentError::OutOfRange {
                    field: format!("intervention {} toxicity plume radius", cmd.id),
                    value: *radius as f64,
                    min: f64::MIN_POSITIVE,
                    max: f64::from(f32::MAX),
                });
            }
        }
        crate::core::intervention::InterventionKind::PathogenTransmission {
            transmission_rate,
            mortality_multiplier,
            ..
        } => {
            if !transmission_rate.is_finite() || !mortality_multiplier.is_finite() {
                return Err(ExperimentError::NotFinite {
                    field: format!("intervention {} pathogen parameters", cmd.id),
                });
            }
        }
        _ => {}
    }

    Ok(())
}
