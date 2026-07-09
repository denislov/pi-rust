use super::CodingSessionError;
use super::agent_invocation_flow::{AgentInvocationContext, AgentInvocationOptions};
use super::agent_team_flow::{AgentTeamContext, AgentTeamOptions};
use super::delegation::{DelegationLineageEntry, PendingDelegationConfirmationState};
use super::event_service::EventService;
use super::flow_service::FlowService;
use super::plugin_service::PluginService;
use super::profiles::ProfileRegistry;
use super::prompt::{DelegationRequest, PromptTurnOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovedDelegationExecution {
    pub(crate) child_operation_id: String,
    pub(crate) final_text: String,
}

#[derive(Debug)]
pub(crate) struct ApprovedDelegationExecutionOutcome {
    pub(crate) execution: Result<ApprovedDelegationExecution, CodingSessionError>,
    pub(crate) pending_confirmations: Vec<PendingDelegationConfirmationState>,
}

#[derive(Debug, Default)]
pub(crate) struct DelegationExecutionService;

impl DelegationExecutionService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn execute_agent(
        &self,
        flow_service: &FlowService,
        profile_registry: ProfileRegistry,
        plugin_service: PluginService,
        event_service: EventService,
        request: &DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
        delegation_lineage: Vec<DelegationLineageEntry>,
    ) -> ApprovedDelegationExecutionOutcome {
        let mut context = AgentInvocationContext::new(
            AgentInvocationOptions::new(
                request.target_id.clone(),
                request.task.clone(),
                prompt_options,
            )
            .with_delegation_depth(child_delegation_depth)
            .with_delegation_lineage(delegation_lineage),
            profile_registry,
            plugin_service,
            event_service.clone(),
        );
        let child_operation_id = context.operation_id().to_owned();
        event_service.emit_delegation_started(request, child_operation_id.clone());
        let result = flow_service.run_agent_invocation(&mut context).await;
        let pending_confirmations = context.take_pending_delegation_confirmations();
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                event_service.emit_delegation_failed(request, child_operation_id, error.clone());
                return ApprovedDelegationExecutionOutcome {
                    execution: Err(error),
                    pending_confirmations,
                };
            }
        };
        let final_text = outcome.final_text;
        event_service.emit_delegation_completed(
            request,
            child_operation_id.clone(),
            final_text.clone(),
        );
        ApprovedDelegationExecutionOutcome {
            execution: Ok(ApprovedDelegationExecution {
                child_operation_id,
                final_text,
            }),
            pending_confirmations,
        }
    }

    pub(crate) async fn execute_team(
        &self,
        flow_service: &FlowService,
        profile_registry: ProfileRegistry,
        plugin_service: PluginService,
        event_service: EventService,
        request: &DelegationRequest,
        prompt_options: PromptTurnOptions,
        child_delegation_depth: usize,
        delegation_lineage: Vec<DelegationLineageEntry>,
    ) -> ApprovedDelegationExecutionOutcome {
        let mut context = AgentTeamContext::new(
            AgentTeamOptions::new(
                request.target_id.clone(),
                request.task.clone(),
                prompt_options,
            )
            .with_delegation_depth(child_delegation_depth)
            .with_delegation_lineage(delegation_lineage),
            profile_registry,
            plugin_service,
            event_service.clone(),
        );
        let child_operation_id = context.operation_id().to_owned();
        event_service.emit_delegation_started(request, child_operation_id.clone());
        let result = flow_service.run_agent_team(&mut context).await;
        let pending_confirmations = context.take_pending_delegation_confirmations();
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                event_service.emit_delegation_failed(request, child_operation_id, error.clone());
                return ApprovedDelegationExecutionOutcome {
                    execution: Err(error),
                    pending_confirmations,
                };
            }
        };
        let final_text = outcome.final_text;
        event_service.emit_delegation_completed(
            request,
            child_operation_id.clone(),
            final_text.clone(),
        );
        ApprovedDelegationExecutionOutcome {
            execution: Ok(ApprovedDelegationExecution {
                child_operation_id,
                final_text,
            }),
            pending_confirmations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::capability_snapshot::{ActorId, OperationCapabilitySnapshot};
    use super::super::delegation::capability_snapshot_for_delegated_profile;
    use super::super::profiles::{
        AgentProfile, DelegationPolicy, ProfileId, ProfileSource, SupervisionPolicy,
    };

    #[tokio::test]
    async fn delegated_operation_receives_released_tool_capabilities_only() {
        let parent = OperationCapabilitySnapshot::test_with_tools("op_parent", ["read", "bash"]);
        let target_profile = AgentProfile {
            schema_version: 1,
            id: ProfileId::from("coder"),
            display_name: "Coder".into(),
            description: None,
            model: None,
            system_prompt: None,
            tools: vec!["read".into()],
            skills: Vec::new(),
            supervision: SupervisionPolicy::Session,
            delegation: DelegationPolicy::default(),
            source: ProfileSource::BuiltIn,
            path: None,
        };

        let child = capability_snapshot_for_delegated_profile(
            &parent,
            "op_child",
            &target_profile,
            ActorId::ChildOperation("op_parent".into()),
        );

        assert!(child.tools.allows("read"));
        assert!(!child.tools.allows("bash"));
        assert_eq!(child.generation, parent.generation);
    }
}
