use std::future::Future;
use std::pin::Pin;

use futures::StreamExt;
use pi_agent_core::api::agent::{AgentEvent, AgentMessage, AgentStream};
use pi_agent_core::api::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome};

use super::CodingSessionError;
use super::context::{CodingDiagnostic, PromptTurnContext, QueuedPromptInput};
use crate::app::bootstrap::PromptInvocation;
use crate::plugins::PromptHookPoint;
use crate::runtime::control::PromptControlCommand;
use crate::services::runtime::RuntimeService;

const DEFAULT_ACTION: &str = "default";

pub(crate) const PROMPT_TURN_NODE_IDS: &[&str] = &[
    "start_prompt_turn",
    "resolve_request",
    "prepare_input",
    "resolve_runtime",
    "load_resources",
    "open_session",
    "build_agent_runtime",
    "record_user_input",
    "run_agent_turn",
    "finalize_turn",
    "emit_completion",
];

const PROMPT_TURN_NODE_SPECS: &[PromptTurnNodeSpec] = &[
    PromptTurnNodeSpec {
        id: "start_prompt_turn",
        name: "StartPromptTurn",
        kind: PromptTurnNodeKind::Default,
    },
    PromptTurnNodeSpec {
        id: "resolve_request",
        name: "ResolveRequest",
        kind: PromptTurnNodeKind::ResolveRequest,
    },
    PromptTurnNodeSpec {
        id: "prepare_input",
        name: "PrepareInput",
        kind: PromptTurnNodeKind::PrepareInput,
    },
    PromptTurnNodeSpec {
        id: "resolve_runtime",
        name: "ResolveRuntime",
        kind: PromptTurnNodeKind::ResolveRuntime,
    },
    PromptTurnNodeSpec {
        id: "load_resources",
        name: "LoadResources",
        kind: PromptTurnNodeKind::LoadResources,
    },
    PromptTurnNodeSpec {
        id: "open_session",
        name: "OpenSession",
        kind: PromptTurnNodeKind::OpenSession,
    },
    PromptTurnNodeSpec {
        id: "build_agent_runtime",
        name: "BuildAgentRuntime",
        kind: PromptTurnNodeKind::BuildAgentRuntime,
    },
    PromptTurnNodeSpec {
        id: "record_user_input",
        name: "RecordUserInput",
        kind: PromptTurnNodeKind::RecordUserInput,
    },
    PromptTurnNodeSpec {
        id: "run_agent_turn",
        name: "RunAgentTurn",
        kind: PromptTurnNodeKind::RunAgentTurn,
    },
    PromptTurnNodeSpec {
        id: "finalize_turn",
        name: "FinalizeTurn",
        kind: PromptTurnNodeKind::FinalizeTurn,
    },
    PromptTurnNodeSpec {
        id: "emit_completion",
        name: "EmitCompletion",
        kind: PromptTurnNodeKind::EmitCompletion,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromptTurnNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: PromptTurnNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptTurnNodeKind {
    Default,
    ResolveRequest,
    PrepareInput,
    ResolveRuntime,
    LoadResources,
    OpenSession,
    BuildAgentRuntime,
    RecordUserInput,
    RunAgentTurn,
    FinalizeTurn,
    EmitCompletion,
}

pub(crate) struct PromptTurnFlow {
    flow: Flow<PromptTurnContext>,
}

impl PromptTurnFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(PROMPT_TURN_NODE_IDS[0]).map_err(flow_error)?;
        for spec in PROMPT_TURN_NODE_SPECS {
            flow.add_node(spec.id, PromptTurnNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in PROMPT_TURN_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct PromptTurnNode {
    name: &'static str,
    kind: PromptTurnNodeKind,
}

impl PromptTurnNode {
    fn new(name: &'static str, kind: PromptTurnNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<PromptTurnContext> for PromptTurnNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut PromptTurnContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            match self.kind {
                PromptTurnNodeKind::Default => default_action(),
                PromptTurnNodeKind::ResolveRequest => resolve_request(ctx),
                PromptTurnNodeKind::PrepareInput => prepare_input(ctx),
                PromptTurnNodeKind::ResolveRuntime => resolve_runtime(ctx),
                PromptTurnNodeKind::LoadResources => load_resources(ctx),
                PromptTurnNodeKind::OpenSession => open_session(ctx),
                PromptTurnNodeKind::BuildAgentRuntime => build_agent_runtime(ctx),
                PromptTurnNodeKind::RecordUserInput => record_user_input(ctx),
                PromptTurnNodeKind::RunAgentTurn => run_agent_turn(ctx).await,
                PromptTurnNodeKind::FinalizeTurn => finalize_turn(ctx),
                PromptTurnNodeKind::EmitCompletion => emit_completion(ctx),
            }
        })
    }
}

fn resolve_request(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.resolve_request().map_err(|error| error.to_string())?;
    default_action()
}

fn prepare_input(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.run_prompt_hook(PromptHookPoint::BeforePromptPrepare)
        .map_err(|error| error.to_string())?;
    ctx.prepare_input().map_err(|error| error.to_string())?;
    ctx.run_prompt_hook(PromptHookPoint::AfterInputPrepared)
        .map_err(|error| error.to_string())?;
    default_action()
}

fn resolve_runtime(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.resolve_runtime_from_options()
        .map_err(|error| error.to_string())?;
    default_action()
}

fn load_resources(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.load_resources_from_runtime()
        .map_err(|error| error.to_string())?;
    ctx.run_prompt_hook(PromptHookPoint::AfterResourcesLoaded)
        .map_err(|error| error.to_string())?;
    default_action()
}

fn open_session(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    if ctx.session_id().is_some() {
        if ctx.replay().is_none() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot continue before session replay is loaded".into(),
            }
            .to_string());
        }
        if !ctx.has_active_transaction() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot continue before a turn transaction is active".into(),
            }
            .to_string());
        }
        return default_action();
    }

    if ctx.non_persistent_runtime_id().is_some() {
        if ctx.replay().is_none() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot continue before non-persistent replay is loaded"
                    .into(),
            }
            .to_string());
        }
        return default_action();
    }

    if ctx.session_id().is_none() {
        return Err(CodingSessionError::Session {
            message: "prompt turn cannot continue before a session is opened".into(),
        }
        .to_string());
    }
    default_action()
}

