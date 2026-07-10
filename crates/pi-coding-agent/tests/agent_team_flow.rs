mod support;

use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_stream::stream;
use pi_agent_core::AgentResources;
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::api::{
    AgentTeamOptions, CodingAgentProductEvent, CodingAgentProductEventReceiver, CodingAgentSession,
    CodingAgentSessionOptions, PromptInvocation, PromptRunOptions, PromptTurnOptions,
    SessionRunOptions,
};
use support::{EnvGuard, ProviderGuard as RegistryProviderGuard};
use tempfile::tempdir;

#[tokio::test]
async fn team_invocation_runs_members_with_isolated_child_operations() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_agent(&cwd, "coder", "Coder", Some("Coder instructions."));
    write_agent(&cwd, "reviewer", "Reviewer", Some("Reviewer instructions."));
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder", "reviewer"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "agent-team-deterministic-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard =
        ProviderGuard::register(api, calls.clone(), vec!["coder result", "reviewer result"]);
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_cwd(&cwd)
            .with_session_id("sess_agent_team")
            .with_session_log_root(temp.path().join("sessions")),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    let outcome = session
        .invoke_team(AgentTeamOptions::new(
            "implementation",
            "ship the feature",
            prompt_options(&cwd, api, "ship the feature"),
        ))
        .await
        .unwrap();

    assert_eq!(outcome.team_id.as_str(), "implementation");
    assert_eq!(outcome.member_results.len(), 2);
    assert_eq!(outcome.member_results[0].profile_id.as_str(), "coder");
    assert_eq!(outcome.member_results[1].profile_id.as_str(), "reviewer");
    assert!(outcome.final_text.contains("coder result"));
    assert!(outcome.final_text.contains("reviewer result"));
    assert!(outcome.supervisor_result.is_none());
    assert_ne!(outcome.operation_id, outcome.member_results[0].operation_id);
    assert_ne!(
        outcome.member_results[0].operation_id,
        outcome.member_results[1].operation_id
    );

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].system_prompt.as_deref(),
        Some("Coder instructions.")
    );
    assert_eq!(
        calls[1].system_prompt.as_deref(),
        Some("Reviewer instructions.")
    );

    let export = session.export_current().unwrap();
    assert!(
        export.transcript.is_empty(),
        "team child work must not write parent transcript: {export:#?}"
    );

    let events = drain_events(&mut events);
    assert!(has_event(&events, "Team(Started)"));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "Team(MemberCompleted)")
            .count(),
        2,
        "expected two member completion events: {events:#?}"
    );
    assert!(has_event(&events, "Team(Completed)"));
}

#[tokio::test]
async fn team_invocation_runs_profile_backed_supervisor_after_members() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_agent(&cwd, "coder", "Coder", None);
    write_agent(&cwd, "lead", "Lead", Some("Lead instructions."));
    write_file(
        cwd.join(".pi-rust/teams/supervised.toml"),
        r#"
schema_version = 1
id = "supervised"
display_name = "Supervised Team"
supervisor = "lead"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "agent-team-supervisor-api";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let _provider_guard =
        ProviderGuard::register(api, calls.clone(), vec!["coder draft", "lead final"]);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&cwd))
            .await
            .unwrap();

    let outcome = session
        .invoke_team(AgentTeamOptions::new(
            "supervised",
            "finish the plan",
            prompt_options(&cwd, api, "finish the plan"),
        ))
        .await
        .unwrap();

    assert_eq!(outcome.final_text, "lead final");
    let supervisor = outcome
        .supervisor_result
        .as_ref()
        .expect("profile-backed supervisor should produce a result");
    assert_eq!(supervisor.profile_id.as_str(), "lead");

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[1].system_prompt.as_deref(),
        Some("Lead instructions.")
    );
    assert!(
        calls[1]
            .user_texts
            .iter()
            .any(|text| text.contains("coder draft")),
        "supervisor prompt should include member result: {calls:#?}"
    );
}

#[tokio::test]
async fn team_invocation_rejects_unknown_member_with_product_event() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_file(
        cwd.join(".pi-rust/teams/broken.toml"),
        r#"
schema_version = 1
id = "broken"
display_name = "Broken Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["missing"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "agent-team-missing-member-api";
    let _provider_guard = ProviderGuard::register(api, Arc::new(Mutex::new(Vec::new())), vec![]);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&cwd))
            .await
            .unwrap();
    let mut events = session.subscribe_product_events_public();

    let error = session
        .invoke_team(AgentTeamOptions::new(
            "broken",
            "task",
            prompt_options(&cwd, api, "task"),
        ))
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Unknown team member agent profile: missing"),
        "{error}"
    );
    let events = drain_events(&mut events);
    assert!(has_event(&events, "Team(Failed)"));
}

