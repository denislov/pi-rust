use std::future::Future;
use std::pin::Pin;

use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome};
use pi_ai::types::AssistantMessage;

use super::CodingSessionError;
use super::event::CodingAgentEvent;
use super::event_service::EventService;
use super::plugin_service::PluginService;
use super::profiles::{AgentProfile, ProfileId, ProfileRegistry, TeamProfile, TeamSupervisor};
use super::prompt::{
    CodingDiagnostic, PromptTurnContext, PromptTurnIds, PromptTurnOptions, PromptTurnOutcome,
};
use super::prompt_flow::PromptTurnFlow;
use super::session_log::id::{IdGenerator, SystemIdGenerator};
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
        }
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
            failure_error: None,
        }
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
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
        self.event_service.emit(CodingAgentEvent::AgentTeamStarted {
            operation_id: self.operation_id.clone(),
            team_id: self.options.team_id.clone(),
            task: self.options.task.clone(),
        });
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
            self.event_service.emit(CodingAgentEvent::Diagnostic {
                operation_id: Some(self.operation_id.clone()),
                message: diagnostic.message,
            });
        }
        self.event_service
            .emit(CodingAgentEvent::AgentTeamCompleted {
                operation_id: self.operation_id.clone(),
                team_id: self.options.team_id.clone(),
                final_text,
            });
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
        self.event_service
            .emit(CodingAgentEvent::AgentTeamMemberStarted {
                operation_id: self.operation_id.clone(),
                child_operation_id: child_operation_id.clone(),
                team_id: self.options.team_id.clone(),
                profile_id: profile.id.clone(),
                task: prompt_text.clone(),
            });

        let mut prompt_options = self.options.prompt_options.clone();
        prompt_options.set_invocation(PromptInvocation::Text(prompt_text));
        prompt_options.apply_agent_profile(profile, Vec::new())?;
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

        let outcome = match PromptTurnFlow::new()?.run(&mut child_context).await {
            Ok(_) => child_context.finish_success(
                child_context.non_persistent_runtime_id().map(str::to_owned),
                None,
            )?,
            Err(error) => match child_context.abort_reason() {
                Some(reason) => child_context.finish_abort(
                    reason.to_owned(),
                    child_context.non_persistent_runtime_id().map(str::to_owned),
                ),
                None => child_context.finish_failure(error),
            },
        };

        match outcome {
            PromptTurnOutcome::Success {
                turn_id,
                final_text,
                final_message,
                diagnostics,
                ..
            } => {
                self.event_service.emit(CodingAgentEvent::PromptCompleted {
                    operation_id: child_operation_id.clone(),
                    turn_id: turn_id.clone(),
                });
                self.event_service
                    .emit(CodingAgentEvent::AgentTeamMemberCompleted {
                        operation_id: self.operation_id.clone(),
                        child_operation_id: child_operation_id.clone(),
                        team_id: self.options.team_id.clone(),
                        profile_id: profile.id.clone(),
                        final_text: final_text.clone(),
                    });
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
                self.event_service.emit(CodingAgentEvent::PromptAborted {
                    operation_id: child_operation_id,
                    reason: reason.clone(),
                });
                Err(CodingSessionError::Session {
                    message: format!("agent team child aborted: {reason}"),
                })
            }
            PromptTurnOutcome::Failed { error, .. } => {
                self.event_service.emit(CodingAgentEvent::PromptFailed {
                    operation_id: child_operation_id,
                    error: error.clone(),
                });
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
            self.event_service.emit(CodingAgentEvent::AgentTeamFailed {
                operation_id: self.operation_id.clone(),
                team_id: self.options.team_id.clone(),
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
