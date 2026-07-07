use pi_agent_core::transcript::StoredAgentMessage;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason, Usage};
use pi_coding_agent::api::{
    CodingAgentEvent, CodingSessionError, ProfileKind, SelfHealingEditCheckOutput,
    SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use pi_coding_agent::protocol::events::CodingProtocolEventAdapter;
use pi_coding_agent::protocol::types::{CompactionReason, ProtocolEvent};
use serde_json::{Value, json};

const FLOW_NODE_FIELD_NAMES: &[&str] = &[
    "flowNode",
    "flowNodeId",
    "flowNodeName",
    "lastNode",
    "nodeId",
];

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
        CodingAgentEvent::AgentTurnStarted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            agent_turn: 1,
        },
        CodingAgentEvent::AssistantMessageStarted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
        },
        CodingAgentEvent::AssistantThinkingDelta {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "think".into(),
        },
        CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        },
        CodingAgentEvent::AssistantMessageCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            final_text: "hello".into(),
            usage: Usage::default(),
        },
        CodingAgentEvent::PromptCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
        },
    ] {
        events.extend(adapter.push(&event));
    }

    assert!(matches!(events[0], ProtocolEvent::TurnStart));
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
        CodingAgentEvent::AgentTurnStarted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            agent_turn: 1,
        },
        CodingAgentEvent::AssistantMessageStarted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
        },
        CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
            text: "hello".into(),
        },
        CodingAgentEvent::AssistantMessageCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            message_id: Some("msg_prompt".into()),
            final_text: "hello".into(),
            usage: Usage::default(),
        },
        CodingAgentEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
        },
        CodingAgentEvent::RuntimeCompactionCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_prompt".into(),
            summary: "runtime summary".into(),
            first_kept_message_id: "msg_prompt".into(),
            tokens_before: 120,
        },
        CodingAgentEvent::SessionCompactionCompleted {
            operation_id: "op_compact".into(),
            turn_id: "turn_compact".into(),
            summary: "manual summary".into(),
            first_kept_message_id: "msg_prompt".into(),
            tokens_before: 100,
        },
        CodingAgentEvent::DefaultAgentProfileChanged {
            profile_id: "coder".into(),
        },
        CodingAgentEvent::SelfHealingEditStarted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            replacements: 1,
        },
        CodingAgentEvent::SelfHealingEditRepairAttempted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            attempt: 1,
            replacements: vec![SelfHealingEditReplacement::new("old", "new")],
            diagnostics: vec![SelfHealingEditDiagnostic {
                message: "fixed".into(),
            }],
            check_output: Some(check_output.clone()),
        },
        CodingAgentEvent::SelfHealingEditCompleted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            attempts: 2,
            first_changed_line: Some(2),
            check_output: Some(check_output),
        },
        CodingAgentEvent::DelegationRequested {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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
        CodingAgentEvent::AgentInvocationStarted {
            operation_id: "op_parent".into(),
            child_operation_id: "op_child".into(),
            profile_id: "coder".into(),
            task: "do work".into(),
        },
        CodingAgentEvent::AgentInvocationCompleted {
            operation_id: "op_parent".into(),
            child_operation_id: "op_child".into(),
            profile_id: "coder".into(),
            final_text: "done".into(),
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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
        CodingAgentEvent::AgentTeamStarted {
            operation_id: "op_team".into(),
            team_id: "implementation".into(),
            task: "ship feature".into(),
        },
        CodingAgentEvent::AgentTeamMemberStarted {
            operation_id: "op_team".into(),
            child_operation_id: "op_member".into(),
            team_id: "implementation".into(),
            profile_id: "coder".into(),
            task: "ship feature".into(),
        },
        CodingAgentEvent::AgentTeamMemberCompleted {
            operation_id: "op_team".into(),
            child_operation_id: "op_member".into(),
            team_id: "implementation".into(),
            profile_id: "coder".into(),
            final_text: "member done".into(),
        },
        CodingAgentEvent::AgentTeamCompleted {
            operation_id: "op_team".into(),
            team_id: "implementation".into(),
            final_text: "team done".into(),
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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
        CodingAgentEvent::DelegationRequested {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
        },
        CodingAgentEvent::DelegationStarted {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
            child_operation_id: "op_child".into(),
        },
        CodingAgentEvent::DelegationCompleted {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
            child_operation_id: "op_child".into(),
            final_text: "child result".into(),
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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
        CodingAgentEvent::DefaultAgentProfileChanged {
            profile_id: "coder".into(),
        },
        CodingAgentEvent::DelegationRequested {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
        },
        CodingAgentEvent::DelegationRejected {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate_team".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Team,
            target_id: "review-team".into(),
            task: "review parser".into(),
            reason: "delegation target is not allowed".into(),
        },
        CodingAgentEvent::DelegationApproved {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
        },
        CodingAgentEvent::DelegationConfirmationRequired {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate_team".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Team,
            target_id: "review-team".into(),
            task: "review parser".into(),
            reason: "team delegation requires confirmation under writes policy".into(),
        },
        CodingAgentEvent::DelegationStarted {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
            child_operation_id: "op_child".into(),
        },
        CodingAgentEvent::DelegationCompleted {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".into(),
            child_operation_id: "op_child".into(),
            final_text: "child result".into(),
        },
        CodingAgentEvent::DelegationFailed {
            operation_id: "op_parent".into(),
            turn_id: "turn_parent".into(),
            tool_call_id: "tool_delegate_failed".into(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "missing-coder".into(),
            task: "implement parser".into(),
            child_operation_id: "op_child_failed".into(),
            error: CodingSessionError::Input {
                message: "Unknown agent profile: missing-coder".into(),
            },
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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
        CodingAgentEvent::SelfHealingEditStarted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            replacements: 1,
        },
        CodingAgentEvent::SelfHealingEditRepairAttempted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            attempt: 1,
            replacements: vec![SelfHealingEditReplacement::new("deux", "dos")],
            diagnostics: vec![SelfHealingEditDiagnostic {
                message: "compile error".into(),
            }],
            check_output: Some(check_output.clone()),
        },
        CodingAgentEvent::SelfHealingEditCompleted {
            operation_id: "op_edit".into(),
            path: "src/app.txt".into(),
            attempts: 2,
            first_changed_line: Some(2),
            check_output: Some(check_output),
        },
        CodingAgentEvent::SelfHealingEditFailed {
            operation_id: "op_edit_failed".into(),
            path: "src/bad.txt".into(),
            error: CodingSessionError::Input {
                message: "bad edit".into(),
            },
        },
    ]
    .into_iter()
    .flat_map(|event| adapter.push(&event))
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

    let start = adapter.push(&CodingAgentEvent::ToolCallStarted {
        operation_id: "op_1".into(),
        turn_id: "turn_1".into(),
        tool_call_id: "tool_1".into(),
        name: "read".into(),
        arguments_json: r#"{"path":"Cargo.toml"}"#.into(),
    });
    assert!(matches!(start[0], ProtocolEvent::ToolExecutionStart { .. }));

    let update = adapter.push(&CodingAgentEvent::ToolCallUpdated {
        operation_id: "op_1".into(),
        turn_id: "turn_1".into(),
        tool_call_id: "tool_1".into(),
        name: "read".into(),
        message: "reading".into(),
    });
    assert!(matches!(
        update[0],
        ProtocolEvent::ToolExecutionUpdate { .. }
    ));

    let completed = adapter.push(&CodingAgentEvent::ToolCallCompleted {
        operation_id: "op_1".into(),
        turn_id: "turn_1".into(),
        tool_call_id: "tool_1".into(),
        name: "read".into(),
        summary: "file".into(),
    });
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

    let events = adapter.push(&CodingAgentEvent::SessionCompactionCompleted {
        operation_id: "op_1".into(),
        turn_id: "turn_1".into(),
        summary: "manual summary".into(),
        first_kept_message_id: "msg_2".into(),
        tokens_before: 1200,
    });

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

    let events = adapter.push(&CodingAgentEvent::PromptFailed {
        operation_id: "op_1".into(),
        error: pi_coding_agent::api::CodingSessionError::Provider {
            message: "LLM failed".into(),
        },
    });

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
