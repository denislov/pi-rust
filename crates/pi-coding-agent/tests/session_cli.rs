use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput, StopReason};
use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
use pi_coding_agent::{CliRunOptions, run_cli_with_options};
use std::sync::Arc;

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

fn test_options(api: &str, cwd: &std::path::Path, sessions: &std::path::Path) -> CliRunOptions {
    CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        session: SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: cwd.to_path_buf(),
            session_dir: Some(sessions.to_path_buf()),
        },
    }
}

#[tokio::test]
async fn continue_uses_previous_session_context() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-first";
    registry::register(api1, Arc::new(FauxProvider::simple_text("first answer")));
    let options1 = test_options(api1, &cwd, &sessions);
    let first = run_cli_with_options(vec!["-p".into(), "first".into()], options1).await;
    assert_eq!(first.exit_code, 0);
    assert_eq!(first.stdout, "first answer\n");
    registry::unregister(api1);

    let api2 = "session-cli-second";
    registry::register(
        api2,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![text_response("second answer")],
            stop_reason: StopReason::Stop,
        }])),
    );
    let options2 = test_options(api2, &cwd, &sessions);
    let second = run_cli_with_options(
        vec!["--continue".into(), "-p".into(), "second".into()],
        options2,
    )
    .await;
    assert_eq!(second.exit_code, 0);
    assert_eq!(second.stdout, "second answer\n");
    registry::unregister(api2);
}

#[tokio::test]
async fn no_session_does_not_write_files() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api = "session-cli-no-persist";
    registry::register(api, Arc::new(FauxProvider::simple_text("answer")));

    let options = CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        session: SessionRunOptions {
            mode: SessionMode::Disabled,
            cwd: cwd.clone(),
            session_dir: Some(sessions.clone()),
        },
    };

    let output = run_cli_with_options(
        vec!["--no-session".into(), "-p".into(), "hi".into()],
        options,
    )
    .await;
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, "answer\n");

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert!(files.is_empty());
    registry::unregister(api);
}

#[tokio::test]
async fn session_path_appends_to_specific_file() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-path-create";
    registry::register(api1, Arc::new(FauxProvider::simple_text("first")));
    let options1 = test_options(api1, &cwd, &sessions);

    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "path-test-id".into(),
            "-p".into(),
            "first prompt".into(),
        ],
        options1,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api1);

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    let session_file = files.first().unwrap().clone();

    let api2 = "session-cli-path-append";
    registry::register(api2, Arc::new(FauxProvider::simple_text("second")));
    let options2 = test_options(api2, &cwd, &sessions);
    let session_path = session_file.display().to_string();

    let result = run_cli_with_options(
        vec![
            "--session".into(),
            session_path,
            "-p".into(),
            "second prompt".into(),
        ],
        options2,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "second\n");

    let text = std::fs::read_to_string(&session_file).unwrap();
    let lines = text.lines().count();
    assert!(
        lines >= 2,
        "expected at least 2 lines, session header + entries"
    );
    assert!(text.contains("second prompt"));
    registry::unregister(api2);
}

#[tokio::test]
async fn session_id_creates_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-id-create";
    registry::register(api1, Arc::new(FauxProvider::simple_text("first")));
    let options1 = test_options(api1, &cwd, &sessions);

    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "my-id-123".into(),
            "-p".into(),
            "first prompt".into(),
        ],
        options1,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api1);

    let api2 = "session-cli-id-reopen";
    registry::register(api2, Arc::new(FauxProvider::simple_text("second")));
    let options2 = test_options(api2, &cwd, &sessions);

    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "my-id-123".into(),
            "-p".into(),
            "second prompt".into(),
        ],
        options2,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "second\n");
    registry::unregister(api2);
}

#[tokio::test]
async fn fork_creates_parent_session_header() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-fork-source";
    registry::register(api1, Arc::new(FauxProvider::simple_text("source")));
    let options1 = test_options(api1, &cwd, &sessions);

    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "fork-source-id".into(),
            "-p".into(),
            "source prompt".into(),
        ],
        options1,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api1);

    let api2 = "session-cli-fork-target";
    registry::register(api2, Arc::new(FauxProvider::simple_text("fork")));

    let options2 = test_options(api2, &cwd, &sessions);
    let result = run_cli_with_options(
        vec![
            "--fork".into(),
            "fork-source-id".into(),
            "-p".into(),
            "fork prompt".into(),
        ],
        options2,
    )
    .await;
    assert_eq!(result.exit_code, 0);

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert!(files.len() >= 2);

    let mut found_parent = false;
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        if text.contains("source") {
            if text.contains("parentSession") {
                found_parent = true;
            }
        }
    }
    assert!(found_parent, "fork should contain parentSession reference");

    registry::unregister(api2);
}

