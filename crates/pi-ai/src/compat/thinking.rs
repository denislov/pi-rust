use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThinkingLevelValue {
    String(String),
    Null,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct ThinkingLevelMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimal: Option<ThinkingLevelValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<ThinkingLevelValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<ThinkingLevelValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<ThinkingLevelValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xhigh: Option<ThinkingLevelValue>,
}

impl ThinkingLevelMap {
    pub fn from_json(value: Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingFormat {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "zai")]
    Zai,
    #[serde(rename = "qwen")]
    Qwen,
    #[serde(rename = "qwen-chat-template")]
    QwenChatTemplate,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "string-thinking")]
    StringThinking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheControlFormat {
    #[serde(rename = "anthropic")]
    Anthropic,
}
