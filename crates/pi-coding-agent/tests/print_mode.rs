use pi_agent_core::{AgentTool, AgentToolOutput};
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::{
    CliError, PrintModeOptions, PromptInvocation, ResolvedSessionTarget, SessionMode,
    SessionRunOptions, run_print_mode,
};
use std::sync::{Arc, Mutex};

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

fn echo_tool() -> AgentTool {
    AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        execution_mode: None,
        execute: Arc::new(|args, _on_update| {
            let text = args
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {text}"),
                text_signature: None,
            }];
            Box::pin(async move { Ok(AgentToolOutput::new(result)) })
        }),
    }
}

struct RecordingProvider {
    contexts: Arc<Mutex<Vec<Context>>>,
    response: String,
}

impl RecordingProvider {
    fn new(contexts: Arc<Mutex<Vec<Context>>>, response: impl Into<String>) -> Self {
        Self {
            contexts,
            response: response.into(),
        }
    }
}

impl ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.contexts.lock().unwrap().push(ctx);
        let model_id = model.id.clone();
        let response = self.response.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("recording", &model_id);
            message.provider = Some("recording".into());
            message.content.push(ContentBlock::Text {
                text: response,
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

#[tokio::test]
async fn prints_single_turn_text_response() {
    let api = "pi-coding-print-text";
    registry::register(
        api,
        Arc::new(FauxProvider::new(vec![text_response("Hello")])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Hello");
    registry::unregister(api);
}

#[tokio::test]
async fn disabled_session_print_uses_non_persistent_runtime_without_session_files() {
    let api = "pi-coding-print-disabled-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("No files")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Disabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "No files");
    assert!(!sessions_dir.exists());
    registry::unregister(api);
}

#[tokio::test]
async fn treats_length_as_successful_final_text() {
    let api = "pi-coding-print-length";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![text_response("Partial final text")],
            stop_reason: StopReason::Length,
        }])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Partial final text");
    registry::unregister(api);
}

#[tokio::test]
async fn returns_agent_failure_on_error_stop_reason() {
    let api = "pi-coding-print-error";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Error,
        }])),
    );

    let error = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap_err();

    assert_eq!(error, CliError::AgentFailure("LLM error".into()));
    registry::unregister(api);
}

