mod support;

use pi_agent_core::transcript::StoredAgentMessage;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason, Usage};
use pi_coding_agent::api::{
    CodingAgentAgentProductEvent, CodingAgentCapabilityProductEvent,
    CodingAgentDelegationEventContext, CodingAgentDelegationProductEvent,
    CodingAgentMessageProductEvent, CodingAgentProductEventCapabilityRevocation,
    CodingAgentProductEventKind, CodingAgentProductEventProfileKind,
    CodingAgentProfileProductEvent, CodingAgentRuntimeProductEvent, CodingAgentSessionProductEvent,
    CodingAgentTeamProductEvent, CodingAgentToolProductEvent, CodingAgentWorkflowProductEvent,
    CodingSessionError, SelfHealingEditCheckOutput, SelfHealingEditDiagnostic,
    SelfHealingEditReplacement,
};
use pi_coding_agent::protocol::events::CodingProtocolEventAdapter;
use pi_coding_agent::protocol::types::{
    CompactionReason, ProtocolEvent, RpcDetachLifecycleEvent, RpcDetachRequest, RpcDetachResponse,
    RpcDetachStatus, RpcShutdownLifecycleEvent, RpcShutdownRequest, RpcShutdownResponse,
    RpcShutdownStatus,
};
use serde_json::{Value, json};
use support::{
    product_check_output, product_diagnostic, product_error, product_event, product_replacement,
    product_usage,
};

const FLOW_NODE_FIELD_NAMES: &[&str] = &[
    "flowNode",
    "flowNodeId",
    "flowNodeName",
    "lastNode",
    "nodeId",
];

#[test]
fn lifecycle_wire_values_are_additive_and_exact() {
    assert_eq!(
        serde_json::to_value(RpcDetachRequest {
            id: Some("detach-1".into()),
        })
        .unwrap(),
        json!({"id": "detach-1", "type": "detach"})
    );
    assert_eq!(
        serde_json::to_value(RpcDetachResponse {
            status: RpcDetachStatus::AlreadyDetached,
        })
        .unwrap(),
        json!({"status": "already_detached"})
    );
    assert_eq!(
        serde_json::to_value(RpcDetachLifecycleEvent {
            status: RpcDetachStatus::Detached,
        })
        .unwrap(),
        json!({"type": "client_detached", "status": "detached"})
    );

    assert_eq!(
        serde_json::to_value(RpcShutdownRequest {
            id: Some("shutdown-1".into()),
        })
        .unwrap(),
        json!({"id": "shutdown-1", "type": "shutdown"})
    );
    assert_eq!(
        serde_json::to_value(RpcShutdownResponse {
            status: RpcShutdownStatus::AlreadyShutDown,
        })
        .unwrap(),
        json!({"status": "already_shut_down"})
    );
    assert_eq!(
        serde_json::to_value(RpcShutdownLifecycleEvent {
            status: RpcShutdownStatus::ShutDown,
        })
        .unwrap(),
        json!({"type": "runtime_shut_down", "status": "shut_down"})
    );

    assert_eq!(
        serde_json::to_value(RpcDetachStatus::StaleGeneration).unwrap(),
        json!("stale_generation")
    );
    assert_eq!(
        serde_json::to_value(RpcShutdownStatus::ShutdownRequested).unwrap(),
        json!("shutdown_requested")
    );
}

#[test]
fn product_event_adapter_reads_the_owned_typed_payload() {
    let source = include_str!("../src/protocol/events.rs");
    let rpc_source = include_str!("../src/protocol/rpc/events.rs");

    assert!(
        source.contains("event.event()"),
        "product-event projection must read the owned typed payload"
    );
    assert!(
        !source.contains("event.compatibility_event()"),
        "product-event projection must not consult compatibility storage"
    );
    assert!(
        rpc_source.contains("product_event.event()"),
        "RPC fixtures must validate typed payloads before forwarding"
    );
}

fn assert_no_flow_node_fields(value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                assert!(
                    !FLOW_NODE_FIELD_NAMES.contains(&key.as_str()),
                    "protocol event exposed Flow node field `{key}` in {value}"
                );
                assert_no_flow_node_fields(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_flow_node_fields(item);
            }
        }
        _ => {}
    }
}

