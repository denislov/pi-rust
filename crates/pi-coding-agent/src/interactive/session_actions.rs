use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use pi_agent_core::session::{
    JsonlSessionMetadata, JsonlSessionRepo, JsonlSessionStorage, SessionEntry, SessionHeader,
    SessionTreeNode, StoredAgentMessage, StoredUsage, create_session_id, create_timestamp,
    generate_entry_id,
};
use pi_agent_core::{AgentMessage, session};
use pi_ai::types::{ContentBlock, StopReason};

use crate::CliError;
use crate::coding_session::{
    CodingAgentSession, CodingAgentSessionHydration, CodingAgentSessionOptions,
    CodingAgentSessionTranscriptItem,
};
use crate::interactive::transcript::TranscriptItem;
use crate::runtime::{SessionMode, SessionRunOptions};
use crate::session::ResolvedSessionTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionChoice {
    pub(super) id: String,
    pub(super) cwd: String,
    pub(super) path: PathBuf,
    pub(super) created_at: String,
    pub(super) name: Option<String>,
    pub(super) entry_count: usize,
    pub(super) active_leaf_id: Option<String>,
    pub(super) kind: SessionChoiceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionChoiceKind {
    RustNative,
    LegacyJsonl,
}

/// Cumulative usage statistics computed from all assistant messages in a
/// hydrated session.  Used to initialise [`super::root::FooterStats`] so the
/// footer shows correct token/cost numbers immediately after resume, without
/// waiting for the next turn.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct CumulativeUsage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub cost: f64,
    /// Context-token estimate from the *last* assistant message with a usage
    /// block.  `None` means no assistant message has reported usage yet.
    pub last_context_tokens: Option<u32>,
}

pub(super) struct HydratedSession {
    pub(super) choice: SessionChoice,
    pub(super) transcript_items: Vec<TranscriptItem>,
    pub(super) leaf_id: Option<String>,
    pub(super) cumulative_usage: CumulativeUsage,
}

impl SessionChoice {
    pub(super) fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    pub(super) fn searchable_text(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.id,
            self.name.as_deref().unwrap_or_default(),
            self.cwd,
            self.path.display(),
            self.created_at
        )
    }

    pub(super) fn matches_target(&self, target: &str) -> bool {
        self.id == target
            || self.id.starts_with(target)
            || self.path.display().to_string() == target
            || self.name.as_deref() == Some(target)
    }
}

pub(super) fn hydrate_existing_session_target(
    session_options: &Option<SessionRunOptions>,
    target: Option<&ResolvedSessionTarget>,
) -> Result<Option<HydratedSession>, CliError> {
    let Some(session_options) = session_options else {
        return Ok(None);
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return Ok(None);
    }
    let Some(target) = target else {
        return Ok(None);
    };

    let root = interactive_session_root(session_options)?;
    let cwd = session_options.cwd.display().to_string();

    if let Some(hydrated) = hydrate_rust_native_session_target(&root, &session_options.cwd, target)?
    {
        return Ok(Some(hydrated));
    }

    let repo = JsonlSessionRepo::new(root);
    let storage = match target {
        ResolvedSessionTarget::ContinueMostRecent => repo
            .most_recent(&cwd)
            .map_err(|error| CliError::SessionFailure(error.message))?,
        ResolvedSessionTarget::OpenTarget(target) => Some(
            repo.open_target(&cwd, target)
                .map_err(|error| CliError::SessionFailure(error.message))?,
        ),
        ResolvedSessionTarget::OpenOrCreateId(id) => repo.open_target(&cwd, id).ok(),
        ResolvedSessionTarget::New | ResolvedSessionTarget::ForkTarget(_) => None,
    };

    storage.map(hydrate_session_storage).transpose()
}

fn hydrate_rust_native_session_target(
    root: &Path,
    cwd: &Path,
    target: &ResolvedSessionTarget,
) -> Result<Option<HydratedSession>, CliError> {
    let base_options = CodingAgentSessionOptions::new()
        .with_cwd(cwd)
        .with_session_log_root(root);
    let hydration = match target {
        ResolvedSessionTarget::New | ResolvedSessionTarget::ForkTarget(_) => return Ok(None),
        ResolvedSessionTarget::ContinueMostRecent => rust_native_choices(root, cwd)?
            .into_iter()
            .next()
            .map(|choice| {
                CodingAgentSession::hydrate(base_options.clone().with_session_id(choice.id.clone()))
            })
            .transpose()?,
        ResolvedSessionTarget::OpenOrCreateId(session_id) => {
            match CodingAgentSession::hydrate(base_options.with_session_id(session_id.clone())) {
                Ok(hydration) => Some(hydration),
                Err(_) => return Ok(None),
            }
        }
        ResolvedSessionTarget::OpenTarget(target) => {
            let options = if target_looks_like_rust_native_session_dir(target) {
                base_options.with_session_path(target)
            } else {
                base_options.with_session_id(target.clone())
            };
            match CodingAgentSession::hydrate(options) {
                Ok(hydration) => Some(hydration),
                Err(error) if target_looks_like_rust_native_session_dir(target) => {
                    return Err(CliError::SessionFailure(error.to_string()));
                }
                Err(_) => return Ok(None),
            }
        }
    };

    Ok(hydration
        .filter(|hydration| hydration_matches_cwd(hydration, cwd))
        .map(hydrated_session_from_rust_native))
}

