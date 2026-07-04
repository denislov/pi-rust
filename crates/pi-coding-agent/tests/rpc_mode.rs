mod support;

use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::{CliRunOptions, SessionRunOptions, protocol::rpc::run_rpc_mode_for_io};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::EnvGuard;
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, oneshot};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

fn large_context_faux_model(api: &str) -> Model {
    let mut model = faux_model(api);
    model.context_window = 100_000;
    model
}

fn parse_lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn delegate_agent_response(tool_call_id: &str, agent_id: &str, task: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: Vec::new(),
        thinking_deltas: Vec::new(),
        tool_calls: vec![FauxToolCall {
            id: tool_call_id.into(),
            name: "delegate_agent".into(),
            deltas: Vec::new(),
            final_arguments: serde_json::json!({
                "agent_id": agent_id,
                "task": task,
            }),
        }],
    }
}

fn write_file(path: impl AsRef<std::path::Path>, content: &str) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content.trim_start()).unwrap();
}

struct PausingProvider {
    release: Arc<Notify>,
    opened: Arc<AtomicBool>,
}

impl pi_ai::registry::ApiProvider for PausingProvider {
    fn stream(&self, model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let release = Arc::clone(&self.release);
        let opened = Arc::clone(&self.opened);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut partial = AssistantMessage::empty("pausing", &model_id);
            partial.provider = Some("pausing".into());
            yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };

            partial.content.push(ContentBlock::Text {
                text: String::new(),
                text_signature: None,
            });
            yield AssistantMessageEvent::TextStart { content_index: 0, partial: partial.clone() };

            if let Some(ContentBlock::Text { text, .. }) = partial.content.last_mut() {
                text.push_str("partial");
            }
            yield AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "partial".to_string(),
                partial: partial.clone(),
            };

            if !opened.load(Ordering::SeqCst) {
                release.notified().await;
                opened.store(true, Ordering::SeqCst);
            }

            yield AssistantMessageEvent::TextEnd { content_index: 0, partial: partial.clone() };
            partial.stop_reason = StopReason::Stop;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: partial,
            };
        })
    }
}

struct AbortAwareProvider {
    cancelled: Arc<AtomicBool>,
    release: Arc<Notify>,
}

impl pi_ai::registry::ApiProvider for AbortAwareProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let cancelled = Arc::clone(&self.cancelled);
        let release = Arc::clone(&self.release);
        let cancel = opts.and_then(|opts| opts.cancel);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut partial = AssistantMessage::empty("abort-aware", &model_id);
            partial.provider = Some("abort-aware".into());
            yield AssistantMessageEvent::Start { content_index: None, partial: partial.clone() };

            if let Some(cancel) = cancel {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        cancelled.store(true, Ordering::SeqCst);
                        partial.stop_reason = StopReason::Aborted;
                    }
                    _ = release.notified() => {
                        partial.stop_reason = StopReason::Stop;
                    }
                }
            } else {
                release.notified().await;
                partial.stop_reason = StopReason::Stop;
            }

            let reason = partial.stop_reason.clone();
            yield AssistantMessageEvent::Done {
                reason,
                message: partial,
            };
        })
    }
}

struct BlockingTwoTurnProvider {
    contexts: Arc<Mutex<Vec<Context>>>,
    first_started: Mutex<Option<oneshot::Sender<()>>>,
    release_first: Mutex<Option<oneshot::Receiver<()>>>,
}

impl BlockingTwoTurnProvider {
    fn new(
        contexts: Arc<Mutex<Vec<Context>>>,
        first_started: oneshot::Sender<()>,
        release_first: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            contexts,
            first_started: Mutex::new(Some(first_started)),
            release_first: Mutex::new(Some(release_first)),
        }
    }
}