#[test]
fn coding_event_adapter_maps_prompt_sequence_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let mut events = Vec::new();
    for event in [
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::TurnStarted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            agent_turn: 1,
        }),
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::ProviderRequestStarted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            provider: "typed-provider".into(),
            model: "typed-model".into(),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Started {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::ThinkingDelta {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "think".into(),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Delta {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            final_text: "hello".into(),
            usage: product_usage(Usage::default()),
        }),
        CodingAgentProductEventKind::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
        }),
    ] {
        events.extend(adapter.push_product_event(&product_event(event)));
    }

    assert!(matches!(events[0], ProtocolEvent::TurnStart));
    assert!(events.iter().any(|event| matches!(
        event,
        ProtocolEvent::MessageStart {
            message: StoredAgentMessage::Assistant {
                provider,
                model,
                ..
            }
        } if provider == "typed-provider" && model == "typed-model"
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProtocolEvent::MessageUpdate { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        ProtocolEvent::MessageUpdate {
            assistant_message_event: AssistantMessageEvent::ThinkingDelta {
                delta,
                partial,
                ..
            },
            ..
        } if delta == "think"
            && partial.content == vec![ContentBlock::Thinking {
                thinking: "think".into(),
                thinking_signature: None,
                redacted: None,
            }]
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProtocolEvent::TurnEnd { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProtocolEvent::AgentEnd { .. }))
    );
}

#[test]
fn product_event_protocol_adapter_does_not_emit_flow_node_fields() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );
    let check_output = SelfHealingEditCheckOutput {
        command: "cargo check".into(),
        stdout: "ok".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let events = [
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::TurnStarted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            agent_turn: 1,
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Started {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Delta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
            text: "hello".into(),
        }),
        CodingAgentProductEventKind::Message(CodingAgentMessageProductEvent::Completed {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
            final_text: "hello".into(),
            usage: product_usage(Usage::default()),
        }),
        CodingAgentProductEventKind::Workflow(CodingAgentWorkflowProductEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
        }),
        CodingAgentProductEventKind::Runtime(CodingAgentRuntimeProductEvent::CompactionCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            summary: "runtime summary".into(),
            first_kept_message_id: "msg_prompt".into(),
            tokens_before: 120,
        }),
        CodingAgentProductEventKind::Session(CodingAgentSessionProductEvent::CompactionCompleted {
            operation_id: "op_compact".into(),
            turn_id: "turn_compact".into(),
            summary: "manual summary".into(),
            first_kept_message_id: "msg_prompt".into(),
            tokens_before: 100,
        }),
        CodingAgentProductEventKind::Profile(CodingAgentProfileProductEvent::DefaultChanged {
            profile_id: "coder".into(),
        }),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                replacements: 1,
            },
        ),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                attempt: 1,
                replacements: vec![product_replacement(SelfHealingEditReplacement::new(
                    "old", "new",
                ))],
                diagnostics: vec![product_diagnostic(SelfHealingEditDiagnostic {
                    message: "fixed".into(),
                })],
                check_output: Some(product_check_output(check_output.clone())),
            },
        ),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                attempts: 2,
                first_changed_line: Some(2),
                check_output: Some(product_check_output(check_output)),
            },
        ),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Requested {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
        }),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert!(!events.is_empty());
    for event in events {
        assert_no_flow_node_fields(&event);
    }
}

#[test]
fn coding_event_adapter_maps_agent_invocation_lifecycle_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = [
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::InvocationStarted {
            operation_id: "op_parent".into(),
            child_operation_id: "op_child".into(),
            profile_id: "coder".into(),
            task: "do work".into(),
        }),
        CodingAgentProductEventKind::Agent(CodingAgentAgentProductEvent::InvocationCompleted {
            operation_id: "op_parent".into(),
            child_operation_id: "op_child".into(),
            profile_id: "coder".into(),
            final_text: "done".into(),
        }),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(
        events,
        vec![
            json!({
                "type": "agent_invocation_start",
                "operationId": "op_parent",
                "childOperationId": "op_child",
                "profileId": "coder",
                "task": "do work"
            }),
            json!({
                "type": "agent_invocation_end",
                "operationId": "op_parent",
                "childOperationId": "op_child",
                "profileId": "coder",
                "finalText": "done"
            })
        ]
    );
}

