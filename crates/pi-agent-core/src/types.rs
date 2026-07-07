use crate::hooks::AgentHooks;
use futures::Stream;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, StreamOptions,
};
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

// ── ThinkingLevel ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingLevel {
    #[default]
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl std::fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ThinkingLevel::Off => "off",
            ThinkingLevel::Minimal => "minimal",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
            ThinkingLevel::XHigh => "xhigh",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for ThinkingLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off" => Ok(ThinkingLevel::Off),
            "minimal" => Ok(ThinkingLevel::Minimal),
            "low" => Ok(ThinkingLevel::Low),
            "medium" => Ok(ThinkingLevel::Medium),
            "high" => Ok(ThinkingLevel::High),
            "xhigh" => Ok(ThinkingLevel::XHigh),
            _ => Err(format!("unknown thinking level: {}", s)),
        }
    }
}

// ── ToolExecutionMode ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolExecutionMode {
    Sequential,
    #[default]
    Parallel,
}

impl std::fmt::Display for ToolExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ToolExecutionMode::Sequential => "sequential",
            ToolExecutionMode::Parallel => "parallel",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for ToolExecutionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sequential" => Ok(ToolExecutionMode::Sequential),
            "parallel" => Ok(ToolExecutionMode::Parallel),
            _ => Err(format!("unknown tool execution mode: {}", s)),
        }
    }
}

// ── QueueMode ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueMode {
    #[default]
    All,
    OneAtATime,
}

impl std::fmt::Display for QueueMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            QueueMode::All => "all",
            QueueMode::OneAtATime => "one-at-a-time",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for QueueMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(QueueMode::All),
            "one-at-a-time" => Ok(QueueMode::OneAtATime),
            _ => Err(format!("unknown queue mode: {}", s)),
        }
    }
}

// ── AgentToolResult ────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentToolOutput {
    pub content: Vec<ContentBlock>,
    pub details: Option<serde_json::Value>,
}

impl AgentToolOutput {
    pub fn new(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl From<Vec<ContentBlock>> for AgentToolOutput {
    fn from(content: Vec<ContentBlock>) -> Self {
        Self::new(content)
    }
}

// ── AgentToolResult ────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub terminate: bool,
    pub details: Option<serde_json::Value>,
}

impl AgentToolResult {
    pub fn ok(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            is_error: false,
            terminate: false,
            details: None,
        }
    }

    pub fn from_output(output: AgentToolOutput) -> Self {
        Self {
            content: output.content,
            is_error: false,
            terminate: false,
            details: output.details,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: message.into(),
                text_signature: None,
            }],
            is_error: true,
            terminate: false,
            details: None,
        }
    }
}

// ── Resource types ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub location: String,
    pub content: String,
    pub disable_model_invocation: bool,
}

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub content: String,
    pub location: String,
}

#[derive(Debug, Clone, Default)]
pub struct AgentResources {
    pub skills: Vec<Skill>,
    pub prompt_templates: Vec<PromptTemplate>,
}

impl AgentResources {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.prompt_templates.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ResourceDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Provenance for a [`Skill`] or [`PromptTemplate`] loaded by the
/// `load_sourced_*` helpers. Mirrors the `source` parameter of TS
/// `loadSourcedSkills` (`pi/packages/agent/src/harness/skills.ts:83`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTag {
    /// Original input path the entry was loaded from.
    pub source_path: std::path::PathBuf,
    /// Caller-defined provenance label (e.g. "project", "user", "builtin").
    pub source_type: String,
}

/// A [`Skill`] paired with the [`SourceTag`] of the input it was loaded from.
#[derive(Debug, Clone)]
pub struct SourcedSkill {
    pub skill: Skill,
    pub source: SourceTag,
}

/// A [`PromptTemplate`] paired with the [`SourceTag`] of the input it was
/// loaded from.
#[derive(Debug, Clone)]
pub struct SourcedPromptTemplate {
    pub template: PromptTemplate,
    pub source: SourceTag,
}

/// A [`ResourceDiagnostic`] carrying the [`SourceTag`] of the input that
/// produced it.
#[derive(Debug, Clone)]
pub struct SourcedResourceDiagnostic {
    pub diagnostic: ResourceDiagnostic,
    pub source: SourceTag,
}

