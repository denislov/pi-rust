use pi_agent_core::api::{AgentConfig, AgentTool, QueueMode, ThinkingLevel, ToolExecutionMode};

#[test]
fn thinking_level_parses_cli_values() {
    assert_eq!("off".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Off);
    assert_eq!(
        "minimal".parse::<ThinkingLevel>().unwrap(),
        ThinkingLevel::Minimal
    );
    assert_eq!("low".parse::<ThinkingLevel>().unwrap(), ThinkingLevel::Low);
    assert_eq!(
        "medium".parse::<ThinkingLevel>().unwrap(),
        ThinkingLevel::Medium
    );
    assert_eq!(
        "high".parse::<ThinkingLevel>().unwrap(),
        ThinkingLevel::High
    );
    assert_eq!(
        "xhigh".parse::<ThinkingLevel>().unwrap(),
        ThinkingLevel::XHigh
    );
    assert!("extreme".parse::<ThinkingLevel>().is_err());
}

#[test]
fn tool_execution_mode_parses_cli_values() {
    assert_eq!(
        "parallel".parse::<ToolExecutionMode>().unwrap(),
        ToolExecutionMode::Parallel
    );
    assert_eq!(
        "sequential".parse::<ToolExecutionMode>().unwrap(),
        ToolExecutionMode::Sequential
    );
    assert!("serial".parse::<ToolExecutionMode>().is_err());
}

#[test]
fn queue_mode_parses_cli_values() {
    assert_eq!("all".parse::<QueueMode>().unwrap(), QueueMode::All);
    assert_eq!(
        "one-at-a-time".parse::<QueueMode>().unwrap(),
        QueueMode::OneAtATime
    );
    assert!("one".parse::<QueueMode>().is_err());
}

#[test]
fn agent_config_defaults_match_m4_baseline() {
    let model = pi_ai::Model {
        id: "test".into(),
        name: "Test".into(),
        api: "test-api".into(),
        provider: "test-provider".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![pi_ai::ModelInput::Text],
        cost: pi_ai::ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    };
    let config = AgentConfig::new(model);
    assert_eq!(config.thinking_level, ThinkingLevel::Off);
    assert_eq!(config.tool_execution, ToolExecutionMode::Parallel);
    assert_eq!(config.steering_mode, QueueMode::OneAtATime);
    assert_eq!(config.follow_up_mode, QueueMode::OneAtATime);
    assert!(config.hooks.is_empty());
    assert!(config.resources.is_empty());
    assert!(config.compaction.is_none());
}

#[test]
fn agent_tool_defaults_to_global_execution_mode() {
    let tool = AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    );
    assert_eq!(tool.execution_mode, None);
}
