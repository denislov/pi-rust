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
        let event = CodingAgentProductEventKind::from(source.compatibility_event());
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

    fn project(sequence: u64, event: CodingAgentEvent) -> CodingAgentProductEvent {
        CodingAgentProductEvent::from_internal(ProductEvent::from_compat_event(
            ProductEventSequence::new(sequence),
            event,
        ))
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
}
