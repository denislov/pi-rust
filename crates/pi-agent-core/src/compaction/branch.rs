use crate::agent::provider::stream_model_with_provider_streamer;
use crate::agent::types::{AgentMessage, AgentResources, ProviderStreamer};
use crate::compaction::branch_error::{BranchSummaryError, BranchSummaryErrorCode};
use crate::compaction::estimate::estimate_tokens;
use crate::context::conversion::convert_to_context;
use crate::transcript::{SessionEntry, StoredAgentMessage};
use pi_ai::api::conversation::{AssistantMessage, ContentBlock, Cost, StopReason, Usage};
use pi_ai::api::model::Model;
use pi_ai::api::stream::{AssistantMessageEvent, StreamOptions};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileOperations {
    pub read: BTreeSet<String>,
    pub modified: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct BranchPreparation {
    pub messages: Vec<AgentMessage>,
    pub file_ops: FileOperations,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct CollectEntriesResult {
    pub entries: Vec<SessionEntry>,
    pub common_ancestor_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BranchSummaryOptions {
    pub model: Model,
    pub api_key: String,
    pub headers: Option<serde_json::Value>,
    pub custom_instructions: Option<String>,
    pub replace_instructions: bool,
    pub reserve_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSummaryResult {
    pub summary: String,
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
}

const SUMMARIZATION_SYSTEM_PROMPT: &str =
    "You are a careful conversation summarizer. Preserve concrete details.";

const BRANCH_SUMMARY_PREAMBLE: &str = "The user explored a different conversation branch before returning here.\nSummary of that exploration:\n\n";

const BRANCH_SUMMARY_PROMPT: &str = r#"Create a structured summary of this conversation branch for context when returning later.

Use this EXACT format:

## Goal
[What was the user trying to accomplish in this branch?]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned]
- [Or "(none)" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Work that was started but not finished]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [What should happen next to continue this work]

Keep each section concise. Preserve exact file paths, function names, and error messages."#;

pub fn collect_entries_for_branch_summary(
    entries: &[SessionEntry],
    old_leaf_id: Option<&str>,
    target_id: &str,
) -> Result<CollectEntriesResult, BranchSummaryError> {
    let Some(old_leaf_id) = old_leaf_id else {
        return Ok(CollectEntriesResult {
            entries: Vec::new(),
            common_ancestor_id: None,
        });
    };

    let by_id: HashMap<&str, &SessionEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    let old_path = path_to_root(old_leaf_id, &by_id)?;
    let target_path = path_to_root(target_id, &by_id)?;
    let old_ids: BTreeSet<&str> = old_path.iter().map(|entry| entry.id.as_str()).collect();
    let common_ancestor_id = target_path
        .iter()
        .rev()
        .find(|entry| old_ids.contains(entry.id.as_str()))
        .map(|entry| entry.id.clone());

    let mut current = Some(old_leaf_id);
    let mut abandoned = Vec::new();
    while let Some(id) = current {
        if Some(id) == common_ancestor_id.as_deref() {
            break;
        }
        let entry = by_id.get(id).copied().ok_or_else(|| {
            BranchSummaryError::new(
                BranchSummaryErrorCode::InvalidSession,
                format!("entry {id} not found"),
            )
        })?;
        abandoned.push(entry.clone());
        current = entry.parent_id.as_deref();
    }
    abandoned.reverse();

    Ok(CollectEntriesResult {
        entries: abandoned,
        common_ancestor_id,
    })
}

pub fn prepare_branch_entries(entries: &[SessionEntry], token_budget: u32) -> BranchPreparation {
    let mut file_ops = FileOperations::default();
    for entry in entries {
        if entry.entry_type == "branch_summary"
            && entry.field("fromHook").and_then(|value| value.as_bool()) != Some(true)
            && let Some(details) = entry.field("details")
        {
            collect_file_details(details, &mut file_ops);
        }
    }

    let mut messages = Vec::new();
    let mut total_tokens = 0;
    for entry in entries.iter().rev() {
        let Some(message) = message_from_entry(entry) else {
            continue;
        };
        let tokens = estimate_tokens(std::slice::from_ref(&message));
        if token_budget > 0 && total_tokens + tokens > token_budget {
            if matches!(
                message,
                AgentMessage::CompactionSummary { .. } | AgentMessage::BranchSummary { .. }
            ) && total_tokens < token_budget.saturating_mul(9) / 10
            {
                total_tokens += tokens;
                messages.push(message);
            }
            break;
        }
        total_tokens += tokens;
        messages.push(message);
    }
    messages.reverse();

    BranchPreparation {
        messages,
        file_ops,
        total_tokens,
    }
}

pub async fn generate_branch_summary(
    entries: &[SessionEntry],
    options: BranchSummaryOptions,
) -> Result<BranchSummaryResult, BranchSummaryError> {
    generate_branch_summary_with_provider_streamer(entries, options, None).await
}

pub async fn generate_branch_summary_with_provider_streamer(
    entries: &[SessionEntry],
    options: BranchSummaryOptions,
    provider_streamer: Option<ProviderStreamer>,
) -> Result<BranchSummaryResult, BranchSummaryError> {
    let context_window = options.model.context_window.max(options.reserve_tokens);
    let token_budget = context_window.saturating_sub(options.reserve_tokens);
    let preparation = prepare_branch_entries(entries, token_budget);

    if preparation.messages.is_empty() {
        return Ok(BranchSummaryResult {
            summary: "No content to summarize".into(),
            read_files: Vec::new(),
            modified_files: Vec::new(),
        });
    }

    let conversation = serialize_conversation(&preparation.messages);
    let instructions = match (
        options.replace_instructions,
        options.custom_instructions.as_deref(),
    ) {
        (true, Some(custom)) => custom.to_string(),
        (false, Some(custom)) => format!("{BRANCH_SUMMARY_PROMPT}\n\nAdditional focus: {custom}"),
        _ => BRANCH_SUMMARY_PROMPT.to_string(),
    };
    let prompt = format!("<conversation>\n{conversation}\n</conversation>\n\n{instructions}");
    let context = convert_to_context(
        &Some(SUMMARIZATION_SYSTEM_PROMPT.into()),
        &[AgentMessage::UserText {
            message_id: "branch_summary_prompt".into(),
            text: prompt,
        }],
        &[],
        &AgentResources::default(),
    );
    let mut stream = stream_model_with_provider_streamer(
        &options.model,
        context,
        Some(StreamOptions {
            api_key: Some(options.api_key),
            headers: options.headers,
            max_tokens: Some(2048),
            ..Default::default()
        }),
        provider_streamer,
    );

    let mut assistant = None;
    use futures::StreamExt;
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::Done { message, .. } => {
                assistant = Some(message);
                break;
            }
            AssistantMessageEvent::Error { message, .. } => {
                if message.stop_reason == StopReason::Aborted {
                    return Err(BranchSummaryError::new(
                        BranchSummaryErrorCode::Aborted,
                        message
                            .error_message
                            .unwrap_or_else(|| "Branch summary aborted".into()),
                    ));
                }
                return Err(BranchSummaryError::new(
                    BranchSummaryErrorCode::SummarizationFailed,
                    format!(
                        "Branch summary failed: {}",
                        message
                            .error_message
                            .unwrap_or_else(|| "Unknown error".into())
                    ),
                ));
            }
            _ => {}
        }
    }

    let assistant = assistant.ok_or_else(|| {
        BranchSummaryError::new(
            BranchSummaryErrorCode::SummarizationFailed,
            "Branch summary stream ended without a final message",
        )
    })?;
    let mut summary = assistant
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if summary.is_empty() {
        summary = "No summary generated".into();
    }
    summary = format!("{BRANCH_SUMMARY_PREAMBLE}{summary}");

    let read_files: Vec<String> = preparation.file_ops.read.into_iter().collect();
    let modified_files: Vec<String> = preparation.file_ops.modified.into_iter().collect();
    summary.push_str(&format_file_operations(&read_files, &modified_files));

    Ok(BranchSummaryResult {
        summary,
        read_files,
        modified_files,
    })
}

fn path_to_root<'a>(
    leaf_id: &str,
    by_id: &HashMap<&str, &'a SessionEntry>,
) -> Result<Vec<&'a SessionEntry>, BranchSummaryError> {
    let mut path = Vec::new();
    let mut current = Some(leaf_id);
    let mut visited = BTreeSet::new();
    while let Some(id) = current {
        if !visited.insert(id.to_string()) {
            return Err(BranchSummaryError::new(
                BranchSummaryErrorCode::InvalidSession,
                format!("cycle detected in session entries at id: {id}"),
            ));
        }
        let entry = by_id.get(id).copied().ok_or_else(|| {
            BranchSummaryError::new(
                BranchSummaryErrorCode::InvalidSession,
                format!("entry {id} not found"),
            )
        })?;
        path.push(entry);
        current = entry.parent_id.as_deref();
    }
    path.reverse();
    Ok(path)
}

