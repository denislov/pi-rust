use crate::support;

use pi_agent_core::api::tool::AgentTool;
use pi_ai::api::conversation::{
    AssistantMessage, ContentBlock, Context, Cost, Message, StopReason, Usage,
};
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::stream::{AssistantMessageEvent, EventStream, StreamOptions};
use pi_ai::api::testing::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_coding_agent::api::protocol::run_rpc_mode_for_io;
use pi_coding_agent::api::runtime::{CliRunOptions, SessionRunOptions};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::{EnvGuard, ProviderGuard};
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, oneshot};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const RPC_LINE_READ_TIMEOUT: Duration = Duration::from_secs(2);
const RPC_RAW_OUTPUT_READ_TIMEOUT: Duration = Duration::from_millis(250);
const RPC_TASK_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const RPC_PROVIDER_START_TIMEOUT: Duration = Duration::from_millis(250);

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

fn multimodal_faux_model(api: &str) -> Model {
    let mut model = faux_model(api);
    model.input = vec![ModelInput::Text, ModelInput::Image];
    model
}

fn parse_lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

async fn read_rpc_line<R>(lines: &mut tokio::io::Lines<R>, context: &str) -> String
where
    R: AsyncBufRead + Unpin,
{
    tokio::time::timeout(RPC_LINE_READ_TIMEOUT, lines.next_line())
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {context}"))
        .unwrap_or_else(|error| panic!("failed reading {context}: {error}"))
        .unwrap_or_else(|| panic!("rpc output closed before {context}"))
}

async fn read_rpc_output_bytes<R>(output_reader: &mut R, buf: &mut [u8], context: &str) -> usize
where
    R: tokio::io::AsyncRead + Unpin,
{
    tokio::time::timeout(RPC_RAW_OUTPUT_READ_TIMEOUT, output_reader.read(buf))
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {context}"))
        .unwrap_or_else(|error| panic!("failed reading {context}: {error}"))
}

async fn await_rpc_task_completion(
    task: tokio::task::JoinHandle<()>,
    release_on_timeout: &Notify,
    context: &str,
) {
    let task_result = tokio::time::timeout(RPC_TASK_SHUTDOWN_TIMEOUT, task).await;
    if task_result.is_err() {
        release_on_timeout.notify_one();
        panic!("timed out waiting for {context}");
    }
    task_result.unwrap().unwrap();
}

async fn wait_for_rpc_provider_start(started_rx: oneshot::Receiver<()>, context: &str) {
    tokio::time::timeout(RPC_PROVIDER_START_TIMEOUT, started_rx)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {context}"))
        .unwrap_or_else(|_| panic!("provider start channel closed before {context}"));
}

async fn read_rpc_json_line<R>(lines: &mut tokio::io::Lines<R>, context: &str) -> serde_json::Value
where
    R: AsyncBufRead + Unpin,
{
    let line = read_rpc_line(lines, context).await;
    serde_json::from_str(&line)
        .unwrap_or_else(|error| panic!("invalid JSON for {context}: {error}"))
}

async fn read_rpc_json_matching<R>(
    lines: &mut tokio::io::Lines<R>,
    context: &str,
    mut matches: impl FnMut(&serde_json::Value) -> bool,
) -> serde_json::Value
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let value = read_rpc_json_line(lines, context).await;
        if matches(&value) {
            return value;
        }
    }
}

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: Vec::new(),
        tool_calls: Vec::new(),
    }
}

fn rpc_mutation_tool(executions: Arc<AtomicUsize>) -> AgentTool {
    AgentTool::new_text(
        "rpc_mutate",
        "mutate state for RPC authorization testing",
        serde_json::json!({"type": "object"}),
        move |_context, _args| {
            let executions = executions.clone();
            async move {
                executions.fetch_add(1, Ordering::SeqCst);
                Ok("mutated".to_string())
            }
        },
    )
}

fn rpc_read_only_stats_tool() -> AgentTool {
    AgentTool::new_text(
        "rpc_stats_read",
        "read deterministic statistics fixture data",
        serde_json::json!({
            "type": "object",
            "x-pi-authorization-risk": "workspace_local_read_only"
        }),
        |_context, _args| async { Ok("fixture result".to_owned()) },
    )
}

#[tokio::test]
async fn rpc_session_stats_use_persistent_replay_usage_and_tool_facts() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-session-stats";
    let usage = Usage {
        input: 7,
        output: 11,
        cache_read: 2,
        cache_write: 3,
        total_tokens: 23,
        cost: Cost {
            known: true,
            input: 0.1,
            output: 0.2,
            cache_read: 0.3,
            cache_write: 0.4,
        },
    };
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(
            FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: Vec::new(),
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "tool-stats-read".into(),
                            name: "rpc_stats_read".into(),
                            deltas: vec!["{}".into()],
                            final_arguments: serde_json::json!({}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("statistics complete", StopReason::Stop),
            ])
            .with_default_usage(usage),
        ),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: vec![rpc_read_only_stats_tool()],
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session,
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-stats\",\"type\":\"prompt\",\"message\":\"read stats\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "stats prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"get-stats\",\"type\":\"get_session_stats\"}\n")
        .await
        .unwrap();
    let response = read_rpc_json_matching(&mut lines, "session stats response", |value| {
        value["type"] == "response" && value["command"] == "get_session_stats"
    })
    .await;

    assert_eq!(response["success"], true, "{response}");
    assert_eq!(response["data"]["userMessages"], 1);
    assert_eq!(response["data"]["assistantMessages"], 2);
    assert_eq!(response["data"]["toolCalls"], 1);
    assert_eq!(response["data"]["toolResults"], 1);
    assert_eq!(response["data"]["totalMessages"], 4);
    assert_eq!(response["data"]["tokens"]["input"], 14);
    assert_eq!(response["data"]["tokens"]["output"], 22);
    assert_eq!(response["data"]["tokens"]["cacheRead"], 4);
    assert_eq!(response["data"]["tokens"]["cacheWrite"], 6);
    assert_eq!(response["data"]["tokens"]["total"], 46);
    assert_eq!(response["data"]["cost"], 2.0);
    assert_eq!(response["data"]["costKnown"], true);
    assert!(response["data"]["sessionId"].as_str().is_some());
    assert!(response["data"]["activeLeafId"].as_str().is_some());

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_manual_compaction_runs_through_persistent_session_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-manual-compaction";
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("first response", StopReason::Stop),
            FauxProvider::text_call("compact summary", StopReason::Stop),
        ])),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session,
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-compact\",\"type\":\"prompt\",\"message\":\"retain this history\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "pre-compaction prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"state-before-compact\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let before = read_rpc_json_matching(&mut lines, "pre-compaction state", |value| {
        value["id"] == "state-before-compact"
    })
    .await;
    assert_eq!(
        before["data"]["capabilities"]["compact"]["status"],
        "available"
    );

    input_writer
        .write_all(
            b"{\"id\":\"compact-1\",\"type\":\"compact\",\"customInstructions\":\"preserve decisions\"}\n",
        )
        .await
        .unwrap();
    let accepted = read_rpc_json_matching(&mut lines, "compact acceptance", |value| {
        value["id"] == "compact-1"
    })
    .await;
    assert_eq!(accepted["success"], true, "{accepted}");

    let ended = read_rpc_json_matching(&mut lines, "manual compaction completion", |value| {
        value["type"] == "compaction_end"
    })
    .await;
    assert_eq!(ended["reason"], "manual");
    assert_eq!(ended["aborted"], false);
    assert_eq!(ended["result"]["summary"], "compact summary");
    assert!(ended["result"]["firstKeptMessageId"].as_str().is_some());
    assert!(ended["result"]["tokensBefore"].as_u64().is_some());

    input_writer
        .write_all(b"{\"id\":\"state-after-compact\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let after = read_rpc_json_matching(&mut lines, "post-compaction state", |value| {
        value["id"] == "state-after-compact"
    })
    .await;
    assert_eq!(after["data"]["isCompacting"], false);
    assert_eq!(
        after["data"]["capabilities"]["compact"]["status"],
        "available"
    );

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_manual_compaction_rejects_non_persistent_sessions() {
    let api = "pi-coding-rpc-manual-compaction-disabled";
    let provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));
    let input = b"{\"id\":\"compact-disabled\",\"type\":\"compact\"}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let response = &parse_lines(&output)[0];
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "manual compaction requires a persistent Rust-native session"
    );
}

