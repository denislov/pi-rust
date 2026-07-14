#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use pi_agent_core::{
    Agent, AgentEvent, AgentResources, AgentTool, AgentToolResult, ThinkingLevel, ToolExecutionMode,
};
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Model, ProviderAuthDiagnostic,
};

use crate::args::CliMode;
use crate::config::Settings;
use crate::plugins::{PromptHookContext, PromptHookPoint};
use crate::prompt_options::{PromptRunOptions, assistant_text};
use crate::request::ResolvedPromptRequest;
use crate::runtime::{PromptInvocation, SessionRunOptions};
use crate::session::ResolvedSessionTarget;

use super::CodingSessionError;
use super::capability_snapshot::OperationCapabilitySnapshot;
use super::delegation::{
    DelegationAuthorizationDecision, DelegationLineageEntry,
    authorize_delegation_requests_with_lineage,
};
use super::event::CodingAgentEvent;
use super::event_service::{AgentEventMappingContext, EventService, map_agent_event};
use super::operation_control::PromptControlReceiver;
use super::plugin_service::PluginService;
use super::profiles::{AgentProfile, DelegationPolicy, ProfileId, ProfileKind};
use super::session_log::event::{
    DiagnosticLevel, OperationKind, PersistedContentBlock, PersistedDelegationStatus,
    PersistedToolResult, SessionEventEnvelope,
};
use super::session_log::id::{SystemClock, SystemIdGenerator};
use super::session_log::replay::{MessageStatus, SessionReplay, TranscriptItem};
use super::session_log::store::{SessionHandle, SessionLogStore};
use super::session_log::transaction::TurnTransaction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTurnMode {
    Print,
    Json,
    Rpc,
}

impl From<CliMode> for PromptTurnMode {
    fn from(mode: CliMode) -> Self {
        match mode {
            CliMode::Print => Self::Print,
            CliMode::Json => Self::Json,
            CliMode::Rpc => Self::Rpc,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PromptTurnOptions {
    invocation: PromptInvocation,
    mode: PromptTurnMode,
    session_target: Option<ResolvedSessionTarget>,
    session_name: Option<String>,
    runtime: Option<RuntimeSnapshot>,
}

impl PromptTurnOptions {
    pub fn new(invocation: PromptInvocation) -> Self {
        Self {
            invocation,
            mode: PromptTurnMode::Print,
            session_target: None,
            session_name: None,
            runtime: None,
        }
    }

    pub fn with_mode(mut self, mode: PromptTurnMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_session_target(mut self, target: ResolvedSessionTarget) -> Self {
        self.session_target = Some(target);
        self
    }

    pub fn with_session_name(mut self, name: impl Into<String>) -> Self {
        self.session_name = Some(name.into());
        self
    }

    pub fn invocation(&self) -> &PromptInvocation {
        &self.invocation
    }

    pub fn mode(&self) -> PromptTurnMode {
        self.mode
    }

    pub fn session_target(&self) -> Option<&ResolvedSessionTarget> {
        self.session_target.as_ref()
    }

    pub fn session_name(&self) -> Option<&str> {
        self.session_name.as_deref()
    }

    pub fn from_prompt_run_options(options: PromptRunOptions) -> Self {
        let invocation = options.invocation.clone();
        let session_target = options.session_target.clone();
        let session_name = options.session_name.clone();
        let runtime = RuntimeSnapshot::from_prompt_run_options(options);
        Self {
            invocation,
            mode: PromptTurnMode::Print,
            session_target,
            session_name,
            runtime: Some(runtime),
        }
    }

    pub(crate) fn runtime(&self) -> Option<&RuntimeSnapshot> {
        self.runtime.as_ref()
    }

    pub(crate) fn set_invocation(&mut self, invocation: PromptInvocation) {
        self.invocation = invocation;
    }

    pub(crate) fn apply_agent_profile(
        &mut self,
        profile: &AgentProfile,
        diagnostics: Vec<CodingDiagnostic>,
    ) -> Result<(), CodingSessionError> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            })?;
        runtime.apply_agent_profile(profile, diagnostics);
        Ok(())
    }

    pub(crate) fn apply_delegated_agent_profile(
        &mut self,
        profile: &AgentProfile,
        diagnostics: Vec<CodingDiagnostic>,
    ) -> Result<(), CodingSessionError> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            })?;
        runtime.apply_delegated_agent_profile(profile, diagnostics);
        Ok(())
    }
}

impl From<&ResolvedPromptRequest> for PromptTurnOptions {
    fn from(request: &ResolvedPromptRequest) -> Self {
        Self {
            invocation: request.invocation.clone(),
            mode: request.context.parsed.mode.into(),
            session_target: request.context.session_target.clone(),
            session_name: request.context.session_name.clone(),
            runtime: None,
        }
    }
}

impl From<ResolvedPromptRequest> for PromptTurnOptions {
    fn from(request: ResolvedPromptRequest) -> Self {
        let mut options = PromptTurnOptions::from(&request);
        options.runtime = Some(RuntimeSnapshot::from_prompt_run_options(
            request.session_options,
        ));
        options
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodingDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingDiagnostic {
    pub severity: CodingDiagnosticSeverity,
    pub message: String,
    pub source: Option<std::path::PathBuf>,
    pub code: Option<String>,
}

impl CodingDiagnostic {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            severity: CodingDiagnosticSeverity::Info,
            message: message.into(),
            source: None,
            code: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: CodingDiagnosticSeverity::Warning,
            message: message.into(),
            source: None,
            code: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: CodingDiagnosticSeverity::Error,
            message: message.into(),
            source: None,
            code: None,
        }
    }

