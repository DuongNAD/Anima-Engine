use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

/// Maximum decoded HTTP body accepted from an external Meta-AI service.
///
/// A valid response contains one short event token; 64 KiB leaves ample room for provider metadata
/// while bounding both declared bodies and streams whose size is unknown in advance.
pub const MAX_META_AI_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentalEvent {
    Stable,
    ResourceDrought,
    TemperatureSpike,
    GlacialPeriod,
    ToxicDeluge,
}

impl std::fmt::Display for EnvironmentalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stable => write!(f, "Stable"),
            Self::ResourceDrought => write!(f, "Resource Drought"),
            Self::TemperatureSpike => write!(f, "Temperature Spike"),
            Self::GlacialPeriod => write!(f, "Glacial Period"),
            Self::ToxicDeluge => write!(f, "Toxic Deluge"),
        }
    }
}

fn parse_environmental_event(response: &str) -> Option<EnvironmentalEvent> {
    let choice = response.trim();
    if choice.eq_ignore_ascii_case("Stable") {
        Some(EnvironmentalEvent::Stable)
    } else if choice.eq_ignore_ascii_case("ResourceDrought") {
        Some(EnvironmentalEvent::ResourceDrought)
    } else if choice.eq_ignore_ascii_case("TemperatureSpike") {
        Some(EnvironmentalEvent::TemperatureSpike)
    } else if choice.eq_ignore_ascii_case("GlacialPeriod") {
        Some(EnvironmentalEvent::GlacialPeriod)
    } else if choice.eq_ignore_ascii_case("ToxicDeluge") {
        Some(EnvironmentalEvent::ToxicDeluge)
    } else {
        None
    }
}