#[test]
fn coding_event_adapter_maps_agent_team_lifecycle_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = [
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Started {
            operation_id: "op_team".into(),
            team_id: "implementation".into(),
            task: "ship feature".into(),
        }),
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::MemberStarted {
            operation_id: "op_team".into(),
            child_operation_id: "op_member".into(),
            team_id: "implementation".into(),
            profile_id: "coder".into(),
            task: "ship feature".into(),
        }),
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::MemberCompleted {
            operation_id: "op_team".into(),
            child_operation_id: "op_member".into(),
            team_id: "implementation".into(),
            profile_id: "coder".into(),
            final_text: "member done".into(),
        }),
        CodingAgentProductEventKind::Team(CodingAgentTeamProductEvent::Completed {
            operation_id: "op_team".into(),
            team_id: "implementation".into(),
            final_text: "team done".into(),
        }),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(
        events,
        vec![
            json!({
                "type": "agent_team_start",
                "operationId": "op_team",
                "teamId": "implementation",
                "task": "ship feature"
            }),
            json!({
                "type": "agent_team_member_start",
                "operationId": "op_team",
                "childOperationId": "op_member",
                "teamId": "implementation",
                "profileId": "coder",
                "task": "ship feature"
            }),
            json!({
                "type": "agent_team_member_end",
                "operationId": "op_team",
                "childOperationId": "op_member",
                "teamId": "implementation",
                "profileId": "coder",
                "finalText": "member done"
            }),
            json!({
                "type": "agent_team_end",
                "operationId": "op_team",
                "teamId": "implementation",
                "finalText": "team done"
            })
        ]
    );
}

#[test]
fn delegation_protocol_events_include_folded_block_payload() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = [
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Requested {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Started {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
            child_operation_id: "op_child".into(),
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Completed {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
            child_operation_id: "op_child".into(),
            final_text: "child result".into(),
        }),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["foldedBlock"]["toolCallId"], "tool_delegate");
    assert_eq!(events[0]["foldedBlock"]["status"], "requested");
    assert_eq!(events[0]["foldedBlock"]["targetKind"], "agent");
    assert_eq!(events[0]["foldedBlock"]["targetId"], "coder");
    assert_eq!(events[0]["foldedBlock"]["task"], "implement parser");
    assert_eq!(events[0]["foldedBlock"]["isError"], false);

    assert_eq!(events[1]["foldedBlock"]["status"], "running");
    assert_eq!(events[1]["foldedBlock"]["childOperationId"], "op_child");

    assert_eq!(events[2]["foldedBlock"]["status"], "completed");
    assert_eq!(events[2]["foldedBlock"]["childOperationId"], "op_child");
    assert_eq!(
        events[2]["foldedBlock"]["summary"],
        "completed: child result"
    );
    assert_eq!(events[2]["foldedBlock"]["isError"], false);
}

