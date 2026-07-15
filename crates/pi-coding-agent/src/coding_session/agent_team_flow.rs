use std::future::Future;
use std::pin::Pin;

use pi_agent_core::api::{Action, Flow, FlowError, FlowNode, FlowOutcome};
use pi_ai::api::AssistantMessage;

use super::agent_invocation_flow::{AgentInvocationContext, AgentInvocationOptions};
use super::capability_snapshot::{ActorId, OperationCapabilitySnapshot};
use super::delegation::{
    DelegationAuthorizationDecision, DelegationLineageEntry,
    capability_snapshot_for_delegated_profile, delegation_lineage_for_request,
};
use super::event_service::EventService;
use super::flow_service::FlowService;
use super::operation_control::OperationKind;
use super::plugin_service::PluginService;
use super::profiles::{
    AgentProfile, ProfileId, ProfileKind, ProfileRegistry, TeamProfile, TeamSupervisor,
};
use super::prompt::{
    CodingDiagnostic, DelegationRequest, PromptTurnContext, PromptTurnIds, PromptTurnOptions,
    PromptTurnOutcome,
};
use super::scheduler::OperationScheduler;
use super::session_log::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};
use super::{CodingSessionError, PendingDelegationConfirmationState};
use crate::runtime::PromptInvocation;

const DEFAULT_ACTION: &str = "default";

pub(crate) const AGENT_TEAM_NODE_IDS: &[&str] = &[
    "start_team",
    "plan_subtasks",
    "run_member_agent",
    "collect_member_result",
    "merge_or_reject_result",
    "finalize_team",
];

const AGENT_TEAM_NODE_SPECS: &[AgentTeamNodeSpec] = &[
    AgentTeamNodeSpec {
        id: "start_team",
        name: "StartTeam",
        kind: AgentTeamNodeKind::StartTeam,
    },
    AgentTeamNodeSpec {
        id: "plan_subtasks",
        name: "PlanSubtasks",
        kind: AgentTeamNodeKind::PlanSubtasks,
    },
    AgentTeamNodeSpec {
        id: "run_member_agent",
        name: "RunMemberAgent",
        kind: AgentTeamNodeKind::RunMemberAgent,
    },
    AgentTeamNodeSpec {
        id: "collect_member_result",
        name: "CollectMemberResult",
        kind: AgentTeamNodeKind::CollectMemberResult,
    },
    AgentTeamNodeSpec {
        id: "merge_or_reject_result",
        name: "MergeOrRejectResult",
        kind: AgentTeamNodeKind::MergeOrRejectResult,
    },
    AgentTeamNodeSpec {
        id: "finalize_team",
        name: "FinalizeTeam",
        kind: AgentTeamNodeKind::FinalizeTeam,
    },
];

#[derive(Debug, Clone)]
pub struct AgentTeamOptions {
    team_id: ProfileId,
    task: String,
    prompt_options: PromptTurnOptions,
    delegation_depth: usize,
    delegation_lineage: Vec<DelegationLineageEntry>,
}

