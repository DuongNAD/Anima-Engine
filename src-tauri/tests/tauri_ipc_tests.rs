use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AgentState {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub energy: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct SimulationStatus {
    pub running: bool,
    pub tick_count: u64,
    pub avg_tick_time_ms: f64,
    pub fps: f64,
}

pub struct MockAppState {
    pub running: Arc<AtomicBool>,
    pub tick_count: Arc<AtomicU64>,
}

impl Default for MockAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAppState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tick_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

// Mock Tauri IPC command handlers
pub fn mock_get_simulation_status(state: &MockAppState) -> Result<SimulationStatus, String> {
    Ok(SimulationStatus {
        running: state.running.load(Ordering::SeqCst),
        tick_count: state.tick_count.load(Ordering::SeqCst),
        avg_tick_time_ms: 1.45,
        fps: 60.2,
    })
}

pub fn mock_toggle_simulation(state: &MockAppState) -> Result<bool, String> {
    let current = state.running.load(Ordering::SeqCst);
    state.running.store(!current, Ordering::SeqCst);
    Ok(!current)
}

// Mock Tauri event emission channel
pub struct MockEventChannel {
    pub emitted_events: std::sync::Mutex<Vec<(String, String)>>,
}

impl Default for MockEventChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEventChannel {
    pub fn new() -> Self {
        Self {
            emitted_events: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn emit<S: Serialize>(&self, event_name: &str, payload: &S) -> Result<(), String> {
        let serialized = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        self.emitted_events
            .lock()
            .unwrap()
            .push((event_name.to_string(), serialized));
        Ok(())
    }
}

#[test]
fn test_serialization_simulation_status() {
    let status = SimulationStatus {
        running: true,
        tick_count: 120,
        avg_tick_time_ms: 1.45,
        fps: 60.2,
    };
    let serialized = serde_json::to_string(&status).unwrap();
    let deserialized: SimulationStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, status);
}

#[test]
fn test_serialization_agent_state() {
    let agent = AgentState {
        id: 1,
        x: 10.0,
        y: 20.0,
        z: 30.0,
        yaw: 0.1,
        pitch: 0.2,
        roll: 0.3,
        energy: 99.5,
    };
    let serialized = serde_json::to_string(&agent).unwrap();
    let deserialized: AgentState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, agent);
}

#[test]
fn test_mock_get_simulation_status() {
    let app_state = MockAppState::new();
    let status = mock_get_simulation_status(&app_state).unwrap();
    assert!(!status.running);
    assert_eq!(status.tick_count, 0);
}

#[test]
fn test_mock_toggle_simulation() {
    let app_state = MockAppState::new();

    // Toggle on
    let running1 = mock_toggle_simulation(&app_state).unwrap();
    assert!(running1);
    let status1 = mock_get_simulation_status(&app_state).unwrap();
    assert!(status1.running);

    // Toggle off
    let running2 = mock_toggle_simulation(&app_state).unwrap();
    assert!(!running2);
    let status2 = mock_get_simulation_status(&app_state).unwrap();
    assert!(!status2.running);
}

#[test]
fn test_mock_event_emission() {
    let channel = MockEventChannel::new();
    let agents = vec![AgentState {
        id: 1,
        x: 1.0,
        y: 2.0,
        z: 3.0,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        energy: 100.0,
    }];

    channel.emit("simulation-tick", &agents).unwrap();

    let events = channel.emitted_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "simulation-tick");

    let deserialized: Vec<AgentState> = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(deserialized, agents);
}

#[test]
fn test_real_simulation_engine_start_stop() {
    let engine = anima_engine_lib::core::engine::SimulationEngine::new();

    // Verify it is not running initially
    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!engine.get_status().running);

    // Start the simulation engine
    let evolution_settings = Arc::new(std::sync::Mutex::new(anima_engine_lib::commands::EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let map_elites_grid = Arc::new(std::sync::Mutex::new(anima_engine_lib::commands::MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));
    engine.start(
        None::<tauri::AppHandle<tauri::test::MockRuntime>>,
        evolution_settings,
        evolution_running,
        map_elites_grid,
    );

    // Wait briefly for the simulation thread to start and tick
    let mut started = false;
    for _ in 0..200 {
        if engine.get_status().running {
            started = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(started, "Simulation failed to start within 2 seconds");

    // Verify it runs
    assert!(engine.running.load(std::sync::atomic::Ordering::SeqCst));
    let status = engine.get_status();
    assert!(status.running);
    assert!(status.tick_count > 0);

    // Stop the engine
    engine.stop();

    // Verify that it is no longer running and status is updated
    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!engine.get_status().running);

    // Verify agent segment telemetry data
    {
        let states = engine.agent_states.read().unwrap();
        assert_eq!(states.len(), 30, "Should track exactly 30 segments (10 agents * 3 segments)");
        
        // Find segment with ID 1 and verify parent
        let seg_1 = states.iter().find(|s| s.segment_id == 1).expect("Should find segment 1");
        assert_eq!(seg_1.parent_segment_id, Some(0), "Segment 1 must have parent 0");

        let seg_2 = states.iter().find(|s| s.segment_id == 2).expect("Should find segment 2");
        assert_eq!(seg_2.parent_segment_id, Some(1), "Segment 2 must have parent 1");

        // Verify root segment (segment 0) has no parent
        let seg_0 = states.iter().find(|s| s.segment_id == 0).expect("Should find segment 0");
        assert_eq!(seg_0.parent_segment_id, None, "Segment 0 must have no parent");
    }
}
