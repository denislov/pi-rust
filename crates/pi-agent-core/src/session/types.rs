use pi_ai::types::{ContentBlock, StopReason};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionHeader {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub version: u32,
    pub id: String,
    pub timestamp: String,
    pub cwd: String,
    #[serde(rename = "parentSession", skip_serializing_if = "Option::is_none")]
    pub parent_session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub id: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub timestamp: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl SessionEntry {
    pub fn message(
        id: String,
        parent_id: Option<String>,
        timestamp: String,
        message: StoredAgentMessage,
    ) -> Self {
        let mut fields = Map::new();
        fields.insert(
            "message".into(),
            serde_json::to_value(message).expect("stored message serializes"),
        );
        Self {
            entry_type: "message".into(),
            id,
            parent_id,
            timestamp,
            fields,
        }
    }

    pub fn session_info(
        id: String,
        parent_id: Option<String>,
        timestamp: String,
        name: String,
    ) -> Self {
        let mut fields = Map::new();
        fields.insert("name".into(), Value::String(name));
        Self {
            entry_type: "session_info".into(),
            id,
            parent_id,
            timestamp,
            fields,
        }
    }

    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum StoredAgentMessage {
    #[serde(rename = "user")]
    User {
        content: Vec<ContentBlock>,
        timestamp: u64,
    },
    #[serde(rename = "assistant")]
    Assistant {
        content: Vec<ContentBlock>,
        api: String,
        provider: String,
        model: String,
        #[serde(rename = "responseModel", skip_serializing_if = "Option::is_none")]
        response_model: Option<String>,
        #[serde(rename = "responseId", skip_serializing_if = "Option::is_none")]
        response_id: Option<String>,
        usage: StoredUsage,
        #[serde(rename = "stopReason")]
        stop_reason: StopReason,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
        timestamp: u64,
    },
    #[serde(rename = "toolResult")]
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        content: Vec<ContentBlock>,
        #[serde(rename = "isError")]
        is_error: bool,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StoredUsageCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StoredUsage {
    pub input: u32,
    pub output: u32,
    #[serde(rename = "cacheRead")]
    pub cache_read: u32,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u32,
    pub total: u32,
    pub cost: StoredUsageCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlSessionMetadata {
    pub id: String,
    pub created_at: String,
    pub cwd: String,
    pub path: std::path::PathBuf,
    pub parent_session_path: Option<std::path::PathBuf>,
}
