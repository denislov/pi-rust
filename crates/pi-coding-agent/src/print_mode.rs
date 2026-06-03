use crate::{CliError, build_agent_config};
use futures::StreamExt;
use pi_agent_core::{Agent, AgentEvent, AgentTool};
use pi_ai::types::{AssistantMessage, ContentBlock, Model};

pub struct PrintModeOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
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
        options.model,
        options.system_prompt,
        options.max_turns,
        options.api_key,
    );
    let agent = Agent::new(config);
    for tool in options.tools {
        agent.add_tool(tool);
    }

    let mut stream = agent.prompt(&options.prompt);
    let mut final_message: Option<AssistantMessage> = None;

    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::AgentDone { message } => final_message = Some(message),
            AgentEvent::AgentError { error } => return Err(CliError::AgentFailure(error)),
            _ => {}
        }
    }

    let message = final_message.ok_or_else(|| {
        CliError::AgentFailure("agent stream ended without completion".to_string())
    })?;
    Ok(assistant_text(&message))
}
