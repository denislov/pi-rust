use std::sync::Arc;

use super::capability_snapshot::{OperationCapabilitySnapshot, SessionWriteCapability};
use super::event_service::{EventService, SelfHealingEditEventObserver};
use super::flow_service::FlowService;
use super::self_healing_edit_flow::{
    PlannedSelfHealingEditRepairStrategy, SelfHealingEditContext, SelfHealingEditOptions,
    SelfHealingEditOutcome, SelfHealingEditRepairStrategy, SelfHealingEditReplacement,
};
use super::session_service::{FinalizedSessionWrite, SessionService};
use super::{CodingSessionError, default_cwd, session_cwd};

pub(crate) struct SelfHealingEditServiceOutcome {
    pub(crate) result: Result<SelfHealingEditOutcome, CodingSessionError>,
    pub(crate) finalized: FinalizedSessionWrite,
}

#[derive(Debug, Default)]
pub(crate) struct SelfHealingEditService;

impl SelfHealingEditService {
    pub(crate) fn new() -> Self {
        Self
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_persistent(
        &self,
        session_service: &mut SessionService,
        flow_service: &FlowService,
        event_service: EventService,
        path: String,
        replacements: Vec<SelfHealingEditReplacement>,
        check_command: Option<String>,
        repair_attempts: Vec<Vec<SelfHealingEditReplacement>>,
        model_repair_policy: Option<(Arc<dyn SelfHealingEditRepairStrategy>, usize)>,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<SelfHealingEditServiceOutcome, CodingSessionError> {
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

        match flow_service.run_self_healing_edit(&mut context).await {
            Ok(outcome) => {
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
                Ok(SelfHealingEditServiceOutcome {
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
                event_service.emit_self_healing_edit_failed(
                    operation_id.clone(),
                    event_path,
                    &error,
                );
                let finalized = session_service.fail_self_healing_edit_transaction(
                    Some(transaction),
                    operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                Ok(SelfHealingEditServiceOutcome {
                    result: Err(error),
                    finalized,
                })
            }
        }
    }
}
