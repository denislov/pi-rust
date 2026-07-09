use crate::coding_session::{
    CodingAgentEvent, ProductEvent, ProductEventSequence, ProfileKind, UiSnapshot,
};
use pi_ai::types::Usage;

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
    DelegationBlock {
        call_id: String,
        target_kind: String,
        target_id: String,
        task: String,
        status: String,
        child_operation_id: Option<String>,
        summary: Option<String>,
        is_error: bool,
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

#[derive(Debug, Clone)]
pub(crate) struct UiProjection {
    bridge: CodingEventBridge,
    last_sequence: ProductEventSequence,
    pending: Vec<UiEvent>,
}

impl Default for UiProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl UiProjection {
    pub(crate) fn new() -> Self {
        Self {
            bridge: CodingEventBridge::new(),
            last_sequence: ProductEventSequence::default(),
            pending: Vec::new(),
        }
    }

    pub(crate) fn from_snapshot(snapshot: UiSnapshot) -> Self {
        Self {
            bridge: CodingEventBridge::new(),
            last_sequence: snapshot.cursor.last_event_sequence,
            pending: snapshot_hydration_events(&snapshot),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn last_sequence(&self) -> ProductEventSequence {
        self.last_sequence
    }

    pub(crate) fn apply_product_event(&mut self, event: &ProductEvent) {
        if event.sequence() <= self.last_sequence {
            return;
        }
        self.last_sequence = event.sequence();
        self.pending.extend(self.bridge.push_product_event(event));
    }

    pub(crate) fn drain(&mut self) -> Vec<UiEvent> {
        self.pending.drain(..).collect()
    }
}

fn snapshot_hydration_events(snapshot: &UiSnapshot) -> Vec<UiEvent> {
    let _ = snapshot;
    Vec::new()
}

/// Stateless event bridge: converts `CodingAgentEvent` to `Vec<UiEvent>`.
///
/// No longer accumulates tokens — `UiEvent::UsageUpdate` carries per-event
/// delta values. The receiver (`InteractiveRoot::apply_events`) accumulates
/// them into `FooterStats`.
#[derive(Debug, Clone)]
pub struct CodingEventBridge;

/// Estimate current context size from a usage snapshot.
/// Mirrors `pi-agent-core::compaction::estimate::calculate_context_tokens`
/// and the TS `getContextUsage` use of the latest assistant usage.
fn calculate_context_tokens(usage: &Usage) -> u32 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write)
    }
}

impl Default for CodingEventBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl CodingEventBridge {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn push_product_event(&mut self, event: &ProductEvent) -> Vec<UiEvent> {
        self.handle(event.compatibility_event())
    }

