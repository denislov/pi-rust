use super::{
    DelegationLineageEntry, PendingDelegationConfirmationState,
    capability_snapshot_for_child_operation,
};
use crate::operations::agent_invocation::runner::{AgentInvocationContext, AgentInvocationOptions};
use crate::operations::prompt::context::{DelegationRequest, PromptTurnOptions};
use crate::operations::team_invocation::runner::{AgentTeamContext, AgentTeamOptions};
use crate::profiles::{ProfileKind, ProfileRegistry};
use crate::runtime::capability::{OperationCapabilitySnapshot, SessionWriteCapability};
use crate::runtime::control::{OperationControl, OperationKind};
use crate::runtime::facade::CodingSessionError;
use crate::runtime::scheduler::OperationScheduler;
use crate::runtime::session_coordinator::{
    SessionCoordinator, SessionWriterCommand, SessionWriterReply,
};
use crate::services::event::EventService;
use crate::services::plugin::PluginService;
use crate::services::runtime::RuntimeService;
use crate::services::workflow::WorkflowService;

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

pub(crate) async fn execute_agent(
    workflow_service: &WorkflowService,
    profile_registry: ProfileRegistry,
    plugin_service: PluginService,
    event_service: EventService,
    operation_control: OperationControl,
    request: &DelegationRequest,
    prompt_options: PromptTurnOptions,
    child_delegation_depth: usize,
    delegation_lineage: Vec<DelegationLineageEntry>,
    parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
) -> ApprovedDelegationExecutionOutcome {
    let child_operation_id = OperationScheduler::allocate_child_operation_id();
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
        operation_control.clone(),
        child_operation_id.clone(),
    );
    let Some(parent_capability_snapshot) = parent_capability_snapshot else {
        return ApprovedDelegationExecutionOutcome {
            execution: Err(CodingSessionError::UnsupportedCapability {
                capability: "delegated agent execution requires a parent capability snapshot"
                    .into(),
            }),
            pending_confirmations: Vec::new(),
        };
    };
    let child_snapshot = capability_snapshot_for_child_operation(
        &parent_capability_snapshot,
        child_operation_id.clone(),
    );
    let child_admission = match OperationScheduler::admit_child(
        &operation_control,
        OperationKind::AgentInvocation,
        child_snapshot.clone(),
    ) {
        Ok(admission) => admission,
        Err(rejection) => {
            return ApprovedDelegationExecutionOutcome {
                execution: Err(rejection.into_error()),
                pending_confirmations: Vec::new(),
            };
        }
    };
    context = context.with_parent_capability_snapshot(child_snapshot);
    event_service.emit_delegation_started(request, child_operation_id.clone());
    let result = match child_admission.cancellation_token() {
        Some(cancellation) => {
            Box::pin(
                workflow_service.run_agent_invocation_with_cancellation(&mut context, cancellation),
            )
            .await
        }
        None => Box::pin(workflow_service.run_agent_invocation(&mut context)).await,
    };
    let pending_confirmations = context.take_pending_delegation_confirmations();
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            event_service.emit_delegation_failed(request, child_operation_id, error.clone());
            drop(child_admission);
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
    drop(child_admission);
    ApprovedDelegationExecutionOutcome {
        execution: Ok(ApprovedDelegationExecution {
            child_operation_id,
            final_text,
        }),
        pending_confirmations,
    }
}

