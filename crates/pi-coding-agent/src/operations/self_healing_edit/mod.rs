use std::sync::Arc;

use self::flow::{
    ModelSelfHealingEditRepairStrategy, PlannedSelfHealingEditRepairStrategy,
    SelfHealingEditContext, SelfHealingEditOptions, SelfHealingEditOutcome,
    SelfHealingEditRepairStrategy, SelfHealingEditReplacement,
};
use crate::operations::prompt::context::PromptTurnOptions;
use crate::runtime::capability::{
    ModelCapability, OperationCapabilitySnapshot, SessionWriteCapability,
};
use crate::runtime::control::OperationCancellationHandle;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::{EventService, SelfHealingEditEventObserver};
use crate::services::flow::FlowService;
use crate::services::session::{default_cwd, session_cwd};
use crate::session::service::{FinalizedSessionWrite, SessionService};
use tokio_util::sync::CancellationToken;

pub(crate) mod flow;

pub(crate) struct SelfHealingEditExecution {
    pub(crate) result: Result<SelfHealingEditOutcome, CodingSessionError>,
    pub(crate) finalized: FinalizedSessionWrite,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    session_service: &mut SessionService,
    flow_service: &FlowService,
    event_service: EventService,
    path: String,
    replacements: Vec<SelfHealingEditReplacement>,
    check_command: Option<String>,
    repair_attempts: Vec<Vec<SelfHealingEditReplacement>>,
    model_repair_policy: Option<(Arc<dyn SelfHealingEditRepairStrategy>, usize)>,
    snapshot: &OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
    cancellation_handle: Option<OperationCancellationHandle>,
) -> Result<SelfHealingEditExecution, CodingSessionError> {
    SessionWriteCapability::require(snapshot.session_write.as_ref())?;
    let replacement_count = replacements.len();
    let event_path = path.clone();
    let cwd = session_cwd(session_service).unwrap_or_else(default_cwd);
    let mut transaction = session_service.begin_self_healing_edit_transaction();
    let operation_id = transaction.operation_id().to_owned();
    event_service.emit_self_healing_edit_started(
        operation_id.clone(),
        event_path.clone(),
        replacement_count,
    );
    SessionService::record_self_healing_edit_started(
        &mut transaction,
        path.clone(),
        replacement_count,
    )?;
    let mut options =
        SelfHealingEditOptions::new(cwd, path, replacements).with_repair_observer(Arc::new(
            SelfHealingEditEventObserver::new(event_service.clone(), operation_id.clone()),
        ));
    if let Some(command) = check_command {
        options = options.with_check_command(command).with_real_check_runner();
    }
    let repair_attempt_count = repair_attempts.len();
    if repair_attempt_count > 0 {
        options = options
            .with_repair_strategy(Arc::new(PlannedSelfHealingEditRepairStrategy::new(
                repair_attempts,
            )))
            .with_max_repair_attempts(repair_attempt_count);
    } else if let Some((strategy, max_attempts)) = model_repair_policy {
        options = options
            .with_repair_strategy(strategy)
            .with_max_repair_attempts(max_attempts);
    }
    let mut context = SelfHealingEditContext::new(options);
    if let Some(cancellation_handle) = cancellation_handle {
        context.set_cancellation_handle(cancellation_handle);
    }

    let result = match cancellation.as_ref() {
        Some(cancellation) => {
            flow_service
                .run_self_healing_edit_with_cancellation(&mut context, cancellation.clone())
                .await
        }
        None => flow_service.run_self_healing_edit(&mut context).await,
    };
    match result {
        Ok(outcome) => {
            if cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                return Err(CodingSessionError::Cancelled);
            }
            for repair in outcome.repair_attempts.iter() {
                SessionService::record_self_healing_edit_repair_attempted(
                    &mut transaction,
                    &outcome.path,
                    repair,
                )?;
            }
            SessionService::record_self_healing_edit_completed(&mut transaction, &outcome)?;
            event_service.emit_self_healing_edit_completed(operation_id.clone(), &outcome);
            let finalized = session_service
                .commit_self_healing_edit_transaction(Some(transaction), operation_id)?;
            Ok(SelfHealingEditExecution {
                result: Ok(outcome),
                finalized,
            })
        }
        Err(error) => {
            for repair in context.repair_attempts() {
                SessionService::record_self_healing_edit_repair_attempted(
                    &mut transaction,
                    &event_path,
                    repair,
                )?;
            }
            if error == CodingSessionError::Cancelled {
                event_service.emit_self_healing_edit_aborted(
                    operation_id.clone(),
                    event_path.clone(),
                    error.to_string(),
                );
            } else {
                event_service.emit_self_healing_edit_failed(
                    operation_id.clone(),
                    event_path.clone(),
                    &error,
                );
            }
            let finalized = session_service.fail_self_healing_edit_transaction(
                Some(transaction),
                operation_id,
                error.code(),
                error.to_string(),
            )?;
            Ok(SelfHealingEditExecution {
                result: Err(error),
                finalized,
            })
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn model_repair_policy(
    prompt_options: PromptTurnOptions,
    max_attempts: usize,
    snapshot: &OperationCapabilitySnapshot,
) -> Result<(Arc<dyn SelfHealingEditRepairStrategy>, usize), CodingSessionError> {
    let runtime = prompt_options
        .runtime()
        .cloned()
        .ok_or_else(|| CodingSessionError::Config {
            message: "self-healing edit model repair options do not include a runtime snapshot"
                .into(),
        })?;
    let model_capability = ModelCapability::require(snapshot.model.as_ref(), runtime.profile_id())?;
    Ok((
        Arc::new(ModelSelfHealingEditRepairStrategy::new(
            runtime,
            model_capability.clone(),
        )),
        max_attempts,
    ))
}
