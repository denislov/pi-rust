use crate::hooks::AgentHooks;
use futures::Stream;
use pi_ai::types::{AssistantMessage, AssistantMessageEvent, ContentBlock, Model, StreamOptions};
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
pub struct AgentToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub terminate: bool,
}

impl AgentToolResult {
    pub fn ok(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            is_error: false,
            terminate: false,
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
pub struct CompactionConfig {
    pub settings: CompactionSettings,
    pub custom_instructions: Option<String>,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            settings: CompactionSettings::default(),
            custom_instructions: None,
        }
    }
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
        ) -> Pin<Box<dyn Future<Output = Result<Vec<ContentBlock>, String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub execute: ToolFn,
    pub execution_mode: Option<ToolExecutionMode>,
}

impl AgentTool {
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
            execute: Arc::new(move |args| {
                let fut = f(args);
                Box::pin(async move {
                    fut.await.map(|text| {
                        vec![ContentBlock::Text {
                            text,
                            text_signature: None,
                        }]
                    })
                })
            }),
        }
    }
}

// ── AgentConfig ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    pub max_turns: u32,
    pub stream_options: Option<StreamOptions>,
    pub thinking_level: ThinkingLevel,
    pub tool_execution: ToolExecutionMode,
    pub steering_mode: QueueMode,
    pub follow_up_mode: QueueMode,
    pub hooks: AgentHooks,
    pub resources: AgentResources,
    pub compaction: Option<CompactionConfig>,
}

impl AgentConfig {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            system_prompt: None,
            max_turns: 30,
            stream_options: None,
            thinking_level: ThinkingLevel::Off,
            tool_execution: ToolExecutionMode::Parallel,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            hooks: AgentHooks::default(),
            resources: AgentResources::default(),
            compaction: None,
        }
    }
}

// ── AgentEvent ─────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TurnStart {
        turn: u32,
    },
    LlmEvent(AssistantMessageEvent),
    ToolCallStart {
        tool_call_id: String,
        tool_name: String,
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
            execute: Arc::new(|args| {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no text");
                let result: Vec<ContentBlock> = vec![ContentBlock::Text {
                    text: text.to_string(),
                    text_signature: None,
                }];
                Box::pin(async move { Ok(result) })
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
        let result = (tool.execute)(serde_json::json!({"text": "hi"})).await;
        assert!(result.is_ok());
        let blocks = result.unwrap();
        assert_eq!(blocks.len(), 1);
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
    fn agent_tool_result_ok_constructs() {
        let result = AgentToolResult::ok(vec![ContentBlock::Text {
            text: "hello".into(),
            text_signature: None,
        }]);
        assert!(!result.is_error);
        assert!(!result.terminate);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn agent_tool_result_error_constructs() {
        let result = AgentToolResult::error("something went wrong");
        assert!(result.is_error);
        assert!(!result.terminate);
        assert_eq!(result.content.len(), 1);
    }
}
