use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub enum EnvironmentEvent {
    Drought,
    TemperatureSpike,
    PredatorWave,
    UnknownEvent,
}

pub struct MotherNatureMock {
    api_key_set: bool,
    request_count: AtomicUsize,
    rate_limit: usize,
}

impl MotherNatureMock {
    pub fn new(api_key_set: bool, rate_limit: usize) -> Self {
        Self {
            api_key_set,
            request_count: AtomicUsize::new(0),
            rate_limit,
        }
    }

    pub fn fetch_next_event(&self) -> Result<EnvironmentEvent, String> {
        let current_requests = self.request_count.fetch_add(1, Ordering::SeqCst);
        if current_requests >= self.rate_limit {
            return Err("Rate limit exceeded".to_string());
        }

        if !self.api_key_set {
            // Offline fallback event loop
            Ok(EnvironmentEvent::TemperatureSpike)
        } else {
            // Simulate Gemini trigger...
            Ok(EnvironmentEvent::Drought)
        }
    }
}

// Struct representing Bevy engine configuration parameters modified by the chronicle
#[derive(Default, Debug, Clone)]
pub struct SimParameters {
    pub metabolic_decay_rate: f64,
    pub max_food_count: u32,
    pub predator_spawn_rate: f64,
}

pub fn apply_chronicle_event(event: &EnvironmentEvent, params: &mut SimParameters) {
    match event {
        EnvironmentEvent::TemperatureSpike => {
            params.metabolic_decay_rate *= 1.5;
        }
        EnvironmentEvent::Drought => {
            params.max_food_count = (params.max_food_count as f64 * 0.5) as u32;
        }
        EnvironmentEvent::PredatorWave => {
            params.predator_spawn_rate *= 2.0;
        }
        EnvironmentEvent::UnknownEvent => {
            // Invalid event: no change
        }
    }

    // Boundary checks & clamping
    if params.metabolic_decay_rate < 0.1 {
        params.metabolic_decay_rate = 0.1;
    } else if params.metabolic_decay_rate > 10.0 {
        params.metabolic_decay_rate = 10.0;
    }

    if params.max_food_count < 10 {
        params.max_food_count = 10;
    } else if params.max_food_count > 1000 {
        params.max_food_count = 1000;
    }

    if params.predator_spawn_rate < 0.1 {
        params.predator_spawn_rate = 0.1;
    } else if params.predator_spawn_rate > 5.0 {
        params.predator_spawn_rate = 5.0;
    }
}

#[test]
fn test_chronicle_mock_client_and_param_updates() {
    let mock_nature = MotherNatureMock::new(false, 100); // No API Key set
    let event = mock_nature.fetch_next_event().unwrap();
    assert_eq!(event, EnvironmentEvent::TemperatureSpike, "Should fall back to TemperatureSpike event");

    let mut sim_params = SimParameters {
        metabolic_decay_rate: 1.0,
        max_food_count: 100,
        predator_spawn_rate: 1.0,
    };

    apply_chronicle_event(&event, &mut sim_params);
    assert_eq!(sim_params.metabolic_decay_rate, 1.5, "Metabolic decay should increase by 1.5x");

    // Test Drought
    apply_chronicle_event(&EnvironmentEvent::Drought, &mut sim_params);
    assert_eq!(sim_params.max_food_count, 50, "Max food count should be halved");
}

