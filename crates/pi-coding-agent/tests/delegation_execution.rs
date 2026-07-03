use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_stream::stream;
use pi_agent_core::AgentResources;
use pi_ai::registry::{self, ApiProvider};
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::api::{
    CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions, PromptInvocation,
    PromptRunOptions, PromptTurnOptions, SessionRunOptions,
};
use tempfile::tempdir;

#[tokio::test]
async fn prompt_executes_approved_agent_delegation_after_parent_success() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

    let api = "delegation-execution-agent-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("child result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "expected parent tool, parent final, and child calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[2].context), vec!["implement parser"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some("Coder child instructions.")
    );

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation request event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation approved event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationStarted { target_id, child_operation_id, .. }
                if target_id.as_str() == "coder" && !child_operation_id.is_empty()
        )),
        "expected delegation started event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "coder" && final_text == "child result"
        )),
        "expected delegation completed event, got {events:#?}"
    );
}

#[tokio::test]
async fn prompt_executes_approved_team_delegation_after_parent_success() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_team = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_teams = ["implementation"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Team member instructions."
"#,
    );
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    );
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

    let api = "delegation-execution-team-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_team",
                "delegate_team",
                serde_json::json!({"team_id": "implementation", "task": "build feature"}),
            ),
            ScriptedResponse::text("parent ready"),
            ScriptedResponse::text("member result"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan team work"))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        3,
        "expected parent tool, parent final, and team member calls"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan team work"]);
    assert_eq!(user_texts(&calls[2].context), vec!["build feature"]);
    assert_eq!(
        calls[2].context.system_prompt.as_deref(),
        Some("Team member instructions.")
    );

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationStarted { target_id, target_kind, child_operation_id, .. }
                if target_id.as_str() == "implementation"
                    && *target_kind == pi_coding_agent::api::ProfileKind::Team
                    && !child_operation_id.is_empty()
        )),
        "expected team delegation started event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationCompleted { target_id, final_text, .. }
                if target_id.as_str() == "implementation"
                    && final_text.contains("Team implementation completed.")
                    && final_text.contains("member result")
        )),
        "expected team delegation completed event, got {events:#?}"
    );
}

#[tokio::test]
async fn prompt_emits_confirmation_required_without_running_child_delegation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/agents/delegating-planner.toml"),
        r#"
schema_version = 1
id = "delegating-planner"
display_name = "Delegating Planner"

[delegation]
allow_delegate_agent = true
max_depth = 1
max_parallel_children = 1
require_confirmation = "always"
allowed_agents = ["coder"]
"#,
    );
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
system_prompt = "Coder child instructions."
"#,
    );
    let _env_guard = EnvGuard::set_pi_rust_dir(global);

    let api = "delegation-confirmation-required-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard = ProviderGuard::register(
        api,
        calls.clone(),
        vec![
            ScriptedResponse::tool_call(
                "tool_delegate_agent",
                "delegate_agent",
                serde_json::json!({"agent_id": "coder", "task": "implement parser"}),
            ),
            ScriptedResponse::text("parent ready"),
        ],
    );

    let mut session = CodingAgentSession::non_persistent(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_default_agent_profile_id("delegating-planner"),
    )
    .await
    .unwrap();
    let mut events = session.subscribe();

    let outcome = session
        .prompt(prompt_options(&cwd, api, "plan feature"))
        .await
        .unwrap();

    assert_eq!(outcome.final_text(), Some("parent ready"));
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "confirmation-required delegation should not run child work"
    );
    assert_eq!(user_texts(&calls[0].context), vec!["plan feature"]);
    assert_eq!(user_texts(&calls[1].context), vec!["plan feature"]);

    let events = drain_events(&mut events);
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested { target_id, task, .. }
                if target_id.as_str() == "coder" && task == "implement parser"
        )),
        "expected delegation request event, got {events:#?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationConfirmationRequired {
                target_id,
                task,
                reason,
                ..
            } if target_id.as_str() == "coder"
                && task == "implement parser"
                && reason == "delegation policy requires confirmation"
        )),
        "expected delegation confirmation-required event, got {events:#?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationApproved { .. }
                | CodingAgentEvent::DelegationStarted { .. }
                | CodingAgentEvent::DelegationCompleted { .. }
        )),
        "confirmation-required delegation must not approve or run child work: {events:#?}"
    );
}

fn prompt_options(cwd: &Path, api: &str, prompt: &str) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(4),
        tools: Vec::new(),
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

trait PromptOutcomeExt {
    fn final_text(&self) -> Option<&str>;
}

impl PromptOutcomeExt for pi_coding_agent::api::PromptTurnOutcome {
    fn final_text(&self) -> Option<&str> {
        match self {
            pi_coding_agent::api::PromptTurnOutcome::Success { final_text, .. } => {
                Some(final_text.as_str())
            }
            _ => None,
        }
    }
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
    context: Context,
}

#[derive(Debug, Clone)]
enum ScriptedResponse {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
}

impl ScriptedResponse {
    fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

struct ScriptedProvider {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    responses: Arc<Mutex<Vec<ScriptedResponse>>>,
}

impl ApiProvider for ScriptedProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.calls
            .lock()
            .unwrap()
            .push(RecordedCall { context: ctx });
        let response = self.responses.lock().unwrap().remove(0);
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("delegation-execution-test", &model_id);
            message.provider = Some("delegation-execution-test".into());
            match response {
                ScriptedResponse::Text(text) => {
                    message.content.push(ContentBlock::Text {
                        text,
                        text_signature: None,
                    });
                    message.stop_reason = StopReason::Stop;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message,
                    };
                }
                ScriptedResponse::ToolCall { id, name, arguments } => {
                    message.content.push(ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        thought_signature: None,
                    });
                    message.stop_reason = StopReason::ToolUse;
                    yield AssistantMessageEvent::Done {
                        reason: StopReason::ToolUse,
                        message,
                    };
                }
            }
        })
    }
}

struct ProviderGuard {
    api: String,
}

impl ProviderGuard {
    fn register(
        api: &str,
        calls: Arc<Mutex<Vec<RecordedCall>>>,
        responses: Vec<ScriptedResponse>,
    ) -> Self {
        registry::register(
            api,
            Arc::new(ScriptedProvider {
                calls,
                responses: Arc::new(Mutex::new(responses)),
            }),
        );
        Self { api: api.into() }
    }
}

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        registry::unregister(&self.api);
    }
}

struct EnvGuard {
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set_pi_rust_dir(path: PathBuf) -> Self {
        let previous = std::env::var_os("PI_RUST_DIR");
        unsafe {
            std::env::set_var("PI_RUST_DIR", path);
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(previous) => std::env::set_var("PI_RUST_DIR", previous),
                None => std::env::remove_var("PI_RUST_DIR"),
            }
        }
    }
}
