use std::collections::VecDeque;

use pi_ai::types::{AssistantMessage, Context, StreamOptions};
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, AgentState};
use crate::types::{
    AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentTool, AgentToolResult,
    ProviderRequestSnapshot,
};

#[derive(Debug, Clone, PartialEq)]
pub struct PendingToolCall {
    pub index: usize,
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

#[derive(Debug, Clone)]
pub struct AgentTurnProviderRequestOverride {
    pub context: Context,
    pub stream_options: Option<StreamOptions>,
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
    pub provider_request_override: Option<AgentTurnProviderRequestOverride>,
    pub(crate) provider_request_override_consumed: bool,
    pub assistant_message: Option<AssistantMessage>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub tool_results: Vec<AgentToolResult>,
    pub tool_results_all_terminate: bool,
    pub runtime_compaction: RuntimeCompactionState,
    pub max_turns_exceeded: Option<u32>,
    pub should_finish: bool,
    pub has_more_queued_input: bool,
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
            provider_request_override: state.provider_request_override.as_ref().map(|request| {
                AgentTurnProviderRequestOverride {
                    context: request.context.clone(),
                    stream_options: request.stream_options.clone(),
                }
            }),
            provider_request_override_consumed: false,
            assistant_message: None,
            pending_tool_calls: Vec::new(),
            tool_results: Vec::new(),
            tool_results_all_terminate: false,
            runtime_compaction: RuntimeCompactionState::default(),
            max_turns_exceeded: None,
            should_finish: false,
            has_more_queued_input: false,
            events: Vec::new(),
        }
    }

    pub(crate) fn apply_to_state(&self, state: &mut AgentState) {
        let mut config = self.config.clone();
        config.resources = self.resources.clone();

        state.messages = self.messages.clone();
        state.tools = self.tools.clone();
        state.config = config;
        state.cancel_token = self.cancel_token.clone();
        state.steering_queue = self.steering_queue.clone();
        state.follow_up_queue = self.follow_up_queue.clone();

        if self.provider_request_override_consumed {
            state.provider_request_override = None;
        }
    }
}
