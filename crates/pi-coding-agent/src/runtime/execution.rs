use tokio::task::JoinHandle;

use super::client::projection::CodingAgentClientConnection;
use super::control::OperationCancellationHandle;
use super::facade::{CodingAgentSession, CodingSessionError};
use super::operation::{Operation, OperationClass, OperationDispatchMode, OperationOutcome};
use super::outcome::{CodingAgentOperation, CodingAgentOperationOutcome};
use super::scheduler::OperationScheduler;
use crate::services::flow::FlowService;

#[derive(Debug)]
#[must_use = "dropping the handle detaches the runtime-owned operation task"]
pub struct CodingAgentOperationTask {
    operation_id: String,
    cancellation: OperationCancellationHandle,
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

    pub(crate) fn bind_control_owner(&self, connection: &CodingAgentClientConnection) {
        connection
            .bind_operation_cancellation(self.operation_id.clone(), self.cancellation.clone());
    }
}

impl CodingAgentSession {
    pub fn submit(
        &mut self,
        operation: CodingAgentOperation,
    ) -> Result<CodingAgentOperationTask, CodingSessionError> {
        self.runtime_host
            .client_projection
            .coordinator
            .ensure_runtime_running()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
            CodingSessionError::UnsupportedCapability {
                capability: "runtime operation submission requires an active Tokio runtime".into(),
            }
        })?;
        let fingerprint = operation.submission_fingerprint();
        let mut operation =
            operation.into_internal(self.runtime_host.default_plugin_load_options.clone());
        let descriptor = operation.descriptor();
        if descriptor.admission_class() != OperationClass::NonSessionRoot
            || descriptor.dispatch_mode != OperationDispatchMode::Async
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
            self.runtime_host
                .runtime_service
                .install_provider_runtime(runtime);
        }

        let mut submission = self.consume_submission_lease(descriptor, fingerprint.as_ref());
        let admission = self.resolve_operation_admission(&operation)?;
        let operation_permit = OperationScheduler::admit(
            &self.runtime_host.operation_supervisor.control,
            &admission,
            OperationDispatchMode::Async,
        )
        .map_err(|rejection| rejection.into_error())?;
        if let Some(guard) = submission.as_mut() {
            guard.commit_execution(operation_permit.execution())?;
        }

        let execution = operation_permit.execution().clone();
        let snapshot = execution.capability_snapshot.clone();
        let operation_id = execution.operation_id.clone();
        let operation_cancellation = operation_permit.cancellation_token();
        let cancellation_handle = operation_permit
            .cancellation_handle()
            .expect("runtime-owned roots must have cancellation authority");
        let execution_cancellation_handle = cancellation_handle.clone();
        if let (Some(submission), Some(cancellation)) =
            (submission.as_ref(), Some(cancellation_handle.clone()))
        {
            self.runtime_host
                .client_projection
                .coordinator
                .bind_operation_cancellation(
                    submission.handle.clone(),
                    operation_id.clone(),
                    cancellation,
                );
        }
        let prompt_control_receiver = if matches!(operation, Operation::AgentInvocation(_)) {
            let receiver = self
                .runtime_host
                .operation_supervisor
                .control
                .take_prompt_control_receiver();
            self.runtime_host
                .operation_supervisor
                .control
                .clear_prompt_control_receiver();
            receiver
        } else {
            None
        };
        let profile_registry = self.runtime_host.profile_registry.clone();
        let plugin_service = self.runtime_host.plugin_service.clone();
        let event_service = self.runtime_host.event_hub.service.clone();
        let operation_control = self.runtime_host.operation_supervisor.control.clone();
        let operation_finalizer = self.runtime_host.operation_supervisor.finalizer;

        let task = runtime.spawn(async move {
            let result = match operation {
                Operation::PluginCommand { command_id, args } => {
                    if operation_cancellation
                        .as_ref()
                        .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
                    {
                        Err(CodingSessionError::Cancelled)
                    } else {
                        execution_cancellation_handle.close()?;
                        let result = plugin_service
                            .run_command_with_capabilities(&command_id, args, &snapshot.plugin)
                            .map(OperationOutcome::PluginCommand);
                        if operation_cancellation
                            .as_ref()
                            .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
                        {
                            Err(CodingSessionError::Cancelled)
                        } else {
                            result
                        }
                    }
                }
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
                        operation_cancellation.clone(),
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
                    operation_cancellation.clone(),
                )
                .await
                .map(OperationOutcome::AgentTeam),
                _ => unreachable!("runtime-owned operation class checked before spawn"),
            };
            if let Some(guard) = submission.as_mut() {
                let decision = operation_finalizer.freeze(&execution, &result);
                let commit_result = operation_finalizer.resolve_non_session(&decision)?;
                guard.finish(&decision, &commit_result)?;
            }
            drop(operation_permit);
            result.map(CodingAgentOperationOutcome::from_internal)
        });

        Ok(CodingAgentOperationTask {
            operation_id,
            cancellation: cancellation_handle,
            task,
        })
    }
}