#[tokio::test]
async fn rpc_manual_compaction_rejects_while_an_operation_is_running() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-manual-compaction-busy";
    let release = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(PausingProvider {
            release: release.clone(),
            opened,
        }),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session: SessionRunOptions::enabled(cwd),
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-busy\",\"type\":\"prompt\",\"message\":\"hold\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "busy prompt start", |value| {
        value["type"] == "message_update"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"compact-busy\",\"type\":\"compact\"}\n")
        .await
        .unwrap();
    let response = read_rpc_json_matching(&mut lines, "busy compact rejection", |value| {
        value["id"] == "compact-busy"
    })
    .await;
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "cannot compact while another operation is running"
    );

    release.notify_one();
    drop(input_writer);
    await_rpc_task_completion(task, &release, "busy compact rpc task").await;
}

#[tokio::test]
async fn rpc_new_session_parent_forks_durable_history_and_remains_usable() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-parent-session";
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("parent response", StopReason::Stop),
            FauxProvider::text_call("child response", StopReason::Stop),
        ])),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(32 * 1024);
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions.clone());
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session,
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"parent-prompt\",\"type\":\"prompt\",\"message\":\"parent message\"}\n",
        )
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "parent prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"parent-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let parent_state = read_rpc_json_matching(&mut lines, "parent state", |value| {
        value["id"] == "parent-state"
    })
    .await;
    let parent_id = parent_state["data"]["sessionId"]
        .as_str()
        .expect("persistent parent exposes session ID")
        .to_owned();

    input_writer
        .write_all(
            b"{\"id\":\"invalid-before-fork\",\"type\":\"new_session\",\"parentSession\":\"../escape\"}\n",
        )
        .await
        .unwrap();
    let invalid = read_rpc_json_matching(&mut lines, "invalid parent rejection", |value| {
        value["id"] == "invalid-before-fork"
    })
    .await;
    assert_eq!(invalid["success"], false);
    input_writer
        .write_all(b"{\"id\":\"state-after-invalid\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let unchanged = read_rpc_json_matching(&mut lines, "state after invalid parent", |value| {
        value["id"] == "state-after-invalid"
    })
    .await;
    assert_eq!(unchanged["data"]["sessionId"], parent_id);

    let new_session = serde_json::json!({
        "id": "fork-session",
        "type": "new_session",
        "parentSession": parent_id,
    });
    input_writer
        .write_all(format!("{new_session}\n").as_bytes())
        .await
        .unwrap();
    let forked = read_rpc_json_matching(&mut lines, "forked session response", |value| {
        value["id"] == "fork-session"
    })
    .await;
    assert_eq!(forked["success"], true, "{forked}");
    assert_eq!(forked["data"]["parentSession"], parent_id);
    let child_id = forked["data"]["sessionId"]
        .as_str()
        .expect("fork response exposes child session ID")
        .to_owned();
    assert_ne!(child_id, parent_id);

    input_writer
        .write_all(b"{\"id\":\"fork-stats\",\"type\":\"get_session_stats\"}\n")
        .await
        .unwrap();
    let inherited = read_rpc_json_matching(&mut lines, "forked session stats", |value| {
        value["id"] == "fork-stats"
    })
    .await;
    assert_eq!(inherited["data"]["sessionId"], child_id);
    assert_eq!(inherited["data"]["userMessages"], 1);
    assert_eq!(inherited["data"]["assistantMessages"], 1);

    input_writer
        .write_all(b"{\"id\":\"child-prompt\",\"type\":\"prompt\",\"message\":\"child message\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "child prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"child-stats\",\"type\":\"get_session_stats\"}\n")
        .await
        .unwrap();
    let child_stats = read_rpc_json_matching(&mut lines, "child session stats", |value| {
        value["id"] == "child-stats"
    })
    .await;
    assert_eq!(child_stats["data"]["sessionId"], child_id);
    assert_eq!(child_stats["data"]["userMessages"], 2);
    assert_eq!(child_stats["data"]["assistantMessages"], 2);

    drop(input_writer);
    task.await.unwrap();

    let events = std::fs::read_to_string(sessions.join(&child_id).join("events.jsonl")).unwrap();
    let provenance = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| event["kind"] == "session.forked")
        .expect("forked session persists provenance");
    assert_eq!(provenance["data"]["source_session_id"], parent_id);
    assert!(provenance["data"]["source_leaf_id"].as_str().is_some());
}

#[tokio::test]
async fn rpc_new_session_parent_rejects_missing_and_invalid_session_ids() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-parent-session-errors";
    let provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));
    let input = b"{\"id\":\"missing-parent\",\"type\":\"new_session\",\"parentSession\":\"sess_missing\"}\n\
                  {\"id\":\"invalid-parent\",\"type\":\"new_session\",\"parentSession\":\"../escape\"}\n\
                  {\"id\":\"blank-parent\",\"type\":\"new_session\",\"parentSession\":\" \"}\n";
    let mut output = Vec::new();
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions);

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(provider_guard.ai_client()),
            session,
        },
    )
    .await
    .unwrap();

    let responses = parse_lines(&output);
    assert_eq!(responses.len(), 3);
    for response in &responses {
        assert_eq!(response["success"], false, "{response}");
    }
    assert_eq!(responses[0]["data"]["code"], "session");
    assert_eq!(responses[1]["data"]["code"], "session");
    assert_eq!(responses[2]["data"]["code"], "input");
}

#[tokio::test]
async fn rpc_lists_and_approves_pending_tool_authorization_before_execution() {
    let api = "pi-coding-rpc-tool-authorization";
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: Vec::new(),
                    thinking_deltas: Vec::new(),
                    tool_calls: vec![FauxToolCall {
                        id: "tool-rpc-mutate".into(),
                        name: "rpc_mutate".into(),
                        deltas: vec!["{}".into()],
                        final_arguments: serde_json::json!({}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxProvider::text_call("approved and completed", StopReason::Stop),
        ])),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: vec![rpc_mutation_tool(executions.clone())],
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        executions.load(Ordering::SeqCst)
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-auth\",\"type\":\"prompt\",\"message\":\"mutate\"}\n")
        .await
        .unwrap();
    let required = read_rpc_json_matching(&mut lines, "tool authorization request", |value| {
        value["type"] == "tool_authorization_required"
    })
    .await;
    let authorization_id = required["request"]["authorizationId"]
        .as_str()
        .expect("authorization request exposes its identity")
        .to_owned();

    input_writer
        .write_all(b"{\"id\":\"state-auth\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let state = read_rpc_json_matching(&mut lines, "authorization state response", |value| {
        value["type"] == "response" && value["command"] == "get_state"
    })
    .await;
    assert_eq!(
        state["data"]["pendingToolAuthorizations"][0]["authorizationId"],
        authorization_id
    );

    input_writer
        .write_all(b"{\"id\":\"list-auth\",\"type\":\"list_tool_authorizations\"}\n")
        .await
        .unwrap();
    let listed = read_rpc_json_matching(&mut lines, "list authorization response", |value| {
        value["type"] == "response" && value["command"] == "list_tool_authorizations"
    })
    .await;
    assert_eq!(listed["success"], true, "{listed}");
    assert_eq!(
        listed["data"]["authorizations"][0]["authorizationId"],
        authorization_id
    );

    let approve = serde_json::json!({
        "id": "approve-auth",
        "type": "approve_tool_authorization",
        "authorizationId": authorization_id,
        "scope": "once"
    });
    input_writer
        .write_all(format!("{approve}\n").as_bytes())
        .await
        .unwrap();

    let mut observed = Vec::new();
    loop {
        let value = read_rpc_json_line(&mut lines, "approved tool completion").await;
        let terminal = value["type"] == "agent_end";
        observed.push(value);
        if terminal {
            break;
        }
    }
    assert!(observed.iter().any(|value| {
        value["type"] == "response"
            && value["command"] == "approve_tool_authorization"
            && value["success"] == true
    }));
    assert!(
        observed
            .iter()
            .any(|value| value["type"] == "tool_authorization_approved")
    );
    assert!(
        observed
            .iter()
            .any(|value| value["type"] == "tool_execution_start")
    );
    assert!(
        observed
            .iter()
            .any(|value| value["type"] == "tool_execution_end")
    );

    drop(input_writer);
    assert_eq!(task.await.unwrap(), 1);
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

