use pi_agent_core::{AgentEvent, AgentToolOutput, AgentToolResult};
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Cost, StopReason, Usage,
};
use pi_coding_agent::api::{CodingAgentEvent, CodingSessionError};
use pi_coding_agent::interactive::{
    CodingEventBridge, InteractiveEventBridge, Transcript, TranscriptItem, UiEvent,
};

fn assistant(text: &str) -> AssistantMessage {
    let mut message = AssistantMessage::empty("faux", "faux-model");
    message.provider = Some("faux".to_string());
    message.stop_reason = StopReason::Stop;
    message.content.push(ContentBlock::Text {
        text: text.to_string(),
        text_signature: None,
    });
    message
}

fn assistant_done_message(input: u32, output: u32) -> AssistantMessage {
    let mut message = AssistantMessage::empty("faux", "faux-model");
    message.provider = Some("faux".to_string());
    message.stop_reason = StopReason::Stop;
    message.content.push(ContentBlock::Text {
        text: "done".to_string(),
        text_signature: None,
    });
    message.usage = Usage {
        input,
        output,
        cache_read: 0,
        cache_write: 0,
        total_tokens: input + output,
        cost: Cost::default(),
    };
    message
}

#[test]
fn text_delta_updates_assistant_markdown() {
    let mut bridge = InteractiveEventBridge::new();
    let events = bridge.handle(&AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta {
        content_index: 0,
        delta: "hello".to_string(),
        partial: AssistantMessage::empty("faux", "faux-model"),
    }));
    assert_eq!(
        events,
        vec![UiEvent::AssistantDelta {
            text: "hello".to_string()
        }]
    );
}

#[test]
fn tool_events_map_to_start_and_end_rows() {
    let mut bridge = InteractiveEventBridge::new();
    let start = bridge.handle(&AgentEvent::ToolCallStart {
        tool_call_id: "tool_1".to_string(),
        tool_name: "read".to_string(),
        arguments: serde_json::json!({"path": "src/lib.rs"}),
    });
    assert_eq!(
        start,
        vec![UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        }]
    );

    let end = bridge.handle(&AgentEvent::ToolCallEnd {
        tool_call_id: "tool_1".to_string(),
        tool_name: "read".to_string(),
        result: AgentToolResult {
            content: vec![ContentBlock::Text {
                text: "ok".to_string(),
                text_signature: None,
            }],
            is_error: false,
            terminate: false,
            details: None,
        },
    });
    assert_eq!(
        end,
        vec![UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "ok".to_string(),
            is_error: false,
        }]
    );
}

#[test]
fn tool_update_replaces_running_tool_output() {
    let mut bridge = InteractiveEventBridge::new();
    let update = bridge.handle(&AgentEvent::ToolCallUpdate {
        tool_call_id: "tool_1".to_string(),
        tool_name: "bash".to_string(),
        update: AgentToolOutput::new(vec![ContentBlock::Text {
            text: "line 1".to_string(),
            text_signature: None,
        }]),
    });

    assert_eq!(
        update,
        vec![UiEvent::ToolUpdated {
            call_id: "tool_1".to_string(),
            result: "line 1".to_string(),
        }]
    );

    let mut transcript = Transcript::new();
    transcript.apply_event(UiEvent::ToolStarted {
        call_id: "tool_1".to_string(),
        name: "bash".to_string(),
        args: serde_json::Value::Null,
    });
    transcript.apply_event(update.into_iter().next().unwrap());

    assert_eq!(
        transcript.items(),
        &[TranscriptItem::Tool {
            call_id: "tool_1".to_string(),
            name: "bash".to_string(),
            args: serde_json::Value::Null,
            result: Some("line 1".to_string()),
            is_error: false,
        }]
    );
}

#[test]
fn agent_done_marks_assistant_complete() {
    let mut bridge = InteractiveEventBridge::new();
    let events = bridge.handle(&AgentEvent::AgentDone {
        message: assistant("done"),
    });
    assert!(events.contains(&UiEvent::AssistantDone));
}

#[test]
fn ui_events_apply_to_transcript() {
    let mut transcript = Transcript::new();
    transcript.apply_event(UiEvent::AssistantDelta {
        text: "hel".to_string(),
    });
    transcript.apply_event(UiEvent::AssistantDelta {
        text: "lo".to_string(),
    });
    transcript.apply_event(UiEvent::AssistantDone);

    assert_eq!(
        transcript.items(),
        &[TranscriptItem::Assistant {
            id: "assistant_0".to_string(),
            markdown: "hello".to_string(),
            thinking: String::new(),
            done: true,
        }]
    );
}

