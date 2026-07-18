pub(crate) mod flow;

use crate::profiles::ProfileRegistry;
use crate::runtime::capability::OperationCapabilitySnapshot;
use crate::runtime::control::OperationControl;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::PluginService;
use flow::{AgentTeamContext, AgentTeamOptions, AgentTeamOutcome};
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(
    options: AgentTeamOptions,
    scheduler_parent_operation_id: String,
    profile_registry: &ProfileRegistry,
    plugin_service: &PluginService,
    event_service: &EventService,
    flow_service: &FlowService,
    operation_control: &OperationControl,
    parent_capability_snapshot: OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
) -> Result<AgentTeamOutcome, CodingSessionError> {
    let mut context = AgentTeamContext::new(
        options,
        profile_registry.clone(),
        plugin_service.clone(),
        event_service.clone(),
        operation_control.clone(),
        scheduler_parent_operation_id,
    )
    .with_deferred_terminal_publication()
    .with_parent_capability_snapshot(parent_capability_snapshot);
    match cancellation {
        Some(cancellation) => {
            flow_service
                .run_agent_team_with_cancellation(&mut context, cancellation)
                .await
        }
        None => flow_service.run_agent_team(&mut context).await,
    }
}
