use crate::session::{SessionEntry, SessionError, SessionErrorCode, StoredAgentMessage};
use crate::types::AgentMessage;
use pi_ai::types::{AssistantMessage, ContentBlock, Cost, Usage};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub messages: Vec<AgentMessage>,
    pub thinking_level: String,
    pub model: Option<(String, String)>,
    pub active_tool_names: Option<Vec<String>>,
}

fn infer_leaf_id(entries: &[SessionEntry]) -> Option<String> {
    for entry in entries.iter().rev() {
        if entry.entry_type == "leaf" {
            let target = entry.field("targetId");
            if target.map_or(true, |v| v.is_null()) {
                return None;
            }
            return target.and_then(|v| v.as_str()).map(str::to_string);
        } else if entry.entry_type != "session" {
            return Some(entry.id.clone());
        }
    }
    None
}

fn path_to_root<'a>(
    leaf_id: Option<&str>,
    by_id: &HashMap<&str, &'a SessionEntry>,
) -> Result<Vec<&'a SessionEntry>, SessionError> {
    let leaf_id = match leaf_id {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let mut path: Vec<&SessionEntry> = Vec::new();
    let mut current = leaf_id;
    let mut visited: HashMap<&str, bool> = HashMap::new();

    loop {
        if visited.contains_key(current) {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("cycle detected in session entries at id: {}", current),
            ));
        }
        visited.insert(current, true);

        let entry = by_id.get(current).ok_or_else(|| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("entry not found in session: {}", current),
            )
        })?;

        path.push(*entry);

        if let Some(parent_id) = &entry.parent_id {
            current = parent_id;
        } else {
            break;
        }
    }

    path.reverse();
    Ok(path)
}

fn stored_to_agent_message(_entry_id: &str, stored: StoredAgentMessage) -> Option<AgentMessage> {
    match stored {
        StoredAgentMessage::User {
            content,
            timestamp: _,
        } => {
            let text = content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(AgentMessage::UserText {
                message_id: _entry_id.to_string(),
                text,
            })
        }
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
            let cost = Cost {
                input: usage.cost.input,
                output: usage.cost.output,
                cache_read: usage.cost.cache_read,
                cache_write: usage.cost.cache_write,
            };
            let rust_usage = Usage {
                input: usage.input,
                output: usage.output,
                cache_read: usage.cache_read,
                cache_write: usage.cache_write,
                total_tokens: usage.total,
                cost,
            };
            let assistant = AssistantMessage {
                content,
                api,
                provider: Some(provider),
                model,
                response_model,
                response_id,
                usage: rust_usage,
                stop_reason,
                error_message,
                diagnostics: None,
                timestamp,
            };
            Some(AgentMessage::Assistant {
                message_id: _entry_id.to_string(),
                message: assistant,
            })
        }
        StoredAgentMessage::ToolResult {
            tool_call_id,
            tool_name,
            content,
            is_error,
            timestamp: _,
        } => Some(AgentMessage::ToolResult {
            message_id: _entry_id.to_string(),
            tool_call_id,
            tool_name,
            is_error,
            content,
        }),
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
            message_id: _entry_id.to_string(),
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
            message_id: _entry_id.to_string(),
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
            message_id: _entry_id.to_string(),
            summary,
            from_id,
            timestamp,
        }),
    }
}

fn compaction_summary_message(entry: &SessionEntry) -> Option<AgentMessage> {
    entry
        .field("summary")
        .and_then(|value| value.as_str())
        .map(|summary| AgentMessage::UserText {
            message_id: entry.id.clone(),
            text: format!(
                "The conversation history before this point was compacted into the following summary:\n\n<summary>\n{summary}\n</summary>"
            ),
        })
}

fn append_context_entry(context: &mut SessionContext, entry: &SessionEntry) {
    match entry.entry_type.as_str() {
        "message" => {
            if let Some(message) = entry
                .field("message")
                .and_then(|value| serde_json::from_value::<StoredAgentMessage>(value.clone()).ok())
            {
                if let Some(agent_message) = stored_to_agent_message(&entry.id, message) {
                    context.messages.push(agent_message);
                }
            }
        }
        "branch_summary" => {
            if let Some(summary) = entry.field("summary").and_then(|value| value.as_str()) {
                context.messages.push(AgentMessage::UserText {
                    message_id: entry.id.clone(),
                    text: format!(
                        "The following is a summary of a branch that this conversation came back from:\n\n<summary>\n{summary}\n</summary>"
                    ),
                });
            }
        }
        _ => {}
    }
}

