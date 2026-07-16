use tokio::task::JoinHandle;

use super::facade::{CodingAgentSession, CodingSessionError};
use super::operation::{Operation, OperationClass, OperationDispatchMode, OperationOutcome};
use super::outcome::{CodingAgentOperation, CodingAgentOperationOutcome};
use super::scheduler::OperationScheduler;
use super::submission::submitted_terminal_status;
use crate::services::flow::FlowService;

#[derive(Debug)]
#[must_use = "dropping the handle detaches the runtime-owned operation task"]
pub struct CodingAgentOperationTask {
    operation_id: String,
    task: JoinHandle<Result<CodingAgentOperationOutcome, CodingSessionError>>,
}

impl CodingAgentOperationTask {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub async fn join(self) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
        self.task
            .await
            .map_err(|error| CodingSessionError::Session {
                message: format!("runtime-owned operation task failed: {error}"),
            })?
    }
}

impl CodingAgentSession {
    pub fn submit(
        &mut self,
        operation: CodingAgentOperation,
    ) -> Result<CodingAgentOperationTask, CodingSessionError> {
        self.snapshot_coordinator.ensure_runtime_running()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
            CodingSessionError::UnsupportedCapability {
                capability: "runtime operation submission requires an active Tokio runtime".into(),
            }
        })?;
        let descriptor = operation.descriptor();
        let fingerprint = operation.submission_fingerprint();
        let mut operation = operation.into_internal(self.default_plugin_load_options.clone());
        let metadata = operation.metadata();
        if metadata.class != OperationClass::NonSessionRoot
            || metadata.dispatch_mode != OperationDispatchMode::Async
            || !matches!(
                operation,
                Operation::PluginCommand { .. }
                    | Operation::AgentInvocation(_)
                    | Operation::AgentTeam(_)
            )
        {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "runtime-owned execution accepts supported async non-session roots"
                    .into(),
            });
        }
        if let Some(options) = operation.prompt_options_mut()
            && let Some(runtime) = options.runtime_mut()
        {
            self.runtime_service.install_provider_runtime(runtime);
        }

        let mut submission = self.consume_submission_lease(descriptor, fingerprint.as_ref());
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = OperationScheduler::admit(
            &self.operation_control,
            &admission,
            OperationDispatchMode::Async,
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(guard) = submission.as_mut() {
            guard.commit(operation_permit.capability_snapshot().operation_id.clone())?;
        }

        let snapshot = operation_permit.capability_snapshot().clone();
        let operation_id = snapshot.operation_id.clone();
        let prompt_control_receiver = if matches!(operation, Operation::AgentInvocation(_)) {
            let receiver = self.operation_control.take_prompt_control_receiver();
            self.operation_control.clear_prompt_control_receiver();
            receiver
        } else {
            None
        };
        let profile_registry = self.profile_registry.clone();
        let plugin_service = self.plugin_service.clone();
        let event_service = self.event_service.clone();
        let operation_control = self.operation_control.clone();

        let task = runtime.spawn(async move {
            let result = match operation {
                Operation::PluginCommand { command_id, args } => plugin_service
                    .run_command_with_capabilities(&command_id, args, &snapshot.plugin)
                    .map(OperationOutcome::PluginCommand),
                Operation::AgentInvocation(options) => {
                    let result = crate::operations::agent_invocation::run(
                        options,
                        snapshot.operation_id.clone(),
                        prompt_control_receiver,
                        &profile_registry,
                        &plugin_service,
                        &event_service,
                        &FlowService::new(),
                        &operation_control,
                        snapshot.clone(),
                    )
                    .await;
                    result.map(OperationOutcome::AgentInvocation)
                }
                Operation::AgentTeam(options) => crate::operations::team_invocation::run(
                    options,
                    snapshot.operation_id.clone(),
                    &profile_registry,
                    &plugin_service,
                    &event_service,
                    &FlowService::new(),
                    &operation_control,
                    snapshot.clone(),
                )
                .await
                .map(OperationOutcome::AgentTeam),
                _ => unreachable!("runtime-owned operation class checked before spawn"),
            };
            if let Some(guard) = submission.as_mut() {
                guard.finish(submitted_terminal_status(&result))?;
            }
            drop(operation_permit);
            result.map(CodingAgentOperationOutcome::from_internal)
        });

        Ok(CodingAgentOperationTask { operation_id, task })
    }
}