impl pi_ai::api::provider::ApiProvider for PausingProvider {
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

impl pi_ai::api::provider::ApiProvider for AbortAwareProvider {
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

struct RecordingProvider {
    contexts: Arc<Mutex<Vec<Context>>>,
}

struct ScriptedUsageProvider {
    contexts: Arc<Mutex<Vec<Context>>>,
    responses: Mutex<VecDeque<(String, Usage)>>,
}

impl pi_ai::api::provider::ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.contexts.lock().unwrap().push(ctx);
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("recording", &model_id);
            message.provider = Some("recording".into());
            message.content.push(ContentBlock::Text {
                text: "multimodal complete".into(),
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

impl pi_ai::api::provider::ApiProvider for ScriptedUsageProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.contexts.lock().unwrap().push(ctx);
        let (text, usage) = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted usage provider response");
        let model_id = model.id.clone();
        Box::pin(async_stream::stream! {
            let mut message = AssistantMessage::empty("scripted-usage", &model_id);
            message.provider = Some("scripted-usage".into());
            message.content.push(ContentBlock::Text {
                text,
                text_signature: None,
            });
            message.usage = usage;
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

#[tokio::test]
async fn rpc_multimodal_prompt_reaches_provider_and_durable_session_content() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-multimodal-prompt";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(RecordingProvider {
            contexts: contexts.clone(),
        }),
    );
    let image_data = "aW1hZ2UtZml4dHVyZQ==";
    let input = format!(
        "{{\"id\":\"prompt-image\",\"type\":\"prompt\",\"message\":\"describe image\",\"images\":[{{\"type\":\"image\",\"data\":\"{image_data}\",\"mimeType\":\"image/png\"}}]}}\n\
         {{\"id\":\"image-state\",\"type\":\"get_state\"}}\n"
    );
    let mut output = Vec::new();
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions.clone());

    run_rpc_mode_for_io(
        input.as_bytes(),
        &mut output,
        CliRunOptions {
            model_override: Some(multimodal_faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(provider_guard.ai_client()),
            session,
        },
    )
    .await
    .unwrap();

    let values = parse_lines(&output);
    let prompt = values
        .iter()
        .find(|value| value["id"] == "prompt-image")
        .unwrap();
    assert_eq!(prompt["success"], true, "{prompt}");
    let state = values
        .iter()
        .find(|value| value["id"] == "image-state")
        .unwrap();
    let session_id = state["data"]["sessionId"].as_str().unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 1);
    let user_content = contexts[0]
        .messages
        .iter()
        .find_map(|message| match message {
            Message::User { content } => Some(content),
            _ => None,
        })
        .expect("provider request contains a user message");
    assert!(
        user_content.iter().any(
            |block| matches!(block, ContentBlock::Text { text, .. } if text == "describe image")
        )
    );
    assert!(user_content.iter().any(|block| matches!(
        block,
        ContentBlock::Image { data, mime_type }
            if data == image_data && mime_type == "image/png"
    )));
    drop(contexts);

    let events = std::fs::read_to_string(sessions.join(session_id).join("events.jsonl")).unwrap();
    assert!(events.contains(image_data), "{events}");
    assert!(events.contains("image/png"), "{events}");
}

#[tokio::test]
async fn rpc_prompt_images_reject_non_image_content_before_provider_execution() {
    let api = "pi-coding-rpc-invalid-image-content";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(RecordingProvider {
            contexts: contexts.clone(),
        }),
    );
    let input = b"{\"id\":\"invalid-image\",\"type\":\"prompt\",\"message\":\"hello\",\"images\":[{\"type\":\"text\",\"text\":\"not an image\"}]}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(multimodal_faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let response = &parse_lines(&output)[0];
    assert_eq!(response["success"], false);
    assert!(
        response["error"]
            .as_str()
            .unwrap()
            .contains("must contain only image content blocks")
    );
    assert!(contexts.lock().unwrap().is_empty());
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

impl pi_ai::api::provider::ApiProvider for BlockingTwoTurnProvider {
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
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

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
                ai_client: Some(_provider_guard.ai_client()),
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
    let bytes_read = read_rpc_output_bytes(
        &mut output_reader,
        &mut buf,
        "rpc response before stdin EOF",
    )
    .await;

