use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use pi_ai::api::conversation::ContentBlock;
use tokio_util::sync::CancellationToken;

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

// ── AgentTool ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    scope_id: Option<Arc<str>>,
    turn: u32,
    tool_call_id: Arc<str>,
    tool_name: Arc<str>,
    cancel_token: CancellationToken,
}

impl ToolExecutionContext {
    pub fn new(
        scope_id: Option<impl Into<Arc<str>>>,
        turn: u32,
        tool_call_id: impl Into<Arc<str>>,
        tool_name: impl Into<Arc<str>>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            scope_id: scope_id.map(Into::into),
            turn,
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            cancel_token,
        }
    }

    pub fn standalone(tool_name: impl Into<Arc<str>>) -> Self {
        Self::new(
            None::<Arc<str>>,
            0,
            Arc::<str>::from("direct"),
            tool_name,
            CancellationToken::new(),
        )
    }

    pub fn scope_id(&self) -> Option<&str> {
        self.scope_id.as_deref()
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn tool_call_id(&self) -> &str {
        &self.tool_call_id
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }
}

pub type ToolFn = Arc<
    dyn Fn(
            ToolExecutionContext,
            serde_json::Value,
            Option<ToolUpdateCallback>,
        ) -> Pin<Box<dyn Future<Output = Result<AgentToolOutput, String>> + Send>>
        + Send
        + Sync,
>;
pub type ToolUpdateCallback = Arc<dyn Fn(AgentToolOutput) + Send + Sync>;
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
        F: Fn(ToolExecutionContext, serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            execution_mode: None,
            execute: Arc::new(move |context, args, _on_update| {
                let fut = f(context, args);
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
