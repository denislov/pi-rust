use std::future::Future;
use std::pin::Pin;

use pi_agent_core::api::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
use pi_ai::api::conversation::AssistantMessage;
use tokio_util::sync::CancellationToken;

use crate::app::bootstrap::PromptInvocation;
use crate::operations::delegation::{
    DelegationAuthorizationDecision, DelegationLineageEntry,
    capability_snapshot_for_delegated_profile, delegation_lineage_for_request,
};
use crate::operations::prompt::context::{
    CodingDiagnostic, PromptTurnContext, PromptTurnIds, PromptTurnOptions, PromptTurnOutcome,
};
use crate::profiles::{AgentProfile, ProfileId, ProfileKind, ProfileRegistry};
use crate::runtime::capability::{ActorId, OperationCapabilitySnapshot};
use crate::runtime::control::{OperationControl, OperationKind, PromptControlReceiver};
use crate::runtime::facade::{CodingSessionError, PendingDelegationConfirmationState};
use crate::runtime::scheduler::OperationScheduler;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::PluginService;
use crate::session::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};

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
    delegation_lineage: Vec<DelegationLineageEntry>,
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
            delegation_lineage: Vec::new(),
        }
    }

    pub fn with_delegation_depth(mut self, depth: usize) -> Self {
        self.delegation_depth = depth;
        self
    }

    pub(crate) fn with_delegation_lineage(mut self, lineage: Vec<DelegationLineageEntry>) -> Self {
        self.delegation_lineage = lineage;
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

    pub(crate) fn prompt_options_mut(&mut self) -> &mut PromptTurnOptions {
        &mut self.prompt_options
    }

    pub fn delegation_depth(&self) -> usize {
        self.delegation_depth
    }

    pub(crate) fn delegation_lineage(&self) -> &[DelegationLineageEntry] {
        &self.delegation_lineage
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

    pub(crate) async fn run(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }

    pub(crate) async fn run_with_cancellation(
        &self,
        ctx: &mut AgentInvocationContext,
        cancellation: CancellationToken,
    ) -> Result<FlowOutcome, CodingSessionError> {
        let result = self
            .flow
            .run_with_options(
                ctx,
                FlowRunOptions {
                    cancel: Some(cancellation),
                    ..FlowRunOptions::default()
                },
            )
            .await
            .map_err(flow_error);
        if let Err(error @ CodingSessionError::Cancelled) = &result {
            ctx.fail(error.clone());
        }
        result
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
    operation_control: OperationControl,
    operation_id: String,
    child_operation_id: String,
    turn_id: String,
    profile: Option<AgentProfile>,
    child_context: Option<PromptTurnContext>,
    prompt_control_receiver: Option<PromptControlReceiver>,
    prompt_outcome: Option<PromptTurnOutcome>,
    parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
    child_capability_snapshot: Option<OperationCapabilitySnapshot>,
    child_admission: Option<crate::runtime::intent::OperationPermit>,
    pending_delegation_confirmations: Vec<PendingDelegationConfirmationState>,
    failure_error: Option<CodingSessionError>,
    defer_terminal_publication: bool,
}

impl AgentInvocationContext {
    pub(crate) fn new(
        options: AgentInvocationOptions,
        registry: ProfileRegistry,
        plugin_service: PluginService,
        event_service: EventService,
        operation_control: OperationControl,
        operation_id: String,
    ) -> Self {
        let mut ids = SystemIdGenerator;
        Self {
            options,
            registry,
            plugin_service,
            event_service,
            operation_control,
            operation_id,
            child_operation_id: OperationScheduler::allocate_child_operation_id(),
            turn_id: ids.next_turn_id(),
            profile: None,
            child_context: None,
            prompt_control_receiver: None,
            prompt_outcome: None,
            parent_capability_snapshot: None,
            child_capability_snapshot: None,
            child_admission: None,
            pending_delegation_confirmations: Vec::new(),
            failure_error: None,
            defer_terminal_publication: false,
        }
    }

    pub(crate) fn with_parent_capability_snapshot(
        mut self,
        snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        self.parent_capability_snapshot = Some(snapshot);
        self
    }

    pub(crate) fn with_deferred_terminal_publication(mut self) -> Self {
        self.defer_terminal_publication = true;
        self
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn ensure_failure_terminal_draft(&self, error: &CodingSessionError) {
        if self
            .event_service
            .has_deferred_terminal_draft(&self.operation_id)
        {
            return;
        }
        let draft = if *error == CodingSessionError::Cancelled {
            EventService::agent_invocation_aborted_draft(
                self.operation_id.clone(),
                self.child_operation_id.clone(),
                self.options.profile_id.clone(),
                error.to_string(),
            )
        } else {
            EventService::agent_invocation_failed_draft(
                self.operation_id.clone(),
                self.child_operation_id.clone(),
                self.options.profile_id.clone(),
                error,
            )
        };
        self.event_service
            .defer_terminal_draft(self.operation_id.clone(), draft);
    }

    pub(crate) fn take_pending_delegation_confirmations(
        &mut self,
    ) -> Vec<PendingDelegationConfirmationState> {
        std::mem::take(&mut self.pending_delegation_confirmations)
    }

    pub(crate) fn set_prompt_control_receiver(&mut self, receiver: PromptControlReceiver) {
        self.prompt_control_receiver = Some(receiver);
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
        self.event_service.emit_agent_invocation_started(
            self.operation_id.clone(),
            self.child_operation_id.clone(),
            self.options.profile_id.clone(),
            self.options.task.clone(),
        );
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
        if self.options.delegation_depth > 0 {
            prompt_options.apply_delegated_agent_profile(profile, &self.registry, Vec::new())?;
        } else {
            prompt_options.apply_agent_profile(profile, &self.registry, Vec::new())?;
        }
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
        if let Some(receiver) = self.prompt_control_receiver.take() {
            child_context.set_prompt_control_receiver(receiver);
        }
        child_context.enable_live_events(self.event_service.clone());
        let mut capability_snapshot = match self.parent_capability_snapshot.as_ref() {
            Some(parent) => capability_snapshot_for_delegated_profile(
                parent,
                self.child_operation_id.clone(),
                profile,
                ActorId::ChildOperation(parent.operation_id.clone()),
            ),
            None => OperationCapabilitySnapshot::permissive(self.child_operation_id.clone()),
        };
        if self.parent_capability_snapshot.is_none() {
            capability_snapshot.actor = ActorId::ChildOperation(self.operation_id.clone());
        }
        let child_admission = OperationScheduler::admit_child(
            &self.operation_control,
            OperationKind::Prompt,
            capability_snapshot.clone(),
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(cancellation) = child_admission.cancellation_token() {
            child_context.set_operation_cancellation(cancellation);
        }
        self.child_admission = Some(child_admission);
        child_context.set_capability_snapshot(capability_snapshot);
        self.child_context = Some(child_context);
        Ok(())
    }

    async fn run_child_agent(&mut self) -> Result<(), CodingSessionError> {
        let mut finished_outcome = None;
        let child_delegations = {
            let child_context =
                self.child_context
                    .as_mut()
                    .ok_or_else(|| CodingSessionError::Session {
                        message: "agent invocation cannot run before child prompt preparation"
                            .into(),
                    })?;
            match FlowService::new()
                .run_prompt_subflow_for_agent_invocation(child_context)
                .await
            {
                Ok(_) => Some((
                    child_context
                        .authorize_delegation_requests_with_lineage(
                            self.options.delegation_depth,
                            self.options.delegation_lineage(),
                        )?
                        .to_vec(),
                    child_context.options().clone(),
                    child_context.non_persistent_runtime_id().map(str::to_owned),
                )),
                Err(error) => {
                    finished_outcome = Some(match child_context.abort_reason() {
                        Some(reason) => child_context.finish_abort(
                            reason.to_owned(),
                            child_context.non_persistent_runtime_id().map(str::to_owned),
                        ),
                        None => child_context.finish_failure(error),
                    });
                    None
                }
            }
        };

        let Some((decisions, prompt_options, runtime_id)) = child_delegations else {
            self.prompt_outcome = finished_outcome;
            return Ok(());
        };
        self.child_capability_snapshot = self
            .child_context
            .as_ref()
            .and_then(|context| context.capability_snapshot().cloned());
        if let Err(error) = self
            .execute_authorized_delegations(&decisions, prompt_options)
            .await
        {
            self.event_service.emit_diagnostic(
                Some(self.child_operation_id.clone()),
                format!("delegation execution failed: {error}"),
            );
        }
        let child_context =
            self.child_context
                .as_ref()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "agent invocation completed without child prompt context".into(),
                })?;
        self.prompt_outcome = Some(child_context.finish_success(runtime_id, None)?);
        Ok(())
    }

    async fn execute_authorized_delegations(
        &mut self,
        decisions: &[DelegationAuthorizationDecision],
        prompt_options: PromptTurnOptions,
    ) -> Result<(), CodingSessionError> {
        for decision in decisions {
            match decision {
                DelegationAuthorizationDecision::Approved {
                    request,
                    child_delegation_depth,
                } => {
                    self.event_service.emit_delegation_approved(request);
                    match request.target_kind {
                        ProfileKind::Agent => {
                            self.execute_approved_agent_delegation(
                                request,
                                prompt_options.clone(),
                                *child_delegation_depth,
                            )
                            .await?;
                        }
                        ProfileKind::Team => {
                            self.execute_approved_team_delegation(
                                request,
                                prompt_options.clone(),
                                *child_delegation_depth,
                            )
                            .await?;
                        }
                    }
                }
                DelegationAuthorizationDecision::RequiresConfirmation {
                    request,
                    reason,
                    child_delegation_depth,
                } => {
                    self.pending_delegation_confirmations.push(
                        PendingDelegationConfirmationState {
                            request: request.clone(),
                            prompt_options: prompt_options.clone(),
                            reason: reason.clone(),
                            requested_at: SystemClock.now_rfc3339(),
                            child_delegation_depth: *child_delegation_depth,
                            delegation_lineage: delegation_lineage_for_request(
                                self.options.delegation_lineage(),
                                request,
                            ),
                        },
                    );
                    self.event_service
                        .emit_delegation_confirmation_required(request, reason);
                }
                DelegationAuthorizationDecision::Rejected { request, reason } => {
                    self.event_service.emit_delegation_rejected(request, reason);
                }
            }
        }
        Ok(())
    }

    async fn execute_approved_agent_delegation(
        &mut self,
        request: &crate::operations::prompt::context::DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
    ) -> Result<(), CodingSessionError> {
        let outcome = crate::operations::delegation::execution::execute_agent(
            &FlowService::new(),
            self.registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
            self.operation_control.clone(),
            request,
            prompt_options,
            child_delegation_depth,
            delegation_lineage_for_request(self.options.delegation_lineage(), request),
            self.child_capability_snapshot.clone(),
        )
        .await;
        self.pending_delegation_confirmations
            .extend(outcome.pending_confirmations);
        outcome.execution.map(|_| ())
    }

    async fn execute_approved_team_delegation(
        &mut self,
        request: &crate::operations::prompt::context::DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
    ) -> Result<(), CodingSessionError> {
        let outcome = crate::operations::delegation::execution::execute_team(
            &FlowService::new(),
            self.registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
            self.operation_control.clone(),
            request,
            prompt_options,
            child_delegation_depth,
            delegation_lineage_for_request(self.options.delegation_lineage(), request),
            self.child_capability_snapshot.clone(),
        )
        .await;
        self.pending_delegation_confirmations
            .extend(outcome.pending_confirmations);
        outcome.execution.map(|_| ())
    }

    fn finalize_agent_invocation(&mut self) -> Result<(), CodingSessionError> {
        self.child_admission.take();
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
                    self.event_service.emit_diagnostic(
                        Some(self.operation_id.clone()),
                        diagnostic.message.clone(),
                    );
                }
                self.event_service
                    .emit_prompt_completed(self.child_operation_id.clone(), turn_id.clone());
                let draft = EventService::agent_invocation_completed_draft(
                    self.operation_id.clone(),
                    self.child_operation_id.clone(),
                    self.options.profile_id.clone(),
                    final_text.clone(),
                );
                if self.defer_terminal_publication {
                    self.event_service
                        .defer_terminal_draft(self.operation_id.clone(), draft);
                } else {
                    self.event_service
                        .emit_committed_terminal_draft(draft, OperationKind::AgentInvocation);
                }
                Ok(())
            }
            PromptTurnOutcome::Aborted { reason, .. } => {
                let error = CodingSessionError::Cancelled;
                self.failure_error = Some(error.clone());
                self.event_service
                    .emit_prompt_aborted(self.child_operation_id.clone(), reason.clone());
                let draft = EventService::agent_invocation_aborted_draft(
                    self.operation_id.clone(),
                    self.child_operation_id.clone(),
                    self.options.profile_id.clone(),
                    reason.clone(),
                );
                if self.defer_terminal_publication {
                    self.event_service
                        .defer_terminal_draft(self.operation_id.clone(), draft);
                } else {
                    self.event_service
                        .emit_committed_terminal_draft(draft, OperationKind::AgentInvocation);
                }
                Err(error)
            }
            PromptTurnOutcome::Failed { error, .. } => {
                self.failure_error = Some(error.clone());
                self.event_service
                    .emit_prompt_failed(self.child_operation_id.clone(), error.clone());
                let draft = EventService::agent_invocation_failed_draft(
                    self.operation_id.clone(),
                    self.child_operation_id.clone(),
                    self.options.profile_id.clone(),
                    error,
                );
                if self.defer_terminal_publication {
                    self.event_service
                        .defer_terminal_draft(self.operation_id.clone(), draft);
                } else {
                    self.event_service
                        .emit_committed_terminal_draft(draft, OperationKind::AgentInvocation);
                }
                Err(error.clone())
            }
        }
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        self.child_admission.take();
        if self.failure_error.is_none() {
            self.failure_error = Some(error.clone());
            if error == CodingSessionError::Cancelled {
                let draft = EventService::agent_invocation_aborted_draft(
                    self.operation_id.clone(),
                    self.child_operation_id.clone(),
                    self.options.profile_id.clone(),
                    error.to_string(),
                );
                if self.defer_terminal_publication {
                    self.event_service
                        .defer_terminal_draft(self.operation_id.clone(), draft);
                } else {
                    self.event_service
                        .emit_committed_terminal_draft(draft, OperationKind::AgentInvocation);
                }
            } else {
                let draft = EventService::agent_invocation_failed_draft(
                    self.operation_id.clone(),
                    self.child_operation_id.clone(),
                    self.options.profile_id.clone(),
                    &error,
                );
                if self.defer_terminal_publication {
                    self.event_service
                        .defer_terminal_draft(self.operation_id.clone(), draft);
                } else {
                    self.event_service
                        .emit_committed_terminal_draft(draft, OperationKind::AgentInvocation);
                }
            }
        }
        error.to_string()
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    match error {
        FlowError::Cancelled => CodingSessionError::Cancelled,
        error => CodingSessionError::Flow {
            message: error.to_string(),
        },
    }
}
