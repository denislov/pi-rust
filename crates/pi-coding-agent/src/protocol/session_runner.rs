use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::{
    ActiveSession, ResolvedSessionTarget, append_agent_message, open_active_session,
};
use crate::{CliError, build_agent_config};
use futures::StreamExt;
use pi_agent_core::compaction::estimate::estimate_tokens;
use pi_agent_core::compaction::summarize::summarize;
use pi_agent_core::session::{self, create_timestamp, generate_entry_id};
use pi_agent_core::{
    Agent, AgentEvent, AgentMessage, AgentResources, AgentStream, AgentTool, CompactionConfig,
    ThinkingLevel, ToolExecutionMode,
};
use pi_ai::types::{AssistantMessage, ContentBlock, Model};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub struct SessionPromptOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
    pub thinking_level: Option<ThinkingLevel>,
    pub tool_execution: Option<ToolExecutionMode>,
    pub resources: AgentResources,
    pub settings: Option<crate::config::Settings>,
    pub invocation: PromptInvocation,
}

pub struct SessionPromptResult {
    pub final_message: AssistantMessage,
    pub messages: Vec<AgentMessage>,
    pub session_path: Option<PathBuf>,
    pub leaf_id: Option<String>,
}

#[derive(Clone)]
pub struct SessionPromptControlHandle {
    agent: Agent,
}

impl SessionPromptControlHandle {
    pub fn abort(&self) {
        self.agent.abort();
    }

    pub fn steer(&self, text: impl Into<String>) {
        self.agent.steer(text);
    }

    pub fn follow_up(&self, text: impl Into<String>) {
        self.agent.follow_up(text);
    }
}

pub type SessionPromptAbortHandle = SessionPromptControlHandle;

pub struct SpawnedSessionPrompt {
    pub abort: SessionPromptAbortHandle,
    pub events: mpsc::UnboundedReceiver<AgentEvent>,
    pub done: oneshot::Receiver<Result<SessionPromptResult, CliError>>,
}

struct PreparedSessionPrompt {
    agent: Agent,
    active_session: Option<ActiveSession>,
    existing_ids: HashSet<String>,
    session_name: Option<String>,
    invocation: PromptInvocation,
    model: Model,
}

struct StartedSessionPrompt {
    agent: Agent,
    active_session: Option<ActiveSession>,
    existing_ids: HashSet<String>,
    session_name: Option<String>,
    stream: AgentStream,
}

#[derive(Clone, Debug)]
struct PendingCompaction {
    summary: String,
    first_kept_message_id: String,
    tokens_before: u32,
    details: Option<serde_json::Value>,
}

pub(crate) fn assistant_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn run_session_prompt(
    options: SessionPromptOptions,
    on_event: Option<&mut (dyn FnMut(&AgentEvent) -> Result<(), CliError> + Send)>,
) -> Result<SessionPromptResult, CliError> {
    let prepared = prepare_session_prompt(options)?;
    drive_prepared_session_prompt(prepared, on_event).await
}

pub fn spawn_session_prompt(
    options: SessionPromptOptions,
) -> Result<SpawnedSessionPrompt, CliError> {
    let prepared = prepare_session_prompt(options)?;
    let abort = SessionPromptControlHandle {
        agent: prepared.agent.clone(),
    };
    if matches!(prepared.invocation, PromptInvocation::Compact { .. }) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

        tokio::spawn(async move {
            let mut on_event = |event: &AgentEvent| {
                let _ = event_tx.send(event.clone());
                Ok(())
            };
            let result = drive_prepared_session_prompt(prepared, Some(&mut on_event)).await;
            let _ = done_tx.send(result);
        });

        return Ok(SpawnedSessionPrompt {
            abort,
            events: event_rx,
            done: done_rx,
        });
    }

    let started = start_prepared_session_prompt(prepared)?;
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = oneshot::channel();

    tokio::spawn(async move {
        let mut on_event = |event: &AgentEvent| {
            let _ = event_tx.send(event.clone());
            Ok(())
        };
        let result = drive_started_session_prompt(started, Some(&mut on_event)).await;
        let _ = done_tx.send(result);
    });

    Ok(SpawnedSessionPrompt {
        abort,
        events: event_rx,
        done: done_rx,
    })
}

