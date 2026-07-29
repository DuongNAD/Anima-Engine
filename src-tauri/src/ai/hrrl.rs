use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LearningQueueSnapshot {
    pub queued: u64,
    pub full_rejections: u64,
    pub disconnected_rejections: u64,
    pub backpressure_skipped: u64,
}

#[derive(Default)]
struct LearningQueueCounters {
    queued: AtomicU64,
    full_rejections: AtomicU64,
    disconnected_rejections: AtomicU64,
    backpressure_skipped: AtomicU64,
}

/// Monotonic, allocation-free accounting for the simulation-to-learner boundary.
///
/// The learner is intentionally decoupled from the real-time tick. When its bounded queue is full,
/// the tick drops that transition instead of blocking, and records the loss here so degraded
/// training throughput is observable rather than silently biasing a run.
#[derive(Resource, Clone, Default)]
pub struct LearningQueueDiagnostics(Arc<LearningQueueCounters>);

impl LearningQueueDiagnostics {
    fn increment(counter: &AtomicU64) {
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_add(1))
        });
    }

    pub fn record_queued(&self) {
        Self::increment(&self.0.queued);
    }

    pub fn record_full_rejection(&self) {
        Self::increment(&self.0.full_rejections);
    }

    pub fn record_disconnected_rejection(&self) {
        Self::increment(&self.0.disconnected_rejections);
    }

    pub fn record_backpressure_skipped(&self, count: usize) {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        let _ = self.0.backpressure_skipped.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |value| Some(value.saturating_add(count)),
        );
    }

    pub fn snapshot(&self) -> LearningQueueSnapshot {
        LearningQueueSnapshot {
            queued: self.0.queued.load(Ordering::Relaxed),
            full_rejections: self.0.full_rejections.load(Ordering::Relaxed),
            disconnected_rejections: self.0.disconnected_rejections.load(Ordering::Relaxed),
            backpressure_skipped: self.0.backpressure_skipped.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.0.queued.store(0, Ordering::Relaxed);
        self.0.full_rejections.store(0, Ordering::Relaxed);
        self.0.disconnected_rejections.store(0, Ordering::Relaxed);
        self.0.backpressure_skipped.store(0, Ordering::Relaxed);
    }
}