    let lines = parse_lines(&buf[..bytes_read]);
    assert_eq!(lines[0]["id"], "s1");
    assert_eq!(lines[0]["command"], "get_state");
    assert_eq!(lines[0]["success"], true);

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_lifecycle_detach_returns_typed_idempotent_status() {
    let api = "pi-coding-rpc-lifecycle-detach";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input =
        b"{\"id\":\"detach-1\",\"type\":\"detach\"}\n{\"id\":\"detach-2\",\"type\":\"detach\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        parse_lines(&output),
        vec![
            serde_json::json!({
                "type": "response",
                "id": "detach-1",
                "command": "detach",
                "success": true,
                "data": {"status": "already_detached"}
            }),
            serde_json::json!({
                "type": "response",
                "id": "detach-2",
                "command": "detach",
                "success": true,
                "data": {"status": "already_detached"}
            })
        ]
    );
}

#[tokio::test]
async fn rpc_lifecycle_detach_during_prompt_is_observable_without_cancelling_work() {
    let api = "pi-coding-rpc-lifecycle-active-detach";
    let release = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened,
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(_provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-1\",\"type\":\"prompt\",\"message\":\"continue\"}\n")
        .await
        .unwrap();
    let _admitted = read_rpc_json_matching(&mut lines, "admitted prompt event", |value| {
        value["type"] == "message_update"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"detach-1\",\"type\":\"detach\"}\n")
        .await
        .unwrap();

    let lifecycle = read_rpc_json_matching(&mut lines, "detach lifecycle event", |value| {
        value["type"] == "client_detached"
    })
    .await;
    assert_eq!(lifecycle["status"], "detached");
    let detached = read_rpc_json_matching(&mut lines, "detach response", |value| {
        value["id"] == "detach-1"
    })
    .await;
    assert_eq!(detached["success"], true);
    assert_eq!(detached["data"], serde_json::json!({"status": "detached"}));

    release.notify_one();
    let _completed = read_rpc_json_matching(&mut lines, "detached prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_lifecycle_shutdown_waits_for_owner_restoration_and_uses_stable_rejection_code() {
    let api = "pi-coding-rpc-lifecycle-shutdown";
    let release = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened,
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(_provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"prompt-1\",\"type\":\"prompt\",\"message\":\"hold\"}\n")
        .await
        .unwrap();
    let _prompt_started = read_rpc_json_matching(&mut lines, "prompt start", |value| {
        value["type"] == "message_update"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"shutdown-1\",\"type\":\"shutdown\"}\n")
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            read_rpc_json_matching(&mut lines, "premature shutdown response", |value| {
                value["command"] == "shutdown"
            })
        )
        .await
        .is_err(),
        "shutdown response must wait for the admitted operation and owner restoration"
    );

    release.notify_one();
    let shutdown = read_rpc_json_matching(&mut lines, "shutdown response", |value| {
        value["command"] == "shutdown"
    })
    .await;
    assert_eq!(shutdown["success"], true);
    assert_eq!(shutdown["data"], serde_json::json!({"status": "shut_down"}));

    input_writer
        .write_all(b"{\"id\":\"detach-after-shutdown\",\"type\":\"detach\"}\n")
        .await
        .unwrap();
    let rejected = read_rpc_json_matching(&mut lines, "shutdown detach rejection", |value| {
        value["id"] == "detach-after-shutdown"
    })
    .await;
    assert_eq!(rejected["success"], false);
    assert_eq!(rejected["data"]["code"], "runtime_shut_down");

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_state_reports_capabilities_when_idle() {
    let api = "pi-coding-rpc-capabilities-idle";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let capabilities = &lines[0]["data"]["capabilities"];
    assert_eq!(capabilities["prompt"]["status"], "available");
    assert_eq!(capabilities["abort"]["status"], "disabled");
    assert_eq!(
        capabilities["abort"]["reason"],
        "no cancellable operation is running"
    );
    for capability in ["steer", "followUp"] {
        assert_eq!(capabilities[capability]["status"], "disabled");
        assert_eq!(capabilities[capability]["reason"], "no prompt is running");
    }
    assert_eq!(capabilities["tools"]["status"], "available");
    assert_eq!(capabilities["agentProfiles"]["status"], "available");
    assert_eq!(capabilities["teamProfiles"]["status"], "available");
    assert_eq!(capabilities["delegation"]["status"], "available");
    assert_eq!(
        capabilities["delegation"]["rendering"]["mode"],
        "folded_block"
    );
    assert_eq!(
        capabilities["delegation"]["rendering"]["eventFamily"],
        "delegation"
    );
    assert_eq!(
        capabilities["delegation"]["rendering"]["payloadField"],
        "foldedBlock"
    );
    assert_eq!(
        capabilities["delegation"]["rendering"]["upsertKey"],
        "toolCallId"
    );
    assert_eq!(
        capabilities["delegation"]["rendering"]["lifecycleEvents"],
        serde_json::json!([
            "delegation_requested",
            "delegation_rejected",
            "delegation_approved",
            "delegation_confirmation_required",
            "delegation_started",
            "delegation_completed",
            "delegation_failed"
        ])
    );
    for capability in ["pluginReload", "selfHealingEdit"] {
        assert_eq!(capabilities[capability]["status"], "disabled");
        assert_eq!(
            capabilities[capability]["reason"],
            "requires persistent Rust-native session"
        );
    }
    assert_eq!(capabilities["compact"]["status"], "disabled");
    assert_eq!(
        capabilities["compact"]["reason"],
        "requires persistent Rust-native session"
    );
    let unsupported = [
        ("fork", "RPC protocol 2.1 does not expose a fork command"),
        (
            "cloneSession",
            "RPC protocol 2.1 does not expose a cloneSession command",
        ),
        (
            "branchSummary",
            "RPC protocol 2.1 does not expose a branchSummary command",
        ),
        (
            "switchSession",
            "RPC protocol 2.1 does not expose a switchSession command",
        ),
        (
            "export",
            "RPC protocol 2.1 does not expose an export command",
        ),
    ];
    for (capability, reason) in unsupported {
        assert_eq!(capabilities[capability]["status"], "unsupported");
        assert_eq!(capabilities[capability]["reason"], reason);
    }
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
            ai_client: None,
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
            ai_client: None,
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
            ai_client: None,
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
    let _provider_guard = ProviderGuard::register(
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
            ai_client: Some(_provider_guard.ai_client()),
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
            ai_client: None,
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
            ai_client: None,
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
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("from agent")));
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
            ai_client: Some(_provider_guard.ai_client()),
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
}

#[tokio::test]
async fn rpc_prompt_runs_while_agent_invocation_is_backgrounded() {
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

    let api = "pi-coding-rpc-agent-background-prompt";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (agent_started_tx, agent_started_rx) = oneshot::channel();
    let (release_agent_tx, release_agent_rx) = oneshot::channel();
    let provider =
        BlockingTwoTurnProvider::new(Arc::clone(&contexts), agent_started_tx, release_agent_rx);
    let provider_guard = ProviderGuard::register(api, Arc::new(provider));
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"a1\",\"type\":\"invoke_agent\",\"profileId\":\"coder\",\"task\":\"blocked background work\"}\n",
        )
        .await
        .unwrap();
    let agent_response = read_rpc_json_matching(&mut lines, "invoke_agent response", |value| {
        value["type"] == "response" && value["command"] == "invoke_agent"
    })
    .await;
    assert_eq!(agent_response["success"], true, "{agent_response}");
    let agent_operation_id = agent_response["data"]["operationId"]
        .as_str()
        .expect("background invocation response exposes operationId")
        .to_owned();
    wait_for_rpc_provider_start(agent_started_rx, "background agent provider start").await;

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let state_response = read_rpc_json_matching(&mut lines, "background session state", |value| {
        value["type"] == "response" && value["command"] == "get_state"
    })
    .await;
    let reconnect = serde_json::json!({
        "id": "r1",
        "type": "prompt",
        "message": "resume background stream",
        "afterSnapshotCursor": {
            "streamId": state_response["data"]["eventStreamId"],
            "snapshotProtocolMajor": state_response["data"]["snapshotVersion"]["major"],
            "lastEventSequence": state_response["data"]["snapshotSequence"],
            "capabilityGeneration": state_response["data"]["capabilityGeneration"],
        }
    });
    input_writer
        .write_all(format!("{reconnect}\n").as_bytes())
        .await
        .unwrap();
    let reconnect_response =
        read_rpc_json_matching(&mut lines, "background reconnect response", |value| {
            value["type"] == "response" && value["command"] == "prompt"
        })
        .await;
    assert_eq!(reconnect_response["success"], true, "{reconnect_response}");

    input_writer
        .write_all(b"{\"id\":\"x1\",\"type\":\"abort\"}\n")
        .await
        .unwrap();
    let abort_response = read_rpc_json_matching(&mut lines, "background abort response", |value| {
        value["type"] == "response" && value["command"] == "abort"
    })
    .await;
    assert_eq!(abort_response["success"], true, "{abort_response}");
    assert_eq!(abort_response["data"]["cancelled"], false);

    input_writer
        .write_all(b"{\"id\":\"n1\",\"type\":\"new_session\"}\n")
        .await
        .unwrap();
    let new_session_response =
        read_rpc_json_matching(&mut lines, "active-root new_session response", |value| {
            value["type"] == "response" && value["command"] == "new_session"
        })
        .await;
    assert_eq!(
        new_session_response["success"], false,
        "{new_session_response}"
    );

    input_writer
        .write_all(b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"foreground work\"}\n")
        .await
        .unwrap();
    let prompt_response =
        read_rpc_json_matching(&mut lines, "foreground prompt response", |value| {
            value["type"] == "response" && value["command"] == "prompt"
        })
        .await;
    assert_eq!(prompt_response["success"], true, "{prompt_response}");
    let prompt_output = read_rpc_json_matching(&mut lines, "foreground prompt output", |value| {
        value.to_string().contains("second")
    })
    .await;
    assert!(prompt_output.to_string().contains("second"));
    assert_eq!(contexts.lock().unwrap().len(), 2);

    let targeted_abort = serde_json::json!({
        "id": "x2",
        "type": "abort",
        "operationId": agent_operation_id.clone(),
    });
    input_writer
        .write_all(format!("{targeted_abort}\n").as_bytes())
        .await
        .unwrap();
    let targeted_abort_response =
        read_rpc_json_matching(&mut lines, "targeted background abort response", |value| {
            value["type"] == "response" && value["id"] == "x2"
        })
        .await;
    assert_eq!(
        targeted_abort_response["data"]["cancelled"], true,
        "{targeted_abort_response}"
    );
    // The targeted abort may already have dropped the blocked provider stream.
    let _ = release_agent_tx.send(());
    drop(input_writer);
    let invocation_abort =
        read_rpc_json_matching(&mut lines, "background invocation abort", |value| {
            value["type"] == "agent_invocation_abort"
        })
        .await;
    assert_eq!(invocation_abort["operationId"], agent_operation_id);
    drop(lines);
    tokio::time::timeout(RPC_TASK_SHUTDOWN_TIMEOUT, task)
        .await
        .expect("RPC task completes after foreground and background operations drain")
        .unwrap();
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
    let _provider_guard = ProviderGuard::register(
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
                responses: vec![text_response("child result")],
                stop_reason: StopReason::Stop,
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let set_default_response =
        read_rpc_json_matching(&mut lines, "set_default_agent_profile response", |value| {
            value["type"] == "response" && value["command"] == "set_default_agent_profile"
        })
        .await;
    assert_eq!(set_default_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();
    let required = {
        let mut seen = Vec::new();
        loop {
            let value = read_rpc_json_line(&mut lines, "delegation authorization event").await;
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
            if value["type"] == "tool_authorization_required" {
                break value;
            }
            if value["type"] == "agent_end" {
                panic!("prompt ended before delegation authorization event: {seen:?}");
            }
        }
    };
    let authorization_id = required["request"]["authorizationId"].as_str().unwrap();
    let tool_call_id = required["request"]["toolCallId"].as_str().unwrap();
    let approve_command = serde_json::json!({
        "id": "a1",
        "type": "approve_tool_authorization",
        "authorizationId": authorization_id,
        "scope": "once",
    })
    .to_string()
        + "\n";
    input_writer
        .write_all(approve_command.as_bytes())
        .await
        .unwrap();

    let approve_response =
        read_rpc_json_matching(&mut lines, "approve_tool_authorization response", |value| {
            value["type"] == "response" && value["command"] == "approve_tool_authorization"
        })
        .await;
    assert_eq!(approve_response["success"], true);

    let mut saw_approved = false;
    let mut saw_completed = false;
    {
        let mut seen = Vec::new();
        loop {
            let context = format!(
                "delegation approval completion: saw_approved={saw_approved}, saw_completed={saw_completed}, seen={seen:?}"
            );
            let value = read_rpc_json_line(&mut lines, &context).await;
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
                assert_eq!(value["foldedBlock"]["targetId"], "coder");
                saw_approved = true;
            }
            if value["type"] == "delegation_completed"
                && value["targetId"] == "coder"
                && value["finalText"] == "child result"
            {
                assert_eq!(value["foldedBlock"]["toolCallId"], tool_call_id);
                assert_eq!(value["foldedBlock"]["status"], "completed");
                assert_eq!(
                    value["foldedBlock"]["childOperationId"],
                    value["childOperationId"]
                );
                assert_eq!(value["foldedBlock"]["summary"], "completed: child result");
                assert_eq!(value["foldedBlock"]["isError"], false);
                saw_completed = true;
            }
            if saw_approved && saw_completed {
                break;
            }
        }
    }

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_child_tool_authorization_is_scoped_to_child_operation() {
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
tools = ["rpc_mutate"]
"#,
    );

    let api = "pi-coding-rpc-child-tool-authorization";
    let provider_guard = ProviderGuard::register(
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
                responses: vec![FauxResponse {
                    text_deltas: Vec::new(),
                    thinking_deltas: Vec::new(),
                    tool_calls: vec![FauxToolCall {
                        id: "tool-child-mutate".into(),
                        name: "rpc_mutate".into(),
                        deltas: vec!["{}".into()],
                        final_arguments: serde_json::json!({}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxProvider::text_call("child result", StopReason::Stop),
            FauxProvider::text_call("parent ready", StopReason::Stop),
        ])),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(large_context_faux_model(api)),
                tools: vec![rpc_mutation_tool(executions.clone())],
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
        executions.load(Ordering::SeqCst)
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"profile\",\"type\":\"set_default_agent_profile\",\"profileId\":\"delegating-planner\"}\n",
        )
        .await
        .unwrap();
    read_rpc_json_matching(&mut lines, "profile response", |value| {
        value["type"] == "response" && value["command"] == "set_default_agent_profile"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"prompt\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();

    let mut child_operation_id = None;
    let required = loop {
        let value = read_rpc_json_line(&mut lines, "child authorization event").await;
        if value["type"] == "delegation_started" {
            child_operation_id = value["childOperationId"].as_str().map(ToOwned::to_owned);
        }
        if value["type"] == "tool_authorization_required" {
            break value;
        }
    };
    let child_operation_id = child_operation_id.expect("delegation start exposes child operation");
    assert_ne!(required["request"]["operationId"], child_operation_id);
    assert_eq!(required["request"]["toolCallId"], "tool-child-mutate");
    assert_eq!(required["request"]["toolName"], "rpc_mutate");

    let approve = serde_json::json!({
        "id": "approve-child",
        "type": "approve_tool_authorization",
        "authorizationId": required["request"]["authorizationId"],
        "scope": "once",
    })
    .to_string()
        + "\n";
    input_writer.write_all(approve.as_bytes()).await.unwrap();
    read_rpc_json_matching(&mut lines, "child authorization approval", |value| {
        value["type"] == "response" && value["command"] == "approve_tool_authorization"
    })
    .await;
    read_rpc_json_matching(&mut lines, "parent prompt completion", |value| {
        value["type"] == "agent_end" && value["operationId"].as_str() != Some(&child_operation_id)
    })
    .await;

    drop(input_writer);
    assert_eq!(task.await.unwrap(), 1);
}

#[tokio::test]
async fn rpc_parent_abort_cancels_pending_child_tool_authorization() {
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
tools = ["rpc_mutate"]
"#,
    );

    let api = "pi-coding-rpc-child-tool-authorization-abort";
    let provider_guard = ProviderGuard::register(
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
                responses: vec![FauxResponse {
                    text_deltas: Vec::new(),
                    thinking_deltas: Vec::new(),
                    tool_calls: vec![FauxToolCall {
                        id: "tool-child-mutate".into(),
                        name: "rpc_mutate".into(),
                        deltas: vec!["{}".into()],
                        final_arguments: serde_json::json!({}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
        ])),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(large_context_faux_model(api)),
                tools: vec![rpc_mutation_tool(executions.clone())],
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session: SessionRunOptions::disabled(cwd),
            },
        )
        .await
        .unwrap();
        executions.load(Ordering::SeqCst)
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"profile\",\"type\":\"set_default_agent_profile\",\"profileId\":\"delegating-planner\"}\n",
        )
        .await
        .unwrap();
    read_rpc_json_matching(&mut lines, "profile response", |value| {
        value["type"] == "response" && value["command"] == "set_default_agent_profile"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"prompt\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();

    let required = read_rpc_json_matching(&mut lines, "child authorization request", |value| {
        value["type"] == "tool_authorization_required"
            && value["request"]["toolCallId"] == "tool-child-mutate"
    })
    .await;
    let child_authorization_id = required["request"]["authorizationId"]
        .as_str()
        .unwrap()
        .to_owned();

    input_writer
        .write_all(b"{\"id\":\"abort-child-wait\",\"type\":\"abort\"}\n")
        .await
        .unwrap();
    let mut saw_abort_response = false;
    let mut saw_child_authorization_cancelled = false;
    while !saw_abort_response || !saw_child_authorization_cancelled {
        let value = read_rpc_json_line(&mut lines, "child authorization abort convergence").await;
        if value["type"] == "response" && value["command"] == "abort" {
            assert_eq!(value["success"], true);
            assert_eq!(value["data"]["cancelled"], true);
            saw_abort_response = true;
        }
        if value["type"] == "tool_authorization_cancelled"
            && value["authorizationId"] == child_authorization_id
        {
            saw_child_authorization_cancelled = true;
        }
    }

    drop(input_writer);
    let executions = tokio::time::timeout(RPC_TASK_SHUTDOWN_TIMEOUT, task)
        .await
        .expect("RPC task should stop after parent abort")
        .unwrap();
    assert_eq!(executions, 0);
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
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let set_default_response =
        read_rpc_json_matching(&mut lines, "set_default_agent_profile response", |value| {
            value["type"] == "response" && value["command"] == "set_default_agent_profile"
        })
        .await;
    assert_eq!(set_default_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"plan feature\"}\n")
        .await
        .unwrap();
    let required = read_rpc_json_matching(&mut lines, "delegation authorization event", |value| {
        value["type"] == "tool_authorization_required"
    })
    .await;
    let authorization_id = required["request"]["authorizationId"].as_str().unwrap();

    let reject_command = serde_json::json!({
        "id": "r1",
        "type": "deny_tool_authorization",
        "authorizationId": authorization_id,
        "reason": "not now",
    })
    .to_string()
        + "\n";
    input_writer
        .write_all(reject_command.as_bytes())
        .await
        .unwrap();

    let reject_response =
        read_rpc_json_matching(&mut lines, "deny_tool_authorization response", |value| {
            value["type"] == "response" && value["command"] == "deny_tool_authorization"
        })
        .await;
    assert_eq!(reject_response["success"], true);

    let rejected_event = read_rpc_json_matching(&mut lines, "delegation_rejected event", |value| {
        value["type"] == "delegation_rejected"
    })
    .await;
    assert_eq!(rejected_event["targetId"], "coder");
    assert_eq!(rejected_event["reason"], "not now");

    read_rpc_json_matching(&mut lines, "prompt completion", |value| {
        value["type"] == "agent_end"
    })
    .await;

    drop(input_writer);
    task.await.unwrap();
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
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("member result")));
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
            ai_client: Some(_provider_guard.ai_client()),
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
            ai_client: None,
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
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let prompt_response =
        read_rpc_json_line(&mut lines, "prompt response before provider completes").await;
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(
            b"{\"id\":\"s1\",\"type\":\"set_default_agent_profile\",\"profileId\":\"default\"}\n",
        )
        .await
        .unwrap();

    let response = read_rpc_json_matching(
        &mut lines,
        "set_default_agent_profile rejection while prompt is running",
        |value| value["type"] == "response" && value["command"] == "set_default_agent_profile",
    )
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(response["id"], "s1");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "cannot set default agent profile while agent is streaming"
    );
}

#[tokio::test]
async fn rpc_list_agent_profiles_rejects_while_prompt_running() {
    let api = "pi-coding-rpc-list-agents-busy";
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let prompt_response =
        read_rpc_json_line(&mut lines, "prompt response before provider completes").await;
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"a1\",\"type\":\"list_agent_profiles\"}\n")
        .await
        .unwrap();

    let response = read_rpc_json_matching(
        &mut lines,
        "list_agent_profiles rejection while prompt is running",
        |value| value["type"] == "response" && value["command"] == "list_agent_profiles",
    )
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(response["id"], "a1");
    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "cannot list agent profiles while agent is streaming"
    );
}

#[tokio::test]
async fn rpc_state_reports_prompt_busy_while_running() {
    let api = "pi-coding-rpc-capabilities-busy";
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let prompt_response =
        read_rpc_json_line(&mut lines, "prompt response before provider completes").await;
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state = read_rpc_json_matching(&mut lines, "get_state response", |value| {
        value["type"] == "response" && value["command"] == "get_state"
    })
    .await;

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
    assert_eq!(capabilities["agentProfiles"]["status"], "available");
    assert_eq!(capabilities["teamProfiles"]["status"], "available");
    assert_eq!(capabilities["delegation"]["status"], "available");
    assert_eq!(capabilities["selfHealingEdit"]["status"], "disabled");
    assert_eq!(
        capabilities["selfHealingEdit"]["reason"],
        "requires persistent Rust-native session"
    );
}

#[tokio::test]
async fn rpc_parse_error_keeps_process_alive_for_next_command() {
    let api = "pi-coding-rpc-parse";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{bad json}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
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
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

    let input = b"{\"id\":\"m1\",\"type\":\"set_model\",\"provider\":\"faux\",\"modelId\":\"x\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
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
}

#[tokio::test]
async fn rpc_prompt_returns_response_then_agent_events() {
    let api = "pi-coding-rpc-prompt";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let input = b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
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
}

#[tokio::test]
async fn rpc_streams_agent_events_before_prompt_finishes() {
    let api = "pi-coding-rpc-live-events";
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let response =
        read_rpc_json_line(&mut lines, "prompt response before provider completes").await;
    assert_eq!(response["id"], "p1");
    assert_eq!(response["command"], "prompt");
    assert_eq!(response["success"], true);

    let event = read_rpc_json_line(&mut lines, "agent event before prompt finishes").await;
    release.notify_one();
    drop(input_writer);
    task.await.unwrap();
    assert_eq!(event["type"], "agent_start");
}

#[tokio::test]
async fn rpc_abort_cancels_running_prompt() {
    let api = "pi-coding-rpc-abort";
    let cancelled = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let prompt_response =
        read_rpc_json_line(&mut lines, "prompt response before provider completes").await;
    assert_eq!(prompt_response["success"], true);

    input_writer
        .write_all(b"{\"id\":\"a1\",\"type\":\"abort\"}\n")
        .await
        .unwrap();

    let abort_response = read_rpc_json_matching(
        &mut lines,
        "abort response while prompt is running",
        |value| value["type"] == "response" && value["command"] == "abort",
    )
    .await;

    drop(input_writer);
    await_rpc_task_completion(task, &release, "rpc task to finish after abort").await;

    assert_eq!(abort_response["id"], "a1");
    assert_eq!(abort_response["success"], true);
    assert_eq!(abort_response["data"]["cancelled"], true);
    // The operation may be cancelled before the provider stream is polled;
    // the RPC response is the stable cancellation contract here.
}

#[tokio::test]
async fn rpc_steer_while_coding_prompt_running_sends_control() {
    let api = "pi-coding-rpc-steer-live";
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let _ = read_rpc_line(&mut lines, "initial prompt response").await;
    let _ = read_rpc_line(&mut lines, "initial agent_start event").await;

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"steer\",\"message\":\"look here\"}\n")
        .await
        .unwrap();

    let response = read_rpc_json_matching(
        &mut lines,
        "steer response while prompt is running",
        |value| value["type"] == "response" && value["command"] == "steer",
    )
    .await;

    assert_eq!(response["success"], true);
    release.notify_one();
    drop(input_writer);
    read_rpc_json_matching(
        &mut lines,
        "agent_end after releasing paused provider",
        |value| value["type"] == "agent_end",
    )
    .await;
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_follow_up_prompt_while_coding_prompt_running_sends_control() {
    let api = "pi-coding-rpc-follow-up-live";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let _provider_guard = ProviderGuard::register(
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
                model_override: Some(multimodal_faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(_provider_guard.ai_client()),
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
    let _ = read_rpc_line(&mut lines, "initial prompt response").await;
    let _ = read_rpc_line(&mut lines, "initial agent_start event").await;
    wait_for_rpc_provider_start(started_rx, "provider first turn to start").await;

    input_writer
        .write_all(
            b"{\"id\":\"f1\",\"type\":\"prompt\",\"message\":\"next\",\"images\":[{\"type\":\"image\",\"data\":\"Zm9sbG93LXVw\",\"mimeType\":\"image/png\"}],\"streamingBehavior\":\"followUp\"}\n",
        )
        .await
        .unwrap();

    let response = read_rpc_json_matching(
        &mut lines,
        "follow-up response while prompt is running",
        |value| value["type"] == "response" && value["id"] == "f1",
    )
    .await;

    assert_eq!(response["success"], true);
    release_tx.send(()).unwrap();
    drop(input_writer);
    read_rpc_json_matching(
        &mut lines,
        "agent_end after releasing paused provider",
        |value| value["type"] == "agent_end",
    )
    .await;
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
                )) && content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Image { data, mime_type }
                        if data == "Zm9sbG93LXVw" && mime_type == "image/png"
                ))
        )),
        "{:#?}",
        contexts[1].messages
    );
}