#[tokio::test]
async fn name_appends_session_info() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api = "session-cli-name";
    registry::register(api, Arc::new(FauxProvider::simple_text("answer")));
    let options = test_options(api, &cwd, &sessions);

    let result = run_cli_with_options(
        vec![
            "--name".into(),
            "named-run".into(),
            "-p".into(),
            "hi".into(),
        ],
        options,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "answer\n");

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains("session_info"));
    assert!(text.contains("named-run"));
    registry::unregister(api);
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    collect_jsonl_files(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    out.push(path);
                }
            }
        }
    }
}

#[tokio::test]
async fn continue_maintains_parent_chain() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "chain-first";
    registry::register(api1, Arc::new(FauxProvider::simple_text("first response")));
    let options1 = test_options(api1, &cwd, &sessions);
    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "test123".into(),
            "-p".into(),
            "first".into(),
        ],
        options1,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api1);

    let api2 = "chain-second";
    registry::register(api2, Arc::new(FauxProvider::simple_text("second response")));
    let options2 = test_options(api2, &cwd, &sessions);
    let result = run_cli_with_options(
        vec![
            "--session-id".into(),
            "test123".into(),
            "-p".into(),
            "second".into(),
        ],
        options2,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api2);

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    let entries: Vec<serde_json::Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let mut user_ids: Vec<String> = Vec::new();
    let mut assistant_ids: Vec<String> = Vec::new();
    for entry in &entries {
        let entry_type = entry["type"].as_str().unwrap_or("");
        if entry_type == "message" {
            let role = entry["message"]["role"].as_str().unwrap_or("");
            let id = entry["id"].as_str().unwrap().to_string();
            if role == "user" {
                user_ids.push(id);
            } else if role == "assistant" {
                assistant_ids.push(id);
            }
        }
    }

    assert!(
        user_ids.len() >= 2,
        "expected at least 2 user messages, got {}",
        user_ids.len()
    );
    assert!(
        assistant_ids.len() >= 2,
        "expected at least 2 assistant messages, got {}",
        assistant_ids.len()
    );

    let second_user_parent = entries
        .iter()
        .find(|e| e["id"].as_str() == Some(&user_ids[1]))
        .and_then(|e| e["parentId"].as_str())
        .map(|s| s.to_string());
    assert_eq!(
        second_user_parent.as_deref(),
        Some(assistant_ids[0].as_str()),
        "second user message should chain from first assistant"
    );

    let second_assistant_parent = entries
        .iter()
        .find(|e| e["id"].as_str() == Some(&assistant_ids[1]))
        .and_then(|e| e["parentId"].as_str())
        .map(|s| s.to_string());
    assert_eq!(
        second_assistant_parent.as_deref(),
        Some(user_ids[1].as_str()),
        "second assistant message should chain from second user"
    );
}

#[tokio::test]
async fn session_dir_flag_writes_to_custom_path() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let custom_sessions = dir.path().join("custom").join("sessions").join("path");
    std::fs::create_dir_all(&custom_sessions).unwrap();

    let api = "session-dir-custom";
    registry::register(api, Arc::new(FauxProvider::simple_text("answer")));

    let options = CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        session: SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: cwd.clone(),
            session_dir: None,
        },
    };

    let result = run_cli_with_options(
        vec![
            "--session-dir".into(),
            custom_sessions.display().to_string(),
            "-p".into(),
            "hi".into(),
        ],
        options,
    )
    .await;
    assert_eq!(result.exit_code, 0);
    registry::unregister(api);

    let mut files = Vec::new();
    collect_jsonl_files(&custom_sessions, &mut files);
    assert!(
        !files.is_empty(),
        "expected jsonl files under custom session path {:?}",
        custom_sessions
    );
}
