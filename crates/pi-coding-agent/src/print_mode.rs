use crate::CliError;
use crate::coding_session::{
    CodingAgentSession, CodingAgentSessionOptions, CodingSessionError, PromptTurnOptions,
    PromptTurnOutcome,
};
use crate::protocol::session_runner::{SessionPromptOptions, assistant_text, run_session_prompt};
use crate::runtime::{PromptInvocation, SessionMode, SessionRunOptions};
use crate::session::{ResolvedSessionTarget, resolve_session_dir};
use pi_agent_core::{AgentResources, AgentTool, ThinkingLevel, ToolExecutionMode};
use pi_ai::types::Model;
use std::path::PathBuf;

pub struct PrintModeOptions {
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

impl PrintModeOptions {
    pub fn new(prompt: impl Into<String>, model: Model) -> Self {
        Self {
            prompt: prompt.into(),
            model,
            api_key: None,
            system_prompt: None,
            max_turns: None,
            tools: Vec::new(),
            register_builtins: false,
            session: None,
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text(String::new()),
        }
    }
}

impl From<SessionPromptOptions> for PrintModeOptions {
    fn from(options: SessionPromptOptions) -> Self {
        Self {
            prompt: options.prompt,
            model: options.model,
            api_key: options.api_key,
            system_prompt: options.system_prompt,
            max_turns: options.max_turns,
            tools: options.tools,
            register_builtins: options.register_builtins,
            session: options.session,
            session_target: options.session_target,
            session_name: options.session_name,
            thinking_level: options.thinking_level,
            tool_execution: options.tool_execution,
            resources: options.resources,
            settings: options.settings,
            invocation: options.invocation,
        }
    }
}

pub async fn run_print_mode(options: PrintModeOptions) -> Result<String, CliError> {
    let options = session_prompt_options_from_print_options(options);
    match run_print_mode_with_coding_session(options).await? {
        PrintModeRoute::Handled(text) => Ok(text),
        PrintModeRoute::Legacy(options) => {
            let result = run_session_prompt(options, None).await?;
            Ok(assistant_text(&result.final_message))
        }
    }
}

fn session_prompt_options_from_print_options(options: PrintModeOptions) -> SessionPromptOptions {
    SessionPromptOptions {
        prompt: options.prompt,
        model: options.model,
        api_key: options.api_key,
        system_prompt: options.system_prompt,
        max_turns: options.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
        session: options.session,
        session_target: options.session_target,
        session_name: options.session_name,
        thinking_level: options.thinking_level,
        tool_execution: options.tool_execution,
        resources: options.resources,
        settings: options.settings,
        invocation: options.invocation,
    }
}

async fn run_print_mode_with_coding_session(
    options: SessionPromptOptions,
) -> Result<PrintModeRoute, CliError> {
    let Some(session_options) = options.session.as_ref() else {
        return Ok(PrintModeRoute::Legacy(options));
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return Ok(PrintModeRoute::Legacy(options));
    }

    let session_root = print_coding_session_root(session_options)?;
    let session_options = CodingAgentSessionOptions::new().with_session_log_root(session_root);

    let mut session =
        open_print_coding_session(session_options, options.session_target.as_ref()).await?;
    let prompt_options = PromptTurnOptions::from_session_prompt_options(options);

    let outcome = session.prompt(prompt_options).await?;
    Ok(PrintModeRoute::Handled(print_text_from_prompt_outcome(
        outcome,
    )?))
}

async fn open_print_coding_session(
    options: CodingAgentSessionOptions,
    target: Option<&ResolvedSessionTarget>,
) -> Result<CodingAgentSession, CliError> {
    match target.unwrap_or(&ResolvedSessionTarget::New) {
        ResolvedSessionTarget::New => Ok(CodingAgentSession::create(options).await?),
        ResolvedSessionTarget::OpenTarget(session_id) => {
            Ok(CodingAgentSession::open(options.with_session_id(session_id.clone())).await?)
        }
        ResolvedSessionTarget::OpenOrCreateId(session_id) => Ok(
            CodingAgentSession::open_or_create(options.with_session_id(session_id.clone())).await?,
        ),
        ResolvedSessionTarget::ContinueMostRecent => {
            let session_id = CodingAgentSession::list(options.clone())?
                .into_iter()
                .next()
                .map(|summary| summary.session_id)
                .ok_or_else(|| CodingSessionError::Session {
                    message: "no previous session to continue".into(),
                })?;
            Ok(CodingAgentSession::open(options.with_session_id(session_id)).await?)
        }
        ResolvedSessionTarget::ForkTarget(_) => Err(CodingSessionError::UnsupportedCapability {
            capability: "Rust-native session fork".into(),
        }
        .into()),
    }
}

fn print_coding_session_root(options: &SessionRunOptions) -> Result<PathBuf, CliError> {
    match options.session_dir.as_ref() {
        Some(root) => Ok(root.clone()),
        None => resolve_session_dir(&options.cwd, None, None),
    }
}

fn print_text_from_prompt_outcome(outcome: PromptTurnOutcome) -> Result<String, CliError> {
    match outcome {
        PromptTurnOutcome::Success { final_text, .. } => Ok(final_text),
        PromptTurnOutcome::Aborted { reason, .. } => Err(CliError::SessionFailure(reason)),
        PromptTurnOutcome::Failed { error, .. } => Err(CliError::from(error)),
    }
}

enum PrintModeRoute {
    Handled(String),
    Legacy(SessionPromptOptions),
}
