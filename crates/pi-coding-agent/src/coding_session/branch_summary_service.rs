use super::branch_summary_flow::{
    BranchSummaryContext, BranchSummaryOptions, branch_summary_failed_outcome,
    branch_summary_outcome_text, branch_summary_success_outcome,
};
use super::capability_snapshot::{
    OperationCapabilitySnapshot, SessionReadCapability, SessionWriteCapability,
};
use super::event_service::EventService;
use super::flow_service::FlowService;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome, RuntimeSnapshot};
use super::session_log::id::{IdGenerator, SystemIdGenerator};
use super::session_service::{SessionPersistence, SessionService};
use super::{CodingSessionError, apply_finalized_session_write};

#[derive(Debug, Default)]
pub(crate) struct BranchSummaryService;

impl BranchSummaryService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn reused_outcome(
        &self,
        persistence: &SessionPersistence,
        options: &PromptTurnOptions,
        source_leaf_id: &str,
        target_leaf_id: &str,
    ) -> Result<Option<PromptTurnOutcome>, CodingSessionError> {
        let runtime = branch_summary_runtime(options)?;
        let SessionPersistence::Persistent(session_service) = persistence else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "branch summary without persistent session".into(),
            });
        };
        let Some(summary) = session_service.branch_summary_for(source_leaf_id, target_leaf_id)?
        else {
            return Ok(None);
        };
        let mut ids = SystemIdGenerator;
        let operation_id = ids.next_operation_id();
        let turn_id = ids.next_turn_id();
        Ok(Some(branch_summary_success_outcome(
            operation_id,
            turn_id,
            session_service.session_id().to_owned(),
            session_service.active_leaf_id().map(str::to_owned),
            &runtime,
            summary,
        )))
    }

    pub(crate) async fn run_persistent(
        &self,
        session_service: &mut SessionService,
        flow_service: &FlowService,
        event_service: &EventService,
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        SessionReadCapability::require(snapshot.session_read.as_ref())?;
        SessionWriteCapability::require(snapshot.session_write.as_ref())?;
        let runtime = branch_summary_runtime(&options)?;
        let mut branch_options = BranchSummaryOptions::new()
            .with_source_leaf_id(source_leaf_id)
            .with_target_leaf_id(target_leaf_id)
            .with_runtime(runtime.clone());
        if let Some(custom_instructions) = custom_instructions {
            branch_options = branch_options.with_custom_instructions(custom_instructions);
        }
        let replay = session_service.replay()?;
        let transaction = session_service.begin_branch_summary_transaction();
        let mut context = BranchSummaryContext::new(branch_options, replay, transaction);
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        match flow_service.run_branch_summary(&mut context).await {
            Ok(branch_summary) => {
                let final_text = branch_summary_outcome_text(&branch_summary);
                let mut outcome = branch_summary_success_outcome(
                    operation_id.clone(),
                    turn_id,
                    session_service.session_id().to_owned(),
                    session_service.active_leaf_id().map(str::to_owned),
                    &runtime,
                    final_text,
                );
                let finalized = session_service
                    .commit_branch_summary_transaction(context.take_transaction(), operation_id)?;
                apply_finalized_session_write(&mut outcome, &finalized);
                event_service.emit_session_write_events(&finalized);
                Ok(outcome)
            }
            Err(error) => {
                let mut outcome =
                    branch_summary_failed_outcome(operation_id.clone(), turn_id, error.clone());
                let finalized = session_service.fail_prompt_transaction(
                    context.take_transaction(),
                    operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                apply_finalized_session_write(&mut outcome, &finalized);
                event_service.emit_session_write_events(&finalized);
                Ok(outcome)
            }
        }
    }
}

fn branch_summary_runtime(
    options: &PromptTurnOptions,
) -> Result<RuntimeSnapshot, CodingSessionError> {
    options
        .runtime()
        .cloned()
        .ok_or_else(|| CodingSessionError::Config {
            message: "branch summary options do not include a runtime snapshot".into(),
        })
}