    pub fn with_source(mut self, source: impl AsRef<Path>) -> Self {
        self.source = Some(source.as_ref().to_path_buf());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromptTurnOutcome {
    Success {
        operation_id: String,
        turn_id: String,
        session_id: Option<String>,
        leaf_id: Option<String>,
        final_text: String,
        final_message: AssistantMessage,
        diagnostics: Vec<CodingDiagnostic>,
    },
    Aborted {
        operation_id: String,
        turn_id: Option<String>,
        reason: String,
        session_id: Option<String>,
    },
    Failed {
        operation_id: String,
        turn_id: Option<String>,
        error: CodingSessionError,
        diagnostics: Vec<CodingDiagnostic>,
    },
}

impl PromptTurnOutcome {
    pub(crate) fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub(crate) fn apply_success_session_write_metadata(
        &mut self,
        session_id: Option<String>,
        leaf_id: Option<String>,
    ) {
        let Self::Success {
            session_id: outcome_session_id,
            leaf_id: outcome_leaf_id,
            ..
        } = self
        else {
            return;
        };
        if let Some(session_id) = session_id {
            *outcome_session_id = Some(session_id);
        }
        *outcome_leaf_id = leaf_id;
    }
}

pub(crate) type PromptTurnTransaction = TurnTransaction<SystemIdGenerator, SystemClock>;

#[derive(Debug, Clone)]
pub(crate) struct AgentRunObservation {
    event: AgentEvent,
    coding_events: Vec<CodingAgentEvent>,
}

impl AgentRunObservation {
    pub(crate) fn event(&self) -> &AgentEvent {
        &self.event
    }

    pub(crate) fn coding_events(&self) -> &[CodingAgentEvent] {
        &self.coding_events
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DelegationRequest {
    pub(crate) operation_id: String,
    pub(crate) turn_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) requesting_profile_id: ProfileId,
    pub(crate) target_kind: ProfileKind,
    pub(crate) target_id: ProfileId,
    pub(crate) task: String,
}

#[derive(Clone)]
pub(crate) struct RuntimeSnapshot {
    model: Model,
    api_key: Option<String>,
    auth_diagnostics: Vec<ProviderAuthDiagnostic>,
    system_prompt: Option<String>,
    max_turns: Option<u32>,
    tools: Vec<AgentTool>,
    register_builtins: bool,
    resources: AgentResources,
    settings: Option<Settings>,
    thinking_level: Option<ThinkingLevel>,
    tool_execution: Option<ToolExecutionMode>,
    session_run_options: Option<SessionRunOptions>,
    profile_id: Option<ProfileId>,
    profile_delegation_policy: Option<DelegationPolicy>,
    profile_tool_allowlist: Option<Vec<String>>,
    profile_skill_allowlist: Option<Vec<String>>,
    profile_diagnostics: Vec<CodingDiagnostic>,
}

impl std::fmt::Debug for RuntimeSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeSnapshot")
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("auth_diagnostics", &self.auth_diagnostics)
            .field("system_prompt", &self.system_prompt)
            .field("max_turns", &self.max_turns)
            .field("tools_len", &self.tools.len())
            .field("register_builtins", &self.register_builtins)
            .field("resources", &self.resources)
            .field("settings", &self.settings)
            .field("thinking_level", &self.thinking_level)
            .field("tool_execution", &self.tool_execution)
            .field("session_run_options", &self.session_run_options)
            .field("profile_id", &self.profile_id)
            .field("profile_delegation_policy", &self.profile_delegation_policy)
            .field("profile_tool_allowlist", &self.profile_tool_allowlist)
            .field("profile_skill_allowlist", &self.profile_skill_allowlist)
            .field("profile_diagnostics", &self.profile_diagnostics)
            .finish()
    }
}

impl RuntimeSnapshot {
    pub(crate) fn from_prompt_run_options(options: PromptRunOptions) -> Self {
        let PromptRunOptions {
            prompt: _,
            model,
            api_key,
            auth_diagnostics,
            system_prompt,
            max_turns,
            tools,
            register_builtins,
            session,
            session_target: _,
            session_name: _,
            thinking_level,
            tool_execution,
            resources,
            settings,
            invocation: _,
        } = options;

        Self {
            model,
            api_key,
            auth_diagnostics,
            system_prompt,
            max_turns,
            tools,
            register_builtins,
            resources,
            settings,
            thinking_level,
            tool_execution,
            session_run_options: session,
            profile_id: None,
            profile_delegation_policy: None,
            profile_tool_allowlist: None,
            profile_skill_allowlist: None,
            profile_diagnostics: Vec::new(),
        }
    }

    pub(crate) fn apply_agent_profile(
        &mut self,
        profile: &AgentProfile,
        diagnostics: Vec<CodingDiagnostic>,
    ) {
        self.apply_agent_profile_core(profile, diagnostics);
        self.profile_tool_allowlist = (!profile.tools.is_empty()).then(|| profile.tools.clone());
        self.profile_skill_allowlist = (!profile.skills.is_empty()).then(|| profile.skills.clone());
    }

    pub(crate) fn apply_delegated_agent_profile(
        &mut self,
        profile: &AgentProfile,
        diagnostics: Vec<CodingDiagnostic>,
    ) {
        self.apply_agent_profile_core(profile, diagnostics);
        self.profile_tool_allowlist = Some(profile.tools.clone());
        self.profile_skill_allowlist = Some(profile.skills.clone());
    }

    fn apply_agent_profile_core(
        &mut self,
        profile: &AgentProfile,
        mut diagnostics: Vec<CodingDiagnostic>,
    ) {
        self.profile_diagnostics.append(&mut diagnostics);
        if let Some(model_id) = profile.model.as_deref() {
            match pi_ai::lookup_model(model_id) {
                Some(model) => self.model = model,
                None => self
                    .profile_diagnostics
                    .push(CodingDiagnostic::warning(format!(
                        "agent profile {} requested unavailable model: {model_id}",
                        profile.id
                    ))),
            }
        }
        if let Some(system_prompt) = profile.system_prompt.as_ref() {
            self.system_prompt = Some(system_prompt.clone());
        }
        self.profile_id = Some(profile.id.clone());
        self.profile_delegation_policy = Some(profile.delegation.clone());
    }

    pub(crate) fn model(&self) -> &Model {
        &self.model
    }

    pub(crate) fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub(crate) fn auth_diagnostics(&self) -> &[ProviderAuthDiagnostic] {
        &self.auth_diagnostics
    }

    pub(crate) fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub(crate) fn max_turns(&self) -> Option<u32> {
        self.max_turns
    }

    pub(crate) fn tools(&self) -> &[AgentTool] {
        &self.tools
    }

    pub(crate) fn register_builtins(&self) -> bool {
        self.register_builtins
    }

    pub(crate) fn resources(&self) -> &AgentResources {
        &self.resources
    }

    pub(crate) fn settings(&self) -> Option<&Settings> {
        self.settings.as_ref()
    }

