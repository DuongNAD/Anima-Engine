use bevy_ecs::prelude::*;

#[derive(Component, Clone, Debug)]
pub struct CpgOscillator {
    pub phase: f32,
    pub frequency: f32,
    pub amplitude: f32,
    pub output: f32,
}

impl CpgOscillator {
    pub fn new(frequency: f32, amplitude: f32) -> Self {
        Self {
            phase: 0.0,
            frequency,
            amplitude,
            output: 0.0,
        }
    }

    pub fn tick(&mut self, delta_time: f32) {
        let two_pi = 2.0 * std::f32::consts::PI;
        self.phase = (self.phase + two_pi * self.frequency * delta_time).rem_euclid(two_pi);
        self.output = self.amplitude * self.phase.sin();
    }
}

use crate::ai::hrrl::HomeostaticState;
use crate::core::ecs::ParentAgent;

pub fn update_cpg_system(
    mut query: Query<(
        &mut CpgOscillator,
        Option<&ParentAgent>,
        Option<&HomeostaticState>,
    )>,
    agent_query: Query<&HomeostaticState>,
    time_step: Res<TimeStep>,
) {
    for (mut osc, parent_agent, local_homeo) in query.iter_mut() {
        let energy = if let Some(parent) = parent_agent {
            agent_query.get(parent.0).map(|h| h.energy).ok()
        } else {
            local_homeo.map(|h| h.energy)
        };

        if let Some(energy) = energy {
            if energy <= 0.0 {
                osc.output = 0.0;
                continue;
            }
        }
        osc.tick(time_step.0);
    }
}

#[derive(Resource)]
pub struct TimeStep(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_wraps_phase_into_zero_to_two_pi_across_multi_period_and_negative_steps() {
        let two_pi = 2.0 * std::f32::consts::PI;

        // Multi-period positive advance (3.25 full turns in one step): single subtraction of 2*pi
        // would leave phase > 2*pi, whereas rem_euclid wraps into [0, 2*pi).
        let mut osc = CpgOscillator::new(3.25, 1.0);
        osc.tick(1.0);
        assert!(
            (0.0..two_pi).contains(&osc.phase),
            "phase after multi-period step must stay in [0, 2*pi), got {}",
            osc.phase
        );
        assert!((osc.phase - 0.25 * two_pi).abs() < 1e-5);
        assert!((osc.output - 1.0).abs() < 1e-5);

        // Negative frequency advance: single `if phase > 2*pi` left phase negative, whereas
        // rem_euclid wraps negative phases into [0, 2*pi).
        let mut neg = CpgOscillator::new(-0.25, 1.0);
        neg.tick(1.0);
        assert!(
            (0.0..two_pi).contains(&neg.phase),
            "phase after negative frequency step must stay in [0, 2*pi), got {}",
            neg.phase
        );
        assert!((neg.phase - 0.75 * two_pi).abs() < 1e-5);
        assert!((neg.output - (-1.0)).abs() < 1e-5);
    }
}
