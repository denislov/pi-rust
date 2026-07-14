use serde::{Deserialize, Serialize};

use super::event::{
    CodingAgentEvent, ProductEvent, ProductEventDurability, ProductEventTerminalOperation,
    ProductEventTerminalStatus,
};
use super::operation_control::OperationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentProductEventFamily {
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

impl CodingAgentProductEventFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Profile => "profile",
            Self::Agent => "agent",
            Self::Team => "team",
            Self::Message => "message",
            Self::Tool => "tool",
            Self::Runtime => "runtime",
            Self::Delegation => "delegation",
            Self::Workflow => "workflow",
            Self::Diagnostic => "diagnostic",
            Self::Capability => "capability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentProductEventTerminalStatus {
    Completed,
    Failed,
    Aborted,
    Recovered,
}

impl CodingAgentProductEventTerminalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
            Self::Recovered => "recovered",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentProductEventTerminalOperationKind {
    Prompt,
    AgentInvocation,
    AgentTeam,
    SelfHealingEdit,
    Compact,
}

impl CodingAgentProductEventTerminalOperationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::AgentInvocation => "agent_invocation",
            Self::AgentTeam => "agent_team",
            Self::SelfHealingEdit => "self_healing_edit",
            Self::Compact => "compact",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CodingAgentProductEventTerminalOperation {
    pub kind: CodingAgentProductEventTerminalOperationKind,
    pub status: CodingAgentProductEventTerminalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum CodingAgentProductEventDurability {
    LiveOnly,
    PendingSessionWrite { operation_id: String },
    Durable { session_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CodingAgentProductEventError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CodingAgentProductEventUsage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub total_tokens: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_write_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CodingAgentProductEventReplacement {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CodingAgentProductEventDiagnostic {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CodingAgentProductEventCheckOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentProductEventProfileKind {
    Agent,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgentProductEventCapabilityRevocation {
    FutureOnly,
    CancelMatchingOperations,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentSessionProductEvent {
    Opened {
        session_id: String,
    },
    WritePending {
        operation_id: String,
    },
    WriteCommitted {
        operation_id: String,
        session_id: String,
    },
    WriteSkipped {
        operation_id: String,
        reason: String,
    },
    CompactionCompleted {
        operation_id: String,
        turn_id: String,
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentProfileProductEvent {
    DefaultChanged { profile_id: String },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentAgentProductEvent {
    InvocationStarted {
        operation_id: String,
        child_operation_id: String,
        profile_id: String,
        task: String,
    },
    InvocationCompleted {
        operation_id: String,
        child_operation_id: String,
        profile_id: String,
        final_text: String,
    },
    InvocationFailed {
        operation_id: String,
        child_operation_id: String,
        profile_id: String,
        error: CodingAgentProductEventError,
    },
    InvocationAborted {
        operation_id: String,
        child_operation_id: String,
        profile_id: String,
        reason: String,
    },
    TurnStarted {
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
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentTeamProductEvent {
    Started {
        operation_id: String,
        team_id: String,
        task: String,
    },
    MemberStarted {
        operation_id: String,
        child_operation_id: String,
        team_id: String,
        profile_id: String,
        task: String,
    },
    MemberCompleted {
        operation_id: String,
        child_operation_id: String,
        team_id: String,
        profile_id: String,
        final_text: String,
    },
    Completed {
        operation_id: String,
        team_id: String,
        final_text: String,
    },
    Failed {
        operation_id: String,
        team_id: String,
        error: CodingAgentProductEventError,
    },
    Aborted {
        operation_id: String,
        team_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentMessageProductEvent {
    Started {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
    },
    Delta {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        text: String,
    },
    ThinkingDelta {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        text: String,
    },
    Completed {
        operation_id: String,
        turn_id: String,
        message_id: Option<String>,
        final_text: String,
        usage: CodingAgentProductEventUsage,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentToolProductEvent {
    Started {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        arguments_json: String,
    },
    Updated {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        message: String,
    },
    Completed {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        summary: String,
    },
    Failed {
        operation_id: String,
        turn_id: String,
        tool_call_id: String,
        name: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentRuntimeProductEvent {
    CompactionCompleted {
        operation_id: String,
        turn_id: String,
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
    },
    ShutDown,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CodingAgentDelegationEventContext {
    pub operation_id: String,
    pub turn_id: String,
    pub tool_call_id: String,
    pub requesting_profile_id: String,
    pub target_kind: CodingAgentProductEventProfileKind,
    pub target_id: String,
    pub task: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentDelegationProductEvent {
    Requested {
        context: CodingAgentDelegationEventContext,
    },
    Rejected {
        context: CodingAgentDelegationEventContext,
        reason: String,
    },
    Approved {
        context: CodingAgentDelegationEventContext,
    },
    ConfirmationRequired {
        context: CodingAgentDelegationEventContext,
        reason: String,
    },
    Started {
        context: CodingAgentDelegationEventContext,
        child_operation_id: String,
    },
    Completed {
        context: CodingAgentDelegationEventContext,
        child_operation_id: String,
        final_text: String,
    },
    Failed {
        context: CodingAgentDelegationEventContext,
        child_operation_id: String,
        error: CodingAgentProductEventError,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentWorkflowProductEvent {
    SelfHealingEditStarted {
        operation_id: String,
        path: String,
        replacements: usize,
    },
    SelfHealingEditRepairAttempted {
        operation_id: String,
        path: String,
        attempt: usize,
        replacements: Vec<CodingAgentProductEventReplacement>,
        diagnostics: Vec<CodingAgentProductEventDiagnostic>,
        check_output: Option<CodingAgentProductEventCheckOutput>,
    },
    SelfHealingEditCompleted {
        operation_id: String,
        path: String,
        attempts: usize,
        first_changed_line: Option<usize>,
        check_output: Option<CodingAgentProductEventCheckOutput>,
    },
    SelfHealingEditFailed {
        operation_id: String,
        path: String,
        error: CodingAgentProductEventError,
    },
    PromptStarted {
        operation_id: String,
        turn_id: String,
    },
    PromptCompleted {
        operation_id: String,
        turn_id: String,
    },
    PromptFailed {
        operation_id: String,
        error: CodingAgentProductEventError,
    },
    PromptAborted {
        operation_id: String,
        reason: String,
    },
    OperationRecovered {
        operation_id: String,
        recovery_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentDiagnosticProductEvent {
    Diagnostic {
        operation_id: Option<String>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CodingAgentCapabilityProductEvent {
    Changed {
        generation: u64,
        revocation: CodingAgentProductEventCapabilityRevocation,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "family", content = "payload")]
pub enum CodingAgentProductEventKind {
    Session(CodingAgentSessionProductEvent),
    Profile(CodingAgentProfileProductEvent),
    Agent(CodingAgentAgentProductEvent),
    Team(CodingAgentTeamProductEvent),
    Message(CodingAgentMessageProductEvent),
    Tool(CodingAgentToolProductEvent),
    Runtime(CodingAgentRuntimeProductEvent),
    Delegation(CodingAgentDelegationProductEvent),
    Workflow(CodingAgentWorkflowProductEvent),
    Diagnostic(CodingAgentDiagnosticProductEvent),
    Capability(CodingAgentCapabilityProductEvent),
}

impl CodingAgentProductEventKind {
    pub const fn family(&self) -> CodingAgentProductEventFamily {
        match self {
            Self::Session(_) => CodingAgentProductEventFamily::Session,
            Self::Profile(_) => CodingAgentProductEventFamily::Profile,
            Self::Agent(_) => CodingAgentProductEventFamily::Agent,
            Self::Team(_) => CodingAgentProductEventFamily::Team,
            Self::Message(_) => CodingAgentProductEventFamily::Message,
            Self::Tool(_) => CodingAgentProductEventFamily::Tool,
            Self::Runtime(_) => CodingAgentProductEventFamily::Runtime,
            Self::Delegation(_) => CodingAgentProductEventFamily::Delegation,
            Self::Workflow(_) => CodingAgentProductEventFamily::Workflow,
            Self::Diagnostic(_) => CodingAgentProductEventFamily::Diagnostic,
            Self::Capability(_) => CodingAgentProductEventFamily::Capability,
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Session(CodingAgentSessionProductEvent::Opened { .. }) => "opened",
            Self::Session(CodingAgentSessionProductEvent::WritePending { .. }) => "write_pending",
            Self::Session(CodingAgentSessionProductEvent::WriteCommitted { .. }) => {
                "write_committed"
            }
            Self::Session(CodingAgentSessionProductEvent::WriteSkipped { .. }) => "write_skipped",
            Self::Session(CodingAgentSessionProductEvent::CompactionCompleted { .. }) => {
                "compaction_completed"
            }
            Self::Profile(CodingAgentProfileProductEvent::DefaultChanged { .. }) => {
                "default_changed"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationStarted { .. }) => {
                "invocation_started"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationCompleted { .. }) => {
                "invocation_completed"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationFailed { .. }) => {
                "invocation_failed"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationAborted { .. }) => {
                "invocation_aborted"
            }
            Self::Agent(CodingAgentAgentProductEvent::TurnStarted { .. }) => "turn_started",
            Self::Agent(CodingAgentAgentProductEvent::ProviderRequestStarted { .. }) => {
                "provider_request_started"
            }
            Self::Team(CodingAgentTeamProductEvent::Started { .. }) => "started",
            Self::Team(CodingAgentTeamProductEvent::MemberStarted { .. }) => "member_started",
            Self::Team(CodingAgentTeamProductEvent::MemberCompleted { .. }) => "member_completed",
            Self::Team(CodingAgentTeamProductEvent::Completed { .. }) => "completed",
            Self::Team(CodingAgentTeamProductEvent::Failed { .. }) => "failed",
            Self::Team(CodingAgentTeamProductEvent::Aborted { .. }) => "aborted",
            Self::Message(CodingAgentMessageProductEvent::Started { .. }) => "started",
            Self::Message(CodingAgentMessageProductEvent::Delta { .. }) => "delta",
            Self::Message(CodingAgentMessageProductEvent::ThinkingDelta { .. }) => "thinking_delta",
            Self::Message(CodingAgentMessageProductEvent::Completed { .. }) => "completed",
            Self::Tool(CodingAgentToolProductEvent::Started { .. }) => "started",
            Self::Tool(CodingAgentToolProductEvent::Updated { .. }) => "updated",
            Self::Tool(CodingAgentToolProductEvent::Completed { .. }) => "completed",
            Self::Tool(CodingAgentToolProductEvent::Failed { .. }) => "failed",
            Self::Runtime(CodingAgentRuntimeProductEvent::CompactionCompleted { .. }) => {
                "compaction_completed"
            }
            Self::Runtime(CodingAgentRuntimeProductEvent::ShutDown) => "shut_down",
            Self::Delegation(CodingAgentDelegationProductEvent::Requested { .. }) => "requested",
            Self::Delegation(CodingAgentDelegationProductEvent::Rejected { .. }) => "rejected",
            Self::Delegation(CodingAgentDelegationProductEvent::Approved { .. }) => "approved",
            Self::Delegation(CodingAgentDelegationProductEvent::ConfirmationRequired {
                ..
            }) => "confirmation_required",
            Self::Delegation(CodingAgentDelegationProductEvent::Started { .. }) => "started",
            Self::Delegation(CodingAgentDelegationProductEvent::Completed { .. }) => "completed",
            Self::Delegation(CodingAgentDelegationProductEvent::Failed { .. }) => "failed",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditStarted { .. }) => {
                "self_healing_edit_started"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                ..
            }) => "self_healing_edit_repair_attempted",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                ..
            }) => "self_healing_edit_completed",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditFailed { .. }) => {
                "self_healing_edit_failed"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptStarted { .. }) => {
                "prompt_started"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted { .. }) => {
                "prompt_completed"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptFailed { .. }) => "prompt_failed",
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptAborted { .. }) => {
                "prompt_aborted"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::OperationRecovered { .. }) => {
                "operation_recovered"
            }
            Self::Diagnostic(CodingAgentDiagnosticProductEvent::Diagnostic { .. }) => "diagnostic",
            Self::Capability(CodingAgentCapabilityProductEvent::Changed { .. }) => "changed",
        }
    }

    const fn legacy_family_name(&self) -> &'static str {
        match self.family() {
            CodingAgentProductEventFamily::Session => "Session",
            CodingAgentProductEventFamily::Profile => "Profile",
            CodingAgentProductEventFamily::Agent => "Agent",
            CodingAgentProductEventFamily::Team => "Team",
            CodingAgentProductEventFamily::Message => "Message",
            CodingAgentProductEventFamily::Tool => "Tool",
            CodingAgentProductEventFamily::Runtime => "Runtime",
            CodingAgentProductEventFamily::Delegation => "Delegation",
            CodingAgentProductEventFamily::Workflow => "Workflow",
            CodingAgentProductEventFamily::Diagnostic => "Diagnostic",
            CodingAgentProductEventFamily::Capability => "Capability",
        }
    }

    const fn legacy_variant_name(&self) -> &'static str {
        match self {
            Self::Session(CodingAgentSessionProductEvent::Opened { .. }) => "Opened",
            Self::Session(CodingAgentSessionProductEvent::WritePending { .. }) => "WritePending",
            Self::Session(CodingAgentSessionProductEvent::WriteCommitted { .. }) => {
                "WriteCommitted"
            }
            Self::Session(CodingAgentSessionProductEvent::WriteSkipped { .. }) => "WriteSkipped",
            Self::Session(CodingAgentSessionProductEvent::CompactionCompleted { .. }) => {
                "CompactionCompleted"
            }
            Self::Profile(CodingAgentProfileProductEvent::DefaultChanged { .. }) => {
                "DefaultChanged"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationStarted { .. }) => {
                "InvocationStarted"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationCompleted { .. }) => {
                "InvocationCompleted"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationFailed { .. }) => {
                "InvocationFailed"
            }
            Self::Agent(CodingAgentAgentProductEvent::InvocationAborted { .. }) => {
                "InvocationAborted"
            }
            Self::Agent(CodingAgentAgentProductEvent::TurnStarted { .. }) => "TurnStarted",
            Self::Agent(CodingAgentAgentProductEvent::ProviderRequestStarted { .. }) => {
                "ProviderRequestStarted"
            }
            Self::Team(CodingAgentTeamProductEvent::Started { .. }) => "Started",
            Self::Team(CodingAgentTeamProductEvent::MemberStarted { .. }) => "MemberStarted",
            Self::Team(CodingAgentTeamProductEvent::MemberCompleted { .. }) => "MemberCompleted",
            Self::Team(CodingAgentTeamProductEvent::Completed { .. }) => "Completed",
            Self::Team(CodingAgentTeamProductEvent::Failed { .. }) => "Failed",
            Self::Team(CodingAgentTeamProductEvent::Aborted { .. }) => "Aborted",
            Self::Message(CodingAgentMessageProductEvent::Started { .. }) => "Started",
            Self::Message(CodingAgentMessageProductEvent::Delta { .. }) => "Delta",
            Self::Message(CodingAgentMessageProductEvent::ThinkingDelta { .. }) => "ThinkingDelta",
            Self::Message(CodingAgentMessageProductEvent::Completed { .. }) => "Completed",
            Self::Tool(CodingAgentToolProductEvent::Started { .. }) => "Started",
            Self::Tool(CodingAgentToolProductEvent::Updated { .. }) => "Updated",
            Self::Tool(CodingAgentToolProductEvent::Completed { .. }) => "Completed",
            Self::Tool(CodingAgentToolProductEvent::Failed { .. }) => "Failed",
            Self::Runtime(CodingAgentRuntimeProductEvent::CompactionCompleted { .. }) => {
                "CompactionCompleted"
            }
            Self::Runtime(CodingAgentRuntimeProductEvent::ShutDown) => "ShutDown",
            Self::Delegation(CodingAgentDelegationProductEvent::Requested { .. }) => "Requested",
            Self::Delegation(CodingAgentDelegationProductEvent::Rejected { .. }) => "Rejected",
            Self::Delegation(CodingAgentDelegationProductEvent::Approved { .. }) => "Approved",
            Self::Delegation(CodingAgentDelegationProductEvent::ConfirmationRequired {
                ..
            }) => "ConfirmationRequired",
            Self::Delegation(CodingAgentDelegationProductEvent::Started { .. }) => "Started",
            Self::Delegation(CodingAgentDelegationProductEvent::Completed { .. }) => "Completed",
            Self::Delegation(CodingAgentDelegationProductEvent::Failed { .. }) => "Failed",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditStarted { .. }) => {
                "SelfHealingEditStarted"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                ..
            }) => "SelfHealingEditRepairAttempted",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                ..
            }) => "SelfHealingEditCompleted",
            Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditFailed { .. }) => {
                "SelfHealingEditFailed"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptStarted { .. }) => {
                "PromptStarted"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted { .. }) => {
                "PromptCompleted"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptFailed { .. }) => "PromptFailed",
            Self::Workflow(CodingAgentWorkflowProductEvent::PromptAborted { .. }) => {
                "PromptAborted"
            }
            Self::Workflow(CodingAgentWorkflowProductEvent::OperationRecovered { .. }) => {
                "OperationRecovered"
            }
            Self::Diagnostic(CodingAgentDiagnosticProductEvent::Diagnostic { .. }) => "Diagnostic",
            Self::Capability(CodingAgentCapabilityProductEvent::Changed { .. }) => "Changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CodingAgentProductEvent {
    pub sequence: u64,
    #[deprecated(note = "match event() or family_typed() instead")]
    pub family: String,
    #[deprecated(note = "match event() or use kind_name() instead")]
    pub kind: String,
    event: CodingAgentProductEventKind,
    operation_id: Option<String>,
    terminal_status: Option<CodingAgentProductEventTerminalStatus>,
    terminal_operation: Option<CodingAgentProductEventTerminalOperation>,
    durability: CodingAgentProductEventDurability,
}

impl CodingAgentProductEvent {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn event(&self) -> &CodingAgentProductEventKind {
        &self.event
    }
    pub fn family_typed(&self) -> CodingAgentProductEventFamily {
        self.event.family()
    }
    pub fn family(&self) -> CodingAgentProductEventFamily {
        self.event.family()
    }
    pub fn kind_name(&self) -> &'static str {
        self.event.as_str()
    }
    pub fn operation_id(&self) -> Option<&str> {
        self.operation_id.as_deref()
    }
    pub fn terminal_status(&self) -> Option<CodingAgentProductEventTerminalStatus> {
        self.terminal_status
    }
    pub fn terminal_operation(&self) -> Option<CodingAgentProductEventTerminalOperation> {
        self.terminal_operation
    }
    pub fn durability(&self) -> &CodingAgentProductEventDurability {
        &self.durability
    }

    #[allow(deprecated)]
    pub(crate) fn from_internal(source: ProductEvent) -> Self {
        let sequence = source.sequence().get();
        let operation_id = source.operation_id().map(str::to_owned);
        let terminal_status = source.terminal_status().map(Into::into);
        let terminal_operation = source.terminal_operation().map(Into::into);
        let durability = source.durability().clone().into();
        let event = source.event().clone();
        Self {
            sequence,
            family: event.legacy_family_name().to_owned(),
            kind: format!(
                "{}({})",
                event.legacy_family_name(),
                event.legacy_variant_name()
            ),
            event,
            operation_id,
            terminal_status,
            terminal_operation,
            durability,
        }
    }
}

impl From<ProductEventTerminalStatus> for CodingAgentProductEventTerminalStatus {
    fn from(value: ProductEventTerminalStatus) -> Self {
        match value {
            ProductEventTerminalStatus::Completed => Self::Completed,
            ProductEventTerminalStatus::Failed => Self::Failed,
            ProductEventTerminalStatus::Aborted => Self::Aborted,
            ProductEventTerminalStatus::Recovered => Self::Recovered,
        }
    }
}

impl From<ProductEventTerminalOperation> for CodingAgentProductEventTerminalOperation {
    fn from(value: ProductEventTerminalOperation) -> Self {
        let kind = match value.kind {
            OperationKind::Prompt => CodingAgentProductEventTerminalOperationKind::Prompt,
            OperationKind::AgentInvocation => {
                CodingAgentProductEventTerminalOperationKind::AgentInvocation
            }
            OperationKind::AgentTeam => CodingAgentProductEventTerminalOperationKind::AgentTeam,
            OperationKind::SelfHealingEdit => {
                CodingAgentProductEventTerminalOperationKind::SelfHealingEdit
            }
            OperationKind::Compact => CodingAgentProductEventTerminalOperationKind::Compact,
            _ => unreachable!(
                "only the five classified root operations produce terminal associations"
            ),
        };
        Self {
            kind,
            status: value.status.into(),
        }
    }
}

impl From<ProductEventDurability> for CodingAgentProductEventDurability {
    fn from(value: ProductEventDurability) -> Self {
        match value {
            ProductEventDurability::LiveOnly => Self::LiveOnly,
            ProductEventDurability::PendingSessionWrite { operation_id } => {
                Self::PendingSessionWrite { operation_id }
            }
            ProductEventDurability::Durable { session_id } => Self::Durable { session_id },
        }
    }
}

fn error(error: &super::CodingSessionError) -> CodingAgentProductEventError {
    CodingAgentProductEventError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}
fn profile(id: &super::ProfileId) -> String {
    id.as_str().to_owned()
}
fn profile_kind(kind: super::ProfileKind) -> CodingAgentProductEventProfileKind {
    match kind {
        super::ProfileKind::Agent => CodingAgentProductEventProfileKind::Agent,
        super::ProfileKind::Team => CodingAgentProductEventProfileKind::Team,
    }
}
fn delegation_context(
    operation_id: &str,
    turn_id: &str,
    tool_call_id: &str,
    requesting_profile_id: &super::ProfileId,
    target_kind: super::ProfileKind,
    target_id: &super::ProfileId,
    task: &str,
) -> CodingAgentDelegationEventContext {
    CodingAgentDelegationEventContext {
        operation_id: operation_id.to_owned(),
        turn_id: turn_id.to_owned(),
        tool_call_id: tool_call_id.to_owned(),
        requesting_profile_id: profile(requesting_profile_id),
        target_kind: profile_kind(target_kind),
        target_id: profile(target_id),
        task: task.to_owned(),
    }
}
fn check_output(value: &super::SelfHealingEditCheckOutput) -> CodingAgentProductEventCheckOutput {
    CodingAgentProductEventCheckOutput {
        command: value.command.clone(),
        stdout: value.stdout.clone(),
        stderr: value.stderr.clone(),
        exit_code: value.exit_code,
    }
}

impl From<&CodingAgentEvent> for CodingAgentProductEventKind {
    fn from(value: &CodingAgentEvent) -> Self {
        use CodingAgentEvent as E;
        match value {
            E::SessionOpened { session_id } => {
                Self::Session(CodingAgentSessionProductEvent::Opened {
                    session_id: session_id.clone(),
                })
            }
            E::DefaultAgentProfileChanged { profile_id } => {
                Self::Profile(CodingAgentProfileProductEvent::DefaultChanged {
                    profile_id: profile(profile_id),
                })
            }
            E::AgentInvocationStarted {
                operation_id,
                child_operation_id,
                profile_id,
                task,
            } => Self::Agent(CodingAgentAgentProductEvent::InvocationStarted {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile(profile_id),
                task: task.clone(),
            }),
            E::AgentInvocationCompleted {
                operation_id,
                child_operation_id,
                profile_id,
                final_text,
            } => Self::Agent(CodingAgentAgentProductEvent::InvocationCompleted {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile(profile_id),
                final_text: final_text.clone(),
            }),
            E::AgentInvocationFailed {
                operation_id,
                child_operation_id,
                profile_id,
                error: e,
            } => Self::Agent(CodingAgentAgentProductEvent::InvocationFailed {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile(profile_id),
                error: error(e),
            }),
            E::AgentInvocationAborted {
                operation_id,
                child_operation_id,
                profile_id,
                reason,
            } => Self::Agent(CodingAgentAgentProductEvent::InvocationAborted {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile(profile_id),
                reason: reason.clone(),
            }),
            E::AgentTeamStarted {
                operation_id,
                team_id,
                task,
            } => Self::Team(CodingAgentTeamProductEvent::Started {
                operation_id: operation_id.clone(),
                team_id: profile(team_id),
                task: task.clone(),
            }),
            E::AgentTeamMemberStarted {
                operation_id,
                child_operation_id,
                team_id,
                profile_id,
                task,
            } => Self::Team(CodingAgentTeamProductEvent::MemberStarted {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                team_id: profile(team_id),
                profile_id: profile(profile_id),
                task: task.clone(),
            }),
            E::AgentTeamMemberCompleted {
                operation_id,
                child_operation_id,
                team_id,
                profile_id,
                final_text,
            } => Self::Team(CodingAgentTeamProductEvent::MemberCompleted {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                team_id: profile(team_id),
                profile_id: profile(profile_id),
                final_text: final_text.clone(),
            }),
            E::AgentTeamCompleted {
                operation_id,
                team_id,
                final_text,
            } => Self::Team(CodingAgentTeamProductEvent::Completed {
                operation_id: operation_id.clone(),
                team_id: profile(team_id),
                final_text: final_text.clone(),
            }),
            E::AgentTeamFailed {
                operation_id,
                team_id,
                error: e,
            } => Self::Team(CodingAgentTeamProductEvent::Failed {
                operation_id: operation_id.clone(),
                team_id: profile(team_id),
                error: error(e),
            }),
            E::AgentTeamAborted {
                operation_id,
                team_id,
                reason,
            } => Self::Team(CodingAgentTeamProductEvent::Aborted {
                operation_id: operation_id.clone(),
                team_id: profile(team_id),
                reason: reason.clone(),
            }),
            E::SelfHealingEditStarted {
                operation_id,
                path,
                replacements,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                operation_id: operation_id.clone(),
                path: path.clone(),
                replacements: *replacements,
            }),
            E::SelfHealingEditRepairAttempted {
                operation_id,
                path,
                attempt,
                replacements,
                diagnostics,
                check_output: output,
            } => Self::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    operation_id: operation_id.clone(),
                    path: path.clone(),
                    attempt: *attempt,
                    replacements: replacements
                        .iter()
                        .map(|r| CodingAgentProductEventReplacement {
                            old_text: r.old_text.clone(),
                            new_text: r.new_text.clone(),
                        })
                        .collect(),
                    diagnostics: diagnostics
                        .iter()
                        .map(|d| CodingAgentProductEventDiagnostic {
                            message: d.message.clone(),
                        })
                        .collect(),
                    check_output: output.as_ref().map(check_output),
                },
            ),
            E::SelfHealingEditCompleted {
                operation_id,
                path,
                attempts,
                first_changed_line,
                check_output: output,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                operation_id: operation_id.clone(),
                path: path.clone(),
                attempts: *attempts,
                first_changed_line: *first_changed_line,
                check_output: output.as_ref().map(check_output),
            }),
            E::SelfHealingEditFailed {
                operation_id,
                path,
                error: e,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditFailed {
                operation_id: operation_id.clone(),
                path: path.clone(),
                error: error(e),
            }),
            E::DelegationRequested {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Requested {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
            }),
            E::DelegationRejected {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                reason,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Rejected {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
                reason: reason.clone(),
            }),
            E::DelegationApproved {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Approved {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
            }),
            E::DelegationConfirmationRequired {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                reason,
            } => Self::Delegation(CodingAgentDelegationProductEvent::ConfirmationRequired {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
                reason: reason.clone(),
            }),
            E::DelegationStarted {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Started {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
                child_operation_id: child_operation_id.clone(),
            }),
            E::DelegationCompleted {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
                final_text,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Completed {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
                child_operation_id: child_operation_id.clone(),
                final_text: final_text.clone(),
            }),
            E::DelegationFailed {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
                error: e,
            } => Self::Delegation(CodingAgentDelegationProductEvent::Failed {
                context: delegation_context(
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    *target_kind,
                    target_id,
                    task,
                ),
                child_operation_id: child_operation_id.clone(),
                error: error(e),
            }),
            E::SessionWritePending { operation_id } => {
                Self::Session(CodingAgentSessionProductEvent::WritePending {
                    operation_id: operation_id.clone(),
                })
            }
            E::SessionWriteCommitted {
                operation_id,
                session_id,
            } => Self::Session(CodingAgentSessionProductEvent::WriteCommitted {
                operation_id: operation_id.clone(),
                session_id: session_id.clone(),
            }),
            E::SessionWriteSkipped {
                operation_id,
                reason,
            } => Self::Session(CodingAgentSessionProductEvent::WriteSkipped {
                operation_id: operation_id.clone(),
                reason: reason.clone(),
            }),
            E::PromptStarted {
                operation_id,
                turn_id,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::PromptStarted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
            }),
            E::AgentTurnStarted {
                operation_id,
                turn_id,
                agent_turn,
            } => Self::Agent(CodingAgentAgentProductEvent::TurnStarted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                agent_turn: *agent_turn,
            }),
            E::ProviderRequestStarted {
                operation_id,
                turn_id,
                provider,
                model,
            } => Self::Agent(CodingAgentAgentProductEvent::ProviderRequestStarted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                provider: provider.clone(),
                model: model.clone(),
            }),
            E::AssistantMessageStarted {
                operation_id,
                turn_id,
                message_id,
            } => Self::Message(CodingAgentMessageProductEvent::Started {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                message_id: message_id.clone(),
            }),
            E::AssistantMessageDelta {
                operation_id,
                turn_id,
                message_id,
                text,
            } => Self::Message(CodingAgentMessageProductEvent::Delta {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                message_id: message_id.clone(),
                text: text.clone(),
            }),
            E::AssistantThinkingDelta {
                operation_id,
                turn_id,
                message_id,
                text,
            } => Self::Message(CodingAgentMessageProductEvent::ThinkingDelta {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                message_id: message_id.clone(),
                text: text.clone(),
            }),
            E::AssistantMessageCompleted {
                operation_id,
                turn_id,
                message_id,
                final_text,
                usage,
            } => Self::Message(CodingAgentMessageProductEvent::Completed {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                message_id: message_id.clone(),
                final_text: final_text.clone(),
                usage: CodingAgentProductEventUsage {
                    input: usage.input,
                    output: usage.output,
                    cache_read: usage.cache_read,
                    cache_write: usage.cache_write,
                    total_tokens: usage.total_tokens,
                    input_cost: usage.cost.input,
                    output_cost: usage.cost.output,
                    cache_read_cost: usage.cost.cache_read,
                    cache_write_cost: usage.cost.cache_write,
                },
            }),
            E::ToolCallStarted {
                operation_id,
                turn_id,
                tool_call_id,
                name,
                arguments_json,
            } => Self::Tool(CodingAgentToolProductEvent::Started {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                arguments_json: arguments_json.clone(),
            }),
            E::ToolCallUpdated {
                operation_id,
                turn_id,
                tool_call_id,
                name,
                message,
            } => Self::Tool(CodingAgentToolProductEvent::Updated {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                message: message.clone(),
            }),
            E::ToolCallCompleted {
                operation_id,
                turn_id,
                tool_call_id,
                name,
                summary,
            } => Self::Tool(CodingAgentToolProductEvent::Completed {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                summary: summary.clone(),
            }),
            E::ToolCallFailed {
                operation_id,
                turn_id,
                tool_call_id,
                name,
                message,
            } => Self::Tool(CodingAgentToolProductEvent::Failed {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                message: message.clone(),
            }),
            E::RuntimeCompactionCompleted {
                operation_id,
                turn_id,
                summary,
                first_kept_message_id,
                tokens_before,
            } => Self::Runtime(CodingAgentRuntimeProductEvent::CompactionCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                summary: summary.clone(),
                first_kept_message_id: first_kept_message_id.clone(),
                tokens_before: *tokens_before,
            }),
            E::RuntimeShutDown => Self::Runtime(CodingAgentRuntimeProductEvent::ShutDown),
            E::SessionCompactionCompleted {
                operation_id,
                turn_id,
                summary,
                first_kept_message_id,
                tokens_before,
            } => Self::Session(CodingAgentSessionProductEvent::CompactionCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                summary: summary.clone(),
                first_kept_message_id: first_kept_message_id.clone(),
                tokens_before: *tokens_before,
            }),
            E::PromptCompleted {
                operation_id,
                turn_id,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
            }),
            E::PromptFailed {
                operation_id,
                error: e,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::PromptFailed {
                operation_id: operation_id.clone(),
                error: error(e),
            }),
            E::PromptAborted {
                operation_id,
                reason,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::PromptAborted {
                operation_id: operation_id.clone(),
                reason: reason.clone(),
            }),
            E::Diagnostic {
                operation_id,
                message,
            } => Self::Diagnostic(CodingAgentDiagnosticProductEvent::Diagnostic {
                operation_id: operation_id.clone(),
                message: message.clone(),
            }),
            E::CapabilityChanged {
                generation,
                revocation,
            } => Self::Capability(CodingAgentCapabilityProductEvent::Changed {
                generation: *generation,
                revocation: match revocation {
                    super::CapabilityRevocationPolicy::FutureOnly => {
                        CodingAgentProductEventCapabilityRevocation::FutureOnly
                    }
                    super::CapabilityRevocationPolicy::CancelMatchingOperations => {
                        CodingAgentProductEventCapabilityRevocation::CancelMatchingOperations
                    }
                },
            }),
            E::OperationRecovered {
                operation_id,
                recovery_id,
                reason,
            } => Self::Workflow(CodingAgentWorkflowProductEvent::OperationRecovered {
                operation_id: operation_id.clone(),
                recovery_id: recovery_id.clone(),
                reason: reason.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::event::ProductEventSequence;
    use crate::coding_session::{
        ProfileId, ProfileKind, SelfHealingEditCheckOutput, SelfHealingEditDiagnostic,
        SelfHealingEditReplacement,
    };
    use pi_ai::types::{Cost, Usage};

    // product-event-inventory:start
    const EXPECTED_PUBLIC_EVENT_INVENTORY: [(&str, CodingAgentProductEventFamily, &str); 46] = [
        (
            "SessionOpened",
            CodingAgentProductEventFamily::Session,
            "opened",
        ),
        (
            "SessionWritePending",
            CodingAgentProductEventFamily::Session,
            "write_pending",
        ),
        (
            "SessionWriteCommitted",
            CodingAgentProductEventFamily::Session,
            "write_committed",
        ),
        (
            "SessionWriteSkipped",
            CodingAgentProductEventFamily::Session,
            "write_skipped",
        ),
        (
            "SessionCompactionCompleted",
            CodingAgentProductEventFamily::Session,
            "compaction_completed",
        ),
        (
            "DefaultAgentProfileChanged",
            CodingAgentProductEventFamily::Profile,
            "default_changed",
        ),
        (
            "AgentInvocationStarted",
            CodingAgentProductEventFamily::Agent,
            "invocation_started",
        ),
        (
            "AgentInvocationCompleted",
            CodingAgentProductEventFamily::Agent,
            "invocation_completed",
        ),
        (
            "AgentInvocationFailed",
            CodingAgentProductEventFamily::Agent,
            "invocation_failed",
        ),
        (
            "AgentInvocationAborted",
            CodingAgentProductEventFamily::Agent,
            "invocation_aborted",
        ),
        (
            "AgentTurnStarted",
            CodingAgentProductEventFamily::Agent,
            "turn_started",
        ),
        (
            "ProviderRequestStarted",
            CodingAgentProductEventFamily::Agent,
            "provider_request_started",
        ),
        (
            "AgentTeamStarted",
            CodingAgentProductEventFamily::Team,
            "started",
        ),
        (
            "AgentTeamMemberStarted",
            CodingAgentProductEventFamily::Team,
            "member_started",
        ),
        (
            "AgentTeamMemberCompleted",
            CodingAgentProductEventFamily::Team,
            "member_completed",
        ),
        (
            "AgentTeamCompleted",
            CodingAgentProductEventFamily::Team,
            "completed",
        ),
        (
            "AgentTeamFailed",
            CodingAgentProductEventFamily::Team,
            "failed",
        ),
        (
            "AgentTeamAborted",
            CodingAgentProductEventFamily::Team,
            "aborted",
        ),
        (
            "AssistantMessageStarted",
            CodingAgentProductEventFamily::Message,
            "started",
        ),
        (
            "AssistantMessageDelta",
            CodingAgentProductEventFamily::Message,
            "delta",
        ),
        (
            "AssistantThinkingDelta",
            CodingAgentProductEventFamily::Message,
            "thinking_delta",
        ),
        (
            "AssistantMessageCompleted",
            CodingAgentProductEventFamily::Message,
            "completed",
        ),
        (
            "ToolCallStarted",
            CodingAgentProductEventFamily::Tool,
            "started",
        ),
        (
            "ToolCallUpdated",
            CodingAgentProductEventFamily::Tool,
            "updated",
        ),
        (
            "ToolCallCompleted",
            CodingAgentProductEventFamily::Tool,
            "completed",
        ),
        (
            "ToolCallFailed",
            CodingAgentProductEventFamily::Tool,
            "failed",
        ),
        (
            "RuntimeCompactionCompleted",
            CodingAgentProductEventFamily::Runtime,
            "compaction_completed",
        ),
        (
            "RuntimeShutDown",
            CodingAgentProductEventFamily::Runtime,
            "shut_down",
        ),
        (
            "DelegationRequested",
            CodingAgentProductEventFamily::Delegation,
            "requested",
        ),
        (
            "DelegationRejected",
            CodingAgentProductEventFamily::Delegation,
            "rejected",
        ),
        (
            "DelegationApproved",
            CodingAgentProductEventFamily::Delegation,
            "approved",
        ),
        (
            "DelegationConfirmationRequired",
            CodingAgentProductEventFamily::Delegation,
            "confirmation_required",
        ),
        (
            "DelegationStarted",
            CodingAgentProductEventFamily::Delegation,
            "started",
        ),
        (
            "DelegationCompleted",
            CodingAgentProductEventFamily::Delegation,
            "completed",
        ),
        (
            "DelegationFailed",
            CodingAgentProductEventFamily::Delegation,
            "failed",
        ),
        (
            "SelfHealingEditStarted",
            CodingAgentProductEventFamily::Workflow,
            "self_healing_edit_started",
        ),
        (
            "SelfHealingEditRepairAttempted",
            CodingAgentProductEventFamily::Workflow,
            "self_healing_edit_repair_attempted",
        ),
        (
            "SelfHealingEditCompleted",
            CodingAgentProductEventFamily::Workflow,
            "self_healing_edit_completed",
        ),
        (
            "SelfHealingEditFailed",
            CodingAgentProductEventFamily::Workflow,
            "self_healing_edit_failed",
        ),
        (
            "PromptStarted",
            CodingAgentProductEventFamily::Workflow,
            "prompt_started",
        ),
        (
            "PromptCompleted",
            CodingAgentProductEventFamily::Workflow,
            "prompt_completed",
        ),
        (
            "PromptFailed",
            CodingAgentProductEventFamily::Workflow,
            "prompt_failed",
        ),
        (
            "PromptAborted",
            CodingAgentProductEventFamily::Workflow,
            "prompt_aborted",
        ),
        (
            "OperationRecovered",
            CodingAgentProductEventFamily::Workflow,
            "operation_recovered",
        ),
        (
            "Diagnostic",
            CodingAgentProductEventFamily::Diagnostic,
            "diagnostic",
        ),
        (
            "CapabilityChanged",
            CodingAgentProductEventFamily::Capability,
            "changed",
        ),
    ];
    // product-event-inventory:end

    fn project(sequence: u64, event: CodingAgentEvent) -> CodingAgentProductEvent {
        CodingAgentProductEvent::from_internal(ProductEvent::from_event_for_tests(
            ProductEventSequence::new(sequence),
            event,
        ))
    }

    fn exhaustive_inventory_fixture() -> Vec<CodingAgentProductEvent> {
        let pid = || ProfileId::from("profile");
        let error = || super::super::CodingSessionError::UnsupportedCapability {
            capability: "fixture".into(),
        };
        let delegation = || {
            (
                "op".to_owned(),
                "turn".to_owned(),
                "call".to_owned(),
                pid(),
                ProfileKind::Agent,
                pid(),
                "task".to_owned(),
            )
        };
        let usage = Usage {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            total_tokens: 10,
            cost: Cost::default(),
        };
        // product-event-fixture:start
        let mut events = vec![
            CodingAgentEvent::SessionOpened {
                session_id: "session".into(),
            },
            CodingAgentEvent::SessionWritePending {
                operation_id: "op".into(),
            },
            CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op".into(),
                session_id: "session".into(),
            },
            CodingAgentEvent::SessionWriteSkipped {
                operation_id: "op".into(),
                reason: "skip".into(),
            },
            CodingAgentEvent::SessionCompactionCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                summary: "summary".into(),
                first_kept_message_id: "message".into(),
                tokens_before: 5,
            },
            CodingAgentEvent::DefaultAgentProfileChanged { profile_id: pid() },
            CodingAgentEvent::AgentInvocationStarted {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                profile_id: pid(),
                task: "task".into(),
            },
            CodingAgentEvent::AgentInvocationCompleted {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                profile_id: pid(),
                final_text: "done".into(),
            },
            CodingAgentEvent::AgentInvocationFailed {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                profile_id: pid(),
                error: error(),
            },
            CodingAgentEvent::AgentInvocationAborted {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                profile_id: pid(),
                reason: "abort".into(),
            },
            CodingAgentEvent::AgentTurnStarted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                agent_turn: 1,
            },
            CodingAgentEvent::ProviderRequestStarted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                provider: "faux".into(),
                model: "model".into(),
            },
            CodingAgentEvent::AgentTeamStarted {
                operation_id: "op".into(),
                team_id: pid(),
                task: "task".into(),
            },
            CodingAgentEvent::AgentTeamMemberStarted {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                team_id: pid(),
                profile_id: pid(),
                task: "task".into(),
            },
            CodingAgentEvent::AgentTeamMemberCompleted {
                operation_id: "op".into(),
                child_operation_id: "child".into(),
                team_id: pid(),
                profile_id: pid(),
                final_text: "done".into(),
            },
            CodingAgentEvent::AgentTeamCompleted {
                operation_id: "op".into(),
                team_id: pid(),
                final_text: "done".into(),
            },
            CodingAgentEvent::AgentTeamFailed {
                operation_id: "op".into(),
                team_id: pid(),
                error: error(),
            },
            CodingAgentEvent::AgentTeamAborted {
                operation_id: "op".into(),
                team_id: pid(),
                reason: "abort".into(),
            },
            CodingAgentEvent::AssistantMessageStarted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                message_id: None,
            },
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                message_id: Some("message".into()),
                text: "delta".into(),
            },
            CodingAgentEvent::AssistantThinkingDelta {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                message_id: None,
                text: "thinking".into(),
            },
            CodingAgentEvent::AssistantMessageCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                message_id: Some("message".into()),
                final_text: "done".into(),
                usage,
            },
            CodingAgentEvent::ToolCallStarted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                tool_call_id: "call".into(),
                name: "read".into(),
                arguments_json: "{}".into(),
            },
            CodingAgentEvent::ToolCallUpdated {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                tool_call_id: "call".into(),
                name: "read".into(),
                message: "running".into(),
            },
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                tool_call_id: "call".into(),
                name: "read".into(),
                summary: "done".into(),
            },
            CodingAgentEvent::ToolCallFailed {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                tool_call_id: "call".into(),
                name: "read".into(),
                message: "failed".into(),
            },
            CodingAgentEvent::RuntimeCompactionCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                summary: "summary".into(),
                first_kept_message_id: "message".into(),
                tokens_before: 5,
            },
            CodingAgentEvent::RuntimeShutDown,
        ];
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationRequested {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationRejected {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            reason: "rejected".into(),
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationApproved {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationConfirmationRequired {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            reason: "confirm".into(),
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationStarted {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            child_operation_id: "child".into(),
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationCompleted {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            child_operation_id: "child".into(),
            final_text: "done".into(),
        });
        let (
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        ) = delegation();
        events.push(CodingAgentEvent::DelegationFailed {
            operation_id,
            turn_id,
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            child_operation_id: "child".into(),
            error: error(),
        });
        events.extend([
            CodingAgentEvent::SelfHealingEditStarted {
                operation_id: "op".into(),
                path: "file".into(),
                replacements: 1,
            },
            CodingAgentEvent::SelfHealingEditRepairAttempted {
                operation_id: "op".into(),
                path: "file".into(),
                attempt: 1,
                replacements: vec![SelfHealingEditReplacement::new("old", "new")],
                diagnostics: vec![SelfHealingEditDiagnostic {
                    message: "diagnostic".into(),
                }],
                check_output: Some(SelfHealingEditCheckOutput {
                    command: "check".into(),
                    stdout: "out".into(),
                    stderr: String::new(),
                    exit_code: 0,
                }),
            },
            CodingAgentEvent::SelfHealingEditCompleted {
                operation_id: "op".into(),
                path: "file".into(),
                attempts: 1,
                first_changed_line: Some(1),
                check_output: None,
            },
            CodingAgentEvent::SelfHealingEditFailed {
                operation_id: "op".into(),
                path: "file".into(),
                error: error(),
            },
            CodingAgentEvent::PromptStarted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
            },
            CodingAgentEvent::PromptCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
            },
            CodingAgentEvent::PromptFailed {
                operation_id: "op".into(),
                error: error(),
            },
            CodingAgentEvent::PromptAborted {
                operation_id: "op".into(),
                reason: "abort".into(),
            },
            CodingAgentEvent::OperationRecovered {
                operation_id: "op".into(),
                recovery_id: "recovery".into(),
                reason: "restart".into(),
            },
            CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "diagnostic".into(),
            },
            CodingAgentEvent::CapabilityChanged {
                generation: 2,
                revocation: super::super::CapabilityRevocationPolicy::FutureOnly,
            },
        ]);
        // product-event-fixture:end
        events
            .into_iter()
            .enumerate()
            .map(|(index, event)| project(index as u64 + 1, event))
            .collect()
    }

    #[test]
    fn typed_contract_has_stable_names_and_independent_metadata() {
        let pending = project(
            7,
            CodingAgentEvent::SessionWritePending {
                operation_id: "op-7".into(),
            },
        );
        assert_eq!(
            pending.family_typed(),
            CodingAgentProductEventFamily::Session
        );
        assert_eq!(pending.kind_name(), "write_pending");
        assert_eq!(pending.operation_id(), Some("op-7"));
        assert_eq!(pending.terminal_status(), None);
        assert_eq!(pending.terminal_operation(), None);
        assert_eq!(
            pending.durability(),
            &CodingAgentProductEventDurability::PendingSessionWrite {
                operation_id: "op-7".into()
            }
        );
        assert_eq!(
            serde_json::to_string(&CodingAgentProductEventDurability::LiveOnly).unwrap(),
            "{\"state\":\"live_only\"}"
        );
    }

    #[test]
    fn event_terminal_does_not_imply_root_operation_terminal() {
        let tool = project(
            8,
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
                tool_call_id: "call".into(),
                name: "read".into(),
                summary: "ok".into(),
            },
        );
        assert_eq!(
            tool.terminal_status(),
            Some(CodingAgentProductEventTerminalStatus::Completed)
        );
        assert_eq!(tool.terminal_operation(), None);
        let prompt = project(
            9,
            CodingAgentEvent::PromptCompleted {
                operation_id: "op".into(),
                turn_id: "turn".into(),
            },
        );
        assert_eq!(
            prompt.terminal_operation().unwrap().kind,
            CodingAgentProductEventTerminalOperationKind::Prompt
        );
    }

    #[test]
    fn exhaustive_inventory_covers_all_current_variants() {
        let projected = exhaustive_inventory_fixture();
        assert_eq!(projected.len(), EXPECTED_PUBLIC_EVENT_INVENTORY.len());
        assert_eq!(projected.len(), 46);
        for (index, (event, (_, family, kind))) in projected
            .iter()
            .zip(EXPECTED_PUBLIC_EVENT_INVENTORY.iter())
            .enumerate()
        {
            assert_eq!(event.sequence(), index as u64 + 1, "inventory row {index}");
            assert_eq!(event.family(), *family, "inventory row {index}");
            assert_eq!(event.kind_name(), *kind, "inventory row {index}");
            assert_public_inventory_payload(index, event);
        }
        let expected_counts = [5, 1, 6, 6, 4, 4, 2, 7, 9, 1, 1];
        let families = [
            CodingAgentProductEventFamily::Session,
            CodingAgentProductEventFamily::Profile,
            CodingAgentProductEventFamily::Agent,
            CodingAgentProductEventFamily::Team,
            CodingAgentProductEventFamily::Message,
            CodingAgentProductEventFamily::Tool,
            CodingAgentProductEventFamily::Runtime,
            CodingAgentProductEventFamily::Delegation,
            CodingAgentProductEventFamily::Workflow,
            CodingAgentProductEventFamily::Diagnostic,
            CodingAgentProductEventFamily::Capability,
        ];
        let counts: Vec<_> = families
            .iter()
            .map(|family| {
                projected
                    .iter()
                    .filter(|event| event.family() == *family)
                    .count()
            })
            .collect();
        assert_eq!(counts, expected_counts);
        assert_eq!(projected[0].operation_id(), None);
        assert_eq!(
            projected[1].durability(),
            &CodingAgentProductEventDurability::PendingSessionWrite {
                operation_id: "op".into()
            }
        );
        assert_eq!(
            projected[2].durability(),
            &CodingAgentProductEventDurability::Durable {
                session_id: "session".into()
            }
        );
        assert_eq!(
            projected[3].durability(),
            &CodingAgentProductEventDurability::LiveOnly
        );
        assert_eq!(projected[27].operation_id(), None);
        assert_eq!(projected[44].operation_id(), None);
        assert_eq!(projected[45].operation_id(), None);
        assert_eq!(
            projected[43].terminal_status(),
            Some(CodingAgentProductEventTerminalStatus::Recovered)
        );
        assert_eq!(projected[43].terminal_operation(), None);
        assert_eq!(
            projected
                .iter()
                .map(CodingAgentProductEvent::sequence)
                .collect::<Vec<_>>(),
            (1..=46).collect::<Vec<_>>()
        );
    }

    fn assert_public_inventory_payload(index: usize, event: &CodingAgentProductEvent) {
        use CodingAgentProductEventKind as K;
        let valid = match (index, event.event()) {
            (0, K::Session(CodingAgentSessionProductEvent::Opened { session_id })) => {
                session_id == "session"
            }
            (1, K::Session(CodingAgentSessionProductEvent::WritePending { operation_id })) => {
                operation_id == "op"
            }
            (2, K::Session(CodingAgentSessionProductEvent::WriteCommitted { session_id, .. })) => {
                session_id == "session"
            }
            (3, K::Session(CodingAgentSessionProductEvent::WriteSkipped { reason, .. })) => {
                reason == "skip"
            }
            (
                4,
                K::Session(CodingAgentSessionProductEvent::CompactionCompleted {
                    turn_id,
                    tokens_before,
                    ..
                }),
            ) => turn_id == "turn" && *tokens_before == 5,
            (5, K::Profile(CodingAgentProfileProductEvent::DefaultChanged { profile_id })) => {
                profile_id == "profile"
            }
            (
                6,
                K::Agent(CodingAgentAgentProductEvent::InvocationStarted {
                    child_operation_id,
                    task,
                    ..
                }),
            ) => child_operation_id == "child" && task == "task",
            (7, K::Agent(CodingAgentAgentProductEvent::InvocationCompleted { final_text, .. })) => {
                final_text == "done"
            }
            (8, K::Agent(CodingAgentAgentProductEvent::InvocationFailed { error, .. })) => {
                error.code == "unsupported_capability"
            }
            (9, K::Agent(CodingAgentAgentProductEvent::InvocationAborted { reason, .. })) => {
                reason == "abort"
            }
            (10, K::Agent(CodingAgentAgentProductEvent::TurnStarted { agent_turn, .. })) => {
                *agent_turn == 1
            }
            (
                11,
                K::Agent(CodingAgentAgentProductEvent::ProviderRequestStarted {
                    provider,
                    model,
                    ..
                }),
            ) => provider == "faux" && model == "model",
            (12, K::Team(CodingAgentTeamProductEvent::Started { team_id, task, .. })) => {
                team_id == "profile" && task == "task"
            }
            (
                13,
                K::Team(CodingAgentTeamProductEvent::MemberStarted {
                    child_operation_id,
                    profile_id,
                    ..
                }),
            ) => child_operation_id == "child" && profile_id == "profile",
            (14, K::Team(CodingAgentTeamProductEvent::MemberCompleted { final_text, .. })) => {
                final_text == "done"
            }
            (15, K::Team(CodingAgentTeamProductEvent::Completed { final_text, .. })) => {
                final_text == "done"
            }
            (16, K::Team(CodingAgentTeamProductEvent::Failed { error, .. })) => {
                error.code == "unsupported_capability"
            }
            (17, K::Team(CodingAgentTeamProductEvent::Aborted { reason, .. })) => reason == "abort",
            (18, K::Message(CodingAgentMessageProductEvent::Started { message_id, .. })) => {
                message_id.is_none()
            }
            (
                19,
                K::Message(CodingAgentMessageProductEvent::Delta {
                    message_id, text, ..
                }),
            ) => message_id.as_deref() == Some("message") && text == "delta",
            (20, K::Message(CodingAgentMessageProductEvent::ThinkingDelta { text, .. })) => {
                text == "thinking"
            }
            (
                21,
                K::Message(CodingAgentMessageProductEvent::Completed {
                    final_text, usage, ..
                }),
            ) => final_text == "done" && usage.total_tokens == 10,
            (
                22,
                K::Tool(CodingAgentToolProductEvent::Started {
                    tool_call_id,
                    arguments_json,
                    ..
                }),
            ) => tool_call_id == "call" && arguments_json == "{}",
            (23, K::Tool(CodingAgentToolProductEvent::Updated { message, .. })) => {
                message == "running"
            }
            (24, K::Tool(CodingAgentToolProductEvent::Completed { summary, .. })) => {
                summary == "done"
            }
            (25, K::Tool(CodingAgentToolProductEvent::Failed { message, .. })) => {
                message == "failed"
            }
            (
                26,
                K::Runtime(CodingAgentRuntimeProductEvent::CompactionCompleted {
                    first_kept_message_id,
                    tokens_before,
                    ..
                }),
            ) => first_kept_message_id == "message" && *tokens_before == 5,
            (27, K::Runtime(CodingAgentRuntimeProductEvent::ShutDown)) => true,
            (28, K::Delegation(CodingAgentDelegationProductEvent::Requested { context })) => {
                context.target_kind == CodingAgentProductEventProfileKind::Agent
                    && context.task == "task"
            }
            (29, K::Delegation(CodingAgentDelegationProductEvent::Rejected { reason, .. })) => {
                reason == "rejected"
            }
            (30, K::Delegation(CodingAgentDelegationProductEvent::Approved { context })) => {
                context.tool_call_id == "call"
            }
            (
                31,
                K::Delegation(CodingAgentDelegationProductEvent::ConfirmationRequired {
                    reason,
                    ..
                }),
            ) => reason == "confirm",
            (
                32,
                K::Delegation(CodingAgentDelegationProductEvent::Started {
                    child_operation_id,
                    ..
                }),
            ) => child_operation_id == "child",
            (
                33,
                K::Delegation(CodingAgentDelegationProductEvent::Completed { final_text, .. }),
            ) => final_text == "done",
            (34, K::Delegation(CodingAgentDelegationProductEvent::Failed { error, .. })) => {
                error.code == "unsupported_capability"
            }
            (
                35,
                K::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                    path,
                    replacements,
                    ..
                }),
            ) => path == "file" && *replacements == 1,
            (
                36,
                K::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    attempt,
                    replacements,
                    diagnostics,
                    check_output,
                    ..
                }),
            ) => {
                *attempt == 1
                    && replacements[0].old_text == "old"
                    && diagnostics[0].message == "diagnostic"
                    && check_output
                        .as_ref()
                        .is_some_and(|output| output.command == "check")
            }
            (
                37,
                K::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    attempts,
                    first_changed_line,
                    ..
                }),
            ) => *attempts == 1 && *first_changed_line == Some(1),
            (
                38,
                K::Workflow(CodingAgentWorkflowProductEvent::SelfHealingEditFailed {
                    error, ..
                }),
            ) => error.code == "unsupported_capability",
            (39, K::Workflow(CodingAgentWorkflowProductEvent::PromptStarted { turn_id, .. })) => {
                turn_id == "turn"
            }
            (40, K::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted { turn_id, .. })) => {
                turn_id == "turn"
            }
            (41, K::Workflow(CodingAgentWorkflowProductEvent::PromptFailed { error, .. })) => {
                error.code == "unsupported_capability"
            }
            (42, K::Workflow(CodingAgentWorkflowProductEvent::PromptAborted { reason, .. })) => {
                reason == "abort"
            }
            (
                43,
                K::Workflow(CodingAgentWorkflowProductEvent::OperationRecovered {
                    recovery_id,
                    reason,
                    ..
                }),
            ) => recovery_id == "recovery" && reason == "restart",
            (
                44,
                K::Diagnostic(CodingAgentDiagnosticProductEvent::Diagnostic {
                    operation_id,
                    message,
                }),
            ) => operation_id.is_none() && message == "diagnostic",
            (
                45,
                K::Capability(CodingAgentCapabilityProductEvent::Changed {
                    generation,
                    revocation,
                }),
            ) => {
                *generation == 2
                    && *revocation == CodingAgentProductEventCapabilityRevocation::FutureOnly
            }
            _ => false,
        };
        assert!(
            valid,
            "typed payload mismatch at inventory row {index}: {:?}",
            event.event()
        );
    }
}