pub(super) fn hydrated_session_from_rust_native(
    hydration: CodingAgentSessionHydration,
) -> HydratedSession {
    let choice = session_choice_from_rust_native(&hydration);
    HydratedSession {
        choice,
        transcript_items: hydration
            .transcript
            .into_iter()
            .map(transcript_item_from_rust_native)
            .collect(),
        leaf_id: hydration.summary.active_leaf_id,
        cumulative_usage: CumulativeUsage::default(),
    }
}

pub(super) fn hydrate_rust_native_choice(
    choice: &SessionChoice,
) -> Result<HydratedSession, CliError> {
    if choice.kind != SessionChoiceKind::RustNative {
        return Err(CliError::SessionFailure(
            "session choice is not Rust-native".into(),
        ));
    }
    let options = rust_native_choice_options(choice);
    CodingAgentSession::hydrate(options)
        .map(hydrated_session_from_rust_native)
        .map_err(CliError::from)
}

pub(super) fn clone_rust_native_choice(
    choice: &SessionChoice,
) -> Result<HydratedSession, CliError> {
    if choice.kind != SessionChoiceKind::RustNative {
        return Err(CliError::SessionFailure(
            "session choice is not Rust-native".into(),
        ));
    }
    CodingAgentSession::clone_session(rust_native_choice_options(choice))
        .map(hydrated_session_from_rust_native)
        .map_err(CliError::from)
}

pub(super) fn fork_rust_native_choice(
    choice: &SessionChoice,
    target_leaf_id: Option<&str>,
) -> Result<HydratedSession, CliError> {
    if choice.kind != SessionChoiceKind::RustNative {
        return Err(CliError::SessionFailure(
            "session choice is not Rust-native".into(),
        ));
    }
    CodingAgentSession::fork_session(rust_native_choice_options(choice), target_leaf_id)
        .map(hydrated_session_from_rust_native)
        .map_err(CliError::from)
}

fn rust_native_choice_options(choice: &SessionChoice) -> CodingAgentSessionOptions {
    let mut options = CodingAgentSessionOptions::new().with_session_path(&choice.path);
    if let Some(root) = choice.path.parent() {
        options = options.with_session_log_root(root);
    }
    if !choice.cwd.is_empty() {
        options = options.with_cwd(PathBuf::from(&choice.cwd));
    }
    options
}

pub(super) fn rust_native_tree_from_hydrated_session(
    hydrated: &HydratedSession,
) -> (Vec<SessionTreeNode>, Option<String>) {
    let timestamp = hydrated.choice.created_at.clone();
    let mut entries = Vec::new();
    let mut parent_id: Option<String> = None;

    for (index, item) in hydrated.transcript_items.iter().enumerate() {
        let entry_id = format!("rust_native_entry_{index}");
        let message = match item {
            TranscriptItem::User { text } => StoredAgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
                timestamp: 0,
            },
            TranscriptItem::Assistant { markdown, .. } => StoredAgentMessage::Assistant {
                content: vec![ContentBlock::Text {
                    text: markdown.clone(),
                    text_signature: None,
                }],
                api: "coding-session".to_string(),
                provider: "coding-session".to_string(),
                model: "coding-session".to_string(),
                response_model: None,
                response_id: None,
                usage: StoredUsage::default(),
                stop_reason: StopReason::Stop,
                error_message: None,
                timestamp: 0,
            },
            TranscriptItem::Tool {
                call_id,
                name,
                result,
                is_error,
                ..
            } => StoredAgentMessage::ToolResult {
                tool_call_id: call_id.clone(),
                tool_name: name.clone(),
                content: vec![ContentBlock::Text {
                    text: result.clone().unwrap_or_default(),
                    text_signature: None,
                }],
                is_error: *is_error,
                timestamp: 0,
            },
            TranscriptItem::Error { text } => StoredAgentMessage::Custom {
                custom_type: "error".to_string(),
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
                display: true,
                details: None,
                timestamp: 0,
            },
            TranscriptItem::System { text } => StoredAgentMessage::Custom {
                custom_type: "system".to_string(),
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
                display: true,
                details: None,
                timestamp: 0,
            },
        };

        entries.push(SessionEntry::message(
            entry_id.clone(),
            parent_id.clone(),
            timestamp.clone(),
            message,
        ));
        parent_id = Some(entry_id);
    }

    let current_leaf_id = entries.last().map(|entry| entry.id.clone());
    let mut child: Option<SessionTreeNode> = None;
    for entry in entries.into_iter().rev() {
        let mut node = SessionTreeNode {
            entry,
            children: Vec::new(),
            label: None,
            label_timestamp: None,
        };
        if let Some(child) = child {
            node.children.push(child);
        }
        child = Some(node);
    }

    (child.into_iter().collect(), current_leaf_id)
}

