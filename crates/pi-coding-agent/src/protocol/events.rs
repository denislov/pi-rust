use crate::coding_session::{
    CodingAgentAgentProductEvent, CodingAgentCapabilityProductEvent,
    CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
    CodingAgentDiagnosticProductEvent, CodingAgentMessageProductEvent, CodingAgentProductEvent,
    CodingAgentProductEventCheckOutput, CodingAgentProductEventKind,
    CodingAgentProductEventProfileKind, CodingAgentProductEventReplacement,
    CodingAgentProfileProductEvent, CodingAgentRuntimeProductEvent, CodingAgentSessionProductEvent,
    CodingAgentTeamProductEvent, CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
    ProductEvent,
};
use crate::protocol::types::{
    CompactionProtocolResult, CompactionReason, ProtocolDelegationFoldedBlock, ProtocolEvent,
    ProtocolSelfHealingEditCheckOutput, ProtocolSelfHealingEditReplacement, ToolExecutionResult,
};
use pi_agent_core::api::{StoredAgentMessage, StoredUsage, StoredUsageCost};
use pi_ai::api::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};

pub struct CodingProtocolEventAdapter {
    api: String,
    provider: String,
    model: String,
    messages: Vec<StoredAgentMessage>,
    current_assistant: Option<AssistantMessage>,
    current_tool_results: Vec<StoredAgentMessage>,
    assistant_open: bool,
}