#[tokio::test]
async fn team_invocation_rejects_unknown_supervisor_with_product_event() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_agent(&cwd, "coder", "Coder", None);
    write_file(
        cwd.join(".pi-rust/teams/broken-supervisor.toml"),
        r#"
schema_version = 1
id = "broken-supervisor"
display_name = "Broken Supervisor Team"
supervisor = "missing-lead"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "agent-team-missing-supervisor-api";
    let _provider_guard = ProviderGuard::register(api, Arc::new(Mutex::new(Vec::new())), vec![]);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&cwd))
            .await
            .unwrap();
    let mut events = session.subscribe_product_events_public();

    let error = session
        .invoke_team(AgentTeamOptions::new(
            "broken-supervisor",
            "task",
            prompt_options(&cwd, api, "task"),
        ))
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Unknown team supervisor agent profile: missing-lead"),
        "{error}"
    );
    let events = drain_events(&mut events);
    assert!(has_event(&events, "Team(Failed)"));
}

#[tokio::test]
async fn team_invocation_reports_child_runtime_failure() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().join("workspace");
    let global = temp.path().join("global");
    fs::create_dir_all(&global).unwrap();
    write_agent(&cwd, "coder", "Coder", None);
    write_file(
        cwd.join(".pi-rust/teams/failing-member.toml"),
        r#"
schema_version = 1
id = "failing-member"
display_name = "Failing Member Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    );
    let _env_guard = EnvGuard::with_pi_rust_dir(global);

    let api = "agent-team-child-failure-api";
    let _provider_guard = ProviderGuard::register_failing(api);
    let mut session =
        CodingAgentSession::non_persistent(CodingAgentSessionOptions::new().with_cwd(&cwd))
            .await
            .unwrap();
    let mut events = session.subscribe_product_events_public();

    let error = session
        .invoke_team(AgentTeamOptions::new(
            "failing-member",
            "task",
            prompt_options(&cwd, api, "task"),
        ))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("member child failed"), "{error}");
    let events = drain_events(&mut events);
    assert!(has_event(&events, "Workflow(PromptFailed)"));
    assert!(has_event(&events, "Team(Failed)"));
    assert!(!has_event(&events, "Team(Completed)"));
}

fn prompt_options(cwd: &Path, api: &str, prompt: &str) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        auth_diagnostics: Vec::new(),
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(2),
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

fn write_agent(cwd: &Path, id: &str, display_name: &str, system_prompt: Option<&str>) {
    let system_prompt = system_prompt
        .map(|prompt| format!("system_prompt = {prompt:?}\n"))
        .unwrap_or_default();
    write_file(
        cwd.join(format!(".pi-rust/agents/{id}.toml")),
        &format!(
            "schema_version = 1\nid = {id:?}\ndisplay_name = {display_name:?}\n{system_prompt}"
        ),
    );
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content.trim_start()).unwrap();
}

fn drain_events(receiver: &mut CodingAgentProductEventReceiver) -> Vec<CodingAgentProductEvent> {
    let mut events = Vec::new();
    while let Ok(Some(event)) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn has_event(events: &[CodingAgentProductEvent], kind: &str) -> bool {
    events.iter().any(|event| event.kind == kind)
}

#[derive(Debug, Clone)]
struct RecordedCall {
    system_prompt: Option<String>,
    user_texts: Vec<String>,
}

struct QueueProvider {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    responses: Arc<Mutex<VecDeque<String>>>,
}

struct FailingProvider;

impl ApiProvider for QueueProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let user_texts = ctx
            .messages
            .iter()
            .filter_map(|message| match message {
                Message::User { content } => Some(
                    content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text, .. } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.calls.lock().unwrap().push(RecordedCall {
            system_prompt: ctx.system_prompt.clone(),
            user_texts,
        });
        let text = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "queued response missing".to_string());
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("agent-team-test", &model_id);
            message.provider = Some("agent-team-test".into());
            message.content.push(ContentBlock::Text {
                text,
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

impl ApiProvider for FailingProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let model_id = model.id.clone();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("agent-team-failure", &model_id);
            message.error_message = Some("member child failed".into());
            message.stop_reason = StopReason::Error;
            yield AssistantMessageEvent::Error {
                reason: StopReason::Error,
                message,
            };
        })
    }
}

struct ProviderGuard {
    _guard: RegistryProviderGuard<'static>,
}

impl ProviderGuard {
    fn register(api: &str, calls: Arc<Mutex<Vec<RecordedCall>>>, responses: Vec<&str>) -> Self {
        let guard = RegistryProviderGuard::register(
            api,
            Arc::new(QueueProvider {
                calls,
                responses: Arc::new(Mutex::new(
                    responses.into_iter().map(str::to_string).collect(),
                )),
            }),
        );
        Self { _guard: guard }
    }

    fn register_failing(api: &str) -> Self {
        let guard = RegistryProviderGuard::register(api, Arc::new(FailingProvider));
        Self { _guard: guard }
    }
}