fn build_agent_runtime(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    if ctx.agent().is_some() {
        return default_action();
    }

    if ctx.loaded_resources().is_none() {
        return Err(CodingSessionError::Config {
            message: "prompt turn cannot build agent runtime before resources are loaded".into(),
        }
        .to_string());
    }

    let runtime = ctx.runtime().cloned().ok_or_else(|| {
        CodingSessionError::Config {
            message: "prompt turn cannot build agent runtime without a runtime snapshot".into(),
        }
        .to_string()
    })?;
    let snapshot = ctx.capability_snapshot().ok_or_else(|| {
        CodingSessionError::UnsupportedCapability {
            capability: "prompt runtime build requires operation capability snapshot".into(),
        }
        .to_string()
    })?;
    let service = RuntimeService::new();
    let authorization = ctx.authorization_hook_context();
    let build = service
        .build_agent_runtime_with_authorization(
            &runtime,
            ctx.plugin_service(),
            snapshot,
            authorization,
        )
        .map_err(|error| error.to_string())?;
    for diagnostic in build.diagnostics {
        ctx.record_diagnostic(diagnostic);
    }
    if let Some(replay) = ctx.replay() {
        service.hydrate_agent_runtime(&build.agent, &runtime, replay);
    }
    for input in ctx.options().queued_steering() {
        match input {
            QueuedPromptInput::Text(text) => build.agent.steer(text.clone()),
            QueuedPromptInput::Content(content) => build.agent.steer_content(content.clone()),
        }
    }
    for input in ctx.options().queued_follow_up() {
        match input {
            QueuedPromptInput::Text(text) => build.agent.follow_up(text.clone()),
            QueuedPromptInput::Content(content) => build.agent.follow_up_content(content.clone()),
        }
    }
    ctx.set_agent(build.agent);
    default_action()
}

fn record_user_input(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.record_user_input().map_err(|error| error.to_string())?;
    default_action()
}

async fn run_agent_turn(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.run_prompt_hook(PromptHookPoint::BeforeAgentTurn)
        .map_err(|error| error.to_string())?;
    let agent = ctx.agent().cloned().ok_or_else(|| {
        CodingSessionError::Session {
            message: "prompt turn has no agent runtime".into(),
        }
        .to_string()
    })?;
    let mut controls = ctx.take_prompt_control_receiver();
    let mut cancellation = ctx.operation_cancellation();
    let mut stream = start_agent_turn(ctx).map_err(|error| error.to_string())?;
    loop {
        let next = match (controls.as_mut(), cancellation.as_ref()) {
            (Some(receiver), Some(cancellation)) => {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => AgentTurnInput::Cancellation,
                    command = receiver.recv() => AgentTurnInput::Control(command),
                    event = stream.next() => AgentTurnInput::Event(event),
                }
            }
            (Some(receiver), None) => {
                tokio::select! {
                    biased;
                    command = receiver.recv() => AgentTurnInput::Control(command),
                    event = stream.next() => AgentTurnInput::Event(event),
                }
            }
            (None, Some(cancellation)) => {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => AgentTurnInput::Cancellation,
                    event = stream.next() => AgentTurnInput::Event(event),
                }
            }
            (None, None) => AgentTurnInput::Event(stream.next().await),
        };

        let event = match next {
            AgentTurnInput::Cancellation => {
                ctx.request_abort("parent operation ended");
                agent.abort();
                cancellation = None;
                continue;
            }
            AgentTurnInput::Control(Some(command)) => {
                apply_prompt_control_command(ctx, &agent, command);
                continue;
            }
            AgentTurnInput::Control(None) => {
                controls = None;
                continue;
            }
            AgentTurnInput::Event(Some(event)) => event,
            AgentTurnInput::Event(None) => break,
        };

        match &event {
            AgentEvent::AgentDone { message } => {
                ctx.record_final_message(message.clone());
            }
            AgentEvent::AgentError { error } => {
                let message = error.clone();
                ctx.record_diagnostic(CodingDiagnostic::error(message.clone()));
                ctx.record_agent_event(event)
                    .map_err(|error| error.to_string())?;
                return Err(CodingSessionError::Provider { message }.to_string());
            }
            _ => {}
        }
        ctx.record_agent_event(event)
            .map_err(|error| error.to_string())?;
    }

    if ctx.final_message().is_none() {
        return Err(CodingSessionError::Provider {
            message: "agent turn ended without a final assistant message".into(),
        }
        .to_string());
    }

    ctx.run_prompt_hook(PromptHookPoint::AfterAgentTurn)
        .map_err(|error| error.to_string())?;
    default_action()
}

enum AgentTurnInput {
    Cancellation,
    Control(Option<PromptControlCommand>),
    Event(Option<AgentEvent>),
}

fn apply_prompt_control_command(
    ctx: &mut PromptTurnContext,
    agent: &pi_agent_core::api::agent::Agent,
    command: PromptControlCommand,
) {
    match command {
        PromptControlCommand::Abort { reason } => {
            ctx.request_abort(reason);
            agent.abort();
        }
        PromptControlCommand::Steer { text } => agent.steer(text),
        PromptControlCommand::SteerContent { content } => agent.steer_content(content),
        PromptControlCommand::FollowUp { text } => agent.follow_up(text),
        PromptControlCommand::FollowUpContent { content } => agent.follow_up_content(content),
    }
}