    pub(crate) fn thinking_level(&self) -> Option<ThinkingLevel> {
        self.thinking_level
    }

    pub(crate) fn tool_execution(&self) -> Option<ToolExecutionMode> {
        self.tool_execution
    }

    pub(crate) fn session_run_options(&self) -> Option<&SessionRunOptions> {
        self.session_run_options.as_ref()
    }

    pub(crate) fn profile_id(&self) -> Option<&ProfileId> {
        self.profile_id.as_ref()
    }

    pub(crate) fn profile_delegation_policy(&self) -> Option<&DelegationPolicy> {
        self.profile_delegation_policy.as_ref()
    }

    pub(crate) fn profile_tool_allowlist(&self) -> Option<&[String]> {
        self.profile_tool_allowlist.as_deref()
    }

    pub(crate) fn profile_skill_allowlist(&self) -> Option<&[String]> {
        self.profile_skill_allowlist.as_deref()
    }

    pub(crate) fn profile_diagnostics(&self) -> &[CodingDiagnostic] {
        &self.profile_diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptTurnIds {
    operation_id: String,
    turn_id: String,
}

impl PromptTurnIds {
    pub(crate) fn new(operation_id: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
        }
    }
}

pub(crate) struct PromptTurnContext {
    ids: PromptTurnIds,
    options: PromptTurnOptions,
    request_resolved: bool,
    runtime: Option<RuntimeSnapshot>,
    prepared_input: Option<Vec<PersistedContentBlock>>,
    loaded_resources: Option<AgentResources>,
    replay: Option<SessionReplay>,
    session_id: Option<String>,
    non_persistent_runtime_id: Option<String>,
    agent: Option<Agent>,
    transaction: Option<PromptTurnTransaction>,
    final_message: Option<AssistantMessage>,
    agent_observations: Vec<AgentRunObservation>,
    coding_events: Vec<CodingAgentEvent>,
    delegation_requests: Vec<DelegationRequest>,
    delegation_authorization_decisions: Vec<DelegationAuthorizationDecision>,
    assistant_session_message_id: Option<String>,
    completed_assistant_session_message_id: Option<String>,
    live_event_service: Option<EventService>,
    prompt_control_receiver: Option<PromptControlReceiver>,
    plugin_service: PluginService,
    tool_session_call_ids: HashMap<String, String>,
    diagnostics: Vec<CodingDiagnostic>,
    requested_abort_reason: Option<String>,
    capability_snapshot: Option<OperationCapabilitySnapshot>,
}

impl PromptTurnContext {
    pub(crate) fn new(ids: PromptTurnIds, options: PromptTurnOptions) -> Self {
        Self {
            ids,
            options,
            request_resolved: false,
            runtime: None,
            prepared_input: None,
            loaded_resources: None,
            replay: None,
            session_id: None,
            non_persistent_runtime_id: None,
            agent: None,
            transaction: None,
            final_message: None,
            agent_observations: Vec::new(),
            coding_events: Vec::new(),
            delegation_requests: Vec::new(),
            delegation_authorization_decisions: Vec::new(),
            assistant_session_message_id: None,
            completed_assistant_session_message_id: None,
            live_event_service: None,
            prompt_control_receiver: None,
            plugin_service: PluginService::new(),
            tool_session_call_ids: HashMap::new(),
            diagnostics: Vec::new(),
            requested_abort_reason: None,
            capability_snapshot: None,
        }
    }

    pub(crate) fn from_resolved_request(
        ids: PromptTurnIds,
        request: ResolvedPromptRequest,
    ) -> Result<Self, CodingSessionError> {
        let options = PromptTurnOptions::from(request);
        Ok(Self::new(ids, options))
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.ids.operation_id
    }

    pub(crate) fn turn_id(&self) -> &str {
        &self.ids.turn_id
    }

    pub(crate) fn options(&self) -> &PromptTurnOptions {
        &self.options
    }

    pub(crate) fn set_plugin_service(&mut self, plugin_service: PluginService) {
        self.plugin_service = plugin_service;
    }

    pub(crate) fn plugin_service(&self) -> &PluginService {
        &self.plugin_service
    }

    pub(crate) fn set_capability_snapshot(&mut self, snapshot: OperationCapabilitySnapshot) {
        self.capability_snapshot = Some(snapshot);
    }

    pub(crate) fn capability_snapshot(&self) -> Option<&OperationCapabilitySnapshot> {
        self.capability_snapshot.as_ref()
    }

    pub(crate) fn run_prompt_hook(
        &mut self,
        point: PromptHookPoint,
    ) -> Result<(), CodingSessionError> {
        let hook_context = PromptHookContext {
            operation_id: self.operation_id().to_owned(),
            turn_id: self.turn_id().to_owned(),
            session_id: self.session_id.clone(),
            point,
        };
        for diagnostic in self.plugin_service.run_prompt_hook(point, hook_context)? {
            self.record_diagnostic(diagnostic);
        }
        Ok(())
    }

    pub(crate) fn set_runtime(&mut self, runtime: RuntimeSnapshot) {
        self.runtime = Some(runtime);
    }

    pub(crate) fn resolve_request(&mut self) -> Result<(), CodingSessionError> {
        if self.request_resolved {
            return Ok(());
        }
        match self.options.invocation() {
            PromptInvocation::Text(text) if text.is_empty() => {
                return Err(CodingSessionError::Input {
                    message: "prompt turn requires non-empty text input".into(),
                });
            }
            PromptInvocation::Content(content) if content.is_empty() => {
                return Err(CodingSessionError::Input {
                    message: "prompt turn requires non-empty content input".into(),
                });
            }
            PromptInvocation::Compact { .. } => {
                return Err(CodingSessionError::UnsupportedCapability {
                    capability: "manual compaction in PromptTurnFlow".into(),
                });
            }
            PromptInvocation::Text(_)
            | PromptInvocation::Content(_)
            | PromptInvocation::Skill { .. }
            | PromptInvocation::PromptTemplate { .. } => {}
        }
        if self.options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        self.request_resolved = true;
        Ok(())
    }

    pub(crate) fn request_is_resolved(&self) -> bool {
        self.request_resolved
    }