impl pi_ai::registry::ApiProvider for BlockingTwoTurnProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let call_index = {
            let mut contexts = self.contexts.lock().unwrap();
            contexts.push(ctx);
            contexts.len()
        };
        let first_release = if call_index == 1 {
            if let Some(started) = self.first_started.lock().unwrap().take() {
                let _ = started.send(());
            }
            self.release_first.lock().unwrap().take()
        } else {
            None
        };
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            if let Some(release) = first_release {
                let _ = release.await;
            }
            let text = if call_index == 1 { "first" } else { "second" };
            let mut message = AssistantMessage::empty("blocking", &model_id);
            message.provider = Some("blocking".into());
            message.content.push(ContentBlock::Text {
                text: text.into(),
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
async fn rpc_processes_command_before_stdin_eof() {
    let api = "pi-coding-rpc-streaming";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let (mut input_writer, input_reader) = tokio::io::duplex(128);
    let (output_writer, mut output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let mut buf = vec![0; 4096];
    let bytes_read = tokio::time::timeout(Duration::from_millis(250), output_reader.read(&mut buf))
        .await
        .expect("rpc response before stdin EOF")
        .unwrap();

    let lines = parse_lines(&buf[..bytes_read]);
    assert_eq!(lines[0]["id"], "s1");
    assert_eq!(lines[0]["command"], "get_state");
    assert_eq!(lines[0]["success"], true);

    drop(input_writer);
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_reports_capabilities_when_idle() {
    let api = "pi-coding-rpc-capabilities-idle";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let capabilities = &lines[0]["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "available");
    for capability in ["abort", "steer", "followUp"] {
        assert_eq!(capabilities[capability]["status"], "disabled");
        assert_eq!(capabilities[capability]["reason"], "no prompt is running");
    }
    assert_eq!(capabilities["tools"]["status"], "available");
    assert_eq!(capabilities["agentProfiles"]["status"], "available");
    assert_eq!(capabilities["teamProfiles"]["status"], "available");
    assert_eq!(capabilities["delegation"]["status"], "available");
    for capability in [
        "compact",
        "fork",
        "cloneSession",
        "branchSummary",
        "export",
        "pluginReload",
        "selfHealingEdit",
    ] {
        assert_eq!(capabilities[capability]["status"], "disabled");
        assert_eq!(
            capabilities[capability]["reason"],
            "requires persistent Rust-native session"
        );
    }
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_self_healing_edit_applies_edit_through_persistent_session() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let target = cwd.join("src/app.txt");
    write_file(&target, "one\ntwo\nthree\n");

    let input = br#"{"id":"e1","type":"self_healing_edit","path":"src/app.txt","edits":[{"oldText":"two","newText":"deux"}]}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd.clone());
    session_options.session_dir = Some(sessions.clone());
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-self-healing-edit")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert!(
        sessions.exists(),
        "self-healing edit should create a persistent session log"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "one\ndeux\nthree\n"
    );
    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "e1");
    assert_eq!(lines[0]["command"], "self_healing_edit");
    assert_eq!(lines[0]["success"], true);
    let data = &lines[0]["data"];
    assert_eq!(data["path"], "src/app.txt");
    assert_eq!(data["attempts"], 1);
    assert_eq!(data["firstChangedLine"], 2);
    assert_eq!(data["diagnostics"], serde_json::json!([]));
    assert_eq!(data["checkOutput"], serde_json::Value::Null);
    assert!(
        data["message"]
            .as_str()
            .unwrap()
            .contains("Successfully replaced 1 block(s) in src/app.txt.")
    );
    assert!(data["diff"].as_str().unwrap().contains("-2 two"));
    assert!(data["diff"].as_str().unwrap().contains("+2 deux"));
    assert!(data["patch"].as_str().unwrap().contains("--- src/app.txt"));
    assert!(data["patch"].as_str().unwrap().contains("+++ src/app.txt"));
}

#[tokio::test]
async fn rpc_self_healing_edit_runs_check_command() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let target = cwd.join("src/app.txt");
    write_file(&target, "one\ntwo\nthree\n");

    let input = br#"{"id":"e-check","type":"self_healing_edit","path":"src/app.txt","edits":[{"oldText":"two","newText":"deux"}],"checkCommand":"printf rpc-check-ok"}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd.clone());
    session_options.session_dir = Some(sessions);
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-self-healing-edit-check")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "one\ndeux\nthree\n"
    );
    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "e-check");
    assert_eq!(lines[0]["command"], "self_healing_edit");
    assert_eq!(lines[0]["success"], true);
    let check_output = &lines[0]["data"]["checkOutput"];
    assert_eq!(check_output["command"], "printf rpc-check-ok");
    assert_eq!(check_output["stdout"], "rpc-check-ok");
    assert_eq!(check_output["stderr"], "");
    assert_eq!(check_output["exitCode"], 0);
}

#[tokio::test]
async fn rpc_self_healing_edit_failed_check_returns_check_output() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let target = cwd.join("src/app.txt");
    write_file(&target, "one\ntwo\nthree\n");

    let input = br#"{"id":"e-check-fail","type":"self_healing_edit","path":"src/app.txt","edits":[{"oldText":"two","newText":"deux"}],"checkCommand":"printf rpc-check-failed >&2; exit 9"}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd.clone());
    session_options.session_dir = Some(sessions);
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-self-healing-edit-check-fail")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "one\ndeux\nthree\n"
    );
    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "e-check-fail");
    assert_eq!(lines[0]["command"], "self_healing_edit");
    assert_eq!(lines[0]["success"], false);
    assert!(
        lines[0]["error"]
            .as_str()
            .unwrap()
            .contains("self-healing edit check failed")
    );
    let data = &lines[0]["data"];
    assert_eq!(data["diagnostics"].as_array().unwrap().len(), 1);
    assert!(
        data["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("rpc-check-failed")
    );
    let check_output = &data["checkOutput"];
    assert_eq!(
        check_output["command"],
        "printf rpc-check-failed >&2; exit 9"
    );
    assert_eq!(check_output["stdout"], "");
    assert_eq!(check_output["stderr"], "rpc-check-failed");
    assert_eq!(check_output["exitCode"], 9);
}

