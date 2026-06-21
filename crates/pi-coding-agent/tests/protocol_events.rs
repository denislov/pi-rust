use pi_agent_core::{AgentToolOutput, AgentToolResult, session::StoredAgentMessage};
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, StopReason};
use pi_coding_agent::protocol::events::ProtocolEventAdapter;
use pi_coding_agent::protocol::types::ProtocolEvent;

fn assistant(text: &str) -> AssistantMessage {
    let mut msg = AssistantMessage::empty("faux", "faux-model");
    msg.provider = Some("faux".into());
    msg.content.push(ContentBlock::Text {
        text: text.into(),
        text_signature: None,
    });
    msg.stop_reason = StopReason::Stop;
    msg
}

#[test]
fn adapter_maps_text_stream_to_message_lifecycle() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let msg = assistant("hi");
    let events = adapter.push(&pi_agent_core::AgentEvent::LlmEvent(
        AssistantMessageEvent::Start {
            content_index: None,
            partial: AssistantMessage::empty("faux", "faux-model"),
        },
    ));
    assert!(matches!(events[0], ProtocolEvent::MessageStart { .. }));

    let events = adapter.push(&pi_agent_core::AgentEvent::LlmEvent(
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hi".into(),
            partial: msg.clone(),
        },
    ));
    assert!(matches!(events[0], ProtocolEvent::MessageUpdate { .. }));

    let events = adapter.push(&pi_agent_core::AgentEvent::AgentDone { message: msg });
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProtocolEvent::MessageEnd { .. }))
    );
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
fn adapter_maps_tool_events_with_content_result() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let mut msg = AssistantMessage::empty("faux", "faux-model");
    msg.provider = Some("faux".into());
    msg.content.push(ContentBlock::ToolCall {
        id: "tool_1".into(),
        name: "read".into(),
        arguments: serde_json::json!({"path": "Cargo.toml"}),
        thought_signature: None,
    });
    adapter.push(&pi_agent_core::AgentEvent::LlmEvent(
        AssistantMessageEvent::Done {
            reason: StopReason::ToolUse,
            message: msg,
        },
    ));

    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallStart {
        tool_call_id: "tool_1".into(),
        tool_name: "read".into(),
    });
    assert!(matches!(
        events[0],
        ProtocolEvent::ToolExecutionStart { .. }
    ));

    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallEnd {
        tool_call_id: "tool_1".into(),
        tool_name: "read".into(),
        result: AgentToolResult {
            content: vec![ContentBlock::Text {
                text: "file".into(),
                text_signature: None,
            }],
            is_error: false,
            terminate: false,
            details: None,
        },
    });
    assert!(matches!(
        events[0],
        ProtocolEvent::ToolExecutionEnd {
            is_error: false,
            ..
        }
    ));
}

#[test]
fn adapter_includes_tool_result_details() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallEnd {
        tool_call_id: "tool_1".into(),
        tool_name: "edit".into(),
        result: AgentToolResult {
            content: vec![ContentBlock::Text {
                text: "edited".into(),
                text_signature: None,
            }],
            is_error: false,
            terminate: false,
            details: Some(serde_json::json!({
                "diff": "-1 old\n+1 new",
                "firstChangedLine": 1
            })),
        },
    });

    match &events[0] {
        ProtocolEvent::ToolExecutionEnd { result, .. } => {
            assert_eq!(result.details.as_ref().unwrap()["firstChangedLine"], 1);
            assert_eq!(result.details.as_ref().unwrap()["diff"], "-1 old\n+1 new");
        }
        other => panic!("expected tool execution end, got {other:?}"),
    }
}

#[test]
fn adapter_maps_tool_update_event() {
    let mut adapter = ProtocolEventAdapter::new("faux".into(), "faux-model".into());
    let events = adapter.push(&pi_agent_core::AgentEvent::ToolCallUpdate {
        tool_call_id: "tool_1".into(),
        tool_name: "bash".into(),
        update: AgentToolOutput::new(vec![ContentBlock::Text {
            text: "partial output".into(),
            text_signature: None,
        }])
        .with_details(serde_json::json!({"stream": "stdout"})),
    });

    match &events[0] {
        ProtocolEvent::ToolExecutionUpdate {
            tool_call_id,
            tool_name,
            result,
        } => {
            assert_eq!(tool_call_id, "tool_1");
            assert_eq!(tool_name, "bash");
            assert_eq!(result.details.as_ref().unwrap()["stream"], "stdout");
            assert_eq!(
                result.content,
                vec![ContentBlock::Text {
                    text: "partial output".into(),
                    text_signature: None,
                }]
            );
        }
        other => panic!("expected tool execution update, got {other:?}"),
    }
}

#[test]
fn adapter_maps_agent_error_to_error_assistant_with_provider() {
    let mut adapter = ProtocolEventAdapter::new_with_provider(
        "faux".into(),
        "faux-provider".into(),
        "faux-model".into(),
    );

    let events = adapter.push(&pi_agent_core::AgentEvent::AgentError {
        error: "LLM failed".into(),
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
        } if provider == "faux-provider" && error_message == "LLM failed"
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