    pub(crate) fn resolve_runtime_from_options(&mut self) -> Result<(), CodingSessionError> {
        if self.runtime.is_some() {
            return Ok(());
        }
        self.require_resolved_request("resolve runtime")?;
        let runtime =
            self.options
                .runtime()
                .cloned()
                .ok_or_else(|| CodingSessionError::Config {
                    message: "prompt turn options do not include a runtime snapshot".into(),
                })?;
        self.set_runtime(runtime);
        Ok(())
    }

    pub(crate) fn runtime(&self) -> Option<&RuntimeSnapshot> {
        self.runtime.as_ref()
    }

    pub(crate) fn prepare_input(&mut self) -> Result<(), CodingSessionError> {
        if self.prepared_input.is_some() {
            return Ok(());
        }
        self.require_resolved_request("prepare input")?;
        self.prepared_input = Some(persisted_content_blocks_from_invocation(
            self.options.invocation(),
        )?);
        Ok(())
    }

    pub(crate) fn prepared_input(&self) -> Option<&[PersistedContentBlock]> {
        self.prepared_input.as_deref()
    }

    pub(crate) fn load_resources_from_runtime(&mut self) -> Result<(), CodingSessionError> {
        if self.loaded_resources.is_some() {
            return Ok(());
        }
        let resources = self
            .runtime
            .as_ref()
            .ok_or_else(|| CodingSessionError::Config {
                message: "prompt turn cannot load resources without a runtime snapshot".into(),
            })?
            .resources()
            .clone();
        self.loaded_resources = Some(resources);
        Ok(())
    }

    pub(crate) fn loaded_resources(&self) -> Option<&AgentResources> {
        self.loaded_resources.as_ref()
    }

    pub(crate) fn set_replay(&mut self, replay: SessionReplay) {
        self.replay = Some(replay);
    }

    pub(crate) fn replay(&self) -> Option<&SessionReplay> {
        self.replay.as_ref()
    }

    pub(crate) fn set_non_persistent_session(
        &mut self,
        runtime_id: impl Into<String>,
        transcript: Vec<TranscriptItem>,
    ) {
        let runtime_id = runtime_id.into();
        self.non_persistent_runtime_id = Some(runtime_id.clone());
        self.session_id = None;
        self.transaction = None;
        self.replay = Some(SessionReplay {
            session_id: runtime_id,
            cwd: None,
            active_leaf_id: None,
            leaves: Vec::new(),
            transcript,
            diagnostics: Vec::new(),
            pending_delegation_confirmations: Vec::new(),
            usage: Default::default(),
            operation_statuses: Default::default(),
        });
    }

    pub(crate) fn non_persistent_runtime_id(&self) -> Option<&str> {
        self.non_persistent_runtime_id.as_deref()
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(crate) fn set_session_id(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
        self.non_persistent_runtime_id = None;
    }

    pub(crate) fn set_agent(&mut self, agent: Agent) {
        self.agent = Some(agent);
    }

    pub(crate) fn agent(&self) -> Option<&Agent> {
        self.agent.as_ref()
    }

    pub(crate) fn begin_transaction(
        &mut self,
        store: &SessionLogStore,
        handle: SessionHandle,
    ) -> Result<(), CodingSessionError> {
        if self.transaction.is_some() {
            return Err(CodingSessionError::Session {
                message: "prompt turn already has an active transaction".into(),
            });
        }
        self.session_id = Some(handle.manifest().session_id.clone());
        self.transaction = Some(TurnTransaction::begin(
            store,
            handle,
            SystemIdGenerator,
            SystemClock,
            OperationKind::Prompt,
        ));
        Ok(())
    }

    pub(crate) fn set_transaction(&mut self, transaction: PromptTurnTransaction) {
        self.transaction = Some(transaction);
    }

    pub(crate) fn transaction_mut(&mut self) -> Option<&mut PromptTurnTransaction> {
        self.transaction.as_mut()
    }

    pub(crate) fn has_active_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    pub(crate) fn take_transaction(&mut self) -> Option<PromptTurnTransaction> {
        self.transaction.take()
    }

    pub(crate) fn pending_session_events(&self) -> &[SessionEventEnvelope] {
        self.transaction
            .as_ref()
            .map(TurnTransaction::pending_events)
            .unwrap_or_default()
    }

    pub(crate) fn enable_live_events(&mut self, event_service: EventService) {
        self.live_event_service = Some(event_service);
    }

    pub(crate) fn live_events_enabled(&self) -> bool {
        self.live_event_service.is_some()
    }

    pub(crate) fn set_prompt_control_receiver(&mut self, receiver: PromptControlReceiver) {
        self.prompt_control_receiver = Some(receiver);
    }

    pub(crate) fn take_prompt_control_receiver(&mut self) -> Option<PromptControlReceiver> {
        self.prompt_control_receiver.take()
    }

    pub(crate) fn completed_transcript_items(&self) -> Vec<TranscriptItem> {
        let mut transcript = Vec::new();

        if let Some(input) = self.prepared_input.as_deref() {
            let text = persisted_content_blocks_text(input);
            if !text.is_empty() {
                transcript.push(TranscriptItem::UserInput {
                    turn_id: self.turn_id().to_owned(),
                    text,
                });
            }
        }

        if let Some(message) = self.final_message.as_ref() {
            let content = persisted_assistant_content_blocks(&message.content);
            if !content.is_empty() {
                transcript.push(TranscriptItem::AssistantMessage {
                    message_id: self
                        .assistant_session_message_id
                        .clone()
                        .unwrap_or_else(|| format!("msg_{}", self.turn_id())),
                    content,
                    status: MessageStatus::Completed,
                });
            }
        }

        transcript
    }

    pub(crate) fn record_user_input(&mut self) -> Result<(), CodingSessionError> {
        let content = self
            .prepared_input
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "prompt turn input has not been prepared".into(),
            })?;
        if let Some(transaction) = self.transaction.as_mut() {
            transaction.record_user_input(content)?;
        }
        Ok(())
    }

