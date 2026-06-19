use pi_agent_core::AgentResources;
use pi_coding_agent::{
    CliError, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, build_agent_config, parse_args, select_model,
};

#[test]
fn selects_default_model_when_no_override_is_provided() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None).unwrap();
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
    let model = select_model(&args, None, None).unwrap();
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
        select_model(&args, None, None).unwrap_err(),
        CliError::UnknownModel("missing-model".into())
    );
}

#[test]
fn model_override_is_used_when_cli_model_is_absent() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let mut override_model = select_model(&args, None, None).unwrap();
    override_model.id = "override-model".into();
    let model = select_model(&args, None, Some(override_model)).unwrap();
    assert_eq!(model.id, "override-model");
}

#[test]
fn builds_agent_config_with_defaults() {
    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let model = select_model(&args, None, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
        None,
        None,
        AgentResources::default(),
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
    let model = select_model(&args, None, None).unwrap();
    let config = build_agent_config(
        model,
        args.system_prompt.clone(),
        args.max_turns,
        args.api_key.clone(),
        None,
        None,
        AgentResources::default(),
    );
    assert_eq!(config.system_prompt.as_deref(), Some("Be brief."));
    assert_eq!(config.max_turns, 9);
    assert_eq!(
        config.stream_options.unwrap().api_key.as_deref(),
        Some("sk-test")
    );
}
