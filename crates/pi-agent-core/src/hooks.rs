use crate::types::{AgentMessage, AgentToolResult, ThinkingLevel};
use pi_ai::types::{AssistantMessage, ContentBlock, Model, StreamOptions};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type HookFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

#[derive(Clone, Default)]
pub struct AgentHooks {
    pub before_tool_call: Option<BeforeToolCallHook>,
    pub after_tool_call: Option<AfterToolCallHook>,
    pub should_stop_after_turn: Option<ShouldStopAfterTurnHook>,
    pub prepare_next_turn: Option<PrepareNextTurnHook>,
}

impl std::fmt::Debug for AgentHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHooks")
            .field(
                "before_tool_call",
                &self.before_tool_call.as_ref().map(|_| ".."),
            )
            .field(
                "after_tool_call",
                &self.after_tool_call.as_ref().map(|_| ".."),
            )
            .field(
                "should_stop_after_turn",
                &self.should_stop_after_turn.as_ref().map(|_| ".."),
            )
            .field(
                "prepare_next_turn",
                &self.prepare_next_turn.as_ref().map(|_| ".."),
            )
            .finish()
    }
}

impl AgentHooks {
    pub fn is_empty(&self) -> bool {
        self.before_tool_call.is_none()
            && self.after_tool_call.is_none()
            && self.should_stop_after_turn.is_none()
            && self.prepare_next_turn.is_none()
    }
}

pub type BeforeToolCallHook =
    Arc<dyn Fn(BeforeToolCallContext) -> HookFuture<Option<BeforeToolCallResult>> + Send + Sync>;
pub type AfterToolCallHook =
    Arc<dyn Fn(AfterToolCallContext) -> HookFuture<Option<AfterToolCallResult>> + Send + Sync>;
pub type ShouldStopAfterTurnHook =
    Arc<dyn Fn(ShouldStopAfterTurnContext) -> HookFuture<bool> + Send + Sync>;
pub type PrepareNextTurnHook =
    Arc<dyn Fn(PrepareNextTurnContext) -> HookFuture<Option<AgentLoopTurnUpdate>> + Send + Sync>;

#[derive(Clone)]
pub struct BeforeToolCallContext {
    pub assistant_message: AssistantMessage,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub messages: Vec<AgentMessage>,
}

#[derive(Clone)]
pub struct BeforeToolCallResult {
    pub block: bool,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct AfterToolCallContext {
    pub assistant_message: AssistantMessage,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub result: AgentToolResult,
    pub messages: Vec<AgentMessage>,
}

#[derive(Clone, Default)]
pub struct AfterToolCallResult {
    pub content: Option<Vec<ContentBlock>>,
    pub is_error: Option<bool>,
    pub terminate: Option<bool>,
}

#[derive(Clone)]
pub struct ShouldStopAfterTurnContext {
    pub messages: Vec<AgentMessage>,
    pub assistant_message: AssistantMessage,
}

#[derive(Clone)]
pub struct PrepareNextTurnContext {
    pub messages: Vec<AgentMessage>,
    pub turn: u32,
}

#[derive(Clone, Default)]
pub struct AgentLoopTurnUpdate {
    pub messages: Option<Vec<AgentMessage>>,
    pub thinking_level: Option<ThinkingLevel>,
    pub model: Option<Model>,
    pub stream_options: Option<StreamOptions>,
}
