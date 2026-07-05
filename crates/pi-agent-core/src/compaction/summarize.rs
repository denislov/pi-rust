use crate::ai_runtime::stream_model_with_global_runtime;
use crate::compaction::error::CompactionError;
use crate::types::AgentMessage;
use pi_ai::types::{ContentBlock, Context, Message, Model, StreamOptions};
use tokio_util::sync::CancellationToken;

/// Maximum characters for a tool result in serialized summaries. Mirrors TS
/// `TOOL_RESULT_MAX_CHARS`: keeps the summarization request within a reasonable
/// token budget without losing the signal of long outputs.
const TOOL_RESULT_MAX_CHARS: usize = 2000;

/// Truncate text to a maximum character length for summarization, keeping the
/// beginning and appending a truncation marker. Mirrors TS `truncateForSummary`.
fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let truncated_chars = char_count - max_chars;
    let prefix: String = text.chars().take(max_chars).collect();
    format!("{prefix}\n\n[... {truncated_chars} more characters truncated]")
}

/// Render tool-call arguments as `key=value` pairs, mirroring TS
/// `serializeConversation`'s argument formatting.
fn arguments_kv(arguments: &serde_json::Value) -> String {
    if let Some(obj) = arguments.as_object() {
        obj.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        arguments.to_string()
    }
}

/// Collect the text-bearing content of a content block list as a single
/// string, using placeholders for images.
fn content_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text, .. } => Some(text.clone()),
            ContentBlock::Image { mime_type, .. } => Some(format!("[image: {mime_type}]")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Serialize a single [`AgentMessage`] into a text line for summarization.
/// Returns `None` for messages that contribute no summarizable text (e.g.
/// system prompts, excluded bash executions, empty content).
fn serialize_message(msg: &AgentMessage) -> Option<String> {
    match msg {
        AgentMessage::UserText { text, .. } => {
            if text.is_empty() {
                None
            } else {
                Some(format!("[User]: {text}"))
            }
        }
        AgentMessage::Assistant { message, .. } => {
            let mut thinking_parts: Vec<String> = Vec::new();
            let mut text_parts: Vec<String> = Vec::new();
            let mut tool_calls: Vec<String> = Vec::new();
            for block in &message.content {
                match block {
                    ContentBlock::Text { text, .. } => text_parts.push(text.clone()),
                    ContentBlock::Thinking { thinking, .. } => {
                        thinking_parts.push(thinking.clone())
                    }
                    ContentBlock::ToolCall {
                        name, arguments, ..
                    } => {
                        tool_calls.push(format!("{}({})", name, arguments_kv(arguments)));
                    }
                    ContentBlock::Image { mime_type, .. } => {
                        text_parts.push(format!("[image: {mime_type}]"));
                    }
                }
            }
            let mut out: Vec<String> = Vec::new();
            if !thinking_parts.is_empty() {
                out.push(format!(
                    "[Assistant thinking]: {}",
                    thinking_parts.join("\n")
                ));
            }
            if !text_parts.is_empty() {
                out.push(format!("[Assistant]: {}", text_parts.join("\n")));
            }
            if !tool_calls.is_empty() {
                out.push(format!("[Assistant tool calls]: {}", tool_calls.join("; ")));
            }
            if out.is_empty() {
                None
            } else {
                Some(out.join("\n\n"))
            }
        }
        AgentMessage::ToolResult {
            content,
            is_error,
            tool_name,
            ..
        } => {
            let text = content_text(content);
            if text.is_empty() {
                None
            } else {
                let label = if *is_error { " (error)" } else { "" };
                Some(format!(
                    "[Tool result{label} ({tool_name})]: {}",
                    truncate_for_summary(&text, TOOL_RESULT_MAX_CHARS)
                ))
            }
        }
        AgentMessage::SystemPrompt { .. } => None,
        AgentMessage::CompactionSummary { summary, .. } => {
            if summary.is_empty() {
                None
            } else {
                Some(format!("[Compaction summary]: {summary}"))
            }
        }
        AgentMessage::BashExecution {
            command,
            output,
            exclude_from_context,
            ..
        } => {
            if *exclude_from_context {
                None
            } else {
                let text = crate::convert::bash_execution_to_text(
                    command, output, None, false, false, None,
                );
                Some(format!("[User]: {text}"))
            }
        }
        AgentMessage::Custom { content, .. } => {
            let text = content_text(content);
            if text.is_empty() {
                None
            } else {
                Some(format!("[User]: {text}"))
            }
        }
        AgentMessage::BranchSummary { summary, .. } => {
            if summary.is_empty() {
                None
            } else {
                Some(format!("[Branch summary]: {summary}"))
            }
        }
    }
}

/// Serialize conversation history to text for summarization, mirroring TS
/// `serializeConversation` (`pi/packages/coding-agent/src/core/compaction/utils.ts`).
///
/// This prevents the summarization model from treating the history as a
/// conversation to continue, and—critically—avoids emitting provider-level
/// assistant `tool_calls` or `ToolResult` messages. A summarized slice that
/// ends between an assistant `toolCall` and its `toolResult` would otherwise
/// violate OpenAI's rule that assistant `tool_calls` must be immediately
/// followed by tool messages.
pub fn serialize_conversation(messages: &[AgentMessage]) -> String {
    let parts: Vec<String> = messages.iter().filter_map(serialize_message).collect();
    parts.join("\n\n")
}

/// Build the summarization [`Context`] as a single user message wrapping the
/// serialized conversation in `<conversation>` tags, mirroring TS
/// `generateSummary`. No assistant or tool-result messages are emitted, so the
/// request stays valid even for histories containing tool calls.
pub fn build_summarization_context(messages: &[AgentMessage], system_prompt: &str) -> Context {
    let conversation_text = serialize_conversation(messages);
    let prompt_text = format!(
        "<conversation>\n{conversation_text}\n</conversation>\n\nPlease summarize the conversation history above."
    );
    Context {
        system_prompt: Some(system_prompt.to_string()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text {
                text: prompt_text,
                text_signature: None,
            }],
        }],
        tools: None,
    }
}

