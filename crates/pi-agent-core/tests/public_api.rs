use pi_agent_core::api::{
    Agent, AgentConfig, AgentEvent, AgentHooks, AgentMessage, AgentResources, AgentStream,
    AgentTool, AgentToolOutput, AgentToolResult, CompactionConfig, CompactionSettings, ExecOptions,
    ExecutionEnv, ExecutionError, ExecutionOutput, FileError, FileInfo, FileKind, FileSystem,
    InMemoryExecutionEnv, PromptTemplate, ProviderRequestSnapshot, QueueMode, ResourceDiagnostic,
    Shell, Skill, SourceTag, SourcedPromptTemplate, SourcedResourceDiagnostic, SourcedSkill,
    ThinkingLevel, ToolExecutionMode, ToolFn, ToolUpdateCallback,
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
        None,
    );

    fn accepts_traits<T: ExecutionEnv + FileSystem + Shell>(_env: &T) {}
    let env = InMemoryExecutionEnv::new(".");
    accepts_traits(&env);
}
