use pi_ai::types::{ContentBlock, StopReason};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

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
    #[serde(rename = "bashExecution")]
    BashExecution {
        command: String,
        output: String,
        #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        cancelled: bool,
        truncated: bool,
        #[serde(rename = "fullOutputPath", skip_serializing_if = "Option::is_none")]
        full_output_path: Option<String>,
        #[serde(rename = "excludeFromContext", skip_serializing_if = "Option::is_none")]
        exclude_from_context: Option<bool>,
        timestamp: u64,
    },
    #[serde(rename = "custom")]
    Custom {
        #[serde(rename = "customType")]
        custom_type: String,
        content: Vec<ContentBlock>,
        display: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<Value>,
        timestamp: u64,
    },
    #[serde(rename = "branchSummary")]
    BranchSummary {
        summary: String,
        #[serde(rename = "fromId")]
        from_id: String,
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

/// A node in the session tree, built from a `SessionEntry` with resolved
/// label information and child nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionTreeNode {
    pub entry: SessionEntry,
    pub children: Vec<SessionTreeNode>,
    pub label: Option<String>,
    pub label_timestamp: Option<String>,
}

/// Filter mode for the `/tree` selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeFilterMode {
    Default,
    NoTools,
    UserOnly,
    LabeledOnly,
    All,
}

impl TreeFilterMode {
    pub fn from_str_name(s: &str) -> Self {
        match s {
            "default" => Self::Default,
            "no-tools" => Self::NoTools,
            "user-only" => Self::UserOnly,
            "labeled-only" => Self::LabeledOnly,
            "all" => Self::All,
            _ => Self::Default,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::NoTools => "no-tools",
            Self::UserOnly => "user-only",
            Self::LabeledOnly => "labeled-only",
            Self::All => "all",
        }
    }
}

impl fmt::Display for TreeFilterMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_filter_mode_from_str() {
        assert_eq!(TreeFilterMode::from_str_name("default"), TreeFilterMode::Default);
        assert_eq!(
            TreeFilterMode::from_str_name("no-tools"),
            TreeFilterMode::NoTools
        );
        assert_eq!(
            TreeFilterMode::from_str_name("user-only"),
            TreeFilterMode::UserOnly
        );
        assert_eq!(
            TreeFilterMode::from_str_name("labeled-only"),
            TreeFilterMode::LabeledOnly
        );
        assert_eq!(TreeFilterMode::from_str_name("all"), TreeFilterMode::All);
        assert_eq!(TreeFilterMode::from_str_name("invalid"), TreeFilterMode::Default);
        assert_eq!(TreeFilterMode::from_str_name(""), TreeFilterMode::Default);
    }

    #[test]
    fn tree_filter_mode_display() {
        assert_eq!(TreeFilterMode::Default.to_string(), "default");
        assert_eq!(TreeFilterMode::NoTools.to_string(), "no-tools");
        assert_eq!(TreeFilterMode::UserOnly.to_string(), "user-only");
        assert_eq!(TreeFilterMode::LabeledOnly.to_string(), "labeled-only");
        assert_eq!(TreeFilterMode::All.to_string(), "all");
    }
}