fn transcript_item_from_rust_native(item: CodingAgentSessionTranscriptItem) -> TranscriptItem {
    match item {
        CodingAgentSessionTranscriptItem::User { text } => TranscriptItem::user(text),
        CodingAgentSessionTranscriptItem::Assistant { id, text, done } => {
            TranscriptItem::assistant(id, text, done)
        }
        CodingAgentSessionTranscriptItem::Tool {
            call_id,
            name,
            args,
            result,
            is_error,
        } => TranscriptItem::Tool {
            call_id,
            name,
            args,
            result,
            is_error,
        },
        CodingAgentSessionTranscriptItem::CompactionSummary { summary } => {
            TranscriptItem::assistant("compaction", summary, true)
        }
        CodingAgentSessionTranscriptItem::Diagnostic { message } => TranscriptItem::system(message),
    }
}

pub(super) fn hydrate_session_storage(
    storage: JsonlSessionStorage,
) -> Result<HydratedSession, CliError> {
    let leaf_id = storage
        .get_leaf_id()
        .map_err(|error| CliError::SessionFailure(error.message))?;
    let entries = storage.get_entries();
    let context = session::build_session_context(&entries, leaf_id.as_deref())
        .map_err(|error| CliError::SessionFailure(error.message))?;
    let cumulative_usage = compute_cumulative_usage(&context.messages);
    let choice = session_choice_from_metadata(storage.metadata());
    Ok(HydratedSession {
        choice,
        transcript_items: transcript_items_from_messages(&context.messages),
        leaf_id,
        cumulative_usage,
    })
}

/// Walk every [`AgentMessage::Assistant`] in the session and sum their
/// [`Usage`] blocks into a [`CumulativeUsage`] that mirrors the running
/// totals the [`super::event_bridge::InteractiveEventBridge`] maintains
/// during a live conversation.
fn compute_cumulative_usage(messages: &[AgentMessage]) -> CumulativeUsage {
    let mut acc = CumulativeUsage::default();
    for message in messages {
        if let AgentMessage::Assistant { message, .. } = message {
            let usage = &message.usage;
            acc.input = acc.input.saturating_add(usage.input);
            acc.output = acc.output.saturating_add(usage.output);
            acc.cache_read = acc.cache_read.saturating_add(usage.cache_read);
            acc.cache_write = acc.cache_write.saturating_add(usage.cache_write);
            acc.cost += usage.cost.input
                + usage.cost.output
                + usage.cost.cache_read
                + usage.cost.cache_write;
            // Context tokens come from the *last* message's usage block, same
            // as InteractiveEventBridge which mirrors the latest LLM response.
            acc.last_context_tokens =
                Some(pi_agent_core::compaction::estimate::calculate_context_tokens(usage));
        }
    }
    acc
}

