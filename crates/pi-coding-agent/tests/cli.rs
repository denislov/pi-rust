mod support;

use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
use pi_coding_agent::{CliRunOptions, run_cli_with_options};
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

#[tokio::test]
async fn help_returns_success_with_help_text() {
    let output = run_cli_with_options(
        vec!["--help".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn version_returns_success_with_package_version() {
    let output = run_cli_with_options(
        vec!["--version".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, format!("{}\n", env!("CARGO_PKG_VERSION")));
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn list_models_returns_success_without_prompt() {
    let output = run_cli_with_options(
        vec!["--list-models".to_string(), "claude".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    assert!(output.stdout.contains("provider"));
    assert!(output.stdout.contains("model"));
    assert!(output.stdout.contains("claude"));
}

#[tokio::test]
async fn list_models_supports_provider_filter_and_json_output() {
    let output = run_cli_with_options(
        vec![
            "--list-models".to_string(),
            "--provider".to_string(),
            "anthropic".to_string(),
            "--json".to_string(),
        ],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    let rows: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    let rows = rows
        .as_array()
        .expect("list models JSON should be an array");
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row["provider"] == "anthropic"));
    assert!(rows.iter().all(|row| row.get("model").is_some()));
}

#[tokio::test]
async fn list_models_is_read_only_for_session_id() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&project).unwrap();

    let output = run_cli_with_options(
        vec![
            "--session-id".to_string(),
            "read-only-models".to_string(),
            "--list-models".to_string(),
            "--provider".to_string(),
            "anthropic".to_string(),
        ],
        CliRunOptions {
            register_builtins: false,
            session: SessionRunOptions {
                mode: SessionMode::Enabled,
                cwd: project,
                session_dir: Some(sessions.clone()),
            },
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(
        !sessions.exists(),
        "--list-models must not create or reserve session files"
    );
}

#[tokio::test]
async fn default_prompt_routes_to_interactive_mode() {
    let output = run_cli_with_options(
        vec!["hello".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, "interactive mode requires a TTY\n");
}

#[tokio::test]
async fn missing_prompt_is_rejected() {
    let output = run_cli_with_options(
        vec!["-p".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(output.stderr, "missing prompt\n");
}

#[tokio::test]
async fn unknown_model_is_rejected() {
    let output = run_cli_with_options(
        vec![
            "--model".to_string(),
            "missing-model".to_string(),
            "-p".to_string(),
            "hello".to_string(),
        ],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(output.stderr, "unknown model: missing-model\n");
}

#[tokio::test]
async fn print_mode_uses_injected_model_and_returns_stdout() {
    let api = "pi-coding-cli-success";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello from CLI")));

    let output = run_cli_with_options(
        vec!["-p".to_string(), "hello".to_string()],
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, "Hello from CLI\n");
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn json_mode_uses_injected_model_and_returns_jsonl() {
    let api = "pi-coding-cli-json";
    let _provider_guard =
        ProviderGuard::register(api, Arc::new(FauxProvider::simple_text("Hello JSON")));

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
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.stderr.is_empty());
    assert!(
        output
            .stdout
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[tokio::test]
async fn rpc_mode_is_not_run_through_buffered_cli_output() {
    let output = run_cli_with_options(
        vec!["--mode".to_string(), "rpc".to_string()],
        CliRunOptions {
            register_builtins: false,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stderr,
        "unsupported mode: rpc requires the streaming binary entry point\n"
    );
}