// ── Compaction types ───────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u32,
    pub keep_recent_tokens: u32,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct CompactionConfig {
    pub settings: CompactionSettings,
    pub custom_instructions: Option<String>,
}


// ── AgentMessage ───────────────────────────────────

#[derive(Debug, Clone)]
pub enum AgentMessage {
    UserText {
        message_id: String,
        text: String,
    },
    Assistant {
        message_id: String,
        message: AssistantMessage,
    },
    ToolResult {
        message_id: String,
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        content: Vec<ContentBlock>,
    },
    SystemPrompt {
        message_id: String,
        text: String,
    },
    CompactionSummary {
        message_id: String,
        summary: String,
        tokens_before: u32,
    },
    BashExecution {
        message_id: String,
        command: String,
        output: String,
        exit_code: Option<i32>,
        cancelled: bool,
        truncated: bool,
        full_output_path: Option<String>,
        exclude_from_context: bool,
        timestamp: u64,
    },
    Custom {
        message_id: String,
        custom_type: String,
        content: Vec<ContentBlock>,
        display: bool,
        details: Option<serde_json::Value>,
        timestamp: u64,
    },
    BranchSummary {
        message_id: String,
        summary: String,
        from_id: String,
        timestamp: u64,
    },
}

// ── AgentTool ──────────────────────────────────────

pub type ToolFn = Arc<
    dyn Fn(
            serde_json::Value,
            Option<ToolUpdateCallback>,
        ) -> Pin<Box<dyn Future<Output = Result<AgentToolOutput, String>> + Send>>
        + Send
        + Sync,
>;
pub type ToolUpdateCallback = Arc<dyn Fn(AgentToolOutput) + Send + Sync>;
pub type ProviderStreamer =
    Arc<dyn Fn(&Model, Context, Option<StreamOptions>) -> EventStream + Send + Sync>;

#[derive(Clone)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub execute: ToolFn,
    pub execution_mode: Option<ToolExecutionMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolDefinitionError {
    field: &'static str,
    message: String,
}

impl AgentToolDefinitionError {
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    pub fn field(&self) -> &'static str {
        self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for AgentToolDefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid agent tool {}: {}", self.field, self.message)
    }
}

impl std::error::Error for AgentToolDefinitionError {}

impl AgentTool {
    pub fn validate(&self) -> Result<(), AgentToolDefinitionError> {
        if self.name.trim().is_empty() {
            return Err(AgentToolDefinitionError::new(
                "name",
                "tool name must not be empty",
            ));
        }
        if self.description.trim().is_empty() {
            return Err(AgentToolDefinitionError::new(
                "description",
                "tool description must not be empty",
            ));
        }
        if !self.parameters.is_object() {
            return Err(AgentToolDefinitionError::new(
                "parameters",
                "tool parameters schema must be a JSON object",
            ));
        }
        Ok(())
    }

    pub fn new_text<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        f: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            execution_mode: None,
            execute: Arc::new(move |args, _on_update| {
                let fut = f(args);
                Box::pin(async move {
                    fut.await.map(|text| {
                        AgentToolOutput::new(vec![ContentBlock::Text {
                            text,
                            text_signature: None,
                        }])
                    })
                })
            }),
        }
    }
}

// ── AgentConfig ────────────────────────────────────

#[derive(Clone)]
pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    /// Optional turn ceiling. `None` means no hard cap (the loop only stops
    /// when the model finishes or an explicit hook requests it). Provided to
    /// match the TS `pi/packages/agent` `while (true)` semantics.
    pub max_turns: Option<u32>,
    pub stream_options: Option<StreamOptions>,
    pub thinking_level: ThinkingLevel,
    pub tool_execution: ToolExecutionMode,
    pub steering_mode: QueueMode,
    pub follow_up_mode: QueueMode,
    pub hooks: AgentHooks,
    pub resources: AgentResources,
    pub compaction: Option<CompactionConfig>,
    pub provider_streamer: Option<ProviderStreamer>,
}