#[test]
fn coding_event_adapter_maps_profile_and_delegation_lifecycle_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = [
        CodingAgentProductEventKind::Profile(CodingAgentProfileProductEvent::DefaultChanged {
            profile_id: "coder".into(),
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Requested {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Rejected {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate_team".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Team,
                target_id: "review-team".into(),
                task: "review parser".into(),
            },
            reason: "delegation target is not allowed".into(),
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Approved {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
        }),
        CodingAgentProductEventKind::Delegation(
            CodingAgentDelegationProductEvent::ConfirmationRequired {
                context: CodingAgentDelegationEventContext {
                    operation_id: "op_parent".into(),
                    turn_id: "turn_parent".into(),
                    tool_call_id: "tool_delegate_team".into(),
                    requesting_profile_id: "planner".into(),
                    target_kind: CodingAgentProductEventProfileKind::Team,
                    target_id: "review-team".into(),
                    task: "review parser".into(),
                },
                reason: "team delegation requires confirmation under writes policy".into(),
            },
        ),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Started {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
            child_operation_id: "op_child".into(),
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Completed {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "coder".into(),
                task: "implement parser".into(),
            },
            child_operation_id: "op_child".into(),
            final_text: "child result".into(),
        }),
        CodingAgentProductEventKind::Delegation(CodingAgentDelegationProductEvent::Failed {
            context: CodingAgentDelegationEventContext {
                operation_id: "op_parent".into(),
                turn_id: "turn_parent".into(),
                tool_call_id: "tool_delegate_failed".into(),
                requesting_profile_id: "planner".into(),
                target_kind: CodingAgentProductEventProfileKind::Agent,
                target_id: "missing-coder".into(),
                task: "implement parser".into(),
            },
            child_operation_id: "op_child_failed".into(),
            error: product_error(CodingSessionError::Input {
                message: "Unknown agent profile: missing-coder".into(),
            }),
        }),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(
        events,
        vec![
            json!({
                "type": "default_agent_profile_changed",
                "profileId": "coder"
            }),
            json!({
                "type": "delegation_requested",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser",
                "foldedBlock": {
                    "toolCallId": "tool_delegate",
                    "targetKind": "agent",
                    "targetId": "coder",
                    "task": "implement parser",
                    "status": "requested",
                    "summary": "requested",
                    "isError": false
                }
            }),
            json!({
                "type": "delegation_rejected",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate_team",
                "requestingProfileId": "planner",
                "targetKind": "team",
                "targetId": "review-team",
                "task": "review parser",
                "reason": "delegation target is not allowed",
                "foldedBlock": {
                    "toolCallId": "tool_delegate_team",
                    "targetKind": "team",
                    "targetId": "review-team",
                    "task": "review parser",
                    "status": "rejected",
                    "summary": "rejected: delegation target is not allowed",
                    "isError": true
                }
            }),
            json!({
                "type": "delegation_approved",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser",
                "foldedBlock": {
                    "toolCallId": "tool_delegate",
                    "targetKind": "agent",
                    "targetId": "coder",
                    "task": "implement parser",
                    "status": "approved",
                    "summary": "approved",
                    "isError": false
                }
            }),
            json!({
                "type": "delegation_confirmation_required",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate_team",
                "requestingProfileId": "planner",
                "targetKind": "team",
                "targetId": "review-team",
                "task": "review parser",
                "reason": "team delegation requires confirmation under writes policy",
                "foldedBlock": {
                    "toolCallId": "tool_delegate_team",
                    "targetKind": "team",
                    "targetId": "review-team",
                    "task": "review parser",
                    "status": "confirmation_required",
                    "summary": "confirmation required: team delegation requires confirmation under writes policy",
                    "isError": false
                }
            }),
            json!({
                "type": "delegation_started",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser",
                "childOperationId": "op_child",
                "foldedBlock": {
                    "toolCallId": "tool_delegate",
                    "targetKind": "agent",
                    "targetId": "coder",
                    "task": "implement parser",
                    "status": "running",
                    "childOperationId": "op_child",
                    "summary": "running",
                    "isError": false
                }
            }),
            json!({
                "type": "delegation_completed",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser",
                "childOperationId": "op_child",
                "finalText": "child result",
                "foldedBlock": {
                    "toolCallId": "tool_delegate",
                    "targetKind": "agent",
                    "targetId": "coder",
                    "task": "implement parser",
                    "status": "completed",
                    "childOperationId": "op_child",
                    "summary": "completed: child result",
                    "isError": false
                }
            }),
            json!({
                "type": "delegation_failed",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate_failed",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "missing-coder",
                "task": "implement parser",
                "childOperationId": "op_child_failed",
                "error": "invalid input: Unknown agent profile: missing-coder",
                "foldedBlock": {
                    "toolCallId": "tool_delegate_failed",
                    "targetKind": "agent",
                    "targetId": "missing-coder",
                    "task": "implement parser",
                    "status": "failed",
                    "childOperationId": "op_child_failed",
                    "summary": "failed: invalid input: Unknown agent profile: missing-coder",
                    "isError": true
                }
            })
        ]
    );
}

#[test]
fn coding_event_adapter_maps_self_healing_edit_lifecycle_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let check_output = SelfHealingEditCheckOutput {
        command: "cargo check".into(),
        stdout: "fixed".into(),
        stderr: String::new(),
        exit_code: 0,
    };
    let events = [
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditStarted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                replacements: 1,
            },
        ),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditRepairAttempted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                attempt: 1,
                replacements: vec![product_replacement(SelfHealingEditReplacement::new(
                    "deux", "dos",
                ))],
                diagnostics: vec![product_diagnostic(SelfHealingEditDiagnostic {
                    message: "compile error".into(),
                })],
                check_output: Some(product_check_output(check_output.clone())),
            },
        ),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditCompleted {
                operation_id: "op_edit".into(),
                path: "src/app.txt".into(),
                attempts: 2,
                first_changed_line: Some(2),
                check_output: Some(product_check_output(check_output)),
            },
        ),
        CodingAgentProductEventKind::Workflow(
            CodingAgentWorkflowProductEvent::SelfHealingEditFailed {
                operation_id: "op_edit_failed".into(),
                path: "src/bad.txt".into(),
                error: product_error(CodingSessionError::Input {
                    message: "bad edit".into(),
                }),
            },
        ),
    ]
    .into_iter()
    .flat_map(|event| adapter.push_product_event(&product_event(event)))
    .map(|event| serde_json::to_value(event).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(
        events,
        vec![
            json!({
                "type": "self_healing_edit_start",
                "operationId": "op_edit",
                "path": "src/app.txt",
                "replacements": 1
            }),
            json!({
                "type": "self_healing_edit_repair_attempt",
                "operationId": "op_edit",
                "path": "src/app.txt",
                "attempt": 1,
                "edits": [{"oldText": "deux", "newText": "dos"}],
                "diagnostics": ["compile error"],
                "checkOutput": {
                    "command": "cargo check",
                    "stdout": "fixed",
                    "stderr": "",
                    "exitCode": 0
                }
            }),
            json!({
                "type": "self_healing_edit_end",
                "operationId": "op_edit",
                "path": "src/app.txt",
                "attempts": 2,
                "firstChangedLine": 2,
                "checkOutput": {
                    "command": "cargo check",
                    "stdout": "fixed",
                    "stderr": "",
                    "exitCode": 0
                }
            }),
            json!({
                "type": "self_healing_edit_error",
                "operationId": "op_edit_failed",
                "path": "src/bad.txt",
                "error": "invalid input: bad edit"
            })
        ]
    );
}

