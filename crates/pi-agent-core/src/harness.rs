use crate::errors::{AgentHarnessError, AgentHarnessErrorCode};
use crate::{Agent, AgentConfig, AgentEvent, AgentMessage, AgentStream};
use futures::{Stream, StreamExt};
use pi_ai::types::{Context, StreamOptions};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type AgentHarnessStream = Pin<Box<dyn Stream<Item = AgentHarnessEvent> + Send>>;
pub type HarnessHookFuture<T> =
    Pin<Box<dyn Future<Output = Result<Option<T>, AgentHarnessError>> + Send>>;

#[derive(Debug, Clone)]
pub struct HarnessContext {
    pub messages: Vec<AgentMessage>,
    pub system_prompt: Option<String>,
}

#[derive(Clone, Default)]
pub struct AgentHarnessHooks {
    pub before_agent_start: Option<BeforeAgentStartHook>,
    pub context: Option<ContextHook>,
    pub before_provider_request: Option<BeforeProviderRequestHook>,
}

impl std::fmt::Debug for AgentHarnessHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHarnessHooks")
            .field("before_agent_start", &self.before_agent_start.is_some())
            .field("context", &self.context.is_some())
            .field(
                "before_provider_request",
                &self.before_provider_request.is_some(),
            )
            .finish()
    }
}

pub type BeforeAgentStartHook =
    Arc<dyn Fn(HarnessContext) -> HarnessHookFuture<HarnessContext> + Send + Sync>;
pub type ContextHook =
    Arc<dyn Fn(HarnessContext) -> HarnessHookFuture<HarnessContext> + Send + Sync>;
pub type BeforeProviderRequestHook =
    Arc<dyn Fn(BeforeProviderRequest) -> HarnessHookFuture<BeforeProviderRequest> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct BeforeProviderRequest {
    pub context: Context,
    pub stream_options: Option<StreamOptions>,
}

#[derive(Debug, Clone)]
pub enum AgentHarnessEvent {
    Agent(AgentEvent),
    BeforeAgentStart {
        context: HarnessContext,
    },
    Context {
        context: HarnessContext,
    },
    BeforeProviderRequest {
        request: BeforeProviderRequest,
    },
    BeforeProviderPayload {
        payload: serde_json::Value,
    },
    AfterProviderResponse {
        status: Option<u16>,
    },
    ToolCall {
        tool_call_id: String,
        tool_name: String,
    },
    ToolResult {
        tool_call_id: String,
        tool_name: String,
    },
    SessionBeforeCompact,
    SessionCompact,
    SessionBeforeTree,
    SessionTree,
    ModelUpdate,
    ThinkingLevelUpdate,
    ResourcesUpdate,
    ToolsUpdate,
    QueueUpdate,
    SavePoint,
    Abort,
    Settled,
    Error {
        error: AgentHarnessError,
    },
}

#[derive(Clone)]
pub struct AgentHarness {
    agent: Agent,
    hooks: AgentHarnessHooks,
}

impl AgentHarness {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            agent: Agent::new(config),
            hooks: AgentHarnessHooks::default(),
        }
    }

    pub fn with_hooks(mut self, hooks: AgentHarnessHooks) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn add_message(&self, message: AgentMessage) {
        self.agent.add_message(message);
    }

    pub fn messages(&self) -> Vec<AgentMessage> {
        self.agent.messages()
    }

    pub fn abort(&self) {
        self.agent.abort();
    }

    pub fn prompt(&self, text: &str) -> AgentHarnessStream {
        let mut messages = self.agent.messages();
        messages.push(AgentMessage::UserText {
            message_id: format!("user_{}", messages.len()),
            text: text.to_string(),
        });
        let agent = self.agent.clone();
        let hooks = self.hooks.clone();
        let config = {
            let mut context_config = None;
            if let Some(first) = messages.first()
                && matches!(first, AgentMessage::SystemPrompt { .. })
            {
                context_config = None;
            }
            context_config
        };

        Box::pin(async_stream::stream! {
            let mut harness_context = HarnessContext {
                messages,
                system_prompt: config,
            };
            yield AgentHarnessEvent::BeforeAgentStart {
                context: harness_context.clone(),
            };

            if let Some(hook) = hooks.before_agent_start.as_ref() {
                match hook(harness_context.clone()).await {
                    Ok(Some(updated)) => harness_context = updated,
                    Ok(None) => {}
                    Err(error) => {
                        yield AgentHarnessEvent::Error { error };
                        yield AgentHarnessEvent::Settled;
                        return;
                    }
                }
            }

            yield AgentHarnessEvent::Context {
                context: harness_context.clone(),
            };

            if let Some(hook) = hooks.context.as_ref() {
                match hook(harness_context.clone()).await {
                    Ok(Some(updated)) => harness_context = updated,
                    Ok(None) => {}
                    Err(error) => {
                        yield AgentHarnessEvent::Error { error };
                        yield AgentHarnessEvent::Settled;
                        return;
                    }
                }
            }

            agent.replace_messages(harness_context.messages.clone());
            let (request_context, request_stream_options) = agent.provider_request_snapshot();
            let mut before_request = BeforeProviderRequest {
                context: request_context,
                stream_options: request_stream_options,
            };
            let mut request_overridden = false;
            yield AgentHarnessEvent::BeforeProviderRequest {
                request: before_request.clone(),
            };

            if let Some(hook) = hooks.before_provider_request.as_ref() {
                match hook(before_request.clone()).await {
                    Ok(Some(updated)) => {
                        before_request = updated;
                        request_overridden = true;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        yield AgentHarnessEvent::Error { error };
                        yield AgentHarnessEvent::Settled;
                        return;
                    }
                }
            }

            if request_overridden {
                agent.set_provider_request_override(
                    before_request.context,
                    before_request.stream_options,
                );
            }
            let mut stream: AgentStream = agent.run();
            while let Some(event) = stream.next().await {
                yield map_agent_event(event);
            }
            yield AgentHarnessEvent::Settled;
        })
    }
}

fn map_agent_event(event: AgentEvent) -> AgentHarnessEvent {
    match &event {
        AgentEvent::ToolCallStart {
            tool_call_id,
            tool_name,
        } => AgentHarnessEvent::ToolCall {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
        },
        AgentEvent::ToolCallEnd {
            tool_call_id,
            tool_name,
            ..
        } => AgentHarnessEvent::ToolResult {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
        },
        AgentEvent::SessionCompacted { .. } => AgentHarnessEvent::SessionCompact,
        _ => AgentHarnessEvent::Agent(event),
    }
}

impl From<String> for AgentHarnessError {
    fn from(message: String) -> Self {
        AgentHarnessError::new(AgentHarnessErrorCode::Unknown, message)
    }
}