pub async fn summarize(
    model: &Model,
    messages: &[AgentMessage],
    custom_instructions: Option<&str>,
    stream_options: Option<StreamOptions>,
    cancel: Option<CancellationToken>,
) -> Result<String, CompactionError> {
    let system_prompt = custom_instructions.unwrap_or(
        "You are helping compact conversation history. Summarize the key points, decisions, and actions.",
    );

    let ctx = build_summarization_context(messages, system_prompt);

    let mut opts = stream_options.unwrap_or_default();
    opts.cancel = cancel;
    opts.max_tokens = Some(4096);

    let stream = stream_model_with_global_runtime(model, ctx, Some(opts));
    let message = pi_ai::complete(stream)
        .await
        .map_err(|e| CompactionError::SummarizationFailed(format!("complete failed: {}", e)))?;

    let text_blocks: Vec<String> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();

    let summary = text_blocks.join("\n");

    if summary.trim().is_empty() {
        return Err(CompactionError::SummarizationFailed("empty summary".into()));
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::types::AssistantMessage;

    fn user_msg(text: &str) -> AgentMessage {
        AgentMessage::UserText {
            message_id: "u".into(),
            text: text.into(),
        }
    }

    fn assistant_text(text: &str) -> AgentMessage {
        let mut msg = AssistantMessage::empty("test", "test-model");
        msg.content.push(ContentBlock::Text {
            text: text.into(),
            text_signature: None,
        });
        AgentMessage::Assistant {
            message_id: "a".into(),
            message: msg,
        }
    }

    fn assistant_tool_call(id: &str, name: &str, args: serde_json::Value) -> AgentMessage {
        let mut msg = AssistantMessage::empty("test", "test-model");
        msg.content.push(ContentBlock::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args,
            thought_signature: None,
        });
        AgentMessage::Assistant {
            message_id: "a".into(),
            message: msg,
        }
    }

    fn tool_result(call_id: &str, name: &str, text: &str) -> AgentMessage {
        AgentMessage::ToolResult {
            message_id: "t".into(),
            tool_call_id: call_id.into(),
            tool_name: name.into(),
            is_error: false,
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
        }
    }

    fn assistant_messages(ctx: &Context) -> Vec<&Message> {
        ctx.messages
            .iter()
            .filter(|m| matches!(m, Message::Assistant { .. }))
            .collect()
    }

    fn tool_result_messages(ctx: &Context) -> Vec<&Message> {
        ctx.messages
            .iter()
            .filter(|m| matches!(m, Message::ToolResult { .. }))
            .collect()
    }

    fn user_text(ctx: &Context) -> String {
        match &ctx.messages[0] {
            Message::User { content } => content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>(),
            _ => panic!("expected user message, got {:?}", ctx.messages[0]),
        }
    }

    // ---- serialize_conversation ----

    #[test]
    fn serialize_includes_user_and_assistant_text() {
        let msgs = vec![user_msg("hello there"), assistant_text("hi back")];
        let text = serialize_conversation(&msgs);
        assert!(text.contains("[User]: hello there"), "{text}");
        assert!(text.contains("[Assistant]: hi back"), "{text}");
    }

    #[test]
    fn serialize_represents_tool_calls_as_text() {
        let msgs = vec![
            assistant_tool_call("call_1", "read", serde_json::json!({"path": "src/lib.rs"})),
            tool_result("call_1", "read", "file contents here"),
        ];
        let text = serialize_conversation(&msgs);
        assert!(text.contains("read"), "tool call name missing: {text}");
        assert!(
            text.contains("src/lib.rs"),
            "tool call args missing: {text}"
        );
        assert!(
            text.contains("file contents here"),
            "tool result missing: {text}"
        );
    }

    #[test]
    fn serialize_handles_split_history_without_orphan_tool_call() {
        // Core bug scenario: the summarized slice ends right after an
        // assistant tool call, with its tool result OUTSIDE the slice.
        // Serialization must still produce valid text (no protocol constraint
        // that tool_calls must be followed by tool messages).
        let msgs = vec![
            user_msg("please read the file"),
            assistant_tool_call("call_1", "read", serde_json::json!({"path": "src/lib.rs"})),
        ];
        let text = serialize_conversation(&msgs);
        assert!(text.contains("read"), "{text}");
        assert!(text.contains("src/lib.rs"), "{text}");
    }

    // ---- build_summarization_context ----

    #[test]
    fn summarization_context_is_single_user_message() {
        let msgs = vec![user_msg("hello"), assistant_text("hi")];
        let ctx = build_summarization_context(&msgs, "system");
        assert_eq!(ctx.messages.len(), 1, "{:?}", ctx.messages);
        assert!(matches!(ctx.messages[0], Message::User { .. }));
        assert!(assistant_messages(&ctx).is_empty());
        assert!(tool_result_messages(&ctx).is_empty());
    }

    #[test]
    fn summarization_context_has_no_structured_tool_calls() {
        let msgs = vec![
            user_msg("read the file"),
            assistant_tool_call("call_1", "read", serde_json::json!({"path": "src/lib.rs"})),
            tool_result("call_1", "read", "contents"),
        ];
        let ctx = build_summarization_context(&msgs, "system");
        // No assistant messages at all (so no ToolCall blocks), no ToolResult messages.
        assert!(
            assistant_messages(&ctx).is_empty(),
            "no assistant messages: {:?}",
            ctx.messages
        );
        assert!(
            tool_result_messages(&ctx).is_empty(),
            "no tool result messages: {:?}",
            ctx.messages
        );
        assert_eq!(ctx.messages.len(), 1);
        // The single user message must contain only text blocks (no ToolCall).
        if let Message::User { content } = &ctx.messages[0] {
            for block in content {
                assert!(
                    matches!(block, ContentBlock::Text { .. }),
                    "non-text block in user message: {block:?}"
                );
            }
        }
    }

    #[test]
    fn summarization_context_represents_tool_calls_in_text() {
        let msgs = vec![
            assistant_tool_call("call_1", "read", serde_json::json!({"path": "src/lib.rs"})),
            tool_result("call_1", "read", "the file contents"),
        ];
        let ctx = build_summarization_context(&msgs, "system");
        let text = user_text(&ctx);
        assert!(text.contains("read"), "tool call name in text: {text}");
        assert!(
            text.contains("src/lib.rs"),
            "tool call args in text: {text}"
        );
        assert!(
            text.contains("the file contents"),
            "tool result in text: {text}"
        );
        assert!(text.contains("<conversation>"), "wrapped: {text}");
        assert!(text.contains("</conversation>"), "wrapped: {text}");
    }
}
