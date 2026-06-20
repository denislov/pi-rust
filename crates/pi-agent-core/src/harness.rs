use crate::errors::{AgentHarnessError, AgentHarnessErrorCode};
use crate::hooks::{
    BeforeProviderRequestContext as LoopBeforeProviderRequest,
    BeforeProviderRequestHook as LoopBeforeProviderRequestHook,
    BeforeProviderRequestResult as LoopBeforeProviderRequestResult,
};
use crate::{Agent, AgentConfig, AgentEvent, AgentMessage, AgentStream, AgentTool};
use futures::{Stream, StreamExt};
use pi_ai::types::{Context, Model, ProviderResponseInfo, ProviderStreamHooks, StreamOptions};
use std::collections::BTreeMap;
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
    pub before_provider_payload: Option<BeforeProviderPayloadHook>,
    pub after_provider_response: Option<AfterProviderResponseHook>,
    pub get_api_key_and_headers: Option<GetApiKeyAndHeadersHook>,
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
            .field(
                "before_provider_payload",
                &self.before_provider_payload.is_some(),
            )
            .field(
                "after_provider_response",
                &self.after_provider_response.is_some(),
            )
            .field(
                "get_api_key_and_headers",
                &self.get_api_key_and_headers.is_some(),
            )
            .finish()
    }
}

pub type BeforeAgentStartHook =
    Arc<dyn Fn(HarnessContext) -> HarnessHookFuture<HarnessContext> + Send + Sync>;
pub type ContextHook =
    Arc<dyn Fn(HarnessContext) -> HarnessHookFuture<HarnessContext> + Send + Sync>;
pub type BeforeProviderRequestHook = Arc<
    dyn Fn(BeforeProviderRequest) -> HarnessHookFuture<BeforeProviderRequestPatch> + Send + Sync,
>;
pub type BeforeProviderPayloadHook = Arc<
    dyn Fn(BeforeProviderPayload) -> HarnessHookFuture<BeforeProviderPayloadPatch> + Send + Sync,
>;
pub type AfterProviderResponseHook =
    Arc<dyn Fn(ProviderResponse) -> HarnessHookFuture<()> + Send + Sync>;
