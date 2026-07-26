//! Per-frame payload preparation for the Tauri event bridge.
//!
//! The emit thread lives in [`crate::core::simulation_loop`], but the two decisions it makes every
//! frame — what goes into the tick payload, and whether the pheromone field is worth sending at all
//! — are pure functions of their inputs. They are here so they can be tested without a Tauri
//! `AppHandle`, without a running engine, and without sleeping: `simulation_loop.rs` has no
//! `#[cfg(test)]` block at all, and the emit path was the part of it with the most per-frame cost
//! and the least coverage.
//!
//! ## Why this path is allocation-sensitive
//!
//! It runs ~30 times a second for the life of the process. The version this replaced pre-allocated a
//! segment buffer and then `.clone()`d it into a freshly constructed payload, cloned the
//! environmental state twice, and built a fresh un-hinted `HashMap` each frame. Reusing one payload
//! and clearing it keeps steady state at zero allocations, which `emit_zero_alloc_tests` pins.

use crate::ai::pheromone::PheromoneGridState;
use crate::core::components::{CombatEvent, EnvironmentalState, RaycastTelemetry};
use crate::core::simulation_state::{SegmentState, SimulationTickPayload};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// How often the emit thread pushes a frame to the frontend. ~30 Hz; the simulation itself ticks at
/// 60, and the webview cannot usefully repaint faster than this.
pub const EMIT_INTERVAL: Duration = Duration::from_millis(33);

/// Rate limit for `pheromone-update` specifically — see [`PheromoneEmitGate`] for why it needs one.
pub const PHEROMONE_EMIT_INTERVAL: Duration = Duration::from_millis(100);

/// Starting capacity for the reused per-frame buffers. Both grow if a run exceeds them and then stay
/// grown; the point is that steady state allocates nothing, not that these are hard limits.
pub const EMIT_SEGMENT_CAPACITY: usize = 1000;
pub const EMIT_HEAD_DIRECTION_CAPACITY: usize = 256;

/// A tick payload sized for reuse. Construct once per emit thread, refresh per frame.
pub fn new_tick_payload() -> SimulationTickPayload {
    SimulationTickPayload {
        segments: Vec::with_capacity(EMIT_SEGMENT_CAPACITY),
        environmental_state: EnvironmentalState::default(),
        head_directions: std::collections::HashMap::with_capacity(EMIT_HEAD_DIRECTION_CAPACITY),
    }
}

/// Overwrite `payload` in place with this frame's data.
///
/// Every field is cleared and refilled rather than reassigned: `Vec::clear` and `HashMap::clear`
/// keep their capacity. Assigning `payload.environmental_state = env.clone()` instead would allocate
/// a fresh Vec every frame and free the old one — the exact cost this is avoiding.
pub fn refresh_tick_payload(
    payload: &mut SimulationTickPayload,
    segments: &[SegmentState],
    environmental_state: &EnvironmentalState,
) {
    // `SegmentState` is plain scalars, so this is a memcpy into retained capacity.
    payload.segments.clear();
    payload.segments.extend_from_slice(segments);

    copy_environmental_state(&mut payload.environmental_state, environmental_state);

    payload.head_directions.clear();
    for seg in &payload.segments {
        // The root segment carries the agent's facing. `segment_id == 0` is the same condition
        // stated a second way, kept because restored saves have produced roots with a parent id.
        if seg.parent_segment_id.is_none() || seg.segment_id == 0 {
            payload
                .head_directions
                .insert(seg.agent_id, seg.head_direction);
        }
    }
}

/// Copy the environmental state into `dst`, reusing every buffer `dst` already owns.
///
/// `dst.clone_from(src)` is the obvious way to write this and it allocates once per element per
/// frame. `EnvironmentalElement` holds a `String`, and `#[derive(Clone)]` does not generate a
/// `clone_from` — it inherits the trait's default, which is `*self = source.clone()`. `Vec`'s
/// `clone_from` dutifully calls that per element, so each frame drops 300-odd Strings and allocates
/// 300-odd replacements for the same two literals ("tree", "lake"). Measured at 301 allocations per
/// frame, which is 9,000 a second on the emit thread; `emit_zero_alloc_tests` is what caught it.
///
/// Copying the fields by hand lets `String::clear` + `push_str` reuse the destination's buffer,
/// which for two short literals never needs to grow after the first frame.
fn copy_environmental_state(dst: &mut EnvironmentalState, src: &EnvironmentalState) {
    dst.elements.truncate(src.elements.len());

    for (d, s) in dst.elements.iter_mut().zip(src.elements.iter()) {
        d.element_type.clear();
        d.element_type.push_str(&s.element_type);
        d.x = s.x;
        d.y = s.y;
        d.radius = s.radius;
        d.resources = s.resources;
    }

    // Only the elements beyond what `dst` already had need real allocation, and only until the Vec
    // reaches the run's high-water mark.
    if src.elements.len() > dst.elements.len() {
        dst.elements
            .extend_from_slice(&src.elements[dst.elements.len()..]);
    }
}

