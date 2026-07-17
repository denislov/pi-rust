use std::cmp::Reverse;
use std::collections::HashMap;

use serde_json::Value;

use crate::events::{
    CodingAgentAgentProductEvent, CodingAgentDelegationProductEvent,
    CodingAgentDiagnosticProductEvent, CodingAgentMessageProductEvent, CodingAgentProductEvent,
    CodingAgentProductEventKind, CodingAgentProductEventProfileKind,
    CodingAgentProductEventTerminalOperationKind, CodingAgentProductEventTerminalStatus,
    CodingAgentTeamProductEvent, CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
};
use crate::runtime::control::OperationKind;

const MAX_OPERATIONS: usize = 32;
const MAX_CHANGES: usize = 64;
const MAX_DELEGATIONS: usize = 32;
const MAX_OPERATION_DIAGNOSTICS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiOperationStatus {
    Running,
    Completed,
    Failed,
    Aborted,
    Recovered,
}

impl UiOperationStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
            Self::Recovered => "recovered",
        }
    }

    pub(crate) const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiOperationProjection {
    pub(crate) operation_id: String,
    pub(crate) kind: String,
    pub(crate) parent_operation_id: Option<String>,
    pub(crate) root_operation_id: Option<String>,
    pub(crate) status: UiOperationStatus,
    pub(crate) started_sequence: u64,
    pub(crate) updated_sequence: u64,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiFileChangeProjection {
    pub(crate) path: String,
    pub(crate) mutation_kind: String,
    pub(crate) operation_id: String,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) updated_sequence: u64,
    pub(crate) first_changed_line: Option<usize>,
    pub(crate) added_lines: Option<usize>,
    pub(crate) removed_lines: Option<usize>,
    pub(crate) diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiDelegationProjection {
    pub(crate) tool_call_id: String,
    pub(crate) child_operation_id: Option<String>,
    pub(crate) target_kind: String,
    pub(crate) target_id: String,
    pub(crate) task: String,
    pub(crate) status: String,
    pub(crate) updated_sequence: u64,
    pub(crate) summary: Option<String>,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UiTurnUsageProjection {
    pub(crate) turn_id: String,
    pub(crate) input: u32,
    pub(crate) output: u32,
    pub(crate) cache_read: u32,
    pub(crate) cache_write: u32,
    pub(crate) context_tokens: Option<u32>,
    pub(crate) cost: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UiUsageProjection {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
    pub(crate) cost: Option<f64>,
    pub(crate) latest_turn: Option<UiTurnUsageProjection>,
    pub(crate) model_id: Option<String>,
    pub(crate) context_window: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMutation {
    operation_id: String,
    tool_call_id: String,
    path: String,
    mutation_kind: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UiContextProjection {
    pub(crate) operations: Vec<UiOperationProjection>,
    pub(crate) changes: Vec<UiFileChangeProjection>,
    pub(crate) delegations: Vec<UiDelegationProjection>,
    pub(crate) usage: UiUsageProjection,
    pending_mutations: HashMap<String, PendingMutation>,
}

impl UiContextProjection {
    pub(crate) fn apply_product_event(
        &mut self,
        event: &CodingAgentProductEvent,
        operation_kind: Option<OperationKind>,
    ) {
        self.apply_operation(event, operation_kind);
        self.apply_change(event);
        self.apply_delegation(event);
        self.apply_usage(event);
    }

    fn apply_operation(
        &mut self,
        event: &CodingAgentProductEvent,
        operation_kind: Option<OperationKind>,
    ) {
        let Some(operation_id) = event.operation_id() else {
            return;
        };
        let sequence = event.sequence();
        let inferred_kind = operation_kind
            .map(|kind| kind.as_str())
            .or_else(|| terminal_operation_kind(event))
            .unwrap_or_else(|| inferred_event_operation_kind(event.event()));
        let index = self
            .operations
            .iter()
            .position(|operation| operation.operation_id == operation_id);
        let index = match index {
            Some(index) => index,
            None => {
                self.operations.push(UiOperationProjection {
                    operation_id: operation_id.to_owned(),
                    kind: inferred_kind.to_owned(),
                    parent_operation_id: event.parent_operation_id().map(ToOwned::to_owned),
                    root_operation_id: event.root_operation_id().map(ToOwned::to_owned),
                    status: UiOperationStatus::Running,
                    started_sequence: sequence,
                    updated_sequence: sequence,
                    diagnostics: Vec::new(),
                    failure: None,
                });
                self.operations.len() - 1
            }
        };
        let operation = &mut self.operations[index];
        if operation_kind.is_some() || event.terminal_operation().is_some() {
            operation.kind = inferred_kind.to_owned();
        }
        operation.updated_sequence = sequence;
        if operation.parent_operation_id.is_none() {
            operation.parent_operation_id = event.parent_operation_id().map(ToOwned::to_owned);
        }
        if operation.root_operation_id.is_none() {
            operation.root_operation_id = event.root_operation_id().map(ToOwned::to_owned);
        }
        if let Some(terminal) = event.terminal_operation() {
            operation.status = operation_status(terminal.status);
        }
        if let Some(failure) = event_failure(event.event()) {
            operation.failure = Some(failure);
        }
        if let CodingAgentProductEventKind::Diagnostic(
            CodingAgentDiagnosticProductEvent::Diagnostic { message, .. },
        ) = event.event()
        {
            operation.diagnostics.push(message.clone());
            if operation.diagnostics.len() > MAX_OPERATION_DIAGNOSTICS {
                operation.diagnostics.remove(0);
            }
        }

        self.operations.sort_by_key(|operation| {
            (
                !operation.status.is_running(),
                Reverse(operation.updated_sequence),
            )
        });
        self.operations.truncate(MAX_OPERATIONS);
    }

    fn apply_change(&mut self, event: &CodingAgentProductEvent) {
        match event.event() {
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Started {
                operation_id,
                tool_call_id,
                name,
                arguments_json,
                ..
            }) if matches!(name.as_str(), "edit" | "write") => {
                if let Some(path) = mutation_path(arguments_json) {
                    self.pending_mutations.insert(
                        tool_call_id.clone(),
                        PendingMutation {
                            operation_id: operation_id.clone(),
                            tool_call_id: tool_call_id.clone(),
                            path,
                            mutation_kind: name.clone(),
                        },
                    );
                }
            }
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Completed {
                tool_call_id,
                name,
                ..
            }) if matches!(name.as_str(), "edit" | "write") => {
                if let Some(pending) = self.pending_mutations.remove(tool_call_id) {
                    self.upsert_change(UiFileChangeProjection {
                        path: pending.path,
                        mutation_kind: pending.mutation_kind,
                        operation_id: pending.operation_id,
                        tool_call_id: Some(pending.tool_call_id),
                        updated_sequence: event.sequence(),
                        first_changed_line: None,
                        added_lines: None,
                        removed_lines: None,
                        diff: None,
                    });
                }
            }
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Failed {
                tool_call_id,
                name,
                ..
            }) if matches!(name.as_str(), "edit" | "write") => {
                self.pending_mutations.remove(tool_call_id);
            }
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    operation_id,
                    path,
                    first_changed_line,
                    ..
                },
            ) => self.upsert_change(UiFileChangeProjection {
                path: path.clone(),
                mutation_kind: "self_healing_edit".into(),
                operation_id: operation_id.clone(),
                tool_call_id: None,
                updated_sequence: event.sequence(),
                first_changed_line: *first_changed_line,
                added_lines: None,
                removed_lines: None,
                diff: None,
            }),
            _ => {}
        }
    }

    fn upsert_change(&mut self, change: UiFileChangeProjection) {
        self.changes.retain(|current| current.path != change.path);
        self.changes.insert(0, change);
        self.changes.truncate(MAX_CHANGES);
    }

    fn apply_delegation(&mut self, event: &CodingAgentProductEvent) {
        let CodingAgentProductEventKind::Delegation(delegation) = event.event() else {
            return;
        };
        let (context, child_operation_id, status, summary, failure) = match delegation {
            CodingAgentDelegationProductEvent::Requested { context } => {
                (context, None, "requested", None, None)
            }
            CodingAgentDelegationProductEvent::Rejected { context, reason } => {
                (context, None, "rejected", None, Some(reason.clone()))
            }
            CodingAgentDelegationProductEvent::Approved { context } => {
                (context, None, "approved", None, None)
            }
            CodingAgentDelegationProductEvent::ConfirmationRequired { context, reason } => (
                context,
                None,
                "confirmation_required",
                None,
                Some(reason.clone()),
            ),
            CodingAgentDelegationProductEvent::Started {
                context,
                child_operation_id,
            } => (
                context,
                Some(child_operation_id.clone()),
                "running",
                None,
                None,
            ),
            CodingAgentDelegationProductEvent::Completed {
                context,
                child_operation_id,
                final_text,
            } => (
                context,
                Some(child_operation_id.clone()),
                "completed",
                Some(final_text.clone()),
                None,
            ),
            CodingAgentDelegationProductEvent::Failed {
                context,
                child_operation_id,
                error,
            } => (
                context,
                Some(child_operation_id.clone()),
                "failed",
                None,
                Some(error.message.clone()),
            ),
        };
        let target_kind = match context.target_kind {
            CodingAgentProductEventProfileKind::Agent => "agent",
            CodingAgentProductEventProfileKind::Team => "team",
        };
        let next = UiDelegationProjection {
            tool_call_id: context.tool_call_id.clone(),
            child_operation_id,
            target_kind: target_kind.into(),
            target_id: context.target_id.clone(),
            task: context.task.clone(),
            status: status.into(),
            updated_sequence: event.sequence(),
            summary,
            failure,
        };
        self.delegations
            .retain(|current| current.tool_call_id != next.tool_call_id);
        self.delegations.insert(0, next);
        self.delegations.truncate(MAX_DELEGATIONS);
    }

    fn apply_usage(&mut self, event: &CodingAgentProductEvent) {
        if let CodingAgentProductEventKind::Agent(
            CodingAgentAgentProductEvent::ProviderRequestStarted {
                model,
                context_window,
                ..
            },
        ) = event.event()
        {
            self.usage.model_id = Some(model.clone());
            self.usage.context_window = *context_window;
            return;
        }
        let CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
            turn_id,
            usage,
            ..
        }) = event.event()
        else {
            return;
        };
        let cost =
            usage.input_cost + usage.output_cost + usage.cache_read_cost + usage.cache_write_cost;
        self.usage.input = self.usage.input.saturating_add(usage.input as u64);
        self.usage.output = self.usage.output.saturating_add(usage.output as u64);
        self.usage.cache_read = self
            .usage
            .cache_read
            .saturating_add(usage.cache_read as u64);
        self.usage.cache_write = self
            .usage
            .cache_write
            .saturating_add(usage.cache_write as u64);
        let priced = (usage.cost_known && cost > 0.0).then_some(cost);
        if let Some(cost) = priced {
            self.usage.cost = Some(self.usage.cost.unwrap_or(0.0) + cost);
        }
        let component_total = usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write);
        self.usage.latest_turn = Some(UiTurnUsageProjection {
            turn_id: turn_id.clone(),
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            context_tokens: Some(if usage.total_tokens > 0 {
                usage.total_tokens
            } else {
                component_total
            }),
            cost: priced,
        });
    }
}