#[tokio::test]
async fn rpc_self_healing_edit_uses_planned_repair_attempts() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let target = cwd.join("src/app.txt");
    write_file(&target, "one\ntwo\nthree\n");

    let input = br#"{"id":"e-repair","type":"self_healing_edit","path":"src/app.txt","edits":[{"oldText":"two","newText":"deux"}],"checkCommand":"grep -q dos src/app.txt","repairAttempts":[[{"oldText":"deux","newText":"dos"}]]}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd.clone());
    session_options.session_dir = Some(sessions);
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-self-healing-edit-repair")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "one\ndos\nthree\n"
    );
    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "e-repair");
    assert_eq!(lines[0]["command"], "self_healing_edit");
    assert_eq!(lines[0]["success"], true);
    let data = &lines[0]["data"];
    assert_eq!(data["attempts"], 2);
    assert_eq!(data["checkOutput"]["command"], "grep -q dos src/app.txt");
    assert_eq!(data["checkOutput"]["exitCode"], 0);
    assert_eq!(data["diagnostics"].as_array().unwrap().len(), 1);
    assert!(
        data["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("grep -q dos src/app.txt")
    );
    let repair_attempts = data["repairAttempts"].as_array().unwrap();
    assert_eq!(repair_attempts.len(), 1);
    let repair = &repair_attempts[0];
    assert_eq!(repair["attempt"], 1);
    assert_eq!(repair["edits"][0]["oldText"], "deux");
    assert_eq!(repair["edits"][0]["newText"], "dos");
    assert!(
        repair["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("grep -q dos src/app.txt")
    );
    assert_eq!(repair["checkOutput"]["command"], "grep -q dos src/app.txt");
    assert_eq!(repair["checkOutput"]["exitCode"], 0);
}

#[tokio::test]
async fn rpc_self_healing_edit_uses_model_repair_policy() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let target = cwd.join("src/app.txt");
    write_file(&target, "one\ntwo\nthree\n");
    let api = "pi-coding-rpc-self-healing-edit-model-repair";
    registry::register(
        api,
        Arc::new(FauxProvider::simple_text(
            r#"{"edits":[{"oldText":"deux","newText":"dos"}]}"#,
        )),
    );

    let input = br#"{"id":"e-model-repair","type":"self_healing_edit","path":"src/app.txt","edits":[{"oldText":"two","newText":"deux"}],"checkCommand":"grep -q dos src/app.txt","modelRepair":{"maxAttempts":1}}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd.clone());
    session_options.session_dir = Some(sessions);
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "one\ndos\nthree\n"
    );
    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "e-model-repair");
    assert_eq!(lines[0]["command"], "self_healing_edit");
    assert_eq!(lines[0]["success"], true);
    let data = &lines[0]["data"];
    assert_eq!(data["attempts"], 2);
    assert_eq!(data["checkOutput"]["command"], "grep -q dos src/app.txt");
    assert_eq!(data["checkOutput"]["exitCode"], 0);
    assert_eq!(data["diagnostics"].as_array().unwrap().len(), 1);
    assert!(
        data["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("grep -q dos src/app.txt")
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_list_agent_profiles_reports_registry() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
description = "Implementation agent"
model = "gpt-5-codex"
system_prompt = "You write code."
tools = ["shell", "apply_patch"]
skills = ["superpowers:test-driven-development"]
supervision = "self_review"

[delegation]
allow_delegate_agent = true
max_depth = 1
require_confirmation = "writes"
allowed_agents = ["reviewer"]
"#,
    );

    let input = br#"{"id":"a1","type":"list_agent_profiles"}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions.clone());
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-list-agents")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert!(
        !sessions.exists(),
        "profile listing should not create a session"
    );

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "a1");
    assert_eq!(lines[0]["command"], "list_agent_profiles");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[0]["data"]["defaultAgentProfileId"], "default");
    assert!(
        lines[0]["data"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let agents = lines[0]["data"]["agents"].as_array().unwrap();
    let default = agents
        .iter()
        .find(|profile| profile["id"] == "default")
        .expect("default profile is listed");
    assert_eq!(default["source"], "built_in");
    assert_eq!(default["isDefault"], true);

    let coder = agents
        .iter()
        .find(|profile| profile["id"] == "coder")
        .expect("project profile is listed");
    assert_eq!(coder["displayName"], "Coder");
    assert_eq!(coder["description"], "Implementation agent");
    assert_eq!(coder["source"], "project");
    assert_eq!(coder["isDefault"], false);
    assert_eq!(coder["model"], "gpt-5-codex");
    assert_eq!(coder["systemPrompt"], "You write code.");
    assert_eq!(coder["tools"][0], "shell");
    assert_eq!(coder["skills"][0], "superpowers:test-driven-development");
    assert_eq!(coder["supervision"], "self_review");
    assert_eq!(coder["delegation"]["allowDelegateAgent"], true);
    assert_eq!(coder["delegation"]["allowDelegateTeam"], false);
    assert_eq!(coder["delegation"]["maxDepth"], 1);
    assert_eq!(coder["delegation"]["requireConfirmation"], "writes");
    assert_eq!(coder["delegation"]["allowedAgents"][0], "reviewer");
}

#[tokio::test]
async fn rpc_list_team_profiles_reports_registry() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
description = "Planner and coder"
supervisor = "planner"
strategy = "plan_execute_review"
members = ["planner", "coder"]

[delegation]
max_parallel_children = 2
max_depth = 1
require_confirmation = "always"
"#,
    );

    let input = br#"{"id":"t1","type":"list_team_profiles"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-list-teams")),
            tools: Vec::new(),
            register_builtins: false,
            session: SessionRunOptions::disabled(cwd),
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "t1");
    assert_eq!(lines[0]["command"], "list_team_profiles");
    assert_eq!(lines[0]["success"], true);
    assert!(
        lines[0]["data"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let teams = lines[0]["data"]["teams"].as_array().unwrap();
    let team = teams
        .iter()
        .find(|profile| profile["id"] == "implementation")
        .expect("project team profile is listed");
    assert_eq!(team["displayName"], "Implementation Team");
    assert_eq!(team["description"], "Planner and coder");
    assert_eq!(team["source"], "project");
    assert_eq!(team["supervisor"]["mode"], "agent");
    assert_eq!(team["supervisor"]["profileId"], "planner");
    assert_eq!(team["strategy"], "plan_execute_review");
    assert_eq!(team["members"][0], "planner");
    assert_eq!(team["members"][1], "coder");
    assert_eq!(team["delegation"]["maxParallelChildren"], 2);
    assert_eq!(team["delegation"]["requireConfirmation"], "always");
}

#[tokio::test]
async fn rpc_invoke_agent_returns_response_then_agent_events() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    );

    let api = "pi-coding-rpc-invoke-agent";
    registry::register(api, Arc::new(FauxProvider::simple_text("from agent")));
    let input = br#"{"id":"a1","type":"invoke_agent","profileId":"coder","task":"do work"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: SessionRunOptions::disabled(cwd),
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "a1");
    assert_eq!(lines[0]["command"], "invoke_agent");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[0]["data"]["profileId"], "coder");
    assert_eq!(lines[0]["data"]["task"], "do work");
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "agent_invocation_start")
    );
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "agent_invocation_end")
    );
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    assert!(String::from_utf8_lossy(&output).contains("from agent"));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_invoke_agent_rejects_unknown_profile() {
    let input = br#"{"id":"a1","type":"invoke_agent","profileId":"missing","task":"do work"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-invoke-agent-missing")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "a1");
    assert_eq!(lines[0]["command"], "invoke_agent");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["error"], "Unknown agent profile: missing");
}

