use crate::support;

use pi_ai::api::conversation::StopReason;
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::testing::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_coding_agent::api::cli::runner::run_cli_with_options;
use pi_coding_agent::api::cli::runtime::{CliRunOptions, SessionMode, SessionRunOptions};
use std::sync::Arc;
use support::ProviderGuard;

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

fn json_lines(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn json_mode_emits_session_header_and_lifecycle_events() {
    let api = "pi-coding-json-lifecycle";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello")));

    let output = run_cli_with_options(
        vec![
            "--mode".to_string(),
            "json".to_string(),
            "hello".to_string(),
        ],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    let lines = json_lines(&output.stdout);
    assert_eq!(lines[0]["type"], "session");
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "turn_start"));
    assert!(lines.iter().any(|line| line["type"] == "message_update"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
}

#[tokio::test]
async fn json_mode_emits_tool_execution_events() {
    let api = "pi-coding-json-tool";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_1".into(),
                        name: "echo".into(),
                        deltas: vec!["{\"text\":\"hi\"}".into()],
                        final_arguments: serde_json::json!({"text": "hi"}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxProvider::text_call("done", StopReason::Stop),
        ])),
    );

    let tool = pi_agent_core::api::tool::AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type":"object","properties":{"text":{"type":"string"}}}),
        |_context, args| async move { Ok(format!("echo: {}", args["text"].as_str().unwrap_or(""))) },
    );

    let output = run_cli_with_options(
        vec![
            "--mode".to_string(),
            "json".to_string(),
            "echo hi".to_string(),
        ],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: vec![tool],
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    let lines = json_lines(&output.stdout);
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "tool_execution_start")
    );
    assert!(
        lines
            .iter()
            .any(|line| line["type"] == "tool_execution_end")
    );
}

#[tokio::test]
async fn json_mode_maps_provider_failure_to_error_output() {
    let api = "pi-coding-json-error";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: Vec::new(),
                thinking_deltas: Vec::new(),
                tool_calls: Vec::new(),
            }],
            stop_reason: StopReason::Error,
        }])),
    );

    let output = run_cli_with_options(
        vec!["--mode".to_string(), "json".to_string(), "fail".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("LLM error"));
    let lines = json_lines(&output.stdout);
    assert!(lines.iter().any(|line| line["type"] == "agent_start"));
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));
}

#[tokio::test]
async fn json_mode_enabled_session_uses_rust_native_log() {
    let api = "pi-coding-json-native-session";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("stored json")));
    let temp = tempfile::tempdir().unwrap();
    let project_dir = temp.path().join("project");
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let output = run_cli_with_options(
        vec![
            "--mode".to_string(),
            "json".to_string(),
            "persist json".to_string(),
        ],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            session: SessionRunOptions {
                mode: SessionMode::Enabled,
                cwd: project_dir.clone(),
                session_dir: Some(sessions_dir.clone()),
            },
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    let lines = json_lines(&output.stdout);
    assert!(lines.iter().any(|line| line["type"] == "agent_end"));

    let session_dirs = rust_session_dirs(&sessions_dir);
    assert_eq!(session_dirs.len(), 1);
    assert!(session_dirs[0].join("session.json").is_file());
    let events = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(events.contains(r#""kind":"session.created""#));
    assert!(events.contains(&format!(r#""cwd":"{}""#, project_dir.display())));
    assert!(events.contains(r#""kind":"turn.input.recorded""#));
    assert!(events.contains(r#""kind":"message.completed""#));
    assert!(legacy_jsonl_files(&sessions_dir).is_empty());
}

fn rust_session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    collect_rust_session_dirs(root, &mut dirs);
    dirs.sort();
    dirs
}

fn collect_rust_session_dirs(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("session.json").is_file() && path.join("events.jsonl").is_file() {
                    out.push(path);
                } else {
                    collect_rust_session_dirs(&path, out);
                }
            }
        }
    }
}

fn legacy_jsonl_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_legacy_jsonl_files(root, &mut files);
    files.sort();
    files
}

fn collect_legacy_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_legacy_jsonl_files(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                && path.file_name().and_then(|name| name.to_str()) != Some("events.jsonl")
            {
                out.push(path);
            }
        }
    }
}
