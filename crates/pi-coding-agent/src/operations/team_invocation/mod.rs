pub(crate) mod runner;

use crate::profiles::ProfileRegistry;
use crate::runtime::capability::OperationCapabilitySnapshot;
use crate::runtime::control::OperationControl;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::workflow::WorkflowService;
use runner::{AgentTeamContext, AgentTeamOptions, AgentTeamOutcome};
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(
    options: AgentTeamOptions,
    scheduler_parent_operation_id: String,
    profile_registry: &ProfileRegistry,
    event_service: &EventService,
    workflow_service: &WorkflowService,
    operation_control: &OperationControl,
    parent_capability_snapshot: OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
) -> Result<AgentTeamOutcome, CodingSessionError> {
    let mut context = AgentTeamContext::new(
        options,
        profile_registry.clone(),
        event_service.clone(),
        operation_control.clone(),
        scheduler_parent_operation_id,
    )
    .with_deferred_terminal_publication()
    .with_parent_capability_snapshot(parent_capability_snapshot);
    let result = match cancellation {
        Some(cancellation) => {
            workflow_service
                .run_agent_team_with_cancellation(&mut context, cancellation)
                .await
        }
        None => workflow_service.run_agent_team(&mut context).await,
    };
    if let Err(error) = &result {
        context.ensure_failure_terminal_draft(error);
    }
    result
}