pub type GetApiKeyAndHeadersHook =
    Arc<dyn Fn(Model) -> HarnessHookFuture<ProviderAuth> + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub enum Patch<T> {
    Set(T),
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeaderPatch {
    Set(serde_json::Value),
    Clear,
    Merge(BTreeMap<String, Option<serde_json::Value>>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamOptionsPatch {
    pub temperature: Option<Patch<f64>>,
    pub max_tokens: Option<Patch<u32>>,
    pub api_key: Option<Patch<String>>,
    pub cache_retention: Option<Patch<serde_json::Value>>,
    pub thinking: Option<Patch<pi_ai::types::ThinkingConfig>>,
    pub tool_choice: Option<Patch<serde_json::Value>>,
    pub session_id: Option<Patch<String>>,
    pub azure_api_version: Option<Patch<String>>,
    pub azure_resource_name: Option<Patch<String>>,
    pub azure_base_url: Option<Patch<String>>,
    pub azure_deployment_name: Option<Patch<String>>,
    pub bedrock_region: Option<Patch<String>>,
    pub bedrock_profile: Option<Patch<String>>,
    pub bedrock_bearer_token: Option<Patch<String>>,
    pub headers: Option<HeaderPatch>,
    pub timeout_ms: Option<Patch<u64>>,
    pub max_retries: Option<Patch<u32>>,
    pub max_retry_delay_ms: Option<Patch<u64>>,
}

#[derive(Debug, Clone)]
pub struct BeforeProviderRequest {
    pub model: Model,
    pub session_id: Option<String>,
    pub context: Context,
    pub stream_options: StreamOptions,
}

#[derive(Debug, Clone, Default)]
pub struct BeforeProviderRequestPatch {
    pub context: Option<Context>,
    pub stream_options: Option<StreamOptionsPatch>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderAuth {
    pub api_key: Option<String>,
    pub headers: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BeforeProviderPayload {
    pub model: Model,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct BeforeProviderPayloadPatch {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderResponse {
    pub status: Option<u16>,
    pub headers: Option<serde_json::Value>,
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
    base_before_provider_request_hook: Option<LoopBeforeProviderRequestHook>,
}

impl AgentHarness {
    pub fn new(config: AgentConfig) -> Self {
        let base_before_provider_request_hook = config.hooks.before_provider_request.clone();
        Self {
            agent: Agent::new(config),
            hooks: AgentHarnessHooks::default(),
            base_before_provider_request_hook,
        }
    }

    pub fn with_hooks(mut self, hooks: AgentHarnessHooks) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn add_message(&self, message: AgentMessage) {
        self.agent.add_message(message);
    }

    pub fn add_tool(&self, tool: AgentTool) {
        self.agent.add_tool(tool);
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
        let base_provider_hook = self.base_before_provider_request_hook.clone();

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
            let base_provider_hook = base_provider_hook.clone();
            if base_provider_hook.is_some()
                || hooks.before_provider_request.is_some()
                || hooks.before_provider_payload.is_some()
                || hooks.after_provider_response.is_some()
                || hooks.get_api_key_and_headers.is_some()
            {
                agent.set_before_provider_request_hook(Some(make_provider_request_hook(
                    hooks.clone(),
                    base_provider_hook,
                )));
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
        AgentEvent::BeforeProviderRequest { request } => AgentHarnessEvent::BeforeProviderRequest {
            request: BeforeProviderRequest {
                model: request.model.clone(),
                session_id: request.stream_options.session_id.clone(),
                context: request.context.clone(),
                stream_options: request.stream_options.clone(),
            },
        },
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

fn make_provider_request_hook(
    hooks: AgentHarnessHooks,
    base_hook: Option<LoopBeforeProviderRequestHook>,
) -> LoopBeforeProviderRequestHook {
    Arc::new(move |request: LoopBeforeProviderRequest| {
        let hooks = hooks.clone();
        let base_hook = base_hook.clone();
        Box::pin(async move {
            let mut context = request.context;
            let mut stream_options = request.stream_options;
            let model = request.model;

            if let Some(base_hook) = base_hook {
                if let Some(update) = base_hook(LoopBeforeProviderRequest {
                    model: model.clone(),
                    context: context.clone(),
                    stream_options: stream_options.clone(),
                })
                .await?
                {
                    if let Some(updated_context) = update.context {
                        context = updated_context;
                    }
                    if let Some(updated_stream_options) = update.stream_options {
                        stream_options = updated_stream_options;
                    }
                }
            }

            if let Some(auth_hook) = hooks.get_api_key_and_headers.as_ref() {
                let auth = auth_hook(model.clone()).await.map_err(|err| err.message)?;
                if let Some(auth) = auth {
                    if let Some(api_key) = auth.api_key {
                        stream_options.api_key = Some(api_key);
                    }
                    if let Some(headers) = auth.headers {
                        stream_options.headers = merge_headers(stream_options.headers, headers);
                    }
                }
            }

            if let Some(hook) = hooks.before_provider_request.as_ref() {
                let request = BeforeProviderRequest {
                    model: model.clone(),
                    session_id: stream_options.session_id.clone(),
                    context: context.clone(),
                    stream_options: stream_options.clone(),
                };
                if let Some(patch) = hook(request).await.map_err(|err| err.message)? {
                    if let Some(updated_context) = patch.context {
                        context = updated_context;
                    }
                    if let Some(stream_options_patch) = patch.stream_options {
                        stream_options =
                            apply_stream_options_patch(stream_options, stream_options_patch);
                    }
                }
            }

            if hooks.before_provider_payload.is_some() || hooks.after_provider_response.is_some() {
                stream_options.hooks = Some(make_stream_hooks(
                    stream_options.hooks.clone(),
                    hooks.clone(),
                ));
            }

            Ok(Some(LoopBeforeProviderRequestResult {
                context: Some(context),
                stream_options: Some(stream_options),
            }))
        })
    })
}

fn make_stream_hooks(
    prior: Option<ProviderStreamHooks>,
    hooks: AgentHarnessHooks,
) -> ProviderStreamHooks {
    let payload_prior = prior.clone();
    let response_prior = prior;
    let payload_hooks = hooks.clone();
    let response_hooks = hooks;

    ProviderStreamHooks {
        on_payload: Some(Arc::new(move |model, payload| {
            let prior = payload_prior.clone();
            let hooks = payload_hooks.clone();
            Box::pin(async move {
                let mut payload = if let Some(prior) = prior.as_ref() {
                    prior.apply_payload(&model, payload).await?
                } else {
                    payload
                };
                if let Some(hook) = hooks.before_provider_payload.as_ref()
                    && let Some(patch) = hook(BeforeProviderPayload {
                        model: model.clone(),
                        payload: payload.clone(),
                    })
                    .await
                    .map_err(|err| err.message)?
                {
                    payload = patch.payload;
                }
                Ok(payload)
            })
        })),
        on_response: Some(Arc::new(move |response: ProviderResponseInfo| {
            let prior = response_prior.clone();
            let hooks = response_hooks.clone();
            Box::pin(async move {
                if let Some(prior) = prior.as_ref() {
                    prior.emit_response(response.clone()).await?;
                }
                if let Some(hook) = hooks.after_provider_response.as_ref() {
                    let _ = hook(ProviderResponse {
                        status: response.status,
                        headers: response.headers,
                    })
                    .await
                    .map_err(|err| err.message)?;
                }
                Ok(())
            })
        })),
    }
}

pub fn apply_stream_options_patch(
    mut base: StreamOptions,
    patch: StreamOptionsPatch,
) -> StreamOptions {
    apply_patch_value(&mut base.temperature, patch.temperature);
    apply_patch_value(&mut base.max_tokens, patch.max_tokens);
    apply_patch_value(&mut base.api_key, patch.api_key);
    apply_patch_value(&mut base.cache_retention, patch.cache_retention);
    apply_patch_value(&mut base.thinking, patch.thinking);
    apply_patch_value(&mut base.tool_choice, patch.tool_choice);
    apply_patch_value(&mut base.session_id, patch.session_id);
    apply_patch_value(&mut base.azure_api_version, patch.azure_api_version);
    apply_patch_value(&mut base.azure_resource_name, patch.azure_resource_name);
    apply_patch_value(&mut base.azure_base_url, patch.azure_base_url);
    apply_patch_value(&mut base.azure_deployment_name, patch.azure_deployment_name);
    apply_patch_value(&mut base.bedrock_region, patch.bedrock_region);
    apply_patch_value(&mut base.bedrock_profile, patch.bedrock_profile);
    apply_patch_value(&mut base.bedrock_bearer_token, patch.bedrock_bearer_token);
    apply_patch_value(&mut base.timeout_ms, patch.timeout_ms);
    apply_patch_value(&mut base.max_retries, patch.max_retries);
    apply_patch_value(&mut base.max_retry_delay_ms, patch.max_retry_delay_ms);
    if let Some(headers) = patch.headers {
        base.headers = apply_header_patch(base.headers, headers);
    }
    base
}

fn apply_patch_value<T>(target: &mut Option<T>, patch: Option<Patch<T>>) {
    match patch {
        Some(Patch::Set(value)) => *target = Some(value),
        Some(Patch::Clear) => *target = None,
        None => {}
    }
}

fn merge_headers(
    base: Option<serde_json::Value>,
    incoming: serde_json::Value,
) -> Option<serde_json::Value> {
    let mut map = match base {
        Some(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    if let serde_json::Value::Object(incoming) = incoming {
        for (key, value) in incoming {
            map.insert(key, value);
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

fn apply_header_patch(
    base: Option<serde_json::Value>,
    patch: HeaderPatch,
) -> Option<serde_json::Value> {
    match patch {
        HeaderPatch::Set(value) => Some(value),
        HeaderPatch::Clear => None,
        HeaderPatch::Merge(entries) => {
            let mut map = match base {
                Some(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };
            for (key, value) in entries {
                if let Some(value) = value {
                    map.insert(key, value);
                } else {
                    map.remove(&key);
                }
            }
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        }
    }
}