#[test]
fn test_meta_ai_tier1_tier2_keys_rate_limits_invalid() {
    // Tier 1: Client initialization with different parameters
    let mock_nature_no_key = MotherNatureMock::new(false, 5);
    let mock_nature_with_key = MotherNatureMock::new(true, 5);

    // Environmental event variant tests
    assert_eq!(mock_nature_no_key.fetch_next_event().unwrap(), EnvironmentEvent::TemperatureSpike);
    assert_eq!(mock_nature_with_key.fetch_next_event().unwrap(), EnvironmentEvent::Drought);

    // Tier 2: Rate limit simulation
    for _ in 0..4 {
        let _ = mock_nature_no_key.fetch_next_event();
    }
    // Next request should hit rate limit
    let rate_limit_err = mock_nature_no_key.fetch_next_event();
    assert!(rate_limit_err.is_err());
    assert_eq!(rate_limit_err.unwrap_err(), "Rate limit exceeded");

    // Tier 2: Invalid event handling
    let mut params = SimParameters {
        metabolic_decay_rate: 1.0,
        max_food_count: 100,
        predator_spawn_rate: 1.0,
    };
    let initial_params = params.clone();
    apply_chronicle_event(&EnvironmentEvent::UnknownEvent, &mut params);
    assert_eq!(params.metabolic_decay_rate, initial_params.metabolic_decay_rate);
    assert_eq!(params.max_food_count, initial_params.max_food_count);
    assert_eq!(params.predator_spawn_rate, initial_params.predator_spawn_rate);
}

#[test]
fn test_meta_ai_tier3_simultaneous_adjustments_and_boundaries() {
    // Multi-parameter simultaneous adjustments & boundaries
    let mut params = SimParameters {
        metabolic_decay_rate: 8.0,
        max_food_count: 15,
        predator_spawn_rate: 3.0,
    };

    // Apply TemperatureSpike -> metabolic decay climbs to 12.0 (clamped to 10.0)
    apply_chronicle_event(&EnvironmentEvent::TemperatureSpike, &mut params);
    assert_eq!(params.metabolic_decay_rate, 10.0);

    // Apply Drought -> max food count halves to 7 (clamped to 10)
    apply_chronicle_event(&EnvironmentEvent::Drought, &mut params);
    assert_eq!(params.max_food_count, 10);

    // Apply PredatorWave -> predator spawn rate climbs to 6.0 (clamped to 5.0)
    apply_chronicle_event(&EnvironmentEvent::PredatorWave, &mut params);
    assert_eq!(params.predator_spawn_rate, 5.0);
}

#[test]
fn test_meta_ai_tier4_timeline_event_stream_drift() {
    let mut params = SimParameters {
        metabolic_decay_rate: 1.0,
        max_food_count: 1000,
        predator_spawn_rate: 1.0,
    };

    // Stream of 10 sequential events representing environment timeline
    let event_stream = vec![
        EnvironmentEvent::TemperatureSpike, // decay = 1.5
        EnvironmentEvent::TemperatureSpike, // decay = 2.25
        EnvironmentEvent::Drought,          // food = 500
        EnvironmentEvent::PredatorWave,     // predator = 2.0
        EnvironmentEvent::Drought,          // food = 250
        EnvironmentEvent::TemperatureSpike, // decay = 3.375
        EnvironmentEvent::PredatorWave,     // predator = 4.0
        EnvironmentEvent::Drought,          // food = 125
        EnvironmentEvent::PredatorWave,     // predator = 8.0 (clamped to 5.0)
        EnvironmentEvent::Drought,          // food = 62
    ];

    for event in event_stream {
        apply_chronicle_event(&event, &mut params);
    }

    // Assert Bevy parameter drift
    assert_eq!(params.metabolic_decay_rate, 3.375);
    assert_eq!(params.max_food_count, 62);
    assert_eq!(params.predator_spawn_rate, 5.0); // should be clamped
}

use bevy_ecs::prelude::*;
use anima_engine_lib::evolution::meta_ai::{EnvironmentalEvent, MetaAiClient, MockMetaAiClient, GeminiMetaAiClient};
use anima_engine_lib::core::ecs::{ActiveEnvironmentEvent, FoodSpawnSettings, apply_environmental_effects_system, Agent};
use anima_engine_lib::ai::hrrl::HomeostaticState;