fn message_from_entry(entry: &SessionEntry) -> Option<AgentMessage> {
    match entry.entry_type.as_str() {
        "message" => entry
            .field("message")
            .and_then(|value| serde_json::from_value::<StoredAgentMessage>(value.clone()).ok())
            .and_then(|message| stored_to_agent_message(&entry.id, message)),
        "branch_summary" => Some(AgentMessage::BranchSummary {
            message_id: entry.id.clone(),
            summary: entry.field("summary")?.as_str()?.to_string(),
            from_id: entry.field("fromId")?.as_str()?.to_string(),
            timestamp: 0,
        }),
        "compaction" => Some(AgentMessage::CompactionSummary {
            message_id: entry.id.clone(),
            summary: entry.field("summary")?.as_str()?.to_string(),
            tokens_before: entry
                .field("tokensBefore")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as u32,
        }),
        _ => None,
    }
}

fn stored_to_agent_message(entry_id: &str, stored: StoredAgentMessage) -> Option<AgentMessage> {
    match stored {
        StoredAgentMessage::User { content, .. } => Some(AgentMessage::UserText {
            message_id: entry_id.to_string(),
            text: text_from_blocks(&content),
        }),
        StoredAgentMessage::Assistant {
            content,
            api,
            provider,
            model,
            response_model,
            response_id,
            usage,
            stop_reason,
            error_message,
            timestamp,
        } => {
            let assistant = AssistantMessage {
                content,
                api,
                provider: Some(provider),
                model,
                response_model,
                response_id,
                usage: Usage {
                    input: usage.input,
                    output: usage.output,
                    cache_read: usage.cache_read,
                    cache_write: usage.cache_write,
                    total_tokens: usage.total,
                    cost: Cost {
                        known: usage.cost.known,
                        input: usage.cost.input,
                        output: usage.cost.output,
                        cache_read: usage.cost.cache_read,
                        cache_write: usage.cost.cache_write,
                    },
                },
                stop_reason,
                error_message,
                diagnostics: None,
                timestamp,
            };
            Some(AgentMessage::Assistant {
                message_id: entry_id.to_string(),
                message: assistant,
            })
        }
        StoredAgentMessage::ToolResult { .. } => None,
        StoredAgentMessage::BashExecution {
            command,
            output,
            exit_code,
            cancelled,
            truncated,
            full_output_path,
            exclude_from_context,
            timestamp,
        } => Some(AgentMessage::BashExecution {
            message_id: entry_id.to_string(),
            command,
            output,
            exit_code,
            cancelled,
            truncated,
            full_output_path,
            exclude_from_context: exclude_from_context.unwrap_or(false),
            timestamp,
        }),
        StoredAgentMessage::Custom {
            custom_type,
            content,
            display,
            details,
            timestamp,
        } => Some(AgentMessage::Custom {
            message_id: entry_id.to_string(),
            custom_type,
            content,
            display,
            details,
            timestamp,
        }),
        StoredAgentMessage::BranchSummary {
            summary,
            from_id,
            timestamp,
        } => Some(AgentMessage::BranchSummary {
            message_id: entry_id.to_string(),
            summary,
            from_id,
            timestamp,
        }),
    }
}