#[tokio::test]
async fn supports_tool_call_loop_with_injected_tool() {
    let api = "pi-coding-print-tool-loop";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_1".into(),
                        name: "echo".into(),
                        deltas: vec!["{\"text\":".into(), "\"hi\"}".into()],
                        final_arguments: serde_json::json!({"text": "hi"}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("Tool completed")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "echo hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: vec![echo_tool()],
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("echo hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Tool completed");
    registry::unregister(api);
}

#[tokio::test]
async fn explicit_new_session_writes_rust_native_session_events() {
    let api = "pi-coding-print-rust-native-new-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("Generated")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::New),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Generated");

    let session_dirs = std::fs::read_dir(&sessions_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    assert_eq!(session_dirs.len(), 1);
    assert!(session_dirs[0].join("session.json").is_file());
    let events = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(events.contains(r#""kind":"session.created""#));
    assert!(events.contains(r#""kind":"operation.committed""#));
    assert!(events.contains(r#""kind":"message.completed""#));
    assert!(!events.contains(r#""type":"session""#));
    registry::unregister(api);
}

#[tokio::test]
async fn open_or_create_session_target_reopens_rust_native_session() {
    let first_api = "pi-coding-print-open-or-create-first";
    registry::register(first_api, Arc::new(FauxProvider::simple_text("first")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    let first = run_print_mode(PrintModeOptions {
        prompt: "first question".into(),
        model: faux_model(first_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir.clone(),
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::OpenOrCreateId("shared".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("first question".into()),
    })
    .await
    .unwrap();
    assert_eq!(first, "first");
    registry::unregister(first_api);

    let second_api = "pi-coding-print-open-or-create-second";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        second_api,
        Arc::new(RecordingProvider::new(Arc::clone(&contexts), "second")),
    );

    let second = run_print_mode(PrintModeOptions {
        prompt: "second question".into(),
        model: faux_model(second_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::OpenOrCreateId("shared".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("second question".into()),
    })
    .await
    .unwrap();

    assert_eq!(second, "second");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].messages.len(), 3);
    assert!(matches!(
        &contexts[0].messages[0],
        Message::User { content }
            if content == &vec![ContentBlock::Text {
                text: "first question".into(),
                text_signature: None,
            }]
    ));
    assert!(matches!(
        &contexts[0].messages[1],
        Message::Assistant { content }
            if content == &vec![ContentBlock::Text {
                text: "first".into(),
                text_signature: None,
            }]
    ));
    assert!(matches!(
        &contexts[0].messages[2],
        Message::User { content }
            if content == &vec![ContentBlock::Text {
                text: "second question".into(),
                text_signature: None,
            }]
    ));
    assert!(sessions_dir.join("shared").join("session.json").is_file());
    registry::unregister(second_api);
}

#[tokio::test]
async fn open_target_reuses_existing_rust_native_session() {
    let first_api = "pi-coding-print-open-target-first";
    registry::register(first_api, Arc::new(FauxProvider::simple_text("stored")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    run_print_mode(PrintModeOptions {
        prompt: "remember".into(),
        model: faux_model(first_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir.clone(),
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::OpenOrCreateId("existing".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("remember".into()),
    })
    .await
    .unwrap();
    registry::unregister(first_api);

    let second_api = "pi-coding-print-open-target-second";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        second_api,
        Arc::new(RecordingProvider::new(Arc::clone(&contexts), "opened")),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "continue".into(),
        model: faux_model(second_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir),
        }),
        session_target: Some(ResolvedSessionTarget::OpenTarget("existing".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("continue".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "opened");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts[0].messages.len(), 3);
    registry::unregister(second_api);
}

#[tokio::test]
async fn continue_most_recent_uses_rust_native_session() {
    let first_api = "pi-coding-print-continue-first";
    registry::register(first_api, Arc::new(FauxProvider::simple_text("prior")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    run_print_mode(PrintModeOptions {
        prompt: "prior question".into(),
        model: faux_model(first_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir.clone(),
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::OpenOrCreateId("recent".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("prior question".into()),
    })
    .await
    .unwrap();
    registry::unregister(first_api);

    let second_api = "pi-coding-print-continue-second";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        second_api,
        Arc::new(RecordingProvider::new(Arc::clone(&contexts), "continued")),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "next".into(),
        model: faux_model(second_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir),
        }),
        session_target: Some(ResolvedSessionTarget::ContinueMostRecent),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("next".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "continued");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts[0].messages.len(), 3);
    registry::unregister(second_api);
}

#[tokio::test]
async fn continue_most_recent_reports_missing_rust_native_session() {
    let api = "pi-coding-print-continue-missing";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    let error = run_print_mode(PrintModeOptions {
        prompt: "next".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir),
        }),
        session_target: Some(ResolvedSessionTarget::ContinueMostRecent),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("next".into()),
    })
    .await
    .unwrap_err();

    assert_eq!(
        error,
        CliError::SessionFailure("no previous session to continue".into())
    );
    registry::unregister(api);
}

#[tokio::test]
async fn fork_target_routes_through_rust_native_session() {
    let first_api = "pi-coding-print-fork-source";
    registry::register(
        first_api,
        Arc::new(FauxProvider::simple_text("source answer")),
    );
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("project");
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();

    let first = run_print_mode(PrintModeOptions {
        prompt: "source question".into(),
        model: faux_model(first_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir.clone(),
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::OpenOrCreateId("source".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("source question".into()),
    })
    .await
    .unwrap();
    assert_eq!(first, "source answer");
    registry::unregister(first_api);

    let second_api = "pi-coding-print-fork-followup";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    registry::register(
        second_api,
        Arc::new(RecordingProvider::new(Arc::clone(&contexts), "fork answer")),
    );

    let second = run_print_mode(PrintModeOptions {
        prompt: "fork question".into(),
        model: faux_model(second_api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::ForkTarget("source".into())),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("fork question".into()),
    })
    .await
    .unwrap();

    assert_eq!(second, "fork answer");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].messages.len(), 3);
    assert!(matches!(
        &contexts[0].messages[0],
        Message::User { content }
            if content == &vec![ContentBlock::Text {
                text: "source question".into(),
                text_signature: None,
            }]
    ));
    assert!(matches!(
        &contexts[0].messages[1],
        Message::Assistant { content }
            if content == &vec![ContentBlock::Text {
                text: "source answer".into(),
                text_signature: None,
            }]
    ));
    assert!(matches!(
        &contexts[0].messages[2],
        Message::User { content }
            if content == &vec![ContentBlock::Text {
                text: "fork question".into(),
                text_signature: None,
            }]
    ));

    let fork_session_dirs = std::fs::read_dir(&sessions_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.is_dir() && path.file_name().and_then(|name| name.to_str()) != Some("source")
        })
        .collect::<Vec<_>>();
    assert_eq!(fork_session_dirs.len(), 1);
    let fork_events = std::fs::read_to_string(fork_session_dirs[0].join("events.jsonl")).unwrap();
    assert!(fork_events.contains(r#""kind":"session.forked""#));
    assert!(fork_events.contains("source question"));
    assert!(fork_events.contains("fork question"));
    let source_events = std::fs::read_to_string(sessions_dir.join("source/events.jsonl")).unwrap();
    assert!(!source_events.contains("fork question"));
    registry::unregister(second_api);
}