pub fn build_session_context(
    entries: &[SessionEntry],
    explicit_leaf_id: Option<&str>,
) -> Result<SessionContext, SessionError> {
    let by_id: HashMap<&str, &SessionEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    let leaf_id = explicit_leaf_id
        .map(str::to_string)
        .or_else(|| infer_leaf_id(entries));
    let path = path_to_root(leaf_id.as_deref(), &by_id)?;
    let mut context = SessionContext {
        thinking_level: "off".into(),
        ..Default::default()
    };

    let mut latest_compaction: Option<usize> = None;
    for (idx, entry) in path.iter().enumerate() {
        match entry.entry_type.as_str() {
            "thinking_level_change" => {
                if let Some(level) = entry
                    .field("thinkingLevel")
                    .and_then(|value| value.as_str())
                {
                    context.thinking_level = level.to_string();
                }
            }
            "model_change" => {
                let provider = entry.field("provider").and_then(|value| value.as_str());
                let model_id = entry.field("modelId").and_then(|value| value.as_str());
                if let (Some(provider), Some(model_id)) = (provider, model_id) {
                    context.model = Some((provider.to_string(), model_id.to_string()));
                }
            }
            "active_tools_change" => {
                if let Some(names) = entry
                    .field("activeToolNames")
                    .and_then(|value| value.as_array())
                {
                    context.active_tool_names = Some(
                        names
                            .iter()
                            .filter_map(|value| value.as_str().map(str::to_string))
                            .collect(),
                    );
                }
            }
            "compaction" => {
                latest_compaction = Some(idx);
            }
            _ => {}
        }
    }

    if let Some(compaction_idx) = latest_compaction {
        let compaction = path[compaction_idx];
        if let Some(message) = compaction_summary_message(compaction) {
            context.messages.push(message);
        }

        let first_kept_id = compaction
            .field("firstKeptEntryId")
            .and_then(|value| value.as_str());
        let mut found_first_kept = first_kept_id.is_none();
        for entry in &path[..compaction_idx] {
            if first_kept_id == Some(entry.id.as_str()) {
                found_first_kept = true;
            }
            if found_first_kept {
                append_context_entry(&mut context, entry);
            }
        }
        for entry in &path[compaction_idx + 1..] {
            append_context_entry(&mut context, entry);
        }
    } else {
        for entry in &path {
            append_context_entry(&mut context, entry);
        }
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionEntry;
    use pi_ai::types::ContentBlock;
    use serde_json::Map;

    fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
        SessionEntry::message(
            id.into(),
            parent.map(str::to_string),
            "2026-06-05T00:00:00.000Z".into(),
            StoredAgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: text.into(),
                    text_signature: None,
                }],
                timestamp: 1,
            },
        )
    }

    fn assistant_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
        SessionEntry::message(
            id.into(),
            parent.map(str::to_string),
            "2026-06-05T00:00:01.000Z".into(),
            StoredAgentMessage::Assistant {
                content: vec![ContentBlock::Text {
                    text: text.into(),
                    text_signature: None,
                }],
                api: "faux".into(),
                provider: "faux".into(),
                model: "faux-model".into(),
                response_model: None,
                response_id: None,
                usage: crate::session::StoredUsage::default(),
                stop_reason: pi_ai::types::StopReason::Stop,
                error_message: None,
                timestamp: 1,
            },
        )
    }

    #[test]
    fn builds_context_from_latest_linear_leaf() {
        let entries = vec![
            user_entry("a", None, "one"),
            user_entry("b", Some("a"), "two"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 2);
    }

    #[test]
    fn builds_context_from_explicit_leaf_entry_target() {
        let mut leaf = SessionEntry {
            entry_type: "leaf".into(),
            id: "leaf0001".into(),
            parent_id: Some("b".into()),
            timestamp: "2026-06-05T00:00:01.000Z".into(),
            fields: Map::new(),
        };
        leaf.fields
            .insert("targetId".into(), serde_json::Value::String("a".into()));
        let entries = vec![
            user_entry("a", None, "one"),
            user_entry("b", Some("a"), "two"),
            leaf,
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 1);
    }

    #[test]
    fn empty_entries_returns_empty_context() {
        let context = build_session_context(&[], None).unwrap();
        assert!(context.messages.is_empty());
    }

    #[test]
    fn assistant_message_included_in_context() {
        let entries = vec![
            user_entry("a", None, "hello"),
            assistant_entry("b", Some("a"), "hi there"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 2);
        match &context.messages[1] {
            AgentMessage::Assistant {
                message_id,
                message,
            } => {
                assert_eq!(message_id, "b");
                assert_eq!(
                    message.content[0],
                    ContentBlock::Text {
                        text: "hi there".into(),
                        text_signature: None
                    }
                );
            }
            _ => panic!("expected assistant message"),
        }
    }

    #[test]
    fn context_includes_thinking_level_change() {
        let entries = vec![
            SessionEntry {
                entry_type: "thinking_level_change".into(),
                id: "think1".into(),
                parent_id: None,
                timestamp: "2026-06-05T00:00:00.000Z".into(),
                fields: {
                    let mut m = Map::new();
                    m.insert(
                        "thinkingLevel".into(),
                        serde_json::Value::String("high".into()),
                    );
                    m
                },
            },
            user_entry("a", Some("think1"), "hello"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.thinking_level, "high");
    }

    #[test]
    fn context_includes_model_change() {
        let entries = vec![
            SessionEntry {
                entry_type: "model_change".into(),
                id: "model1".into(),
                parent_id: None,
                timestamp: "2026-06-05T00:00:00.000Z".into(),
                fields: {
                    let mut m = Map::new();
                    m.insert(
                        "provider".into(),
                        serde_json::Value::String("anthropic".into()),
                    );
                    m.insert(
                        "modelId".into(),
                        serde_json::Value::String("claude-sonnet-4-5".into()),
                    );
                    m
                },
            },
            user_entry("a", Some("model1"), "hello"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(
            context.model,
            Some(("anthropic".to_string(), "claude-sonnet-4-5".to_string()))
        );
    }

    #[test]
    fn context_includes_compaction_summary() {
        let entries = vec![
            SessionEntry {
                entry_type: "compaction".into(),
                id: "comp1".into(),
                parent_id: None,
                timestamp: "2026-06-05T00:00:00.000Z".into(),
                fields: {
                    let mut m = Map::new();
                    m.insert(
                        "summary".into(),
                        serde_json::Value::String("compacted text".into()),
                    );
                    m
                },
            },
            user_entry("a", Some("comp1"), "hello"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 2);
        match &context.messages[0] {
            AgentMessage::UserText { text, .. } => {
                assert!(text.contains("compacted"));
                assert!(text.contains("compacted text"));
            }
            _ => panic!("expected user text"),
        }
    }

    #[test]
    fn compaction_replaces_prior_history_and_keeps_from_first_kept_entry() {
        let entries = vec![
            user_entry("u1", None, "old user"),
            assistant_entry("a1", Some("u1"), "old assistant"),
            user_entry("u2", Some("a1"), "kept user"),
            assistant_entry("a2", Some("u2"), "kept assistant"),
            SessionEntry {
                entry_type: "compaction".into(),
                id: "comp1".into(),
                parent_id: Some("a2".into()),
                timestamp: "2026-06-05T00:00:02.000Z".into(),
                fields: {
                    let mut m = Map::new();
                    m.insert(
                        "summary".into(),
                        serde_json::Value::String("summary of old history".into()),
                    );
                    m.insert(
                        "firstKeptEntryId".into(),
                        serde_json::Value::String("u2".into()),
                    );
                    m.insert("tokensBefore".into(), serde_json::Value::Number(20.into()));
                    m
                },
            },
            user_entry("u3", Some("comp1"), "new user"),
        ];

        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 4);
        assert!(matches!(
            &context.messages[0],
            AgentMessage::UserText { text, .. } if text.contains("summary of old history")
        ));
        assert!(matches!(
            &context.messages[1],
            AgentMessage::UserText { text, .. } if text == "kept user"
        ));
        assert!(matches!(
            &context.messages[2],
            AgentMessage::Assistant { message, .. }
                if message.content == vec![ContentBlock::Text {
                    text: "kept assistant".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &context.messages[3],
            AgentMessage::UserText { text, .. } if text == "new user"
        ));
    }

    #[test]
    fn context_includes_branch_summary() {
        let entries = vec![
            SessionEntry {
                entry_type: "branch_summary".into(),
                id: "branch1".into(),
                parent_id: None,
                timestamp: "2026-06-05T00:00:00.000Z".into(),
                fields: {
                    let mut m = Map::new();
                    m.insert(
                        "summary".into(),
                        serde_json::Value::String("branch text".into()),
                    );
                    m
                },
            },
            user_entry("a", Some("branch1"), "hello"),
        ];
        let context = build_session_context(&entries, None).unwrap();
        assert_eq!(context.messages.len(), 2);
        match &context.messages[0] {
            AgentMessage::UserText { text, .. } => {
                assert!(text.contains("branch"));
                assert!(text.contains("branch text"));
            }
            _ => panic!("expected user text"),
        }
    }
}