#[tokio::test]
async fn rpc_direct_steer_and_follow_up_images_reach_later_provider_turns() {
    let api = "pi-coding-rpc-direct-multimodal-controls";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(BlockingTwoTurnProvider::new(
            contexts.clone(),
            started_tx,
            release_rx,
        )),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(2048);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(multimodal_faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"control-base\",\"type\":\"prompt\",\"message\":\"base\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "multimodal control prompt start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    wait_for_rpc_provider_start(started_rx, "multimodal control provider start").await;

    input_writer
        .write_all(
            b"{\"id\":\"steer-image\",\"type\":\"steer\",\"message\":\"steer image\",\"images\":[{\"type\":\"image\",\"data\":\"c3RlZXI=\",\"mimeType\":\"image/png\"}]}\n",
        )
        .await
        .unwrap();
    input_writer
        .write_all(
            b"{\"id\":\"follow-image\",\"type\":\"follow_up\",\"message\":\"follow image\",\"images\":[{\"type\":\"image\",\"data\":\"Zm9sbG93\",\"mimeType\":\"image/jpeg\"}]}\n",
        )
        .await
        .unwrap();
    for id in ["steer-image", "follow-image"] {
        let response = read_rpc_json_matching(&mut lines, "multimodal control response", |value| {
            value["id"] == id
        })
        .await;
        assert_eq!(response["success"], true, "{response}");
    }

    release_tx.send(()).unwrap();
    let _ = read_rpc_json_matching(&mut lines, "multimodal controls completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    drop(input_writer);
    task.await.unwrap();

    let contexts = contexts.lock().unwrap();
    assert!(contexts.len() >= 2, "{contexts:#?}");
    let later_messages = contexts[1..]
        .iter()
        .flat_map(|context| context.messages.iter())
        .collect::<Vec<_>>();
    assert!(
        later_messages.iter().any(|message| matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Image { data, mime_type }
                        if data == "c3RlZXI=" && mime_type == "image/png"
                ))
        )),
        "{later_messages:#?}"
    );
    assert!(
        later_messages.iter().any(|message| matches!(
            message,
            Message::User { content }
                if content.iter().any(|block| matches!(
                    block,
                    ContentBlock::Image { data, mime_type }
                        if data == "Zm9sbG93" && mime_type == "image/jpeg"
                ))
        )),
        "{later_messages:#?}"
    );
}

