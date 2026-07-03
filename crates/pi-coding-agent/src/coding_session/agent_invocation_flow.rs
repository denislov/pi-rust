#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;

use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
use pi_ai::types::AssistantMessage;

use super::CodingSessionError;
use super::delegation::emit_child_delegation_authorization_decision;
use super::event::CodingAgentEvent;
use super::event_service::EventService;
use super::plugin_service::PluginService;
use super::profiles::{AgentProfile, ProfileId, ProfileRegistry};
use super::prompt::{
    CodingDiagnostic, PromptTurnContext, PromptTurnIds, PromptTurnOptions, PromptTurnOutcome,
};
use super::prompt_flow::PromptTurnFlow;
use super::session_log::id::{IdGenerator, SystemIdGenerator};
use crate::runtime::PromptInvocation;

const DEFAULT_ACTION: &str = "default";

pub(crate) const AGENT_INVOCATION_NODE_IDS: &[&str] = &[
    "start_agent_invocation",
    "resolve_agent_profile",
    "prepare_child_prompt",
    "run_child_agent",
    "finalize_agent_invocation",
];

const AGENT_INVOCATION_NODE_SPECS: &[AgentInvocationNodeSpec] = &[
    AgentInvocationNodeSpec {
        id: "start_agent_invocation",
        name: "StartAgentInvocation",
        kind: AgentInvocationNodeKind::StartAgentInvocation,
    },
    AgentInvocationNodeSpec {
        id: "resolve_agent_profile",
        name: "ResolveAgentProfile",
        kind: AgentInvocationNodeKind::ResolveAgentProfile,
    },
    AgentInvocationNodeSpec {
        id: "prepare_child_prompt",
        name: "PrepareChildPrompt",
        kind: AgentInvocationNodeKind::PrepareChildPrompt,
    },
    AgentInvocationNodeSpec {
        id: "run_child_agent",
        name: "RunChildAgent",
        kind: AgentInvocationNodeKind::RunChildAgent,
    },
    AgentInvocationNodeSpec {
        id: "finalize_agent_invocation",
        name: "FinalizeAgentInvocation",
        kind: AgentInvocationNodeKind::FinalizeAgentInvocation,
    },
];

#[derive(Debug, Clone)]
pub struct AgentInvocationOptions {
    profile_id: ProfileId,
    task: String,
    prompt_options: PromptTurnOptions,
    delegation_depth: usize,
}

impl AgentInvocationOptions {
    pub fn new(
        profile_id: impl Into<ProfileId>,
        task: impl Into<String>,
        prompt_options: PromptTurnOptions,
    ) -> Self {
        Self {
            profile_id: profile_id.into(),
            task: task.into(),
            prompt_options,
            delegation_depth: 0,
        }
    }

