use pi_ai::types::{Cost, Usage};
use pi_coding_agent::api::{
    CapabilityRevocationPolicy, CodingAgentEvent, CodingSessionError, ProfileKind,
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use pi_coding_agent::interactive::{CodingEventBridge, Transcript, TranscriptItem, UiEvent};

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
fn system_notice_ui_event_applies_to_transcript() {
    let mut transcript = Transcript::new();
    transcript.apply_event(UiEvent::SystemNotice {
        text: "Delegation pending".to_string(),
    });

    assert_eq!(
        transcript.items(),
        &[TranscriptItem::System {
            text: "Delegation pending".to_string(),
        }]
    );
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
    let thinking = bridge.handle(&CodingAgentEvent::AssistantThinkingDelta {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_1".to_string()),
        text: "thinking".to_string(),
    });
    assert_eq!(
        thinking,
        vec![UiEvent::ThinkingDelta {
            text: "thinking".to_string()
        }]
    );

    let done = bridge.handle(&CodingAgentEvent::AssistantMessageCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_1".to_string()),
        final_text: "hello".to_string(),
        usage: Usage {
            input: 100,
            output: 50,
            cache_read: 0,
            cache_write: 0,
            total_tokens: 150,
            cost: Cost {
                input: 0.125,
                output: 0.125,
                cache_read: 0.0,
                cache_write: 0.0,
            },
        },
    });
    assert_eq!(
        done,
        vec![
            UiEvent::AssistantDone,
            UiEvent::UsageUpdate {
                input: 100,
                output: 50,
                cache_read: 0,
                cache_write: 0,
                cost: 0.25,
                context_tokens: Some(150),
            },
        ]
    );

    // A second assistant message is a separate delta; the bridge no longer
    // accumulates (the receiver does). Each UsageUpdate carries per-event
    // usage only; context_tokens reflects the latest message (mirrors TS
    // getContextUsage using the most recent usage).
    let done2 = bridge.handle(&CodingAgentEvent::AssistantMessageCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_2".to_string()),
        final_text: "world".to_string(),
        usage: Usage {
            input: 30,
            output: 20,
            cache_read: 5,
            cache_write: 0,
            total_tokens: 55,
            cost: Cost {
                input: 0.0625,
                output: 0.0625,
                cache_read: 0.0,
                cache_write: 0.0,
            },
        },
    });
    assert_eq!(
        done2,
        vec![
            UiEvent::AssistantDone,
            UiEvent::UsageUpdate {
                input: 30,
                output: 20,
                cache_read: 5,
                cache_write: 0,
                cost: 0.125,
                context_tokens: Some(55),
            },
        ]
    );
}

