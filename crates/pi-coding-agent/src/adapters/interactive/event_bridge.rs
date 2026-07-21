#[cfg(test)]
use crate::runtime::facade::CodingAgentProductEvent;
use std::collections::{HashMap, VecDeque};

use crate::runtime::facade::{
    CodingAgentAgentProductEvent, CodingAgentDelegationProductEvent, CodingAgentImageContent,
    CodingAgentMessageProductEvent, CodingAgentProductEventKind,
    CodingAgentProductEventProfileKind, CodingAgentProductEventUsage,
    CodingAgentProfileProductEvent, CodingAgentRuntimeProductEvent, CodingAgentSessionView,
    CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent, ProductEvent,
    ProductEventSequence, ProfileId, ProfileKind, UiContextProjection, UiSnapshot,
};

const MAX_CHILD_CONVERSATIONS: usize = 32;
const MAX_CHILD_UI_EVENTS: usize = 2_048;

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    TurnStarted,
    AssistantDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    AssistantDone,
    AssistantImages {
        images: Vec<CodingAgentImageContent>,
    },
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
    ToolAuthorizationRequired {
        request: crate::authorization::ToolAuthorizationRequest,
    },
    ToolAuthorizationResolved {
        authorization_id: String,
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
    DelegationConfirmationRequired {
        pending: crate::runtime::facade::PendingDelegationConfirmation,
    },
    DelegationConfirmationResolved {
        operation_id: String,
        tool_call_id: String,
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
    context: UiContextProjection,
    capabilities: Option<crate::runtime::facade::CodingAgentCapabilities>,
    session: Option<CodingAgentSessionView>,
    child_pending: HashMap<String, VecDeque<UiEvent>>,
    child_order: VecDeque<String>,
    child_summaries: HashMap<String, ChildDelegationSummary>,
}

#[derive(Debug, Clone)]
struct ChildDelegationSummary {
    call_id: String,
    target_kind: String,
    target_id: String,
    task: String,
    child_operation_id: String,
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
            context: UiContextProjection::default(),
            capabilities: None,
            session: None,
            child_pending: HashMap::new(),
            child_order: VecDeque::new(),
            child_summaries: HashMap::new(),
        }
    }

    pub(crate) fn from_snapshot(snapshot: UiSnapshot) -> Self {
        let recent_child_events = snapshot.recent_child_events;
        let mut projection = Self {
            bridge: CodingEventBridge::new(),
            last_sequence: snapshot.cursor.last_event_sequence,
            pending: Vec::new(),
            context: snapshot.context,
            capabilities: Some(snapshot.capabilities),
            session: Some(snapshot.session),
            child_pending: HashMap::new(),
            child_order: VecDeque::new(),
            child_summaries: HashMap::new(),
        };
        for event in recent_child_events {
            let ui_events = projection.bridge.push_product_event(&event);
            projection.remember_child_summaries(&ui_events);
            if let Some(child_operation_id) = projection.child_operation_id(&event) {
                projection.push_child_events(&child_operation_id, ui_events);
            }
        }
        let mut pending_authorizations = snapshot.pending_authorizations;
        pending_authorizations.sort_by(|left, right| {
            left.requested_at
                .cmp(&right.requested_at)
                .then_with(|| left.authorization_id.cmp(&right.authorization_id))
        });
        for request in pending_authorizations {
            let event = UiEvent::ToolAuthorizationRequired {
                request: request.clone(),
            };
            if let Some(conversation_id) =
                projection.conversation_operation_id(&request.operation_id)
            {
                projection.push_child_events(&conversation_id, vec![event]);
            } else {
                projection.pending.push(event);
            }
        }
        projection
    }

    pub(crate) fn apply_product_event(&mut self, event: &ProductEvent) {
        if event.sequence_internal() <= self.last_sequence {
            return;
        }
        self.last_sequence = event.sequence_internal();
        if let CodingAgentProductEventKind::Profile(
            CodingAgentProfileProductEvent::DefaultChanged { profile_id },
        ) = event.event()
            && let Some(session) = self.session.as_mut()
        {
            session.default_agent_profile_id = ProfileId::from(profile_id.clone());
        }
        self.context.apply_product_event(event, None);
        let ui_events = self.bridge.push_product_event(event);
        self.remember_child_summaries(&ui_events);
        if let Some(child_operation_id) = self.child_operation_id(event) {
            if ui_events
                .iter()
                .any(|event| matches!(event, UiEvent::ToolAuthorizationRequired { .. }))
                && let Some(summary) =
                    self.child_status_event(&child_operation_id, "waiting_permission")
            {
                self.pending.push(summary);
            }
            if ui_events
                .iter()
                .any(|event| matches!(event, UiEvent::ToolAuthorizationResolved { .. }))
                && let Some(summary) = self.child_status_event(&child_operation_id, "running")
            {
                self.pending.push(summary);
            }
            self.push_child_events(&child_operation_id, ui_events);
        } else {
            self.pending.extend(ui_events);
        }
    }

    pub(crate) fn drain(&mut self) -> Vec<UiEvent> {
        self.pending.drain(..).collect()
    }

    pub(crate) fn drain_children(&mut self) -> Vec<(String, Vec<UiEvent>)> {
        let operation_ids = self.child_order.iter().cloned().collect::<Vec<_>>();
        operation_ids
            .into_iter()
            .filter_map(|operation_id| {
                let events = self
                    .child_pending
                    .get_mut(&operation_id)?
                    .drain(..)
                    .collect::<Vec<_>>();
                (!events.is_empty()).then_some((operation_id, events))
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn drain_child(&mut self, operation_id: &str) -> Vec<UiEvent> {
        self.child_pending
            .get_mut(operation_id)
            .map(|events| events.drain(..).collect())
            .unwrap_or_default()
    }

    fn child_operation_id(&self, event: &ProductEvent) -> Option<String> {
        let operation_id = event.operation_id()?;
        if event.parent_operation_id().is_none() && !self.is_child_operation(operation_id) {
            return None;
        }
        self.conversation_operation_id(operation_id)
            .or_else(|| Some(operation_id.to_owned()))
    }

    fn conversation_operation_id(&self, operation_id: &str) -> Option<String> {
        let mut conversation_id = operation_id;
        while let Some(operation) = self
            .context
            .operations
            .iter()
            .find(|operation| operation.operation_id == conversation_id)
        {
            let Some(parent_id) = operation.parent_operation_id.as_deref() else {
                return (conversation_id != operation_id).then(|| conversation_id.to_owned());
            };
            let parent_is_root = self
                .context
                .operations
                .iter()
                .find(|operation| operation.operation_id == parent_id)
                .is_none_or(|parent| parent.parent_operation_id.is_none());
            if parent_is_root {
                return Some(conversation_id.to_owned());
            }
            conversation_id = parent_id;
        }
        self.is_child_operation(operation_id)
            .then(|| operation_id.to_owned())
    }

    fn is_child_operation(&self, operation_id: &str) -> bool {
        self.context.operations.iter().any(|operation| {
            operation.operation_id == operation_id && operation.parent_operation_id.is_some()
        })
    }

    fn push_child_events(&mut self, operation_id: &str, events: Vec<UiEvent>) {
        if events.is_empty() {
            return;
        }
        if !self.child_pending.contains_key(operation_id) {
            while self.child_order.len() >= MAX_CHILD_CONVERSATIONS {
                if let Some(evicted) = self.child_order.pop_front() {
                    self.child_pending.remove(&evicted);
                }
            }
            self.child_order.push_back(operation_id.to_owned());
        }
        let pending = self
            .child_pending
            .entry(operation_id.to_owned())
            .or_default();
        pending.extend(events);
        while pending.len() > MAX_CHILD_UI_EVENTS {
            pending.pop_front();
        }
    }

    fn remember_child_summaries(&mut self, events: &[UiEvent]) {
        for event in events {
            let UiEvent::DelegationBlock {
                call_id,
                target_kind,
                target_id,
                task,
                child_operation_id: Some(child_operation_id),
                ..
            } = event
            else {
                continue;
            };
            self.child_summaries.insert(
                child_operation_id.clone(),
                ChildDelegationSummary {
                    call_id: call_id.clone(),
                    target_kind: target_kind.clone(),
                    target_id: target_id.clone(),
                    task: task.clone(),
                    child_operation_id: child_operation_id.clone(),
                },
            );
        }
    }

    fn child_status_event(&self, operation_id: &str, status: &str) -> Option<UiEvent> {
        let summary = self.child_summaries.get(operation_id)?;
        Some(UiEvent::DelegationBlock {
            call_id: summary.call_id.clone(),
            target_kind: summary.target_kind.clone(),
            target_id: summary.target_id.clone(),
            task: summary.task.clone(),
            status: status.to_owned(),
            child_operation_id: Some(summary.child_operation_id.clone()),
            summary: Some(status.replace('_', " ")),
            is_error: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn last_sequence_for_tests(&self) -> ProductEventSequence {
        self.last_sequence
    }

    pub(crate) fn context(&self) -> &UiContextProjection {
        &self.context
    }

    pub(crate) fn capabilities(&self) -> Option<&crate::runtime::facade::CodingAgentCapabilities> {
        self.capabilities.as_ref()
    }

    pub(crate) fn session(&self) -> Option<&CodingAgentSessionView> {
        self.session.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn replace_context(&mut self, context: UiContextProjection) {
        self.context = context;
    }

    #[cfg(test)]
    pub(crate) fn replace_capabilities(
        &mut self,
        capabilities: Option<crate::runtime::facade::CodingAgentCapabilities>,
    ) {
        self.capabilities = capabilities;
    }
}

/// Stateless event bridge: converts typed product events to `Vec<UiEvent>`.
///
/// No longer accumulates tokens — `UiEvent::UsageUpdate` carries per-event
/// delta values. The receiver (`InteractiveRoot::apply_events`) accumulates
/// them into `FooterStats`.
#[derive(Debug, Clone)]
pub struct CodingEventBridge;

/// Estimate current context size from a usage snapshot.
/// Mirrors `pi-agent-core::compaction::estimate::calculate_context_tokens`
/// and the TS `getContextUsage` use of the latest assistant usage.
fn calculate_context_tokens(usage: &CodingAgentProductEventUsage) -> u32 {
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
        self.handle_typed(event.event())
    }

    #[cfg(test)]
    pub fn handle_product_event(&mut self, event: &CodingAgentProductEvent) -> Vec<UiEvent> {
        self.handle_typed(event.event())
    }

    fn handle_typed(&mut self, event: &CodingAgentProductEventKind) -> Vec<UiEvent> {
        match event {
            CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::TurnStarted {
                ..
            }) => {
                vec![UiEvent::TurnStarted]
            }
            CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Delta {
                text,
                ..
            }) => {
                vec![UiEvent::AssistantDelta { text: text.clone() }]
            }
            CodingAgentProductEventKind::Message(
                CodingAgentMessageProductEvent::ThinkingDelta { text, .. },
            ) => {
                vec![UiEvent::ThinkingDelta { text: text.clone() }]
            }
            CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
                images,
                usage,
                ..
            }) => {
                let context_tokens = match calculate_context_tokens(usage) {
                    0 => None,
                    tokens => Some(tokens),
                };
                let mut events = vec![UiEvent::AssistantDone];
                if !images.is_empty() {
                    events.push(UiEvent::AssistantImages {
                        images: images.clone(),
                    });
                }
                events.push(UiEvent::UsageUpdate {
                    input: usage.input,
                    output: usage.output,
                    cache_read: usage.cache_read,
                    cache_write: usage.cache_write,
                    cost: usage.input_cost
                        + usage.output_cost
                        + usage.cache_read_cost
                        + usage.cache_write_cost,
                    context_tokens,
                });
                events
            }
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Started {
                tool_call_id,
                name,
                arguments_json,
                ..
            }) => delegation_block_from_tool_start(tool_call_id, name, arguments_json).map_or_else(
                || {
                    vec![UiEvent::ToolStarted {
                        call_id: tool_call_id.clone(),
                        name: name.clone(),
                        args: parse_tool_arguments(arguments_json),
                    }]
                },
                |event| vec![event],
            ),
            CodingAgentProductEventKind::Tool(
                CodingAgentToolProductEvent::AuthorizationRequired { request },
            ) => vec![UiEvent::ToolAuthorizationRequired {
                request: request.clone(),
            }],
            CodingAgentProductEventKind::Tool(
                CodingAgentToolProductEvent::AuthorizationApproved {
                    authorization_id, ..
                }
                | CodingAgentToolProductEvent::AuthorizationDenied {
                    authorization_id, ..
                }
                | CodingAgentToolProductEvent::AuthorizationCancelled {
                    authorization_id, ..
                },
            ) => vec![UiEvent::ToolAuthorizationResolved {
                authorization_id: authorization_id.clone(),
            }],
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Updated {
                tool_call_id,
                message,
                ..
            }) => {
                vec![UiEvent::ToolUpdated {
                    call_id: tool_call_id.clone(),
                    result: message.clone(),
                }]
            }
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Completed {
                tool_call_id,
                name,
                summary,
                ..
            }) => delegation_block_from_tool_result(tool_call_id, name, summary).map_or_else(
                || {
                    vec![UiEvent::ToolFinished {
                        call_id: tool_call_id.clone(),
                        result: summary.clone(),
                        is_error: false,
                    }]
                },
                |event| vec![event],
            ),
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Failed {
                tool_call_id,
                name,
                message,
                ..
            }) => {
                if is_delegation_tool(name) {
                    delegation_block_from_tool_result(tool_call_id, name, message).map_or_else(
                        || {
                            vec![UiEvent::DelegationBlock {
                                call_id: tool_call_id.clone(),
                                target_kind: delegation_tool_kind_label(name)
                                    .unwrap_or("agent")
                                    .to_string(),
                                target_id: String::new(),
                                task: String::new(),
                                status: "failed".into(),
                                child_operation_id: None,
                                summary: Some(format!("failed: {message}")),
                                is_error: true,
                            }]
                        },
                        |event| vec![event],
                    )
                } else {
                    vec![UiEvent::ToolFinished {
                        call_id: tool_call_id.clone(),
                        result: message.clone(),
                        is_error: true,
                    }]
                }
            }
            CodingAgentProductEventKind::Runtime(
                CodingAgentRuntimeProductEvent::CompactionCompleted { summary, .. },
            )
            | CodingAgentProductEventKind::Session(
                crate::runtime::facade::CodingAgentSessionProductEvent::CompactionCompleted {
                    summary,
                    ..
                },
            ) => vec![
                UiEvent::CompactionNotice {
                    summary: summary.clone(),
                },
                UiEvent::UsageUpdate {
                    input: 0,
                    output: 0,
                    cache_read: 0,
                    cache_write: 0,
                    cost: 0.0,
                    context_tokens: None,
                },
            ],
            CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::ShutDown) => {
                Vec::new()
            }
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptFailed { error, .. },
            ) => vec![UiEvent::AgentError {
                error: error.message.clone(),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptAborted { reason, .. },
            ) => vec![UiEvent::AgentError {
                error: format!("prompt aborted: {reason}"),
            }],
            CodingAgentProductEventKind::Delegation(payload) => {
                let confirmation_required = match payload {
                    CodingAgentDelegationProductEvent::ConfirmationRequired { context, reason } => {
                        Some(UiEvent::DelegationConfirmationRequired {
                            pending: crate::runtime::facade::PendingDelegationConfirmation {
                                operation_id: context.operation_id.clone(),
                                turn_id: context.turn_id.clone(),
                                tool_call_id: context.tool_call_id.clone(),
                                requesting_profile_id: ProfileId::from(
                                    context.requesting_profile_id.as_str(),
                                ),
                                target_kind: match context.target_kind {
                                    CodingAgentProductEventProfileKind::Agent => ProfileKind::Agent,
                                    CodingAgentProductEventProfileKind::Team => ProfileKind::Team,
                                },
                                target_id: ProfileId::from(context.target_id.as_str()),
                                task: context.task.clone(),
                                reason: reason.clone(),
                            },
                        })
                    }
                    _ => None,
                };
                let confirmation_resolved = match payload {
                    CodingAgentDelegationProductEvent::Approved { context }
                    | CodingAgentDelegationProductEvent::Rejected { context, .. } => {
                        Some(UiEvent::DelegationConfirmationResolved {
                            operation_id: context.operation_id.clone(),
                            tool_call_id: context.tool_call_id.clone(),
                        })
                    }
                    _ => None,
                };
                let (ctx, status, summary, child, is_error) = match payload {
                    CodingAgentDelegationProductEvent::Requested { context } => {
                        (context, "requested", Some("requested".into()), None, false)
                    }
                    CodingAgentDelegationProductEvent::ConfirmationRequired { context, reason } => {
                        (
                            context,
                            "confirmation_required",
                            Some(format!("confirmation required: {reason}")),
                            None,
                            false,
                        )
                    }
                    CodingAgentDelegationProductEvent::Approved { context } => {
                        (context, "approved", Some("approved".into()), None, false)
                    }
                    CodingAgentDelegationProductEvent::Rejected { context, reason } => (
                        context,
                        "rejected",
                        Some(format!("rejected: {reason}")),
                        None,
                        true,
                    ),
                    CodingAgentDelegationProductEvent::Started {
                        context,
                        child_operation_id,
                    } => (context, "running", None, Some(child_operation_id), false),
                    CodingAgentDelegationProductEvent::Completed {
                        context,
                        child_operation_id,
                        final_text,
                    } => (
                        context,
                        "completed",
                        Some(format!("completed: {final_text}")),
                        Some(child_operation_id),
                        false,
                    ),
                    CodingAgentDelegationProductEvent::Failed {
                        context,
                        child_operation_id,
                        error,
                    } => (
                        context,
                        "failed",
                        Some(format!("failed: {}", error.message)),
                        Some(child_operation_id),
                        true,
                    ),
                };
                let mut events = vec![UiEvent::DelegationBlock {
                    call_id: ctx.tool_call_id.clone(),
                    target_kind: profile_kind_label(match ctx.target_kind {
                        CodingAgentProductEventProfileKind::Agent => ProfileKind::Agent,
                        CodingAgentProductEventProfileKind::Team => ProfileKind::Team,
                    })
                    .into(),
                    target_id: ctx.target_id.clone(),
                    task: ctx.task.clone(),
                    status: status.into(),
                    child_operation_id: child.cloned(),
                    summary,
                    is_error,
                }];
                events.extend(confirmation_required);
                events.extend(confirmation_resolved);
                events
            }
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                    path, replacements, ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit started for {} ({}).",
                    path,
                    replacement_count_label(*replacements)
                ),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::OperationRecoveryResolved {
                    operation_id,
                    resolution,
                    reason,
                    ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!(
                    "Operation {operation_id} recovery resolved as {resolution:?}: {reason}"
                ),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    path,
                    attempt,
                    replacements,
                    check_output,
                    ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit repair attempt {} for {}: {}, {}.",
                    attempt,
                    path,
                    replacement_count_label(replacements.len()),
                    check_output
                        .as_ref()
                        .map(|o| format!("check exit {}", o.exit_code))
                        .unwrap_or_else(|| "no check output".into())
                ),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    path,
                    attempts,
                    first_changed_line,
                    ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!(
                    "Self-healing edit completed for {} after {}{}.",
                    path,
                    attempt_count_label(*attempts),
                    first_changed_line_label(*first_changed_line)
                ),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditFailed { path, error, .. },
            ) => vec![UiEvent::SystemNotice {
                text: format!("Self-healing edit failed for {}: {}", path, error.message),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditAborted { path, reason, .. },
            ) => vec![UiEvent::SystemNotice {
                text: format!("Self-healing edit cancelled for {path}: {reason}"),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::OperationRecoveryPending {
                    operation_id,
                    recovery_id,
                    reason,
                    ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!(
                    "Operation {operation_id} requires recovery ({recovery_id}): {reason}"
                ),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::OperationRecovered {
                    operation_id,
                    reason,
                    ..
                },
            ) => vec![UiEvent::SystemNotice {
                text: format!("Recovered incomplete operation {operation_id}: {reason}"),
            }],
            _ => Vec::new(),
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
        .or_else(|| value.get("error"))
        .and_then(|value| value.as_str())
        .unwrap_or(status);
    let is_error = matches!(status, "rejected" | "failed" | "cancelled");
    let summary = match status {
        "requested" => Some("requested".to_string()),
        "rejected" => Some(format!("rejected: {message}")),
        "failed" => Some(format!("failed: {message}")),
        "cancelled" => Some(format!("cancelled: {message}")),
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
    use crate::authorization::{
        ToolAuthorizationPreview, ToolAuthorizationRequest, ToolAuthorizationRisk,
        ToolAuthorizationScope,
    };
    use crate::events::message::MessageEvent;
    use crate::events::prompt_stream::PromptStreamEvent;
    use crate::events::runtime::RuntimeEvent;
    use crate::runtime::client::context::{
        UiContextProjection, UiOperationProjection, UiOperationStatus,
    };
    use crate::runtime::facade::{
        CapabilityStatus, CodingAgentCapabilities, CodingAgentSession, CodingAgentSessionOptions,
        CodingAgentSessionView, ProductEvent, ProductEventSequence, ProfileId, UiSnapshot,
        UiSnapshotCursor,
    };

    fn stream_event(sequence: u64, event: PromptStreamEvent) -> ProductEvent {
        ProductEvent::from_draft_for_tests(
            ProductEventSequence::new(sequence),
            event.into_product_draft(),
            None,
        )
    }

    fn profile_event(sequence: u64, profile_id: &str) -> ProductEvent {
        ProductEvent::from_draft_for_tests(
            ProductEventSequence::new(sequence),
            crate::events::profile::ProfileEvent::DefaultChanged {
                profile_id: ProfileId::from(profile_id),
            }
            .into_product_draft(),
            None,
        )
    }

    fn authorization_request(id: &str, requested_at: &str) -> ToolAuthorizationRequest {
        ToolAuthorizationRequest {
            authorization_id: id.into(),
            operation_id: "op-auth".into(),
            turn_id: "turn-auth".into(),
            tool_call_id: format!("call-{id}"),
            tool_name: "write".into(),
            risk: ToolAuthorizationRisk::FilesystemMutation,
            scope: ToolAuthorizationScope::Path {
                path: "/workspace/file.txt".into(),
            },
            preview: ToolAuthorizationPreview {
                summary: "Modify a file".into(),
                path: Some("/workspace/file.txt".into()),
                command: None,
                cwd: None,
                content_preview: Some("new content".into()),
            },
            capability_generation: 1,
            requested_at: requested_at.into(),
        }
    }

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
                stream_id: base.cursor.stream_id.clone(),
                last_event_sequence,
                last_session_sequence: base.cursor.last_session_sequence,
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
            Vec::new(),
        )
    }

    #[test]
    fn coding_event_bridge_accepts_product_events() {
        let product_event = stream_event(
            1,
            PromptStreamEvent::Message(MessageEvent::Delta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello interactive".into(),
            }),
        );
        let mut bridge = CodingEventBridge::new();

        let events = bridge.push_product_event(&product_event);

        assert_eq!(
            events,
            vec![UiEvent::AssistantDelta {
                text: "hello interactive".into()
            }]
        );
    }

    #[test]
    fn ui_projection_routes_child_events_away_from_root_pending() {
        let child_event = assistant_delta_event(1, "child output")
            .with_lineage_for_tests("op-parent", "op-parent");
        let mut projection = UiProjection::new();

        projection.apply_product_event(&child_event);

        assert!(projection.drain().is_empty());
        assert_eq!(
            projection.drain_child("op_interactive"),
            vec![UiEvent::AssistantDelta {
                text: "child output".into()
            }]
        );
    }

    #[tokio::test]
    async fn ui_projection_folds_nested_child_events_into_delegation_conversation() {
        let mut snapshot = snapshot(ProductEventSequence::new(0), "sess_projection").await;
        snapshot.context.operations = vec![
            UiOperationProjection {
                operation_id: "op-root".into(),
                kind: "prompt".into(),
                parent_operation_id: None,
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 1,
                updated_sequence: 1,
                diagnostics: Vec::new(),
                failure: None,
            },
            UiOperationProjection {
                operation_id: "op-delegation".into(),
                kind: "agent_invocation".into(),
                parent_operation_id: Some("op-root".into()),
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 2,
                updated_sequence: 2,
                diagnostics: Vec::new(),
                failure: None,
            },
            UiOperationProjection {
                operation_id: "op_interactive".into(),
                kind: "prompt".into(),
                parent_operation_id: Some("op-delegation".into()),
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 3,
                updated_sequence: 3,
                diagnostics: Vec::new(),
                failure: None,
            },
        ];
        let mut projection = UiProjection::from_snapshot(snapshot);
        let event = assistant_delta_event(1, "nested child output")
            .with_lineage_for_tests("op-delegation", "op-root");

        projection.apply_product_event(&event);

        assert!(projection.drain().is_empty());
        assert!(projection.drain_child("op_interactive").is_empty());
        assert_eq!(
            projection.drain_child("op-delegation"),
            vec![UiEvent::AssistantDelta {
                text: "nested child output".into()
            }]
        );
    }

    #[tokio::test]
    async fn child_authorization_projects_status_only_to_root_conversation() {
        let mut snapshot = snapshot(ProductEventSequence::new(0), "sess_projection").await;
        snapshot.context.operations = vec![
            UiOperationProjection {
                operation_id: "op-root".into(),
                kind: "prompt".into(),
                parent_operation_id: None,
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 1,
                updated_sequence: 1,
                diagnostics: Vec::new(),
                failure: None,
            },
            UiOperationProjection {
                operation_id: "op-delegation".into(),
                kind: "agent_invocation".into(),
                parent_operation_id: Some("op-root".into()),
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 2,
                updated_sequence: 2,
                diagnostics: Vec::new(),
                failure: None,
            },
            UiOperationProjection {
                operation_id: "op-auth".into(),
                kind: "prompt".into(),
                parent_operation_id: Some("op-delegation".into()),
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Running,
                started_sequence: 3,
                updated_sequence: 3,
                diagnostics: Vec::new(),
                failure: None,
            },
        ];
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.remember_child_summaries(&[UiEvent::DelegationBlock {
            call_id: "delegate-1".into(),
            target_kind: "agent".into(),
            target_id: "coder".into(),
            task: "implement parser".into(),
            status: "running".into(),
            child_operation_id: Some("op-delegation".into()),
            summary: None,
            is_error: false,
        }]);
        let request = authorization_request("auth-child", "2026-07-17T00:00:01Z");
        let event = stream_event(
            1,
            PromptStreamEvent::Tool(crate::events::tool::ToolEvent::AuthorizationRequired {
                request: request.clone(),
            }),
        )
        .with_lineage_for_tests("op-delegation", "op-root");

        projection.apply_product_event(&event);

        assert_eq!(
            projection.drain(),
            vec![UiEvent::DelegationBlock {
                call_id: "delegate-1".into(),
                target_kind: "agent".into(),
                target_id: "coder".into(),
                task: "implement parser".into(),
                status: "waiting_permission".into(),
                child_operation_id: Some("op-delegation".into()),
                summary: Some("waiting permission".into()),
                is_error: false,
            }]
        );
        assert_eq!(
            projection.drain_child("op-delegation"),
            vec![UiEvent::ToolAuthorizationRequired { request }]
        );
    }

    #[test]
    fn runtime_shutdown_has_no_interactive_projection() {
        let product_event = stream_event(1, PromptStreamEvent::Runtime(RuntimeEvent::ShutDown));
        let mut bridge = CodingEventBridge::new();

        assert!(bridge.push_product_event(&product_event).is_empty());
    }

    fn assistant_delta_event(sequence: u64, text: &str) -> ProductEvent {
        stream_event(
            sequence,
            PromptStreamEvent::Message(MessageEvent::Delta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: text.into(),
            }),
        )
    }

    #[tokio::test]
    async fn ui_projection_hydrates_from_snapshot() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);

        assert_eq!(projection.last_sequence, ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_hydrates_retained_child_conversation_events() {
        let mut snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        snapshot.context.operations = vec![
            UiOperationProjection {
                operation_id: "op-root".into(),
                kind: "prompt".into(),
                parent_operation_id: None,
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Completed,
                started_sequence: 1,
                updated_sequence: 7,
                diagnostics: Vec::new(),
                failure: None,
            },
            UiOperationProjection {
                operation_id: "op_interactive".into(),
                kind: "agent_invocation".into(),
                parent_operation_id: Some("op-root".into()),
                root_operation_id: Some("op-root".into()),
                status: UiOperationStatus::Completed,
                started_sequence: 2,
                updated_sequence: 6,
                diagnostics: Vec::new(),
                failure: None,
            },
        ];
        snapshot.recent_child_events = vec![
            assistant_delta_event(5, "retained child output")
                .with_lineage_for_tests("op-root", "op-root"),
        ];

        let mut projection = UiProjection::from_snapshot(snapshot);

        assert!(projection.drain().is_empty());
        assert_eq!(
            projection.drain_child("op_interactive"),
            vec![UiEvent::AssistantDelta {
                text: "retained child output".into()
            }]
        );
    }

    #[tokio::test]
    async fn ui_projection_preserves_context_snapshot_across_live_events() {
        let mut snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut context = UiContextProjection::default();
        context.operations = vec![UiOperationProjection {
            operation_id: "op_interactive".into(),
            kind: "prompt".into(),
            parent_operation_id: None,
            root_operation_id: Some("op_interactive".into()),
            status: UiOperationStatus::Running,
            started_sequence: 1,
            updated_sequence: 7,
            diagnostics: Vec::new(),
            failure: None,
        }];
        snapshot.context = context;
        let mut projection = UiProjection::from_snapshot(snapshot);

        projection.apply_product_event(&stream_event(
            8,
            PromptStreamEvent::Tool(crate::events::tool::ToolEvent::Updated {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool-1".into(),
                name: "bash".into(),
                message: "running".into(),
            }),
        ));

        assert_eq!(projection.context().operations.len(), 1);
        assert_eq!(projection.context().operations[0].kind, "prompt");
        assert_eq!(projection.context().operations[0].updated_sequence, 8);
    }

    #[tokio::test]
    async fn ui_projection_owns_snapshot_session_and_orders_profile_changes() {
        let mut projection = UiProjection::from_snapshot(
            snapshot(ProductEventSequence::new(7), "sess_profile").await,
        );

        projection.apply_product_event(&profile_event(8, "reviewer"));
        projection.apply_product_event(&profile_event(6, "stale"));

        let session = projection.session().expect("snapshot session projection");
        assert_eq!(session.session_id, "sess_profile");
        assert_eq!(session.default_agent_profile_id.as_str(), "reviewer");
    }

    #[tokio::test]
    async fn ui_projection_reconstructs_pending_authorizations_in_request_order() {
        let mut snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        snapshot.pending_authorizations = vec![
            authorization_request("auth-later", "2026-07-17T00:00:02Z"),
            authorization_request("auth-first", "2026-07-17T00:00:01Z"),
        ];
        let mut projection = UiProjection::from_snapshot(snapshot);

        let events = projection.drain();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            UiEvent::ToolAuthorizationRequired { request }
                if request.authorization_id == "auth-first"
        ));
        assert!(matches!(
            &events[1],
            UiEvent::ToolAuthorizationRequired { request }
                if request.authorization_id == "auth-later"
        ));
    }

    #[test]
    fn authorization_product_events_open_and_resolve_the_ui_surface() {
        let request = authorization_request("auth-1", "2026-07-17T00:00:01Z");
        let mut bridge = CodingEventBridge::new();
        let required =
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::AuthorizationRequired {
                request: request.clone(),
            });
        assert_eq!(
            bridge.handle_typed(&required),
            vec![UiEvent::ToolAuthorizationRequired { request }]
        );

        let resolved =
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::AuthorizationDenied {
                authorization_id: "auth-1".into(),
                operation_id: "op-auth".into(),
                tool_call_id: "call-auth-1".into(),
                reason: "denied".into(),
            });
        assert_eq!(
            bridge.handle_typed(&resolved),
            vec![UiEvent::ToolAuthorizationResolved {
                authorization_id: "auth-1".into()
            }]
        );
    }

    #[tokio::test]
    async fn ui_projection_ignores_equal_sequence_events() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        projection.apply_product_event(&assistant_delta_event(7, "duplicate"));

        assert_eq!(projection.last_sequence, ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_ignores_stale_sequence_events() {
        let snapshot = snapshot(ProductEventSequence::new(7), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        projection.apply_product_event(&assistant_delta_event(6, "stale"));

        assert_eq!(projection.last_sequence, ProductEventSequence::new(7));
        assert!(projection.drain().is_empty());
    }

    #[tokio::test]
    async fn ui_projection_applies_product_events_in_sequence_order() {
        let snapshot = snapshot(ProductEventSequence::new(2), "sess_projection").await;
        let mut projection = UiProjection::from_snapshot(snapshot);
        projection.drain();

        let first = stream_event(
            3,
            PromptStreamEvent::Message(MessageEvent::Delta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello ".into(),
            }),
        );
        let second = stream_event(
            4,
            PromptStreamEvent::Message(MessageEvent::ThinkingDelta {
                operation_id: "op_interactive".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "thinking".into(),
            }),
        );

        projection.apply_product_event(&first);
        projection.apply_product_event(&second);

        assert_eq!(projection.last_sequence, ProductEventSequence::new(4));
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
