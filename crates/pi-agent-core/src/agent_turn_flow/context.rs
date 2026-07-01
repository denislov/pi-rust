use std::collections::VecDeque;

use pi_ai::types::AssistantMessage;
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, AgentState};
use crate::types::{
    AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentTool, AgentToolResult,
    ProviderRequestSnapshot,
};

#[derive(Debug, Clone, PartialEq)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCompactionState {
    pub summary: Option<String>,
    pub first_kept_message_id: Option<String>,
    pub tokens_before: Option<u32>,
}

#[derive(Clone)]
pub struct AgentTurnContext {
    pub config: AgentConfig,
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub resources: AgentResources,
    pub steering_queue: VecDeque<AgentMessage>,
    pub follow_up_queue: VecDeque<AgentMessage>,
    pub cancel_token: CancellationToken,
    pub turn: u32,
    pub provider_request: Option<ProviderRequestSnapshot>,
    pub assistant_message: Option<AssistantMessage>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub tool_results: Vec<AgentToolResult>,
    pub runtime_compaction: RuntimeCompactionState,
    pub events: Vec<AgentEvent>,
}

impl AgentTurnContext {
    pub fn from_agent(agent: &Agent) -> Self {
        agent.agent_turn_context_snapshot()
    }

    pub(crate) fn from_state(state: &AgentState) -> Self {
        Self {
            config: state.config.clone(),
            messages: state.messages.clone(),
            tools: state.tools.clone(),
            resources: state.config.resources.clone(),
            steering_queue: state.steering_queue.clone(),
            follow_up_queue: state.follow_up_queue.clone(),
            cancel_token: state.cancel_token.clone(),
            turn: 0,
            provider_request: None,
            assistant_message: None,
            pending_tool_calls: Vec::new(),
            tool_results: Vec::new(),
            runtime_compaction: RuntimeCompactionState::default(),
            events: Vec::new(),
        }
    }
}
