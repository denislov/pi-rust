use super::{
    CodingSessionError, ProfileId, ProfileKind, SelfHealingEditCheckOutput,
    SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use pi_ai::types::Usage;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductEventClassification<'event> {
    pub(crate) family: ProductEventFamily,
    pub(crate) operation_id: Option<&'event str>,
    pub(crate) terminal_status: Option<ProductEventTerminalStatus>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventFamily {
    Session,
    Profile,
    Agent,
    Team,
    Message,
    Tool,
    Runtime,
    Delegation,
    Workflow,
    Diagnostic,
    Capability,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventTerminalStatus {
    Completed,
    Failed,
    Aborted,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProductEvent {
    pub(crate) sequence: ProductEventSequence,
    pub(crate) family: ProductEventFamily,
    pub(crate) operation_id: Option<String>,
    pub(crate) terminal_status: Option<ProductEventTerminalStatus>,
    pub(crate) durability: ProductEventDurability,
    compatibility_event: CodingAgentEvent,
}

impl ProductEvent {
    pub(crate) fn from_compat_event(
        sequence: ProductEventSequence,
        compatibility_event: CodingAgentEvent,
    ) -> Self {
        let classification = compatibility_event.classification();
        let family = classification.family;
        let operation_id = classification.operation_id.map(str::to_owned);
        let terminal_status = classification.terminal_status;
        Self {
            sequence,
            family,
            operation_id,
            terminal_status,
            durability: ProductEventDurability::LiveOnly,
            compatibility_event,
        }
    }

    pub(crate) fn compatibility_event(&self) -> &CodingAgentEvent {
        &self.compatibility_event
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductEventSequence(pub(crate) u64);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventDurability {
    LiveOnly,
}

#[derive(Debug, Clone, PartialEq)]
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
    SelfHealingEditStarted {
        operation_id: String,
        path: String,
        replacements: usize,
    },
    SelfHealingEditRepairAttempted {
        operation_id: String,
        path: String,
        attempt: usize,
        replacements: Vec<SelfHealingEditReplacement>,
        diagnostics: Vec<SelfHealingEditDiagnostic>,
        check_output: Option<SelfHealingEditCheckOutput>,
    },
    SelfHealingEditCompleted {
        operation_id: String,
        path: String,
        attempts: usize,
        first_changed_line: Option<usize>,
        check_output: Option<SelfHealingEditCheckOutput>,
    },
    SelfHealingEditFailed {
        operation_id: String,
        path: String,
        error: CodingSessionError,
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
    DelegationApproved {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
    },
    DelegationConfirmationRequired {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        reason: String,
    },
    DelegationStarted {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        child_operation_id: String,
    },
    DelegationCompleted {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        child_operation_id: String,
        final_text: String,
    },
    DelegationFailed {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        child_operation_id: String,
        error: CodingSessionError,
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
        usage: Usage,
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

#[allow(dead_code)]
impl CodingAgentEvent {
    pub(crate) fn classification(&self) -> ProductEventClassification<'_> {
        ProductEventClassification {
            family: self.family(),
            operation_id: self.operation_id(),
            terminal_status: self.terminal_status(),
        }
    }

    fn family(&self) -> ProductEventFamily {
        match self {
            Self::SessionOpened { .. }
            | Self::SessionWritePending { .. }
            | Self::SessionWriteCommitted { .. }
            | Self::SessionWriteSkipped { .. }
            | Self::SessionCompactionCompleted { .. } => ProductEventFamily::Session,
            Self::DefaultAgentProfileChanged { .. } => ProductEventFamily::Profile,
            Self::AgentInvocationStarted { .. }
            | Self::AgentInvocationCompleted { .. }
            | Self::AgentInvocationFailed { .. }
            | Self::AgentInvocationAborted { .. }
            | Self::AgentTurnStarted { .. }
            | Self::ProviderRequestStarted { .. } => ProductEventFamily::Agent,
            Self::AgentTeamStarted { .. }
            | Self::AgentTeamMemberStarted { .. }
            | Self::AgentTeamMemberCompleted { .. }
            | Self::AgentTeamCompleted { .. }
            | Self::AgentTeamFailed { .. }
            | Self::AgentTeamAborted { .. } => ProductEventFamily::Team,
            Self::AssistantMessageStarted { .. }
            | Self::AssistantMessageDelta { .. }
            | Self::AssistantThinkingDelta { .. }
            | Self::AssistantMessageCompleted { .. } => ProductEventFamily::Message,
            Self::ToolCallStarted { .. }
            | Self::ToolCallUpdated { .. }
            | Self::ToolCallCompleted { .. }
            | Self::ToolCallFailed { .. } => ProductEventFamily::Tool,
            Self::RuntimeCompactionCompleted { .. } => ProductEventFamily::Runtime,
            Self::DelegationRequested { .. }
            | Self::DelegationRejected { .. }
            | Self::DelegationApproved { .. }
            | Self::DelegationConfirmationRequired { .. }
            | Self::DelegationStarted { .. }
            | Self::DelegationCompleted { .. }
            | Self::DelegationFailed { .. } => ProductEventFamily::Delegation,
            Self::SelfHealingEditStarted { .. }
            | Self::SelfHealingEditRepairAttempted { .. }
            | Self::SelfHealingEditCompleted { .. }
            | Self::SelfHealingEditFailed { .. }
            | Self::PromptStarted { .. }
            | Self::PromptCompleted { .. }
            | Self::PromptFailed { .. }
            | Self::PromptAborted { .. } => ProductEventFamily::Workflow,
            Self::Diagnostic { .. } => ProductEventFamily::Diagnostic,
            Self::CapabilityChanged => ProductEventFamily::Capability,
        }
    }

    fn operation_id(&self) -> Option<&str> {
        match self {
            Self::AgentInvocationStarted { operation_id, .. }
            | Self::AgentInvocationCompleted { operation_id, .. }
            | Self::AgentInvocationFailed { operation_id, .. }
            | Self::AgentInvocationAborted { operation_id, .. }
            | Self::AgentTeamStarted { operation_id, .. }
            | Self::AgentTeamMemberStarted { operation_id, .. }
            | Self::AgentTeamMemberCompleted { operation_id, .. }
            | Self::AgentTeamCompleted { operation_id, .. }
            | Self::AgentTeamFailed { operation_id, .. }
            | Self::AgentTeamAborted { operation_id, .. }
            | Self::SelfHealingEditStarted { operation_id, .. }
            | Self::SelfHealingEditRepairAttempted { operation_id, .. }
            | Self::SelfHealingEditCompleted { operation_id, .. }
            | Self::SelfHealingEditFailed { operation_id, .. }
            | Self::DelegationRequested { operation_id, .. }
            | Self::DelegationRejected { operation_id, .. }
            | Self::DelegationApproved { operation_id, .. }
            | Self::DelegationConfirmationRequired { operation_id, .. }
            | Self::DelegationStarted { operation_id, .. }
            | Self::DelegationCompleted { operation_id, .. }
            | Self::DelegationFailed { operation_id, .. }
            | Self::SessionWritePending { operation_id }
            | Self::SessionWriteCommitted { operation_id, .. }
            | Self::SessionWriteSkipped { operation_id, .. }
            | Self::PromptStarted { operation_id, .. }
            | Self::AgentTurnStarted { operation_id, .. }
            | Self::ProviderRequestStarted { operation_id, .. }
            | Self::AssistantMessageStarted { operation_id, .. }
            | Self::AssistantMessageDelta { operation_id, .. }
            | Self::AssistantThinkingDelta { operation_id, .. }
            | Self::AssistantMessageCompleted { operation_id, .. }
            | Self::ToolCallStarted { operation_id, .. }
            | Self::ToolCallUpdated { operation_id, .. }
            | Self::ToolCallCompleted { operation_id, .. }
            | Self::ToolCallFailed { operation_id, .. }
            | Self::RuntimeCompactionCompleted { operation_id, .. }
            | Self::SessionCompactionCompleted { operation_id, .. }
            | Self::PromptCompleted { operation_id, .. }
            | Self::PromptFailed { operation_id, .. }
            | Self::PromptAborted { operation_id, .. } => Some(operation_id.as_str()),
            Self::Diagnostic { operation_id, .. } => operation_id.as_deref(),
            Self::SessionOpened { .. }
            | Self::DefaultAgentProfileChanged { .. }
            | Self::CapabilityChanged => None,
        }
    }

    fn terminal_status(&self) -> Option<ProductEventTerminalStatus> {
        match self {
            Self::AgentInvocationCompleted { .. }
            | Self::AgentTeamCompleted { .. }
            | Self::SelfHealingEditCompleted { .. }
            | Self::DelegationCompleted { .. }
            | Self::SessionWriteCommitted { .. }
            | Self::SessionCompactionCompleted { .. }
            | Self::PromptCompleted { .. }
            | Self::ToolCallCompleted { .. } => Some(ProductEventTerminalStatus::Completed),
            Self::AgentInvocationFailed { .. }
            | Self::AgentTeamFailed { .. }
            | Self::SelfHealingEditFailed { .. }
            | Self::DelegationFailed { .. }
            | Self::PromptFailed { .. }
            | Self::ToolCallFailed { .. } => Some(ProductEventTerminalStatus::Failed),
            Self::AgentInvocationAborted { .. }
            | Self::AgentTeamAborted { .. }
            | Self::PromptAborted { .. } => Some(ProductEventTerminalStatus::Aborted),
            Self::SessionOpened { .. }
            | Self::DefaultAgentProfileChanged { .. }
            | Self::AgentInvocationStarted { .. }
            | Self::AgentTeamStarted { .. }
            | Self::AgentTeamMemberStarted { .. }
            | Self::AgentTeamMemberCompleted { .. }
            | Self::SelfHealingEditStarted { .. }
            | Self::SelfHealingEditRepairAttempted { .. }
            | Self::DelegationRequested { .. }
            | Self::DelegationRejected { .. }
            | Self::DelegationApproved { .. }
            | Self::DelegationConfirmationRequired { .. }
            | Self::DelegationStarted { .. }
            | Self::SessionWritePending { .. }
            | Self::SessionWriteSkipped { .. }
            | Self::PromptStarted { .. }
            | Self::AgentTurnStarted { .. }
            | Self::ProviderRequestStarted { .. }
            | Self::AssistantMessageStarted { .. }
            | Self::AssistantMessageDelta { .. }
            | Self::AssistantThinkingDelta { .. }
            | Self::AssistantMessageCompleted { .. }
            | Self::ToolCallStarted { .. }
            | Self::ToolCallUpdated { .. }
            | Self::RuntimeCompactionCompleted { .. }
            | Self::Diagnostic { .. }
            | Self::CapabilityChanged => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_id(value: &str) -> ProfileId {
        ProfileId::new(value.to_owned()).expect("valid profile id")
    }

    #[test]
    fn coding_agent_events_report_internal_product_families() {
        assert_eq!(
            CodingAgentEvent::SessionOpened {
                session_id: "session_1".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Session
        );
        assert_eq!(
            CodingAgentEvent::DefaultAgentProfileChanged {
                profile_id: profile_id("agent-main"),
            }
            .classification()
            .family,
            ProductEventFamily::Profile
        );
        assert_eq!(
            CodingAgentEvent::AgentInvocationStarted {
                operation_id: "op_agent".into(),
                child_operation_id: "op_child".into(),
                profile_id: profile_id("agent-main"),
                task: "review".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Agent
        );
        assert_eq!(
            CodingAgentEvent::AgentTeamStarted {
                operation_id: "op_team".into(),
                team_id: profile_id("team-main"),
                task: "review".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Team
        );
        assert_eq!(
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Message
        );
        assert_eq!(
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Tool
        );
        assert_eq!(
            CodingAgentEvent::DelegationStarted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: profile_id("agent-main"),
                target_kind: ProfileKind::Agent,
                target_id: profile_id("agent-helper"),
                task: "review".into(),
                child_operation_id: "op_child".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Delegation
        );
        assert_eq!(
            CodingAgentEvent::SelfHealingEditStarted {
                operation_id: "op_edit".into(),
                path: "src/lib.rs".into(),
                replacements: 1,
            }
            .classification()
            .family,
            ProductEventFamily::Workflow
        );
        assert_eq!(
            CodingAgentEvent::RuntimeCompactionCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                summary: "summary".into(),
                first_kept_message_id: "msg_2".into(),
                tokens_before: 128,
            }
            .classification()
            .family,
            ProductEventFamily::Runtime
        );
        assert_eq!(
            CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "notice".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Diagnostic
        );
        assert_eq!(
            CodingAgentEvent::CapabilityChanged.classification().family,
            ProductEventFamily::Capability
        );
    }

    #[test]
    fn coding_agent_events_report_operation_correlation_and_terminal_status() {
        let completed_event = CodingAgentEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
        };
        let completed = completed_event.classification();
        assert_eq!(completed.operation_id, Some("op_prompt"));
        assert_eq!(
            completed.terminal_status,
            Some(ProductEventTerminalStatus::Completed)
        );

        let failed_event = CodingAgentEvent::SelfHealingEditFailed {
            operation_id: "op_edit".into(),
            path: "src/lib.rs".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        };
        let failed = failed_event.classification();
        assert_eq!(failed.operation_id, Some("op_edit"));
        assert_eq!(
            failed.terminal_status,
            Some(ProductEventTerminalStatus::Failed)
        );

        let aborted_event = CodingAgentEvent::AgentInvocationAborted {
            operation_id: "op_agent".into(),
            child_operation_id: "op_child".into(),
            profile_id: profile_id("agent-main"),
            reason: "cancelled".into(),
        };
        let aborted = aborted_event.classification();
        assert_eq!(aborted.operation_id, Some("op_agent"));
        assert_eq!(
            aborted.terminal_status,
            Some(ProductEventTerminalStatus::Aborted)
        );

        let progress_event = CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        };
        let progress = progress_event.classification();
        assert_eq!(progress.operation_id, Some("op_prompt"));
        assert_eq!(progress.terminal_status, None);

        let uncorrelated = CodingAgentEvent::CapabilityChanged.classification();
        assert_eq!(uncorrelated.operation_id, None);
        assert_eq!(uncorrelated.terminal_status, None);
    }

    #[test]
    fn product_event_wrapper_owns_compatibility_event_and_metadata() {
        let event = CodingAgentEvent::PromptFailed {
            operation_id: "op_prompt".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        };

        let product_event =
            ProductEvent::from_compat_event(ProductEventSequence(42), event.clone());

        assert_eq!(product_event.sequence, ProductEventSequence(42));
        assert_eq!(product_event.family, ProductEventFamily::Workflow);
        assert_eq!(product_event.operation_id.as_deref(), Some("op_prompt"));
        assert_eq!(
            product_event.terminal_status,
            Some(ProductEventTerminalStatus::Failed)
        );
        assert_eq!(product_event.durability, ProductEventDurability::LiveOnly);
        assert_eq!(product_event.compatibility_event(), &event);
    }
}