impl CodingProtocolEventAdapter {
    pub fn new_with_provider(api: String, provider: String, model: String) -> Self {
        Self {
            api,
            provider,
            model,
            messages: Vec::new(),
            current_assistant: None,
            current_tool_results: Vec::new(),
            assistant_open: false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn push_internal_product_event(
        &mut self,
        event: &ProductEvent,
    ) -> Vec<ProtocolEvent> {
        self.push_typed(event.event())
    }

    pub fn push_product_event(&mut self, event: &CodingAgentProductEvent) -> Vec<ProtocolEvent> {
        self.push_typed(event.event())
    }

    pub(crate) fn push_prompt_failure(&mut self, message: &str) -> Vec<ProtocolEvent> {
        self.push_prompt_failed_message(message)
    }

    fn push_typed(&mut self, event: &CodingAgentProductEventKind) -> Vec<ProtocolEvent> {
        match event {
            CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::TurnStarted {
                ..
            }) => {
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::TurnStart);
                events
            }
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::ProviderRequestStarted {
                    provider, model, ..
                },
            ) => {
                self.provider = provider.clone();
                self.model = model.clone();
                Vec::new()
            }
            CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Started {
                ..
            }) => {
                if self.assistant_open {
                    return Vec::new();
                }
                let message = self.ensure_assistant();
                self.assistant_open = true;
                vec![ProtocolEvent::MessageStart {
                    message: stored_assistant(&message),
                }]
            }
            CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Delta {
                text,
                ..
            }) => {
                let (content_index, message) = self.append_assistant_text(text);
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                events.push(ProtocolEvent::MessageUpdate {
                    message: stored_assistant(&message),
                    assistant_message_event: AssistantMessageEvent::TextDelta {
                        content_index,
                        delta: text.clone(),
                        partial: message,
                    },
                });
                events
            }
            CodingAgentProductEventKind::Message(
                CodingAgentMessageProductEvent::ThinkingDelta { text, .. },
            ) => {
                let (content_index, message) = self.append_assistant_thinking(text);
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                events.push(ProtocolEvent::MessageUpdate {
                    message: stored_assistant(&message),
                    assistant_message_event: AssistantMessageEvent::ThinkingDelta {
                        content_index,
                        delta: text.clone(),
                        partial: message,
                    },
                });
                events
            }
            CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
                final_text,
                ..
            }) => {
                let mut message = self.ensure_assistant();
                if message.content.is_empty() && !final_text.is_empty() {
                    message.content = text_content(final_text);
                }
                let mut events = Vec::new();
                if !self.assistant_open {
                    self.assistant_open = true;
                    events.push(ProtocolEvent::MessageStart {
                        message: stored_assistant(&message),
                    });
                }
                self.current_assistant = Some(message);
                events
            }
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Started {
                tool_call_id,
                name,
                arguments_json,
                ..
            }) => vec![ProtocolEvent::ToolExecutionStart {
                tool_call_id: tool_call_id.clone(),
                tool_name: name.clone(),
                args: serde_json::from_str(arguments_json).unwrap_or(serde_json::Value::Null),
            }],
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Updated {
                tool_call_id,
                name,
                message,
                ..
            }) => vec![ProtocolEvent::ToolExecutionUpdate {
                tool_call_id: tool_call_id.clone(),
                tool_name: name.clone(),
                result: ToolExecutionResult {
                    content: text_content(message),
                    terminate: false,
                    details: None,
                },
            }],
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Completed {
                tool_call_id,
                name,
                summary,
                ..
            }) => self.push_tool_result(tool_call_id, name, summary, false),
            CodingAgentProductEventKind::Tool(CodingAgentToolProductEvent::Failed {
                tool_call_id,
                name,
                message,
                ..
            }) => self.push_tool_result(tool_call_id, name, message, true),
            CodingAgentProductEventKind::Runtime(
                CodingAgentRuntimeProductEvent::CompactionCompleted {
                    summary,
                    first_kept_message_id,
                    tokens_before,
                    ..
                },
            ) => Self::compaction_events(
                CompactionReason::Threshold,
                summary,
                first_kept_message_id,
                *tokens_before,
            ),
            CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::ShutDown) => {
                Vec::new()
            }
            CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::CompactionCompleted {
                    summary,
                    first_kept_message_id,
                    tokens_before,
                    ..
                },
            ) => Self::compaction_events(
                CompactionReason::Manual,
                summary,
                first_kept_message_id,
                *tokens_before,
            ),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptCompleted { .. },
            ) => {
                let mut events = self.finish_current_turn();
                events.push(ProtocolEvent::AgentEnd {
                    messages: self.messages.clone(),
                });
                events
            }
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptFailed { error, .. },
            ) => self.push_prompt_failed_message(&error.message),
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptAborted { reason, .. },
            ) => self.push_prompt_failed_message(reason),
            CodingAgentProductEventKind::Profile(
                CodingAgentProfileProductEvent::DefaultChanged { profile_id },
            ) => {
                vec![ProtocolEvent::DefaultAgentProfileChanged {
                    profile_id: profile_id.as_str().to_string(),
                }]
            }
            CodingAgentProductEventKind::Capability(
                CodingAgentCapabilityProductEvent::Changed {
                    generation,
                    revocation,
                },
            ) => vec![ProtocolEvent::CapabilityChanged {
                generation: *generation,
                revocation: capability_revocation_to_protocol(*revocation).to_owned(),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::OperationRecovered {
                    operation_id,
                    recovery_id,
                    reason,
                },
            ) => vec![ProtocolEvent::OperationRecovered {
                operation_id: operation_id.clone(),
                recovery_id: recovery_id.clone(),
                reason: reason.clone(),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                    operation_id,
                    path,
                    replacements,
                },
            ) => vec![ProtocolEvent::SelfHealingEditStart {
                operation_id: operation_id.clone(),
                path: path.clone(),
                replacements: *replacements,
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                    operation_id,
                    path,
                    attempt,
                    replacements,
                    diagnostics,
                    check_output,
                },
            ) => vec![ProtocolEvent::SelfHealingEditRepairAttempt {
                operation_id: operation_id.clone(),
                path: path.clone(),
                attempt: *attempt,
                edits: protocol_self_healing_replacements(replacements),
                diagnostics: diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect(),
                check_output: check_output
                    .as_ref()
                    .map(protocol_self_healing_check_output),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                    operation_id,
                    path,
                    attempts,
                    first_changed_line,
                    check_output,
                },
            ) => vec![ProtocolEvent::SelfHealingEditEnd {
                operation_id: operation_id.clone(),
                path: path.clone(),
                attempts: *attempts,
                first_changed_line: *first_changed_line,
                check_output: check_output
                    .as_ref()
                    .map(protocol_self_healing_check_output),
            }],
            CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::SelfHealingEditFailed {
                    operation_id,
                    path,
                    error,
                },
            ) => vec![ProtocolEvent::SelfHealingEditError {
                operation_id: operation_id.clone(),
                path: path.clone(),
                error: error.message.clone(),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Requested {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                },
            ) => vec![ProtocolEvent::DelegationRequested {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "requested",
                    None,
                    Some("requested".into()),
                    false,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Rejected {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                    reason,
                },
            ) => vec![ProtocolEvent::DelegationRejected {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                reason: reason.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "rejected",
                    None,
                    Some(format!("rejected: {reason}")),
                    true,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Approved {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                },
            ) => vec![ProtocolEvent::DelegationApproved {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "approved",
                    None,
                    Some("approved".into()),
                    false,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::ConfirmationRequired {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                    reason,
                },
            ) => vec![ProtocolEvent::DelegationConfirmationRequired {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                reason: reason.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "confirmation_required",
                    None,
                    Some(format!("confirmation required: {reason}")),
                    false,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Started {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                    child_operation_id,
                },
            ) => vec![ProtocolEvent::DelegationStarted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                child_operation_id: child_operation_id.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "running",
                    Some(child_operation_id.clone()),
                    Some("running".into()),
                    false,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Completed {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                    child_operation_id,
                    final_text,
                },
            ) => vec![ProtocolEvent::DelegationCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                child_operation_id: child_operation_id.clone(),
                final_text: final_text.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "completed",
                    Some(child_operation_id.clone()),
                    Some(format!("completed: {final_text}")),
                    false,
                ),
            }],
            CodingAgentProductEventKind::Delegation(
                CodingAgentDelegationProductEvent::Failed {
                    context:
                        CodingAgentDelegationEventContext {
                            operation_id,
                            turn_id,
                            tool_call_id,
                            requesting_profile_id,
                            target_kind,
                            target_id,
                            task,
                        },
                    child_operation_id,
                    error,
                },
            ) => vec![ProtocolEvent::DelegationFailed {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                requesting_profile_id: requesting_profile_id.as_str().to_string(),
                target_kind: profile_kind_to_protocol(*target_kind).to_string(),
                target_id: target_id.as_str().to_string(),
                task: task.clone(),
                child_operation_id: child_operation_id.clone(),
                error: error.message.clone(),
                folded_block: delegation_folded_block(
                    tool_call_id,
                    *target_kind,
                    target_id.as_str(),
                    task,
                    "failed",
                    Some(child_operation_id.clone()),
                    Some(format!("failed: {}", error.message)),
                    true,
                ),
            }],
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::InvocationStarted {
                    operation_id,
                    child_operation_id,
                    profile_id,
                    task,
                },
            ) => vec![ProtocolEvent::AgentInvocationStart {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile_id.as_str().to_string(),
                task: task.clone(),
            }],
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::InvocationCompleted {
                    operation_id,
                    child_operation_id,
                    profile_id,
                    final_text,
                },
            ) => vec![ProtocolEvent::AgentInvocationEnd {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile_id.as_str().to_string(),
                final_text: final_text.clone(),
            }],
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::InvocationFailed {
                    operation_id,
                    child_operation_id,
                    profile_id,
                    error,
                },
            ) => vec![ProtocolEvent::AgentInvocationError {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile_id.as_str().to_string(),
                error: error.message.clone(),
            }],
            CodingAgentProductEventKind::Agent(
                CodingAgentAgentProductEvent::InvocationAborted {
                    operation_id,
                    child_operation_id,
                    profile_id,
                    reason,
                },
            ) => vec![ProtocolEvent::AgentInvocationAbort {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                profile_id: profile_id.as_str().to_string(),
                reason: reason.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Started {
                operation_id,
                team_id,
                task,
            }) => vec![ProtocolEvent::AgentTeamStart {
                operation_id: operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                task: task.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::MemberStarted {
                operation_id,
                child_operation_id,
                team_id,
                profile_id,
                task,
            }) => vec![ProtocolEvent::AgentTeamMemberStart {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                profile_id: profile_id.as_str().to_string(),
                task: task.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::MemberCompleted {
                operation_id,
                child_operation_id,
                team_id,
                profile_id,
                final_text,
            }) => vec![ProtocolEvent::AgentTeamMemberEnd {
                operation_id: operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                profile_id: profile_id.as_str().to_string(),
                final_text: final_text.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Completed {
                operation_id,
                team_id,
                final_text,
            }) => vec![ProtocolEvent::AgentTeamEnd {
                operation_id: operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                final_text: final_text.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Failed {
                operation_id,
                team_id,
                error,
            }) => vec![ProtocolEvent::AgentTeamError {
                operation_id: operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                error: error.message.clone(),
            }],
            CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Aborted {
                operation_id,
                team_id,
                reason,
            }) => vec![ProtocolEvent::AgentTeamAbort {
                operation_id: operation_id.clone(),
                team_id: team_id.as_str().to_string(),
                reason: reason.clone(),
            }],
            CodingAgentProductEventKind::Session(CodingAgentSessionProductEvent::Opened {
                ..
            })
            | CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::WritePending { .. },
            )
            | CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::WriteCommitted { .. },
            )
            | CodingAgentProductEventKind::Session(
                CodingAgentSessionProductEvent::WriteSkipped { .. },
            )
            | CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::PromptStarted { .. },
            )
            | CodingAgentProductEventKind::Diagnostic(
                CodingAgentDiagnosticProductEvent::Diagnostic { .. },
            ) => Vec::new(),
        }
    }

    fn ensure_assistant(&mut self) -> AssistantMessage {
        if self.current_assistant.is_none() {
            self.current_assistant = Some(self.assistant_message(""));
        }
        self.current_assistant
            .clone()
            .expect("assistant was inserted when missing")
    }

    fn append_assistant_text(&mut self, text: &str) -> (u32, AssistantMessage) {
        let mut message = self.ensure_assistant();
        let content_index = append_text_content(&mut message, text);
        self.current_assistant = Some(message.clone());
        (content_index, message)
    }

    fn append_assistant_thinking(&mut self, text: &str) -> (u32, AssistantMessage) {
        let mut message = self.ensure_assistant();
        let content_index = append_thinking_content(&mut message, text);
        self.current_assistant = Some(message.clone());
        (content_index, message)
    }

    fn assistant_message(&self, text: &str) -> AssistantMessage {
        let mut message = AssistantMessage::empty(&self.api, &self.model);
        if !self.provider.is_empty() {
            message.provider = Some(self.provider.clone());
        }
        if !text.is_empty() {
            message.content = text_content(text);
        }
        message
    }

    fn compaction_events(
        reason: CompactionReason,
        summary: &str,
        first_kept_message_id: &str,
        tokens_before: u32,
    ) -> Vec<ProtocolEvent> {
        vec![
            ProtocolEvent::CompactionStart { reason },
            ProtocolEvent::CompactionEnd {
                reason,
                result: Some(CompactionProtocolResult {
                    summary: summary.to_owned(),
                    first_kept_message_id: first_kept_message_id.to_owned(),
                    tokens_before,
                    details: None,
                }),
                aborted: false,
                will_retry: false,
                error_message: None,
            },
        ]
    }

    fn push_tool_result(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        text: &str,
        is_error: bool,
    ) -> Vec<ProtocolEvent> {
        let content = text_content(text);
        let tool_result = StoredAgentMessage::ToolResult {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            content: content.clone(),
            is_error,
            timestamp: 0,
        };
        self.current_tool_results.push(tool_result.clone());

        vec![
            ProtocolEvent::ToolExecutionEnd {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                result: ToolExecutionResult {
                    content,
                    terminate: false,
                    details: None,
                },
                is_error,
            },
            ProtocolEvent::MessageStart {
                message: tool_result.clone(),
            },
            ProtocolEvent::MessageEnd {
                message: tool_result,
            },
        ]
    }

    fn push_prompt_failed_message(&mut self, error: &str) -> Vec<ProtocolEvent> {
        let message = stored_error_assistant(&self.api, &self.provider, &self.model, error);
        self.messages.push(message.clone());
        vec![
            ProtocolEvent::MessageStart {
                message: message.clone(),
            },
            ProtocolEvent::MessageEnd {
                message: message.clone(),
            },
            ProtocolEvent::TurnEnd {
                message,
                tool_results: Vec::new(),
            },
            ProtocolEvent::AgentEnd {
                messages: self.messages.clone(),
            },
        ]
    }

    fn finish_current_turn(&mut self) -> Vec<ProtocolEvent> {
        let Some(message) = self.current_assistant.take() else {
            return Vec::new();
        };

        let stored = stored_assistant(&message);
        if !self.messages.contains(&stored) {
            self.messages.push(stored.clone());
        }
        for tool_result in &self.current_tool_results {
            if !self.messages.contains(tool_result) {
                self.messages.push(tool_result.clone());
            }
        }

        let events = vec![
            ProtocolEvent::MessageEnd {
                message: stored.clone(),
            },
            ProtocolEvent::TurnEnd {
                message: stored,
                tool_results: self.current_tool_results.clone(),
            },
        ];
        self.current_tool_results.clear();
        self.assistant_open = false;
        events
    }
}

