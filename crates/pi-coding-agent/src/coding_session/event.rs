use super::capability_snapshot::CapabilityRevocationPolicy;
use super::public_event::{
    CodingAgentAgentProductEvent, CodingAgentProductEventKind, CodingAgentSessionProductEvent,
    CodingAgentTeamProductEvent, CodingAgentWorkflowProductEvent,
};
use super::{
    CodingSessionError, ProfileId, ProfileKind, SelfHealingEditCheckOutput,
    SelfHealingEditDiagnostic, SelfHealingEditReplacement, operation_control::OperationKind,
};
use pi_ai::api::Usage;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventTerminalStatus {
    Completed,
    Failed,
    Aborted,
    Recovered,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductEventTerminalOperation {
    pub(crate) kind: OperationKind,
    pub(crate) status: ProductEventTerminalStatus,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProductEvent {
    sequence: ProductEventSequence,
    event: CodingAgentProductEventKind,
    operation_id: Option<String>,
    terminal_status: Option<ProductEventTerminalStatus>,
    durability: ProductEventDurability,
}

impl ProductEvent {
    pub(crate) fn new(
        sequence: ProductEventSequence,
        event: CodingAgentProductEventKind,
        operation_id: Option<String>,
        terminal_status: Option<ProductEventTerminalStatus>,
        durability: ProductEventDurability,
    ) -> Self {
        Self {
            sequence,
            event,
            operation_id,
            terminal_status,
            durability,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_event_for_tests(
        sequence: ProductEventSequence,
        source: CodingAgentEvent,
    ) -> Self {
        let operation_id = source.operation_id().map(str::to_owned);
        let terminal_status = source.terminal_status();
        let durability = ProductEventDurability::from_emitted_event(&source);
        let event = CodingAgentProductEventKind::from(&source);
        Self::new(sequence, event, operation_id, terminal_status, durability)
    }

    pub(crate) fn sequence(&self) -> ProductEventSequence {
        self.sequence
    }

    pub(crate) fn event(&self) -> &CodingAgentProductEventKind {
        &self.event
    }

    #[allow(dead_code)]
    pub(crate) fn operation_id(&self) -> Option<&str> {
        self.operation_id.as_deref()
    }

    #[allow(dead_code)]
    pub(crate) fn terminal_status(&self) -> Option<ProductEventTerminalStatus> {
        self.terminal_status
    }

    #[allow(dead_code)]
    pub(crate) fn terminal_operation(&self) -> Option<ProductEventTerminalOperation> {
        let status = self.terminal_status?;
        let kind = match &self.event {
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptCompleted { .. }
                | CodingAgentWorkflowProductEvent::PromptFailed { .. }
                | CodingAgentWorkflowProductEvent::PromptAborted { .. },
            ) => OperationKind::Prompt,
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::InvocationCompleted { .. }
                | CodingAgentAgentProductEvent::InvocationFailed { .. }
                | CodingAgentAgentProductEvent::InvocationAborted { .. },
            ) => OperationKind::AgentInvocation,
            CodingAgentProductEventKind::Team(
                CodingAgentTeamProductEvent::Completed { .. }
                | CodingAgentTeamProductEvent::Failed { .. }
                | CodingAgentTeamProductEvent::Aborted { .. },
            ) => OperationKind::AgentTeam,
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted { .. }
                | CodingAgentWorkflowProductEvent::SelfHealingEditFailed { .. },
            ) => OperationKind::SelfHealingEdit,
            CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::CompactionCompleted { .. },
            ) => OperationKind::Compact,
            _ => return None,
        };
        Some(ProductEventTerminalOperation { kind, status })
    }

    #[allow(dead_code)]
    pub(crate) fn durability(&self) -> &ProductEventDurability {
        &self.durability
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ProductEventSequence(pub(crate) u64);

impl ProductEventSequence {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProductEventDurability {
    LiveOnly,
    PendingSessionWrite { operation_id: String },
    Durable { session_id: String },
}

impl ProductEventDurability {
    pub(crate) fn from_emitted_event(event: &CodingAgentEvent) -> Self {
        match event {
            CodingAgentEvent::SessionWritePending { operation_id } => Self::PendingSessionWrite {
                operation_id: operation_id.clone(),
            },
            CodingAgentEvent::SessionWriteCommitted { session_id, .. } => Self::Durable {
                session_id: session_id.clone(),
            },
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
            | CodingAgentEvent::SelfHealingEditStarted { .. }
            | CodingAgentEvent::SelfHealingEditRepairAttempted { .. }
            | CodingAgentEvent::SelfHealingEditCompleted { .. }
            | CodingAgentEvent::SelfHealingEditFailed { .. }
            | CodingAgentEvent::DelegationRequested { .. }
            | CodingAgentEvent::DelegationRejected { .. }
            | CodingAgentEvent::DelegationApproved { .. }
            | CodingAgentEvent::DelegationConfirmationRequired { .. }
            | CodingAgentEvent::DelegationStarted { .. }
            | CodingAgentEvent::DelegationCompleted { .. }
            | CodingAgentEvent::DelegationFailed { .. }
            | CodingAgentEvent::SessionWriteSkipped { .. }
            | CodingAgentEvent::PromptStarted { .. }
            | CodingAgentEvent::AgentTurnStarted { .. }
            | CodingAgentEvent::ProviderRequestStarted { .. }
            | CodingAgentEvent::AssistantMessageStarted { .. }
            | CodingAgentEvent::AssistantMessageDelta { .. }
            | CodingAgentEvent::AssistantThinkingDelta { .. }
            | CodingAgentEvent::AssistantMessageCompleted { .. }
            | CodingAgentEvent::ToolCallStarted { .. }
            | CodingAgentEvent::ToolCallUpdated { .. }
            | CodingAgentEvent::ToolCallCompleted { .. }
            | CodingAgentEvent::ToolCallFailed { .. }
            | CodingAgentEvent::RuntimeCompactionCompleted { .. }
            | CodingAgentEvent::RuntimeShutDown
            | CodingAgentEvent::SessionCompactionCompleted { .. }
            | CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::PromptFailed { .. }
            | CodingAgentEvent::PromptAborted { .. }
            | CodingAgentEvent::Diagnostic { .. }
            | CodingAgentEvent::CapabilityChanged { .. }
            | CodingAgentEvent::OperationRecovered { .. } => Self::LiveOnly,
        }
    }
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
    RuntimeShutDown,
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
    CapabilityChanged {
        generation: u64,
        revocation: CapabilityRevocationPolicy,
    },
    OperationRecovered {
        operation_id: String,
        recovery_id: String,
        reason: String,
    },
}

impl CodingAgentEvent {
    pub(crate) fn operation_id(&self) -> Option<&str> {
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
            | Self::PromptAborted { operation_id, .. }
            | Self::OperationRecovered { operation_id, .. } => Some(operation_id.as_str()),
            Self::Diagnostic { operation_id, .. } => operation_id.as_deref(),
            Self::SessionOpened { .. }
            | Self::RuntimeShutDown
            | Self::DefaultAgentProfileChanged { .. }
            | Self::CapabilityChanged { .. } => None,
        }
    }

    pub(crate) fn terminal_status(&self) -> Option<ProductEventTerminalStatus> {
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
            Self::OperationRecovered { .. } => Some(ProductEventTerminalStatus::Recovered),
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
            | Self::RuntimeShutDown
            | Self::Diagnostic { .. }
            | Self::CapabilityChanged { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::public_event::CodingAgentProductEventFamily;
    use super::*;

    #[test]
    fn product_event_runtime_envelope_has_no_raw_compatibility_storage() {
        let source = include_str!("event.rs");
        let raw_field = ["compatibility", "_event: CodingAgentEvent"].concat();
        let raw_accessor = ["fn compatibility", "_event(&self)"].concat();

        assert!(!source.contains(&raw_field));
        assert!(!source.contains(&raw_accessor));
    }

    fn profile_id(value: &str) -> ProfileId {
        ProfileId::new(value.to_owned()).expect("valid profile id")
    }

    fn typed_event(source: CodingAgentEvent) -> CodingAgentProductEventKind {
        CodingAgentProductEventKind::from(&source)
    }

    #[test]
    fn coding_agent_events_report_internal_product_families() {
        assert_eq!(
            typed_event(CodingAgentEvent::SessionOpened {
                session_id: "session_1".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Session
        );
        assert_eq!(
            typed_event(CodingAgentEvent::DefaultAgentProfileChanged {
                profile_id: profile_id("agent-main"),
            })
            .family(),
            CodingAgentProductEventFamily::Profile
        );
        assert_eq!(
            typed_event(CodingAgentEvent::AgentInvocationStarted {
                operation_id: "op_agent".into(),
                child_operation_id: "op_child".into(),
                profile_id: profile_id("agent-main"),
                task: "review".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Agent
        );
        assert_eq!(
            typed_event(CodingAgentEvent::AgentTeamStarted {
                operation_id: "op_team".into(),
                team_id: profile_id("team-main"),
                task: "review".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Team
        );
        assert_eq!(
            typed_event(CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Message
        );
        assert_eq!(
            typed_event(CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Tool
        );
        assert_eq!(
            typed_event(CodingAgentEvent::DelegationStarted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: profile_id("agent-main"),
                target_kind: ProfileKind::Agent,
                target_id: profile_id("agent-helper"),
                task: "review".into(),
                child_operation_id: "op_child".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Delegation
        );
        assert_eq!(
            typed_event(CodingAgentEvent::SelfHealingEditStarted {
                operation_id: "op_edit".into(),
                path: "src/lib.rs".into(),
                replacements: 1,
            })
            .family(),
            CodingAgentProductEventFamily::Workflow
        );
        assert_eq!(
            typed_event(CodingAgentEvent::RuntimeCompactionCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                summary: "summary".into(),
                first_kept_message_id: "msg_2".into(),
                tokens_before: 128,
            })
            .family(),
            CodingAgentProductEventFamily::Runtime
        );
        assert_eq!(
            typed_event(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "notice".into(),
            })
            .family(),
            CodingAgentProductEventFamily::Diagnostic
        );
        assert_eq!(
            typed_event(CodingAgentEvent::CapabilityChanged {
                generation: 1,
                revocation: CapabilityRevocationPolicy::FutureOnly,
            })
            .family(),
            CodingAgentProductEventFamily::Capability
        );
    }

    #[test]
    fn coding_agent_events_report_operation_correlation_and_terminal_status() {
        let completed_event = CodingAgentEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
        };
        assert_eq!(completed_event.operation_id(), Some("op_prompt"));
        assert_eq!(
            completed_event.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );

        let failed_event = CodingAgentEvent::SelfHealingEditFailed {
            operation_id: "op_edit".into(),
            path: "src/lib.rs".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        };
        assert_eq!(failed_event.operation_id(), Some("op_edit"));
        assert_eq!(
            failed_event.terminal_status(),
            Some(ProductEventTerminalStatus::Failed)
        );

        let aborted_event = CodingAgentEvent::AgentInvocationAborted {
            operation_id: "op_agent".into(),
            child_operation_id: "op_child".into(),
            profile_id: profile_id("agent-main"),
            reason: "cancelled".into(),
        };
        assert_eq!(aborted_event.operation_id(), Some("op_agent"));
        assert_eq!(
            aborted_event.terminal_status(),
            Some(ProductEventTerminalStatus::Aborted)
        );

        let progress_event = CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        };
        assert_eq!(progress_event.operation_id(), Some("op_prompt"));
        assert_eq!(progress_event.terminal_status(), None);

        let uncorrelated = CodingAgentEvent::CapabilityChanged {
            generation: 1,
            revocation: CapabilityRevocationPolicy::FutureOnly,
        };
        assert_eq!(uncorrelated.operation_id(), None);
        assert_eq!(uncorrelated.terminal_status(), None);
    }

    #[test]
    fn capability_changed_event_carries_generation_and_revocation_policy() {
        let event = CodingAgentEvent::CapabilityChanged {
            generation: 2,
            revocation: CapabilityRevocationPolicy::FutureOnly,
        };
        assert!(matches!(
            typed_event(event),
            CodingAgentProductEventKind::Capability(_)
        ));
    }

    #[test]
    fn product_event_sequence_exposes_stable_cursor_math() {
        let first = ProductEventSequence::new(1);
        let second = first.next();

        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);
        assert!(second > first);
        assert_eq!(
            ProductEventSequence::default(),
            ProductEventSequence::new(0)
        );
    }

    #[test]
    fn product_event_keeps_sequence_accessor_for_projection() {
        let event = ProductEvent::from_event_for_tests(
            ProductEventSequence::new(42),
            CodingAgentEvent::SessionOpened {
                session_id: "sess_cursor".into(),
            },
        );

        assert_eq!(event.sequence(), ProductEventSequence::new(42));
    }

    #[test]
    fn product_event_wrapper_owns_typed_event_and_metadata() {
        let event = CodingAgentEvent::PromptFailed {
            operation_id: "op_prompt".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        };

        let product_event = ProductEvent::from_event_for_tests(ProductEventSequence(42), event);

        assert_eq!(product_event.sequence(), ProductEventSequence(42));
        assert!(matches!(
            product_event.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptFailed { .. }
            )
        ));
        assert_eq!(product_event.operation_id(), Some("op_prompt"));
        assert_eq!(
            product_event.terminal_status(),
            Some(ProductEventTerminalStatus::Failed)
        );
        assert_eq!(
            product_event.durability(),
            &ProductEventDurability::LiveOnly
        );
    }

    #[test]
    fn product_event_wrapper_marks_session_write_durability() {
        let pending = ProductEvent::from_event_for_tests(
            ProductEventSequence(1),
            CodingAgentEvent::SessionWritePending {
                operation_id: "op_prompt".into(),
            },
        );
        assert_eq!(
            pending.durability(),
            &ProductEventDurability::PendingSessionWrite {
                operation_id: "op_prompt".into(),
            }
        );

        let committed = ProductEvent::from_event_for_tests(
            ProductEventSequence(2),
            CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op_prompt".into(),
                session_id: "session_1".into(),
            },
        );
        assert_eq!(
            committed.durability(),
            &ProductEventDurability::Durable {
                session_id: "session_1".into(),
            }
        );
        assert_eq!(committed.operation_id(), Some("op_prompt"));
        assert_eq!(
            committed.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );

        let skipped = ProductEvent::from_event_for_tests(
            ProductEventSequence(3),
            CodingAgentEvent::SessionWriteSkipped {
                operation_id: "op_prompt".into(),
                reason: "session persistence disabled".into(),
            },
        );
        assert_eq!(skipped.durability(), &ProductEventDurability::LiveOnly);
    }

    #[test]
    fn product_event_wrapper_exposes_family_specific_kind() {
        let prompt = ProductEvent::from_event_for_tests(
            ProductEventSequence(10),
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
            },
        );
        assert!(matches!(
            prompt.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptCompleted { .. }
            )
        ));

        let tool = ProductEvent::from_event_for_tests(
            ProductEventSequence(11),
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            },
        );
        assert!(matches!(
            tool.event(),
            CodingAgentProductEventKind::Tool(
                super::super::public_event::CodingAgentToolProductEvent::Completed { .. }
            )
        ));

        let session = ProductEvent::from_event_for_tests(
            ProductEventSequence(12),
            CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op_prompt".into(),
                session_id: "session_1".into(),
            },
        );
        assert!(matches!(
            session.event(),
            CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::WriteCommitted { .. }
            )
        ));
    }

    #[test]
    fn product_event_wrapper_exposes_metadata_through_accessors() {
        let event = ProductEvent::from_event_for_tests(
            ProductEventSequence(13),
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
            },
        );

        assert_eq!(event.sequence(), ProductEventSequence(13));
        assert!(matches!(
            event.event(),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptCompleted { .. }
            )
        ));
        assert_eq!(event.operation_id(), Some("op_prompt"));
        assert_eq!(
            event.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );
        assert_eq!(event.durability(), &ProductEventDurability::LiveOnly);
    }

    #[test]
    fn product_event_wrapper_reports_terminal_operation_metadata() {
        let prompt = ProductEvent::from_event_for_tests(
            ProductEventSequence(14),
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
            },
        );
        assert_eq!(
            prompt.terminal_operation(),
            Some(ProductEventTerminalOperation {
                kind: OperationKind::Prompt,
                status: ProductEventTerminalStatus::Completed,
            })
        );

        let agent = ProductEvent::from_event_for_tests(
            ProductEventSequence(15),
            CodingAgentEvent::AgentInvocationFailed {
                operation_id: "op_agent".into(),
                child_operation_id: "op_child".into(),
                profile_id: profile_id("agent-main"),
                error: CodingSessionError::Provider {
                    message: "provider failed".into(),
                },
            },
        );
        assert_eq!(
            agent.terminal_operation(),
            Some(ProductEventTerminalOperation {
                kind: OperationKind::AgentInvocation,
                status: ProductEventTerminalStatus::Failed,
            })
        );

        let team = ProductEvent::from_event_for_tests(
            ProductEventSequence(16),
            CodingAgentEvent::AgentTeamAborted {
                operation_id: "op_team".into(),
                team_id: profile_id("team-main"),
                reason: "cancelled".into(),
            },
        );
        assert_eq!(
            team.terminal_operation(),
            Some(ProductEventTerminalOperation {
                kind: OperationKind::AgentTeam,
                status: ProductEventTerminalStatus::Aborted,
            })
        );

        let self_healing_edit = ProductEvent::from_event_for_tests(
            ProductEventSequence(17),
            CodingAgentEvent::SelfHealingEditCompleted {
                operation_id: "op_edit".into(),
                path: "src/lib.rs".into(),
                attempts: 1,
                first_changed_line: Some(12),
                check_output: None,
            },
        );
        assert_eq!(
            self_healing_edit.terminal_operation(),
            Some(ProductEventTerminalOperation {
                kind: OperationKind::SelfHealingEdit,
                status: ProductEventTerminalStatus::Completed,
            })
        );

        let compaction = ProductEvent::from_event_for_tests(
            ProductEventSequence(18),
            CodingAgentEvent::SessionCompactionCompleted {
                operation_id: "op_compact".into(),
                turn_id: "turn_1".into(),
                summary: "summary".into(),
                first_kept_message_id: "msg_2".into(),
                tokens_before: 128,
            },
        );
        assert_eq!(
            compaction.terminal_operation(),
            Some(ProductEventTerminalOperation {
                kind: OperationKind::Compact,
                status: ProductEventTerminalStatus::Completed,
            })
        );
    }

    #[test]
    fn product_event_wrapper_does_not_treat_family_completion_as_operation_terminal() {
        let tool = ProductEvent::from_event_for_tests(
            ProductEventSequence(19),
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            },
        );
        assert_eq!(
            tool.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );
        assert_eq!(tool.terminal_operation(), None);

        let session_write = ProductEvent::from_event_for_tests(
            ProductEventSequence(20),
            CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op_prompt".into(),
                session_id: "session_1".into(),
            },
        );
        assert_eq!(
            session_write.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );
        assert_eq!(session_write.terminal_operation(), None);

        let progress = ProductEvent::from_event_for_tests(
            ProductEventSequence(21),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello".into(),
            },
        );
        assert_eq!(progress.terminal_operation(), None);
    }
}