#[tokio::test]
async fn rpc_idle_image_controls_are_queued_into_the_next_prompt() {
    let api = "pi-coding-rpc-idle-multimodal-controls";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(BlockingTwoTurnProvider::new(
            contexts.clone(),
            started_tx,
            release_rx,
        )),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(multimodal_faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"idle-steer-image\",\"type\":\"steer\",\"message\":\"steer\",\"images\":[{\"type\":\"image\",\"data\":\"c3RlZXI=\",\"mimeType\":\"image/png\"}]}\n",
        )
        .await
        .unwrap();
    let steer = read_rpc_json_matching(&mut lines, "idle steer response", |value| {
        value["id"] == "idle-steer-image"
    })
    .await;
    assert_eq!(steer["success"], true, "{steer}");
    let steer_queue = read_rpc_json_matching(&mut lines, "idle steer queue", |value| {
        value["type"] == "queue_update"
    })
    .await;
    assert_eq!(steer_queue["steering"][0], "steer\n[image:image/png]");
    assert!(!steer_queue.to_string().contains("c3RlZXI="));

    input_writer
        .write_all(
            b"{\"id\":\"idle-follow-image\",\"type\":\"follow_up\",\"message\":\"follow\",\"images\":[{\"type\":\"image\",\"data\":\"Zm9sbG93\",\"mimeType\":\"image/jpeg\"}]}\n",
        )
        .await
        .unwrap();
    let follow = read_rpc_json_matching(&mut lines, "idle follow response", |value| {
        value["id"] == "idle-follow-image"
    })
    .await;
    assert_eq!(follow["success"], true, "{follow}");
    let follow_queue = read_rpc_json_matching(&mut lines, "idle follow queue", |value| {
        value["type"] == "queue_update"
    })
    .await;
    assert_eq!(follow_queue["followUp"][0], "follow\n[image:image/jpeg]");
    assert!(!follow_queue.to_string().contains("Zm9sbG93"));

    input_writer
        .write_all(b"{\"id\":\"queued-control-prompt\",\"type\":\"prompt\",\"message\":\"base\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "queued control prompt start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    wait_for_rpc_provider_start(started_rx, "queued control provider start").await;
    release_tx.send(()).unwrap();
    let _ = read_rpc_json_matching(&mut lines, "queued control completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"queued-control-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let state = read_rpc_json_matching(&mut lines, "queued control state", |value| {
        value["id"] == "queued-control-state"
    })
    .await;
    assert_eq!(state["data"]["pendingMessageCount"], 0, "{state}");
    drop(input_writer);
    task.await.unwrap();

    let contexts = contexts.lock().unwrap();
    assert!(contexts.len() >= 2, "{contexts:#?}");
    assert!(contexts[0].messages.iter().any(|message| matches!(
        message,
        Message::User { content }
            if content.iter().any(|block| matches!(
                block,
                ContentBlock::Image { data, mime_type }
                    if data == "c3RlZXI=" && mime_type == "image/png"
            ))
    )));
    assert!(
        contexts[1..]
            .iter()
            .any(|context| context.messages.iter().any(|message| matches!(
                message,
                Message::User { content }
                    if content.iter().any(|block| matches!(
                        block,
                        ContentBlock::Image { data, mime_type }
                            if data == "Zm9sbG93" && mime_type == "image/jpeg"
                    ))
            )))
    );
}

#[tokio::test]
async fn rpc_queue_mode_setters_change_provider_queue_drain_behavior() {
    let api = "pi-coding-rpc-queue-mode-behavior";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(BlockingTwoTurnProvider::new(
            contexts.clone(),
            started_tx,
            release_rx,
        )),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(16 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    for command in [
        "{\"id\":\"steering-all\",\"type\":\"set_steering_mode\",\"mode\":\"all\"}\n",
        "{\"id\":\"follow-all\",\"type\":\"set_follow_up_mode\",\"mode\":\"all\"}\n",
        "{\"id\":\"steer-one\",\"type\":\"steer\",\"message\":\"steer one\"}\n",
        "{\"id\":\"steer-two\",\"type\":\"steer\",\"message\":\"steer two\"}\n",
        "{\"id\":\"follow-one\",\"type\":\"follow_up\",\"message\":\"follow one\"}\n",
        "{\"id\":\"follow-two\",\"type\":\"follow_up\",\"message\":\"follow two\"}\n",
    ] {
        input_writer.write_all(command.as_bytes()).await.unwrap();
    }
    for id in [
        "steering-all",
        "follow-all",
        "steer-one",
        "steer-two",
        "follow-one",
        "follow-two",
    ] {
        let response = read_rpc_json_matching(&mut lines, "queue mode setup response", |value| {
            value["id"] == id
        })
        .await;
        assert_eq!(response["success"], true, "{response}");
    }

    input_writer
        .write_all(b"{\"id\":\"queue-mode-prompt\",\"type\":\"prompt\",\"message\":\"base\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "queue mode prompt start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    wait_for_rpc_provider_start(started_rx, "queue mode provider start").await;
    release_tx.send(()).unwrap();
    let _ = read_rpc_json_matching(&mut lines, "queue mode completion", |value| {
        value["type"] == "agent_end"
    })
    .await;
    drop(input_writer);
    task.await.unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2, "{contexts:#?}");
    for expected in ["steer one", "steer two"] {
        assert!(
            contexts[0].messages.iter().any(|message| matches!(
                message,
                Message::User { content }
                    if content.iter().any(|block| matches!(
                        block,
                        ContentBlock::Text { text, .. } if text == expected
                    ))
            )),
            "missing {expected:?} from first provider request: {:#?}",
            contexts[0]
        );
    }
    for expected in ["follow one", "follow two"] {
        assert!(
            contexts[1].messages.iter().any(|message| matches!(
                message,
                Message::User { content }
                    if content.iter().any(|block| matches!(
                        block,
                        ContentBlock::Text { text, .. } if text == expected
                    ))
            )),
            "missing {expected:?} from second provider request: {:#?}",
            contexts[1]
        );
    }
}

#[tokio::test]
async fn rpc_persistent_session_rejects_ephemeral_auto_compaction() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("project");
    let sessions = temp.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-auto-compaction-behavior";
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let near_limit_usage = Usage {
        input: 95_000,
        total_tokens: 95_000,
        ..Usage::default()
    };
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(ScriptedUsageProvider {
            contexts: contexts.clone(),
            responses: Mutex::new(VecDeque::from([
                ("seed answer".into(), near_limit_usage.clone()),
                ("disabled answer".into(), near_limit_usage),
                ("enabled answer".into(), Usage::default()),
            ])),
        }),
    );
    let mut model = faux_model(api);
    model.context_window = 100_000;
    let mut session = SessionRunOptions::enabled(cwd);
    session.session_dir = Some(sessions);
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(32 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(model),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                session,
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"compact-seed\",\"type\":\"prompt\",\"message\":\"seed history\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "compaction seed start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    let _ = read_rpc_json_matching(&mut lines, "compaction seed end", |value| {
        value["type"] == "agent_end"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"compact-off\",\"type\":\"set_auto_compaction\",\"enabled\":false}\n")
        .await
        .unwrap();
    let disabled = read_rpc_json_matching(&mut lines, "disable auto compaction", |value| {
        value["id"] == "compact-off"
    })
    .await;
    assert_eq!(disabled["success"], true, "{disabled}");
    input_writer
        .write_all(b"{\"id\":\"compact-disabled-prompt\",\"type\":\"prompt\",\"message\":\"without compaction\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "disabled compaction start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    let _ = read_rpc_json_matching(&mut lines, "disabled compaction end", |value| {
        value["type"] == "agent_end"
    })
    .await;
    assert_eq!(contexts.lock().unwrap().len(), 2);

    input_writer
        .write_all(b"{\"id\":\"compact-on\",\"type\":\"set_auto_compaction\",\"enabled\":true}\n")
        .await
        .unwrap();
    let enabled = read_rpc_json_matching(&mut lines, "enable auto compaction", |value| {
        value["id"] == "compact-on"
    })
    .await;
    assert_eq!(enabled["success"], false, "{enabled}");
    assert!(
        enabled["error"]
            .as_str()
            .is_some_and(|error| error.contains("use compact")),
        "{enabled}"
    );
    input_writer
        .write_all(b"{\"id\":\"compact-enabled-prompt\",\"type\":\"prompt\",\"message\":\"with compaction\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "enabled compaction start", |value| {
        value["type"] == "agent_start"
    })
    .await;
    let _ = read_rpc_json_matching(&mut lines, "enabled compaction end", |value| {
        value["type"] == "agent_end"
    })
    .await;
    drop(input_writer);
    task.await.unwrap();

    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 3, "{contexts:#?}");
    assert!(contexts[2].messages.iter().any(|message| matches!(
        message,
        Message::User { content }
            if content.iter().any(|block| matches!(
                block,
                ContentBlock::Text { text, .. } if text == "with compaction"
            ))
    )));
}

#[tokio::test]
async fn rpc_session_name_is_explicit_adapter_local_display_state() {
    let api = "pi-coding-rpc-session-name-display";
    let provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("first", StopReason::Stop),
            FauxProvider::text_call("second", StopReason::Stop),
        ])),
    );
    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
    let (output_writer, output_reader) = tokio::io::duplex(32 * 1024);
    let task = tokio::spawn(async move {
        let mut output_writer = output_writer;
        run_rpc_mode_for_io(
            input_reader,
            &mut output_writer,
            CliRunOptions {
                model_override: Some(faux_model(api)),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(b"{\"id\":\"name-bootstrap\",\"type\":\"prompt\",\"message\":\"first\"}\n")
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "name bootstrap end", |value| {
        value["type"] == "agent_end"
    })
    .await;

    input_writer
        .write_all(
            b"{\"id\":\"set-name\",\"type\":\"set_session_name\",\"name\":\"Review workspace\"}\n",
        )
        .await
        .unwrap();
    let set_name = read_rpc_json_matching(&mut lines, "set session name", |value| {
        value["id"] == "set-name"
    })
    .await;
    assert_eq!(
        set_name["data"],
        serde_json::json!({
            "name": "Review workspace",
            "persistence": "adapter_local"
        })
    );

    input_writer
        .write_all(b"{\"id\":\"named-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let named = read_rpc_json_matching(&mut lines, "named state", |value| {
        value["id"] == "named-state"
    })
    .await;
    assert_eq!(named["data"]["sessionName"], "Review workspace");
    assert_eq!(named["data"]["sessionNamePersistence"], "adapter_local");

    input_writer
        .write_all(b"{\"id\":\"name-detach\",\"type\":\"detach\"}\n")
        .await
        .unwrap();
    let detached = read_rpc_json_matching(&mut lines, "name detach", |value| {
        value["id"] == "name-detach"
    })
    .await;
    assert_eq!(detached["success"], true, "{detached}");
    input_writer
        .write_all(b"{\"id\":\"detached-name-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let detached_state = read_rpc_json_matching(&mut lines, "detached name state", |value| {
        value["id"] == "detached-name-state"
    })
    .await;
    assert_eq!(detached_state["data"]["sessionName"], "Review workspace");

    input_writer
        .write_all(
            b"{\"id\":\"name-reconnect-prompt\",\"type\":\"prompt\",\"message\":\"second\"}\n",
        )
        .await
        .unwrap();
    let _ = read_rpc_json_matching(&mut lines, "name reconnect end", |value| {
        value["type"] == "agent_end"
    })
    .await;
    input_writer
        .write_all(b"{\"id\":\"reconnected-name-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let reconnected = read_rpc_json_matching(&mut lines, "reconnected name state", |value| {
        value["id"] == "reconnected-name-state"
    })
    .await;
    assert_eq!(reconnected["data"]["sessionName"], "Review workspace");

    input_writer
        .write_all(b"{\"id\":\"new-after-name\",\"type\":\"new_session\"}\n")
        .await
        .unwrap();
    let new_session = read_rpc_json_matching(&mut lines, "new session after name", |value| {
        value["id"] == "new-after-name"
    })
    .await;
    assert_eq!(new_session["success"], true, "{new_session}");
    input_writer
        .write_all(b"{\"id\":\"new-name-state\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();
    let reset = read_rpc_json_matching(&mut lines, "reset name state", |value| {
        value["id"] == "new-name-state"
    })
    .await;
    assert!(reset["data"].get("sessionName").is_none(), "{reset}");
    assert_eq!(reset["data"]["sessionNamePersistence"], "adapter_local");

    drop(input_writer);
    task.await.unwrap();
}

