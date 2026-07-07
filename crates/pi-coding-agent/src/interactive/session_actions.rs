use std::path::{Path, PathBuf};

#[cfg(test)]
use pi_agent_core::transcript::{SessionEntry, StoredAgentMessage, StoredUsage};
use pi_agent_core::transcript::{SessionTreeNode, create_timestamp};
#[cfg(test)]
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
    hydrate_rust_native_session_target(&root, &session_options.cwd, target)
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
        cumulative_usage: CumulativeUsage {
            input: hydration.usage.input,
            output: hydration.usage.output,
            cache_read: hydration.usage.cache_read,
            cache_write: hydration.usage.cache_write,
            cost: hydration.usage.cost,
            last_context_tokens: hydration.usage.last_context_tokens,
        },
    }
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

pub(super) fn rust_native_tree_for_choice(
    choice: &SessionChoice,
) -> Result<(Vec<SessionTreeNode>, Option<String>), CliError> {
    let tree = CodingAgentSession::tree_view(rust_native_choice_options(choice))?;
    Ok((tree.tree, tree.active_leaf_id))
}

#[cfg(test)]
fn rust_native_tree_from_hydrated_session(
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
        CodingAgentSessionTranscriptItem::BranchSummary { summary } => {
            TranscriptItem::assistant("branch_summary", summary, true)
        }
        CodingAgentSessionTranscriptItem::Delegation {
            target_kind,
            target_id,
            task,
            status,
            summary,
            ..
        } => {
            let target_kind = match target_kind {
                crate::coding_session::ProfileKind::Agent => "agent",
                crate::coding_session::ProfileKind::Team => "team",
            };
            let mut text =
                format!("Delegation: {target_kind} {target_id}\nStatus: {status}\nTask: {task}");
            if let Some(summary) = summary.filter(|summary| !summary.trim().is_empty()) {
                text.push_str("\nSummary: ");
                text.push_str(&summary);
            }
            TranscriptItem::system(text)
        }
        CodingAgentSessionTranscriptItem::Diagnostic { message } => TranscriptItem::system(message),
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
    let stamp = create_timestamp().replace([':', '.'], "-").replace('Z', "");
    cwd.join(format!("session-{stamp}.html"))
}

pub(super) fn export_rust_native_choice(
    choice: &SessionChoice,
    cwd: &Path,
    args: &str,
) -> Result<PathBuf, String> {
    if choice.kind != SessionChoiceKind::RustNative {
        return Err("session choice is not Rust-native".into());
    }
    let path = resolve_export_path(cwd, args);
    CodingAgentSession::export_session_html(rust_native_choice_options(choice), &path)
        .map_err(|error| error.to_string())
}

pub(super) fn export_transcript(
    cwd: &Path,
    session_label: &str,
    model_id: &str,
    items: &[TranscriptItem],
    args: &str,
) -> Result<PathBuf, String> {
    let path = resolve_export_path(cwd, args);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let _ = model_id;
    if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
        return Err("JSONL session export is no longer supported".to_string());
    }
    export_transcript_html(session_label, items, &path)?;
    Ok(path)
}

fn resolve_export_path(cwd: &Path, args: &str) -> PathBuf {
    let path = export_path_arg(args)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_export_path(cwd));
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
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

/// Get a current RFC3339 timestamp string for use in label changes etc.
pub(super) fn current_timestamp() -> String {
    pi_agent_core::transcript::create_timestamp()
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
    let mut choices = rust_native_choices(&root, &session.cwd).unwrap_or_default();
    choices.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn export_rust_native_choice_uses_session_owned_export() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("workspace");
        std::fs::create_dir_all(&cwd).unwrap();
        let root = temp.path().join("sessions");
        let session_id = "sess_interactive_export";
        let _session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id(session_id)
                .with_cwd(&cwd)
                .with_session_log_root(&root),
        )
        .await
        .unwrap();
        let choice = SessionChoice {
            id: session_id.into(),
            cwd: normalized_path_string(&cwd),
            path: root.join(session_id),
            created_at: "2026-07-01T00:00:00Z".into(),
            name: None,
            entry_count: 0,
            active_leaf_id: None,
            kind: SessionChoiceKind::RustNative,
        };

        let exported = export_rust_native_choice(&choice, &cwd, "export/session.html").unwrap();

        assert_eq!(exported, cwd.join("export/session.html"));
        let html = std::fs::read_to_string(exported).unwrap();
        assert!(html.contains("<!doctype html>"), "{html}");
        assert!(html.contains(session_id), "{html}");
        assert!(html.contains("Workspace"), "{html}");
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
}