fn prepare_session_prompt(
    options: SessionPromptOptions,
) -> Result<PreparedSessionPrompt, CliError> {
    let SessionPromptOptions {
        prompt: _,
        model,
        api_key,
        system_prompt,
        max_turns,
        tools,
        register_builtins,
        session,
        session_target,
        session_name,
        thinking_level,
        tool_execution,
        resources,
        settings,
        invocation,
    } = options;

    if register_builtins {
        pi_ai::providers::register_builtins();
    }

    let mut config = build_agent_config(
        model.clone(),
        system_prompt,
        max_turns,
        api_key,
        thinking_level,
        tool_execution,
        resources,
        settings.as_ref(),
    );
    if matches!(
        session.as_ref().map(|session| &session.mode),
        Some(SessionMode::Enabled)
    ) && settings.is_none()
    {
        config.compaction = Some(CompactionConfig::default());
    }

    let mut active_session: Option<ActiveSession> = None;
    let mut existing_ids: HashSet<String> = HashSet::new();
    let agent;

    if let Some(ref session_opts) = session {
        match session_opts.mode {
            SessionMode::Enabled => {
                let target = session_target.clone().unwrap_or(ResolvedSessionTarget::New);
                let active = open_active_session(target, session_opts)?;

                for entry in active.storage.get_entries() {
                    existing_ids.insert(entry.id);
                }

                let entries = active.storage.get_entries();
                let context = session::build_session_context(&entries, None)
                    .map_err(|e| CliError::SessionFailure(e.message))?;
                agent = Agent::with_messages(config, context.messages);
                active_session = Some(active);
            }
            SessionMode::Disabled => {
                agent = Agent::new(config);
            }
        }
    } else {
        agent = Agent::new(config);
    }

    for tool in tools {
        agent.add_tool(tool);
    }

    Ok(PreparedSessionPrompt {
        agent,
        active_session,
        existing_ids,
        session_name,
        invocation,
        model,
    })
}

fn start_prepared_session_prompt(
    prepared: PreparedSessionPrompt,
) -> Result<StartedSessionPrompt, CliError> {
    let stream = match &prepared.invocation {
        PromptInvocation::Text(text) if !text.is_empty() => prepared.agent.prompt(text),
        PromptInvocation::Content(content) if !content.is_empty() => {
            let message_id = format!("user_{}", prepared.agent.messages().len());
            prepared.agent.add_message(AgentMessage::Custom {
                message_id,
                custom_type: "input".into(),
                content: content.clone(),
                display: true,
                details: None,
                timestamp: 0,
            });
            prepared.agent.run().map_err(CliError::AgentFailure)?
        }
        PromptInvocation::Content(_) => {
            return Err(CliError::MissingPrompt);
        }
        PromptInvocation::Text(_) => {
            return Err(CliError::MissingPrompt);
        }
        PromptInvocation::Compact { .. } => {
            return Err(CliError::AgentFailure(
                "manual compaction must be driven directly".to_string(),
            ));
        }
        PromptInvocation::Skill {
            name,
            additional_instructions,
        } => prepared
            .agent
            .skill(name, additional_instructions.as_deref())
            .map_err(CliError::AgentFailure)?,
        PromptInvocation::PromptTemplate { name, args } => prepared
            .agent
            .prompt_from_template(name, args)
            .map_err(CliError::AgentFailure)?,
    };

    Ok(StartedSessionPrompt {
        agent: prepared.agent,
        active_session: prepared.active_session,
        existing_ids: prepared.existing_ids,
        session_name: prepared.session_name,
        stream,
    })
}

async fn drive_prepared_session_prompt(
    prepared: PreparedSessionPrompt,
    on_event: Option<&mut (dyn FnMut(&AgentEvent) -> Result<(), CliError> + Send)>,
) -> Result<SessionPromptResult, CliError> {
    if matches!(prepared.invocation, PromptInvocation::Compact { .. }) {
        return drive_manual_compaction(prepared, on_event).await;
    }
    let started = start_prepared_session_prompt(prepared)?;
    drive_started_session_prompt(started, on_event).await
}