/// Decides whether the pheromone field is worth putting on the wire this frame.
///
/// It is the most expensive event the loop can send by a wide margin: 128x128 f32, which Tauri
/// serialises to a JSON array of ~16k numbers — on the order of 150-200 KB of text, parsed on the
/// webview's main thread. The previous code sent it unconditionally at the frame rate, several MB/s
/// for a diffusing scalar field that nothing reads that fast and that is frequently unchanged.
///
/// Two guards, cheapest first: a rate limit, then an equality check against what was last sent. The
/// comparison is a 64 KB memcmp, orders of magnitude below the serialisation it avoids.
pub struct PheromoneEmitGate {
    last_sent: Vec<f32>,
    ever_sent: bool,
    last_emit_at: Instant,
    interval: Duration,
}

impl PheromoneEmitGate {
    /// `now` is passed in rather than read, so a test can drive the clock instead of sleeping.
    pub fn new(cell_count: usize, interval: Duration, now: Instant) -> Self {
        Self {
            last_sent: vec![0.0; cell_count],
            ever_sent: false,
            last_emit_at: now,
            interval,
        }
    }

    /// Returns true when the caller should emit `out`, which is filled with the current field.
    ///
    /// `out` is left untouched when the answer is no, so the caller can keep reusing one buffer.
    pub fn poll(
        &mut self,
        shared: &PheromoneGridState,
        out: &mut PheromoneGridState,
        now: Instant,
    ) -> bool {
        if now.duration_since(self.last_emit_at) < self.interval {
            return false;
        }

        let changed = shared.grid != self.last_sent
            || shared.width != out.width
            || shared.height != out.height;

        // The first send is unconditional: a frontend that mounts after the field has settled still
        // needs one baseline frame to render from, and an all-zero grid is a legitimate baseline.
        if !changed && self.ever_sent {
            // Still counts as a poll for rate-limiting purposes; otherwise an unchanging field would
            // re-run the comparison on every frame instead of every `interval`.
            self.last_emit_at = now;
            return false;
        }

        if out.grid.len() == shared.grid.len() {
            out.grid.copy_from_slice(&shared.grid);
        } else {
            // Only reachable if the grid is resized mid-run, which nothing does today.
            out.grid.clear();
            out.grid.extend_from_slice(&shared.grid);
        }
        out.width = shared.width;
        out.height = shared.height;

        if self.last_sent.len() == out.grid.len() {
            self.last_sent.copy_from_slice(&out.grid);
        } else {
            self.last_sent.clear();
            self.last_sent.extend_from_slice(&out.grid);
        }
        self.ever_sent = true;
        self.last_emit_at = now;
        true
    }
}

/// The shared state the emit thread reads from. Grouped into a struct because the loop takes six
/// handles and a positional argument list of six `Arc<RwLock<_>>` is one transposition away from
/// publishing raycasts as combat events.
pub struct EmitChannels {
    pub running: Arc<AtomicBool>,
    pub agent_states: Arc<RwLock<Vec<SegmentState>>>,
    pub pheromone_grid: Arc<RwLock<PheromoneGridState>>,
    pub active_raycasts: Arc<RwLock<Vec<RaycastTelemetry>>>,
    pub combat_events: Arc<RwLock<Vec<CombatEvent>>>,
    pub environmental_state: Arc<RwLock<EnvironmentalState>>,
}