#[tokio::test]
async fn rpc_lists_and_approves_delegation_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
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

    let api = "pi-coding-rpc-delegation-approve";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![delegate_agent_response(
                    "tool_delegate_agent",
                    "coder",
                    "implement parser",
                )],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("parent ready")],
                stop_reason: StopReason::Stop,
            },
            FauxCall {
                responses: vec![text_response("child result")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(8192);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(large_context_faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
    });

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    input_writer
        .write_all(
            b"{\"id\":\"p1\",\"type\":\"set_default_agent_profile\",\"profileId\":\"delegating-planner\"}\n",
        )
        .await
        .unwrap();
    let set_default_response = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before set_default_agent_profile response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "set_default_agent_profile" {
                break value;
            }
        }
    })
    .await
    .expect("set_default_agent_profile response");
    assert_eq!(set_default_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();
    let confirmation = tokio::time::timeout(Duration::from_secs(2), async {
        let mut seen = Vec::new();
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before delegation confirmation event: {seen:?}");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            let event_type = value["type"].as_str().unwrap_or("<missing-type>");
            let summary = if event_type == "response" {
                format!(
                    "response command={} success={} error={}",
                    value["command"].as_str().unwrap_or("<missing-command>"),
                    value["success"].as_bool().unwrap_or(false),
                    value["error"].as_str().unwrap_or("")
                )
            } else {
                event_type.to_string()
            };
            seen.push(summary);
            if value["type"] == "delegation_confirmation_required" {
                break value;
            }
            if value["type"] == "agent_end" {
                panic!("prompt ended before delegation confirmation event: {seen:?}");
            }
        }
    })
    .await
    .expect("delegation confirmation event");
    let operation_id = confirmation["operationId"].as_str().unwrap().to_string();
    let tool_call_id = confirmation["toolCallId"].as_str().unwrap().to_string();
    assert_eq!(confirmation["requestingProfileId"], "delegating-planner");
    assert_eq!(confirmation["targetKind"], "agent");
    assert_eq!(confirmation["targetId"], "coder");
    assert_eq!(confirmation["task"], "implement parser");

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before prompt completion");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("prompt completion");

    input_writer
        .write_all(b"{\"id\":\"l1\",\"type\":\"list_delegation_confirmations\"}\n")
        .await
        .unwrap();
    let list_response = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before list_delegation_confirmations response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "list_delegation_confirmations" {
                break value;
            }
        }
    })
    .await
    .expect("list_delegation_confirmations response");
    assert_eq!(list_response["success"], true);
    assert_eq!(
        list_response["data"]["confirmations"][0]["operationId"],
        operation_id
    );
    assert_eq!(
        list_response["data"]["confirmations"][0]["toolCallId"],
        tool_call_id
    );

    let approve_command = serde_json::json!({
        "id": "a1",
        "type": "approve_delegation",
        "operationId": operation_id,
        "toolCallId": tool_call_id,
    })
    .to_string()
        + "\n";
    input_writer
        .write_all(approve_command.as_bytes())
        .await
        .unwrap();

    let approve_response = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before approve_delegation response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "approve_delegation" {
                break value;
            }
        }
    })
    .await
    .expect("approve_delegation response");
    assert_eq!(approve_response["success"], true);
    assert_eq!(approve_response["data"]["delegation"]["targetId"], "coder");

    let mut saw_approved = false;
    let mut saw_completed = false;
    {
        let mut seen = Vec::new();
        loop {
            let line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
                .await
                .unwrap_or_else(|_| {
                    panic!(
                        "timed out before delegation approval completion: saw_approved={saw_approved}, saw_completed={saw_completed}, seen={seen:?}"
                    )
                })
                .unwrap();
            let Some(line) = line else {
                panic!("rpc output closed before delegation approval completion: {seen:?}");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            seen.push(if value["type"] == "delegation_completed" {
                format!(
                    "delegation_completed target={} finalText={}",
                    value["targetId"].as_str().unwrap_or("<missing-target>"),
                    value["finalText"].as_str().unwrap_or("<missing-finalText>")
                )
            } else {
                value["type"]
                    .as_str()
                    .unwrap_or("<missing-type>")
                    .to_string()
            });
            if value["type"] == "delegation_approved" {
                saw_approved = true;
            }
            if value["type"] == "delegation_completed"
                && value["targetId"] == "coder"
                && value["finalText"] == "child result"
            {
                saw_completed = true;
            }
            if saw_approved && saw_completed {
                break;
            }
        }
    }

    drop(input_writer);
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_rejects_delegation_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
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
"#,
    );

    let api = "pi-coding-rpc-delegation-reject";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![delegate_agent_response(
                    "tool_delegate_agent",
                    "coder",
                    "implement parser",
                )],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("parent ready")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(8192);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(large_context_faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
    });

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    input_writer
        .write_all(
            b"{\"id\":\"p1\",\"type\":\"set_default_agent_profile\",\"profileId\":\"delegating-planner\"}\n",
        )
        .await
        .unwrap();
    let set_default_response = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before set_default_agent_profile response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "set_default_agent_profile" {
                break value;
            }
        }
    })
    .await
    .expect("set_default_agent_profile response");
    assert_eq!(set_default_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();
    let confirmation = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before delegation confirmation event");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "delegation_confirmation_required" {
                break value;
            }
        }
    })
    .await
    .expect("delegation confirmation event");
    let operation_id = confirmation["operationId"].as_str().unwrap().to_string();
    let tool_call_id = confirmation["toolCallId"].as_str().unwrap().to_string();

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before prompt completion");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("prompt completion");

    let reject_command = serde_json::json!({
        "id": "r1",
        "type": "reject_delegation",
        "operationId": operation_id,
        "toolCallId": tool_call_id,
        "reason": "not now",
    })
    .to_string()
        + "\n";
    input_writer
        .write_all(reject_command.as_bytes())
        .await
        .unwrap();

    let reject_response = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before reject_delegation response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "reject_delegation" {
                break value;
            }
        }
    })
    .await
    .expect("reject_delegation response");
    assert_eq!(reject_response["success"], true);
    assert_eq!(reject_response["data"]["reason"], "not now");

    let rejected_event = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before delegation_rejected event");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "delegation_rejected" {
                break value;
            }
        }
    })
    .await
    .expect("delegation_rejected event");
    assert_eq!(rejected_event["targetId"], "coder");
    assert_eq!(rejected_event["reason"], "not now");

    drop(input_writer);
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_approve_delegation_rejects_unknown_pending_request() {
    let input = br#"{"id":"a1","type":"approve_delegation","operationId":"op_missing","toolCallId":"tool_missing"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-delegation-missing")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "a1");
    assert_eq!(lines[0]["command"], "approve_delegation");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["error"], "no active coding session");
}