    pub(crate) fn record_diagnostic(&mut self, diagnostic: CodingDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn record_delegation_folded_update(
        &mut self,
        request: &DelegationRequest,
        status: PersistedDelegationStatus,
        child_operation_id: Option<String>,
        summary: Option<String>,
    ) -> Result<(), CodingSessionError> {
        if let Some(transaction) = self.transaction.as_mut() {
            let session_tool_call_id = self
                .tool_session_call_ids
                .get(&request.tool_call_id)
                .cloned()
                .unwrap_or_else(|| request.tool_call_id.clone());
            transaction.record_delegation_folded_update(
                session_tool_call_id,
                request.requesting_profile_id.clone(),
                request.target_kind,
                request.target_id.clone(),
                request.task.clone(),
                status,
                child_operation_id,
                summary,
            )?;
        }
        Ok(())
    }

    pub(crate) fn request_abort(&mut self, reason: impl Into<String>) {
        self.requested_abort_reason = Some(reason.into());
    }

    pub(crate) fn abort_reason(&self) -> Option<&str> {
        self.requested_abort_reason.as_deref()
    }

    pub(crate) fn diagnostics(&self) -> &[CodingDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn record_final_message(&mut self, message: AssistantMessage) {
        self.final_message = Some(message);
    }

    pub(crate) fn final_message(&self) -> Option<&AssistantMessage> {
        self.final_message.as_ref()
    }

    pub(crate) fn record_agent_event(
        &mut self,
        event: AgentEvent,
    ) -> Result<Vec<CodingAgentEvent>, CodingSessionError> {
        self.record_agent_event_to_transaction(&event)?;
        let mut mapping_context = AgentEventMappingContext::new(
            self.operation_id().to_owned(),
            self.turn_id().to_owned(),
        );
        if let Some(message_id) = self
            .assistant_session_message_id
            .clone()
            .or_else(|| self.completed_assistant_session_message_id.clone())
        {
            mapping_context = mapping_context.with_assistant_message_id(message_id);
        }
        let coding_events = map_agent_event(&mapping_context, &event);
        self.record_delegation_requests(&coding_events);
        self.coding_events.extend(coding_events.clone());
        if let Some(event_service) = &self.live_event_service {
            for event in &coding_events {
                if !is_prompt_outcome_event(event) {
                    event_service.emit(event.clone());
                }
            }
        }
        self.agent_observations.push(AgentRunObservation {
            event,
            coding_events: coding_events.clone(),
        });
        Ok(coding_events)
    }

    pub(crate) fn agent_observations(&self) -> &[AgentRunObservation] {
        &self.agent_observations
    }

    pub(crate) fn coding_events(&self) -> &[CodingAgentEvent] {
        &self.coding_events
    }

    pub(crate) fn delegation_requests(&self) -> &[DelegationRequest] {
        &self.delegation_requests
    }

    pub(crate) fn authorize_delegation_requests(
        &mut self,
        current_depth: usize,
    ) -> Result<&[DelegationAuthorizationDecision], CodingSessionError> {
        self.authorize_delegation_requests_with_lineage(current_depth, &[])
    }

    pub(crate) fn authorize_delegation_requests_with_lineage(
        &mut self,
        current_depth: usize,
        lineage: &[DelegationLineageEntry],
    ) -> Result<&[DelegationAuthorizationDecision], CodingSessionError> {
        if self.delegation_requests.is_empty() {
            self.delegation_authorization_decisions.clear();
            return Ok(&self.delegation_authorization_decisions);
        }
        let policy = self
            .runtime
            .as_ref()
            .and_then(RuntimeSnapshot::profile_delegation_policy)
            .cloned()
            .ok_or_else(|| CodingSessionError::Config {
                message: "prompt turn cannot authorize delegation without active profile policy"
                    .into(),
            })?;
        self.delegation_authorization_decisions = authorize_delegation_requests_with_lineage(
            &self.delegation_requests,
            &policy,
            current_depth,
            lineage,
        );
        Ok(&self.delegation_authorization_decisions)
    }

    pub(crate) fn delegation_authorization_decisions(&self) -> &[DelegationAuthorizationDecision] {
        &self.delegation_authorization_decisions
    }

    fn record_delegation_requests(&mut self, events: &[CodingAgentEvent]) {
        for event in events {
            if let CodingAgentEvent::DelegationRequested {
                operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
            } = event
            {
                self.delegation_requests.push(DelegationRequest {
                    operation_id: operation_id.clone(),
                    turn_id: turn_id.clone(),
                    tool_call_id: tool_call_id.clone(),
                    requesting_profile_id: requesting_profile_id.clone(),
                    target_kind: *target_kind,
                    target_id: target_id.clone(),
                    task: task.clone(),
                });
            }
        }
    }

    pub(crate) fn record_prompt_completed(&mut self) -> Result<(), CodingSessionError> {
        if self.final_message.is_none() {
            return Err(CodingSessionError::Session {
                message: "prompt turn cannot emit completion without a final assistant message"
                    .into(),
            });
        }

        if self.coding_events.iter().any(|event| {
            matches!(
                event,
                CodingAgentEvent::PromptCompleted {
                    operation_id,
                    turn_id,
                } if operation_id == self.operation_id() && turn_id == self.turn_id()
            )
        }) {
            return Ok(());
        }

        self.coding_events
            .push(EventService::prompt_completed_event(
                self.operation_id().to_owned(),
                self.turn_id().to_owned(),
            ));
        Ok(())
    }

    fn record_agent_event_to_transaction(
        &mut self,
        event: &AgentEvent,
    ) -> Result<(), CodingSessionError> {
        if self.transaction.is_none() {
            return Ok(());
        }

        match event {
            AgentEvent::LlmEvent(event) => self.record_assistant_event_to_transaction(event),
            AgentEvent::ToolCallStart {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                self.ensure_tool_session_call_started(tool_call_id, tool_name, Some(arguments))?;
                Ok(())
            }
            AgentEvent::ToolCallUpdate {
                tool_call_id,
                tool_name,
                update,
            } => {
                let session_tool_call_id =
                    self.ensure_tool_session_call_started(tool_call_id, tool_name, None)?;
                let message = content_blocks_text(&update.content);
                self.transaction_mut_required()?
                    .record_tool_updated(session_tool_call_id, message)
            }
            AgentEvent::ToolCallEnd {
                tool_call_id,
                tool_name,
                result,
            } => self.record_tool_result_to_transaction(tool_call_id, tool_name, result),
            AgentEvent::AgentDone { .. } => Ok(()),
            AgentEvent::AgentError { error } => self
                .transaction_mut_required()?
                .emit_diagnostic(DiagnosticLevel::Error, error.clone()),
            AgentEvent::TurnStart { .. }
            | AgentEvent::BeforeProviderRequest { .. }
            | AgentEvent::SessionCompacted { .. } => Ok(()),
        }
    }

    fn record_assistant_event_to_transaction(
        &mut self,
        event: &AssistantMessageEvent,
    ) -> Result<(), CodingSessionError> {
        match event {
            AssistantMessageEvent::Start { .. }
            | AssistantMessageEvent::TextStart { .. }
            | AssistantMessageEvent::ThinkingStart { .. }
            | AssistantMessageEvent::TextDelta { .. }
            | AssistantMessageEvent::ThinkingDelta { .. }
            | AssistantMessageEvent::ToolcallStart { .. }
            | AssistantMessageEvent::ToolcallDelta { .. }
            | AssistantMessageEvent::ToolcallEnd { .. } => {
                self.ensure_assistant_session_message_started()?;
                Ok(())
            }
            AssistantMessageEvent::Done { message, .. } => {
                self.complete_current_assistant_message(message)
            }
            AssistantMessageEvent::Error { message, .. } => {
                self.transaction_mut_required()?.emit_diagnostic(
                    DiagnosticLevel::Error,
                    message
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "assistant stream failed".into()),
                )
            }
            AssistantMessageEvent::TextEnd { .. } | AssistantMessageEvent::ThinkingEnd { .. } => {
                Ok(())
            }
        }
    }

    fn record_tool_result_to_transaction(
        &mut self,
        agent_tool_call_id: &str,
        tool_name: &str,
        result: &AgentToolResult,
    ) -> Result<(), CodingSessionError> {
        let session_tool_call_id =
            self.ensure_tool_session_call_started(agent_tool_call_id, tool_name, None)?;
        if result.is_error {
            self.transaction_mut_required()?
                .record_tool_failed(session_tool_call_id, content_blocks_text(&result.content))
        } else {
            self.transaction_mut_required()?
                .record_tool_completed(session_tool_call_id, persisted_tool_result(&result.content))
        }
    }

    fn ensure_assistant_session_message_started(&mut self) -> Result<String, CodingSessionError> {
        if let Some(message_id) = &self.assistant_session_message_id {
            return Ok(message_id.clone());
        }
        let message_id = self.transaction_mut_required()?.start_assistant_message()?;
        self.assistant_session_message_id = Some(message_id.clone());
        self.completed_assistant_session_message_id = None;
        Ok(message_id)
    }

    fn complete_current_assistant_message(
        &mut self,
        message: &AssistantMessage,
    ) -> Result<(), CodingSessionError> {
        let message_id = self.ensure_assistant_session_message_started()?;
        let content = persisted_assistant_content_blocks(&message.content);
        self.transaction_mut_required()?
            .complete_assistant_message(
                message_id.clone(),
                content,
                stop_reason_string(message),
                message.usage.clone(),
            )?;
        self.assistant_session_message_id = None;
        self.completed_assistant_session_message_id = Some(message_id);
        Ok(())
    }

    fn ensure_tool_session_call_started(
        &mut self,
        agent_tool_call_id: &str,
        tool_name: &str,
        arguments: Option<&serde_json::Value>,
    ) -> Result<String, CodingSessionError> {
        if let Some(tool_call_id) = self.tool_session_call_ids.get(agent_tool_call_id) {
            return Ok(tool_call_id.clone());
        }
        let arguments = arguments.cloned().unwrap_or_else(|| serde_json::json!({}));
        let tool_call_id = self
            .transaction_mut_required()?
            .record_tool_started(tool_name, arguments)?;
        self.tool_session_call_ids
            .insert(agent_tool_call_id.to_owned(), tool_call_id.clone());
        Ok(tool_call_id)
    }

    fn transaction_mut_required(
        &mut self,
    ) -> Result<&mut PromptTurnTransaction, CodingSessionError> {
        self.transaction
            .as_mut()
            .ok_or_else(|| CodingSessionError::Session {
                message: "prompt turn has no active transaction".into(),
            })
    }

    fn require_resolved_request(&self, action: &str) -> Result<(), CodingSessionError> {
        if self.request_resolved {
            return Ok(());
        }
        Err(CodingSessionError::Session {
            message: format!("prompt turn cannot {action} before request is resolved"),
        })
    }

    pub(crate) fn finish_success(
        &self,
        session_id: Option<String>,
        leaf_id: Option<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        let final_message =
            self.final_message
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "prompt turn cannot finish successfully without a final message"
                        .into(),
                })?;
        Ok(PromptTurnOutcome::Success {
            operation_id: self.operation_id().to_owned(),
            turn_id: self.turn_id().to_owned(),
            session_id,
            leaf_id,
            final_text: assistant_text(&final_message),
            final_message,
            diagnostics: self.diagnostics.clone(),
        })
    }