fn transcript_items_from_messages(messages: &[AgentMessage]) -> Vec<TranscriptItem> {
    let mut items = Vec::new();
    // Map from tool_call_id to the arguments declared in the preceding
    // Assistant message, so ToolResult entries can render their original
    // invocation parameters (e.g. `read /path/to/file done`).
    let mut tool_call_args: HashMap<String, serde_json::Value> = HashMap::new();

    for (index, message) in messages.iter().enumerate() {
        match message {
            AgentMessage::UserText { text, .. } => {
                items.push(TranscriptItem::user(text.clone()));
            }
            AgentMessage::Assistant { message, .. } => {
                // Register tool-call arguments for later ToolResult lookup.
                for block in &message.content {
                    if let ContentBlock::ToolCall { id, arguments, .. } = block {
                        tool_call_args.insert(id.clone(), arguments.clone());
                    }
                }

                // Emit one TranscriptItem per content block, merging
                // consecutive Text blocks into a single item.
                let mut pending_text = String::new();
                let flush_pending_text = |pending: &mut String, items: &mut Vec<TranscriptItem>| {
                    let trimmed = pending.trim().to_string();
                    pending.clear();
                    if !trimmed.is_empty() {
                        items.push(TranscriptItem::assistant(
                            format!("assistant_{index}"),
                            trimmed,
                            true,
                        ));
                    }
                };

                for block in &message.content {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            if !pending_text.is_empty() {
                                pending_text.push('\n');
                            }
                            pending_text.push_str(text);
                        }
                        ContentBlock::Thinking { thinking, .. } => {
                            flush_pending_text(&mut pending_text, &mut items);
                            let trimmed = thinking.trim().to_string();
                            if !trimmed.is_empty() {
                                items.push(TranscriptItem::Assistant {
                                    id: format!("assistant_{index}"),
                                    markdown: String::new(),
                                    thinking: trimmed,
                                    done: true,
                                });
                            }
                        }
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            flush_pending_text(&mut pending_text, &mut items);
                            items.push(TranscriptItem::Tool {
                                call_id: id.clone(),
                                name: name.clone(),
                                args: arguments.clone(),
                                result: None,
                                is_error: false,
                            });
                        }
                        ContentBlock::Image { .. } => {
                            // Images are not yet rendered in the transcript.
                        }
                    }
                }
                flush_pending_text(&mut pending_text, &mut items);
            }
            AgentMessage::ToolResult {
                tool_call_id,
                tool_name,
                is_error,
                content,
                ..
            } => {
                let args = tool_call_args
                    .get(tool_call_id)
                    .cloned()
                    .unwrap_or_default();
                // Find the matching tool item from the preceding Assistant
                // message and update it in-place (mirrors `finish_tool` in
                // the live transcript path), avoiding a duplicate header row.
                if let Some(item) = items.iter_mut().rev().find(|item| {
                    matches!(item,
                        TranscriptItem::Tool { call_id, .. } if call_id == tool_call_id
                    )
                }) {
                    if let TranscriptItem::Tool {
                        result: existing,
                        is_error: existing_error,
                        args: existing_args,
                        ..
                    } = item
                    {
                        *existing = Some(tool_result_text(content));
                        *existing_error = *is_error;
                        if !args.is_null() {
                            *existing_args = args;
                        }
                    }
                } else {
                    // Fallback: tool call not seen (e.g. tool executed outside
                    // an assistant message).  Push a standalone item.
                    items.push(TranscriptItem::Tool {
                        call_id: tool_call_id.clone(),
                        name: tool_name.clone(),
                        args,
                        result: Some(tool_result_text(content)),
                        is_error: *is_error,
                    });
                }
            }
            AgentMessage::BashExecution {
                message_id,
                command,
                output,
                exit_code,
                cancelled,
                ..
            } => items.push(TranscriptItem::Tool {
                call_id: message_id.clone(),
                name: "bash".to_string(),
                args: serde_json::json!({ "command": command }),
                result: Some(output.clone()),
                is_error: *cancelled || exit_code.is_some_and(|code| code != 0),
            }),
            AgentMessage::Custom {
                custom_type,
                content,
                display,
                ..
            } if *display => items.push(TranscriptItem::system(format!(
                "{}: {}",
                custom_type,
                tool_result_text(content)
            ))),
            AgentMessage::BranchSummary { summary, .. }
            | AgentMessage::CompactionSummary { summary, .. } => {
                items.push(TranscriptItem::system(summary.clone()))
            }
            AgentMessage::SystemPrompt { .. } | AgentMessage::Custom { .. } => {}
        }
    }

    items
}

/// Extract text-only content from tool-result blocks. Tool results never
/// contain `thinking` blocks (those belong to the assistant message), so
/// this is a plain text concatenation.
fn tool_result_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn export_path_arg(args: &str) -> Option<String> {
    let args = args.trim_start();
    if args.is_empty() {
        return None;
    }

    let first = args.chars().next()?;
    if first == '"' || first == '\'' {
        let closing = args[1..].find(first)?;
        return Some(args[1..1 + closing].to_string());
    }

    let end = args.find(char::is_whitespace).unwrap_or(args.len());
    Some(args[..end].to_string())
}

pub(super) fn default_export_path(cwd: &Path) -> PathBuf {
    let stamp = create_timestamp()
        .replace(':', "-")
        .replace('.', "-")
        .replace('Z', "");
    cwd.join(format!("session-{stamp}.html"))
}