impl std::fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field("model", &self.model)
            .field("system_prompt", &self.system_prompt)
            .field("max_turns", &self.max_turns)
            .field("stream_options", &self.stream_options)
            .field("thinking_level", &self.thinking_level)
            .field("tool_execution", &self.tool_execution)
            .field("steering_mode", &self.steering_mode)
            .field("follow_up_mode", &self.follow_up_mode)
            .field("hooks", &self.hooks)
            .field("resources", &self.resources)
            .field("compaction", &self.compaction)
            .field("provider_streamer", &self.provider_streamer.is_some())
            .finish()
    }
}

impl AgentConfig {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            system_prompt: None,
            max_turns: None,
            stream_options: None,
            thinking_level: ThinkingLevel::Off,
            tool_execution: ToolExecutionMode::Parallel,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            hooks: AgentHooks::default(),
            resources: AgentResources::default(),
            compaction: None,
            provider_streamer: None,
        }
    }
}

// ── Provider request snapshots ──────────────────────────

#[derive(Debug, Clone)]
pub struct ProviderRequestSnapshot {
    pub model: Model,
    pub context: Context,
    pub stream_options: StreamOptions,
}

// ── AgentEvent ─────────────────────────────────────

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum AgentEvent {
    TurnStart {
        turn: u32,
    },
    BeforeProviderRequest {
        request: ProviderRequestSnapshot,
    },
    LlmEvent(AssistantMessageEvent),
    ToolCallStart {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    ToolCallUpdate {
        tool_call_id: String,
        tool_name: String,
        update: AgentToolOutput,
    },
    ToolCallEnd {
        tool_call_id: String,
        tool_name: String,
        result: AgentToolResult,
    },
    AgentDone {
        message: AssistantMessage,
    },
    AgentError {
        error: String,
    },
    SessionCompacted {
        summary: String,
        first_kept_message_id: String,
        tokens_before: u32,
        details: Option<serde_json::Value>,
    },
}

// ── AgentStream ────────────────────────────────────

pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

// ── Unit tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
            execution_mode: None,
            execute: Arc::new(|args, _on_update| {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no text");
                let result: Vec<ContentBlock> = vec![ContentBlock::Text {
                    text: text.to_string(),
                    text_signature: None,
                }];
                Box::pin(async move { Ok(AgentToolOutput::new(result)) })
            }),
        }
    }

    #[test]
    fn agent_message_user_text_constructs() {
        let msg = AgentMessage::UserText {
            message_id: "1".into(),
            text: "hello".into(),
        };
        match &msg {
            AgentMessage::UserText { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn agent_tool_has_correct_fields() {
        let tool = make_text_tool();
        assert_eq!(tool.name, "echo");
        assert!(tool.description.contains("echoes"));
    }

    #[tokio::test]
    async fn tool_fn_executes() {
        let tool = make_text_tool();
        let result = (tool.execute)(serde_json::json!({"text": "hi"}), None).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.content.len(), 1);
        assert_eq!(output.details, None);
    }

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
        let model = pi_ai::types::Model {
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

    #[test]
    fn agent_tool_validation_accepts_object_schema() {
        let tool = make_text_tool();

        assert!(tool.validate().is_ok());
    }

    #[test]
    fn agent_tool_validation_rejects_empty_name() {
        let mut tool = make_text_tool();
        tool.name = "  ".into();

        let error = tool.validate().unwrap_err();

        assert_eq!(error.field(), "name");
        assert!(error.to_string().contains("tool name"));
    }

    #[test]
    fn agent_tool_validation_rejects_non_object_parameters() {
        let mut tool = make_text_tool();
        tool.parameters = serde_json::json!(["not", "a", "schema"]);

        let error = tool.validate().unwrap_err();

        assert_eq!(error.field(), "parameters");
        assert!(error.to_string().contains("JSON object"));
    }

    #[test]
    fn agent_tool_result_ok_constructs() {
        let result = AgentToolResult::ok(vec![ContentBlock::Text {
            text: "hello".into(),
            text_signature: None,
        }]);
        assert!(!result.is_error);
        assert!(!result.terminate);
        assert_eq!(result.details, None);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn agent_tool_result_error_constructs() {
        let result = AgentToolResult::error("something went wrong");
        assert!(result.is_error);
        assert!(!result.terminate);
        assert_eq!(result.details, None);
        assert_eq!(result.content.len(), 1);
    }
}
