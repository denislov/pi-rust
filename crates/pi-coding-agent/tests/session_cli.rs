mod support;

use pi_ai::api::{Model, ModelCost, ModelInput, StopReason};
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_coding_agent::api::{CliRunOptions, SessionMode, SessionRunOptions, run_cli_with_options};
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

fn test_options(
    api: &str,
    cwd: &std::path::Path,
    sessions: &std::path::Path,
    ai_client: pi_ai::api::AiClient,
) -> CliRunOptions {
    CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(ai_client),
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
    let api2 = "session-cli-second";
    let _provider_guard = ProviderGuard::register_many(vec![
        (
            api1.to_string(),
            Arc::new(FauxProvider::simple_text("first answer")),
        ),
        (
            api2.to_string(),
            Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
                responses: vec![text_response("second answer")],
                stop_reason: StopReason::Stop,
            }])),
        ),
    ]);
    let options1 = test_options(api1, &cwd, &sessions, _provider_guard.ai_client());
    let first = run_cli_with_options(vec!["-p".into(), "first".into()], options1).await;
    assert_eq!(first.exit_code, 0);
    assert_eq!(first.stdout, "first answer\n");

    let options2 = test_options(api2, &cwd, &sessions, _provider_guard.ai_client());
    let second = run_cli_with_options(
        vec!["--continue".into(), "-p".into(), "second".into()],
        options2,
    )
    .await;
    assert_eq!(second.exit_code, 0);
    assert_eq!(second.stdout, "second answer\n");
}

#[tokio::test]
async fn no_session_does_not_write_files() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api = "session-cli-no-persist";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("answer")));

    let options = CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
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
}

#[tokio::test]
async fn session_path_target_opens_rust_native_session_directory() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-path-create";
    let api2 = "session-cli-path-append";
    let _provider_guard = ProviderGuard::register_many(vec![
        (
            api1.to_string(),
            Arc::new(FauxProvider::simple_text("first")),
        ),
        (
            api2.to_string(),
            Arc::new(FauxProvider::simple_text("second")),
        ),
    ]);
    let options1 = test_options(api1, &cwd, &sessions, _provider_guard.ai_client());

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

    assert!(sessions.join("path-test-id").join("session.json").is_file());

    let options2 = test_options(api2, &cwd, &sessions, _provider_guard.ai_client());
    let session_path = sessions.join("path-test-id").display().to_string();

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

    let text = std::fs::read_to_string(sessions.join("path-test-id").join("events.jsonl")).unwrap();
    assert_eq!(text.matches(r#""kind":"turn.input.recorded""#).count(), 2);
    assert_eq!(text.matches(r#""kind":"message.completed""#).count(), 2);
    assert!(!text.contains(r#""type":"message""#));
}

#[tokio::test]
async fn session_id_creates_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-id-create";
    let api2 = "session-cli-id-reopen";
    let _provider_guard = ProviderGuard::register_many(vec![
        (
            api1.to_string(),
            Arc::new(FauxProvider::simple_text("first")),
        ),
        (
            api2.to_string(),
            Arc::new(FauxProvider::simple_text("second")),
        ),
    ]);
    let options1 = test_options(api1, &cwd, &sessions, _provider_guard.ai_client());

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

    let options2 = test_options(api2, &cwd, &sessions, _provider_guard.ai_client());

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
}

#[tokio::test]
async fn fork_target_routes_through_rust_native_print_session_cli() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api1 = "session-cli-fork-source";
    let api2 = "session-cli-fork-target";
    let _provider_guard = ProviderGuard::register_many(vec![
        (
            api1.to_string(),
            Arc::new(FauxProvider::simple_text("source")),
        ),
        (
            api2.to_string(),
            Arc::new(FauxProvider::simple_text("fork")),
        ),
    ]);
    let options1 = test_options(api1, &cwd, &sessions, _provider_guard.ai_client());

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

    let options2 = test_options(api2, &cwd, &sessions, _provider_guard.ai_client());
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
    assert_eq!(result.stdout, "fork\n");

    let dirs = session_dirs(&sessions);
    assert_eq!(dirs.len(), 2);
    let fork_dir = dirs
        .iter()
        .find(|path| path.file_name().and_then(|name| name.to_str()) != Some("fork-source-id"))
        .expect("expected generated fork session directory");
    let fork_events = std::fs::read_to_string(fork_dir.join("events.jsonl")).unwrap();
    assert!(fork_events.contains(r#""kind":"session.forked""#));
    assert!(fork_events.contains("source prompt"));
    assert!(fork_events.contains("fork prompt"));

    let source_events =
        std::fs::read_to_string(sessions.join("fork-source-id/events.jsonl")).unwrap();
    assert!(!source_events.contains("fork prompt"));
}

#[tokio::test]
async fn name_does_not_write_legacy_session_info_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let sessions = dir.path().join("sessions");

    let api = "session-cli-name";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("answer")));
    let options = test_options(api, &cwd, &sessions, _provider_guard.ai_client());

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

    let session_dirs = session_dirs(&sessions);
    assert_eq!(session_dirs.len(), 1);
    let text = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(text.contains(r#""kind":"session.created""#));
    assert!(!text.contains("session_info"));
    assert!(!text.contains("named-run"));
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_jsonl_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
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
    let api2 = "chain-second";
    let _provider_guard = ProviderGuard::register_many(vec![
        (
            api1.to_string(),
            Arc::new(FauxProvider::simple_text("first response")),
        ),
        (
            api2.to_string(),
            Arc::new(FauxProvider::simple_text("second response")),
        ),
    ]);
    let options1 = test_options(api1, &cwd, &sessions, _provider_guard.ai_client());
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

    let options2 = test_options(api2, &cwd, &sessions, _provider_guard.ai_client());
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

    let text = std::fs::read_to_string(sessions.join("test123").join("events.jsonl")).unwrap();
    assert_eq!(text.matches(r#""kind":"turn.input.recorded""#).count(), 2);
    assert_eq!(text.matches(r#""kind":"message.completed""#).count(), 2);
    assert!(!text.contains(r#""type":"message""#));
}

#[tokio::test]
async fn session_dir_flag_writes_to_custom_path() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let custom_sessions = dir.path().join("custom").join("sessions").join("path");
    std::fs::create_dir_all(&custom_sessions).unwrap();

    let api = "session-dir-custom";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("answer")));

    let options = CliRunOptions {
        model_override: Some(faux_model(api)),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
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

    let mut files = Vec::new();
    collect_jsonl_files(&custom_sessions, &mut files);
    assert!(
        !files.is_empty(),
        "expected jsonl files under custom session path {:?}",
        custom_sessions
    );
    assert_eq!(session_dirs(&custom_sessions).len(), 1);
    assert!(
        session_dirs(&custom_sessions)[0]
            .join("session.json")
            .is_file()
    );
}

fn session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect()
}