#[tokio::test]
async fn rpc_state_reports_agent_invocation_busy_while_running() {
    let api = "pi-coding-rpc-invoke-agent-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(
            b"{\"id\":\"a1\",\"type\":\"invoke_agent\",\"profileId\":\"default\",\"task\":\"hello\"}\n",
        )
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let invoke_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("invoke_agent response before provider completes")
        .unwrap()
        .unwrap();
    let invoke_response: serde_json::Value = serde_json::from_str(&invoke_response).unwrap();
    assert_eq!(invoke_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before get_state response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "get_state" {
                break value;
            }
        }
    })
    .await
    .expect("state response while agent invocation is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(state["data"]["isStreaming"], true);
    let capabilities = &state["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "busy");
    assert_eq!(capabilities["prompt"]["operation"], "agent_invocation");
    assert_eq!(capabilities["abort"]["status"], "disabled");
    assert_eq!(capabilities["agentProfiles"]["status"], "busy");
    assert_eq!(
        capabilities["agentProfiles"]["operation"],
        "agent_invocation"
    );
    assert_eq!(capabilities["teamProfiles"]["status"], "busy");
    assert_eq!(
        capabilities["teamProfiles"]["operation"],
        "agent_invocation"
    );
    assert_eq!(capabilities["delegation"]["status"], "busy");
    assert_eq!(capabilities["delegation"]["operation"], "agent_invocation");
    assert_eq!(capabilities["selfHealingEdit"]["status"], "disabled");
    assert_eq!(
        capabilities["selfHealingEdit"]["reason"],
        "requires persistent Rust-native session"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_invoke_team_returns_response_then_agent_events() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    );
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation"
supervisor = "deterministic"
members = ["coder"]
"#,
    );

    let api = "pi-coding-rpc-invoke-team";
    registry::register(api, Arc::new(FauxProvider::simple_text("member result")));
    let input =
        br#"{"id":"t1","type":"invoke_team","teamId":"implementation","task":"ship feature"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: SessionRunOptions::disabled(cwd),
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "t1");
    assert_eq!(lines[0]["command"], "invoke_team");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[0]["data"]["teamId"], "implementation");
    assert_eq!(lines[0]["data"]["task"], "ship feature");
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "agent_team_start"));
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "agent_team_member_start")
    );
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "agent_team_member_end")
    );
    assert!(lines.iter().any(|line| line["type"] == "agent_team_end"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    assert!(String::from_utf8_lossy(&output).contains("member result"));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_invoke_team_rejects_unknown_team() {
    let input = br#"{"id":"t1","type":"invoke_team","teamId":"missing","task":"do work"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-invoke-team-missing")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "t1");
    assert_eq!(lines[0]["command"], "invoke_team");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["error"], "Unknown team profile: missing");
}

#[tokio::test]
async fn rpc_state_reports_agent_team_busy_while_running() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    write_file(
        cwd.join(".pi-rust/teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation"
supervisor = "deterministic"
members = ["default"]
"#,
    );

    let api = "pi-coding-rpc-invoke-team-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(
            b"{\"id\":\"t1\",\"type\":\"invoke_team\",\"teamId\":\"implementation\",\"task\":\"hello\"}\n",
        )
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let invoke_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("invoke_team response before provider completes")
        .unwrap()
        .unwrap();
    let invoke_response: serde_json::Value = serde_json::from_str(&invoke_response).unwrap();
    assert_eq!(invoke_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before get_state response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "get_state" {
                break value;
            }
        }
    })
    .await
    .expect("state response while agent team is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(state["data"]["isStreaming"], true);
    let capabilities = &state["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "busy");
    assert_eq!(capabilities["prompt"]["operation"], "agent_team");
    assert_eq!(capabilities["abort"]["status"], "disabled");
    assert_eq!(capabilities["agentProfiles"]["status"], "busy");
    assert_eq!(capabilities["agentProfiles"]["operation"], "agent_team");
    assert_eq!(capabilities["teamProfiles"]["status"], "busy");
    assert_eq!(capabilities["teamProfiles"]["operation"], "agent_team");
    assert_eq!(capabilities["delegation"]["status"], "busy");
    assert_eq!(capabilities["delegation"]["operation"], "agent_team");
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_set_default_agent_profile_updates_session_listing() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    write_file(
        cwd.join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    );

    let input = br#"{"id":"l1","type":"list_agent_profiles"}
{"id":"s1","type":"set_default_agent_profile","profileId":"coder"}
{"id":"l2","type":"list_agent_profiles"}
"#;
    let mut output = Vec::new();
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions.clone());
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-set-default-agent")),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    assert!(
        sessions.exists(),
        "setting the default profile should create a session"
    );
    let lines = parse_lines(&output);
    let first_listing = lines
        .iter()
        .find(|line| line["id"] == "l1" && line["command"] == "list_agent_profiles")
        .expect("first profile listing response");
    assert_eq!(first_listing["data"]["defaultAgentProfileId"], "default");

    let set_response = lines
        .iter()
        .find(|line| line["id"] == "s1" && line["command"] == "set_default_agent_profile")
        .expect("default profile switch response");
    assert_eq!(set_response["success"], true);
    assert_eq!(set_response["data"]["defaultAgentProfileId"], "coder");
    assert!(lines.iter().any(|line| {
        line["type"] == "default_agent_profile_changed" && line["profileId"] == "coder"
    }));

    let second_listing = lines
        .iter()
        .find(|line| line["id"] == "l2" && line["command"] == "list_agent_profiles")
        .expect("second profile listing response");
    assert_eq!(second_listing["data"]["defaultAgentProfileId"], "coder");
    let agents = second_listing["data"]["agents"].as_array().unwrap();
    let coder = agents
        .iter()
        .find(|profile| profile["id"] == "coder")
        .expect("coder profile should be listed");
    assert_eq!(coder["isDefault"], true);
    let default = agents
        .iter()
        .find(|profile| profile["id"] == "default")
        .expect("default profile should be listed");
    assert_eq!(default["isDefault"], false);
}

