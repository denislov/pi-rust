use std::future::Future;
use std::pin::Pin;

use pi_agent_core::api::AgentMessage;
use pi_agent_core::api::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
use pi_agent_core::api::{estimate_tokens, summarize_with_provider_streamer};
use pi_ai::api::{AssistantMessage, ContentBlock, StreamOptions};
use tokio_util::sync::CancellationToken;

use super::CodingSessionError;
use super::capability_snapshot::OperationCapabilitySnapshot;
use super::plugin_service::PluginService;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome, PromptTurnTransaction, RuntimeSnapshot};
use super::runtime_service::{RuntimeService, scoped_provider_streamer_for_runtime};
#[cfg(test)]
use super::session_log::event::SessionEventEnvelope;
use super::session_log::replay::{SessionReplay, transcript_item_id};
use crate::runtime::PromptInvocation;

const DEFAULT_ACTION: &str = "default";

pub(crate) const MANUAL_COMPACTION_NODE_IDS: &[&str] = &[
    "start_compaction",
    "load_session_replay",
    "select_compaction_range",
    "prepare_summary_context",
    "run_summary_model",
    "record_compaction_events",
    "finalize_compaction",
    "emit_completion",
];

const MANUAL_COMPACTION_NODE_SPECS: &[ManualCompactionNodeSpec] = &[
    ManualCompactionNodeSpec {
        id: "start_compaction",
        name: "StartCompaction",
        kind: ManualCompactionNodeKind::StartCompaction,
    },
    ManualCompactionNodeSpec {
        id: "load_session_replay",
        name: "LoadSessionReplay",
        kind: ManualCompactionNodeKind::LoadSessionReplay,
    },
    ManualCompactionNodeSpec {
        id: "select_compaction_range",
        name: "SelectCompactionRange",
        kind: ManualCompactionNodeKind::SelectCompactionRange,
    },
    ManualCompactionNodeSpec {
        id: "prepare_summary_context",
        name: "PrepareSummaryContext",
        kind: ManualCompactionNodeKind::PrepareSummaryContext,
    },
    ManualCompactionNodeSpec {
        id: "run_summary_model",
        name: "RunSummaryModel",
        kind: ManualCompactionNodeKind::RunSummaryModel,
    },
    ManualCompactionNodeSpec {
        id: "record_compaction_events",
        name: "RecordCompactionEvents",
        kind: ManualCompactionNodeKind::RecordCompactionEvents,
    },
    ManualCompactionNodeSpec {
        id: "finalize_compaction",
        name: "FinalizeCompaction",
        kind: ManualCompactionNodeKind::FinalizeCompaction,
    },
    ManualCompactionNodeSpec {
        id: "emit_completion",
        name: "EmitCompletion",
        kind: ManualCompactionNodeKind::EmitCompletion,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManualCompactionNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: ManualCompactionNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManualCompactionNodeKind {
    StartCompaction,
    LoadSessionReplay,
    SelectCompactionRange,
    PrepareSummaryContext,
    RunSummaryModel,
    RecordCompactionEvents,
    FinalizeCompaction,
    EmitCompletion,
}

#[derive(Debug, Clone)]
pub(crate) struct ManualCompactionOptions {
    runtime: RuntimeSnapshot,
    custom_instructions: Option<String>,
    cancellation: Option<CancellationToken>,
}

impl ManualCompactionOptions {
    #[cfg(test)]
    pub(crate) fn new(runtime: RuntimeSnapshot) -> Self {
        Self {
            runtime,
            custom_instructions: None,
            cancellation: None,
        }
    }

    pub(crate) fn from_prompt_turn_options(
        options: &PromptTurnOptions,
    ) -> Result<Self, CodingSessionError> {
        let custom_instructions = match options.invocation() {
            PromptInvocation::Compact {
                custom_instructions,
            } => custom_instructions.clone(),
            _ => {
                return Err(CodingSessionError::Input {
                    message: "compact operation requires a compaction invocation".into(),
                });
            }
        };
        let runtime = options
            .runtime()
            .cloned()
            .ok_or_else(|| CodingSessionError::Config {
                message: "compact operation options do not include a runtime snapshot".into(),
            })?;
        Ok(Self {
            runtime,
            custom_instructions,
            cancellation: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_custom_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.custom_instructions = Some(instructions.into());
        self
    }

    pub(crate) fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    fn custom_instructions(&self) -> Option<&str> {
        self.custom_instructions.as_deref()
    }

    pub(crate) fn cancellation(&self) -> Option<CancellationToken> {
        self.cancellation.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ManualCompactionOutcome {
    pub(crate) summary: String,
    pub(crate) first_kept_message_id: String,
    pub(crate) tokens_before: u32,
    pub(crate) final_message: AssistantMessage,
}

pub(crate) fn manual_compaction_success_outcome(
    operation_id: impl Into<String>,
    turn_id: impl Into<String>,
    session_id: impl Into<String>,
    leaf_id: Option<String>,
    outcome: &ManualCompactionOutcome,
) -> PromptTurnOutcome {
    PromptTurnOutcome::Success {
        operation_id: operation_id.into(),
        turn_id: turn_id.into(),
        session_id: Some(session_id.into()),
        leaf_id,
        final_text: outcome.summary.clone(),
        final_message: outcome.final_message.clone(),
        diagnostics: Vec::new(),
    }
}

pub(crate) fn manual_compaction_failed_outcome(
    operation_id: impl Into<String>,
    turn_id: impl Into<String>,
    error: CodingSessionError,
) -> PromptTurnOutcome {
    PromptTurnOutcome::Failed {
        operation_id: operation_id.into(),
        turn_id: Some(turn_id.into()),
        error,
        diagnostics: Vec::new(),
    }
}

pub(crate) struct ManualCompactionContext {
    options: ManualCompactionOptions,
    operation_id: String,
    turn_id: String,
    replay: SessionReplay,
    transaction: Option<PromptTurnTransaction>,
    capability_snapshot: OperationCapabilitySnapshot,
    first_kept_message_id: Option<String>,
    tokens_before: Option<u32>,
    summary_messages: Vec<AgentMessage>,
    stream_options: Option<StreamOptions>,
    summary: Option<String>,
    final_message: Option<AssistantMessage>,
    failure_error: Option<CodingSessionError>,
}

impl ManualCompactionContext {
    pub(crate) fn new(
        options: ManualCompactionOptions,
        replay: SessionReplay,
        transaction: PromptTurnTransaction,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        let operation_id = transaction.operation_id().to_owned();
        let turn_id = transaction.turn_id().to_owned();
        Self {
            options,
            operation_id,
            turn_id,
            replay,
            transaction: Some(transaction),
            capability_snapshot,
            first_kept_message_id: None,
            tokens_before: None,
            summary_messages: Vec::new(),
            stream_options: None,
            summary: None,
            final_message: None,
            failure_error: None,
        }
    }

    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(crate) fn options(&self) -> &ManualCompactionOptions {
        &self.options
    }

    pub(crate) fn turn_id(&self) -> &str {
        &self.turn_id
    }

    #[cfg(test)]
    pub(crate) fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn first_kept_message_id(&self) -> Option<&str> {
        self.first_kept_message_id.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn tokens_before(&self) -> Option<u32> {
        self.tokens_before
    }

    #[cfg(test)]
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

    pub(crate) fn finish_success(&self) -> Result<ManualCompactionOutcome, CodingSessionError> {
        Ok(ManualCompactionOutcome {
            summary: self
                .summary
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "manual compaction cannot finish without a summary".into(),
                })?,
            first_kept_message_id: self.first_kept_message_id.clone().ok_or_else(|| {
                CodingSessionError::Session {
                    message: "manual compaction cannot finish without a kept message id".into(),
                }
            })?,
            tokens_before: self
                .tokens_before
                .ok_or_else(|| CodingSessionError::Session {
                    message: "manual compaction cannot finish without token accounting".into(),
                })?,
            final_message: self.final_message.clone().ok_or_else(|| {
                CodingSessionError::Session {
                    message: "manual compaction cannot finish without a final message".into(),
                }
            })?,
        })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.failure_error = Some(error);
        message
    }

    fn transaction_mut_required(
        &mut self,
    ) -> Result<&mut PromptTurnTransaction, CodingSessionError> {
        self.transaction
            .as_mut()
            .ok_or_else(|| CodingSessionError::Session {
                message: "manual compaction has no active transaction".into(),
            })
    }

    fn start_compaction(&mut self) -> Result<(), CodingSessionError> {
        if self.transaction.is_none() {
            return Err(CodingSessionError::Session {
                message: "manual compaction cannot start without a transaction".into(),
            });
        }
        Ok(())
    }

    fn load_session_replay(&mut self) -> Result<(), CodingSessionError> {
        if self.replay.session_id.is_empty() {
            return Err(CodingSessionError::Session {
                message: "manual compaction cannot load an unnamed session replay".into(),
            });
        }
        Ok(())
    }

    fn select_compaction_range(&mut self) -> Result<(), CodingSessionError> {
        if self.first_kept_message_id.is_some() {
            return Ok(());
        }
        let first_kept_message_id = self
            .replay
            .transcript
            .iter()
            .rev()
            .find_map(transcript_item_id)
            .ok_or_else(|| CodingSessionError::Session {
                message: "Nothing to compact (no messages yet)".into(),
            })?;
        self.first_kept_message_id = Some(first_kept_message_id);
        Ok(())
    }

    fn prepare_summary_context(&mut self) -> Result<(), CodingSessionError> {
        if !self.summary_messages.is_empty() {
            return Ok(());
        }
        let service = RuntimeService::new();
        let build = service.build_agent_runtime_with_capabilities(
            self.options.runtime(),
            &PluginService::new(),
            &self.capability_snapshot,
        )?;
        let agent = build.agent;
        service.hydrate_agent_runtime(&agent, self.options.runtime(), &self.replay);
        let messages = agent.messages();
        if messages.len() < 2 {
            return Err(CodingSessionError::Session {
                message: "Nothing to compact (no messages yet)".into(),
            });
        }
        let first_kept_index = messages.len() - 1;
        let to_summarize = messages[..first_kept_index].to_vec();
        if to_summarize.is_empty() {
            return Err(CodingSessionError::Session {
                message: "Nothing to compact (no compactable history)".into(),
            });
        }
        let tokens_before = estimate_tokens(&messages);
        let first_kept_message_id =
            self.first_kept_message_id
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "manual compaction range was not selected".into(),
                })?;
        let stream_options = agent.provider_request_snapshot().1;
        self.transaction_mut_required()?
            .record_session_compaction_started(first_kept_message_id, tokens_before)?;
        self.tokens_before = Some(tokens_before);
        self.summary_messages = to_summarize;
        self.stream_options = stream_options;
        Ok(())
    }

    async fn run_summary_model(&mut self) -> Result<(), CodingSessionError> {
        if self.summary.is_some() {
            return Ok(());
        }
        let summary = summarize_with_provider_streamer(
            self.options.runtime().model(),
            &self.summary_messages,
            self.options.custom_instructions(),
            self.stream_options.clone(),
            None,
            Some(scoped_provider_streamer_for_runtime(self.options.runtime())),
        )
        .await
        .map_err(|error| CodingSessionError::Provider {
            message: error.to_string(),
        })?;
        self.summary = Some(summary.clone());
        self.final_message = Some(compaction_final_message(self.options.runtime(), &summary));
        Ok(())
    }

    fn record_compaction_events(&mut self) -> Result<(), CodingSessionError> {
        let summary = self
            .summary
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "manual compaction cannot record events without a summary".into(),
            })?;
        let first_kept_message_id =
            self.first_kept_message_id
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "manual compaction cannot record events without a kept message id"
                        .into(),
                })?;
        let tokens_before = self
            .tokens_before
            .ok_or_else(|| CodingSessionError::Session {
                message: "manual compaction cannot record events without token accounting".into(),
            })?;
        self.transaction_mut_required()?
            .record_session_compaction_completed(summary, first_kept_message_id, tokens_before)
    }

    fn finalize_compaction(&mut self) -> Result<(), CodingSessionError> {
        self.finish_success().map(|_| ())
    }

    fn emit_completion(&mut self) -> Result<(), CodingSessionError> {
        self.finish_success().map(|_| ())
    }
}

