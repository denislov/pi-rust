pub(crate) mod flow;

use crate::profiles::ProfileRegistry;
use crate::runtime::capability::OperationCapabilitySnapshot;
use crate::runtime::control::{OperationControl, PromptControlReceiver};
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::PluginService;
use flow::{AgentInvocationContext, AgentInvocationOptions, AgentInvocationOutcome};
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(
    options: AgentInvocationOptions,
    scheduler_parent_operation_id: String,
    prompt_control_receiver: Option<PromptControlReceiver>,
    profile_registry: &ProfileRegistry,
    plugin_service: &PluginService,
    event_service: &EventService,
    flow_service: &FlowService,
    operation_control: &OperationControl,
    parent_capability_snapshot: OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
) -> Result<AgentInvocationOutcome, CodingSessionError> {
    let mut context = AgentInvocationContext::new(
        options,
        profile_registry.clone(),
        plugin_service.clone(),
        event_service.clone(),
        operation_control.clone(),
        scheduler_parent_operation_id,
    )
    .with_deferred_terminal_publication()
    .with_parent_capability_snapshot(parent_capability_snapshot);
    if let Some(receiver) = prompt_control_receiver {
        context.set_prompt_control_receiver(receiver);
    }
    let result = match cancellation {
        Some(cancellation) => {
            flow_service
                .run_agent_invocation_with_cancellation(&mut context, cancellation)
                .await
        }
        None => flow_service.run_agent_invocation(&mut context).await,
    };
    if let Err(error) = &result {
        context.ensure_failure_terminal_draft(error);
    }
    result
}