#[test]
fn coding_event_adapter_maps_tool_events_to_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let start = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Tool(
        CodingAgentToolProductEvent::Started {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: "tool_1".into(),
            name: "read".into(),
            arguments_json: r#"{"path":"Cargo.toml"}"#.into(),
        },
    )));
    assert!(matches!(
        &start[0],
        ProtocolEvent::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        } if tool_call_id == "tool_1"
            && tool_name == "read"
            && args == &json!({"path": "Cargo.toml"})
    ));

    let invalid = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Tool(
        CodingAgentToolProductEvent::Started {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: "tool_invalid".into(),
            name: "read".into(),
            arguments_json: "not-json".into(),
        },
    )));
    assert!(matches!(
        &invalid[0],
        ProtocolEvent::ToolExecutionStart { args, .. } if args == &Value::Null
    ));

    let update = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Tool(
        CodingAgentToolProductEvent::Updated {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: "tool_1".into(),
            name: "read".into(),
            message: "reading".into(),
        },
    )));
    assert!(matches!(
        update[0],
        ProtocolEvent::ToolExecutionUpdate { .. }
    ));

    let completed = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Tool(
        CodingAgentToolProductEvent::Completed {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: "tool_1".into(),
            name: "read".into(),
            summary: "file".into(),
        },
    )));
    assert!(matches!(
        completed[0],
        ProtocolEvent::ToolExecutionEnd {
            is_error: false,
            ..
        }
    ));
}

#[test]
fn coding_event_adapter_maps_session_compaction_as_manual_protocol_events() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Session(
        CodingAgentSessionProductEvent::CompactionCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            summary: "manual summary".into(),
            first_kept_message_id: "msg_2".into(),
            tokens_before: 1200,
        },
    )));

    assert!(matches!(
        events.as_slice(),
        [
            ProtocolEvent::CompactionStart {
                reason: CompactionReason::Manual,
            },
            ProtocolEvent::CompactionEnd {
                reason: CompactionReason::Manual,
                result: Some(result),
                aborted: false,
                will_retry: false,
                error_message: None,
            },
        ] if result.summary == "manual summary"
            && result.first_kept_message_id == "msg_2"
            && result.tokens_before == 1200
    ));
}

#[test]
fn coding_event_adapter_maps_prompt_failure_with_provider() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = adapter.push_product_event(&product_event(CodingAgentProductEventKind::Workflow(
        CodingAgentWorkflowProductEvent::PromptFailed {
            operation_id: "op_1".into(),
            error: product_error(pi_coding_agent::api::CodingSessionError::Provider {
                message: "LLM failed".into(),
            }),
        },
    )));

    assert!(matches!(
        &events[0],
        ProtocolEvent::MessageStart {
            message: StoredAgentMessage::Assistant {
                provider,
                stop_reason: StopReason::Error,
                error_message: Some(error_message),
                ..
            }
        } if provider == "faux-provider" && error_message == "provider error: LLM failed"
    ));
    assert!(events.iter().any(|event| matches!(
        event,
        ProtocolEvent::TurnEnd {
            message: StoredAgentMessage::Assistant {
                provider,
                stop_reason: StopReason::Error,
                ..
            },
            ..
        } if provider == "faux-provider"
    )));
}

#[test]
fn coding_event_adapter_maps_capability_changed_to_payloaded_protocol_event() {
    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = adapter.push_product_event(&product_event(
        CodingAgentProductEventKind::Capability(CodingAgentCapabilityProductEvent::Changed {
            generation: 7,
            revocation: CodingAgentProductEventCapabilityRevocation::FutureOnly,
        }),
    ));

    assert!(matches!(
        events.as_slice(),
        [ProtocolEvent::CapabilityChanged {
            generation: 7,
            revocation,
        }] if revocation == "future_only"
    ));
}
