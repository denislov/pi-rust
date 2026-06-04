pub mod context;
pub mod error;
pub mod id;
pub mod jsonl;
pub mod memory;
pub mod repo;
pub mod types;

pub use context::{SessionContext, build_session_context};
pub use error::{SessionError, SessionErrorCode};
pub use id::{SessionIdGenerator, create_session_id, create_timestamp, generate_entry_id};
pub use jsonl::JsonlSessionStorage;
pub use memory::InMemorySessionStorage;
pub use repo::JsonlSessionRepo;
pub use types::{
    JsonlSessionMetadata, SessionEntry, SessionHeader, SessionMetadata, StoredAgentMessage,
    StoredUsage, StoredUsageCost,
};

pub fn agent_message_to_stored(
    msg: &crate::types::AgentMessage,
    timestamp_ms: u64,
) -> Option<StoredAgentMessage> {
    use crate::types::AgentMessage;

    match msg {
        AgentMessage::UserText {
            message_id: _,
            text,
        } => Some(StoredAgentMessage::User {
            content: vec![pi_ai::types::ContentBlock::Text {
                text: text.clone(),
                text_signature: None,
            }],
            timestamp: timestamp_ms,
        }),
        AgentMessage::Assistant {
            message_id: _,
            message,
        } => Some(StoredAgentMessage::Assistant {
            content: message.content.clone(),
            api: message.api.clone(),
            provider: message.provider.clone().unwrap_or_default(),
            model: message.model.clone(),
            response_model: message.response_model.clone(),
            response_id: message.response_id.clone(),
            usage: StoredUsage {
                input: message.usage.input,
                output: message.usage.output,
                cache_read: message.usage.cache_read,
                cache_write: message.usage.cache_write,
                total: message.usage.total_tokens,
                cost: StoredUsageCost {
                    input: message.usage.cost.input,
                    output: message.usage.cost.output,
                    cache_read: message.usage.cost.cache_read,
                    cache_write: message.usage.cost.cache_write,
                },
            },
            stop_reason: message.stop_reason.clone(),
            error_message: message.error_message.clone(),
            timestamp: message.timestamp,
        }),
        AgentMessage::ToolResult {
            message_id: _,
            tool_call_id,
            tool_name,
            is_error,
            content,
        } => Some(StoredAgentMessage::ToolResult {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            content: content.clone(),
            is_error: *is_error,
            timestamp: timestamp_ms,
        }),
        AgentMessage::SystemPrompt { .. } => None,
    }
}
