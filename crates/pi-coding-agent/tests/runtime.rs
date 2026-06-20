use pi_agent_core::AgentResources;
use pi_coding_agent::config::settings::{
    CompactionSettings, RetrySettings, Settings, TerminalSettings,
};
use pi_coding_agent::{
    CliError, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, build_agent_config, parse_args, select_model,
};

fn runtime_settings() -> Settings {
    Settings {
        default_provider: None,
        default_model: None,
        default_thinking_level: None,
        transport: "auto".into(),
        steering_mode: "one-at-a-time".into(),
        follow_up_mode: "one-at-a-time".into(),
        session_dir: None,
        skills: Vec::new(),
        prompts: Vec::new(),
        themes: Vec::new(),
        theme: None,
        no_context_files: false,
        terminal: TerminalSettings {
            show_images: true,
            show_progress: true,
        },
        compaction: CompactionSettings {
            enabled: true,
            reserve_tokens: 1234,
            keep_recent_tokens: 5678,
        },
        retry: RetrySettings {
            enabled: true,
            max_retries: 7,
            base_delay_ms: 4444,
        },
    }
}

#[test]
fn selects_default_model_when_no_override_is_provided() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    assert_eq!(model.id, DEFAULT_MODEL_ID);
}

#[test]
fn selects_explicit_model_from_static_table() {
    let args = parse_args(vec![
        "--model".to_string(),
        "claude-haiku-4-5".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    assert_eq!(model.id, "claude-haiku-4-5");
}

#[test]
fn unknown_model_returns_typed_error() {
    let args = parse_args(vec![
        "--model".to_string(),
        "missing-model".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    assert_eq!(
        select_model(&args, None, None, None).unwrap_err(),
        CliError::UnknownModel("missing-model".into())
    );
}

#[test]
fn model_override_is_used_when_cli_model_is_absent() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let mut override_model = select_model(&args, None, None, None).unwrap();
    override_model.id = "override-model".into();
    let model = select_model(&args, None, None, Some(override_model)).unwrap();
    assert_eq!(model.id, "override-model");
}

#[test]
fn selects_default_provider_when_cli_provider_is_absent() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();

    let model = select_model(&args, Some("deepseek"), None, None).unwrap();

    assert_eq!(model.provider, "deepseek");
}

#[test]
fn cli_provider_overrides_settings_default_provider() {
    let args = parse_args(vec![
        "--provider".to_string(),
        "google".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();

    let model = select_model(&args, Some("deepseek"), None, None).unwrap();

    assert_eq!(model.provider, "google");
}

#[test]
fn builds_agent_config_with_defaults() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
        None,
        None,
        AgentResources::default(),
        None,
    );
    assert_eq!(config.system_prompt.as_deref(), Some(DEFAULT_SYSTEM_PROMPT));
    assert_eq!(config.max_turns, 5);
    assert!(config.stream_options.is_none());
}

#[test]
fn builds_agent_config_with_cli_overrides() {
    let args = parse_args(vec![
        "--api-key".to_string(),
        "sk-test".to_string(),
        "--system-prompt".to_string(),
        "Be brief.".to_string(),
        "--max-turns".to_string(),
        "9".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
        None,
        None,
        AgentResources::default(),
        None,
    );
    assert_eq!(config.system_prompt.as_deref(), Some("Be brief."));
    assert_eq!(config.max_turns, 9);
    assert_eq!(
        config.stream_options.unwrap().api_key.as_deref(),
        Some("sk-test")
    );
}

#[test]
fn build_agent_config_applies_settings_retry_and_compaction() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    let settings = runtime_settings();

    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        None,
        None,
        None,
        AgentResources::default(),
        Some(&settings),
    );

    let stream = config.stream_options.unwrap();
    assert_eq!(stream.max_retries, Some(7));
    assert_eq!(stream.max_retry_delay_ms, Some(4444));

    let compaction = config.compaction.unwrap();
    assert!(compaction.settings.enabled);
    assert_eq!(compaction.settings.reserve_tokens, 1234);
    assert_eq!(compaction.settings.keep_recent_tokens, 5678);
}

#[test]
fn build_agent_config_honors_disabled_settings_retry_and_compaction() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None, None).unwrap();
    let mut settings = runtime_settings();
    settings.retry.enabled = false;
    settings.compaction.enabled = false;

    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        None,
        None,
        None,
        AgentResources::default(),
        Some(&settings),
    );

    assert!(config.stream_options.is_none());
    assert!(config.compaction.is_none());
}