fn protocol_self_healing_replacements(
    replacements: &[CodingAgentProductEventReplacement],
) -> Vec<ProtocolSelfHealingEditReplacement> {
    replacements
        .iter()
        .map(|replacement| ProtocolSelfHealingEditReplacement {
            old_text: replacement.old_text.clone(),
            new_text: replacement.new_text.clone(),
        })
        .collect()
}

fn protocol_self_healing_check_output(
    output: &CodingAgentProductEventCheckOutput,
) -> ProtocolSelfHealingEditCheckOutput {
    ProtocolSelfHealingEditCheckOutput {
        command: output.command.clone(),
        stdout: output.stdout.clone(),
        stderr: output.stderr.clone(),
        exit_code: output.exit_code,
    }
}

fn delegation_folded_block(
    tool_call_id: &str,
    target_kind: CodingAgentProductEventProfileKind,
    target_id: &str,
    task: &str,
    status: &str,
    child_operation_id: Option<String>,
    summary: Option<String>,
    is_error: bool,
) -> ProtocolDelegationFoldedBlock {
    ProtocolDelegationFoldedBlock {
        tool_call_id: tool_call_id.to_string(),
        target_kind: profile_kind_to_protocol(target_kind).to_string(),
        target_id: target_id.to_string(),
        task: task.to_string(),
        status: status.to_string(),
        child_operation_id,
        summary,
        is_error,
    }
}