/// Spawn the thread that publishes simulation state to the frontend over Tauri events.
///
/// `app_handle` is `None` in headless runs and in every test that constructs an engine without a
/// Tauri app; the loop still runs and still observes `running`, it just has nowhere to send.
pub fn spawn_emit_thread<R: tauri::Runtime>(
    channels: EmitChannels,
    app_handle: Option<tauri::AppHandle<R>>,
    exit: crate::core::thread_supervisor::ExitToken,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Moved in so it drops when this thread's stack unwinds — on a normal return and on a panic.
        // Holding it outside would make the thread look permanently alive to `stop` (§3.7).
        let _exit = exit;
        let EmitChannels {
            running,
            agent_states,
            pheromone_grid,
            active_raycasts,
            combat_events,
            environmental_state,
        } = channels;

        // One payload, reused for the life of the thread — see `refresh_tick_payload`.
        let mut tick_payload = new_tick_payload();
        let mut local_pheromone = PheromoneGridState {
            grid: vec![0.0; crate::ai::pheromone::CELL_COUNT],
            width: crate::ai::pheromone::GRID_SIZE as u32,
            height: crate::ai::pheromone::GRID_SIZE as u32,
        };
        let mut pheromone_gate = PheromoneEmitGate::new(
            crate::ai::pheromone::CELL_COUNT,
            PHEROMONE_EMIT_INTERVAL,
            Instant::now(),
        );
        let mut local_raycasts = Vec::with_capacity(1000);
        let mut local_combat = Vec::with_capacity(100);

        while running.load(Ordering::SeqCst) {
            thread::sleep(EMIT_INTERVAL);

            let Some(ref handle) = app_handle else {
                continue;
            };

            {
                let states = agent_states.read().unwrap_or_else(|e| e.into_inner());
                let env = environmental_state
                    .read()
                    .unwrap_or_else(|e| e.into_inner());
                refresh_tick_payload(&mut tick_payload, &states, &env);
            }
            let _ = handle.emit("simulation-tick", &tick_payload);

            let should_emit_pheromone = {
                let shared = pheromone_grid.read().unwrap_or_else(|e| e.into_inner());
                pheromone_gate.poll(&shared, &mut local_pheromone, Instant::now())
            };
            if should_emit_pheromone {
                let _ = handle.emit("pheromone-update", &local_pheromone);
            }

            local_raycasts.clear();
            {
                let shared = active_raycasts.read().unwrap_or_else(|e| e.into_inner());
                local_raycasts.extend_from_slice(&shared);
            }
            let _ = handle.emit("raycast-update", &local_raycasts);

            // Swapped out rather than copied: the sim thread appends to the shared Vec, and taking
            // it wholesale leaves an empty buffer behind for the next tick to fill.
            local_combat.clear();
            {
                let mut shared = combat_events.write().unwrap_or_else(|e| e.into_inner());
                std::mem::swap(&mut *shared, &mut local_combat);
            }
            for event in &local_combat {
                let _ = handle.emit("combat-event", event);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::pheromone::{CELL_COUNT, GRID_SIZE};

    fn segment(agent_id: u32, segment_id: u32, parent: Option<u32>) -> SegmentState {
        SegmentState {
            agent_id,
            segment_id,
            parent_segment_id: parent,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            joint_anchor_x: 0.0,
            joint_anchor_y: 0.0,
            joint_anchor_z: 0.0,
            joint_axis_x: 0.0,
            joint_axis_y: 0.0,
            joint_axis_z: 0.0,
            energy: 1.0,
            hydration: 1.0,
            head_direction: [agent_id as f32, 0.0, 0.0],
            agent_type: None,
        }
    }

    fn empty_grid() -> PheromoneGridState {
        PheromoneGridState {
            grid: vec![0.0; CELL_COUNT],
            width: GRID_SIZE as u32,
            height: GRID_SIZE as u32,
        }
    }

    #[test]
    fn refresh_maps_each_agent_root_to_its_head_direction() {
        let mut payload = new_tick_payload();
        let segments = vec![
            segment(1, 0, None),
            segment(1, 1, Some(0)),
            segment(2, 0, None),
        ];

        refresh_tick_payload(&mut payload, &segments, &EnvironmentalState::default());

        assert_eq!(payload.segments.len(), 3);
        assert_eq!(payload.head_directions.len(), 2);
        assert_eq!(payload.head_directions.get(&1), Some(&[1.0, 0.0, 0.0]));
        assert_eq!(payload.head_directions.get(&2), Some(&[2.0, 0.0, 0.0]));
    }

    #[test]
    fn refresh_leaves_no_trace_of_the_previous_frame() {
        // The bug this guards: a payload reused without clearing keeps last frame's segments and
        // head directions, so an agent that died goes on being rendered.
        let mut payload = new_tick_payload();
        refresh_tick_payload(
            &mut payload,
            &[segment(1, 0, None), segment(2, 0, None)],
            &EnvironmentalState::default(),
        );
        assert_eq!(payload.head_directions.len(), 2);

        refresh_tick_payload(
            &mut payload,
            &[segment(3, 0, None)],
            &EnvironmentalState::default(),
        );

        assert_eq!(payload.segments.len(), 1);
        assert_eq!(payload.segments[0].agent_id, 3);
        assert_eq!(payload.head_directions.len(), 1);
        assert!(payload.head_directions.contains_key(&3));
        assert!(!payload.head_directions.contains_key(&1));
    }

    #[test]
    fn refresh_keeps_buffer_capacity_across_frames() {
        let mut payload = new_tick_payload();
        let before = payload.segments.capacity();
        assert!(before >= EMIT_SEGMENT_CAPACITY);

        refresh_tick_payload(
            &mut payload,
            &[segment(1, 0, None)],
            &EnvironmentalState::default(),
        );
        refresh_tick_payload(&mut payload, &[], &EnvironmentalState::default());

        // Emptying must not hand the allocation back; the next frame refills it.
        assert_eq!(payload.segments.capacity(), before);
    }

    fn element(kind: &str, x: f32) -> crate::core::components::EnvironmentalElement {
        crate::core::components::EnvironmentalElement {
            element_type: kind.to_string(),
            x,
            y: 0.0,
            radius: 1.0,
            resources: 10.0,
        }
    }

    fn env(elements: Vec<crate::core::components::EnvironmentalElement>) -> EnvironmentalState {
        EnvironmentalState { elements }
    }

    // `copy_environmental_state` is hand-written to avoid a per-element String allocation, so the
    // grow and shrink paths are its own responsibility rather than `Vec`'s.

    #[test]
    fn environmental_copy_grows_to_match_the_source() {
        let mut payload = new_tick_payload();
        refresh_tick_payload(&mut payload, &[], &env(vec![element("tree", 1.0)]));
        assert_eq!(payload.environmental_state.elements.len(), 1);

        refresh_tick_payload(
            &mut payload,
            &[],
            &env(vec![
                element("tree", 1.0),
                element("lake", 2.0),
                element("tree", 3.0),
            ]),
        );

        let got = &payload.environmental_state.elements;
        assert_eq!(got.len(), 3);
        assert_eq!(
            got.iter().map(|e| e.x).collect::<Vec<_>>(),
            vec![1.0, 2.0, 3.0]
        );
        assert_eq!(got[1].element_type, "lake");
    }

    #[test]
    fn environmental_copy_shrinks_and_leaves_no_stale_elements() {
        // A tree burning down must disappear from the payload, not linger because the buffer was
        // only overwritten up to the new length.
        let mut payload = new_tick_payload();
        refresh_tick_payload(
            &mut payload,
            &[],
            &env(vec![
                element("tree", 1.0),
                element("tree", 2.0),
                element("lake", 3.0),
            ]),
        );
        assert_eq!(payload.environmental_state.elements.len(), 3);

        refresh_tick_payload(&mut payload, &[], &env(vec![element("lake", 9.0)]));

        let got = &payload.environmental_state.elements;
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].x, 9.0);
        assert_eq!(got[0].element_type, "lake");
    }

    #[test]
    fn environmental_copy_overwrites_a_reused_slot_completely() {
        // The slot is reused, so every field has to be assigned — a missed one would show the
        // previous frame's value for an element that has since changed type.
        let mut payload = new_tick_payload();
        refresh_tick_payload(&mut payload, &[], &env(vec![element("tree", 1.0)]));

        let mut replacement = element("lake", 5.0);
        replacement.y = 7.0;
        replacement.radius = 4.0;
        replacement.resources = 99.0;
        refresh_tick_payload(&mut payload, &[], &env(vec![replacement]));

        let got = &payload.environmental_state.elements[0];
        assert_eq!(got.element_type, "lake");
        assert_eq!(
            (got.x, got.y, got.radius, got.resources),
            (5.0, 7.0, 4.0, 99.0)
        );
    }

    #[test]
    fn environmental_copy_empties_when_the_source_is_empty() {
        let mut payload = new_tick_payload();
        refresh_tick_payload(&mut payload, &[], &env(vec![element("tree", 1.0)]));
        refresh_tick_payload(&mut payload, &[], &EnvironmentalState::default());
        assert!(payload.environmental_state.elements.is_empty());
    }

    #[test]
    fn gate_sends_a_baseline_frame_even_when_the_field_is_empty() {
        let start = Instant::now();
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, Duration::from_millis(100), start);
        let shared = empty_grid();
        let mut out = empty_grid();

        // A frontend that mounts late still needs one frame to render from.
        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(100)));
    }

    #[test]
    fn gate_suppresses_an_unchanged_field() {
        let start = Instant::now();
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, Duration::from_millis(100), start);
        let shared = empty_grid();
        let mut out = empty_grid();

        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(100)));
        // Same field, a full interval later: nothing to say.
        assert!(!gate.poll(&shared, &mut out, start + Duration::from_millis(200)));
        assert!(!gate.poll(&shared, &mut out, start + Duration::from_millis(300)));
    }

    #[test]
    fn gate_rate_limits_a_field_that_changes_every_frame() {
        let start = Instant::now();
        let interval = Duration::from_millis(100);
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, interval, start);
        let mut shared = empty_grid();
        let mut out = empty_grid();

        // 30 frames at the emit loop's real 33 ms cadence, with a different field every frame so the
        // equality check never suppresses anything and the rate limit is the only thing acting.
        const FRAMES: u64 = 30;
        let mut send_times = Vec::new();
        for frame in 1..=FRAMES {
            shared.grid[0] = frame as f32;
            let now = start + Duration::from_millis(33 * frame);
            if gate.poll(&shared, &mut out, now) {
                send_times.push(now);
            }
        }

        // The property, not a hand-computed count: no two sends land closer together than the
        // interval. The gate only gets asked every 33 ms, and 33 does not divide 100, so the first
        // opportunity after a send is at +132 ms — the achievable rate is slightly below the cap
        // rather than exactly at it, which is the correct direction for a limiter to err.
        for pair in send_times.windows(2) {
            assert!(
                pair[1].duration_since(pair[0]) >= interval,
                "two sends {:?} apart, closer than the {interval:?} limit",
                pair[1].duration_since(pair[0])
            );
        }

        // And it is a real reduction: unlimited, this loop would have sent ~16k floats 30 times.
        assert!(
            send_times.len() < FRAMES as usize / 3,
            "expected far fewer than {FRAMES} sends, got {}",
            send_times.len()
        );
        assert!(
            !send_times.is_empty(),
            "a changing field must still be sent"
        );
    }

    #[test]
    fn gate_forwards_a_changed_field_verbatim() {
        let start = Instant::now();
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, Duration::from_millis(100), start);
        let mut shared = empty_grid();
        let mut out = empty_grid();

        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(100)));

        shared.grid[42] = 0.75;
        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(200)));
        assert_eq!(out.grid[42], 0.75);
        assert_eq!(out.width, GRID_SIZE as u32);
        assert_eq!(out.height, GRID_SIZE as u32);
    }

    #[test]
    fn gate_leaves_the_output_untouched_when_it_says_no() {
        let start = Instant::now();
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, Duration::from_millis(100), start);
        let mut shared = empty_grid();
        let mut out = empty_grid();

        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(100)));

        // Inside the rate-limit window the caller must be able to keep reusing `out`.
        shared.grid[7] = 1.0;
        assert!(!gate.poll(&shared, &mut out, start + Duration::from_millis(120)));
        assert_eq!(out.grid[7], 0.0, "a suppressed poll must not write to out");
    }

    #[test]
    fn gate_notices_a_field_that_returns_to_a_previously_sent_value() {
        let start = Instant::now();
        let mut gate = PheromoneEmitGate::new(CELL_COUNT, Duration::from_millis(100), start);
        let mut shared = empty_grid();
        let mut out = empty_grid();

        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(100)));
        shared.grid[3] = 5.0;
        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(200)));
        // Decayed back to empty: that is a change from what was last sent, so it must go out.
        shared.grid[3] = 0.0;
        assert!(gate.poll(&shared, &mut out, start + Duration::from_millis(300)));
        assert_eq!(out.grid[3], 0.0);
    }
}