#[tokio::test]
async fn rpc_plain_prompt_while_running_returns_error() {
    let api = "pi-coding-rpc-running-prompt-error";
    let release = Arc::new(Notify::new());
    let _provider_guard = ProviderGuard::register(
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
                ai_client: Some(_provider_guard.ai_client()),
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
    let _ = read_rpc_line(&mut lines, "initial prompt response").await;
    let _ = read_rpc_line(&mut lines, "initial agent_start event").await;

    input_writer
        .write_all(b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"second\"}\n")
        .await
        .unwrap();

    let response = read_rpc_json_matching(
        &mut lines,
        "plain prompt rejection while prompt is running",
        |value| value["type"] == "response" && value["id"] == "p2",
    )
    .await;

    release.notify_one();
    drop(input_writer);
    task.await.unwrap();

    assert_eq!(response["success"], false);
    assert_eq!(
        response["error"],
        "agent is streaming; prompt requires streamingBehavior steer or followUp"
    );
}

#[tokio::test]
async fn rpc_prompt_idempotency_key_deduplicates_running_retry() {
    let api = "pi-coding-rpc-idempotent-prompt";
    let release = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(PausingProvider {
            release: Arc::clone(&release),
            opened: Arc::clone(&opened),
        }),
    );

    let (mut input_writer, input_reader) = tokio::io::duplex(4096);
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
                ai_client: Some(_provider_guard.ai_client()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    });
    let mut lines = tokio::io::BufReader::new(output_reader).lines();

    input_writer
        .write_all(
            b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\",\"idempotencyKey\":\"retry:prompt:1\"}\n",
        )
        .await
        .unwrap();
    let first = read_rpc_json_line(&mut lines, "initial prompt response").await;
    assert_eq!(first["success"], true);

    input_writer
        .write_all(
            b"{\"id\":\"p2\",\"type\":\"prompt\",\"message\":\"hello\",\"idempotencyKey\":\"retry:prompt:1\"}\n",
        )
        .await
        .unwrap();
    let retry = read_rpc_json_matching(&mut lines, "idempotent prompt retry", |value| {
        value["type"] == "response" && value["command"] == "prompt" && value["id"] == "p2"
    })
    .await;

    assert_eq!(retry["success"], true);
    assert_eq!(retry["data"]["deduplicated"], true);
    assert_eq!(retry["data"]["operation"], "prompt");

    release.notify_one();
    drop(input_writer);
    await_rpc_task_completion(task, &release, "idempotent prompt rpc task").await;
}