pub(crate) async fn execute_team(
    workflow_service: &WorkflowService,
    profile_registry: ProfileRegistry,
    plugin_service: PluginService,
    event_service: EventService,
    operation_control: OperationControl,
    request: &DelegationRequest,
    prompt_options: PromptTurnOptions,
    child_delegation_depth: usize,
    delegation_lineage: Vec<DelegationLineageEntry>,
    parent_capability_snapshot: Option<OperationCapabilitySnapshot>,
) -> ApprovedDelegationExecutionOutcome {
    let child_operation_id = OperationScheduler::allocate_child_operation_id();
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
        operation_control.clone(),
        child_operation_id.clone(),
    );
    let Some(parent_capability_snapshot) = parent_capability_snapshot else {
        return ApprovedDelegationExecutionOutcome {
            execution: Err(CodingSessionError::UnsupportedCapability {
                capability: "delegated team execution requires a parent capability snapshot".into(),
            }),
            pending_confirmations: Vec::new(),
        };
    };
    let child_snapshot = capability_snapshot_for_child_operation(
        &parent_capability_snapshot,
        child_operation_id.clone(),
    );
    let child_admission = match OperationScheduler::admit_child(
        &operation_control,
        OperationKind::AgentTeam,
        child_snapshot.clone(),
    ) {
        Ok(admission) => admission,
        Err(rejection) => {
            return ApprovedDelegationExecutionOutcome {
                execution: Err(rejection.into_error()),
                pending_confirmations: Vec::new(),
            };
        }
    };
    context = context.with_parent_capability_snapshot(child_snapshot);
    event_service.emit_delegation_started(request, child_operation_id.clone());
    let result = match child_admission.cancellation_token() {
        Some(cancellation) => {
            Box::pin(workflow_service.run_agent_team_with_cancellation(&mut context, cancellation))
                .await
        }
        None => Box::pin(workflow_service.run_agent_team(&mut context)).await,
    };
    let pending_confirmations = context.take_pending_delegation_confirmations();
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            event_service.emit_delegation_failed(request, child_operation_id, error.clone());
            drop(child_admission);
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
    drop(child_admission);
    ApprovedDelegationExecutionOutcome {
        execution: Ok(ApprovedDelegationExecution {
            child_operation_id,
            final_text,
        }),
        pending_confirmations,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn approve(
    session_coordinator: &mut SessionCoordinator,
    runtime_service: &RuntimeService,
    workflow_service: &WorkflowService,
    profile_registry: &ProfileRegistry,
    plugin_service: &PluginService,
    event_service: &EventService,
    operation_control: &OperationControl,
    operation_id: String,
    tool_call_id: String,
    now: String,
    parent_capability_snapshot: OperationCapabilitySnapshot,
) -> Result<(), CodingSessionError> {
    SessionWriteCapability::require(parent_capability_snapshot.session_write.as_ref())?;
    let approval_operation_id = parent_capability_snapshot.operation_id.clone();
    let approval_generation = parent_capability_snapshot.generation;
    let reply =
        session_coordinator.execute_writer_command(SessionWriterCommand::approve_delegation(
            approval_operation_id.clone(),
            approval_generation,
            operation_id,
            tool_call_id,
            now,
        ))?;
    let SessionWriterReply::DelegationApproved { pending } = reply else {
        unreachable!("delegation approval writer command returns its typed reply")
    };
    let mut pending = *pending;
    event_service.emit_delegation_approved(&pending.request);
    if let Some(runtime) = pending.prompt_options.runtime_mut() {
        runtime_service.install_provider_runtime(runtime);
    }
    let outcome = match pending.request.target_kind {
        ProfileKind::Agent => {
            execute_agent(
                workflow_service,
                profile_registry.clone(),
                plugin_service.clone(),
                event_service.clone(),
                operation_control.clone(),
                &pending.request,
                pending.prompt_options,
                pending.child_delegation_depth,
                pending.delegation_lineage,
                Some(parent_capability_snapshot.clone()),
            )
            .await
        }
        ProfileKind::Team => {
            execute_team(
                workflow_service,
                profile_registry.clone(),
                plugin_service.clone(),
                event_service.clone(),
                operation_control.clone(),
                &pending.request,
                pending.prompt_options,
                pending.child_delegation_depth,
                pending.delegation_lineage,
                Some(parent_capability_snapshot),
            )
            .await
        }
    };
    let reply =
        session_coordinator.execute_writer_command(SessionWriterCommand::adopt_delegations(
            approval_operation_id,
            approval_generation,
            outcome.pending_confirmations,
        ))?;
    let SessionWriterReply::DelegationsAdopted { diagnostics } = reply else {
        unreachable!("delegation adoption writer command returns its typed reply")
    };
    for diagnostic in diagnostics {
        event_service.emit_diagnostic(diagnostic.operation_id, diagnostic.message);
    }
    outcome.execution.map(|_| ())
}

#[cfg(test)]
#[path = "../../internal_tests/delegation_execution_capabilities.rs"]
mod tests;
