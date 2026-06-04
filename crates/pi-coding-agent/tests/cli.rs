use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
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
async fn missing_print_mode_is_rejected() {
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
    assert_eq!(output.stderr, "unsupported mode: interactive\n");
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
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello from CLI")));

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
    registry::unregister(api);
}