fn finalize_turn(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    if ctx.final_message().is_none() {
        return Err(CodingSessionError::Session {
            message: "prompt turn cannot finalize without a final assistant message".into(),
        }
        .to_string());
    }

    if ctx.session_id().is_some() {
        if ctx.replay().is_none() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot finalize before session replay is loaded".into(),
            }
            .to_string());
        }
        if !ctx.has_active_transaction() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot finalize before a turn transaction is active".into(),
            }
            .to_string());
        }
        ctx.run_prompt_hook(PromptHookPoint::BeforeSessionCommit)
            .map_err(|error| error.to_string())?;
        return default_action();
    }

    if ctx.non_persistent_runtime_id().is_some() {
        if ctx.replay().is_none() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot finalize before non-persistent replay is loaded"
                    .into(),
            }
            .to_string());
        }
        ctx.run_prompt_hook(PromptHookPoint::BeforeSessionCommit)
            .map_err(|error| error.to_string())?;
        return default_action();
    }

    Err(CodingSessionError::Session {
        message: "prompt turn cannot finalize before a session is opened".into(),
    }
    .to_string())
}

fn emit_completion(ctx: &mut PromptTurnContext) -> Result<Action, String> {
    ctx.record_prompt_completed()
        .map_err(|error| error.to_string())?;
    ctx.run_prompt_hook(PromptHookPoint::AfterSessionCommit)
        .map_err(|error| error.to_string())?;
    default_action()
}