#[tokio::test]
async fn rpc_state_commands_update_get_state() {
    let api = "pi-coding-rpc-state";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("unused")));

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
            ai_client: Some(_provider_guard.ai_client()),
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
}

#[tokio::test]
async fn rpc_hello_negotiates_supported_protocol_families() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":2,\"minor\":0}}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/architecture-baseline-v1/rpc-hello-response.json"
    ))
    .unwrap();
    assert_eq!(lines, vec![expected]);
}

#[tokio::test]
async fn rpc_hello_rejects_unsupported_major_protocol_version() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":1,\"minor\":0}}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello-reject")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["command"], "hello");
    assert_eq!(lines[0]["success"], false);
    assert_eq!(lines[0]["data"]["code"], "unsupported_protocol_version");
    assert_eq!(lines[0]["data"]["requested"]["major"], 1);
    assert_eq!(lines[0]["data"]["supported"]["major"], 2);
}

#[tokio::test]
async fn rpc_hello_records_negotiated_protocol_state() {
    let input = b"{\"id\":\"h1\",\"type\":\"hello\",\"protocol\":{\"family\":\"rpc\",\"major\":2,\"minor\":0}}\n{\"id\":\"s1\",\"type\":\"get_state\"}\n";
    let mut output = Vec::new();

    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model("pi-coding-rpc-hello-state")),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let lines = parse_lines(&output);
    assert_eq!(lines[0]["command"], "hello");
    assert_eq!(lines[0]["success"], true);
    assert_eq!(lines[1]["command"], "get_state");
    assert_eq!(
        lines[1]["data"]["negotiatedProtocol"]["rpc"]["family"],
        "rpc"
    );
    assert_eq!(lines[1]["data"]["negotiatedProtocol"]["rpc"]["major"], 2);
}
