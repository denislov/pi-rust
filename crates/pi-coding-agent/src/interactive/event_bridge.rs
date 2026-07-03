use crate::coding_session::CodingAgentEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    AgentStarted,
    TurnStarted,
    AssistantDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    AssistantDone,
    ToolStarted {
        call_id: String,
        name: String,
        args: serde_json::Value,
    },
    ToolFinished {
        call_id: String,
        result: String,
        is_error: bool,
    },
    ToolUpdated {
        call_id: String,
        result: String,
    },
    AgentError {
        error: String,
    },
    CompactionNotice {
        summary: String,
    },
    UsageUpdate {
        input: u32,
        output: u32,
        cache_read: u32,
        cache_write: u32,
        cost: f64,
        /// Estimated context tokens from the last assistant usage;
        /// `None` means unknown (e.g. right after compaction).
        context_tokens: Option<u32>,
    },
}

#[derive(Debug, Default)]
pub struct CodingEventBridge {
    total_input: u32,
    total_output: u32,
    total_cache_read: u32,
    total_cache_write: u32,
    total_cost: f64,
}

impl CodingEventBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, event: &CodingAgentEvent) -> Vec<UiEvent> {
        match event {
            CodingAgentEvent::AgentTurnStarted { .. } => vec![UiEvent::TurnStarted],
            CodingAgentEvent::AssistantMessageDelta { text, .. } => {
                vec![UiEvent::AssistantDelta { text: text.clone() }]
            }
            CodingAgentEvent::AssistantThinkingDelta { text, .. } => {
                vec![UiEvent::ThinkingDelta { text: text.clone() }]
            }
            CodingAgentEvent::AssistantMessageCompleted { .. } => vec![UiEvent::AssistantDone],
            CodingAgentEvent::ToolCallStarted {
                tool_call_id,
                name,
                arguments_json,
                ..
            } => vec![UiEvent::ToolStarted {
                call_id: tool_call_id.clone(),
                name: name.clone(),
                args: parse_tool_arguments(arguments_json),
            }],
            CodingAgentEvent::ToolCallUpdated {
                tool_call_id,
                message,
                ..
            } => vec![UiEvent::ToolUpdated {
                call_id: tool_call_id.clone(),
                result: message.clone(),
            }],
            CodingAgentEvent::ToolCallCompleted {
                tool_call_id,
                summary,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: summary.clone(),
                is_error: false,
            }],
            CodingAgentEvent::ToolCallFailed {
                tool_call_id,
                message,
                ..
            } => vec![UiEvent::ToolFinished {
                call_id: tool_call_id.clone(),
                result: message.clone(),
                is_error: true,
            }],
            CodingAgentEvent::RuntimeCompactionCompleted { summary, .. }
            | CodingAgentEvent::SessionCompactionCompleted { summary, .. } => vec![
                UiEvent::CompactionNotice {
                    summary: summary.clone(),
                },
                UiEvent::UsageUpdate {
                    input: self.total_input,
                    output: self.total_output,
                    cache_read: self.total_cache_read,
                    cache_write: self.total_cache_write,
                    cost: self.total_cost,
                    context_tokens: None,
                },
            ],
            CodingAgentEvent::PromptFailed { error, .. } => vec![UiEvent::AgentError {
                error: error.to_string(),
            }],
            CodingAgentEvent::PromptAborted { reason, .. } => vec![UiEvent::AgentError {
                error: format!("prompt aborted: {reason}"),
            }],
            CodingAgentEvent::SessionOpened { .. }
            | CodingAgentEvent::DefaultAgentProfileChanged { .. }
            | CodingAgentEvent::AgentInvocationStarted { .. }
            | CodingAgentEvent::AgentInvocationCompleted { .. }
            | CodingAgentEvent::AgentInvocationFailed { .. }
            | CodingAgentEvent::AgentInvocationAborted { .. }
            | CodingAgentEvent::AgentTeamStarted { .. }
            | CodingAgentEvent::AgentTeamMemberStarted { .. }
            | CodingAgentEvent::AgentTeamMemberCompleted { .. }
            | CodingAgentEvent::AgentTeamCompleted { .. }
            | CodingAgentEvent::AgentTeamFailed { .. }
            | CodingAgentEvent::AgentTeamAborted { .. }
            | CodingAgentEvent::DelegationRequested { .. }
            | CodingAgentEvent::DelegationRejected { .. }
            | CodingAgentEvent::DelegationApproved { .. }
            | CodingAgentEvent::DelegationConfirmationRequired { .. }
            | CodingAgentEvent::DelegationStarted { .. }
            | CodingAgentEvent::DelegationCompleted { .. }
            | CodingAgentEvent::DelegationFailed { .. }
            | CodingAgentEvent::SessionWritePending { .. }
            | CodingAgentEvent::SessionWriteCommitted { .. }
            | CodingAgentEvent::SessionWriteSkipped { .. }
            | CodingAgentEvent::PromptStarted { .. }
            | CodingAgentEvent::ProviderRequestStarted { .. }
            | CodingAgentEvent::AssistantMessageStarted { .. }
            | CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::Diagnostic { .. }
            | CodingAgentEvent::CapabilityChanged => Vec::new(),
        }
    }
}

fn parse_tool_arguments(arguments_json: &str) -> serde_json::Value {
    serde_json::from_str(arguments_json)
        .unwrap_or_else(|_| serde_json::Value::String(arguments_json.to_string()))
}