#[test]
fn agent_done_emits_usage_update_with_cumulative_totals() {
    let mut bridge = InteractiveEventBridge::new();

    let first = bridge.handle(&AgentEvent::AgentDone {
        message: assistant_done_message(100, 40),
    });
    assert!(first.contains(&UiEvent::AssistantDone));
    assert!(first.contains(&UiEvent::UsageUpdate {
        input: 100,
        output: 40,
        cache_read: 0,
        cache_write: 0,
        cost: 0.0,
        context_tokens: Some(140),
    }));

    let second = bridge.handle(&AgentEvent::AgentDone {
        message: assistant_done_message(250, 60),
    });
    assert!(second.contains(&UiEvent::UsageUpdate {
        input: 350,
        output: 100,
        cache_read: 0,
        cache_write: 0,
        cost: 0.0,
        context_tokens: Some(310),
    }));
}

#[test]
fn coding_event_bridge_maps_assistant_events() {
    let mut bridge = CodingEventBridge::new();

    let delta = bridge.handle(&CodingAgentEvent::AssistantMessageDelta {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_1".to_string()),
        text: "hello".to_string(),
    });
    assert_eq!(
        delta,
        vec![UiEvent::AssistantDelta {
            text: "hello".to_string()
        }]
    );

    let done = bridge.handle(&CodingAgentEvent::AssistantMessageCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_1".to_string()),
        final_text: "hello".to_string(),
    });
    assert_eq!(done, vec![UiEvent::AssistantDone]);
}

#[test]
fn coding_event_bridge_maps_tool_events() {
    let mut bridge = CodingEventBridge::new();

    let start = bridge.handle(&CodingAgentEvent::ToolCallStarted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_1".to_string(),
        name: "read".to_string(),
        arguments_json: r#"{"path":"src/lib.rs"}"#.to_string(),
    });
    assert_eq!(
        start,
        vec![UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        }]
    );

    let update = bridge.handle(&CodingAgentEvent::ToolCallUpdated {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_1".to_string(),
        name: "read".to_string(),
        message: "reading".to_string(),
    });
    assert_eq!(
        update,
        vec![UiEvent::ToolUpdated {
            call_id: "tool_1".to_string(),
            result: "reading".to_string(),
        }]
    );

    let completed = bridge.handle(&CodingAgentEvent::ToolCallCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_1".to_string(),
        name: "read".to_string(),
        summary: "ok".to_string(),
    });
    assert_eq!(
        completed,
        vec![UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "ok".to_string(),
            is_error: false,
        }]
    );

    let failed = bridge.handle(&CodingAgentEvent::ToolCallFailed {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_2".to_string(),
        name: "read".to_string(),
        message: "denied".to_string(),
    });
    assert_eq!(
        failed,
        vec![UiEvent::ToolFinished {
            call_id: "tool_2".to_string(),
            result: "denied".to_string(),
            is_error: true,
        }]
    );
}

#[test]
fn coding_event_bridge_preserves_malformed_tool_arguments() {
    let mut bridge = CodingEventBridge::new();

    let events = bridge.handle(&CodingAgentEvent::ToolCallStarted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_1".to_string(),
        name: "bash".to_string(),
        arguments_json: "{bad json".to_string(),
    });

    assert_eq!(
        events,
        vec![UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "bash".to_string(),
            args: serde_json::Value::String("{bad json".to_string()),
        }]
    );
}

#[test]
fn coding_event_bridge_maps_failure_abort_and_compaction() {
    let mut bridge = CodingEventBridge::new();

    let failed = bridge.handle(&CodingAgentEvent::PromptFailed {
        operation_id: "op_1".to_string(),
        error: CodingSessionError::Provider {
            message: "stream failed".to_string(),
        },
    });
    assert_eq!(
        failed,
        vec![UiEvent::AgentError {
            error: "provider error: stream failed".to_string(),
        }]
    );

    let aborted = bridge.handle(&CodingAgentEvent::PromptAborted {
        operation_id: "op_1".to_string(),
        reason: "user cancelled".to_string(),
    });
    assert_eq!(
        aborted,
        vec![UiEvent::AgentError {
            error: "prompt aborted: user cancelled".to_string(),
        }]
    );

    let compacted = bridge.handle(&CodingAgentEvent::RuntimeCompactionCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        summary: "kept recent context".to_string(),
        first_kept_message_id: "msg_2".to_string(),
        tokens_before: 1200,
    });
    assert_eq!(
        compacted,
        vec![
            UiEvent::CompactionNotice {
                summary: "kept recent context".to_string(),
            },
            UiEvent::UsageUpdate {
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                cost: 0.0,
                context_tokens: None,
            },
        ]
    );
}

#[test]
fn coding_event_bridge_ignores_session_write_and_capability_events() {
    let mut bridge = CodingEventBridge::new();

    let ignored = [
        CodingAgentEvent::SessionWritePending {
            operation_id: "op_1".to_string(),
        },
        CodingAgentEvent::SessionWriteCommitted {
            operation_id: "op_1".to_string(),
            session_id: "session_1".to_string(),
        },
        CodingAgentEvent::SessionWriteSkipped {
            operation_id: "op_1".to_string(),
            reason: "session persistence disabled".to_string(),
        },
        CodingAgentEvent::CapabilityChanged,
    ];

    for event in ignored {
        assert!(bridge.handle(&event).is_empty());
    }
}
