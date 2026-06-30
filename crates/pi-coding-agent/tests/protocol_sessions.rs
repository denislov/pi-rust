use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{CliRunOptions, SessionRunOptions, protocol::rpc::run_rpc_mode_for_io};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

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

#[tokio::test]
async fn rpc_prompt_persists_session_messages() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions.clone());

    let input = b"{\"id\":\"n1\",\"type\":\"set_session_name\",\"name\":\"rpc work\"}\n\
                  {\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
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

    let session_dirs = collect_native_session_dirs(&sessions);
    assert_eq!(session_dirs.len(), 1);
    assert!(session_dirs[0].join("session.json").exists());
    let contents = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(contents.contains("\"kind\":\"session.created\""));
    assert!(contents.contains("\"kind\":\"turn.input.recorded\""));
    assert!(contents.contains("\"kind\":\"message.completed\""));
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_state_reports_persisted_session_path_after_prompt() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-session-state";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));
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
    .expect("agent_end after prompt");

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
    .expect("state response after prompt");

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
    assert_eq!(state["data"]["sessionId"], active_leaf_id);
    assert_ne!(active_leaf_id, manifest["session_id"].as_str().unwrap());
    registry::unregister(api);
}

#[tokio::test]
async fn rpc_disabled_session_prompt_uses_non_persistent_runtime_without_session_files() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-disabled-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));
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
    assert_eq!(state["data"]["sessionId"], "in-memory");
    assert!(state["data"]["sessionFile"].is_null());
    assert!(collect_native_session_dirs(&sessions).is_empty());
    registry::unregister(api);
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