    pub(crate) fn handle_product_event(&mut self, event: &ProductEvent) -> Vec<UiEvent> {
        self.push_product_event(event)
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
            CodingAgentEvent::AssistantMessageCompleted { usage, .. } => {
                // Emit per-event delta values. The receiver accumulates.
                let context_tokens = match calculate_context_tokens(usage) {
                    0 => None,
                    tokens => Some(tokens),
                };
                vec![
                    UiEvent::AssistantDone,
                    UiEvent::UsageUpdate {
                        input: usage.input,
                        output: usage.output,
                        cache_read: usage.cache_read,
                        cache_write: usage.cache_write,
                        cost: usage.cost.input
                            + usage.cost.output
                            + usage.cost.cache_read
                            + usage.cost.cache_write,
                        context_tokens,
                    },
                ]
            }
            CodingAgentEvent::ToolCallStarted {
                tool_call_id,
                name,
                arguments_json,
                ..
            } => {
                if let Some(event) =
                    delegation_block_from_tool_start(tool_call_id, name, arguments_json)
                {
                    vec![event]
                } else {
                    vec![UiEvent::ToolStarted {
                        call_id: tool_call_id.clone(),
                        name: name.clone(),
                        args: parse_tool_arguments(arguments_json),
                    }]
                }
            }
            CodingAgentEvent::ToolCallUpdated {
                tool_call_id,
                name: _,
                message,
                ..
            } => {
                vec![UiEvent::ToolUpdated {
                    call_id: tool_call_id.clone(),
                    result: message.clone(),
                }]
            }
            CodingAgentEvent::ToolCallCompleted {
                tool_call_id,
                name,
                summary,
                ..
            } => {
                if let Some(event) = delegation_block_from_tool_result(tool_call_id, name, summary)
                {
                    vec![event]
                } else {
                    vec![UiEvent::ToolFinished {
                        call_id: tool_call_id.clone(),
                        result: summary.clone(),
                        is_error: false,
                    }]
                }
            }
            CodingAgentEvent::ToolCallFailed {
                tool_call_id,
                name,
                message,
                ..
            } => {
                if is_delegation_tool(name) {
                    vec![UiEvent::DelegationBlock {
                        call_id: tool_call_id.clone(),
                        target_kind: delegation_tool_kind_label(name)
                            .unwrap_or("agent")
                            .to_string(),
                        target_id: String::new(),
                        task: String::new(),
                        status: "failed".to_string(),
                        child_operation_id: None,
                        summary: Some(format!("failed: {message}")),
                        is_error: true,
                    }]
                } else {
                    vec![UiEvent::ToolFinished {
                        call_id: tool_call_id.clone(),
                        result: message.clone(),
                        is_error: true,
                    }]
                }
            }
            CodingAgentEvent::RuntimeCompactionCompleted { summary, .. }
            | CodingAgentEvent::SessionCompactionCompleted { summary, .. } => vec![
                UiEvent::CompactionNotice {
                    summary: summary.clone(),
                },
                // Compaction doesn't consume tokens — emit zero delta.
                // Only context_tokens is reset to None.
                UiEvent::UsageUpdate {
                    input: 0,
                    output: 0,
                    cache_read: 0,
                    cache_write: 0,
                    cost: 0.0,
                    context_tokens: None,
                },
            ],
            CodingAgentEvent::PromptFailed { error, .. } => vec![UiEvent::AgentError {
                error: error.to_string(),
            }],
            CodingAgentEvent::PromptAborted { reason, .. } => vec![UiEvent::AgentError {
                error: format!("prompt aborted: {reason}"),
            }],
            CodingAgentEvent::DelegationRequested {
                tool_call_id,
                target_kind,
                target_id,
                task,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "requested".to_string(),
                child_operation_id: None,
                summary: Some("requested".to_string()),
                is_error: false,
            }],
            CodingAgentEvent::DelegationConfirmationRequired {
                operation_id,
                tool_call_id,
                target_kind,
                target_id,
                task,
                reason,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "confirmation_required".to_string(),
                child_operation_id: None,
                summary: Some(format!(
                    "confirmation required: {reason}\nApprove: /delegation approve {operation_id} {tool_call_id}\nReject: /delegation reject {operation_id} {tool_call_id} [reason]\nList pending: /delegations"
                )),
                is_error: false,
            }],
            CodingAgentEvent::DelegationApproved {
                tool_call_id,
                target_kind,
                target_id,
                task,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "approved".to_string(),
                child_operation_id: None,
                summary: Some("approved".to_string()),
                is_error: false,
            }],
            CodingAgentEvent::DelegationRejected {
                tool_call_id,
                target_kind,
                target_id,
                task,
                reason,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "rejected".to_string(),
                child_operation_id: None,
                summary: Some(format!("rejected: {reason}")),
                is_error: true,
            }],
            CodingAgentEvent::DelegationStarted {
                tool_call_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "running".to_string(),
                child_operation_id: Some(child_operation_id.clone()),
                summary: None,
                is_error: false,
            }],
            CodingAgentEvent::DelegationCompleted {
                tool_call_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
                final_text,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "completed".to_string(),
                child_operation_id: Some(child_operation_id.clone()),
                summary: Some(format!("completed: {final_text}")),
                is_error: false,
            }],
            CodingAgentEvent::DelegationFailed {
                tool_call_id,
                target_kind,
                target_id,
                task,
                child_operation_id,
                error,
                ..
            } => vec![UiEvent::DelegationBlock {
                call_id: tool_call_id.clone(),
                target_kind: profile_kind_label(*target_kind).to_string(),
                target_id: target_id.to_string(),
                task: task.clone(),
                status: "failed".to_string(),
                child_operation_id: Some(child_operation_id.clone()),
                summary: Some(format!("failed: {error}")),
                is_error: true,
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
            CodingAgentEvent::OperationRecovered {
                operation_id,
                reason,
                ..
            } => vec![UiEvent::SystemNotice {
                text: format!("Recovered incomplete operation {operation_id}: {reason}"),
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
            | CodingAgentEvent::SessionWritePending { .. }
            | CodingAgentEvent::SessionWriteCommitted { .. }
            | CodingAgentEvent::SessionWriteSkipped { .. }
            | CodingAgentEvent::PromptStarted { .. }
            | CodingAgentEvent::ProviderRequestStarted { .. }
            | CodingAgentEvent::AssistantMessageStarted { .. }
            | CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::Diagnostic { .. }
            | CodingAgentEvent::CapabilityChanged { .. } => Vec::new(),
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

fn delegation_block_from_tool_start(
    tool_call_id: &str,
    tool_name: &str,
    arguments_json: &str,
) -> Option<UiEvent> {
    let target_kind = delegation_tool_kind_label(tool_name)?;
    let args = parse_tool_arguments(arguments_json);
    let target_id_key = delegation_tool_target_key(tool_name)?;
    Some(UiEvent::DelegationBlock {
        call_id: tool_call_id.to_string(),
        target_kind: target_kind.to_string(),
        target_id: args
            .get(target_id_key)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        task: args
            .get("task")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        status: "requested".to_string(),
        child_operation_id: None,
        summary: None,
        is_error: false,
    })
}

fn delegation_block_from_tool_result(
    tool_call_id: &str,
    tool_name: &str,
    summary: &str,
) -> Option<UiEvent> {
    let fallback_kind = delegation_tool_kind_label(tool_name)?;
    let value: serde_json::Value = serde_json::from_str(summary).ok()?;
    let status = value
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("requested");
    let message = value
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or(status);
    let is_error = status == "rejected" || status == "failed";
    let summary = match status {
        "requested" => Some("requested".to_string()),
        "rejected" => Some(format!("rejected: {message}")),
        "failed" => Some(format!("failed: {message}")),
        other => Some(other.to_string()),
    };
    Some(UiEvent::DelegationBlock {
        call_id: tool_call_id.to_string(),
        target_kind: value
            .get("target_kind")
            .and_then(|value| value.as_str())
            .unwrap_or(fallback_kind)
            .to_string(),
        target_id: value
            .get("target_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        task: value
            .get("task")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        status: status.to_string(),
        child_operation_id: None,
        summary,
        is_error,
    })
}

fn is_delegation_tool(name: &str) -> bool {
    delegation_tool_kind_label(name).is_some()
}

fn delegation_tool_kind_label(name: &str) -> Option<&'static str> {
    match name {
        "delegate_agent" => Some("agent"),
        "delegate_team" => Some("team"),
        _ => None,
    }
}

fn delegation_tool_target_key(name: &str) -> Option<&'static str> {
    match name {
        "delegate_agent" => Some("agent_id"),
        "delegate_team" => Some("team_id"),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{
        CapabilityStatus, CodingAgentCapabilities, CodingAgentEvent, CodingAgentSession,
        CodingAgentSessionOptions, CodingAgentSessionView, ProductEvent, ProductEventSequence,
        ProfileId, UiSnapshot, UiSnapshotCursor,
    };

    fn capabilities() -> CodingAgentCapabilities {
        CodingAgentCapabilities {
            prompt: CapabilityStatus::Available,
            abort: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            steer: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            follow_up: CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
            compact: CapabilityStatus::Available,
            fork: CapabilityStatus::Available,
            clone_session: CapabilityStatus::Available,
            branch_summary: CapabilityStatus::Available,
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: CapabilityStatus::Available,
            plugin_reload: CapabilityStatus::Available,
            self_healing_edit: CapabilityStatus::Available,
            agent_profiles: CapabilityStatus::Available,
            team_profiles: CapabilityStatus::Available,
            delegation: CapabilityStatus::Available,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }

    async fn snapshot(last_event_sequence: ProductEventSequence, session_id: &str) -> UiSnapshot {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let base = session.ui_snapshot(Vec::new());
        UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence,
                capability_generation: base.cursor.capability_generation,
            },
            base.version,
            CodingAgentSessionView {
                session_id: session_id.into(),
                default_agent_profile_id: ProfileId::from("default"),
            },
            capabilities(),
            None,
            Vec::new(),
        )
    }

    #[test]
    fn coding_event_bridge_accepts_product_events() {
        let product_event = ProductEvent::from_compat_event(
            ProductEventSequence(1),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello interactive".into(),
            },
        );
        let mut bridge = CodingEventBridge::new();

        let events = bridge.handle_product_event(&product_event);

        assert_eq!(
            events,
            vec![UiEvent::AssistantDelta {
                text: "hello interactive".into()
            }]
        );
    }

    fn assistant_delta_event(sequence: u64, text: &str) -> ProductEvent {
        ProductEvent::from_compat_event(
            ProductEventSequence::new(sequence),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: text.into(),
            },
        )
    }

    #[tokio::test]
    async fn ui_projection_hydrates_from_snapshot() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);

        assert_eq!(projection.last_sequence(), ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_ignores_equal_sequence_events() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        projection.apply_product_event(&assistant_delta_event(7, "duplicate"));

        assert_eq!(projection.last_sequence(), ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_ignores_stale_sequence_events() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        projection.apply_product_event(&assistant_delta_event(6, "stale"));

        assert_eq!(projection.last_sequence(), ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_applies_product_events_in_sequence_order() {
        let snapshot = snapshot(ProductEventSequence::new(2), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        let first = ProductEvent::from_compat_event(
            ProductEventSequence::new(3),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello ".into(),
            },
        );
        let second = ProductEvent::from_compat_event(
            ProductEventSequence::new(4),
            CodingAgentEvent::AssistantThinkingDelta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "thinking".into(),
            },
        );

        projection.apply_product_event(&first);
        projection.apply_product_event(&second);

        assert_eq!(projection.last_sequence(), ProductEventSequence::new(4));
        assert_eq!(
            projection.drain(),
            vec![
                UiEvent::AssistantDelta {
                    text: "hello ".into()
                },
                UiEvent::ThinkingDelta {
                    text: "thinking".into()
                }
            ]
        );
        assert!(projection.drain().is_empty());
    }
}
