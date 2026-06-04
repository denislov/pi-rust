use crate::runtime::{SessionMode, SessionRunOptions};
use crate::session::{
    ActiveSession, ResolvedSessionTarget, append_agent_message, open_active_session,
};
use crate::{CliError, build_agent_config};
use futures::StreamExt;
use pi_agent_core::session::{self, create_timestamp, generate_entry_id};
use pi_agent_core::{Agent, AgentEvent, AgentTool};
use pi_ai::types::{AssistantMessage, ContentBlock, Model};
use std::collections::HashSet;

pub struct PrintModeOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
}

impl PrintModeOptions {
    pub fn new(prompt: impl Into<String>, model: Model) -> Self {
        Self {
            prompt: prompt.into(),
            model,
            api_key: None,
            system_prompt: None,
            max_turns: 5,
            tools: Vec::new(),
            register_builtins: false,
            session: None,
            session_target: None,
            session_name: None,
        }
    }
}

fn assistant_text(message: &AssistantMessage) -> String {
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

pub async fn run_print_mode(options: PrintModeOptions) -> Result<String, CliError> {
    if options.register_builtins {
        pi_ai::providers::register_builtins();
    }

    let config = build_agent_config(
        options.model.clone(),
        options.system_prompt,
        options.max_turns,
        options.api_key,
    );

    let mut active_session: Option<ActiveSession> = None;
    let mut existing_ids: HashSet<String> = HashSet::new();
    let baseline: usize;
    let agent;

    if let Some(ref session_opts) = options.session {
        match session_opts.mode {
            SessionMode::Enabled => {
                let target = options
                    .session_target
                    .clone()
                    .unwrap_or(ResolvedSessionTarget::New);
                let active = open_active_session(target, session_opts)?;

                for entry in active.storage.get_entries() {
                    existing_ids.insert(entry.id);
                }

                let entries = active.storage.get_entries();
                let context = session::build_session_context(&entries, None)
                    .map_err(|e| CliError::SessionFailure(e.message))?;
                baseline = context.messages.len();
                agent = Agent::with_messages(config, context.messages);
                active_session = Some(active);
            }
            SessionMode::Disabled => {
                baseline = 0;
                agent = Agent::new(config);
            }
        }
    } else {
        baseline = 0;
        agent = Agent::new(config);
    }

    for tool in options.tools {
        agent.add_tool(tool);
    }

    let mut stream = agent.prompt(&options.prompt);
    let mut final_message: Option<AssistantMessage> = None;

    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::AgentDone { message } => final_message = Some(message),
            AgentEvent::AgentError { error } => {
                capture_session_messages(
                    &agent,
                    &mut active_session,
                    &mut existing_ids,
                    baseline,
                    &options.session_name,
                )?;
                return Err(CliError::AgentFailure(error));
            }
            _ => {}
        }
    }

    let message = final_message.ok_or_else(|| {
        let _ = capture_session_messages(
            &agent,
            &mut active_session,
            &mut existing_ids,
            baseline,
            &options.session_name,
        );
        CliError::AgentFailure("agent stream ended without completion".to_string())
    })?;

    capture_session_messages(
        &agent,
        &mut active_session,
        &mut existing_ids,
        baseline,
        &options.session_name,
    )?;

    Ok(assistant_text(&message))
}

fn capture_session_messages(
    agent: &Agent,
    active_session: &mut Option<ActiveSession>,
    existing_ids: &mut HashSet<String>,
    baseline: usize,
    session_name: &Option<String>,
) -> Result<(), CliError> {
    let Some(active) = active_session else {
        return Ok(());
    };

    let messages = agent.messages();
    let new_messages = if baseline < messages.len() {
        &messages[baseline..]
    } else {
        &[]
    };

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

    for msg in new_messages {
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
