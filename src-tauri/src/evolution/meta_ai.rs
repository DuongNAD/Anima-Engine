use serde::{Serialize, Deserialize};
use std::time::Duration;

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
}

impl MetaAiClient for GeminiMetaAiClient {
    fn generate_event(&self, epoch: u32, history: &[EnvironmentalEvent]) -> EnvironmentalEvent {
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

        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(body);

        match response {
            Ok(res) => {
                if let Ok(json) = res.into_json::<serde_json::Value>() {
                    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        let cleaned = text.trim().to_lowercase();
                        if cleaned.contains("drought") || cleaned.contains("resource") {
                            EnvironmentalEvent::ResourceDrought
                        } else if cleaned.contains("spike") || cleaned.contains("temperature") {
                            EnvironmentalEvent::TemperatureSpike
                        } else if cleaned.contains("glacial") || cleaned.contains("period") {
                            EnvironmentalEvent::GlacialPeriod
                        } else if cleaned.contains("toxic") || cleaned.contains("deluge") {
                            EnvironmentalEvent::ToxicDeluge
                        } else {
                            EnvironmentalEvent::Stable
                        }
                    } else {
                        MockMetaAiClient.generate_event(epoch, history)
                    }
                } else {
                    MockMetaAiClient.generate_event(epoch, history)
                }
            }
            Err(_) => {
                MockMetaAiClient.generate_event(epoch, history)
            }
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
                if let Ok(json) = res.into_json::<serde_json::Value>() {
                    if let Some(text) = json["response"].as_str() {
                        Ok(text.to_string())
                    } else {
                        Err("Invalid response format".to_string())
                    }
                } else {
                    Err("Failed to parse response JSON".to_string())
                }
            }
            Err(_) => {
                let lower_prompt = prompt.to_lowercase();
                if lower_prompt.contains("drought") {
                    Ok("ResourceDrought".to_string())
                } else if lower_prompt.contains("temperature") {
                    Ok("TemperatureSpike".to_string())
                } else {
                    Ok("Stable".to_string())
                }
            }
        }
    }

    pub fn log_event_to_timeline(
        &self,
        chronicle_history: &std::sync::Arc<std::sync::RwLock<Vec<crate::core::engine::ChronicleEvent>>>,
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
            Ok(text) => {
                let cleaned = text.trim().to_lowercase();
                if cleaned.contains("drought") || cleaned.contains("resource") {
                    EnvironmentalEvent::ResourceDrought
                } else if cleaned.contains("spike") || cleaned.contains("temperature") {
                    EnvironmentalEvent::TemperatureSpike
                } else if cleaned.contains("glacial") || cleaned.contains("period") {
                    EnvironmentalEvent::GlacialPeriod
                } else if cleaned.contains("toxic") || cleaned.contains("deluge") {
                    EnvironmentalEvent::ToxicDeluge
                } else if cleaned.contains("stable") {
                    EnvironmentalEvent::Stable
                } else {
                    MockMetaAiClient.generate_event(epoch, history)
                }
            }
            Err(_) => MockMetaAiClient.generate_event(epoch, history),
        }
    }
}


