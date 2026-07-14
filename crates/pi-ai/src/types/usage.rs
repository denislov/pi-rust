use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    #[serde(rename = "cacheRead")]
    pub cache_read: u32,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u32,
    pub cost: Cost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    Stop,
    Length,
    #[serde(rename = "toolUse")]
    ToolUse,
    Error,
    Aborted,
}