pub(super) fn resolve_command_path(cwd: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

pub(super) fn clone_session_to_sibling(
    source_path: &Path,
    target_cwd: &Path,
    leaf_id: &str,
) -> Result<JsonlSessionStorage, String> {
    let source = JsonlSessionStorage::open(source_path).map_err(|error| error.message)?;
    let entries = source.get_entries();
    let by_id: HashMap<&str, &SessionEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    if !by_id.contains_key(leaf_id) {
        return Err(format!("entry id not found in source session: {leaf_id}"));
    }

    let parent = source_path
        .parent()
        .ok_or_else(|| "source session has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let session_id = create_session_id();
    let timestamp = create_timestamp();
    let filename = format!(
        "{}_{}.jsonl",
        timestamp.replace(':', "_").replace('.', "_"),
        session_id
    );
    let clone_path = parent.join(filename);
    let mut target = JsonlSessionStorage::create(
        &clone_path,
        target_cwd.display().to_string(),
        &session_id,
        timestamp,
        Some(source_path.to_path_buf()),
    )
    .map_err(|error| error.message)?;

    let mut branch = Vec::new();
    let mut current = by_id.get(leaf_id).copied();
    while let Some(entry) = current {
        branch.push(entry.clone());
        current = entry
            .parent_id
            .as_deref()
            .and_then(|parent_id| by_id.get(parent_id).copied());
    }
    branch.reverse();
    for entry in branch {
        target.append_entry(entry).map_err(|error| error.message)?;
    }

    Ok(target)
}

pub(super) fn export_transcript(
    cwd: &Path,
    session_label: &str,
    model_id: &str,
    items: &[TranscriptItem],
    args: &str,
) -> Result<PathBuf, String> {
    let path = export_path_arg(args)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_export_path(cwd));
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
        export_transcript_jsonl(cwd, model_id, items, &path)?;
    } else {
        export_transcript_html(session_label, items, &path)?;
    }
    Ok(path)
}

fn export_transcript_jsonl(
    cwd: &Path,
    model_id: &str,
    items: &[TranscriptItem],
    path: &Path,
) -> Result<(), String> {
    let timestamp = create_timestamp();
    let header = SessionHeader {
        entry_type: "session".to_string(),
        version: 3,
        id: create_session_id(),
        timestamp: timestamp.clone(),
        cwd: cwd.display().to_string(),
        parent_session: None,
    };
    let mut lines = vec![serde_json::to_string(&header).map_err(|error| error.to_string())?];
    let mut existing = HashSet::new();
    let mut parent_id = None;

    for message in exportable_messages(model_id, items) {
        let id = generate_entry_id(&existing);
        existing.insert(id.clone());
        let entry =
            SessionEntry::message(id.clone(), parent_id.clone(), timestamp.clone(), message);
        lines.push(serde_json::to_string(&entry).map_err(|error| error.to_string())?);
        parent_id = Some(id);
    }

    let mut text = lines.join("\n");
    text.push('\n');
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn export_transcript_html(
    session_label: &str,
    items: &[TranscriptItem],
    path: &Path,
) -> Result<(), String> {
    let mut body = String::new();
    for item in items {
        match item {
            TranscriptItem::User { text } => body.push_str(&format!(
                "<section class=\"message user\"><h2>User</h2><pre>{}</pre></section>",
                html_escape(text)
            )),
            TranscriptItem::Assistant { markdown, .. } => body.push_str(&format!(
                "<section class=\"message assistant\"><h2>Assistant</h2><pre>{}</pre></section>",
                html_escape(markdown)
            )),
            TranscriptItem::Tool {
                name,
                result,
                is_error,
                ..
            } => body.push_str(&format!(
                "<section class=\"message tool{}\"><h2>Tool: {}</h2><pre>{}</pre></section>",
                if *is_error { " error" } else { "" },
                html_escape(name),
                html_escape(result.as_deref().unwrap_or(""))
            )),
            TranscriptItem::Error { text } => body.push_str(&format!(
                "<section class=\"message error\"><h2>Error</h2><pre>{}</pre></section>",
                html_escape(text)
            )),
            TranscriptItem::System { .. } => {}
        }
    }

    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>{}</style></head><body><main><h1>{}</h1>{}</main></body></html>",
        html_escape(session_label),
        "body{font-family:system-ui,sans-serif;margin:2rem;background:#101010;color:#f4f4f4}main{max-width:900px;margin:auto}.message{border:1px solid #444;padding:1rem;margin:1rem 0;border-radius:6px}pre{white-space:pre-wrap;font-family:ui-monospace,monospace}.user{border-color:#3b82f6}.assistant{border-color:#10b981}.tool{border-color:#a78bfa}.error{border-color:#ef4444;color:#fecaca}",
        html_escape(session_label),
        body
    );
    std::fs::write(path, html).map_err(|error| error.to_string())
}