fn profile_kind_to_protocol(kind: CodingAgentProductEventProfileKind) -> &'static str {
    match kind {
        CodingAgentProductEventProfileKind::Agent => "agent",
        CodingAgentProductEventProfileKind::Team => "team",
    }
}

fn capability_revocation_to_protocol(
    revocation: crate::coding_session::CodingAgentProductEventCapabilityRevocation,
) -> &'static str {
    match revocation {
        crate::coding_session::CodingAgentProductEventCapabilityRevocation::FutureOnly => {
            "future_only"
        }
        crate::coding_session::CodingAgentProductEventCapabilityRevocation::CancelMatchingOperations => {
            "cancel_matching_operations"
        }
    }
}

fn append_text_content(message: &mut AssistantMessage, text: &str) -> u32 {
    let last_index = message.content.len().saturating_sub(1) as u32;
    match message.content.last_mut() {
        Some(ContentBlock::Text { text: existing, .. }) => {
            existing.push_str(text);
            last_index
        }
        _ => {
            let index = message.content.len() as u32;
            message.content.push(ContentBlock::Text {
                text: text.to_string(),
                text_signature: None,
            });
            index
        }
    }
}

fn append_thinking_content(message: &mut AssistantMessage, text: &str) -> u32 {
    let last_index = message.content.len().saturating_sub(1) as u32;
    match message.content.last_mut() {
        Some(ContentBlock::Thinking { thinking, .. }) => {
            thinking.push_str(text);
            last_index
        }
        _ => {
            let index = message.content.len() as u32;
            message.content.push(ContentBlock::Thinking {
                thinking: text.to_string(),
                thinking_signature: None,
                redacted: None,
            });
            index
        }
    }
}

