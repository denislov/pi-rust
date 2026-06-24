use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use pi_agent_core::session::{
    JsonlSessionMetadata, JsonlSessionRepo, JsonlSessionStorage, SessionEntry, SessionHeader,
    StoredAgentMessage, StoredUsage, create_session_id, create_timestamp, generate_entry_id,
};
use pi_ai::types::{ContentBlock, StopReason};

use crate::interactive::transcript::TranscriptItem;
use crate::runtime::{SessionMode, SessionRunOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionChoice {
    pub(super) id: String,
    pub(super) cwd: String,
    pub(super) path: PathBuf,
    pub(super) created_at: String,
    pub(super) name: Option<String>,
    pub(super) entry_count: usize,
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

    let root = match &session.session_dir {
        Some(dir) => dir.clone(),
        None => match crate::session::resolve_session_dir(&session.cwd, None, None) {
            Ok(dir) => dir,
            Err(_) => return Vec::new(),
        },
    };
    let repo = JsonlSessionRepo::new(root);
    let cwd = session.cwd.display().to_string();
    repo.list(Some(&cwd))
        .unwrap_or_default()
        .into_iter()
        .map(session_choice_from_metadata)
        .collect()
}

pub(super) fn session_choice_from_metadata(metadata: JsonlSessionMetadata) -> SessionChoice {
    let (name, entry_count) = JsonlSessionStorage::open(&metadata.path)
        .map(|storage| {
            let entries = storage.get_entries();
            let name = entries
                .iter()
                .rev()
                .find(|entry| entry.entry_type == "session_info")
                .and_then(|entry| entry.field("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            (name, entries.len())
        })
        .unwrap_or((None, 0));

    SessionChoice {
        id: metadata.id,
        cwd: metadata.cwd,
        path: metadata.path,
        created_at: metadata.created_at,
        name,
        entry_count,
    }
}
