use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{
    CliArgs, CliError, CliOutput, CliRunOptions, PrintModeOptions, help_text, parse_args,
};

fn model(api: &str) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[test]
fn public_api_symbols_are_importable() {
    let args = CliArgs::default();
    assert_eq!(args.max_turns, 5);

    let parsed = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    assert!(parsed.print);
    assert_eq!(parsed.prompt.as_deref(), Some("hello"));

    let print_options = PrintModeOptions::new("hello", model("public-api-test"));
    assert_eq!(print_options.prompt, "hello");
    assert!(!print_options.register_builtins);

    let output = CliOutput {
        exit_code: 0,
        stdout: "ok\n".into(),
        stderr: String::new(),
    };
    assert_eq!(output.exit_code, 0);

    let runtime_options = CliRunOptions::default();
    assert!(runtime_options.register_builtins);

    let err = CliError::MissingPrompt;
    assert_eq!(err.to_string(), "missing prompt");

    assert!(help_text().contains("Usage:"));
}