    pub(crate) fn finish_abort(
        &self,
        reason: impl Into<String>,
        session_id: Option<String>,
    ) -> PromptTurnOutcome {
        PromptTurnOutcome::Aborted {
            operation_id: self.operation_id().to_owned(),
            turn_id: Some(self.turn_id().to_owned()),
            reason: reason.into(),
            session_id,
        }
    }

    pub(crate) fn finish_failure(&self, error: CodingSessionError) -> PromptTurnOutcome {
        PromptTurnOutcome::Failed {
            operation_id: self.operation_id().to_owned(),
            turn_id: Some(self.turn_id().to_owned()),
            error,
            diagnostics: self.diagnostics.clone(),
        }
    }
}

fn stop_reason_string(message: &AssistantMessage) -> Option<String> {
    serde_json::to_value(&message.stop_reason)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
}

fn persisted_tool_result(content: &[ContentBlock]) -> PersistedToolResult {
    PersistedToolResult::Text {
        text: content_blocks_text(content),
    }
}

fn persisted_content_blocks_from_invocation(
    invocation: &PromptInvocation,
) -> Result<Vec<PersistedContentBlock>, CodingSessionError> {
    match invocation {
        PromptInvocation::Text(text) if !text.is_empty() => {
            Ok(vec![PersistedContentBlock::Text { text: text.clone() }])
        }
        PromptInvocation::Text(_) => Err(CodingSessionError::Input {
            message: "prompt turn requires non-empty text input".into(),
        }),
        PromptInvocation::Content(content) if !content.is_empty() => {
            Ok(content.iter().map(persisted_content_block).collect())
        }
        PromptInvocation::Content(_) => Err(CodingSessionError::Input {
            message: "prompt turn requires non-empty content input".into(),
        }),
        PromptInvocation::Skill {
            name,
            additional_instructions,
        } => {
            let text = match additional_instructions {
                Some(instructions) if !instructions.is_empty() => {
                    format!("skill:{name}\n{instructions}")
                }
                _ => format!("skill:{name}"),
            };
            Ok(vec![PersistedContentBlock::Text { text }])
        }
        PromptInvocation::PromptTemplate { name, args } => {
            let text = if args.is_empty() {
                format!("prompt_template:{name}")
            } else {
                format!("prompt_template:{name}\n{}", args.join("\n"))
            };
            Ok(vec![PersistedContentBlock::Text { text }])
        }
        PromptInvocation::Compact { .. } => Err(CodingSessionError::UnsupportedCapability {
            capability: "manual compaction in PromptTurnFlow".into(),
        }),
    }
}