#[tokio::test]
async fn rpc_set_default_agent_profile_rejects_unknown_profile() {
    let input = br#"{"id":"s1","type":"set_default_agent_profile","profileId":"missing"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-set-default-agent-missing")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "s1");
    assert_eq!(lines[0]["command"], "set_default_agent_profile");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["error"], "Unknown agent profile: missing");
}

#[tokio::test]
async fn rpc_set_default_agent_profile_rejects_while_prompt_running() {
    let api = "pi-coding-rpc-set-default-agent-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(
            b"{\"id\":\"s1\",\"type\":\"set_default_agent_profile\",\"profileId\":\"default\"}\n",
        )
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before set_default_agent_profile response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "set_default_agent_profile" {
                break value;
            }
        }
    })
    .await
    .expect("set_default_agent_profile rejection while prompt is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(response["id"], "s1");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "cannot set default agent profile while agent is streaming"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_list_agent_profiles_rejects_while_prompt_running() {
    let api = "pi-coding-rpc-list-agents-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"a1\",\"type\":\"list_agent_profiles\"}\n")
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before list_agent_profiles response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "list_agent_profiles" {
                break value;
            }
        }
    })
    .await
    .expect("list_agent_profiles rejection while prompt is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(response["id"], "a1");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "cannot list agent profiles while agent is streaming"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_reports_prompt_busy_while_running() {
    let api = "pi-coding-rpc-capabilities-busy";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before get_state response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "get_state" {
                break value;
            }
        }
    })
    .await
    .expect("state response while prompt is running");

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(state["data"]["isStreaming"], true);
    let capabilities = &state["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "busy");
    assert_eq!(capabilities["prompt"]["operation"], "prompt");
    assert_eq!(capabilities["abort"]["status"], "available");
    assert_eq!(capabilities["steer"]["status"], "available");
    assert_eq!(capabilities["followUp"]["status"], "available");
    assert_eq!(capabilities["agentProfiles"]["status"], "busy");
    assert_eq!(capabilities["agentProfiles"]["operation"], "prompt");
    assert_eq!(capabilities["teamProfiles"]["status"], "busy");
    assert_eq!(capabilities["teamProfiles"]["operation"], "prompt");
    assert_eq!(capabilities["delegation"]["status"], "busy");
    assert_eq!(capabilities["delegation"]["operation"], "prompt");
    assert_eq!(capabilities["selfHealingEdit"]["status"], "disabled");
    assert_eq!(
        capabilities["selfHealingEdit"]["reason"],
        "requires persistent Rust-native session"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_parse_error_keeps_process_alive_for_next_command() {
    let api = "pi-coding-rpc-parse";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{bad json}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["type"], "response");
    assert_eq!(lines[0]["command"], "parse");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[1]["id"], "s1");
    assert_eq!(lines[1]["command"], "get_state");
    assert_eq!(lines[1]["success"], true);
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_uses_settings_default_model_when_no_override_is_provided() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "default_model = \"claude-haiku-4-5\"\n",
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let input = b"{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["data"]["model"]["id"], "claude-haiku-4-5");
}

