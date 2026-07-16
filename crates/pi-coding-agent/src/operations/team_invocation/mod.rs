pub(crate) mod flow;

use crate::profiles::ProfileRegistry;
use crate::runtime::capability::OperationCapabilitySnapshot;
use crate::runtime::control::OperationControl;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::PluginService;
use flow::{AgentTeamContext, AgentTeamOptions, AgentTeamOutcome};

pub(crate) async fn run(
    options: AgentTeamOptions,
    scheduler_parent_operation_id: String,
    profile_registry: &ProfileRegistry,
    plugin_service: &PluginService,
    event_service: &EventService,
    flow_service: &FlowService,
    operation_control: &OperationControl,
    parent_capability_snapshot: OperationCapabilitySnapshot,
) -> Result<AgentTeamOutcome, CodingSessionError> {
    let mut context = AgentTeamContext::new(
        options,
        profile_registry.clone(),
        plugin_service.clone(),
        event_service.clone(),
        operation_control.clone(),
    )
    .with_parent_capability_snapshot(parent_capability_snapshot)
    .with_scheduler_parent_operation_id(scheduler_parent_operation_id);
    flow_service.run_agent_team(&mut context).await
}
