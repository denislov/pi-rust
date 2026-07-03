use super::{CodingSessionError, ProfileId, ProfileKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodingAgentEvent {
    SessionOpened {
        session_id: String,
    },
    DefaultAgentProfileChanged {
        profile_id: ProfileId,
    },
    AgentInvocationStarted {
        operation_id: String,
        child_operation_id: String,
        profile_id: ProfileId,
        task: String,
    },
    AgentInvocationCompleted {
        operation_id: String,
        child_operation_id: String,
        profile_id: ProfileId,
        final_text: String,
    },
    AgentInvocationFailed {
        operation_id: String,
        child_operation_id: String,
        profile_id: ProfileId,
        error: CodingSessionError,
    },
    AgentInvocationAborted {
        operation_id: String,
        child_operation_id: String,
        profile_id: ProfileId,
        reason: String,
    },
    AgentTeamStarted {
        operation_id: String,
        team_id: ProfileId,
        task: String,
    },
    AgentTeamMemberStarted {
        operation_id: String,
        child_operation_id: String,
        team_id: ProfileId,
        profile_id: ProfileId,
        task: String,
    },
    AgentTeamMemberCompleted {
        operation_id: String,
        child_operation_id: String,
        team_id: ProfileId,
        profile_id: ProfileId,
        final_text: String,
    },
    AgentTeamCompleted {
        operation_id: String,
        team_id: ProfileId,
        final_text: String,
    },
    AgentTeamFailed {
        operation_id: String,
        team_id: ProfileId,
        error: CodingSessionError,
    },
    AgentTeamAborted {
        operation_id: String,
        team_id: ProfileId,
        reason: String,
    },
    DelegationRequested {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
    },
    DelegationRejected {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        reason: String,
    },
    SessionWritePending {
        operation_id: String,
    },
    SessionWriteCommitted {
        operation_id: String,
        session_id: String,
    },
    SessionWriteSkipped {
        operation_id: String,
        reason: String,
    },
    PromptStarted {
        operation_id: String,
        turn_id: String,
    },
    AgentTurnStarted {
        operation_id: String,
        turn_id: String,
        agent_turn: u32,
    },
    ProviderRequestStarted {
        operation_id: String,
        turn_id: String,
        provider: String,
        model: String,
    },
    AssistantMessageStarted {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
    },
    AssistantMessageDelta {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        text: String,
    },
    AssistantThinkingDelta {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        text: String,
    },
    AssistantMessageCompleted {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        final_text: String,
    },
    ToolCallStarted {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        arguments_json: String,
    },
    ToolCallUpdated {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        message: String,
    },
    ToolCallCompleted {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        summary: String,
    },
    ToolCallFailed {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        message: String,
    },
    RuntimeCompactionCompleted {
        operation_id: String,
        turn_id: String,
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
    SessionCompactionCompleted {
        operation_id: String,
        turn_id: String,
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
    PromptCompleted {
        operation_id: String,
        turn_id: String,
    },
    PromptFailed {
        operation_id: String,
        error: CodingSessionError,
    },
    PromptAborted {
        operation_id: String,
        reason: String,
    },
    Diagnostic {
        operation_id: Option<String>,
        message: String,
    },
    CapabilityChanged,
}
