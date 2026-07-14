use crate::runtime::{PromptInvocation, SessionRunOptions};
use crate::session::ResolvedSessionTarget;
use pi_agent_core::{AgentResources, AgentTool, ThinkingLevel, ToolExecutionMode};
use pi_ai::types::{AssistantMessage, ContentBlock, Model, ProviderAuthDiagnostic};

pub struct PromptRunOptions {
    pub prompt: String,
    pub model: Model,
    pub api_key: Option<String>,
    pub auth_diagnostics: Vec<ProviderAuthDiagnostic>,
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