#[test]
fn test_real_mock_client_behavior() {
    let client = MockMetaAiClient;
    let mut history = Vec::new();
    
    // Epoch 1 -> ResourceDrought
    let e1 = client.generate_event(1, &history);
    assert_eq!(e1, EnvironmentalEvent::ResourceDrought);
    history.push(e1);
    
    // Epoch 2 -> TemperatureSpike
    let e2 = client.generate_event(2, &history);
    assert_eq!(e2, EnvironmentalEvent::TemperatureSpike);
    history.push(e2);
    
    // Epoch 3 -> GlacialPeriod
    let e3 = client.generate_event(3, &history);
    assert_eq!(e3, EnvironmentalEvent::GlacialPeriod);
    history.push(e3);
    
    // Epoch 4 -> ToxicDeluge
    let e4 = client.generate_event(4, &history);
    assert_eq!(e4, EnvironmentalEvent::ToxicDeluge);
    history.push(e4);
    
    // Epoch 5 -> Stable
    let e5 = client.generate_event(5, &history);
    assert_eq!(e5, EnvironmentalEvent::Stable);
}

#[test]
fn test_gemini_client_fallback_without_key() {
    std::env::remove_var("GEMINI_API_KEY");
    let client = GeminiMetaAiClient::new(std::time::Duration::from_millis(50));
    
    let event = client.generate_event(1, &[]);
    assert_eq!(event, EnvironmentalEvent::ResourceDrought);
}

#[test]
fn test_food_spawn_settings_drought_multiplier() {
    let mut world = World::new();
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::ResourceDrought));
    world.insert_resource(FoodSpawnSettings {
        max_food_count: 50,
        default_energy: 30.0,
        default_hydration: 20.0,
    });
    
    let mut schedule = Schedule::default();
    schedule.add_systems(apply_environmental_effects_system);
    
    schedule.run(&mut world);
    
    let food_settings = world.resource::<FoodSpawnSettings>();
    assert_eq!(food_settings.max_food_count, 25);
}

#[test]
fn test_homeostatic_targets_shift_under_temperature_events() {
    let mut world = World::new();
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::TemperatureSpike));
    world.insert_resource(FoodSpawnSettings::default());
    
    let agent_entity = world.spawn((
        Agent,
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        }
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(apply_environmental_effects_system);
    
    schedule.run(&mut world);
    
    let state = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert_eq!(state.temp_target, 42.0);
    
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::GlacialPeriod));
    schedule.run(&mut world);
    
    let state2 = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert_eq!(state2.temp_target, 32.0);
}

#[test]
fn test_decoupled_asynchronous_channel() {
    let (tx, rx) = crossbeam_channel::bounded::<EnvironmentalEvent>(32);
    
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        tx.send(EnvironmentalEvent::ToxicDeluge).unwrap();
    });
    
    let start = std::time::Instant::now();
    let mut received = None;
    while start.elapsed() < std::time::Duration::from_millis(200) {
        if let Ok(event) = rx.try_recv() {
            received = Some(event);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    
    assert_eq!(received, Some(EnvironmentalEvent::ToxicDeluge));
}

#[test]
fn test_chronicle_history_integration() {
    use anima_engine_lib::core::engine::{SimulationEngine, ChronicleEvent};
    
    let engine = SimulationEngine::new();
    
    // Check initial state
    assert!(engine.chronicle_history.read().unwrap().is_empty());
    
    // Add an event
    let event = ChronicleEvent {
        id: "test-id".to_string(),
        event_type: "TemperatureSpike".to_string(),
        timestamp: 12345,
        title: "Test Event".to_string(),
        description: "Test Description".to_string(),
        parameter_delta: std::collections::HashMap::new(),
    };
    engine.chronicle_history.write().unwrap().push(event.clone());
    
    // Retrieve and check
    let history = engine.chronicle_history.read().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "Test Event");
    assert_eq!(history[0].description, "Test Description");
    assert_eq!(history[0].event_type, "TemperatureSpike");
    assert_eq!(history[0].timestamp, 12345);
}


