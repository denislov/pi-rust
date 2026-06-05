use crate::{CliArgs, CliError};
use pi_agent_core::{AgentConfig, AgentResources, AgentTool, ThinkingLevel, ToolExecutionMode};
use pi_ai::types::{Model, StreamOptions};
use std::path::PathBuf;

pub const DEFAULT_MODEL_ID: &str = "claude-sonnet-4-5";
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

#[derive(Clone, Debug)]
pub enum SessionMode {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct SessionRunOptions {
    pub mode: SessionMode,
    pub cwd: PathBuf,
    pub session_dir: Option<PathBuf>,
}

impl SessionRunOptions {
    pub fn disabled(cwd: PathBuf) -> Self {
        Self {
            mode: SessionMode::Disabled,
            cwd,
            session_dir: None,
        }
    }

    pub fn enabled(cwd: PathBuf) -> Self {
        Self {
            mode: SessionMode::Enabled,
            cwd,
            session_dir: None,
        }
    }
}

#[derive(Clone)]
pub struct CliRunOptions {
    pub model_override: Option<Model>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub session: SessionRunOptions,
}

impl Default for CliRunOptions {
    fn default() -> Self {
        Self {
            model_override: None,
            tools: Vec::new(),
            register_builtins: true,
            session: SessionRunOptions::disabled(PathBuf::from(".")),
        }
    }
}

pub fn select_model(args: &CliArgs, model_override: Option<Model>) -> Result<Model, CliError> {
    if let Some(model_id) = &args.model {
        return pi_ai::lookup_model(model_id)
            .ok_or_else(|| CliError::UnknownModel(model_id.clone()));
    }

    if let Some(model) = model_override {
        return Ok(model);
    }

    pi_ai::lookup_model(DEFAULT_MODEL_ID)
        .ok_or_else(|| CliError::UnknownModel(DEFAULT_MODEL_ID.to_string()))
}

pub fn build_agent_config(
    model: Model,
    system_prompt: Option<String>,
    max_turns: u32,
    api_key: Option<String>,
    thinking_level: Option<ThinkingLevel>,
    tool_execution: Option<ToolExecutionMode>,
    resources: AgentResources,
) -> AgentConfig {
    let stream_options = api_key.map(|api_key| StreamOptions {
        api_key: Some(api_key),
        ..Default::default()
    });
    let mut config = AgentConfig::new(model);
    config.system_prompt = Some(system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()));
    config.max_turns = max_turns;
    config.stream_options = stream_options;
    if let Some(tl) = thinking_level {
        config.thinking_level = tl;
    }
    if let Some(te) = tool_execution {
        config.tool_execution = te;
    }
    config.resources = resources;
    config
}

#[derive(Clone, Debug)]
pub enum PromptInvocation {
    Text(String),
    Skill {
        name: String,
        additional_instructions: Option<String>,
    },
    PromptTemplate {
        name: String,
        args: Vec<String>,
    },
}
