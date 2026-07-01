use pi_agent_core::api::{
    Agent, AgentConfig, AgentEvent, AgentHooks, AgentMessage, AgentResources, AgentStream,
    AgentTool, AgentToolDefinitionError, AgentToolOutput, AgentToolResult, CompactionConfig,
    CompactionSettings, ExecOptions, ExecutionEnv, ExecutionError, ExecutionOutput, FileError,
    FileInfo, FileKind, FileSystem, InMemoryExecutionEnv, PromptTemplate, ProviderRequestSnapshot,
    QueueMode, ResourceDiagnostic, Shell, Skill, SourceTag, SourcedPromptTemplate,
    SourcedResourceDiagnostic, SourcedSkill, ThinkingLevel, ToolExecutionMode, ToolFn,
    ToolUpdateCallback,
};

#[test]
fn low_level_runtime_symbols_are_importable_from_api_facade() {
    fn accepts_types(
        _agent: Option<Agent>,
        _config: Option<AgentConfig>,
        _event: Option<AgentEvent>,
        _hooks: Option<AgentHooks>,
        _message: Option<AgentMessage>,
        _resources: Option<AgentResources>,
        _stream: Option<AgentStream>,
        _tool: Option<AgentTool>,
        _tool_definition_error: Option<AgentToolDefinitionError>,
        _tool_output: Option<AgentToolOutput>,
        _tool_result: Option<AgentToolResult>,
        _compaction_config: Option<CompactionConfig>,
        _compaction_settings: Option<CompactionSettings>,
        _exec_options: Option<ExecOptions>,
        _execution_output: Option<ExecutionOutput>,
        _file_info: Option<FileInfo>,
        _file_kind: Option<FileKind>,
        _prompt_template: Option<PromptTemplate>,
        _provider_snapshot: Option<ProviderRequestSnapshot>,
        _queue_mode: Option<QueueMode>,
        _diagnostic: Option<ResourceDiagnostic>,
        _skill: Option<Skill>,
        _source_tag: Option<SourceTag>,
        _sourced_template: Option<SourcedPromptTemplate>,
        _sourced_diagnostic: Option<SourcedResourceDiagnostic>,
        _sourced_skill: Option<SourcedSkill>,
        _thinking: Option<ThinkingLevel>,
        _tool_mode: Option<ToolExecutionMode>,
        _tool_fn: Option<ToolFn>,
        _tool_update: Option<ToolUpdateCallback>,
        _execution_error: Option<ExecutionError>,
        _file_error: Option<FileError>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None,
    );

    fn accepts_traits<T: ExecutionEnv + FileSystem + Shell>(_env: &T) {}
    let env = InMemoryExecutionEnv::new(".");
    accepts_traits(&env);
}

#[test]
fn try_add_tool_rejects_invalid_tool_before_provider_context() {
    let agent = Agent::new(AgentConfig::new(test_model()));
    let invalid_tool = AgentTool::new_text(
        " ",
        "empty names are invalid",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    );

    let error = agent.try_add_tool(invalid_tool).unwrap_err();

    assert_eq!(error.field(), "name");
    let (context, _) = agent.provider_request_snapshot();
    assert!(context.tools.unwrap_or_default().is_empty());
}

fn test_model() -> pi_ai::types::Model {
    pi_ai::types::Model {
        id: "test".into(),
        name: "Test".into(),
        api: "test-api".into(),
        provider: "test-provider".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![pi_ai::types::ModelInput::Text],
        cost: pi_ai::types::ModelCost::default(),
        context_window: 8000,
        max_tokens: 1024,
        headers: None,
        compat: None,
    }
}