    pub fn with_delegation_depth(mut self, depth: usize) -> Self {
        self.delegation_depth = depth;
        self
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn task(&self) -> &str {
        &self.task
    }

    pub fn prompt_options(&self) -> &PromptTurnOptions {
        &self.prompt_options
    }

    pub fn delegation_depth(&self) -> usize {
        self.delegation_depth
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentInvocationOutcome {
    pub operation_id: String,
    pub child_operation_id: String,
    pub turn_id: String,
    pub profile_id: ProfileId,
    pub final_text: String,
    pub final_message: AssistantMessage,
    pub diagnostics: Vec<CodingDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentInvocationNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: AgentInvocationNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentInvocationNodeKind {
    StartAgentInvocation,
    ResolveAgentProfile,
    PrepareChildPrompt,
    RunChildAgent,
    FinalizeAgentInvocation,
}

pub(crate) struct AgentInvocationFlow {
    flow: Flow<AgentInvocationContext>,
}

impl AgentInvocationFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(AGENT_INVOCATION_NODE_IDS[0]).map_err(flow_error)?;
        for spec in AGENT_INVOCATION_NODE_SPECS {
            flow.add_node(spec.id, AgentInvocationNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in AGENT_INVOCATION_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    pub(crate) fn node_ids() -> &'static [&'static str] {
        AGENT_INVOCATION_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }

    pub(crate) async fn run_with_options(
        &self,
        ctx: &mut AgentInvocationContext,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow
            .run_with_options(ctx, options)
            .await
            .map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct AgentInvocationNode {
    name: &'static str,
    kind: AgentInvocationNodeKind,
}

impl AgentInvocationNode {
    fn new(name: &'static str, kind: AgentInvocationNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<AgentInvocationContext> for AgentInvocationNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentInvocationContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                AgentInvocationNodeKind::StartAgentInvocation => ctx.start_agent_invocation(),
                AgentInvocationNodeKind::ResolveAgentProfile => ctx.resolve_agent_profile(),
                AgentInvocationNodeKind::PrepareChildPrompt => ctx.prepare_child_prompt(),
                AgentInvocationNodeKind::RunChildAgent => ctx.run_child_agent().await,
                AgentInvocationNodeKind::FinalizeAgentInvocation => ctx.finalize_agent_invocation(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

pub(crate) struct AgentInvocationContext {
    options: AgentInvocationOptions,
    registry: ProfileRegistry,
    plugin_service: PluginService,
    event_service: EventService,
    operation_id: String,
    child_operation_id: String,
    turn_id: String,
    profile: Option<AgentProfile>,
    child_context: Option<PromptTurnContext>,
    prompt_outcome: Option<PromptTurnOutcome>,
    failure_error: Option<CodingSessionError>,
}

impl AgentInvocationContext {
    pub(crate) fn new(
        options: AgentInvocationOptions,
        registry: ProfileRegistry,
        plugin_service: PluginService,
        event_service: EventService,
    ) -> Self {
        let mut ids = SystemIdGenerator;
        Self {
            options,
            registry,
            plugin_service,
            event_service,
            operation_id: ids.next_operation_id(),
            child_operation_id: ids.next_operation_id(),
            turn_id: ids.next_turn_id(),
            profile: None,
            child_context: None,
            prompt_outcome: None,
            failure_error: None,
        }
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn child_operation_id(&self) -> &str {
        &self.child_operation_id
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<AgentInvocationOutcome, CodingSessionError> {
        let outcome = self
            .prompt_outcome
            .as_ref()
            .ok_or_else(|| CodingSessionError::Session {
                message: "agent invocation completed without child prompt outcome".into(),
            })?;
        match outcome {
            PromptTurnOutcome::Success {
                turn_id,
                final_text,
                final_message,
                diagnostics,
                ..
            } => Ok(AgentInvocationOutcome {
                operation_id: self.operation_id.clone(),
                child_operation_id: self.child_operation_id.clone(),
                turn_id: turn_id.clone(),
                profile_id: self.options.profile_id.clone(),
                final_text: final_text.clone(),
                final_message: final_message.clone(),
                diagnostics: diagnostics.clone(),
            }),
            PromptTurnOutcome::Aborted { reason, .. } => Err(CodingSessionError::Session {
                message: format!("agent invocation aborted: {reason}"),
            }),
            PromptTurnOutcome::Failed { error, .. } => Err(error.clone()),
        }
    }

    fn start_agent_invocation(&mut self) -> Result<(), CodingSessionError> {
        if self.options.task.trim().is_empty() {
            return Err(CodingSessionError::Input {
                message: "agent invocation requires a non-empty task".into(),
            });
        }
        self.event_service
            .emit(CodingAgentEvent::AgentInvocationStarted {
                operation_id: self.operation_id.clone(),
                child_operation_id: self.child_operation_id.clone(),
                profile_id: self.options.profile_id.clone(),
                task: self.options.task.clone(),
            });
        Ok(())
    }

    fn resolve_agent_profile(&mut self) -> Result<(), CodingSessionError> {
        let profile = self
            .registry
            .agent(self.options.profile_id.as_str())
            .cloned()
            .ok_or_else(|| CodingSessionError::Input {
                message: format!("Unknown agent profile: {}", self.options.profile_id),
            })?;
        self.profile = Some(profile);
        Ok(())
    }

    fn prepare_child_prompt(&mut self) -> Result<(), CodingSessionError> {
        let profile = self
            .profile
            .as_ref()
            .ok_or_else(|| CodingSessionError::Session {
                message: "agent invocation cannot prepare child prompt before profile resolution"
                    .into(),
            })?;
        let mut prompt_options = self.options.prompt_options.clone();
        prompt_options.set_invocation(PromptInvocation::Text(self.options.task.clone()));
        prompt_options.apply_agent_profile(profile, Vec::new())?;
        if prompt_options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "agent invocation options do not include a runtime snapshot".into(),
            });
        }
        let mut child_context = PromptTurnContext::new(
            PromptTurnIds::new(self.child_operation_id.clone(), self.turn_id.clone()),
            prompt_options,
        );
        child_context.set_plugin_service(self.plugin_service.clone());
        child_context.set_non_persistent_session(
            format!("agent_invocation_{}", self.child_operation_id),
            Vec::new(),
        );
        child_context.enable_live_events(self.event_service.clone());
        self.child_context = Some(child_context);
        Ok(())
    }

    async fn run_child_agent(&mut self) -> Result<(), CodingSessionError> {
        let child_context =
            self.child_context
                .as_mut()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "agent invocation cannot run before child prompt preparation".into(),
                })?;
        let outcome = match PromptTurnFlow::new()?.run(child_context).await {
            Ok(_) => {
                let decisions = child_context
                    .authorize_delegation_requests(self.options.delegation_depth)?
                    .to_vec();
                for decision in &decisions {
                    emit_child_delegation_authorization_decision(&self.event_service, decision);
                }
                child_context.finish_success(
                    child_context.non_persistent_runtime_id().map(str::to_owned),
                    None,
                )?
            }
            Err(error) => match child_context.abort_reason() {
                Some(reason) => child_context.finish_abort(
                    reason.to_owned(),
                    child_context.non_persistent_runtime_id().map(str::to_owned),
                ),
                None => child_context.finish_failure(error),
            },
        };
        self.prompt_outcome = Some(outcome);
        Ok(())
    }

    fn finalize_agent_invocation(&mut self) -> Result<(), CodingSessionError> {
        let outcome = self
            .prompt_outcome
            .as_ref()
            .ok_or_else(|| CodingSessionError::Session {
                message: "agent invocation cannot finalize without child prompt outcome".into(),
            })?;
        match outcome {
            PromptTurnOutcome::Success {
                turn_id,
                final_text,
                diagnostics,
                ..
            } => {
                for diagnostic in diagnostics {
                    self.event_service.emit(CodingAgentEvent::Diagnostic {
                        operation_id: Some(self.operation_id.clone()),
                        message: diagnostic.message.clone(),
                    });
                }
                self.event_service.emit(CodingAgentEvent::PromptCompleted {
                    operation_id: self.child_operation_id.clone(),
                    turn_id: turn_id.clone(),
                });
                self.event_service
                    .emit(CodingAgentEvent::AgentInvocationCompleted {
                        operation_id: self.operation_id.clone(),
                        child_operation_id: self.child_operation_id.clone(),
                        profile_id: self.options.profile_id.clone(),
                        final_text: final_text.clone(),
                    });
                Ok(())
            }
            PromptTurnOutcome::Aborted { reason, .. } => {
                let error = CodingSessionError::Session {
                    message: format!("agent invocation aborted: {reason}"),
                };
                self.failure_error = Some(error.clone());
                self.event_service.emit(CodingAgentEvent::PromptAborted {
                    operation_id: self.child_operation_id.clone(),
                    reason: reason.clone(),
                });
                self.event_service
                    .emit(CodingAgentEvent::AgentInvocationAborted {
                        operation_id: self.operation_id.clone(),
                        child_operation_id: self.child_operation_id.clone(),
                        profile_id: self.options.profile_id.clone(),
                        reason: reason.clone(),
                    });
                Err(error)
            }
            PromptTurnOutcome::Failed { error, .. } => {
                self.failure_error = Some(error.clone());
                self.event_service.emit(CodingAgentEvent::PromptFailed {
                    operation_id: self.child_operation_id.clone(),
                    error: error.clone(),
                });
                self.event_service
                    .emit(CodingAgentEvent::AgentInvocationFailed {
                        operation_id: self.operation_id.clone(),
                        child_operation_id: self.child_operation_id.clone(),
                        profile_id: self.options.profile_id.clone(),
                        error: error.clone(),
                    });
                Err(error.clone())
            }
        }
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        if self.failure_error.is_none() {
            self.failure_error = Some(error.clone());
            self.event_service
                .emit(CodingAgentEvent::AgentInvocationFailed {
                    operation_id: self.operation_id.clone(),
                    child_operation_id: self.child_operation_id.clone(),
                    profile_id: self.options.profile_id.clone(),
                    error: error.clone(),
                });
        }
        error.to_string()
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