#[test]
fn coding_event_bridge_marks_zero_usage_context_unknown() {
    let mut bridge = CodingEventBridge::new();

    let done = bridge.handle(&CodingAgentEvent::AssistantMessageCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        message_id: Some("msg_1".to_string()),
        final_text: "hello".to_string(),
        usage: Usage::default(),
    });

    assert_eq!(
        done,
        vec![
            UiEvent::AssistantDone,
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

    let compacted = bridge.handle(&CodingAgentEvent::SessionCompactionCompleted {
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
fn coding_event_bridge_maps_delegation_confirmation_events() {
    let mut bridge = CodingEventBridge::new();

    let events = bridge.handle(&CodingAgentEvent::DelegationConfirmationRequired {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_delegate_agent".to_string(),
        requesting_profile_id: "planner".into(),
        target_kind: ProfileKind::Agent,
        target_id: "coder".into(),
        task: "implement parser".to_string(),
        reason: "profile policy requires confirmation".to_string(),
    });

    let [
        UiEvent::DelegationBlock {
            call_id,
            target_kind,
            target_id,
            task,
            status,
            summary,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("expected one delegation block, got {events:?}");
    };
    assert_eq!(call_id, "tool_delegate_agent");
    assert_eq!(target_kind, "agent");
    assert_eq!(target_id, "coder");
    assert_eq!(task, "implement parser");
    assert_eq!(status, "confirmation_required");
    let text = summary.as_deref().expect("confirmation summary");
    assert!(text.contains("confirmation required"), "{text}");
    assert!(
        text.contains("/delegation approve op_1 tool_delegate_agent"),
        "{text}"
    );
    assert!(
        text.contains("/delegation reject op_1 tool_delegate_agent"),
        "{text}"
    );

    let completed = bridge.handle(&CodingAgentEvent::DelegationCompleted {
        operation_id: "op_1".to_string(),
        turn_id: "turn_1".to_string(),
        tool_call_id: "tool_delegate_agent".to_string(),
        requesting_profile_id: "planner".into(),
        target_kind: ProfileKind::Agent,
        target_id: "coder".into(),
        task: "implement parser".to_string(),
        child_operation_id: "op_child".to_string(),
        final_text: "child result".to_string(),
    });

    let [
        UiEvent::DelegationBlock {
            call_id,
            target_kind,
            target_id,
            status,
            child_operation_id,
            summary,
            is_error,
            ..
        },
    ] = completed.as_slice()
    else {
        panic!("expected one delegation block, got {completed:?}");
    };
    assert_eq!(call_id, "tool_delegate_agent");
    assert_eq!(target_kind, "agent");
    assert_eq!(target_id, "coder");
    assert_eq!(status, "completed");
    assert_eq!(child_operation_id.as_deref(), Some("op_child"));
    assert_eq!(summary.as_deref(), Some("completed: child result"));
    assert!(!is_error);
}

#[test]
fn coding_event_bridge_folds_delegation_lifecycle_into_one_transcript_item() {
    let mut bridge = CodingEventBridge::new();
    let mut transcript = Transcript::new();

    for event in [
        CodingAgentEvent::DelegationStarted {
            operation_id: "op_1".to_string(),
            turn_id: "turn_1".to_string(),
            tool_call_id: "tool_delegate_agent".to_string(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".to_string(),
            child_operation_id: "op_child".to_string(),
        },
        CodingAgentEvent::DelegationCompleted {
            operation_id: "op_1".to_string(),
            turn_id: "turn_1".to_string(),
            tool_call_id: "tool_delegate_agent".to_string(),
            requesting_profile_id: "planner".into(),
            target_kind: ProfileKind::Agent,
            target_id: "coder".into(),
            task: "implement parser".to_string(),
            child_operation_id: "op_child".to_string(),
            final_text: "child result".to_string(),
        },
    ] {
        for ui_event in bridge.handle(&event) {
            transcript.apply_event(ui_event);
        }
    }

    assert_eq!(
        transcript.items(),
        &[TranscriptItem::Tool {
            call_id: "tool_delegate_agent".to_string(),
            name: "delegation".to_string(),
            args: serde_json::json!({
                "targetKind": "agent",
                "targetId": "coder",
                "task": "implement parser",
                "status": "completed",
                "childOperationId": "op_child"
            }),
            result: Some("completed: child result".to_string()),
            is_error: false,
        }]
    );
}

#[test]
fn coding_event_bridge_maps_self_healing_edit_events() {
    let mut bridge = CodingEventBridge::new();

    let started = bridge.handle(&CodingAgentEvent::SelfHealingEditStarted {
        operation_id: "op_edit".to_string(),
        path: "src/app.txt".to_string(),
        replacements: 1,
    });
    let [UiEvent::SystemNotice { text }] = started.as_slice() else {
        panic!("expected one system notice, got {started:?}");
    };
    assert!(text.contains("Self-healing edit started"), "{text}");
    assert!(text.contains("src/app.txt"), "{text}");

    let repair = bridge.handle(&CodingAgentEvent::SelfHealingEditRepairAttempted {
        operation_id: "op_edit".to_string(),
        path: "src/app.txt".to_string(),
        attempt: 1,
        replacements: vec![SelfHealingEditReplacement::new("deux", "dos")],
        diagnostics: vec![SelfHealingEditDiagnostic {
            message: "compile error".to_string(),
        }],
        check_output: Some(SelfHealingEditCheckOutput {
            command: "cargo check".to_string(),
            stdout: "fixed".to_string(),
            stderr: String::new(),
            exit_code: 0,
        }),
    });
    let [UiEvent::SystemNotice { text }] = repair.as_slice() else {
        panic!("expected one system notice, got {repair:?}");
    };
    assert!(text.contains("repair attempt 1"), "{text}");
    assert!(text.contains("src/app.txt"), "{text}");
    assert!(text.contains("exit 0"), "{text}");

    let completed = bridge.handle(&CodingAgentEvent::SelfHealingEditCompleted {
        operation_id: "op_edit".to_string(),
        path: "src/app.txt".to_string(),
        attempts: 2,
        first_changed_line: Some(2),
        check_output: None,
    });
    let [UiEvent::SystemNotice { text }] = completed.as_slice() else {
        panic!("expected one system notice, got {completed:?}");
    };
    assert!(text.contains("Self-healing edit completed"), "{text}");
    assert!(text.contains("2 attempts"), "{text}");

    let failed = bridge.handle(&CodingAgentEvent::SelfHealingEditFailed {
        operation_id: "op_edit_failed".to_string(),
        path: "src/bad.txt".to_string(),
        error: CodingSessionError::Input {
            message: "bad edit".to_string(),
        },
    });
    let [UiEvent::SystemNotice { text }] = failed.as_slice() else {
        panic!("expected one system notice, got {failed:?}");
    };
    assert!(text.contains("Self-healing edit failed"), "{text}");
    assert!(text.contains("invalid input: bad edit"), "{text}");
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
        CodingAgentEvent::CapabilityChanged {
            generation: 1,
            revocation: CapabilityRevocationPolicy::FutureOnly,
        },
    ];

    for event in ignored {
        assert!(bridge.handle(&event).is_empty());
    }
}