fn text_content(text: &str) -> Vec<ContentBlock> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![ContentBlock::Text {
            text: text.to_string(),
            text_signature: None,
        }]
    }
}

fn stored_assistant(message: &AssistantMessage) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: message.content.clone(),
        api: message.api.clone(),
        provider: message.provider.clone().unwrap_or_default(),
        model: message.model.clone(),
        response_model: message.response_model.clone(),
        response_id: message.response_id.clone(),
        usage: StoredUsage {
            input: message.usage.input,
            output: message.usage.output,
            cache_read: message.usage.cache_read,
            cache_write: message.usage.cache_write,
            total: message.usage.total_tokens,
            cost: StoredUsageCost {
                input: message.usage.cost.input,
                output: message.usage.cost.output,
                cache_read: message.usage.cost.cache_read,
                cache_write: message.usage.cost.cache_write,
            },
        },
        stop_reason: message.stop_reason.clone(),
        error_message: message.error_message.clone(),
        timestamp: message.timestamp,
    }
}

fn stored_error_assistant(
    api: &str,
    provider: &str,
    model: &str,
    error: &str,
) -> StoredAgentMessage {
    StoredAgentMessage::Assistant {
        content: Vec::new(),
        api: api.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        response_model: None,
        response_id: None,
        usage: StoredUsage::default(),
        stop_reason: StopReason::Error,
        error_message: Some(error.to_string()),
        timestamp: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{CodingAgentEvent, ProductEvent, ProductEventSequence};

    #[test]
    fn protocol_adapter_maps_operation_recovered_to_recovery_event() {
        let mut adapter = CodingProtocolEventAdapter::new_with_provider(
            "faux".into(),
            "faux-provider".into(),
            "faux-model".into(),
        );
        let product_event = ProductEvent::from_event_for_tests(
            ProductEventSequence::new(1),
            CodingAgentEvent::OperationRecovered {
                operation_id: "op_recovered".into(),
                recovery_id: "recovery_1".into(),
                reason: "startup recovery marked incomplete operation in-doubt".into(),
            },
        );

        let events = adapter.push_internal_product_event(&product_event);

        assert!(matches!(
            &events[0],
            ProtocolEvent::OperationRecovered {
                operation_id,
                recovery_id,
                reason,
            } if operation_id == "op_recovered"
                && recovery_id == "recovery_1"
                && reason.contains("startup recovery")
        ));
    }

    #[test]
    fn protocol_adapter_does_not_masquerade_shutdown_as_prompt_completion() {
        let mut adapter = CodingProtocolEventAdapter::new_with_provider(
            "faux".into(),
            "faux-provider".into(),
            "faux-model".into(),
        );
        let product_event = ProductEvent::from_event_for_tests(
            ProductEventSequence::new(1),
            CodingAgentEvent::RuntimeShutDown,
        );

        assert!(
            adapter
                .push_internal_product_event(&product_event)
                .is_empty()
        );
    }
}
