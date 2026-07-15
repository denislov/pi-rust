use super::CodingSessionError;
use super::agent_invocation_flow::{AgentInvocationContext, AgentInvocationOptions};
use super::agent_team_flow::{AgentTeamContext, AgentTeamOptions};
use super::capability_snapshot::OperationCapabilitySnapshot;
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
        parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
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
        if let Some(snapshot) = parent_capability_snapshot {
            context = context.with_parent_capability_snapshot(snapshot);
        }
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
        parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
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
        if let Some(snapshot) = parent_capability_snapshot {
            context = context.with_parent_capability_snapshot(snapshot);
        }
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
    use super::super::capability_snapshot::{
        ActorId, ModelCapability, OperationCapabilitySnapshot,
    };
    use super::super::delegation::capability_snapshot_for_delegated_profile;
    use super::super::profiles::{
        AgentProfile, DelegationPolicy, ProfileId, ProfileSource, SupervisionPolicy,
    };

    #[test]
    fn delegated_operation_receives_released_tool_capabilities_only() {
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
        assert_eq!(child.actor, ActorId::ChildOperation("op_parent".into()));
        assert_eq!(
            child.model,
            Some(ModelCapability {
                profile_id: Some(ProfileId::from("coder"))
            })
        );
        assert_eq!(child.filesystem, parent.filesystem);
        assert!(child.shell.is_none());
        assert!(child.session_read.is_none());
        assert!(child.session_write.is_none());
        assert_eq!(child.plugin, parent.plugin);
    }

    #[test]
    fn delegated_operation_releases_delegation_tools_granted_by_policy() {
        let parent =
            OperationCapabilitySnapshot::test_with_tools("op_parent", ["read", "delegate_agent"]);
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
            delegation: DelegationPolicy {
                allow_delegate_agent: true,
                max_depth: 1,
                ..DelegationPolicy::default()
            },
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
        assert!(child.tools.allows("delegate_agent"));
        assert!(!child.tools.allows("delegate_team"));
        assert_eq!(child.generation, parent.generation);
    }

    #[test]
    fn delegated_operation_from_permissive_parent_releases_all_profile_tools() {
        let parent = OperationCapabilitySnapshot::permissive("op_parent");
        let target_profile = AgentProfile {
            schema_version: 1,
            id: ProfileId::from("coder"),
            display_name: "Coder".into(),
            description: None,
            model: None,
            system_prompt: None,
            tools: vec!["read".into(), "edit".into()],
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
        assert!(child.tools.allows("edit"));
        assert_eq!(child.generation, parent.generation);
    }

    #[test]
    fn delegated_operation_does_not_release_filesystem_without_filesystem_tools() {
        let parent = OperationCapabilitySnapshot::test_with_tools("op_parent", ["bash"]);
        let target_profile = AgentProfile {
            schema_version: 1,
            id: ProfileId::from("coder"),
            display_name: "Coder".into(),
            description: None,
            model: None,
            system_prompt: None,
            tools: vec!["bash".into()],
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

        assert!(child.tools.allows("bash"));
        assert!(child.filesystem.is_none());
        assert!(child.shell.is_some());
    }
}

use super::*;

impl CodingAgentSession {
    pub(super) async fn approve_delegation_confirmation_inner(
        &mut self,
        operation_id: String,
        tool_call_id: String,
        now: String,
        parent_capability_snapshot: OperationCapabilitySnapshot,
    ) -> Result<(), CodingSessionError> {
        let mut ids = SystemIdGenerator;
        let mut pending = self.delegation_confirmation_service.approve_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            operation_id.as_str(),
            tool_call_id.as_str(),
            &now,
            ids.next_operation_id(),
        )?;
        if let Some(runtime) = pending.prompt_options.runtime_mut() {
            self.runtime_service.install_provider_runtime(runtime);
        }
        let outcome = match pending.request.target_kind {
            ProfileKind::Agent => {
                self.delegation_execution_service
                    .execute_agent(
                        &self.flow_service,
                        self.profile_registry.clone(),
                        self.plugin_service.clone(),
                        self.event_service.clone(),
                        &pending.request,
                        pending.prompt_options,
                        pending.child_delegation_depth,
                        pending.delegation_lineage,
                        Some(parent_capability_snapshot.clone()),
                    )
                    .await
            }
            ProfileKind::Team => {
                self.delegation_execution_service
                    .execute_team(
                        &self.flow_service,
                        self.profile_registry.clone(),
                        self.plugin_service.clone(),
                        self.event_service.clone(),
                        &pending.request,
                        pending.prompt_options,
                        pending.child_delegation_depth,
                        pending.delegation_lineage,
                        Some(parent_capability_snapshot),
                    )
                    .await
            }
        };
        self.delegation_confirmation_service.adopt_pending(
            &mut self.persistence,
            &mut self.pending_delegation_confirmations,
            &self.event_service,
            outcome.pending_confirmations,
        )?;
        outcome.execution.map(|_| ())
    }
}