#[tokio::test]
async fn rpc_unsupported_command_returns_error_response() {
    let api = "pi-coding-rpc-unsupported";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"m1\",\"type\":\"set_model\",\"provider\":\"faux\",\"modelId\":\"x\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "m1");
    assert_eq!(lines[0]["command"], "set_model");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(
        lines[0]["error"],
        "unsupported command in Rust M5: set_model"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_reload_reports_project_plugin_manifest_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let project_plugin = cwd.join(".pi-rust/plugins/project-lua");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "project-lua"
name = "Project Lua"
version = "0.1.0"
runtime = "lua"
"#,
    )
    .unwrap();
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions);

    let api = "pi-coding-rpc-reload";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));
    let input = br#"{"id":"r1","type":"reload"}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "r1");
    assert_eq!(lines[0]["command"], "reload");
    assert_eq!(lines[0]["success"], true);
    let diagnostics = lines[0]["data"]["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["pluginId"] == "project-lua"
            && diagnostic["message"] == "Lua plugin entry is required"
    }));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_plugin_command_runs_loaded_lua_plugin_command() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    let project_plugin = cwd.join(".pi-rust/plugins/lua-command");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-command"
name = "Lua Command"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.say_hello",
    description = "greets from lua command",
    run = function(input)
      return { content = "hello " .. input.name }
    end
  })
