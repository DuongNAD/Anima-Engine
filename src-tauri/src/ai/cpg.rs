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
        self.phase += 2.0 * std::f32::consts::PI * self.frequency * delta_time;
        if self.phase > 2.0 * std::f32::consts::PI {
            self.phase -= 2.0 * std::f32::consts::PI;
        }
        self.output = self.amplitude * self.phase.sin();
    }
}

use crate::core::ecs::ParentAgent;
use crate::ai::hrrl::HomeostaticState;

pub fn update_cpg_system(
    mut query: Query<(&mut CpgOscillator, Option<&ParentAgent>, Option<&HomeostaticState>)>,
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