fn mutation_path(arguments_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(arguments_json).ok()?;
    ["path", "file_path", "filePath"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .filter(|path| !path.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn operation_status(status: CodingAgentProductEventTerminalStatus) -> UiOperationStatus {
    match status {
        CodingAgentProductEventTerminalStatus::Completed => UiOperationStatus::Completed,
        CodingAgentProductEventTerminalStatus::Failed => UiOperationStatus::Failed,
        CodingAgentProductEventTerminalStatus::Aborted => UiOperationStatus::Aborted,
        CodingAgentProductEventTerminalStatus::Recovered => UiOperationStatus::Recovered,
    }
}

fn terminal_operation_kind(event: &CodingAgentProductEvent) -> Option<&'static str> {
    Some(match event.terminal_operation()?.kind {
        CodingAgentProductEventTerminalOperationKind::Prompt => "prompt",
        CodingAgentProductEventTerminalOperationKind::BranchSummary => "branch_summary",
        CodingAgentProductEventTerminalOperationKind::AgentInvocation => "agent_invocation",
        CodingAgentProductEventTerminalOperationKind::AgentTeam => "agent_team",
        CodingAgentProductEventTerminalOperationKind::SelfHealingEdit => "self_healing_edit",
        CodingAgentProductEventTerminalOperationKind::Compact => "compact",
        CodingAgentProductEventTerminalOperationKind::PluginLoad => "plugin_load",
        CodingAgentProductEventTerminalOperationKind::Export => "export",
    })
}

fn inferred_event_operation_kind(event: &CodingAgentProductEventKind) -> &'static str {
    match event {
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::InvocationStarted {
            ..
        })
        | CodingAgentProductEventKind::Agent(
            CodingAgentAgentProductEvent::InvocationCompleted { .. }
            | CodingAgentAgentProductEvent::InvocationFailed { .. }
            | CodingAgentAgentProductEvent::InvocationAborted { .. },
        ) => "agent_invocation",
        CodingAgentProductEventKind::Agent(
            CodingAgentAgentProductEvent::TurnStarted { .. }
            | CodingAgentAgentProductEvent::ProviderRequestStarted { .. },
        ) => "prompt",
        CodingAgentProductEventKind::Team(_) => "agent_team",
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditStarted { .. }
            | CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted { .. }
            | CodingAgentWorkflowProductEvent::SelfHealingEditCompleted { .. }
            | CodingAgentWorkflowProductEvent::SelfHealingEditFailed { .. }
            | CodingAgentWorkflowProductEvent::SelfHealingEditAborted { .. },
        ) => "self_healing_edit",
        CodingAgentProductEventKind::Workflow(_) | CodingAgentProductEventKind::Message(_) => {
            "prompt"
        }
        CodingAgentProductEventKind::Delegation(_) => "delegation",
        CodingAgentProductEventKind::Tool(_) => "tool",
        _ => event.family().as_str(),
    }
}

