use crate::CliError;
use crate::protocol::session_runner::{SessionPromptOptions, assistant_text, run_session_prompt};
use crate::runtime::{PromptInvocation, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use pi_agent_core::{AgentResources, AgentTool, ThinkingLevel, ToolExecutionMode};
use pi_ai::types::Model;

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
    let result = run_session_prompt(
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
        },
        None,
    )
    .await?;
    Ok(assistant_text(&result.final_message))
}
