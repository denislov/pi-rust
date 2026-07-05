mod support;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_stream::stream;
use pi_agent_core::{AgentResources, AgentTool};
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::api::{
    AgentInvocationOptions, CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions,
    PromptInvocation, PromptRunOptions, PromptTurnOptions, SessionRunOptions,
};
use support::{EnvGuard, ProviderGuard as RegistryProviderGuard};
use tempfile::tempdir;

#[tokio::test]
async fn one_off_agent_invocation_uses_target_profile_runtime_policy() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/runtime-coder.toml"),
        r#"
schema_version = 1
id = "runtime-coder"
display_name = "Runtime Coder"
model = "claude-haiku-4-5"
system_prompt = "Profile invocation instructions."
tools = ["echo"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let profile_model = pi_ai::lookup_model("claude-haiku-4-5").unwrap();
    let fallback_api = "agent-invocation-fallback-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        vec![profile_model.api.clone(), fallback_api.into()],
        calls.clone(),
    );

    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_session_id("sess_agent_invocation")
            .with_session_log_root(temp.path().join("sessions")),
    )
    .await
    .unwrap();
    let mut events = session.subscribe();

    let outcome = session
        .invoke_agent(AgentInvocationOptions::new(
            "runtime-coder",
            "implement the task",
            prompt_options(&cwd, fallback_api, "implement the task"),
        ))
        .await
        .unwrap();

    assert_eq!(outcome.profile_id.as_str(), "runtime-coder");
    assert_eq!(outcome.final_text, "profile applied");
    assert_ne!(outcome.operation_id, outcome.child_operation_id);

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(call.model_id, "claude-haiku-4-5");
    assert_eq!(
        call.context.system_prompt.as_deref(),
        Some("Profile invocation instructions.")
    );
    let tool_names = call
        .context
        .tools
        .as_ref()
        .expect("profile should keep the allowed tool")
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tool_names, vec!["echo"]);

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::AgentInvocationStarted { profile_id, task, .. }
                if profile_id.as_str() == "runtime-coder" && task == "implement the task"
        )),
        "expected invocation start event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::AgentInvocationCompleted { profile_id, final_text, .. }
                if profile_id.as_str() == "runtime-coder" && final_text == "profile applied"
        )),
        "expected invocation completion event, got {events:#?}"
    );
}

#[tokio::test]
async fn one_off_agent_invocation_uses_task_over_prompt_options_invocation() {
    let temp = tempdir().unwrap();
    let api = "agent-invocation-task-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(vec![api.into()], calls.clone());
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(temp.path()))
            .await
            .unwrap();

    session
        .invoke_agent(AgentInvocationOptions::new(
            "default",
            "delegated task",
            prompt_options(temp.path(), api, "parent prompt"),
        ))
        .await
        .unwrap();

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(user_texts(&calls[0].context), vec!["delegated task"]);
}

#[tokio::test]
async fn one_off_agent_invocation_does_not_commit_parent_session_transcript() {
    let temp = tempdir().unwrap();
    let api = "agent-invocation-no-commit-api";
    let _provider_guard =
        ProviderGuard::register(vec![api.into()], Arc::new(Mutex::new(Vec::new())));
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_agent_invocation_no_commit")
            .with_session_log_root(temp.path().join("sessions")),
    )
    .await
    .unwrap();

    let outcome = session
        .invoke_agent(AgentInvocationOptions::new(
            "default",
            "do not persist",
            prompt_options(temp.path(), api, "do not persist"),
        ))
        .await
        .unwrap();

    assert_eq!(outcome.final_text, "profile applied");
    let export = session.export_current().unwrap();
    assert!(
        export.transcript.is_empty(),
        "one-off agent invocation must not write parent transcript: {export:#?}"
    );
}

