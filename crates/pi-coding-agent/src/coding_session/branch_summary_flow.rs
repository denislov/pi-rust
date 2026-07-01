#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;

use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};

use super::CodingSessionError;
use super::prompt::PromptTurnTransaction;
use super::session_log::event::{DiagnosticLevel, SessionEventEnvelope};
use super::session_log::replay::SessionReplay;

const DEFAULT_ACTION: &str = "default";
const NO_ABANDONED_BRANCH_REASON: &str =
    "No abandoned branch is available in the current Rust-native session view";

pub(crate) const BRANCH_SUMMARY_NODE_IDS: &[&str] = &[
    "start_branch_summary",
    "load_branch_events",
    "select_abandoned_range",
    "prepare_summary_prompt",
    "run_summary_model",
    "record_branch_summary",
    "finalize_branch_summary",
];

const BRANCH_SUMMARY_NODE_SPECS: &[BranchSummaryNodeSpec] = &[
    BranchSummaryNodeSpec {
        id: "start_branch_summary",
        name: "StartBranchSummary",
        kind: BranchSummaryNodeKind::StartBranchSummary,
    },
    BranchSummaryNodeSpec {
        id: "load_branch_events",
        name: "LoadBranchEvents",
        kind: BranchSummaryNodeKind::LoadBranchEvents,
    },
    BranchSummaryNodeSpec {
        id: "select_abandoned_range",
        name: "SelectAbandonedRange",
        kind: BranchSummaryNodeKind::SelectAbandonedRange,
    },
    BranchSummaryNodeSpec {
        id: "prepare_summary_prompt",
        name: "PrepareSummaryPrompt",
        kind: BranchSummaryNodeKind::PrepareSummaryPrompt,
    },
    BranchSummaryNodeSpec {
        id: "run_summary_model",
        name: "RunSummaryModel",
        kind: BranchSummaryNodeKind::RunSummaryModel,
    },
    BranchSummaryNodeSpec {
        id: "record_branch_summary",
        name: "RecordBranchSummary",
        kind: BranchSummaryNodeKind::RecordBranchSummary,
    },
    BranchSummaryNodeSpec {
        id: "finalize_branch_summary",
        name: "FinalizeBranchSummary",
        kind: BranchSummaryNodeKind::FinalizeBranchSummary,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BranchSummaryNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: BranchSummaryNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchSummaryNodeKind {
    StartBranchSummary,
    LoadBranchEvents,
    SelectAbandonedRange,
    PrepareSummaryPrompt,
    RunSummaryModel,
    RecordBranchSummary,
    FinalizeBranchSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BranchSummaryOptions {
    source_leaf_id: Option<String>,
    target_leaf_id: Option<String>,
    custom_instructions: Option<String>,
}

impl BranchSummaryOptions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_source_leaf_id(mut self, source_leaf_id: impl Into<String>) -> Self {
        self.source_leaf_id = Some(source_leaf_id.into());
        self
    }

    pub(crate) fn with_target_leaf_id(mut self, target_leaf_id: impl Into<String>) -> Self {
        self.target_leaf_id = Some(target_leaf_id.into());
        self
    }

    pub(crate) fn with_custom_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.custom_instructions = Some(instructions.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BranchSummaryOutcome {
    Created {
        summary: String,
        source_leaf_id: String,
        target_leaf_id: String,
    },
    NoOp {
        reason: String,
    },
}

pub(crate) struct BranchSummaryContext {
    options: BranchSummaryOptions,
    operation_id: String,
    turn_id: String,
    replay: SessionReplay,
    transaction: Option<PromptTurnTransaction>,
    outcome: Option<BranchSummaryOutcome>,
    failure_error: Option<CodingSessionError>,
}

impl BranchSummaryContext {
    pub(crate) fn new(
        options: BranchSummaryOptions,
        replay: SessionReplay,
        transaction: PromptTurnTransaction,
    ) -> Self {
        let operation_id = transaction.operation_id().to_owned();
        let turn_id = transaction.turn_id().to_owned();
        Self {
            options,
            operation_id,
            turn_id,
            replay,
            transaction: Some(transaction),
            outcome: None,
            failure_error: None,
        }
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub(crate) fn outcome(&self) -> Option<&BranchSummaryOutcome> {
        self.outcome.as_ref()
    }

    pub(crate) fn pending_session_events(&self) -> &[SessionEventEnvelope] {
        self.transaction
            .as_ref()
            .map(PromptTurnTransaction::pending_events)
            .unwrap_or_default()
    }

    pub(crate) fn take_transaction(&mut self) -> Option<PromptTurnTransaction> {
        self.transaction.take()
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<BranchSummaryOutcome, CodingSessionError> {
        self.outcome
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "branch summary cannot finish without an outcome".into(),
            })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.failure_error = Some(error);
        message
    }

    fn start_branch_summary(&mut self) -> Result<(), CodingSessionError> {
        if self.transaction.is_none() {
            return Err(CodingSessionError::Session {
                message: "branch summary cannot start without a transaction".into(),
            });
        }
        Ok(())
    }

    fn load_branch_events(&mut self) -> Result<(), CodingSessionError> {
        if self.replay.session_id.is_empty() {
            return Err(CodingSessionError::Session {
                message: "branch summary cannot load an unnamed session replay".into(),
            });
        }
        Ok(())
    }

    fn select_abandoned_range(&mut self) -> Result<(), CodingSessionError> {
        if self.outcome.is_none() {
            self.outcome = Some(BranchSummaryOutcome::NoOp {
                reason: NO_ABANDONED_BRANCH_REASON.into(),
            });
        }
        Ok(())
    }

    fn prepare_summary_prompt(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    async fn run_summary_model(&mut self) -> Result<(), CodingSessionError> {
        Ok(())
    }

    fn record_branch_summary(&mut self) -> Result<(), CodingSessionError> {
        let outcome = self
            .outcome
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "branch summary cannot record events without an outcome".into(),
            })?;
        match outcome {
            BranchSummaryOutcome::Created {
                summary,
                source_leaf_id,
                target_leaf_id,
            } => self
                .transaction_mut_required()?
                .record_branch_summary_created(summary, source_leaf_id, target_leaf_id),
            BranchSummaryOutcome::NoOp { reason } => self
                .transaction_mut_required()?
                .emit_diagnostic(DiagnosticLevel::Info, reason),
        }
    }

    fn transaction_mut_required(
        &mut self,
    ) -> Result<&mut PromptTurnTransaction, CodingSessionError> {
        self.transaction
            .as_mut()
            .ok_or_else(|| CodingSessionError::Session {
                message: "branch summary has no active transaction".into(),
            })
    }

    fn finalize_branch_summary(&mut self) -> Result<(), CodingSessionError> {
        self.finish_success().map(|_| ())
    }
}

pub(crate) struct BranchSummaryFlow {
    flow: Flow<BranchSummaryContext>,
}

impl BranchSummaryFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(BRANCH_SUMMARY_NODE_IDS[0]).map_err(flow_error)?;
        for spec in BRANCH_SUMMARY_NODE_SPECS {
            flow.add_node(spec.id, BranchSummaryNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in BRANCH_SUMMARY_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    pub(crate) fn node_ids() -> &'static [&'static str] {
        BRANCH_SUMMARY_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut BranchSummaryContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }

    pub(crate) async fn run_with_options(
        &self,
        ctx: &mut BranchSummaryContext,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow
            .run_with_options(ctx, options)
            .await
            .map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct BranchSummaryNode {
    name: &'static str,
    kind: BranchSummaryNodeKind,
}

impl BranchSummaryNode {
    fn new(name: &'static str, kind: BranchSummaryNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<BranchSummaryContext> for BranchSummaryNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut BranchSummaryContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                BranchSummaryNodeKind::StartBranchSummary => ctx.start_branch_summary(),
                BranchSummaryNodeKind::LoadBranchEvents => ctx.load_branch_events(),
                BranchSummaryNodeKind::SelectAbandonedRange => ctx.select_abandoned_range(),
                BranchSummaryNodeKind::PrepareSummaryPrompt => ctx.prepare_summary_prompt(),
                BranchSummaryNodeKind::RunSummaryModel => ctx.run_summary_model().await,
                BranchSummaryNodeKind::RecordBranchSummary => ctx.record_branch_summary(),
                BranchSummaryNodeKind::FinalizeBranchSummary => ctx.finalize_branch_summary(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::CodingAgentSessionOptions;
    use crate::coding_session::session_log::event::{SessionEventData, SessionEventEnvelope};
    use crate::coding_session::session_service::SessionService;

    fn event_kinds(events: &[SessionEventEnvelope]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event.data {
                SessionEventData::OperationStarted { .. } => "operation.started",
                SessionEventData::TurnStarted {} => "turn.started",
                SessionEventData::DiagnosticEmitted { .. } => "diagnostic.emitted",
                _ => "other",
            })
            .collect()
    }

    #[tokio::test]
    async fn branch_summary_flow_no_abandoned_branch_records_no_op_without_flushing() {
        let temp = tempfile::tempdir().unwrap();
        let service = SessionService::create(
            &CodingAgentSessionOptions::new()
                .with_session_id("sess_branch_summary_flow")
                .with_session_log_root(temp.path()),
        )
        .unwrap();
        let replay = service.replay().unwrap();
        let transaction = service.begin_branch_summary_transaction();
        let mut context =
            BranchSummaryContext::new(BranchSummaryOptions::new(), replay, transaction);
        let flow = BranchSummaryFlow::new().unwrap();

        let outcome = flow.run(&mut context).await.unwrap();

        assert_eq!(outcome.last_node.as_str(), "finalize_branch_summary");
        assert_eq!(
            context.outcome(),
            Some(&BranchSummaryOutcome::NoOp {
                reason: "No abandoned branch is available in the current Rust-native session view"
                    .into(),
            })
        );
        assert_eq!(
            event_kinds(context.pending_session_events()),
            vec!["operation.started", "turn.started", "diagnostic.emitted"]
        );
        assert!(service.replay().unwrap().transcript.is_empty());
    }
}
