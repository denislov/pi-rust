use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ConverseStreamRequest {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<BedrockMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemBlock>>,
    #[serde(rename = "inferenceConfig", skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    #[serde(
        rename = "additionalModelRequestFields",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_model_request_fields: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InferenceConfig {
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(rename = "cachePoint", skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<CachePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BedrockMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentBlock {
    Text(String),
    Image(ImageBlock),
    ToolUse(ToolUseBlock),
    ToolResult(ToolResultBlock),
    ReasoningContent(ReasoningContentBlock),
    CachePoint(CachePoint),
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageBlock {
    pub format: String,
    pub source: ImageSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageSource {
    pub bytes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolUseBlock {
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResultBlock {
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolResultContentBlock {
    Text(String),
    Image(ImageBlock),
}

#[derive(Debug, Clone, Serialize)]
pub struct ReasoningContentBlock {
    #[serde(rename = "reasoningText")]
    pub reasoning_text: ReasoningText,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReasoningText {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CachePoint {
    #[serde(rename = "type")]
    pub cache_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolConfig {
    pub tools: Vec<BedrockTool>,
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BedrockTool {
    #[serde(rename = "toolSpec")]
    pub tool_spec: ToolSpec,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: InputSchema,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputSchema {
    pub json: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConverseStreamEvent {
    #[serde(rename = "messageStart", default)]
    pub message_start: Option<MessageStartEvent>,
    #[serde(rename = "contentBlockStart", default)]
    pub content_block_start: Option<ContentBlockStartEvent>,
    #[serde(rename = "contentBlockDelta", default)]
    pub content_block_delta: Option<ContentBlockDeltaEvent>,
    #[serde(rename = "contentBlockStop", default)]
    pub content_block_stop: Option<ContentBlockStopEvent>,
    #[serde(rename = "messageStop", default)]
    pub message_stop: Option<MessageStopEvent>,
    #[serde(default)]
    pub metadata: Option<MetadataEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageStartEvent {
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockStartEvent {
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: u32,
    pub start: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockDeltaEvent {
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: u32,
    pub delta: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockStopEvent {
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageStopEvent {
    #[serde(rename = "stopReason")]
    pub stop_reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataEvent {
    #[serde(default)]
    pub usage: Option<BedrockUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BedrockUsage {
    #[serde(rename = "inputTokens", default)]
    pub input_tokens: u32,
    #[serde(rename = "outputTokens", default)]
    pub output_tokens: u32,
    #[serde(rename = "totalTokens", default)]
    pub total_tokens: u32,
    #[serde(rename = "cacheReadInputTokens", default)]
    pub cache_read_input_tokens: u32,
    #[serde(rename = "cacheWriteInputTokens", default)]
    pub cache_write_input_tokens: u32,
}