fn event_failure(event: &CodingAgentProductEventKind) -> Option<String> {
    match event {
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::InvocationFailed {
            error,
            ..
        }) => Some(error.message.clone()),
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::InvocationAborted {
            reason,
            ..
        }) => Some(reason.clone()),
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Failed {
            error, ..
        }) => Some(error.message.clone()),
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Aborted {
            reason, ..
        }) => Some(reason.clone()),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditFailed { error, .. }
            | CodingAgentWorkflowProductEvent::PromptFailed { error, .. },
        ) => Some(error.message.clone()),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditAborted { reason, .. }
            | CodingAgentWorkflowProductEvent::PromptAborted { reason, .. },
        ) => Some(reason.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::events::emission::ProductEventDraft;
    use crate::events::{
        CodingAgentDelegationEventContext, CodingAgentProductEvent,
        CodingAgentProductEventDurability, CodingAgentProductEventTerminalOperation,
        CodingAgentProductEventUsage, ProductEventSequence,
    };

    use super::*;

    fn event(
        sequence: u64,
        operation_id: &str,
        event: CodingAgentProductEventKind,
        terminal: Option<CodingAgentProductEventTerminalOperation>,
    ) -> CodingAgentProductEvent {
        CodingAgentProductEvent::from_draft_for_tests(
            ProductEventSequence::new(sequence),
            ProductEventDraft {
                event,
                operation_id: Some(operation_id.into()),
                session_id: Some("session-1".into()),
                terminal_status: terminal.map(|terminal| terminal.status),
                durability: CodingAgentProductEventDurability::LiveOnly,
            },
            terminal,
        )
    }

    fn terminal_prompt(
        status: CodingAgentProductEventTerminalStatus,
    ) -> CodingAgentProductEventTerminalOperation {
        CodingAgentProductEventTerminalOperation {
            kind: CodingAgentProductEventTerminalOperationKind::Prompt,
            status,
        }
    }

    #[test]
    fn folds_typed_context_facts_without_reclassifying_the_root_operation() {
        let mut projection = UiContextProjection::default();
        projection.apply_product_event(
            &event(
                1,
                "op-1",
                CodingAgentProductEventKind::Workflow(
                    CodingAgentWorkflowProductEvent::PromptStarted {
                        operation_id: "op-1".into(),
                        turn_id: "turn-1".into(),
                    },
                ),
                None,
            ),
            Some(OperationKind::Prompt),
        );
        projection.apply_product_event(
            &event(
                2,
                "op-1",
                CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Started {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    tool_call_id: "tool-1".into(),
                    name: "edit".into(),
                    arguments_json: r#"{"path":"src/lib.rs","oldText":"a","newText":"b"}"#.into(),
                }),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                3,
                "op-1",
                CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Completed {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    tool_call_id: "tool-1".into(),
                    name: "edit".into(),
                    summary: "updated".into(),
                }),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                4,
                "op-1",
                CodingAgentProductEventKind::Delegation(
                    CodingAgentDelegationProductEvent::Started {
                        context: CodingAgentDelegationEventContext {
                            operation_id: "op-1".into(),
                            turn_id: "turn-1".into(),
                            tool_call_id: "delegate-1".into(),
                            requesting_profile_id: "default".into(),
                            target_kind: CodingAgentProductEventProfileKind::Agent,
                            target_id: "review".into(),
                            task: "review the change".into(),
                        },
                        child_operation_id: "child-1".into(),
                    },
                ),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                5,
                "op-1",
                CodingAgentProductEventKind::Agent(
                    CodingAgentAgentProductEvent::ProviderRequestStarted {
                        operation_id: "op-1".into(),
                        turn_id: "turn-1".into(),
                        provider: "test".into(),
                        model: "model-1".into(),
                        context_window: Some(128_000),
                    },
                ),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                6,
                "op-1",
                CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    message_id: Some("message-1".into()),
                    final_text: "done".into(),
                    images: Vec::new(),
                    usage: CodingAgentProductEventUsage {
                        input: 100,
                        output: 20,
                        cache_read: 5,
                        cache_write: 2,
                        total_tokens: 127,
                        cost_known: true,
                        input_cost: 0.001,
                        output_cost: 0.002,
                        cache_read_cost: 0.0,
                        cache_write_cost: 0.0,
                    },
                }),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                7,
                "op-1",
                CodingAgentProductEventKind::Workflow(
                    CodingAgentWorkflowProductEvent::PromptCompleted {
                        operation_id: "op-1".into(),
                        turn_id: "turn-1".into(),
                    },
                ),
                Some(terminal_prompt(
                    CodingAgentProductEventTerminalStatus::Completed,
                )),
            ),
            None,
        );

        assert_eq!(projection.operations.len(), 1);
        assert_eq!(projection.operations[0].kind, "prompt");
        assert_eq!(
            projection.operations[0].status,
            UiOperationStatus::Completed
        );
        assert_eq!(projection.changes.len(), 1);
        assert_eq!(projection.changes[0].path, "src/lib.rs");
        assert_eq!(projection.delegations.len(), 1);
        assert_eq!(projection.delegations[0].target_kind, "agent");
        assert_eq!(projection.delegations[0].status, "running");
        assert_eq!(projection.usage.input, 100);
        assert_eq!(
            projection
                .usage
                .latest_turn
                .as_ref()
                .unwrap()
                .context_tokens,
            Some(127)
        );
        assert_eq!(projection.usage.cost, Some(0.003));
        assert_eq!(projection.usage.model_id.as_deref(), Some("model-1"));
        assert_eq!(projection.usage.context_window, Some(128_000));
    }

    #[test]
    fn failed_mutations_and_unpriced_usage_remain_explicitly_unavailable() {
        let mut projection = UiContextProjection::default();
        projection.apply_product_event(
            &event(
                1,
                "op-1",
                CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Started {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    tool_call_id: "tool-1".into(),
                    name: "write".into(),
                    arguments_json: r#"{"file_path":"README.md","content":"x"}"#.into(),
                }),
                None,
            ),
            Some(OperationKind::Prompt),
        );
        projection.apply_product_event(
            &event(
                2,
                "op-1",
                CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Failed {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    tool_call_id: "tool-1".into(),
                    name: "write".into(),
                    message: "denied".into(),
                }),
                None,
            ),
            None,
        );
        projection.apply_product_event(
            &event(
                3,
                "op-1",
                CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
                    operation_id: "op-1".into(),
                    turn_id: "turn-1".into(),
                    message_id: None,
                    final_text: String::new(),
                    images: Vec::new(),
                    usage: CodingAgentProductEventUsage {
                        input: 1,
                        output: 1,
                        cache_read: 0,
                        cache_write: 0,
                        total_tokens: 2,
                        cost_known: true,
                        input_cost: 0.0,
                        output_cost: 0.0,
                        cache_read_cost: 0.0,
                        cache_write_cost: 0.0,
                    },
                }),
                None,
            ),
            None,
        );

        assert!(projection.changes.is_empty());
        assert_eq!(projection.usage.cost, None);
        assert_eq!(projection.usage.latest_turn.as_ref().unwrap().cost, None);
    }

    #[test]
    fn operation_history_is_bounded_and_keeps_the_latest_entries() {
        let mut projection = UiContextProjection::default();
        for sequence in 1..=40 {
            let operation_id = format!("op-{sequence}");
            projection.apply_product_event(
                &event(
                    sequence,
                    &operation_id,
                    CodingAgentProductEventKind::Workflow(
                        CodingAgentWorkflowProductEvent::PromptStarted {
                            operation_id: operation_id.clone(),
                            turn_id: format!("turn-{sequence}"),
                        },
                    ),
                    None,
                ),
                Some(OperationKind::Prompt),
            );
        }

        assert_eq!(projection.operations.len(), MAX_OPERATIONS);
        assert_eq!(projection.operations[0].operation_id, "op-40");
        assert!(
            projection
                .operations
                .iter()
                .all(|operation| operation.operation_id != "op-1")
        );
    }
}