end
"#,
    )
    .unwrap();
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions);

    let api = "pi-coding-rpc-plugin-command";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));
    let input = br#"{"id":"r1","type":"reload"}
{"id":"c1","type":"plugin_command","commandId":"lua.say_hello","args":{"name":"rpc"}}
"#;
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "r1");
    assert_eq!(lines[0]["command"], "reload");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[1]["id"], "c1");
    assert_eq!(lines[1]["command"], "plugin_command");
    assert_eq!(lines[1]["success"], true);
    assert_eq!(lines[1]["data"]["commandId"], "lua.say_hello");
    assert_eq!(lines[1]["data"]["output"], "hello rpc");
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_prompt_returns_response_then_agent_events() {
    let api = "pi-coding-rpc-prompt";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let input = b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["id"], "p1");
    assert_eq!(lines[0]["command"], "prompt");
    assert_eq!(lines[0]["success"], true);
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_streams_agent_events_before_prompt_finishes() {
    let api = "pi-coding-rpc-live-events";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(256);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let response_line = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();
    assert_eq!(response["id"], "p1");
    assert_eq!(response["command"], "prompt");
    assert_eq!(response["success"], true);

    let event_line = tokio::time::timeout(Duration::from_millis(250), lines.next_line()).await;
    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    let event_line = event_line
        .expect("agent event before prompt finishes")
        .unwrap()
        .unwrap();
    let event: serde_json::Value = serde_json::from_str(&event_line).unwrap();
    assert_eq!(event["type"], "agent_start");
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_abort_cancels_running_prompt() {
    let api = "pi-coding-rpc-abort";
    let cancelled = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(AbortAwareProvider {
            cancelled: Arc::clone(&cancelled),
            release: Arc::clone(&release),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(256);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();

    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let prompt_response = tokio::time::timeout(Duration::from_millis(250), lines.next_line())
        .await
        .expect("prompt response before provider completes")
        .unwrap()
        .unwrap();
    let prompt_response: serde_json::Value = serde_json::from_str(&prompt_response).unwrap();
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"a1\",\"type\":\"abort\"}\n")
        .await
        .unwrap();

    let abort_response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before abort response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "abort" {
                break value;
            }
        }
    })
    .await;

    drop(input_writer);
    let task_result = tokio::time::timeout(Duration::from_millis(500), task).await;
    if task_result.is_err() {
        release.notify_one();
        panic!("rpc task did not finish after abort");
    }
    task_result.unwrap().unwrap();

    let abort_response = abort_response.expect("abort response while prompt is running");
    assert_eq!(abort_response["id"], "a1");
    assert_eq!(abort_response["success"], true);
    assert_eq!(abort_response["data"]["cancelled"], true);
    assert!(cancelled.load(Ordering::SeqCst));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_steer_while_coding_prompt_running_sends_control() {
    let api = "pi-coding-rpc-steer-live";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"steer\",\"message\":\"look here\"}\n")
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before steer response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["command"] == "steer" {
                break value;
            }
        }
    })
    .await;

    let response = response.expect("steer response while prompt is running");
    assert_eq!(response["success"], true);
    release.notify_one();
    drop(input_writer);
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before agent_end");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("agent_end after releasing paused provider");
    task.await.unwrap();
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_follow_up_prompt_while_coding_prompt_running_sends_control() {
    let api = "pi-coding-rpc-follow-up-live";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    registry::register(
        api,
        Arc::new(BlockingTwoTurnProvider::new(
            Arc::clone(&contexts),
            started_tx,
            release_rx,
        )),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_millis(250), started_rx)
        .await
        .expect("provider first turn started")
        .unwrap();

    input_writer
        .write_all(
            b"{\"id\":\"f1\",\"type\":\"prompt\",\"message\":\"next\",\"streamingBehavior\":\"followUp\"}\n",
        )
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before follow-up response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["id"] == "f1" {
                break value;
            }
        }
    })
    .await;

    let response = response.expect("follow-up response while prompt is running");
    assert_eq!(response["success"], true);
    release_tx.send(()).unwrap();
    drop(input_writer);
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before agent_end");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "agent_end" {
                break;
            }
        }
    })
    .await
    .expect("agent_end after releasing paused provider");
    task.await.unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2);
    assert!(
        contexts[1].messages.iter().any(|message| matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Text { text, .. } if text == "next"
                ))
        )),
        "{:#?}",
        contexts[1].messages
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_plain_prompt_while_running_returns_error() {
    let api = "pi-coding-rpc-running-prompt-error";
    let release = Arc::new(Notify::new());
    registry::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::new(AtomicBool::new(false)),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(512);
    let (output_writer, output_reader) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n")
        .await
        .unwrap();
    let mut lines = tokio::io::BufReader::new(output_reader).lines();
    let _ = lines.next_line().await.unwrap().unwrap();
    let _ = lines.next_line().await.unwrap().unwrap();

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"second\"}\n")
        .await
        .unwrap();

    let response = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let Some(line) = lines.next_line().await.unwrap() else {
                panic!("rpc output closed before second prompt response");
            };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "response" && value["id"] == "p2" {
                break value;
            }
        }
    })
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    let response = response.expect("plain prompt rejection while prompt is running");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "agent is streaming; prompt requires streamingBehavior steer or followUp"
    );
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_commands_update_get_state() {
    let api = "pi-coding-rpc-state";
    registry::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"t1\",\"type\":\"set_thinking_level\",\"level\":\"high\"}\n\
                  {\"id\":\"q1\",\"type\":\"set_steering_mode\",\"mode\":\"one-at-a-time\"}\n\
                  {\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let state = lines
        .iter()
        .find(|line| line["command"] == "get_state")
        .unwrap();
    assert_eq!(state["data"]["thinkingLevel"], "high");
    assert_eq!(state["data"]["steeringMode"], "one-at-a-time");
    registry::unregister(api);
}
