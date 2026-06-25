use crate::compaction::error::CompactionError;
use crate::types::AgentMessage;
use pi_ai::types::{ContentBlock, Context, Message, Model, StreamOptions};
use tokio_util::sync::CancellationToken;

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

    let mut llm_messages: Vec<Message> = messages
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::UserText { text, .. } => Some(Message::User {
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
            }),
            AgentMessage::Assistant { message, .. } => Some(Message::Assistant {
                content: message.content.clone(),
            }),
            AgentMessage::ToolResult {
                tool_call_id,
                content,
                tool_name,
                is_error,
                ..
            } => Some(Message::ToolResult {
                tool_call_id: tool_call_id.clone(),
                tool_name: Some(tool_name.clone()),
                is_error: Some(*is_error),
                content: content.clone(),
            }),
            AgentMessage::SystemPrompt { .. } | AgentMessage::CompactionSummary { .. } => None,
            AgentMessage::BashExecution {
                command,
                output,
                exclude_from_context,
                ..
            } => {
                if *exclude_from_context {
                    None
                } else {
                    Some(Message::User {
                        content: vec![ContentBlock::Text {
                            text: crate::convert::bash_execution_to_text(
                                command, output, None, false, false, None,
                            ),
                            text_signature: None,
                        }],
                    })
                }
            }
            AgentMessage::Custom { content, .. } => Some(Message::User {
                content: content.clone(),
            }),
            AgentMessage::BranchSummary { summary, .. } => Some(Message::User {
                content: vec![ContentBlock::Text {
                    text: summary.clone(),
                    text_signature: None,
                }],
            }),
        })
        .collect();

    llm_messages.push(Message::User {
        content: vec![ContentBlock::Text {
            text: "Please summarize the conversation history above.".into(),
            text_signature: None,
        }],
    });

    let ctx = Context {
        system_prompt: Some(system_prompt.into()),
        messages: llm_messages,
        tools: None,
    };

    let mut opts = stream_options.unwrap_or_default();
    opts.cancel = cancel;
    opts.max_tokens = Some(4096);

    let stream = pi_ai::stream_model(model, ctx, Some(opts));
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
