use crate::coding_session::{CodingAgentEvent, ProfileKind};

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
    SystemNotice {
        text: String,
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
            CodingAgentEvent::DelegationConfirmationRequired {
                operation_id,
                tool_call_id,
                target_kind,
                target_id,
                task,
                reason,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation confirmation required for {} {}.\nTask: {}\nReason: {}\nApprove: /delegation approve {} {}\nReject: /delegation reject {} {} [reason]\nList pending: /delegations",
                    profile_kind_label(*target_kind),
                    target_id,
                    task,
                    reason,
                    operation_id,
                    tool_call_id,
                    operation_id,
                    tool_call_id
                ),
            }],
            CodingAgentEvent::DelegationApproved {
                target_kind,
                target_id,
                task,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation approved for {} {}: {}",
                    profile_kind_label(*target_kind),
                    target_id,
                    task
                ),
            }],
            CodingAgentEvent::DelegationRejected {
                target_kind,
                target_id,
                task,
                reason,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation rejected for {} {}: {} ({})",
                    profile_kind_label(*target_kind),
                    target_id,
                    task,
                    reason
                ),
            }],
            CodingAgentEvent::DelegationStarted {
                target_kind,
                target_id,
                task,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation started for {} {}: {}",
                    profile_kind_label(*target_kind),
                    target_id,
                    task
                ),
            }],
            CodingAgentEvent::DelegationCompleted {
                target_kind,
                target_id,
                final_text,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation completed for {} {}: {}",
                    profile_kind_label(*target_kind),
                    target_id,
                    final_text
                ),
            }],
            CodingAgentEvent::DelegationFailed {
                target_kind,
                target_id,
                error,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Delegation failed for {} {}: {}",
                    profile_kind_label(*target_kind),
                    target_id,
                    error
                ),
            }],
            CodingAgentEvent::SelfHealingEditStarted {
                path, replacements, ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit started for {} ({}).",
                    path,
                    replacement_count_label(*replacements)
                ),
            }],
            CodingAgentEvent::SelfHealingEditRepairAttempted {
                path,
                attempt,
                replacements,
                check_output,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit repair attempt {} for {}: {}, {}.",
                    attempt,
                    path,
                    replacement_count_label(replacements.len()),
                    check_output_label(check_output.as_ref())
                ),
            }],
            CodingAgentEvent::SelfHealingEditCompleted {
                path,
                attempts,
                first_changed_line,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit completed for {} after {}{}.",
                    path,
                    attempt_count_label(*attempts),
                    first_changed_line_label(*first_changed_line)
                ),
            }],
            CodingAgentEvent::SelfHealingEditFailed { path, error, .. } => {
                vec![UiEvent::SystemNotice {
                    text: format!("Self-healing edit failed for {}: {}", path, error),
                }]
            }
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

fn replacement_count_label(replacements: usize) -> String {
    match replacements {
        1 => "1 replacement".to_string(),
        count => format!("{count} replacements"),
    }
}

fn attempt_count_label(attempts: usize) -> String {
    match attempts {
        1 => "1 attempt".to_string(),
        count => format!("{count} attempts"),
    }
}

fn first_changed_line_label(first_changed_line: Option<usize>) -> String {
    first_changed_line
        .map(|line| format!(", first changed line {line}"))
        .unwrap_or_default()
}

fn check_output_label(
    output: Option<&crate::coding_session::SelfHealingEditCheckOutput>,
) -> String {
    output
        .map(|output| format!("check exit {}", output.exit_code))
        .unwrap_or_else(|| "no check output".to_string())
}

fn parse_tool_arguments(arguments_json: &str) -> serde_json::Value {
    serde_json::from_str(arguments_json)
        .unwrap_or_else(|_| serde_json::Value::String(arguments_json.to_string()))
}

fn profile_kind_label(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Agent => "agent",
        ProfileKind::Team => "team",
    }
}