fn persisted_content_block(content: &ContentBlock) -> PersistedContentBlock {
    match content {
        ContentBlock::Text { text, .. } => PersistedContentBlock::Text { text: text.clone() },
        ContentBlock::Image { mime_type, data } => PersistedContentBlock::Image {
            mime_type: mime_type.clone(),
            data: data.clone(),
        },
        ContentBlock::Thinking {
            thinking,
            thinking_signature,
            redacted,
        } => PersistedContentBlock::Thinking {
            thinking: thinking.clone(),
            thinking_signature: thinking_signature.clone(),
            redacted: *redacted,
        },
        ContentBlock::ToolCall {
            name, arguments, ..
        } => PersistedContentBlock::Text {
            text: format!("[tool_call:{name} {arguments}]"),
        },
    }
}

fn persisted_assistant_content_blocks(content: &[ContentBlock]) -> Vec<PersistedContentBlock> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => {
                Some(PersistedContentBlock::Text { text: text.clone() })
            }
            ContentBlock::Thinking {
                thinking,
                thinking_signature,
                redacted,
            } => Some(PersistedContentBlock::Thinking {
                thinking: thinking.clone(),
                thinking_signature: thinking_signature.clone(),
                redacted: *redacted,
            }),
            ContentBlock::Image { mime_type, data } => Some(PersistedContentBlock::Image {
                mime_type: mime_type.clone(),
                data: data.clone(),
            }),
            ContentBlock::ToolCall { .. } => None,
        })
        .collect()
}

