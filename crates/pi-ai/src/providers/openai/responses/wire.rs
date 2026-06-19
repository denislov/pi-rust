use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ResponseCreateRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub input: Vec<ResponseInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponseTool>>,
    #[serde(rename = "max_output_tokens", skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "tool_choice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(rename = "prompt_cache_key", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResponseInputItem {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: serde_json::Value,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput { call_id: String, output: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

// ── SSE event types ────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseStreamEvent {
    #[serde(rename = "response.created")]
    ResponseCreated { response: ResponseInfo },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded { item: OutputItem },
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded { part: ContentPart },
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta { delta: String },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone { item: OutputItem },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: ResponseInfo },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseInfo {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub usage: Option<ResponseUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub call_id: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(default)]
    pub text: Option<String>,
}
