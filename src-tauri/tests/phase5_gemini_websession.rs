use anima_engine_lib::core::engine::ChronicleEvent;
use anima_engine_lib::evolution::meta_ai::{
    EnvironmentalEvent, GeminiWebSessionClient, MetaAiClient, MAX_META_AI_RESPONSE_BYTES,
};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

fn client_responding_with(response: &str) -> (GeminiWebSessionClient, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test endpoint");
    let address = listener.local_addr().expect("test endpoint address");
    let body = serde_json::json!({ "response": response }).to_string();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).expect("read request");
            assert_ne!(read, 0, "client closed before sending its complete request");
            request.extend_from_slice(&chunk[..read]);
            let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let headers = std::str::from_utf8(&request[..header_end]).expect("HTTP headers");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write response");
    });
    let client = GeminiWebSessionClient {
        session_token: "test-session".into(),
        endpoint: format!("http://{address}/v1/query"),
    };
    (client, server)
}

#[test]
fn transport_failure_is_reported_instead_of_inferred_from_prompt_keywords() {
    let client = GeminiWebSessionClient {
        session_token: "test-session".into(),
        endpoint: "http://127.0.0.1:0/v1/query".into(),
    };
    let prompt = "Choose one: Stable, ResourceDrought, TemperatureSpike, GlacialPeriod, \
                  ToxicDeluge.";

    let error = client
        .query(prompt)
        .expect_err("an offline endpoint has not selected an environmental event");
    assert!(!error.is_empty());
}

#[test]
fn transport_failure_falls_back_by_epoch_instead_of_prompt_keywords() {
    let client = GeminiWebSessionClient {
        session_token: "test-session".into(),
        endpoint: "http://127.0.0.1:0/v1/query".into(),
    };

    assert_eq!(
        client.generate_event(2, &[]),
        EnvironmentalEvent::TemperatureSpike,
        "epoch 2 is the deterministic fallback; the standard prompt also contains 'drought'"
    );
}

#[test]
fn test_gemini_websession_client_configuration_and_logging() {
    // 1. Instantiate the client with a mock token
    let client = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(client.session_token, "mock-session-token-123");

    // 2. Verify timeline event logging
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
    assert_eq!(
        event.description,
        "A resource drought was generated via Gemini Web Session API query."
    );
    assert!(!event.id.is_empty());
    assert!(event.timestamp > 0);
}

#[test]
fn successful_query_returns_the_remote_response() {
    let (client, server) = client_responding_with("TemperatureSpike");

    assert_eq!(
        client
            .query("real request")
            .expect("valid endpoint response"),
        "TemperatureSpike"
    );
    server.join().expect("test server exits");
}

#[test]
fn oversized_remote_response_is_rejected_before_it_can_drive_the_simulation() {
    let oversized = "x".repeat(MAX_META_AI_RESPONSE_BYTES + 1);
    let (client, server) = client_responding_with(&oversized);

    let error = client
        .query("real request")
        .expect_err("one event token never needs an unbounded response body");
    assert!(error.contains("too large"), "unexpected error: {error}");
    server.join().expect("test server exits");
}

#[test]
fn ambiguous_remote_choice_uses_the_deterministic_fallback() {
    let (client, server) = client_responding_with("ResourceDrought or Stable");

    assert_eq!(
        client.generate_event(2, &[]),
        EnvironmentalEvent::TemperatureSpike,
        "an ambiguous answer must not inject the first event it mentions"
    );
    server.join().expect("test server exits");
}

#[test]
fn test_gemini_websession_client_env_endpoint() {
    std::env::set_var(
        "GEMINI_WEBSESSION_ENDPOINT",
        "https://custom.endpoint.url/v1/query",
    );
    let client = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(client.endpoint, "https://custom.endpoint.url/v1/query");

    // Test with empty string env var (should fallback)
    std::env::set_var("GEMINI_WEBSESSION_ENDPOINT", "");
    let client_fallback_empty = GeminiWebSessionClient::new("mock-session-token-123");
    assert_eq!(
        client_fallback_empty.endpoint,
        "https://api.gemini.websession.local/v1/query"
    );

    // Remove the env var to avoid affecting other tests
    std::env::remove_var("GEMINI_WEBSESSION_ENDPOINT");
}
