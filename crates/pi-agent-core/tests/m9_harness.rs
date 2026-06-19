use futures::StreamExt;
use pi_agent_core::{
    AgentConfig, AgentEvent, AgentHarness, AgentHarnessEvent, AgentHarnessHooks, AgentMessage,
    BeforeProviderRequest, ExecutionError, ExecutionOutput, FileErrorCode, FileKind, FileSystem,
    InMemoryExecutionEnv, Shell,
};
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, ModelCost, ModelInput,
    StopReason, StreamOptions,
};
use std::sync::{Arc, Mutex};

fn faux_model(api: &str) -> Model {
    Model {
        id: "m9-faux-model".into(),
        name: "M9 Faux".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: "http://localhost".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 10_000,
        max_tokens: 1_000,
        headers: None,
        compat: None,
    }
}

#[test]
fn custom_messages_convert_to_context_and_session_wire_shape() {
    let messages = vec![
        AgentMessage::BashExecution {
            message_id: "bash_1".into(),
            command: "cargo test".into(),
            output: "ok".into(),
            exit_code: Some(0),
            cancelled: false,
            truncated: true,
            full_output_path: Some("/tmp/full.log".into()),
            exclude_from_context: false,
            timestamp: 123,
        },
        AgentMessage::Custom {
            message_id: "custom_1".into(),
            custom_type: "note".into(),
            content: vec![ContentBlock::Text {
                text: "remember this".into(),
                text_signature: None,
            }],
            display: true,
            details: Some(serde_json::json!({"source": "test"})),
            timestamp: 124,
        },
        AgentMessage::BranchSummary {
            message_id: "branch_1".into(),
            summary: "branch result".into(),
            from_id: "entry_7".into(),
            timestamp: 125,
        },
    ];

    let ctx =
        pi_agent_core::convert::convert_to_context(&None, &messages, &[], &Default::default());
    assert_eq!(ctx.messages.len(), 3);
    let text = match &ctx.messages[0] {
        pi_ai::types::Message::User { content } => match &content[0] {
            ContentBlock::Text { text, .. } => text,
            _ => panic!("expected text"),
        },
        _ => panic!("expected user message"),
    };
    assert!(text.contains("Ran `cargo test`"));
    assert!(text.contains("Output truncated"));

    let stored = pi_agent_core::session::agent_message_to_stored(&messages[0], 999).unwrap();
    let json = serde_json::to_value(stored).unwrap();
    assert_eq!(json["role"], "bashExecution");
    assert_eq!(json["command"], "cargo test");
    assert_eq!(json["timestamp"], 123);

    let stored = pi_agent_core::session::agent_message_to_stored(&messages[1], 999).unwrap();
    let json = serde_json::to_value(stored).unwrap();
    assert_eq!(json["role"], "custom");
    assert_eq!(json["customType"], "note");

    let stored = pi_agent_core::session::agent_message_to_stored(&messages[2], 999).unwrap();
    let json = serde_json::to_value(stored).unwrap();
    assert_eq!(json["role"], "branchSummary");
    assert_eq!(json["fromId"], "entry_7");
}

#[tokio::test]
async fn in_memory_execution_env_supports_file_and_shell_traits() {
    let env = InMemoryExecutionEnv::new("/workspace");
    env.write_file("/workspace/src/main.rs", b"fn main() {}\n")
        .await
        .unwrap();
    env.append_file("/workspace/src/main.rs", b"// done\n")
        .await
        .unwrap();

    assert!(env.exists("/workspace/src/main.rs").await.unwrap());
    assert_eq!(
        env.read_text_file("/workspace/src/main.rs").await.unwrap(),
        "fn main() {}\n// done\n"
    );
    assert_eq!(
        env.read_text_lines("/workspace/src/main.rs", Some(1))
            .await
            .unwrap(),
        vec!["fn main() {}".to_string()]
    );

    let entries = env.list_dir("/workspace/src").await.unwrap();
    assert_eq!(entries[0].name, "main.rs");
    assert_eq!(entries[0].kind, FileKind::File);

    env.set_command(
        "cargo test",
        ExecutionOutput {
            stdout: "ok".into(),
            stderr: String::new(),
            exit_code: 0,
        },
    );
    let output = env.exec("cargo test", None).await.unwrap();
    assert_eq!(output.stdout, "ok");

    let err = env.read_text_file("/workspace/missing").await.unwrap_err();
    assert_eq!(err.code(), FileErrorCode::NotFound);
    assert!(matches!(
        env.exec("missing", None).await.unwrap_err(),
        ExecutionError::ShellUnavailable { .. }
    ));
}