impl AgentTeamOptions {
    pub fn new(
        team_id: impl Into<ProfileId>,
        task: impl Into<String>,
        prompt_options: PromptTurnOptions,
    ) -> Self {
        Self {
            team_id: team_id.into(),
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

    pub fn team_id(&self) -> &ProfileId {
        &self.team_id
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
pub struct AgentTeamMemberOutcome {
    pub profile_id: ProfileId,
    pub operation_id: String,
    pub turn_id: String,
    pub final_text: String,
    pub final_message: AssistantMessage,
    pub diagnostics: Vec<CodingDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentTeamOutcome {
    pub operation_id: String,
    pub team_id: ProfileId,
    pub final_text: String,
    pub member_results: Vec<AgentTeamMemberOutcome>,
    pub supervisor_result: Option<Box<AgentTeamMemberOutcome>>,
    pub diagnostics: Vec<CodingDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentTeamNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: AgentTeamNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentTeamNodeKind {
    StartTeam,
    PlanSubtasks,
    RunMemberAgent,
    CollectMemberResult,
    MergeOrRejectResult,
    FinalizeTeam,
}

pub(crate) struct AgentTeamFlow {
    flow: Flow<AgentTeamContext>,
}

impl AgentTeamFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(AGENT_TEAM_NODE_IDS[0]).map_err(flow_error)?;
        for spec in AGENT_TEAM_NODE_SPECS {
            flow.add_node(spec.id, AgentTeamNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in AGENT_TEAM_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    #[cfg(test)]
    pub(crate) fn node_ids() -> &'static [&'static str] {
        AGENT_TEAM_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut AgentTeamContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct AgentTeamNode {
    name: &'static str,
    kind: AgentTeamNodeKind,
}

impl AgentTeamNode {
    fn new(name: &'static str, kind: AgentTeamNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<AgentTeamContext> for AgentTeamNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut AgentTeamContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                AgentTeamNodeKind::StartTeam => ctx.start_team(),
                AgentTeamNodeKind::PlanSubtasks => ctx.plan_subtasks(),
                AgentTeamNodeKind::RunMemberAgent => ctx.run_member_agents().await,
                AgentTeamNodeKind::CollectMemberResult => ctx.collect_member_result(),
                AgentTeamNodeKind::MergeOrRejectResult => ctx.merge_or_reject_result().await,
                AgentTeamNodeKind::FinalizeTeam => ctx.finalize_team(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

pub(crate) struct AgentTeamContext {
    options: AgentTeamOptions,
    registry: ProfileRegistry,
    plugin_service: PluginService,
    event_service: EventService,
    operation_id: String,
    team: Option<TeamProfile>,
    member_profiles: Vec<AgentProfile>,
    supervisor_profile: Option<AgentProfile>,
    member_results: Vec<AgentTeamMemberOutcome>,
    supervisor_result: Option<AgentTeamMemberOutcome>,
    final_text: Option<String>,
    parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
    scheduler_parent_operation_id: Option<String>,
    child_capability_snapshot: Option<OperationCapabilitySnapshot>,
    pending_delegation_confirmations: Vec<PendingDelegationConfirmationState>,
    failure_error: Option<CodingSessionError>,
}

impl AgentTeamContext {
    pub(crate) fn new(
        options: AgentTeamOptions,
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
            team: None,
            member_profiles: Vec::new(),
            supervisor_profile: None,
            member_results: Vec::new(),
            supervisor_result: None,
            final_text: None,
            parent_capability_snapshot: None,
            scheduler_parent_operation_id: None,
            child_capability_snapshot: None,
            pending_delegation_confirmations: Vec::new(),
            failure_error: None,
        }
    }

    pub(crate) fn with_parent_capability_snapshot(
        mut self,
        snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        self.parent_capability_snapshot = Some(snapshot);
        self
    }

    pub(crate) fn with_scheduler_parent_operation_id(mut self, operation_id: String) -> Self {
        self.scheduler_parent_operation_id = Some(operation_id);
        self
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn take_pending_delegation_confirmations(
        &mut self,
    ) -> Vec<PendingDelegationConfirmationState> {
        std::mem::take(&mut self.pending_delegation_confirmations)
    }

    pub(crate) fn finish_success(&self) -> Result<AgentTeamOutcome, CodingSessionError> {
        Ok(AgentTeamOutcome {
            operation_id: self.operation_id.clone(),
            team_id: self.options.team_id.clone(),
            final_text: self
                .final_text
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "agent team completed without final text".into(),
                })?,
            member_results: self.member_results.clone(),
            supervisor_result: self.supervisor_result.clone().map(Box::new),
            diagnostics: self.all_diagnostics(),
        })
    }

    fn start_team(&mut self) -> Result<(), CodingSessionError> {
        if self.options.task.trim().is_empty() {
            return Err(CodingSessionError::Input {
                message: "agent team invocation requires a non-empty task".into(),
            });
        }
        self.event_service.emit_agent_team_started(
            self.operation_id.clone(),
            self.options.team_id.clone(),
            self.options.task.clone(),
        );
        Ok(())
    }

    fn plan_subtasks(&mut self) -> Result<(), CodingSessionError> {
        let team = self
            .registry
            .team(self.options.team_id.as_str())
            .cloned()
            .ok_or_else(|| CodingSessionError::Input {
                message: format!("Unknown team profile: {}", self.options.team_id),
            })?;
        if team.members.is_empty() {
            return Err(CodingSessionError::Input {
                message: format!("Team profile {} has no members", team.id),
            });
        }

        let mut member_profiles = Vec::new();
        for member_id in &team.members {
            let profile = self
                .registry
                .agent(member_id.as_str())
                .cloned()
                .ok_or_else(|| CodingSessionError::Input {
                    message: format!("Unknown team member agent profile: {member_id}"),
                })?;
            member_profiles.push(profile);
        }

        let supervisor_profile = match &team.supervisor {
            TeamSupervisor::Deterministic => None,
            TeamSupervisor::Agent(profile_id) => Some(
                self.registry
                    .agent(profile_id.as_str())
                    .cloned()
                    .ok_or_else(|| CodingSessionError::Input {
                        message: format!("Unknown team supervisor agent profile: {profile_id}"),
                    })?,
            ),
        };

        self.team = Some(team);
        self.member_profiles = member_profiles;
        self.supervisor_profile = supervisor_profile;
        Ok(())
    }

    async fn run_member_agents(&mut self) -> Result<(), CodingSessionError> {
        let members = self.member_profiles.clone();
        let task = self.options.task.clone();
        for profile in members {
            let result = self.run_profile_child(&profile, task.clone()).await?;
            self.member_results.push(result);
        }
        Ok(())
    }

    fn collect_member_result(&mut self) -> Result<(), CodingSessionError> {
        if self.member_results.len() != self.member_profiles.len() {
            return Err(CodingSessionError::Session {
                message: "agent team member result collection is incomplete".into(),
            });
        }
        Ok(())
    }

    async fn merge_or_reject_result(&mut self) -> Result<(), CodingSessionError> {
        if let Some(supervisor) = self.supervisor_profile.clone() {
            let prompt = self.supervisor_prompt();
            let result = self.run_profile_child(&supervisor, prompt).await?;
            self.final_text = Some(result.final_text.clone());
            self.supervisor_result = Some(result);
        } else {
            self.final_text = Some(self.deterministic_final_text());
        }
        Ok(())
    }

    fn finalize_team(&mut self) -> Result<(), CodingSessionError> {
        let final_text = self
            .final_text
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "agent team cannot finalize without final text".into(),
            })?;
        for diagnostic in self.all_diagnostics() {
            self.event_service
                .emit_diagnostic(Some(self.operation_id.clone()), diagnostic.message);
        }
        self.event_service.emit_agent_team_completed(
            self.operation_id.clone(),
            self.options.team_id.clone(),
            final_text,
        );
        Ok(())
    }

    async fn run_profile_child(
        &mut self,
        profile: &AgentProfile,
        prompt_text: String,
    ) -> Result<AgentTeamMemberOutcome, CodingSessionError> {
        let mut ids = SystemIdGenerator;
        let child_operation_id = ids.next_operation_id();
        let turn_id = ids.next_turn_id();
        self.event_service.emit_agent_team_member_started(
            self.operation_id.clone(),
            child_operation_id.clone(),
            self.options.team_id.clone(),
            profile.id.clone(),
            prompt_text.clone(),
        );

        let mut prompt_options = self.options.prompt_options.clone();
        prompt_options.set_invocation(PromptInvocation::Text(prompt_text));
        if self.options.delegation_depth > 0 {
            prompt_options.apply_delegated_agent_profile(profile, Vec::new())?;
        } else {
            prompt_options.apply_agent_profile(profile, Vec::new())?;
        }
        if prompt_options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "agent team options do not include a runtime snapshot".into(),
            });
        }

        let mut child_context = PromptTurnContext::new(
            PromptTurnIds::new(child_operation_id.clone(), turn_id),
            prompt_options,
        );
        child_context.set_plugin_service(self.plugin_service.clone());
        child_context
            .set_non_persistent_session(format!("agent_team_{}", child_operation_id), Vec::new());
        child_context.enable_live_events(self.event_service.clone());
        let mut capability_snapshot = match self.parent_capability_snapshot.as_ref() {
            Some(parent) => capability_snapshot_for_delegated_profile(
                parent,
                child_operation_id.clone(),
                profile,
                ActorId::ChildOperation(parent.operation_id.clone()),
            ),
            None => OperationCapabilitySnapshot::permissive(child_operation_id.clone()),
        };
        if self.parent_capability_snapshot.is_none() {
            capability_snapshot.actor = ActorId::ChildOperation(
                self.scheduler_parent_operation_id
                    .clone()
                    .unwrap_or_else(|| self.operation_id.clone()),
            );
        }
        let _child_admission =
            OperationScheduler::admit_child(OperationKind::AgentTeam, capability_snapshot.clone())
                .map_err(|rejection| rejection.into_error())?;
        child_context.set_capability_snapshot(capability_snapshot);

        let mut finished_outcome = None;
        let child_delegations = match FlowService::new()
            .run_prompt_subflow_for_agent_team_member(&mut child_context)
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
        };
        let outcome = if let Some((decisions, prompt_options, runtime_id)) = child_delegations {
            self.child_capability_snapshot = child_context.capability_snapshot().cloned();
            if let Err(error) = self
                .execute_authorized_delegations(&decisions, prompt_options)
                .await
            {
                self.event_service.emit_diagnostic(
                    Some(child_operation_id.clone()),
                    format!("delegation execution failed: {error}"),
                );
            }
            child_context.finish_success(runtime_id, None)?
        } else {
            finished_outcome.ok_or_else(|| CodingSessionError::Session {
                message: "agent team child completed without prompt outcome".into(),
            })?
        };

        match outcome {
            PromptTurnOutcome::Success {
                turn_id,
                final_text,
                final_message,
                diagnostics,
                ..
            } => {
                self.event_service
                    .emit_prompt_completed(child_operation_id.clone(), turn_id.clone());
                self.event_service.emit_agent_team_member_completed(
                    self.operation_id.clone(),
                    child_operation_id.clone(),
                    self.options.team_id.clone(),
                    profile.id.clone(),
                    final_text.clone(),
                );
                Ok(AgentTeamMemberOutcome {
                    profile_id: profile.id.clone(),
                    operation_id: child_operation_id,
                    turn_id,
                    final_text,
                    final_message,
                    diagnostics,
                })
            }
            PromptTurnOutcome::Aborted { reason, .. } => {
                self.event_service
                    .emit_prompt_aborted(child_operation_id, reason.clone());
                Err(CodingSessionError::Session {
                    message: format!("agent team child aborted: {reason}"),
                })
            }
            PromptTurnOutcome::Failed { error, .. } => {
                self.event_service
                    .emit_prompt_failed(child_operation_id, error.clone());
                Err(error)
            }
        }
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
        request: &DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
    ) -> Result<(), CodingSessionError> {
        let mut context = AgentInvocationContext::new(
            AgentInvocationOptions::new(
                request.target_id.clone(),
                request.task.clone(),
                prompt_options,
            )
            .with_delegation_depth(child_delegation_depth)
            .with_delegation_lineage(delegation_lineage_for_request(
                self.options.delegation_lineage(),
                request,
            )),
            self.registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        );
        if let Some(parent_snapshot) = self.child_capability_snapshot.clone() {
            context = context.with_parent_capability_snapshot(parent_snapshot);
        }
        let child_operation_id = context.operation_id().to_owned();
        self.event_service
            .emit_delegation_started(request, child_operation_id.clone());
        let outcome = FlowService::new()
            .run_agent_invocation_subflow(&mut context)
            .await;
        self.pending_delegation_confirmations
            .extend(context.take_pending_delegation_confirmations());
        match outcome {
            Ok(outcome) => {
                self.event_service.emit_delegation_completed(
                    request,
                    child_operation_id,
                    outcome.final_text,
                );
                Ok(())
            }
            Err(error) => {
                self.event_service.emit_delegation_failed(
                    request,
                    child_operation_id,
                    error.clone(),
                );
                Err(error)
            }
        }
    }

    async fn execute_approved_team_delegation(
        &mut self,
        request: &DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
    ) -> Result<(), CodingSessionError> {
        let mut context = AgentTeamContext::new(
            AgentTeamOptions::new(
                request.target_id.clone(),
                request.task.clone(),
                prompt_options,
            )
            .with_delegation_depth(child_delegation_depth)
            .with_delegation_lineage(delegation_lineage_for_request(
                self.options.delegation_lineage(),
                request,
            )),
            self.registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        );
        if let Some(parent_snapshot) = self.child_capability_snapshot.clone() {
            context = context.with_parent_capability_snapshot(parent_snapshot);
        }
        let child_operation_id = context.operation_id().to_owned();
        self.event_service
            .emit_delegation_started(request, child_operation_id.clone());
        let outcome = FlowService::new()
            .run_agent_team_subflow(&mut context)
            .await;
        self.pending_delegation_confirmations
            .extend(context.take_pending_delegation_confirmations());
        match outcome {
            Ok(outcome) => {
                self.event_service.emit_delegation_completed(
                    request,
                    child_operation_id,
                    outcome.final_text,
                );
                Ok(())
            }
            Err(error) => {
                self.event_service.emit_delegation_failed(
                    request,
                    child_operation_id,
                    error.clone(),
                );
                Err(error)
            }
        }
    }

    fn deterministic_final_text(&self) -> String {
        let mut lines = vec![format!("Team {} completed.", self.options.team_id)];
        for result in &self.member_results {
            lines.push(String::new());
            lines.push(format!("[{}]", result.profile_id));
            lines.push(result.final_text.clone());
        }
        lines.join("\n")
    }

    fn supervisor_prompt(&self) -> String {
        let mut lines = vec![
            "You are supervising an agent team.".to_string(),
            String::new(),
            format!("Task: {}", self.options.task),
            String::new(),
            "Member results:".to_string(),
        ];
        for result in &self.member_results {
            lines.push(format!("- {}: {}", result.profile_id, result.final_text));
        }
        lines.push(String::new());
        lines.push("Produce the final team response.".to_string());
        lines.join("\n")
    }

    fn all_diagnostics(&self) -> Vec<CodingDiagnostic> {
        let mut diagnostics = Vec::new();
        for result in &self.member_results {
            diagnostics.extend(result.diagnostics.clone());
        }
        if let Some(result) = &self.supervisor_result {
            diagnostics.extend(result.diagnostics.clone());
        }
        diagnostics
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        if self.failure_error.is_none() {
            self.failure_error = Some(error.clone());
            self.event_service.emit_agent_team_failed(
                self.operation_id.clone(),
                self.options.team_id.clone(),
                error.clone(),
            );
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

use super::*;

impl CodingAgentSession {
    pub(super) async fn invoke_team_inner(
        &mut self,
        options: AgentTeamOptions,
        scheduler_parent_operation_id: String,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        let mut context = AgentTeamContext::new(
            options,
            self.profile_registry.clone(),
            self.plugin_service.clone(),
            self.event_service.clone(),
        )
        .with_scheduler_parent_operation_id(scheduler_parent_operation_id);
        self.flow_service.run_agent_team(&mut context).await
    }
}