fn persisted_content_blocks_text(content: &[PersistedContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            PersistedContentBlock::Text { text } => text.clone(),
            PersistedContentBlock::Thinking { thinking, .. } => thinking.clone(),
            PersistedContentBlock::Image { mime_type, .. } => format!("[image:{mime_type}]"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_prompt_outcome_event(event: &CodingAgentEvent) -> bool {
    matches!(
        event,
        CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::PromptFailed { .. }
            | CodingAgentEvent::PromptAborted { .. }
    )
}

fn content_blocks_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text, .. } => text.clone(),
            ContentBlock::Thinking { thinking, .. } => thinking.clone(),
            ContentBlock::Image { mime_type, .. } => format!("[image:{mime_type}]"),
            ContentBlock::ToolCall { name, .. } => format!("[tool_call:{name}]"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use pi_agent_core::{AgentEvent, AgentResources, AgentToolResult, ToolExecutionMode};
    use pi_ai::types::{ContentBlock, Model, ModelCost, ModelInput};

    use super::super::delegation::DelegationAuthorizationDecision;
    use super::super::profiles::{DelegationConfirmationMode, ProfileSource, SupervisionPolicy};
    use super::*;
    use crate::runtime::{SessionMode, SessionRunOptions};

    fn model() -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: "messages".into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn session_prompt_options() -> PromptRunOptions {
        PromptRunOptions {
            prompt: "hello".into(),
            model: model(),
            api_key: Some("key".into()),
            auth_diagnostics: Vec::new(),
            system_prompt: Some("system".into()),
            max_turns: Some(3),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions {
                mode: SessionMode::Enabled,
                cwd: ".".into(),
                session_dir: Some("sessions".into()),
            }),
            session_target: Some(ResolvedSessionTarget::New),
            session_name: Some("test".into()),
            thinking_level: None,
            tool_execution: Some(ToolExecutionMode::Sequential),
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text("hello".into()),
        }
    }

    #[test]
    fn prompt_turn_options_tracks_invocation_mode_and_session_metadata() {
        let options = PromptTurnOptions::new(PromptInvocation::Text("hello".into()))
            .with_mode(PromptTurnMode::Json)
            .with_session_target(ResolvedSessionTarget::New)
            .with_session_name("named");

        assert!(matches!(
            options.invocation(),
            PromptInvocation::Text(text) if text == "hello"
        ));
        assert_eq!(options.mode(), PromptTurnMode::Json);
        assert!(matches!(
            options.session_target(),
            Some(ResolvedSessionTarget::New)
        ));
        assert_eq!(options.session_name(), Some("named"));
    }

    #[test]
    fn runtime_snapshot_moves_existing_session_prompt_runtime_inputs() {
        let snapshot = RuntimeSnapshot::from_prompt_run_options(session_prompt_options());

        assert_eq!(snapshot.model().id, "test-model");
        assert_eq!(snapshot.api_key(), Some("key"));
        assert!(snapshot.auth_diagnostics().is_empty());
        assert_eq!(snapshot.system_prompt(), Some("system"));
        assert_eq!(snapshot.max_turns(), Some(3));
        assert!(snapshot.tools().is_empty());
        assert!(!snapshot.register_builtins());
        assert!(snapshot.resources().is_empty());
        assert!(snapshot.settings().is_none());
        assert_eq!(snapshot.thinking_level(), None);
        assert_eq!(
            snapshot.tool_execution(),
            Some(ToolExecutionMode::Sequential)
        );
        assert!(matches!(
            snapshot.session_run_options().map(|options| &options.mode),
            Some(SessionMode::Enabled)
        ));
    }

    #[test]
    fn prompt_turn_context_records_diagnostics_and_success_outcome() {
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );
        context.record_diagnostic(CodingDiagnostic::info("prepared"));
        let mut message = AssistantMessage::empty("messages", "test-model");
        message.content.push(ContentBlock::Text {
            text: "hi".into(),
            text_signature: None,
        });
        context.record_final_message(message.clone());

        let outcome = context
            .finish_success(Some("sess_1".into()), Some("leaf_1".into()))
            .unwrap();

        assert_eq!(context.diagnostics().len(), 1);
        assert_eq!(context.final_message(), Some(&message));
        assert_eq!(
            outcome,
            PromptTurnOutcome::Success {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                session_id: Some("sess_1".into()),
                leaf_id: Some("leaf_1".into()),
                final_text: "hi".into(),
                final_message: message,
                diagnostics: vec![CodingDiagnostic::info("prepared")],
            }
        );
    }

    #[test]
    fn prompt_turn_context_requires_final_message_for_success() {
        let context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );

        let error = context.finish_success(None, None).unwrap_err();

        assert_eq!(error.code(), "session");
        assert!(
            error
                .to_string()
                .contains("cannot finish successfully without a final message")
        );
    }

    #[test]
    fn prompt_turn_context_queues_requested_delegation() {
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );
        let envelope = serde_json::json!({
            "status": "requested",
            "target_kind": "agent",
            "target_id": "coder",
            "task": "implement parser",
            "requesting_profile_id": "planner",
            "message": "delegation request captured for session-owned authorization"
        })
        .to_string();

        context
            .record_agent_event(AgentEvent::ToolCallEnd {
                tool_call_id: "tool_delegate".into(),
                tool_name: "delegate_agent".into(),
                result: AgentToolResult::ok(vec![ContentBlock::Text {
                    text: envelope,
                    text_signature: None,
                }]),
            })
            .unwrap();

        assert!(context.coding_events().iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRequested {
                tool_call_id,
                target_id,
                ..
            } if tool_call_id == "tool_delegate" && target_id.as_str() == "coder"
        )));
        let requests = context.delegation_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation_id, "op_1");
        assert_eq!(requests[0].turn_id, "turn_1");
        assert_eq!(requests[0].tool_call_id, "tool_delegate");
        assert_eq!(requests[0].requesting_profile_id.as_str(), "planner");
        assert_eq!(
            requests[0].target_kind,
            super::super::profiles::ProfileKind::Agent
        );
        assert_eq!(requests[0].target_id.as_str(), "coder");
        assert_eq!(requests[0].task, "implement parser");
    }

    #[test]
    fn prompt_turn_context_does_not_queue_rejected_delegation() {
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );
        let envelope = serde_json::json!({
            "status": "rejected",
            "target_kind": "team",
            "target_id": "implementation",
            "task": "ship feature",
            "requesting_profile_id": "planner",
            "message": "target is not allowed by delegation policy"
        })
        .to_string();

        context
            .record_agent_event(AgentEvent::ToolCallEnd {
                tool_call_id: "tool_delegate".into(),
                tool_name: "delegate_team".into(),
                result: AgentToolResult::ok(vec![ContentBlock::Text {
                    text: envelope,
                    text_signature: None,
                }]),
            })
            .unwrap();

        assert!(context.coding_events().iter().any(|event| matches!(
            event,
            CodingAgentEvent::DelegationRejected { tool_call_id, .. }
                if tool_call_id == "tool_delegate"
        )));
        assert!(context.delegation_requests().is_empty());
    }

    #[test]
    fn prompt_turn_context_authorizes_queued_delegation_from_runtime_policy() {
        let mut options = PromptTurnOptions::from_prompt_run_options(session_prompt_options());
        options
            .apply_agent_profile(
                &AgentProfile {
                    schema_version: 1,
                    id: ProfileId::from("planner"),
                    display_name: "Planner".into(),
                    description: None,
                    model: None,
                    system_prompt: None,
                    tools: Vec::new(),
                    skills: Vec::new(),
                    supervision: SupervisionPolicy::Session,
                    delegation: DelegationPolicy {
                        allow_delegate_team: true,
                        max_depth: 1,
                        max_parallel_children: 1,
                        require_confirmation: DelegationConfirmationMode::Writes,
                        allowed_teams: vec![ProfileId::from("implementation")],
                        ..DelegationPolicy::default()
                    },
                    source: ProfileSource::BuiltIn,
                    path: None,
                },
                Vec::new(),
            )
            .unwrap();
        let mut context = PromptTurnContext::new(PromptTurnIds::new("op_1", "turn_1"), options);
        context.resolve_request().unwrap();
        context.resolve_runtime_from_options().unwrap();
        let envelope = serde_json::json!({
            "status": "requested",
            "target_kind": "team",
            "target_id": "implementation",
            "task": "ship feature",
            "requesting_profile_id": "planner",
            "message": "delegation request captured for session-owned authorization"
        })
        .to_string();

        context
            .record_agent_event(AgentEvent::ToolCallEnd {
                tool_call_id: "tool_delegate".into(),
                tool_name: "delegate_team".into(),
                result: AgentToolResult::ok(vec![ContentBlock::Text {
                    text: envelope,
                    text_signature: None,
                }]),
            })
            .unwrap();

        let decisions = context.authorize_delegation_requests(0).unwrap();
        assert_eq!(decisions.len(), 1);
        assert!(matches!(
            &decisions[0],
            DelegationAuthorizationDecision::RequiresConfirmation { request, reason, .. }
                if request.target_id.as_str() == "implementation" && reason.contains("team")
        ));
        assert_eq!(context.delegation_authorization_decisions().len(), 1);
    }

    #[test]
    fn prompt_turn_context_builds_failure_outcome_with_diagnostics() {
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::new(PromptInvocation::Text("hello".into())),
        );
        context.record_diagnostic(CodingDiagnostic::error("provider failed"));

        let outcome = context.finish_failure(CodingSessionError::Provider {
            message: "stream failed".into(),
        });

        assert_eq!(
            outcome,
            PromptTurnOutcome::Failed {
                operation_id: "op_1".into(),
                turn_id: Some("turn_1".into()),
                error: CodingSessionError::Provider {
                    message: "stream failed".into(),
                },
                diagnostics: vec![CodingDiagnostic::error("provider failed")],
            }
        );
    }
}
