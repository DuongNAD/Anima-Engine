use std::sync::{Arc, RwLock};
use anima_engine_lib::evolution::meta_ai::GeminiWebSessionClient;
use anima_engine_lib::core::engine::ChronicleEvent;

#[test]
fn test_gemini_websession_client_query_and_logging() {
    // 1. Instantiate the client with a mock token
    let client = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(client.session_token, "mock-session-token-123");

    // 2. Verify query execution (will run mock fallback because api is offline / local endpoint fails)
    let response = client.query("Please simulate a nutrient drought.");
    assert!(response.is_ok(), "Query execution failed: {:?}", response.err());
    let response_text = response.unwrap();
    assert_eq!(response_text, "ResourceDrought", "Expected mock fallback response to contain ResourceDrought");

    // 3. Verify timeline event logging
    let chronicle_history = Arc::new(RwLock::new(Vec::<ChronicleEvent>::new()));

    client.log_event_to_timeline(
        &chronicle_history,
        "Drought",
        "Gemini Web Session Triggered Drought",
        "A resource drought was generated via Gemini Web Session API query.",
    );

    let history = chronicle_history.read().unwrap();
    assert_eq!(history.len(), 1);
    let event = &history[0];
    assert_eq!(event.event_type, "Drought");
    assert_eq!(event.title, "Gemini Web Session Triggered Drought");
    assert_eq!(event.description, "A resource drought was generated via Gemini Web Session API query.");
    assert!(!event.id.is_empty());
    assert!(event.timestamp > 0);
}

#[test]
fn test_gemini_websession_client_case_insensitive_fallback() {
    let client = GeminiWebSessionClient::new("mock-session-token-123");
    
    // Test uppercase drought
    let response = client.query("Please simulate a nutrient DROUGHT.");
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), "ResourceDrought");
    
    // Test mixedcase temperature
    let response = client.query("Simulate a Temperature change.");
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), "TemperatureSpike");
}

#[test]
fn test_gemini_websession_client_env_endpoint() {
    std::env::set_var("GEMINI_WEBSESSION_ENDPOINT", "https://custom.endpoint.url/v1/query");
    let client = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(client.endpoint, "https://custom.endpoint.url/v1/query");
    
    // Test with empty string env var (should fallback)
    std::env::set_var("GEMINI_WEBSESSION_ENDPOINT", "");
    let client_fallback_empty = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(client_fallback_empty.endpoint, "https://api.gemini.websession.local/v1/query");

    // Remove the env var to avoid affecting other tests
    std::env::remove_var("GEMINI_WEBSESSION_ENDPOINT");
}

