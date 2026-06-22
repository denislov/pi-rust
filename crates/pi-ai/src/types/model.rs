use crate::compat::{ModelCompat, ThinkingLevelMap};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub reasoning: bool,
    #[serde(rename = "thinkingLevelMap", skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<ThinkingLevelMap>,
    pub input: Vec<ModelInput>,
    pub cost: ModelCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u32,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<ModelCompat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    Text,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_serde_camelcase() {
        let m = Model {
            id: "claude-sonnet-4-5".into(),
            name: "Claude Sonnet 4.5".into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""baseUrl""#));
        assert!(json.contains(r#""contextWindow""#));
        assert!(json.contains(r#""maxTokens""#));
    }
}
