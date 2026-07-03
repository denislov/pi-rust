use pi_agent_core::session::StoredAgentMessage;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason};
use pi_coding_agent::api::{CodingAgentEvent, ProfileKind};
use pi_coding_agent::protocol::events::CodingProtocolEventAdapter;
use pi_coding_agent::protocol::types::{CompactionReason, ProtocolEvent};
use serde_json::json;

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
                "task": "implement parser"
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
                "reason": "delegation target is not allowed"
            }),
            json!({
                "type": "delegation_approved",
                "operationId": "op_parent",
                "turnId": "turn_parent",
                "toolCallId": "tool_delegate",
                "requestingProfileId": "planner",
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser"
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
                "childOperationId": "op_child"
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
                "finalText": "child result"
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
