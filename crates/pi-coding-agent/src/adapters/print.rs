use crate::api::operation::{CodingAgentOperation, CodingAgentOperationOutcome};
use crate::app::bootstrap::{PromptInvocation, SessionRunOptions};
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::app::session::{ResolvedSessionTarget, open_headless_prompt_session};
use crate::runtime::facade::{CodingSessionError, PromptTurnOptions, PromptTurnOutcome};
use pi_agent_core::api::agent::ThinkingLevel;
use pi_agent_core::api::resources::AgentResources;
use pi_agent_core::api::tool::{AgentTool, ToolExecutionMode};
use pi_ai::api::client::AiClient;
use pi_ai::api::model::Model;

pub struct PrintModeOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub ai_client: Option<AiClient>,
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
            ai_client: None,
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

impl From<PromptRunOptions> for PrintModeOptions {
    fn from(options: PromptRunOptions) -> Self {
        Self {
            prompt: options.prompt,
            model: options.model,
            api_key: options.api_key,
            system_prompt: options.system_prompt,
            max_turns: options.max_turns,
            tools: options.tools,
            register_builtins: options.register_builtins,
            ai_client: options.ai_client,
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
    run_print_prompt_options(options).await
}

pub(crate) async fn run_print_prompt_options(
    options: PromptRunOptions,
) -> Result<String, CliError> {
    let outcome = run_print_mode_with_coding_session(options).await?;
    print_text_from_prompt_outcome(outcome)
}

fn session_prompt_options_from_print_options(options: PrintModeOptions) -> PromptRunOptions {
    PromptRunOptions {
        prompt: options.prompt,
        model: options.model,
        api_key: options.api_key,
        auth_diagnostics: Vec::new(),
        system_prompt: options.system_prompt,
        max_turns: options.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
        ai_client: options.ai_client,
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
    options: PromptRunOptions,
) -> Result<PromptTurnOutcome, CliError> {
    let mut session = open_headless_prompt_session(&options).await?;
    let prompt_options = PromptTurnOptions::from_prompt_run_options(options);

    let outcome = match session
        .run(CodingAgentOperation::Prompt(prompt_options))
        .await?
    {
        CodingAgentOperationOutcome::Prompt(outcome) => outcome,
        _ => unreachable!("prompt operation returned a different public outcome"),
    };
    Ok(outcome)
}

fn print_text_from_prompt_outcome(outcome: PromptTurnOutcome) -> Result<String, CliError> {
    match outcome {
        PromptTurnOutcome::Success { final_text, .. } => Ok(final_text),
        PromptTurnOutcome::Aborted { reason, .. } => Err(CliError::SessionFailure(reason)),
        PromptTurnOutcome::Failed { error, .. } => Err(print_cli_error_from_prompt_error(error)),
    }
}

fn print_cli_error_from_prompt_error(error: CodingSessionError) -> CliError {
    match error {
        CodingSessionError::Provider { message } => CliError::AgentFailure(message),
        CodingSessionError::Flow { message } => {
            match message.strip_prefix("flow node 'run_agent_turn' failed: provider error: ") {
                Some(provider_message) => CliError::AgentFailure(provider_message.into()),
                None => CliError::SessionFailure(message),
            }
        }
        other => CliError::from(other),
    }
}