async fn drive_manual_compaction(
    mut prepared: PreparedSessionPrompt,
    mut on_event: Option<&mut (dyn FnMut(&AgentEvent) -> Result<(), CliError> + Send)>,
) -> Result<SessionPromptResult, CliError> {
    let Some(active) = prepared.active_session.as_mut() else {
        return Err(CliError::AgentFailure(
            "Nothing to compact (no active session)".to_string(),
        ));
    };

    let messages = prepared.agent.messages();
    if messages.len() < 2 {
        return Err(CliError::AgentFailure(
            "Nothing to compact (no messages yet)".to_string(),
        ));
    }

    let tokens_before = estimate_tokens(&messages);
    let first_kept_index = messages.len() - 1;
    let to_summarize = &messages[..first_kept_index];
    let first_kept_message_id = agent_message_id(&messages[first_kept_index]).to_string();
    if to_summarize.is_empty() {
        return Err(CliError::AgentFailure(
            "Nothing to compact (no compactable history)".to_string(),
        ));
    }

    let custom_instructions = match &prepared.invocation {
        PromptInvocation::Compact {
            custom_instructions,
        } => custom_instructions.as_deref(),
        _ => None,
    };
    let stream_options = prepared.agent.provider_request_snapshot().1;
    let summary = summarize(
        &prepared.model,
        to_summarize,
        custom_instructions,
        stream_options,
        None,
    )
    .await
    .map_err(|error| CliError::AgentFailure(error.to_string()))?;

    if let Some(sink) = on_event.as_mut() {
        sink(&AgentEvent::SessionCompacted {
            summary: summary.clone(),
            first_kept_message_id: first_kept_message_id.clone(),
            tokens_before,
            details: None,
        })?;
    }

    let current_leaf = active
        .storage
        .get_leaf_id()
        .map_err(|e| CliError::SessionFailure(e.message))?;
    let entry_id = generate_entry_id(&prepared.existing_ids);
    prepared.existing_ids.insert(entry_id.clone());
    let timestamp = create_timestamp();
    let entry = pi_agent_core::session::SessionEntry::compaction(
        entry_id,
        current_leaf,
        timestamp,
        summary.clone(),
        first_kept_message_id,
        tokens_before,
        None,
        false,
    );
    active
        .storage
        .append_entry(entry)
        .map_err(|e| CliError::SessionFailure(e.message))?;

    let session_path = Some(active.storage.path().to_path_buf());
    let leaf_id = active
        .storage
        .get_leaf_id()
        .map_err(|error| CliError::SessionFailure(error.message))?;

    let mut final_message = AssistantMessage::empty(&prepared.model.api, &prepared.model.id);
    final_message.content = vec![ContentBlock::Text {
        text: summary.clone(),
        text_signature: None,
    }];

    let mut compacted_messages = Vec::with_capacity(1 + messages.len() - first_kept_index);
    compacted_messages.push(AgentMessage::CompactionSummary {
        message_id: format!("compaction_{tokens_before}"),
        summary,
        tokens_before,
    });
    compacted_messages.extend_from_slice(&messages[first_kept_index..]);
    prepared.agent.replace_messages(compacted_messages);

    Ok(SessionPromptResult {
        final_message,
        messages: prepared.agent.messages(),
        session_path,
        leaf_id,
    })
}