fn exportable_messages(model_id: &str, items: &[TranscriptItem]) -> Vec<StoredAgentMessage> {
    let timestamp_ms = timestamp_millis();
    let mut messages = Vec::new();
    for item in items {
        match item {
            TranscriptItem::User { text } => messages.push(StoredAgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
                timestamp: timestamp_ms,
            }),
            TranscriptItem::Assistant { markdown, .. } => {
                if !markdown.trim().is_empty() {
                    messages.push(StoredAgentMessage::Assistant {
                        content: vec![ContentBlock::Text {
                            text: markdown.clone(),
                            text_signature: None,
                        }],
                        api: "interactive".to_string(),
                        provider: "interactive".to_string(),
                        model: model_id.to_string(),
                        response_model: None,
                        response_id: None,
                        usage: StoredUsage::default(),
                        stop_reason: StopReason::Stop,
                        error_message: None,
                        timestamp: timestamp_ms,
                    });
                }
            }
            TranscriptItem::Tool {
                call_id,
                name,
                result,
                is_error,
                ..
            } => messages.push(StoredAgentMessage::ToolResult {
                tool_call_id: call_id.clone(),
                tool_name: name.clone(),
                content: vec![ContentBlock::Text {
                    text: result.clone().unwrap_or_default(),
                    text_signature: None,
                }],
                is_error: *is_error,
                timestamp: timestamp_ms,
            }),
            TranscriptItem::Error { text } => messages.push(StoredAgentMessage::Custom {
                custom_type: "error".to_string(),
                content: vec![ContentBlock::Text {
                    text: text.clone(),
                    text_signature: None,
                }],
                display: true,
                details: None,
                timestamp: timestamp_ms,
            }),
            TranscriptItem::System { .. } => {}
        }
    }
    messages
}

fn timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get a current RFC3339 timestamp string for use in label changes etc.
pub(super) fn current_timestamp() -> String {
    pi_agent_core::session::create_timestamp()
}

fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(super) fn collect_session_choices(session: &Option<SessionRunOptions>) -> Vec<SessionChoice> {
    let Some(session) = session else {
        return Vec::new();
    };
    if !matches!(session.mode, SessionMode::Enabled) {
        return Vec::new();
    }

    let root = match interactive_session_root(session) {
        Ok(root) => root,
        Err(_) => return Vec::new(),
    };
    let cwd = session.cwd.display().to_string();
    let mut choices = rust_native_choices(&root, &session.cwd).unwrap_or_default();
    let repo = JsonlSessionRepo::new(root);
    choices.extend(
        repo.list(Some(&cwd))
            .unwrap_or_default()
            .into_iter()
            .map(session_choice_from_metadata),
    );
    choices.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| match (left.kind, right.kind) {
                (SessionChoiceKind::RustNative, SessionChoiceKind::LegacyJsonl) => {
                    std::cmp::Ordering::Less
                }
                (SessionChoiceKind::LegacyJsonl, SessionChoiceKind::RustNative) => {
                    std::cmp::Ordering::Greater
                }
                _ => left.id.cmp(&right.id),
            })
    });
    choices
}

fn interactive_session_root(session: &SessionRunOptions) -> Result<PathBuf, CliError> {
    match &session.session_dir {
        Some(dir) => Ok(dir.clone()),
        None => crate::session::resolve_session_dir(&session.cwd, None, None),
    }
}

fn rust_native_choices(root: &Path, cwd: &Path) -> Result<Vec<SessionChoice>, CliError> {
    let options = CodingAgentSessionOptions::new()
        .with_cwd(cwd)
        .with_session_log_root(root);
    Ok(CodingAgentSession::list(options.clone())?
        .into_iter()
        .filter_map(|summary| {
            CodingAgentSession::hydrate(options.clone().with_session_id(summary.session_id.clone()))
                .ok()
        })
        .filter(|hydration| hydration_matches_cwd(hydration, cwd))
        .map(|hydration| session_choice_from_rust_native(&hydration))
        .collect())
}