#[tokio::test]
async fn agent_harness_emits_events_and_hooks_patch_start_messages() {
    let api = "m9-harness-faux";
    registry::unregister(api);
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec!["hello".into()],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Stop,
        }])),
    );

    let mut config = AgentConfig::new(faux_model(api));
    config.max_turns = 1;

    let seen_context = Arc::new(Mutex::new(false));
    let seen_context_hook = seen_context.clone();
    let hooks = AgentHarnessHooks {
        before_agent_start: Some(Arc::new(move |mut ctx| {
            ctx.messages.push(AgentMessage::UserText {
                message_id: "hook_user".into(),
                text: "from hook".into(),
            });
            Box::pin(async move { Ok(Some(ctx)) })
        })),
        context: Some(Arc::new(move |ctx| {
            *seen_context_hook.lock().unwrap() = ctx.messages.iter().any(
                |msg| matches!(msg, AgentMessage::UserText { text, .. } if text == "from hook"),
            );
            Box::pin(async move { Ok(None) })
        })),
        ..Default::default()
    };

    let harness = AgentHarness::new(config).with_hooks(hooks);
    let events = harness.prompt("start").collect::<Vec<_>>().await;

    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentHarnessEvent::BeforeAgentStart { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentHarnessEvent::Context { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentHarnessEvent::BeforeProviderRequest { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        AgentHarnessEvent::Agent(AgentEvent::AgentDone { .. })
    )));
    assert!(*seen_context.lock().unwrap());

    let messages = harness.messages();
    assert!(
        messages
            .iter()
            .any(|msg| matches!(msg, AgentMessage::UserText { text, .. } if text == "from hook"))
    );
}

#[derive(Default)]
struct CapturedProviderRequest {
    context: Option<Context>,
    stream_options: Option<StreamOptions>,
}

struct RecordingProvider {
    captured: Arc<Mutex<CapturedProviderRequest>>,
}

impl ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        {
            let mut captured = self.captured.lock().unwrap();
            captured.context = Some(ctx);
            captured.stream_options = opts;
        }

        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut msg = AssistantMessage::empty("recording", &model_id);
            msg.provider = Some("recording".into());
            msg.content.push(ContentBlock::Text {
                text: "ok".into(),
                text_signature: None,
            });
            msg.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: msg,
            };
        })
    }
}

#[tokio::test]
async fn before_provider_request_hook_patches_actual_provider_request() {
    let api = "m9-harness-recording";
    registry::unregister(api);
    let captured = Arc::new(Mutex::new(CapturedProviderRequest::default()));
    registry::register(
        api,
        Arc::new(RecordingProvider {
            captured: captured.clone(),
        }),
    );

    let mut config = AgentConfig::new(faux_model(api));
    config.max_turns = 1;
    config.stream_options = Some(StreamOptions {
        temperature: Some(0.2),
        ..Default::default()
    });

    let hooks = AgentHarnessHooks {
        before_provider_request: Some(Arc::new(|mut request: BeforeProviderRequest| {
            assert_eq!(
                request
                    .stream_options
                    .as_ref()
                    .and_then(|options| options.temperature),
                Some(0.2)
            );
            request.context.system_prompt = Some("patched system".into());
            request.stream_options = Some(StreamOptions {
                temperature: Some(0.7),
                api_key: Some("hook-key".into()),
                headers: Some(serde_json::json!({"x-hook": "yes"})),
                ..Default::default()
            });
            Box::pin(async move { Ok(Some(request)) })
        })),
        ..Default::default()
    };

    let harness = AgentHarness::new(config).with_hooks(hooks);
    let events = harness.prompt("start").collect::<Vec<_>>().await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentHarnessEvent::BeforeProviderRequest { .. }))
    );

    let captured = captured.lock().unwrap();
    let context = captured.context.as_ref().expect("provider context");
    assert_eq!(context.system_prompt.as_deref(), Some("patched system"));

    let opts = captured
        .stream_options
        .as_ref()
        .expect("provider stream options");
    assert_eq!(opts.temperature, Some(0.7));
    assert_eq!(opts.api_key.as_deref(), Some("hook-key"));
    assert_eq!(
        opts.headers
            .as_ref()
            .and_then(|headers| headers.get("x-hook")),
        Some(&serde_json::json!("yes"))
    );
}