fn text_from_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_conversation(messages: &[AgentMessage]) -> String {
    let context = convert_to_context(&None, messages, &[], &AgentResources::default());
    context
        .messages
        .iter()
        .map(|message| serde_json::to_string(message).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_file_details(details: &serde_json::Value, file_ops: &mut FileOperations) {
    if let Some(files) = details.get("readFiles").and_then(|value| value.as_array()) {
        for file in files.iter().filter_map(|value| value.as_str()) {
            file_ops.read.insert(file.to_string());
        }
    }
    if let Some(files) = details
        .get("modifiedFiles")
        .and_then(|value| value.as_array())
    {
        for file in files.iter().filter_map(|value| value.as_str()) {
            file_ops.modified.insert(file.to_string());
        }
    }
}

fn format_file_operations(read_files: &[String], modified_files: &[String]) -> String {
    if read_files.is_empty() && modified_files.is_empty() {
        return String::new();
    }
    let mut text = String::from("\n\n## File Operations\n");
    if !read_files.is_empty() {
        text.push_str("\n### Read\n");
        for file in read_files {
            text.push_str("- ");
            text.push_str(file);
            text.push('\n');
        }
    }
    if !modified_files.is_empty() {
        text.push_str("\n### Modified\n");
        for file in modified_files {
            text.push_str("- ");
            text.push_str(file);
            text.push('\n');
        }
    }
    text
}