#[tokio::test]
async fn one_off_agent_invocation_emits_single_failed_event_for_child_failure() {
    let temp = tempdir().unwrap();
    let api = "agent-invocation-child-failure-api";
    let _provider_guard = ProviderGuard::register_failing(vec![api.into()]);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(temp.path()))
            .await
            .unwrap();
    let mut events = session.subscribe();

    let error = session
        .invoke_agent(AgentInvocationOptions::new(
            "default",
            "fail child",
            prompt_options(temp.path(), api, "fail child"),
        ))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("child failed"), "{error}");
    let events = drain_events(&mut events);
    let failure_count = events
        .iter()
        .filter(|event| matches!(event, CodingAgentEvent::AgentInvocationFailed { .. }))
        .count();
    assert_eq!(
        failure_count, 1,
        "expected one invocation failure event: {events:#?}"
    );
}

#[tokio::test]
async fn one_off_agent_invocation_rejects_unknown_profile_with_product_event() {
    let temp = tempdir().unwrap();
    let api = "agent-invocation-missing-profile-api";
    let _provider_guard =
        ProviderGuard::register(vec![api.into()], Arc::new(Mutex::new(Vec::new())));
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(temp.path()))
            .await
            .unwrap();
    let mut events = session.subscribe();

    let error = session
        .invoke_agent(AgentInvocationOptions::new(
            "missing",
            "task",
            prompt_options(temp.path(), api, "task"),
        ))
        .await
        .unwrap_err();

    assert!(
        error.to_string().contains("Unknown agent profile: missing"),
        "{error}"
    );
    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::AgentInvocationFailed { profile_id, error, .. }
                if profile_id.as_str() == "missing"
                    && error.to_string().contains("Unknown agent profile: missing")
        )),
        "expected invocation failure event, got {events:#?}"
    );
}

fn prompt_options(cwd: &Path, api: &str, prompt: &str) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(2),
        tools: vec![echo_tool(), extra_tool()],
        register_builtins: false,
        session: Some(SessionRunOptions::disabled(cwd.to_path_buf())),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text(prompt.into()),
    })
}

fn fallback_model(api: &str) -> Model {
    Model {
        id: "fallback-model".into(),
        name: "Fallback Model".into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

fn echo_tool() -> AgentTool {
    AgentTool::new_text(
        "echo",
        "echoes input",
        serde_json::json!({"type": "object"}),
        |_args| async { Ok("echo".to_owned()) },
    )
}

fn extra_tool() -> AgentTool {
    AgentTool::new_text(
        "extra",
        "extra tool",
        serde_json::json!({"type": "object"}),
        |_args| async { Ok("extra".to_owned()) },
    )
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn drain_events(
    receiver: &mut pi_coding_agent::api::CodingAgentEventReceiver,
) -> Vec<CodingAgentEvent> {
    let mut events = Vec::new();
    while let Ok(Some(event)) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn user_texts(context: &Context) -> Vec<String> {
    context
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { content } => Some(
                content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct RecordedCall {
    model_id: String,
    context: Context,
}

struct RecordingProvider {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

struct FailingProvider;

impl ApiProvider for FailingProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("agent-invocation-failure", &model_id);
            message.error_message = Some("child failed".into());
            message.stop_reason = StopReason::Error;
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                message,
            };
        })
    }
}

impl ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.calls.lock().unwrap().push(RecordedCall {
            model_id: model.id.clone(),
            context: ctx,
        });
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("agent-invocation-test", &model_id);
            message.provider = Some("agent-invocation-test".into());
            message.content.push(ContentBlock::Text {
                text: "profile applied".into(),
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

struct ProviderGuard {
    _guard: RegistryProviderGuard<'static>,
}

impl ProviderGuard {
    fn register(apis: Vec<String>, calls: Arc<Mutex<Vec<RecordedCall>>>) -> Self {
        let providers = apis
            .into_iter()
            .map(|api| {
                (
                    api,
                    Arc::new(RecordingProvider {
                        calls: calls.clone(),
                    }) as Arc<dyn ApiProvider>,
                )
            })
            .collect();
        Self {
            _guard: RegistryProviderGuard::register_many(providers),
        }
    }

    fn register_failing(apis: Vec<String>) -> Self {
        let providers = apis
            .into_iter()
            .map(|api| (api, Arc::new(FailingProvider) as Arc<dyn ApiProvider>))
            .collect();
        Self {
            _guard: RegistryProviderGuard::register_many(providers),
        }
    }
}
