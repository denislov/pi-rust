use pi_agent_core::session::StoredAgentMessage;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, StopReason};
use pi_coding_agent::api::CodingAgentEvent;
use pi_coding_agent::protocol::events::CodingProtocolEventAdapter;
use pi_coding_agent::protocol::types::ProtocolEvent;

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
