use self::flow::{
    ManualCompactionContext, ManualCompactionOptions, manual_compaction_failed_outcome,
    manual_compaction_success_outcome,
};
use crate::operations::prompt::context::PromptTurnOutcome;
use crate::runtime::capability::{
    OperationCapabilitySnapshot, SessionReadCapability, SessionWriteCapability,
};
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::session::apply_finalized_session_write;
use crate::session::service::SessionService;

pub(crate) mod flow;

pub(crate) async fn run(
    session_service: &mut SessionService,
    flow_service: &FlowService,
    event_service: &EventService,
    options: ManualCompactionOptions,
    snapshot: &OperationCapabilitySnapshot,
) -> Result<PromptTurnOutcome, CodingSessionError> {
    SessionReadCapability::require(snapshot.session_read.as_ref())?;
    SessionWriteCapability::require(snapshot.session_write.as_ref())?;
    let replay = session_service.replay()?;
    let transaction = session_service.begin_manual_compaction_transaction(snapshot);
    let mut context = ManualCompactionContext::new(options, replay, transaction, snapshot.clone());
    let operation_id = context.operation_id().to_owned();
    let turn_id = context.turn_id().to_owned();

    match flow_service.run_manual_compaction(&mut context).await {
        Ok(compaction) => {
            let mut outcome = manual_compaction_success_outcome(
                operation_id.clone(),
                turn_id.clone(),
                session_service.session_id().to_owned(),
                session_service.active_leaf_id().map(str::to_owned),
                &compaction,
            );
            let finalized = session_service.commit_manual_compaction_transaction(
                context.take_transaction(),
                operation_id.clone(),
            )?;
            apply_finalized_session_write(&mut outcome, &finalized);

            event_service.emit_session_write_pending(&finalized);
            event_service.emit_session_compaction_completed(operation_id, turn_id, &compaction);
            event_service.emit_session_write_committed(&finalized);
            event_service.emit_prompt_outcome(&outcome);
            Ok(outcome)
        }
        Err(error) => {
            let mut outcome =
                manual_compaction_failed_outcome(operation_id.clone(), turn_id, error.clone());
            let finalized = session_service.fail_prompt_transaction(
                context.take_transaction(),
                operation_id,
                error.code(),
                error.to_string(),
            )?;
            apply_finalized_session_write(&mut outcome, &finalized);
            event_service.emit_session_write_events(&finalized);
            event_service.emit_prompt_outcome(&outcome);
            Ok(outcome)
        }
    }
}