fn read_meta_ai_json_from_reader<R: Read>(
    reader: R,
    declared_size: Option<u64>,
) -> Result<serde_json::Value, String> {
    if declared_size.is_some_and(|size| size > MAX_META_AI_RESPONSE_BYTES as u64) {
        return Err(format!(
            "Meta-AI response is too large (limit {MAX_META_AI_RESPONSE_BYTES} bytes)"
        ));
    }

    let requested = declared_size.unwrap_or(0) as usize;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(requested).map_err(|error| {
        format!("cannot allocate {requested} bytes for Meta-AI response: {error}")
    })?;
    reader
        .take(MAX_META_AI_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read Meta-AI response: {error}"))?;
    if bytes.len() > MAX_META_AI_RESPONSE_BYTES {
        return Err(format!(
            "Meta-AI response is too large (limit {MAX_META_AI_RESPONSE_BYTES} bytes)"
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse Meta-AI response JSON: {error}"))
}

fn read_meta_ai_json(response: ureq::Response) -> Result<serde_json::Value, String> {
    let declared_size = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    read_meta_ai_json_from_reader(response.into_reader(), declared_size)
}

pub trait MetaAiClient: Send + Sync {
    fn generate_event(&self, epoch: u32, history: &[EnvironmentalEvent]) -> EnvironmentalEvent;
}

pub struct MockMetaAiClient;

impl MetaAiClient for MockMetaAiClient {
    fn generate_event(&self, epoch: u32, _history: &[EnvironmentalEvent]) -> EnvironmentalEvent {
        match epoch % 5 {
            1 => EnvironmentalEvent::ResourceDrought,
            2 => EnvironmentalEvent::TemperatureSpike,
            3 => EnvironmentalEvent::GlacialPeriod,
            4 => EnvironmentalEvent::ToxicDeluge,
            _ => EnvironmentalEvent::Stable,
        }
    }
}

pub struct GeminiMetaAiClient {
    pub api_key: Option<String>,
    pub timeout: Duration,
}

impl GeminiMetaAiClient {
    pub fn new(timeout: Duration) -> Self {
        let _ = dotenvy::dotenv();
        let api_key = std::env::var("GEMINI_API_KEY").ok();
        Self { api_key, timeout }
    }

    fn request_deadline_expired(&self) -> bool {
        self.timeout.is_zero()
    }
}

impl MetaAiClient for GeminiMetaAiClient {
    fn generate_event(&self, epoch: u32, history: &[EnvironmentalEvent]) -> EnvironmentalEvent {
        // G1.3: a deterministic run may not consult a live model. The answer would depend on a
        // network, a secret and a remote model's weights — none of which are part of the manifest,
        // so a replay could not reproduce it. The contract is that external AI may only *propose*
        // interventions, which are frozen into the manifest and replayed from there; by replay time
        // there is nothing left to ask. `MockMetaAiClient` is a pure function of epoch and history,
        // which is exactly what a replay needs.
        if !crate::core::determinism::DeterministicMode::from_env().allows_external_ai() {
            return MockMetaAiClient.generate_event(epoch, history);
        }
        // A zero deadline has already expired. Letting it reach the HTTP stack can still spend
        // hundreds of milliseconds in DNS/TLS setup because those platform calls are not uniformly
        // covered by a sub-request timeout. Falling back here makes the boundary deterministic.
        if self.request_deadline_expired() {
            return MockMetaAiClient.generate_event(epoch, history);
        }
        let api_key = match &self.api_key {
            Some(key) if !key.is_empty() => key,
            _ => {
                return MockMetaAiClient.generate_event(epoch, history);
            }
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
            api_key
        );

        let history_str: Vec<String> = history.iter().map(|e| e.to_string()).collect();
        let prompt = format!(
            "You are directing an evolutionary simulation. The current epoch is {}. \
             The history of environmental events is: {}. \
             Based on this, choose the next environmental event from the list: Stable, ResourceDrought, TemperatureSpike, GlacialPeriod, ToxicDeluge. \
             Respond with exactly one of those five choices as plain text. Do not include markdown formatting or additional explanation.",
            epoch,
            history_str.join(", ")
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }]
        });

        let response = ureq::post(&url).timeout(self.timeout).send_json(body);

        match response {
            Ok(res) => {
                if let Ok(json) = read_meta_ai_json(res) {
                    if let Some(text) =
                        json["candidates"][0]["content"]["parts"][0]["text"].as_str()
                    {
                        parse_environmental_event(text)
                            .unwrap_or_else(|| MockMetaAiClient.generate_event(epoch, history))
                    } else {
                        MockMetaAiClient.generate_event(epoch, history)
                    }
                } else {
                    MockMetaAiClient.generate_event(epoch, history)
                }
            }
            Err(_) => MockMetaAiClient.generate_event(epoch, history),
        }
    }
}

pub struct GeminiWebSessionClient {
    pub session_token: String,
    pub endpoint: String,
}

impl GeminiWebSessionClient {
    pub fn new(session_token: &str) -> Self {
        let _ = dotenvy::dotenv();
        let endpoint = match std::env::var("GEMINI_WEBSESSION_ENDPOINT") {
            Ok(val) if !val.is_empty() => val,
            _ => "https://api.gemini.websession.local/v1/query".to_string(),
        };
        Self {
            session_token: session_token.to_string(),
            endpoint,
        }
    }

    pub fn query(&self, prompt: &str) -> Result<String, String> {
        if self.session_token.is_empty() {
            return Err("Missing session token".to_string());
        }

        let body = serde_json::json!({
            "prompt": prompt,
            "session_token": self.session_token,
        });

        let response = ureq::post(&self.endpoint)
            .timeout(Duration::from_secs(5))
            .send_json(body);

        match response {
            Ok(res) => {
                let json = read_meta_ai_json(res)?;
                if let Some(text) = json["response"].as_str() {
                    Ok(text.to_string())
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(error) => Err(format!("Gemini WebSession request failed: {error}")),
        }
    }

    pub fn log_event_to_timeline(
        &self,
        chronicle_history: &std::sync::Arc<
            std::sync::RwLock<Vec<crate::core::engine::ChronicleEvent>>,
        >,
        event_type: &str,
        title: &str,
        description: &str,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let chronicle_event = crate::core::engine::ChronicleEvent {
            id,
            event_type: event_type.to_string(),
            timestamp,
            title: title.to_string(),
            description: description.to_string(),
            parameter_delta: std::collections::HashMap::new(),
        };

        if let Ok(mut history) = chronicle_history.write() {
            history.push(chronicle_event);
        }
    }
}

impl MetaAiClient for GeminiWebSessionClient {
    fn generate_event(&self, epoch: u32, history: &[EnvironmentalEvent]) -> EnvironmentalEvent {
        // Same G1.3 gate as `GeminiMetaAiClient`: a deterministic run must not reach the network in
        // either code path, or which client happened to be configured would decide reproducibility.
        if !crate::core::determinism::DeterministicMode::from_env().allows_external_ai() {
            return MockMetaAiClient.generate_event(epoch, history);
        }
        let history_str: Vec<String> = history.iter().map(|e| e.to_string()).collect();
        let prompt = format!(
            "You are directing an evolutionary simulation. The current epoch is {}. \
             The history of environmental events is: {}. \
             Based on this, choose the next environmental event from the list: Stable, ResourceDrought, TemperatureSpike, GlacialPeriod, ToxicDeluge. \
             Respond with exactly one of those five choices as plain text. Do not include markdown formatting or additional explanation.",
            epoch,
            history_str.join(", ")
        );

        match self.query(&prompt) {
            Ok(text) => parse_environmental_event(&text)
                .unwrap_or_else(|| MockMetaAiClient.generate_event(epoch, history)),
            Err(_) => MockMetaAiClient.generate_event(epoch, history),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_choice_parser_accepts_only_a_single_declared_event() {
        for (response, expected) in [
            ("Stable", EnvironmentalEvent::Stable),
            ("ResourceDrought", EnvironmentalEvent::ResourceDrought),
            (
                "  temperaturespike\r\n",
                EnvironmentalEvent::TemperatureSpike,
            ),
            ("GlacialPeriod", EnvironmentalEvent::GlacialPeriod),
            ("ToxicDeluge", EnvironmentalEvent::ToxicDeluge),
        ] {
            assert_eq!(parse_environmental_event(response), Some(expected));
        }
        assert_eq!(parse_environmental_event("ResourceDrought or Stable"), None);
        assert_eq!(
            parse_environmental_event("I recommend ResourceDrought."),
            None
        );
        assert_eq!(parse_environmental_event("unknown"), None);
    }

    #[test]
    fn response_reader_caps_streams_without_a_declared_length() {
        let body = vec![b'x'; MAX_META_AI_RESPONSE_BYTES + 1];
        let error = read_meta_ai_json_from_reader(std::io::Cursor::new(body), None)
            .expect_err("an undeclared stream is still bounded");

        assert!(error.contains("too large"), "unexpected error: {error}");
    }

    #[test]
    fn an_expired_request_deadline_falls_back_before_http() {
        let client = GeminiMetaAiClient {
            api_key: Some("unused-because-deadline-expired".to_owned()),
            timeout: Duration::ZERO,
        };

        assert!(client.request_deadline_expired());
        assert_eq!(
            client.generate_event(3, &[]),
            EnvironmentalEvent::GlacialPeriod
        );
    }
}