fn start_agent_turn(ctx: &mut PromptTurnContext) -> Result<AgentStream, CodingSessionError> {
    let agent = ctx
        .agent()
        .cloned()
        .ok_or_else(|| CodingSessionError::Session {
            message: "prompt turn has no agent runtime".into(),
        })?;

    match ctx.options().invocation() {
        PromptInvocation::Text(text) if !text.is_empty() => Ok(agent.prompt(text)),
        PromptInvocation::Text(_) => Err(CodingSessionError::Input {
            message: "prompt turn requires non-empty text input".into(),
        }),
        PromptInvocation::Content(content) if !content.is_empty() => {
            let message_id = format!("user_{}", agent.messages().len());
            agent.add_message(AgentMessage::Custom {
                message_id,
                custom_type: "input".into(),
                content: content.clone(),
                display: true,
                details: None,
                timestamp: 0,
            });
            agent
                .run()
                .map_err(|message| CodingSessionError::Provider { message })
        }
        PromptInvocation::Content(_) => Err(CodingSessionError::Input {
            message: "prompt turn requires non-empty content input".into(),
        }),
        PromptInvocation::Compact { .. } => Err(CodingSessionError::UnsupportedCapability {
            capability: "manual compaction in PromptTurnFlow".into(),
        }),
        PromptInvocation::Skill {
            name,
            additional_instructions,
        } => agent
            .skill(name, additional_instructions.as_deref())
            .map_err(|message| CodingSessionError::Resource { message }),
        PromptInvocation::PromptTemplate { name, args } => agent
            .prompt_from_template(name, args)
            .map_err(|message| CodingSessionError::Resource { message }),
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{Deref, DerefMut};
    use std::sync::{Arc, Mutex};

    use pi_agent_core::api::agent::{Agent, AgentConfig, AgentResources};
    use pi_agent_core::api::tool::{AgentTool, AgentToolOutput};
    use pi_ai::api::client::AiClient;
    use pi_ai::api::conversation::{AssistantMessage, ContentBlock, Context, Message, StopReason};
    use pi_ai::api::model::{Model, ModelCost, ModelInput};
    use pi_ai::api::provider::ApiProvider;
    use pi_ai::api::stream::{AssistantMessageEvent, EventStream, StreamOptions};
    use pi_ai::api::testing::{FauxProvider, FauxResponse, FauxToolCall};
    use serde_json::json;
    use tokio::sync::oneshot;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::app::bootstrap::PromptInvocation;
    use crate::app::cli::prompt_options::{PromptRunOptions, assistant_text};
    use crate::operations::prompt::context::{PromptTurnIds, PromptTurnOptions};
    use crate::plugins::{
        HookDiagnostic, HookFailurePolicy, HookOutcome, HookProvider, HookRegistration,
        HookRegistrationHost, PluginError, PluginId, PluginMetadata, PluginRegistry, PluginSource,
        PromptHookContext, PromptHookPoint,
    };
    use crate::runtime::capability::OperationCapabilitySnapshot;
    use crate::services::plugin::PluginService;
    use crate::session::event::{PersistedContentBlock, SessionEventData, SessionEventEnvelope};
    use crate::session::replay::{MessageStatus, SessionReplay, TranscriptItem};
    use crate::session::repository::{CreateSessionOptions, SessionHandle, SessionLogStore};

    fn model(api: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn agent_with_provider(api: &str, provider: Arc<dyn ApiProvider>) -> Agent {
        let ai_client = Arc::new(AiClient::new());
        ai_client.register_provider(api, provider);
        let provider_streamer: pi_agent_core::api::agent::ProviderStreamer =
            Arc::new(move |model, context, options| {
                ai_client.stream_model(model, context, options)
            });
        let mut config = AgentConfig::new(model(api));
        config.provider_streamer = Some(provider_streamer);
        Agent::new(config)
    }

    fn context() -> PromptTurnContext {
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );
        context.set_capability_snapshot(OperationCapabilitySnapshot::permissive("op_1"));
        context
    }

    struct PromptTurnFixture {
        context: PromptTurnContext,
        _provider_guard: crate::test_support::ProviderGuard,
    }

    impl Deref for PromptTurnFixture {
        type Target = PromptTurnContext;

        fn deref(&self) -> &Self::Target {
            &self.context
        }
    }

    impl DerefMut for PromptTurnFixture {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.context
        }
    }

    fn context_with_agent(api: &str, response: &str) -> PromptTurnFixture {
        let provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text(response)),
        );
        let agent = agent_with_provider(api, Arc::new(FauxProvider::simple_text(response)));
        let mut context = context();
        context.set_agent(agent);
        PromptTurnFixture {
            context,
            _provider_guard: provider_guard,
        }
    }

    struct BlockingTwoTurnProvider {
        contexts: Arc<Mutex<Vec<Context>>>,
        first_started: Mutex<Option<oneshot::Sender<()>>>,
        release_first: Mutex<Option<oneshot::Receiver<()>>>,
    }

    impl BlockingTwoTurnProvider {
        fn new(
            contexts: Arc<Mutex<Vec<Context>>>,
            first_started: oneshot::Sender<()>,
            release_first: oneshot::Receiver<()>,
        ) -> Self {
            Self {
                contexts,
                first_started: Mutex::new(Some(first_started)),
                release_first: Mutex::new(Some(release_first)),
            }
        }
    }

    impl ApiProvider for BlockingTwoTurnProvider {
        fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            let call_index = {
                let mut contexts = self.contexts.lock().unwrap();
                contexts.push(ctx);
                contexts.len()
            };
            let first_release = if call_index == 1 {
                if let Some(started) = self.first_started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                self.release_first.lock().unwrap().take()
            } else {
                None
            };
            let model_id = model.id.clone();
            Box::pin(async_stream::stream! {
                if let Some(release) = first_release {
                    let _ = release.await;
                }
                let text = if call_index == 1 { "first" } else { "second" };
                let mut message = AssistantMessage::empty("blocking", &model_id);
                message.provider = Some("blocking".into());
                message.content.push(ContentBlock::Text {
                    text: text.into(),
                    text_signature: None,
                });
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message,
                };
            })
        }
    }

    fn context_with_runtime(api: &str, response: &str) -> PromptTurnFixture {
        context_with_runtime_invocation(api, response, PromptInvocation::Text("hello".into()))
    }

    fn context_with_runtime_invocation(
        api: &str,
        response: &str,
        invocation: PromptInvocation,
    ) -> PromptTurnFixture {
        let provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text(response)),
        );
        let options = PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: "hello".into(),
            model: model(api),
            api_key: None,
            auth_diagnostics: Vec::new(),
            system_prompt: None,
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(provider_guard.ai_client()),
            session: None,
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation,
        });
        let mut context = PromptTurnContext::new(PromptTurnIds::new("op_1", "turn_1"), options);
        context.set_capability_snapshot(OperationCapabilitySnapshot::permissive("op_1"));
        PromptTurnFixture {
            context,
            _provider_guard: provider_guard,
        }
    }

    fn setup_session_log() -> (tempfile::TempDir, SessionLogStore, SessionHandle) {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(CreateSessionOptions::new(
                "sess_prompt_flow",
                "2026-06-29T00:00:00Z",
            ))
            .unwrap();
        (temp, store, handle)
    }

    fn attach_session_boundary(
        context: &mut PromptTurnContext,
    ) -> (tempfile::TempDir, SessionLogStore, SessionHandle) {
        let (temp, store, handle) = setup_session_log();
        context.set_replay(SessionReplay {
            session_id: handle.manifest().session_id.clone(),
            committed_through_session_sequence: 0,
            cwd: None,
            active_leaf_id: None,
            leaves: Vec::new(),
            tree_labels: Default::default(),
            transcript: Vec::new(),
            diagnostics: Vec::new(),
            pending_delegation_confirmations: Vec::new(),
            pending_tool_authorizations: Vec::new(),
            usage: Default::default(),
            operation_statuses: Default::default(),
        });
        context.begin_transaction(&store, handle.clone()).unwrap();
        (temp, store, handle)
    }

    struct PromptFlowHookProvider {
        point: PromptHookPoint,
        policy: HookFailurePolicy,
        fail: bool,
    }

    impl HookProvider for PromptFlowHookProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("prompt-flow-hook-plugin"),
                "prompt-flow-hook-plugin",
                "0.1.0",
                PluginSource::FirstParty,
            )
        }

        fn hooks(
            &self,
            _host: &HookRegistrationHost,
        ) -> Result<Vec<HookRegistration>, PluginError> {
            Ok(vec![HookRegistration {
                point: self.point,
                policy: self.policy,
            }])
        }

        fn run_hook(&self, ctx: &PromptHookContext) -> Result<HookOutcome, PluginError> {
            assert_eq!(ctx.point, self.point);
            if self.fail {
                return Err(PluginError::Execution {
                    plugin_id: "prompt-flow-hook-plugin".into(),
                    message: "plugin hook failed".into(),
                });
            }
            Ok(HookOutcome {
                diagnostics: vec![HookDiagnostic {
                    message: format!("hook before agent turn for {}", ctx.operation_id),
                }],
            })
        }
    }

    fn plugin_service_with_hook(
        point: PromptHookPoint,
        policy: HookFailurePolicy,
    ) -> PluginService {
        plugin_service_with_hook_provider(PromptFlowHookProvider {
            point,
            policy,
            fail: false,
        })
    }

    fn plugin_service_with_failing_hook(
        point: PromptHookPoint,
        policy: HookFailurePolicy,
    ) -> PluginService {
        plugin_service_with_hook_provider(PromptFlowHookProvider {
            point,
            policy,
            fail: true,
        })
    }

    fn plugin_service_with_hook_provider(provider: PromptFlowHookProvider) -> PluginService {
        let mut registry = PluginRegistry::new();
        registry.register_hook_provider(Arc::new(provider));
        PluginService::with_registry(registry)
    }

    fn session_event_kinds(context: &PromptTurnContext) -> Vec<&'static str> {
        event_kinds(context.pending_session_events())
    }

    fn event_kinds(events: &[SessionEventEnvelope]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event.data {
                SessionEventData::OperationStarted { .. } => "operation.started",
                SessionEventData::OperationCommitted { .. } => "operation.committed",
                SessionEventData::OperationAborted { .. } => "operation.aborted",
                SessionEventData::OperationFailed { .. } => "operation.failed",
                SessionEventData::OperationRecovered { .. } => "operation.recovered",
                SessionEventData::TurnStarted {} => "turn.started",
                SessionEventData::TurnInputRecorded { .. } => "turn.input.recorded",
                SessionEventData::MessageStarted { .. } => "message.started",
                SessionEventData::MessageCompleted { .. } => "message.completed",
                SessionEventData::MessageCancelled { .. } => "message.cancelled",
                SessionEventData::ToolCallStarted { .. } => "tool.call.started",
                SessionEventData::ToolCallUpdated { .. } => "tool.call.updated",
                SessionEventData::ToolCallCompleted { .. } => "tool.call.completed",
                SessionEventData::ToolCallFailed { .. } => "tool.call.failed",
                SessionEventData::ToolCallCancelled { .. } => "tool.call.cancelled",
                SessionEventData::DiagnosticEmitted { .. } => "diagnostic.emitted",
                SessionEventData::MetadataUpdated { .. } => "metadata.updated",
                SessionEventData::ActiveLeafChanged { .. } => "active_leaf.changed",
                SessionEventData::SessionCreated { .. } => "session.created",
                SessionEventData::SessionCloned { .. } => "session.cloned",
                SessionEventData::SessionForked { .. } => "session.forked",
                SessionEventData::SessionCompactionStarted { .. } => "session.compaction.started",
                SessionEventData::SessionCompactionCompleted { .. } => {
                    "session.compaction.completed"
                }
                SessionEventData::BranchSummaryCreated { .. } => "branch.summary.created",
                SessionEventData::SessionTreeLabelUpdated { .. } => "session.tree_label.updated",
                SessionEventData::PluginLoadCompleted { .. } => "plugin.load.completed",
                SessionEventData::DelegationFoldedUpdated { .. } => "delegation.folded.updated",
                SessionEventData::SelfHealingEditStarted { .. } => "self_healing_edit.started",
                SessionEventData::SelfHealingEditRepairAttempted { .. } => {
                    "self_healing_edit.repair_attempted"
                }
                SessionEventData::SelfHealingEditCompleted { .. } => "self_healing_edit.completed",
                SessionEventData::DelegationConfirmationRequested { .. } => {
                    "delegation.confirmation.requested"
                }
                SessionEventData::DelegationConfirmationApproved { .. } => {
                    "delegation.confirmation.approved"
                }
                SessionEventData::DelegationConfirmationRejected { .. } => {
                    "delegation.confirmation.rejected"
                }
                SessionEventData::ToolAuthorizationRequested { .. } => {
                    "tool.authorization.requested"
                }
                SessionEventData::ToolAuthorizationResolved { .. } => "tool.authorization.resolved",
            })
            .collect()
    }

    fn run_agent_turn_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("run_agent_turn").unwrap();
        flow.add_node(
            "run_agent_turn",
            PromptTurnNode::new("RunAgentTurn", PromptTurnNodeKind::RunAgentTurn),
        )
        .unwrap();
        flow
    }

    fn prepare_input_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("prepare_input").unwrap();
        flow.add_node(
            "prepare_input",
            PromptTurnNode::new("PrepareInput", PromptTurnNodeKind::PrepareInput),
        )
        .unwrap();
        flow
    }

    fn resolve_request_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("resolve_request").unwrap();
        flow.add_node(
            "resolve_request",
            PromptTurnNode::new("ResolveRequest", PromptTurnNodeKind::ResolveRequest),
        )
        .unwrap();
        flow
    }

    fn record_user_input_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("record_user_input").unwrap();
        flow.add_node(
            "record_user_input",
            PromptTurnNode::new("RecordUserInput", PromptTurnNodeKind::RecordUserInput),
        )
        .unwrap();
        flow
    }

    fn resolve_runtime_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("resolve_runtime").unwrap();
        flow.add_node(
            "resolve_runtime",
            PromptTurnNode::new("ResolveRuntime", PromptTurnNodeKind::ResolveRuntime),
        )
        .unwrap();
        flow
    }

    fn resolve_and_load_resources_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("resolve_runtime").unwrap();
        flow.add_node(
            "resolve_runtime",
            PromptTurnNode::new("ResolveRuntime", PromptTurnNodeKind::ResolveRuntime),
        )
        .unwrap()
        .add_node(
            "load_resources",
            PromptTurnNode::new("LoadResources", PromptTurnNodeKind::LoadResources),
        )
        .unwrap()
        .edge("resolve_runtime", "load_resources")
        .unwrap();
        flow
    }

    fn load_resources_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("load_resources").unwrap();
        flow.add_node(
            "load_resources",
            PromptTurnNode::new("LoadResources", PromptTurnNodeKind::LoadResources),
        )
        .unwrap();
        flow
    }

    fn open_session_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("open_session").unwrap();
        flow.add_node(
            "open_session",
            PromptTurnNode::new("OpenSession", PromptTurnNodeKind::OpenSession),
        )
        .unwrap();
        flow
    }

    fn emit_completion_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("emit_completion").unwrap();
        flow.add_node(
            "emit_completion",
            PromptTurnNode::new("EmitCompletion", PromptTurnNodeKind::EmitCompletion),
        )
        .unwrap();
        flow
    }

    fn finalize_turn_only_flow() -> Flow<PromptTurnContext> {
        let mut flow = Flow::new("finalize_turn").unwrap();
        flow.add_node(
            "finalize_turn",
            PromptTurnNode::new("FinalizeTurn", PromptTurnNodeKind::FinalizeTurn),
        )
        .unwrap();
        flow
    }

    #[tokio::test]
    async fn prompt_turn_flow_runs_noncritical_prompt_hooks_as_diagnostics() {
        let api = "prompt-flow-plugin-hook";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "done");
        let _session = attach_session_boundary(&mut context);
        context.set_plugin_service(plugin_service_with_hook(
            PromptHookPoint::BeforeAgentTurn,
            HookFailurePolicy::FailOpen,
        ));

        flow.run(&mut context).await.unwrap();

        assert!(
            context
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("hook before agent turn"))
        );
    }

    #[tokio::test]
    async fn prompt_turn_flow_continues_for_fail_open_hook_error_as_diagnostic() {
        let api = "prompt-flow-plugin-open-hook";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "done");
        let _session = attach_session_boundary(&mut context);
        context.set_plugin_service(plugin_service_with_failing_hook(
            PromptHookPoint::BeforeAgentTurn,
            HookFailurePolicy::FailOpen,
        ));

        flow.run(&mut context).await.unwrap();

        assert_eq!(
            context.final_message().map(assistant_text),
            Some("done".into())
        );
        assert!(context.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("plugin hook failed")
                && diagnostic.code.as_deref() == Some("plugin_hook")
        }));
    }

    #[tokio::test]
    async fn prompt_turn_flow_aborts_for_fail_closed_hook_error() {
        let api = "prompt-flow-plugin-critical-hook";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "done");
        let _session = attach_session_boundary(&mut context);
        context.set_plugin_service(plugin_service_with_failing_hook(
            PromptHookPoint::BeforeAgentTurn,
            HookFailurePolicy::FailClosed,
        ));

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(error.to_string().contains("plugin hook"));
    }

    #[tokio::test]
    async fn prompt_turn_flow_consumes_parent_operation_cancellation() {
        let api = "prompt-flow-parent-cancellation";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "must not complete normally");
        let _session = attach_session_boundary(&mut context);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        context.set_operation_cancellation(cancellation);

        let _ = flow.run(&mut context).await;

        assert_eq!(context.abort_reason(), Some("parent operation ended"));
    }

    #[tokio::test]
    async fn prompt_turn_flow_runs_skeleton_in_expected_order() {
        let api = "prompt-flow-skeleton";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "done");
        let _session = attach_session_boundary(&mut context);

        let outcome = flow.run(&mut context).await.unwrap();

        assert_eq!(outcome.last_node.as_str(), "emit_completion");
        assert_eq!(outcome.last_action.as_str(), DEFAULT_ACTION);
        assert!(context.completion_recorded());
    }

    #[tokio::test]
    async fn prepare_input_node_records_normalized_text_input() {
        let api = "prompt-flow-prepare-input";
        let flow = prepare_input_only_flow();
        let mut context = context_with_runtime(api, "unused");
        context.resolve_request().unwrap();

        assert!(context.prepared_input().is_none());

        flow.run(&mut context).await.unwrap();

        let prepared = context.prepared_input().unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(
            serde_json::to_value(prepared).unwrap()[0]["data"]["text"],
            "hello"
        );
    }

    #[tokio::test]
    async fn resolve_request_node_marks_runtime_backed_prompt_resolved() {
        let api = "prompt-flow-resolve-request";
        let flow = resolve_request_only_flow();
        let mut context = context_with_runtime(api, "unused");

        assert!(!context.request_is_resolved());

        flow.run(&mut context).await.unwrap();
        flow.run(&mut context).await.unwrap();

        assert!(context.request_is_resolved());
    }

    #[tokio::test]
    async fn resolve_request_node_requires_runtime_snapshot() {
        let flow = resolve_request_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("runtime snapshot"));
        assert!(!context.request_is_resolved());
    }

    #[tokio::test]
    async fn resolve_request_node_rejects_empty_text_input() {
        let api = "prompt-flow-resolve-empty-text";
        let flow = resolve_request_only_flow();
        let mut context =
            context_with_runtime_invocation(api, "unused", PromptInvocation::Text(String::new()));

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("non-empty text input"));
        assert!(!context.request_is_resolved());
    }

    #[tokio::test]
    async fn resolve_request_node_rejects_empty_content_input() {
        let api = "prompt-flow-resolve-empty-content";
        let flow = resolve_request_only_flow();
        let mut context =
            context_with_runtime_invocation(api, "unused", PromptInvocation::Content(Vec::new()));

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("non-empty content input"));
        assert!(!context.request_is_resolved());
    }

    #[tokio::test]
    async fn resolve_request_node_rejects_manual_compaction() {
        let api = "prompt-flow-resolve-compact";
        let flow = resolve_request_only_flow();
        let mut context = context_with_runtime_invocation(
            api,
            "unused",
            PromptInvocation::Compact {
                custom_instructions: None,
            },
        );

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("manual compaction"));
        assert!(!context.request_is_resolved());
    }

    #[tokio::test]
    async fn prepare_input_node_requires_resolved_request() {
        let flow = prepare_input_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("before request is resolved"));
        assert!(context.prepared_input().is_none());
    }

    #[tokio::test]
    async fn resolve_runtime_node_attaches_options_runtime_snapshot() {
        let api = "prompt-flow-resolve-runtime";
        let flow = resolve_runtime_only_flow();
        let mut context = context_with_runtime(api, "unused");
        context.resolve_request().unwrap();

        assert!(context.runtime().is_none());

        flow.run(&mut context).await.unwrap();

        let runtime = context.runtime().unwrap();
        assert_eq!(runtime.model().api, api);
        assert_eq!(runtime.max_turns(), Some(2));
    }

    #[tokio::test]
    async fn resolve_runtime_node_requires_resolved_request() {
        let flow = resolve_runtime_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("before request is resolved"));
        assert!(context.runtime().is_none());
    }

    #[tokio::test]
    async fn load_resources_node_attaches_runtime_resources_snapshot() {
        let api = "prompt-flow-load-resources";
        let flow = resolve_and_load_resources_flow();
        let mut context = context_with_runtime(api, "unused");
        context.resolve_request().unwrap();

        assert!(context.loaded_resources().is_none());

        flow.run(&mut context).await.unwrap();

        assert!(
            context
                .loaded_resources()
                .is_some_and(|resources| resources.is_empty())
        );
    }

    #[tokio::test]
    async fn load_resources_node_requires_resolved_runtime() {
        let flow = load_resources_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("load resources"));
        assert!(context.loaded_resources().is_none());
    }

    #[tokio::test]
    async fn open_session_node_requires_owner_prepared_session_boundary() {
        let flow = open_session_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("session is opened"));
    }

    #[tokio::test]
    async fn emit_completion_node_records_completion_state_idempotently() {
        let flow = emit_completion_only_flow();
        let mut context = context();
        let mut message = AssistantMessage::empty("test", "test-model");
        message.content.push(ContentBlock::Text {
            text: "done".into(),
            text_signature: None,
        });
        context.record_final_message(message);

        flow.run(&mut context).await.unwrap();
        flow.run(&mut context).await.unwrap();

        assert!(context.completion_recorded());
        assert!(context.coding_events().is_empty());
    }

    #[tokio::test]
    async fn emit_completion_node_requires_final_message() {
        let flow = emit_completion_only_flow();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("emit completion"));
        assert!(context.coding_events().is_empty());
    }

    #[tokio::test]
    async fn finalize_turn_node_validates_readiness_without_flushing_session_events() {
        let flow = finalize_turn_only_flow();
        let mut context = context();
        let (_temp, store, handle) = attach_session_boundary(&mut context);
        let mut message = AssistantMessage::empty("test", "test-model");
        message.content.push(ContentBlock::Text {
            text: "done".into(),
            text_signature: None,
        });
        context.record_final_message(message);

        flow.run(&mut context).await.unwrap();

        assert_eq!(
            session_event_kinds(&context),
            vec!["operation.started", "turn.started"]
        );
        assert!(store.read_events(&handle).unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_agent_turn_applies_follow_up_control_while_provider_stream_is_running() {
        let api = "prompt-flow-follow-up-control";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let agent = agent_with_provider(
            api,
            Arc::new(BlockingTwoTurnProvider::new(
                contexts.clone(),
                started_tx,
                release_rx,
            )),
        );
        let (handle, receiver) = crate::runtime::control::prompt_control_channel();
        let flow = run_agent_turn_only_flow();
        let mut context = context();
        context.set_agent(agent);
        context.set_prompt_control_receiver(receiver);

        let mut run = Box::pin(flow.run(&mut context));
        tokio::select! {
            started = started_rx => started.unwrap(),
            result = &mut run => panic!("flow finished before provider blocked: {result:?}"),
        }
        handle.follow_up("continue with tests").unwrap();
        release_tx.send(()).unwrap();

        run.await.unwrap();

        assert_eq!(
            context.final_message().map(assistant_text),
            Some("second".to_string())
        );
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 2);
        assert!(
            contexts[1].messages.iter().any(|message| matches!(
                message,
                Message::User { content }
                    if content.iter().any(|block| matches!(
                        block,
                        ContentBlock::Text { text, .. } if text == "continue with tests"
                    ))
            )),
            "{:#?}",
            contexts[1].messages
        );
    }

    #[tokio::test]
    async fn run_agent_turn_records_final_message_and_coding_events() {
        let api = "prompt-flow-run-agent-turn";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "flow answer");
        let _session = attach_session_boundary(&mut context);

        flow.run(&mut context).await.unwrap();

        assert_eq!(
            context.final_message().map(assistant_text),
            Some("flow answer".to_string())
        );
        assert!(context.coding_events().iter().any(|event| matches!(
            event,
            crate::events::prompt_stream::PromptStreamEvent::Message(
                crate::events::message::MessageEvent::Delta { text, .. }
            ) if text == "flow answer"
        )));
        assert!(context.coding_events().iter().any(|event| matches!(
            event,
            crate::events::prompt_stream::PromptStreamEvent::Message(
                crate::events::message::MessageEvent::Completed { final_text, .. }
            ) if final_text == "flow answer"
        )));
    }

    #[tokio::test]
    async fn run_agent_turn_records_assistant_session_events_without_flushing() {
        let api = "prompt-flow-session-events";
        let flow = run_agent_turn_only_flow();
        let mut context = context_with_agent(api, "flow answer");
        let (_temp, store, handle) = setup_session_log();
        context.begin_transaction(&store, handle.clone()).unwrap();

        flow.run(&mut context).await.unwrap();

        assert_eq!(
            session_event_kinds(&context),
            vec![
                "operation.started",
                "turn.started",
                "message.started",
                "message.completed",
            ]
        );
        assert!(
            context
                .pending_session_events()
                .iter()
                .any(|event| matches!(
                    &event.data,
                    SessionEventData::MessageCompleted { content, .. }
                        if content == &vec![PersistedContentBlock::Text {
                            text: "flow answer".into(),
                        }]
                ))
        );
        assert!(matches!(
            context.pending_session_events().last().map(|event| &event.data),
            Some(SessionEventData::MessageCompleted {
                finish_reason: Some(reason),
                ..
            }) if reason == "stop"
        ));
        assert!(store.read_events(&handle).unwrap().is_empty());
    }

    #[tokio::test]
    async fn prompt_turn_flow_records_user_input_before_assistant_events() {
        let api = "prompt-flow-user-input";
        let flow = PromptTurnFlow::new().unwrap();
        let mut context = context_with_runtime(api, "flow answer");
        let (_temp, store, handle) = attach_session_boundary(&mut context);

        flow.run(&mut context).await.unwrap();

        let durable_events = store.read_events(&handle).unwrap();
        let mut kinds = event_kinds(&durable_events);
        kinds.extend(session_event_kinds(&context));
        assert_eq!(
            kinds,
            vec![
                "operation.started",
                "turn.started",
                "turn.input.recorded",
                "message.started",
                "message.completed",
            ]
        );
        assert!(durable_events.iter().any(|event| matches!(
            &event.data,
            SessionEventData::TurnInputRecorded { content }
                if serde_json::to_value(content).unwrap()[0]["data"]["text"] == "hello"
        )));
    }

    #[tokio::test]
    async fn record_user_input_requires_prepared_input() {
        let flow = record_user_input_only_flow();
        let mut context = context();
        let (_temp, store, handle) = setup_session_log();
        context.begin_transaction(&store, handle.clone()).unwrap();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("input has not been prepared"));
        assert_eq!(
            session_event_kinds(&context),
            vec!["operation.started", "turn.started"]
        );
        assert!(store.read_events(&handle).unwrap().is_empty());
    }

    #[tokio::test]
    async fn build_agent_runtime_node_hydrates_replayed_messages() {
        let api = "prompt-flow-build-agent-runtime";
        let mut flow = Flow::new("resolve_runtime").unwrap();
        flow.add_node(
            "resolve_runtime",
            PromptTurnNode::new("ResolveRuntime", PromptTurnNodeKind::ResolveRuntime),
        )
        .unwrap()
        .add_node(
            "load_resources",
            PromptTurnNode::new("LoadResources", PromptTurnNodeKind::LoadResources),
        )
        .unwrap()
        .add_node(
            "build_agent_runtime",
            PromptTurnNode::new("BuildAgentRuntime", PromptTurnNodeKind::BuildAgentRuntime),
        )
        .unwrap()
        .edge("resolve_runtime", "load_resources")
        .unwrap()
        .edge("load_resources", "build_agent_runtime")
        .unwrap();
        let mut context = context_with_runtime(api, "unused");
        context.resolve_request().unwrap();
        context.set_replay(SessionReplay {
            session_id: "sess_replay".into(),
            committed_through_session_sequence: 0,
            cwd: None,
            active_leaf_id: None,
            leaves: Vec::new(),
            tree_labels: Default::default(),
            transcript: vec![
                TranscriptItem::UserInput {
                    turn_id: "turn_old".into(),
                    text: "previous".into(),
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_old".into(),
                    content: vec![PersistedContentBlock::Text {
                        text: "answer".into(),
                    }],
                    status: MessageStatus::Completed,
                },
            ],
            diagnostics: Vec::new(),
            pending_delegation_confirmations: Vec::new(),
            pending_tool_authorizations: Vec::new(),
            usage: Default::default(),
            operation_statuses: Default::default(),
        });

        flow.run(&mut context).await.unwrap();

        let messages = context.agent().unwrap().messages();
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            &messages[0],
            AgentMessage::UserText { text, .. } if text == "previous"
        ));
        assert!(matches!(
            &messages[1],
            AgentMessage::Assistant { message, .. }
                if message.content == vec![ContentBlock::Text {
                    text: "answer".into(),
                    text_signature: None,
                }]
        ));
    }

    #[tokio::test]
    async fn build_agent_runtime_node_requires_loaded_resources() {
        let api = "prompt-flow-build-agent-runtime-without-resources";
        let mut flow = Flow::new("resolve_runtime").unwrap();
        flow.add_node(
            "resolve_runtime",
            PromptTurnNode::new("ResolveRuntime", PromptTurnNodeKind::ResolveRuntime),
        )
        .unwrap()
        .add_node(
            "build_agent_runtime",
            PromptTurnNode::new("BuildAgentRuntime", PromptTurnNodeKind::BuildAgentRuntime),
        )
        .unwrap()
        .edge("resolve_runtime", "build_agent_runtime")
        .unwrap();
        let mut context = context_with_runtime(api, "unused");
        context.resolve_request().unwrap();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("resources are loaded"));
        assert!(context.agent().is_none());
    }

    #[tokio::test]
    async fn run_agent_turn_records_tool_lifecycle_session_events() {
        let api = "prompt-flow-session-tool-events";
        let agent = agent_with_provider(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: Vec::new(),
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "toolu_1".into(),
                            name: "echo".into(),
                            deltas: Vec::new(),
                            final_arguments: json!({"text": "hi"}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("done", StopReason::Stop),
            ])),
        );
        agent.add_tool(AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: json!({"type": "object"}),
            execution_mode: None,
            execute: Arc::new(|_context, args, _on_update| {
                let text = args
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let output = AgentToolOutput::new(vec![ContentBlock::Text {
                    text: format!("echo: {text}"),
                    text_signature: None,
                }]);
                Box::pin(async move { Ok(output) })
            }),
        });
        let flow = run_agent_turn_only_flow();
        let mut context = context();
        context.set_agent(agent);
        let (_temp, store, handle) = setup_session_log();
        context.begin_transaction(&store, handle.clone()).unwrap();

        flow.run(&mut context).await.unwrap();

        let kinds = session_event_kinds(&context);
        assert!(kinds.contains(&"tool.call.started"));
        assert!(kinds.contains(&"tool.call.completed"));
        assert!(
            context
                .pending_session_events()
                .iter()
                .any(|event| matches!(
                    &event.data,
                    SessionEventData::ToolCallStarted { name, arguments, .. }
                        if name == "echo" && arguments == &json!({"text": "hi"})
                ))
        );
        assert!(
            context
                .pending_session_events()
                .iter()
                .any(|event| matches!(
                    &event.data,
                    SessionEventData::ToolCallCompleted { result, .. }
                        if serde_json::to_value(result).unwrap()["data"]["text"] == "echo: hi"
                ))
        );
        assert!(store.read_events(&handle).unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_agent_turn_requires_agent_runtime() {
        let mut flow = Flow::new("run_agent_turn").unwrap();
        flow.add_node(
            "run_agent_turn",
            PromptTurnNode::new("RunAgentTurn", PromptTurnNodeKind::RunAgentTurn),
        )
        .unwrap();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::NodeFailed { .. }));
        assert!(error.to_string().contains("no agent runtime"));
    }

    #[tokio::test]
    async fn misconfigured_prompt_turn_flow_reports_missing_transition() {
        let mut flow = Flow::new("start_prompt_turn").unwrap();
        flow.add_node(
            "start_prompt_turn",
            PromptTurnNode::new("StartPromptTurn", PromptTurnNodeKind::Default),
        )
        .unwrap()
        .add_node(
            "resolve_request",
            PromptTurnNode::new("ResolveRequest", PromptTurnNodeKind::Default),
        )
        .unwrap()
        .edge("start_prompt_turn", "resolve_request")
        .unwrap()
        .edge_on(
            "resolve_request",
            Action::new("not_default").unwrap(),
            "start_prompt_turn",
        )
        .unwrap();
        let mut context = context();

        let error = flow.run(&mut context).await.unwrap_err();

        assert!(matches!(error, FlowError::MissingTransition { .. }));
        assert!(
            error
                .to_string()
                .contains("missing transition from node 'resolve_request'")
        );
    }
}
