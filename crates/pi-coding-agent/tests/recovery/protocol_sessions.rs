use crate::support;

use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::testing::FauxProvider;
use pi_coding_agent::api::protocol::run_rpc_mode_for_io;
use pi_coding_agent::api::runtime::{CliRunOptions, SessionRunOptions};
use std::sync::Arc;
use std::time::Duration;
use support::ProviderGuard;
use tempfile::tempdir;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};

const PROTOCOL_SESSION_LINE_READ_TIMEOUT: Duration = Duration::from_millis(500);

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

async fn read_protocol_session_line<R>(lines: &mut tokio::io::Lines<R>, context: &str) -> String
where
    R: AsyncBufRead + Unpin,
{
    tokio::time::timeout(PROTOCOL_SESSION_LINE_READ_TIMEOUT, lines.next_line())
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {context}"))
        .unwrap_or_else(|error| panic!("failed reading {context}: {error}"))
        .unwrap_or_else(|| panic!("rpc output closed before {context}"))
}

async fn read_protocol_session_json_matching<R>(
    lines: &mut tokio::io::Lines<R>,
    context: &str,
    mut matches: impl FnMut(&serde_json::Value) -> bool,
) -> serde_json::Value
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let line = read_protocol_session_line(lines, context).await;
        let value: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid JSON for {context}: {error}"));
        if matches(&value) {
            return value;
        }
    }
}

#[tokio::test]
async fn rpc_state_reports_persisted_session_path_after_prompt() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-session-state";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello")));
    let ai_client = _provider_guard.ai_client();
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions.clone());

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
                ai_client: Some(ai_client),
                session: session_options,
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
    read_protocol_session_json_matching(&mut lines, "agent_end after prompt", |value| {
        value["type"] == "agent_end"
    })
    .await;

    input_writer
        .write_all(b"{\"id\":\"s1\",\"type\":\"get_state\"}\n")
        .await
        .unwrap();

    let state =
        read_protocol_session_json_matching(&mut lines, "state response after prompt", |value| {
            value["type"] == "response" && value["command"] == "get_state"
        })
        .await;

    drop(input_writer);
    task.await.unwrap();

    assert_eq!(state["data"]["isStreaming"], false);
    assert_ne!(state["data"]["sessionId"], "in-memory");
    let session_dir = std::path::PathBuf::from(state["data"]["sessionFile"].as_str().unwrap());
    assert!(session_dir.join("session.json").exists());
    assert!(session_dir.join("events.jsonl").exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(session_dir.join("session.json")).unwrap())
            .unwrap();
    let active_leaf_id = manifest["active_leaf_id"]
        .as_str()
        .expect("active leaf should be present after RPC prompt");
    let session_id = manifest["session_id"].as_str().unwrap();
    assert_eq!(state["data"]["sessionId"], session_id);
    assert_ne!(active_leaf_id, session_id);
}

#[tokio::test]
async fn rpc_disabled_session_prompt_uses_non_persistent_runtime_without_session_files() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-disabled-session";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello")));
    let mut session_options = SessionRunOptions::disabled(cwd);
    session_options.session_dir = Some(sessions.clone());

    let input = b"{\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n\
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
            session: session_options,
        },
    )
    .await
    .unwrap();

    let lines = String::from_utf8_lossy(&output)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
    let state = lines
        .iter()
        .find(|line| line["type"] == "response" && line["command"] == "get_state")
        .expect("get_state response after prompt");
    let session_id = state["data"]["sessionId"]
        .as_str()
        .expect("non-persistent runtime should still expose a session identity");
    assert!(!session_id.is_empty());
    assert_ne!(session_id, "in-memory");
    assert!(state["data"]["sessionFile"].is_null());
    assert!(collect_native_session_dirs(&sessions).is_empty());
}

fn collect_native_session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    collect_native_session_dirs_inner(root, &mut dirs);
    dirs
}

fn collect_native_session_dirs_inner(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("session.json").exists() && path.join("events.jsonl").exists() {
                out.push(path);
            } else {
                collect_native_session_dirs_inner(&path, out);
            }
        }
    }
}