fn hydration_matches_cwd(hydration: &CodingAgentSessionHydration, cwd: &Path) -> bool {
    let expected = normalized_path_string(cwd);
    hydration.cwd.as_deref() == Some(expected.as_str())
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn session_choice_from_rust_native(hydration: &CodingAgentSessionHydration) -> SessionChoice {
    SessionChoice {
        id: hydration.summary.session_id.clone(),
        cwd: hydration.cwd.clone().unwrap_or_default(),
        path: hydration.summary.session_dir.clone(),
        created_at: hydration.summary.created_at.clone(),
        name: None,
        entry_count: hydration.transcript.len(),
        active_leaf_id: hydration.summary.active_leaf_id.clone(),
        kind: SessionChoiceKind::RustNative,
    }
}

fn target_looks_like_rust_native_session_dir(target: &str) -> bool {
    let path = Path::new(target);
    path.is_dir() && path.join("session.json").is_file() && path.join("events.jsonl").is_file()
}

pub(super) fn session_choice_from_metadata(metadata: JsonlSessionMetadata) -> SessionChoice {
    let (name, entry_count, active_leaf_id) = JsonlSessionStorage::open(&metadata.path)
        .map(|storage| {
            let leaf_id = storage.get_leaf_id().ok().flatten();
            let entries = storage.get_entries();
            let name = entries
                .iter()
                .rev()
                .find(|entry| entry.entry_type == "session_info")
                .and_then(|entry| entry.field("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            (name, entries.len(), leaf_id)
        })
        .unwrap_or((None, 0, None));

    SessionChoice {
        id: metadata.id,
        cwd: metadata.cwd,
        path: metadata.path,
        created_at: metadata.created_at,
        name,
        entry_count,
        active_leaf_id,
        kind: SessionChoiceKind::LegacyJsonl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::types::{AssistantMessage, Cost, Usage};

    fn assistant_msg(input: u32, output: u32, total: u32) -> AgentMessage {
        AgentMessage::Assistant {
            message_id: "m".to_string(),
            message: AssistantMessage {
                content: vec![],
                api: "test".to_string(),
                provider: None,
                model: "test".to_string(),
                response_model: None,
                response_id: None,
                usage: Usage {
                    input,
                    output,
                    cache_read: 0,
                    cache_write: 0,
                    total_tokens: total,
                    cost: Cost {
                        input: 0.001 * input as f64,
                        output: 0.002 * output as f64,
                        cache_read: 0.0,
                        cache_write: 0.0,
                    },
                },
                stop_reason: StopReason::Stop,
                error_message: None,
                diagnostics: None,
                timestamp: 0,
            },
        }
    }

    #[test]
    fn empty_returns_default() {
        let usage = compute_cumulative_usage(&[]);
        assert_eq!(usage.input, 0);
        assert_eq!(usage.output, 0);
        assert_eq!(usage.cost, 0.0);
        assert_eq!(usage.last_context_tokens, None);
    }

    #[test]
    fn sums_multiple_assistant_messages() {
        let messages = vec![assistant_msg(100, 50, 150), assistant_msg(200, 80, 280)];
        let usage = compute_cumulative_usage(&messages);
        assert_eq!(usage.input, 300);
        assert_eq!(usage.output, 130);
        assert!((usage.cost - 0.56).abs() < 0.001, "cost: {}", usage.cost);
        // 0.001*100 + 0.002*50 + 0.001*200 + 0.002*80 = 0.1+0.1+0.2+0.16 = 0.56
        // Last message had total=280, which has priority
        assert_eq!(usage.last_context_tokens, Some(280));
    }

    #[test]
    fn prefers_total_tokens_for_context_estimate() {
        let messages = vec![assistant_msg(10, 20, 0)];
        let usage = compute_cumulative_usage(&messages);
        // total_tokens=0, so fallback to sum: 10+20+0+0=30
        assert_eq!(usage.last_context_tokens, Some(30));

        let messages = vec![assistant_msg(10, 20, 50)];
        let usage = compute_cumulative_usage(&messages);
        assert_eq!(usage.last_context_tokens, Some(50));
    }

    #[test]
    fn ignores_non_assistant_messages() {
        let messages = vec![
            AgentMessage::UserText {
                message_id: "u".to_string(),
                text: "hello".to_string(),
            },
            assistant_msg(100, 50, 150),
            AgentMessage::ToolResult {
                message_id: "t".to_string(),
                tool_call_id: "tc1".to_string(),
                tool_name: "read".to_string(),
                is_error: false,
                content: vec![],
            },
        ];
        let usage = compute_cumulative_usage(&messages);
        assert_eq!(usage.input, 100);
        assert_eq!(usage.output, 50);
    }

    #[test]
    fn rust_native_tree_projection_builds_linear_readonly_tree() {
        let hydrated = HydratedSession {
            choice: SessionChoice {
                id: "sess_rust".to_string(),
                cwd: "/work".to_string(),
                path: PathBuf::from("/sessions/sess_rust"),
                created_at: "2026-06-30T00:00:00Z".to_string(),
                name: None,
                entry_count: 3,
                active_leaf_id: None,
                kind: SessionChoiceKind::RustNative,
            },
            transcript_items: vec![
                TranscriptItem::user("hello".to_string()),
                TranscriptItem::assistant("assistant_1".to_string(), "world".to_string(), true),
                TranscriptItem::Tool {
                    call_id: "tool_1".to_string(),
                    name: "read".to_string(),
                    args: serde_json::json!({"path": "README.md"}),
                    result: Some("contents".to_string()),
                    is_error: false,
                },
            ],
            leaf_id: None,
            cumulative_usage: CumulativeUsage::default(),
        };

        let (tree, leaf_id) = rust_native_tree_from_hydrated_session(&hydrated);

        assert_eq!(leaf_id.as_deref(), Some("rust_native_entry_2"));
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].entry.id, "rust_native_entry_0");
        assert_eq!(tree[0].children[0].entry.id, "rust_native_entry_1");
        assert_eq!(
            tree[0].children[0].children[0].entry.id,
            "rust_native_entry_2"
        );
    }

    #[test]
    fn saturating_prevents_overflow() {
        let messages = vec![assistant_msg(u32::MAX, 1, u32::MAX)];
        let usage = compute_cumulative_usage(&messages);
        assert_eq!(usage.input, u32::MAX);
        assert_eq!(usage.output, 1);
        // Second message adds 1 more; saturating keeps it at MAX
        let messages = vec![assistant_msg(1, 0, 0), assistant_msg(u32::MAX, 0, u32::MAX)];
        let usage = compute_cumulative_usage(&messages);
        assert_eq!(usage.input, u32::MAX);
    }

    #[test]
    fn tool_result_updates_existing_item_in_place() {
        // Regression: ToolResult should NOT push a second TranscriptItem; it
        // must update the existing Tool item that was created from the
        // preceding ContentBlock::ToolCall.
        let messages = vec![
            AgentMessage::Assistant {
                message_id: "a1".to_string(),
                message: AssistantMessage {
                    content: vec![ContentBlock::ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: serde_json::json!({"path": "/tmp/x.rs"}),
                        thought_signature: None,
                    }],
                    api: "test".to_string(),
                    provider: None,
                    model: "m".to_string(),
                    response_model: None,
                    response_id: None,
                    usage: Usage::default(),
                    stop_reason: StopReason::ToolUse,
                    error_message: None,
                    diagnostics: None,
                    timestamp: 0,
                },
            },
            AgentMessage::ToolResult {
                message_id: "t1".to_string(),
                tool_call_id: "call-1".to_string(),
                tool_name: "read".to_string(),
                is_error: false,
                content: vec![ContentBlock::Text {
                    text: "line 1\nline 2".to_string(),
                    text_signature: None,
                }],
            },
        ];
        let items = transcript_items_from_messages(&messages);
        // Exactly one Tool item, not two.
        let tool_items: Vec<_> = items
            .iter()
            .filter(|item| matches!(item, TranscriptItem::Tool { .. }))
            .collect();
        assert_eq!(
            tool_items.len(),
            1,
            "expected 1 Tool item, got {}: {tool_items:#?}",
            tool_items.len()
        );
        let tool = &tool_items[0];
        if let TranscriptItem::Tool {
            name,
            result,
            is_error,
            ..
        } = tool
        {
            assert_eq!(name, "read");
            assert!(!is_error);
            assert!(result.is_some(), "result must be populated");
        }
    }

    #[test]
    fn tool_result_without_preceding_tool_call_creates_standalone_item() {
        // Edge case: ToolResult arrives without a matching ToolCall in any
        // previous assistant message.  Should still create one item.
        let messages = vec![AgentMessage::ToolResult {
            message_id: "t1".to_string(),
            tool_call_id: "orphan-1".to_string(),
            tool_name: "grep".to_string(),
            is_error: true,
            content: vec![ContentBlock::Text {
                text: "not found".to_string(),
                text_signature: None,
            }],
        }];
        let items = transcript_items_from_messages(&messages);
        assert_eq!(items.len(), 1, "{items:#?}");
        if let TranscriptItem::Tool { name, is_error, .. } = &items[0] {
            assert_eq!(name, "grep");
            assert!(*is_error);
        }
    }
}
