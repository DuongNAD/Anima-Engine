use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EvolutionSettings {
    pub mutation_rate: f64,
    pub selection_bias: f64,
    pub grid_resolution: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EliteIndividualState {
    pub fitness: f64,
    pub features: Vec<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapElitesGridState {
    pub grid: HashMap<String, EliteIndividualState>,
    pub grid_resolution: u32,
}

#[test]
fn test_serialization_evolution_settings() {
    let settings = EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    };
    
    // Check deserialization from valid JSON string
    let json_str = r#"{"mutation_rate":0.15,"selection_bias":1.5,"grid_resolution":50}"#;
    let deserialized: EvolutionSettings = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized, settings);
    assert_eq!(deserialized.mutation_rate, 0.15);
    assert_eq!(deserialized.selection_bias, 1.5);
    assert_eq!(deserialized.grid_resolution, 50);

    // Check serialization back to JSON
    let serialized = serde_json::to_string(&settings).unwrap();
    // Verify that the serialized JSON can be deserialized back to the original struct
    let round_trip: EvolutionSettings = serde_json::from_str(&serialized).unwrap();
    assert_eq!(round_trip, settings);
}

#[test]
fn test_serialization_elite_individual_state() {
    let elite = EliteIndividualState {
        fitness: 0.85,
        features: vec![0.2, 0.4],
    };
    
    // Check deserialization from JSON
    let json_str = r#"{"fitness":0.85,"features":[0.2,0.4]}"#;
    let deserialized: EliteIndividualState = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized, elite);
    assert_eq!(deserialized.fitness, 0.85);
    assert_eq!(deserialized.features, vec![0.2, 0.4]);

    // Check serialization
    let serialized = serde_json::to_string(&elite).unwrap();
    let round_trip: EliteIndividualState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(round_trip, elite);
}

#[test]
fn test_serialization_map_elites_grid_state() {
    let mut grid = HashMap::new();
    grid.insert(
        "10,20".to_string(),
        EliteIndividualState {
            fitness: 0.85,
            features: vec![0.2, 0.4],
        },
    );
    grid.insert(
        "30,40".to_string(),
        EliteIndividualState {
            fitness: 0.92,
            features: vec![0.6, 0.8],
        },
    );

    let grid_state = MapElitesGridState {
        grid,
        grid_resolution: 50,
    };

    // Check serialization and round trip
    let serialized = serde_json::to_string(&grid_state).unwrap();
    let deserialized: MapElitesGridState = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(deserialized.grid_resolution, 50);
    assert_eq!(deserialized.grid.len(), 2);
    
    let ind1 = deserialized.grid.get("10,20").unwrap();
    assert_eq!(ind1.fitness, 0.85);
    assert_eq!(ind1.features, vec![0.2, 0.4]);

    let ind2 = deserialized.grid.get("30,40").unwrap();
    assert_eq!(ind2.fitness, 0.92);
    assert_eq!(ind2.features, vec![0.6, 0.8]);
}