pub(crate) struct ManualCompactionFlow {
    flow: Flow<ManualCompactionContext>,
}

impl ManualCompactionFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(MANUAL_COMPACTION_NODE_IDS[0]).map_err(flow_error)?;
        for spec in MANUAL_COMPACTION_NODE_SPECS {
            flow.add_node(spec.id, ManualCompactionNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        super::flow_service::add_linear_edges(&mut flow, MANUAL_COMPACTION_NODE_IDS)?;
        Ok(Self { flow })
    }

    #[cfg(test)]
    pub(crate) fn node_ids() -> &'static [&'static str] {
        MANUAL_COMPACTION_NODE_IDS
    }

    #[cfg(test)]
    pub(crate) async fn run(
        &self,
        ctx: &mut ManualCompactionContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }

    pub(crate) async fn run_with_options(
        &self,
        ctx: &mut ManualCompactionContext,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow
            .run_with_options(ctx, options)
            .await
            .map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct ManualCompactionNode {
    name: &'static str,
    kind: ManualCompactionNodeKind,
}

impl ManualCompactionNode {
    fn new(name: &'static str, kind: ManualCompactionNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<ManualCompactionContext> for ManualCompactionNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ManualCompactionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                ManualCompactionNodeKind::StartCompaction => ctx.start_compaction(),
                ManualCompactionNodeKind::LoadSessionReplay => ctx.load_session_replay(),
                ManualCompactionNodeKind::SelectCompactionRange => ctx.select_compaction_range(),
                ManualCompactionNodeKind::PrepareSummaryContext => ctx.prepare_summary_context(),
                ManualCompactionNodeKind::RunSummaryModel => ctx.run_summary_model().await,
                ManualCompactionNodeKind::RecordCompactionEvents => ctx.record_compaction_events(),
                ManualCompactionNodeKind::FinalizeCompaction => ctx.finalize_compaction(),
                ManualCompactionNodeKind::EmitCompletion => ctx.emit_completion(),
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
    match error {
        FlowError::Cancelled => CodingSessionError::Cancelled,
        other => CodingSessionError::Flow {
            message: other.to_string(),
        },
    }
}

fn compaction_final_message(runtime: &RuntimeSnapshot, summary: &str) -> AssistantMessage {
    let mut message = AssistantMessage::empty(&runtime.model().api, &runtime.model().id);
    message.provider = Some(runtime.model().provider.clone());
    message.content.push(ContentBlock::Text {
        text: summary.to_owned(),
        text_signature: None,
    });
    message
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pi_agent_core::api::AgentResources;
    use pi_ai::api::{Model, ModelCost, ModelInput};
    use pi_ai::providers::faux::FauxProvider;

    use super::*;
    use crate::coding_session::session_log::event::{
        PersistedContentBlock, SessionEventData, SessionEventEnvelope,
    };
    use crate::coding_session::session_service::SessionService;

    #[tokio::test]
    async fn compact_cancellation_maps_flow_cancelled_to_typed_error() {
        let api = "manual-compaction-flow-cancelled";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("unused summary")),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut service = SessionService::create(
            &CodingAgentSessionOptions::new()
                .with_session_id("sess_manual_compaction_cancelled")
                .with_session_log_root(temp.path()),
        )
        .unwrap();
        record_prompt(&mut service);
        let replay = service.replay().unwrap();
        let snapshot = OperationCapabilitySnapshot::permissive("op_cancelled");
        let transaction = service.begin_manual_compaction_transaction(&snapshot);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut context = ManualCompactionContext::new(
            ManualCompactionOptions::new(compact_runtime(api, _provider_guard.ai_client()))
                .with_cancellation(cancellation),
            replay,
            transaction,
            snapshot,
        );

        let error = super::super::flow_service::FlowService::new()
            .run_manual_compaction_graph(&mut context)
            .await
            .unwrap_err();
        assert_eq!(error, CodingSessionError::Cancelled);
    }
    use crate::coding_session::{CodingAgentSessionOptions, PromptTurnOptions};
    use crate::prompt_options::PromptRunOptions;
    use crate::runtime::{PromptInvocation, SessionRunOptions};

    fn model(api: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn compact_runtime(
        api: &str,
        ai_client: pi_ai::api::AiClient,
    ) -> super::super::prompt::RuntimeSnapshot {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: String::new(),
            model: model(api),
            api_key: None,
            auth_diagnostics: Vec::new(),
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            ai_client: Some(ai_client),
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Compact {
                custom_instructions: Some("keep decisions".into()),
            },
        })
        .runtime()
        .cloned()
        .unwrap()
    }

    fn record_prompt(service: &mut SessionService) -> String {
        let mut transaction = service.begin_prompt_transaction();
        let operation_id = transaction.operation_id().to_owned();
        transaction
            .record_user_input(vec![PersistedContentBlock::Text {
                text: "first question".into(),
            }])
            .unwrap();
        let message_id = transaction.start_assistant_message().unwrap();
        transaction
            .complete_assistant_message(
                message_id,
                vec![PersistedContentBlock::Text {
                    text: "first answer".into(),
                }],
                Some("stop".into()),
                Default::default(),
            )
            .unwrap();
        service
            .commit_prompt_transaction(Some(transaction), operation_id)
            .unwrap()
            .leaf_id
            .unwrap()
    }

    fn event_kinds(events: &[SessionEventEnvelope]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event.data {
                SessionEventData::OperationStarted { .. } => "operation.started",
                SessionEventData::TurnStarted {} => "turn.started",
                SessionEventData::SessionCompactionStarted { .. } => "session.compaction.started",
                SessionEventData::SessionCompactionCompleted { .. } => {
                    "session.compaction.completed"
                }
                SessionEventData::OperationCommitted { .. } => "operation.committed",
                SessionEventData::OperationFailed { .. } => "operation.failed",
                SessionEventData::OperationAborted { .. } => "operation.aborted",
                SessionEventData::DiagnosticEmitted { .. } => "diagnostic.emitted",
                _ => "other",
            })
            .collect()
    }

    #[tokio::test]
    async fn manual_compaction_flow_records_summary_events_without_flushing() {
        let api = "manual-compaction-flow-records-summary";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("summary from flow")),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut service = SessionService::create(
            &CodingAgentSessionOptions::new()
                .with_session_id("sess_manual_compaction_flow")
                .with_session_log_root(temp.path()),
        )
        .unwrap();
        let active_leaf = record_prompt(&mut service);
        let replay = service.replay().unwrap();
        let snapshot = OperationCapabilitySnapshot::permissive("op_test");
        let transaction = service.begin_manual_compaction_transaction(&snapshot);
        let mut context = ManualCompactionContext::new(
            ManualCompactionOptions::new(compact_runtime(api, _provider_guard.ai_client()))
                .with_custom_instructions("keep decisions"),
            replay,
            transaction,
            snapshot,
        );
        let flow = ManualCompactionFlow::new().unwrap();

        let outcome = flow.run(&mut context).await.unwrap();

        assert_eq!(outcome.last_node.as_str(), "emit_completion");
        assert_eq!(context.summary(), Some("summary from flow"));
        assert!(
            context
                .first_kept_message_id()
                .is_some_and(|id| id.starts_with("msg_"))
        );
        assert!(context.tokens_before().is_some_and(|tokens| tokens > 0));
        assert_eq!(
            event_kinds(context.pending_session_events()),
            vec![
                "operation.started",
                "turn.started",
                "session.compaction.started",
                "session.compaction.completed",
            ]
        );
        assert_eq!(
            service.replay().unwrap().active_leaf_id.as_deref(),
            Some(active_leaf.as_str())
        );
        assert!(matches!(
            service.replay().unwrap().transcript.as_slice(),
            [
                crate::coding_session::session_log::replay::TranscriptItem::UserInput { .. },
                crate::coding_session::session_log::replay::TranscriptItem::AssistantMessage { .. }
            ]
        ));
    }
}