async fn drive_started_session_prompt(
    mut started: StartedSessionPrompt,
    mut on_event: Option<&mut (dyn FnMut(&AgentEvent) -> Result<(), CliError> + Send)>,
) -> Result<SessionPromptResult, CliError> {
    let mut final_message: Option<AssistantMessage> = None;
    let mut pending_compactions = Vec::new();

    while let Some(event) = started.stream.next().await {
        if let Some(sink) = on_event.as_mut() {
            sink(&event)?;
        }

        match event {
            AgentEvent::AgentDone { message } => final_message = Some(message),
            AgentEvent::AgentError { error } => {
                capture_session_messages(
                    &started.agent,
                    &mut started.active_session,
                    &mut started.existing_ids,
                    &started.session_name,
                    &pending_compactions,
                )?;
                return Err(CliError::AgentFailure(error));
            }
            AgentEvent::SessionCompacted {
                summary,
                first_kept_message_id,
                tokens_before,
                details,
            } => pending_compactions.push(PendingCompaction {
                summary,
                first_kept_message_id,
                tokens_before,
                details,
            }),
            _ => {}
        }
    }

    let final_message = final_message.ok_or_else(|| {
        let _ = capture_session_messages(
            &started.agent,
            &mut started.active_session,
            &mut started.existing_ids,
            &started.session_name,
            &pending_compactions,
        );
        CliError::AgentFailure("agent stream ended without completion".to_string())
    })?;

    capture_session_messages(
        &started.agent,
        &mut started.active_session,
        &mut started.existing_ids,
        &started.session_name,
        &pending_compactions,
    )?;

    let (session_path, leaf_id) = match &started.active_session {
        Some(active) => (
            Some(active.storage.path().to_path_buf()),
            active
                .storage
                .get_leaf_id()
                .map_err(|error| CliError::SessionFailure(error.message))?,
        ),
        None => (None, None),
    };

    Ok(SessionPromptResult {
        final_message,
        messages: started.agent.messages(),
        session_path,
        leaf_id,
    })
}

fn capture_session_messages(
    agent: &Agent,
    active_session: &mut Option<ActiveSession>,
    existing_ids: &mut HashSet<String>,
    session_name: &Option<String>,
    pending_compactions: &[PendingCompaction],
) -> Result<(), CliError> {
    let Some(active) = active_session else {
        return Ok(());
    };

    let messages = agent.messages();
    let new_messages: Vec<AgentMessage> = messages
        .iter()
        .filter(|msg| {
            !matches!(msg, AgentMessage::CompactionSummary { .. })
                && !existing_ids.contains(agent_message_id(msg))
        })
        .cloned()
        .collect();

    let current_leaf = active
        .storage
        .get_leaf_id()
        .map_err(|e| CliError::SessionFailure(e.message))?;

    let timestamp = create_timestamp();
    let mut prev_parent = current_leaf;
    if let Some(name) = session_name {
        let entry_id = generate_entry_id(existing_ids);
        existing_ids.insert(entry_id.clone());
        let session_info_entry = pi_agent_core::session::SessionEntry::session_info(
            entry_id.clone(),
            prev_parent.clone(),
            timestamp.clone(),
            name.clone(),
        );
        active
            .storage
            .append_entry(session_info_entry)
            .map_err(|e| CliError::SessionFailure(e.message))?;
        prev_parent = Some(entry_id);
    }

    for compaction in pending_compactions {
        let entry_id = generate_entry_id(existing_ids);
        existing_ids.insert(entry_id.clone());
        let entry = pi_agent_core::session::SessionEntry::compaction(
            entry_id.clone(),
            prev_parent.clone(),
            timestamp.clone(),
            compaction.summary.clone(),
            compaction.first_kept_message_id.clone(),
            compaction.tokens_before,
            compaction.details.clone(),
            false,
        );
        active
            .storage
            .append_entry(entry)
            .map_err(|e| CliError::SessionFailure(e.message))?;
        prev_parent = Some(entry_id);
    }

    for msg in &new_messages {
        let entry_id = generate_entry_id(existing_ids);
        existing_ids.insert(entry_id.clone());
        append_agent_message(
            &mut active.storage,
            msg,
            entry_id.clone(),
            prev_parent.clone(),
            timestamp.clone(),
        )?;
        prev_parent = Some(entry_id);
    }

    Ok(())
}

fn agent_message_id(message: &AgentMessage) -> &str {
    match message {
        AgentMessage::UserText { message_id, .. }
        | AgentMessage::Assistant { message_id, .. }
        | AgentMessage::ToolResult { message_id, .. }
        | AgentMessage::SystemPrompt { message_id, .. }
        | AgentMessage::CompactionSummary { message_id, .. }
        | AgentMessage::BashExecution { message_id, .. }
        | AgentMessage::Custom { message_id, .. }
        | AgentMessage::BranchSummary { message_id, .. } => message_id,
    }
}
