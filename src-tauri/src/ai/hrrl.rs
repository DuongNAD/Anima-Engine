use bevy_ecs::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct HomeostaticState {
    pub energy: f32,
    pub energy_target: f32,
    pub hydration: f32,
    pub hydration_target: f32,
    pub temperature: f32,
    pub temp_target: f32,
    pub previous_deviation: f32,
}

impl HomeostaticState {
    // Tính tổng độ lệch sinh lý (Homeostatic deviation)
    pub fn compute_deviation(&self) -> f32 {
        0.0001 * (self.energy - self.energy_target).powi(2)
            + 0.0001 * (self.hydration - self.hydration_target).powi(2)
            + 0.0156 * (self.temperature - self.temp_target).powi(2)
    }

    // Phần thưởng nội tại tỷ lệ nghịch với độ lệch nội môi
    pub fn compute_reward(&self, previous_deviation: f32) -> f32 {
        let current_deviation = self.compute_deviation();
        previous_deviation - current_deviation
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Transition {
    pub state: [f32; 15],
    pub action: [f32; 4],
    pub reward: f32,
    pub next_state: [f32; 15],
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LastTransitionState {
    pub state: [f32; 15],
    pub action: [f32; 4],
    pub has_last: bool,
}

#[derive(Resource)]
pub struct TransitionSender(pub crossbeam_channel::Sender<Transition>);
