use super::capability_snapshot::{
    OperationCapabilitySnapshot, SessionReadCapability, SessionWriteCapability,
};
use super::event_service::EventService;
use super::flow_service::FlowService;
use super::manual_compaction_flow::{
    ManualCompactionContext, ManualCompactionOptions, manual_compaction_failed_outcome,
    manual_compaction_success_outcome,
};
use super::prompt::PromptTurnOutcome;
use super::session_service::SessionService;
use super::{CodingSessionError, apply_finalized_session_write};

#[derive(Debug, Default)]
pub(crate) struct ManualCompactionService;

impl ManualCompactionService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn run_persistent(
        &self,
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
        let mut context =
            ManualCompactionContext::new(options, replay, transaction, snapshot.clone());
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
}
