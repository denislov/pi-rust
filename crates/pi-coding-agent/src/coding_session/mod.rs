mod branch_summary_flow;
mod capability_service;
mod context;
mod error;
mod event;
mod event_service;
mod export;
mod flow_service;
mod manual_compaction_flow;
mod plugin_load_flow;
mod plugin_service;
mod prompt;
mod prompt_flow;
mod runtime_service;
mod session_log;
mod session_service;

pub use context::{
    CapabilityStatus, CodingAgentCapabilities, CodingAgentSessionOptions,
    CodingAgentSessionSummary, CodingAgentSessionView,
};
pub(crate) use context::{
    CodingAgentSessionDiagnostic, CodingAgentSessionHydration, CodingAgentSessionTranscriptItem,
    CodingAgentSessionTree,
};
pub use error::CodingSessionError;
pub use event::CodingAgentEvent;
pub use event_service::CodingAgentEventReceiver;
pub use export::{CodingAgentSessionExport, CodingAgentSessionExportItem};
pub use prompt::{
    CodingDiagnostic, CodingDiagnosticSeverity, PromptTurnMode, PromptTurnOptions,
    PromptTurnOutcome,
};

use branch_summary_flow::{BranchSummaryContext, BranchSummaryOptions, BranchSummaryOutcome};
use capability_service::CapabilityService;
use event_service::EventService;
use flow_service::FlowService;
use manual_compaction_flow::{ManualCompactionContext, ManualCompactionOptions};
use plugin_load_flow::{PluginLoadContext, PluginLoadOptions, PluginLoadOutcome};
use plugin_service::PluginService;
use prompt::{PromptTurnContext, PromptTurnIds, RuntimeSnapshot};
use runtime_service::RuntimeService;
use session_log::id::{IdGenerator, SystemIdGenerator};
use session_log::replay::TranscriptItem;
use session_service::{FinalizedSessionWrite, SessionService};
use std::path::{Path, PathBuf};

use crate::plugins::PluginSource;

#[derive(Debug)]
pub struct CodingAgentSession {
    persistence: SessionPersistence,
    runtime_service: RuntimeService,
    flow_service: FlowService,
    event_service: EventService,
    capability_service: CapabilityService,
    plugin_service: PluginService,
    default_plugin_load_options: PluginLoadOptions,
    active_operation: Option<String>,
}

#[derive(Debug)]
enum SessionPersistence {
    Persistent(SessionService),
    NonPersistent(TransientSessionState),
}

#[derive(Debug)]
struct TransientSessionState {
    runtime_id: String,
    transcript: Vec<TranscriptItem>,
}

impl TransientSessionState {
    fn new() -> Self {
        let mut ids = SystemIdGenerator;
        Self {
            runtime_id: format!("runtime_{}", ids.next_session_id()),
            transcript: Vec::new(),
        }
    }
}

fn default_plugin_load_options(options: &CodingAgentSessionOptions) -> PluginLoadOptions {
    let cwd = options
        .cwd()
        .map(Path::to_path_buf)
        .unwrap_or_else(default_cwd);
    let paths = crate::config::resolve_paths(&cwd);
    PluginLoadOptions::new()
        .with_discovery_root(paths.project_dir.join("plugins"), PluginSource::Project)
        .with_discovery_root(paths.global_dir.join("plugins"), PluginSource::User)
}

fn default_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

impl CodingAgentSession {
    pub async fn create(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::create(&options)?;
        Self::from_services(session_service, default_plugin_load_options(&options))
    }

    pub async fn open(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        Self::from_services(session_service, default_plugin_load_options(&options))
    }

    pub async fn open_or_create(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open_or_create(&options)?;
        Self::from_services(session_service, default_plugin_load_options(&options))
    }

    pub async fn non_persistent(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_id().is_some() || options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "non-persistent coding sessions do not accept a session id or path".into(),
            });
        }
        Self::from_transient(
            TransientSessionState::new(),
            default_plugin_load_options(&options),
        )
    }

    pub fn list(
        options: CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        SessionService::list(&options)
    }

    pub(crate) fn hydrate(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::hydrate(&options)
    }

    pub(crate) fn tree_view(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionTree, CodingSessionError> {
        SessionService::tree_view(&options)
    }

    pub(crate) fn clone_session(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .clone_current()?
            .hydrated_view()
    }

    pub(crate) fn fork_session(
        options: CodingAgentSessionOptions,
        target_leaf_id: Option<&str>,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .fork_current(target_leaf_id)?
            .hydrated_view()
    }

    pub fn export_session_html(
        options: CodingAgentSessionOptions,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        SessionService::open(&options)?.export_html(path.as_ref())
    }

    pub fn export_current_html(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.export_html(path.as_ref())
            }
            SessionPersistence::NonPersistent(_) => {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "export requires a persistent Rust-native session".into(),
                })
            }
        }
    }

    pub fn export_current(&self) -> Result<CodingAgentSessionExport, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service.export_view(),
            SessionPersistence::NonPersistent(_) => {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "export requires a persistent Rust-native session".into(),
                })
            }
        }
    }

    pub(crate) fn hydrate_current(
        &self,
    ) -> Result<Option<CodingAgentSessionHydration>, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                Ok(Some(session_service.hydrated_view()?))
            }
            SessionPersistence::NonPersistent(_) => Ok(None),
        }
    }

    pub(crate) fn fork_current_session(
        &self,
        target_leaf_id: Option<&str>,
    ) -> Result<Self, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => Self::from_services(
                session_service.fork_current(target_leaf_id)?,
                self.default_plugin_load_options.clone(),
            ),
            SessionPersistence::NonPersistent(_) => {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "fork requires a persistent Rust-native session".into(),
                })
            }
        }
    }

    pub fn subscribe(&self) -> CodingAgentEventReceiver {
        self.event_service.subscribe()
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        let plugin_capabilities = self.plugin_service.capabilities();
        let persistent = matches!(self.persistence, SessionPersistence::Persistent(_));
        self.capability_service.capabilities(
            self.active_operation.as_deref(),
            &plugin_capabilities,
            persistent,
        )
    }

    pub fn view(&self) -> CodingAgentSessionView {
        let _ = (
            &self.runtime_service,
            &self.flow_service,
            &self.plugin_service,
        );
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service.view(),
            SessionPersistence::NonPersistent(state) => CodingAgentSessionView {
                session_id: state.runtime_id.clone(),
            },
        }
    }

    pub async fn prompt(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "prompt".into(),
            });
        }
        self.active_operation = Some("prompt".into());
        let result = self.prompt_inner(options).await;
        self.active_operation = None;
        result
    }

    pub async fn compact(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "compact".into(),
            });
        }
        self.active_operation = Some("compact".into());
        let result = self.compact_inner(options).await;
        self.active_operation = None;
        result
    }

    #[allow(dead_code)]
    pub(crate) async fn reload_plugins(&mut self) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.load_plugins(self.default_plugin_load_options.clone())
            .await
    }

    #[allow(dead_code)]
    pub(crate) async fn load_plugins(
        &mut self,
        options: PluginLoadOptions,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "plugin_load".into(),
            });
        }
        self.active_operation = Some("plugin_load".into());
        let result = self.load_plugins_inner(options).await;
        self.active_operation = None;
        result
    }

    pub async fn summarize_branch(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: impl Into<String>,
        target_leaf_id: impl Into<String>,
        custom_instructions: Option<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "branch_summary".into(),
            });
        }
        self.active_operation = Some("branch_summary".into());
        let result = self
            .summarize_branch_inner(
                options,
                source_leaf_id.into(),
                target_leaf_id.into(),
                custom_instructions,
            )
            .await;
        self.active_operation = None;
        result
    }

    pub(crate) async fn summarize_branch_for_navigation(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: impl Into<String>,
        target_leaf_id: impl Into<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "branch_summary".into(),
            });
        }
        let source_leaf_id = source_leaf_id.into();
        let target_leaf_id = target_leaf_id.into();
        if let Some(outcome) = self.reused_branch_summary_outcome(
            &options,
            source_leaf_id.as_str(),
            target_leaf_id.as_str(),
        )? {
            return Ok(outcome);
        }

        self.active_operation = Some("branch_summary".into());
        let result = self
            .summarize_branch_inner(options, source_leaf_id, target_leaf_id, None)
            .await;
        self.active_operation = None;
        result
    }

    fn from_services(
        session_service: SessionService,
        default_plugin_load_options: PluginLoadOptions,
    ) -> Result<Self, CodingSessionError> {
        let event_service = EventService::new();
        event_service.emit(CodingAgentEvent::SessionOpened {
            session_id: session_service.session_id().to_owned(),
        });

        Ok(Self {
            persistence: SessionPersistence::Persistent(session_service),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            default_plugin_load_options,
            active_operation: None,
        })
    }

    fn from_transient(
        state: TransientSessionState,
        default_plugin_load_options: PluginLoadOptions,
    ) -> Result<Self, CodingSessionError> {
        Ok(Self {
            persistence: SessionPersistence::NonPersistent(state),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service: EventService::new(),
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            default_plugin_load_options,
            active_operation: None,
        })
    }

    #[allow(dead_code)]
    async fn load_plugins_inner(
        &mut self,
        options: PluginLoadOptions,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        let mut context = PluginLoadContext::new(options);
        let outcome = self.flow_service.run_plugin_load(&mut context).await?;
        if let Some(plugin_service) = context.take_loaded_plugin_service() {
            self.plugin_service = plugin_service;
        }
        for diagnostic in &outcome.diagnostics {
            self.event_service.emit(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: diagnostic.message.clone(),
            });
        }
        if outcome.capability_changed {
            self.event_service.emit(CodingAgentEvent::CapabilityChanged);
        }
        Ok(outcome)
    }

    async fn prompt_inner(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        let mut context = self.prepare_prompt_context(options)?;
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        self.event_service.emit(CodingAgentEvent::PromptStarted {
            operation_id,
            turn_id,
        });
        let mut outcome = match self.flow_service.run_prompt_turn(&mut context).await {
            Ok(outcome) => outcome,
            Err(error) => context.finish_failure(error),
        };
        let finalized = match self.finalize_prompt_transaction(&mut context, &outcome) {
            Ok(finalized) => finalized,
            Err(error) => {
                outcome = context.finish_failure(error.clone());
                SessionService::skip_prompt_transaction(
                    context.operation_id().to_owned(),
                    format!("session write finalization failed: {error}"),
                )
            }
        };
        apply_finalized_session_write(&mut outcome, &finalized);

        if !context.live_events_enabled() {
            self.emit_coding_events_before_prompt_outcome(context.coding_events());
        }
        self.emit_session_write_events(&finalized);
        self.emit_prompt_outcome_event(&outcome);
        Ok(outcome)
    }

    async fn compact_inner(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        let options = ManualCompactionOptions::from_prompt_turn_options(&options)?;
        let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "manual compaction without persistent session".into(),
            });
        };

        let replay = session_service.replay()?;
        let transaction = session_service.begin_manual_compaction_transaction();
        let mut context = ManualCompactionContext::new(options, replay, transaction);
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        match self.flow_service.run_manual_compaction(&mut context).await {
            Ok(compaction) => {
                let mut outcome = PromptTurnOutcome::Success {
                    operation_id: operation_id.clone(),
                    turn_id: turn_id.clone(),
                    session_id: Some(session_service.session_id().to_owned()),
                    leaf_id: session_service.active_leaf_id().map(str::to_owned),
                    final_text: compaction.summary.clone(),
                    final_message: compaction.final_message,
                    diagnostics: Vec::new(),
                };
                let finalized = session_service.commit_manual_compaction_transaction(
                    context.take_transaction(),
                    operation_id.clone(),
                )?;
                apply_finalized_session_write(&mut outcome, &finalized);

                emit_session_write_pending(&self.event_service, &finalized);
                self.event_service
                    .emit(CodingAgentEvent::RuntimeCompactionCompleted {
                        operation_id,
                        turn_id,
                        summary: compaction.summary,
                        first_kept_message_id: compaction.first_kept_message_id,
                        tokens_before: compaction.tokens_before,
                    });
                emit_session_write_committed(&self.event_service, &finalized);
                self.emit_prompt_outcome_event(&outcome);
                Ok(outcome)
            }
            Err(error) => {
                let mut outcome = PromptTurnOutcome::Failed {
                    operation_id: operation_id.clone(),
                    turn_id: Some(turn_id),
                    error: error.clone(),
                    diagnostics: Vec::new(),
                };
                let finalized = session_service.fail_prompt_transaction(
                    context.take_transaction(),
                    operation_id.clone(),
                    error.code(),
                    error.to_string(),
                )?;
                apply_finalized_session_write(&mut outcome, &finalized);
                self.emit_session_write_events(&finalized);
                self.emit_prompt_outcome_event(&outcome);
                Ok(outcome)
            }
        }
    }

    fn reused_branch_summary_outcome(
        &self,
        options: &PromptTurnOptions,
        source_leaf_id: &str,
        target_leaf_id: &str,
    ) -> Result<Option<PromptTurnOutcome>, CodingSessionError> {
        let runtime = options
            .runtime()
            .cloned()
            .ok_or_else(|| CodingSessionError::Config {
                message: "branch summary options do not include a runtime snapshot".into(),
            })?;
        let SessionPersistence::Persistent(session_service) = &self.persistence else {
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
        Ok(Some(PromptTurnOutcome::Success {
            operation_id,
            turn_id,
            session_id: Some(session_service.session_id().to_owned()),
            leaf_id: session_service.active_leaf_id().map(str::to_owned),
            final_text: summary.clone(),
            final_message: branch_summary_final_message(&runtime, &summary),
            diagnostics: Vec::new(),
        }))
    }

    async fn summarize_branch_inner(
        &mut self,
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        let runtime = options
            .runtime()
            .cloned()
            .ok_or_else(|| CodingSessionError::Config {
                message: "branch summary options do not include a runtime snapshot".into(),
            })?;
        let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "branch summary without persistent session".into(),
            });
        };

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

        match self.flow_service.run_branch_summary(&mut context).await {
            Ok(branch_summary) => {
                let final_text = branch_summary_text(branch_summary);
                let mut outcome = PromptTurnOutcome::Success {
                    operation_id: operation_id.clone(),
                    turn_id,
                    session_id: Some(session_service.session_id().to_owned()),
                    leaf_id: session_service.active_leaf_id().map(str::to_owned),
                    final_text: final_text.clone(),
                    final_message: branch_summary_final_message(&runtime, &final_text),
                    diagnostics: Vec::new(),
                };
                let finalized = session_service
                    .commit_branch_summary_transaction(context.take_transaction(), operation_id)?;
                apply_finalized_session_write(&mut outcome, &finalized);
                self.emit_session_write_events(&finalized);
                Ok(outcome)
            }
            Err(error) => {
                let mut outcome = PromptTurnOutcome::Failed {
                    operation_id: operation_id.clone(),
                    turn_id: Some(turn_id),
                    error: error.clone(),
                    diagnostics: Vec::new(),
                };
                let finalized = session_service.fail_prompt_transaction(
                    context.take_transaction(),
                    operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                apply_finalized_session_write(&mut outcome, &finalized);
                self.emit_session_write_events(&finalized);
                Ok(outcome)
            }
        }
    }

    fn prepare_prompt_context(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnContext, CodingSessionError> {
        let event_service = self.event_service.clone();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let replay = session_service.replay()?;
                let transaction = session_service.begin_prompt_transaction();
                let operation_id = transaction.operation_id().to_owned();
                let turn_id = transaction.turn_id().to_owned();
                let mut context =
                    PromptTurnContext::new(PromptTurnIds::new(operation_id, turn_id), options);
                context.set_plugin_service(self.plugin_service.clone());
                context.set_session_id(session_service.session_id().to_owned());
                context.set_replay(replay);
                context.set_transaction(transaction);
                context.enable_live_events(event_service);
                Ok(context)
            }
            SessionPersistence::NonPersistent(state) => {
                let mut ids = SystemIdGenerator;
                let mut context = PromptTurnContext::new(
                    PromptTurnIds::new(ids.next_operation_id(), ids.next_turn_id()),
                    options,
                );
                context.set_plugin_service(self.plugin_service.clone());
                context
                    .set_non_persistent_session(state.runtime_id.clone(), state.transcript.clone());
                context.enable_live_events(event_service);
                Ok(context)
            }
        }
    }

    fn finalize_prompt_transaction(
        &mut self,
        context: &mut PromptTurnContext,
        outcome: &PromptTurnOutcome,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let operation_id = context.operation_id().to_owned();
        let transaction = context.take_transaction();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => match outcome {
                PromptTurnOutcome::Success { .. } => {
                    session_service.commit_prompt_transaction(transaction, operation_id)
                }
                PromptTurnOutcome::Aborted { reason, .. } => session_service
                    .abort_prompt_transaction(transaction, operation_id, reason.clone()),
                PromptTurnOutcome::Failed { error, .. } => session_service.fail_prompt_transaction(
                    transaction,
                    operation_id,
                    error.code(),
                    error.to_string(),
                ),
            },
            SessionPersistence::NonPersistent(state) => {
                if matches!(outcome, PromptTurnOutcome::Success { .. }) {
                    state
                        .transcript
                        .extend(context.completed_transcript_items());
                }
                Ok(SessionService::skip_prompt_transaction(
                    operation_id,
                    "session persistence disabled",
                ))
            }
        }
    }

    fn emit_coding_events_before_prompt_outcome(&self, events: &[CodingAgentEvent]) {
        for event in events {
            if is_prompt_outcome_event(event) {
                continue;
            }
            self.event_service.emit(event.clone());
        }
    }

    fn emit_session_write_events(&self, finalized: &FinalizedSessionWrite) {
        for event in &finalized.events {
            self.event_service.emit(event.clone());
        }
    }

    fn emit_prompt_outcome_event(&self, outcome: &PromptTurnOutcome) {
        match outcome {
            PromptTurnOutcome::Success {
                operation_id,
                turn_id,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
            }),
            PromptTurnOutcome::Aborted {
                operation_id,
                reason,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptAborted {
                operation_id: operation_id.clone(),
                reason: reason.clone(),
            }),
            PromptTurnOutcome::Failed {
                operation_id,
                error,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptFailed {
                operation_id: operation_id.clone(),
                error: error.clone(),
            }),
        }
    }

    #[cfg(test)]
    fn persistent_session_service(&self) -> &SessionService {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service,
            SessionPersistence::NonPersistent(_) => {
                panic!("expected persistent coding agent session")
            }
        }
    }
}

fn branch_summary_text(outcome: BranchSummaryOutcome) -> String {
    match outcome {
        BranchSummaryOutcome::Created { summary, .. } => summary,
        BranchSummaryOutcome::NoOp { reason } => reason,
    }
}

fn branch_summary_final_message(
    runtime: &RuntimeSnapshot,
    summary: &str,
) -> pi_ai::types::AssistantMessage {
    let mut message =
        pi_ai::types::AssistantMessage::empty(&runtime.model().api, &runtime.model().id);
    message.provider = Some(runtime.model().provider.clone());
    message.content.push(pi_ai::types::ContentBlock::Text {
        text: summary.to_owned(),
        text_signature: None,
    });
    message
}

fn is_prompt_outcome_event(event: &CodingAgentEvent) -> bool {
    matches!(
        event,
        CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::PromptFailed { .. }
            | CodingAgentEvent::PromptAborted { .. }
    )
}

fn apply_finalized_session_write(
    outcome: &mut PromptTurnOutcome,
    finalized: &FinalizedSessionWrite,
) {
    if let PromptTurnOutcome::Success {
        session_id,
        leaf_id,
        ..
    } = outcome
    {
        if let Some(finalized_session_id) = &finalized.session_id {
            *session_id = Some(finalized_session_id.clone());
        }
        *leaf_id = finalized.leaf_id.clone();
    }
}

fn emit_session_write_pending(event_service: &EventService, finalized: &FinalizedSessionWrite) {
    for event in &finalized.events {
        if matches!(event, CodingAgentEvent::SessionWritePending { .. }) {
            event_service.emit(event.clone());
        }
    }
}

fn emit_session_write_committed(event_service: &EventService, finalized: &FinalizedSessionWrite) {
    for event in &finalized.events {
        if matches!(
            event,
            CodingAgentEvent::SessionWriteCommitted { .. }
                | CodingAgentEvent::SessionWriteSkipped { .. }
        ) {
            event_service.emit(event.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
    };

    use async_stream::stream;
    use pi_agent_core::{AgentResources, AgentTool, AgentToolOutput};
    use pi_ai::providers::faux::{FauxProvider, FauxResponse, FauxToolCall};
    use pi_ai::registry;
    use pi_ai::registry::ApiProvider;
    use pi_ai::stream::EventStream;
    use pi_ai::types::{
        AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
        ModelInput, StopReason, StreamOptions,
    };

    use super::plugin_load_flow::{PluginLoadCandidate, PluginLoadManifest, PluginLoadOptions};
    use super::*;
    use crate::coding_session::session_log::event::{
        PersistedContentBlock, SessionEventData, SessionEventEnvelope,
    };
    use crate::coding_session::session_log::replay::{MessageStatus, TranscriptItem};
    use crate::plugins::{
        PluginError, PluginId, PluginMetadata, PluginRegistry, PluginSource, ToolProvider,
        ToolRegistrationHost,
    };
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

    fn prompt_options(api: &str, prompt: &str) -> PromptTurnOptions {
        prompt_options_with_tools(api, prompt, Vec::new())
    }

    fn prompt_options_with_tools(
        api: &str,
        prompt: &str,
        tools: Vec<AgentTool>,
    ) -> PromptTurnOptions {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: prompt.into(),
            model: model(api),
            api_key: None,
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools,
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text(prompt.into()),
        })
    }

    fn compact_options(api: &str, custom_instructions: Option<&str>) -> PromptTurnOptions {
        PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
            prompt: String::new(),
            model: model(api),
            api_key: None,
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Compact {
                custom_instructions: custom_instructions.map(str::to_owned),
            },
        })
    }

    fn echo_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object"}),
            execution_mode: None,
            execute: Arc::new(|args, _on_update| {
                let text = args
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_owned();
                Box::pin(async move {
                    Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                        text: format!("echo: {text}"),
                        text_signature: None,
                    }]))
                })
            }),
        }
    }

    struct SessionPluginToolProvider;

    impl ToolProvider for SessionPluginToolProvider {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                PluginId::new("session-plugin-tool"),
                "Session Plugin Tool",
                "1.0.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            Ok(vec![AgentTool::new_text(
                "plugin_echo",
                "echoes plugin input",
                serde_json::json!({"type": "object"}),
                |_args| async { Ok("plugin echo".to_owned()) },
            )])
        }
    }

    struct RecordingProvider {
        contexts: Arc<Mutex<Vec<Context>>>,
        response: String,
    }

    impl RecordingProvider {
        fn new(contexts: Arc<Mutex<Vec<Context>>>, response: impl Into<String>) -> Self {
            Self {
                contexts,
                response: response.into(),
            }
        }
    }

    impl ApiProvider for RecordingProvider {
        fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            self.contexts.lock().unwrap().push(ctx);
            let model_id = model.id.clone();
            let response = self.response.clone();
            Box::pin(stream! {
                let mut message = AssistantMessage::empty("recording", &model_id);
                message.provider = Some("recording".into());
                message.content.push(ContentBlock::Text {
                    text: response,
                    text_signature: None,
                });
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message,
                };
            })
        }
    }

    #[tokio::test]
    async fn load_plugins_updates_session_runtime_and_emits_capability_events() {
        let api = "coding-session-plugin-load-owner";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            api,
            Arc::new(RecordingProvider::new(contexts.clone(), "plugin loaded")),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_plugin_load_owner")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(SessionPluginToolProvider));
        let options = PluginLoadOptions::new()
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new(
                    "session-plugin",
                    "Session Plugin",
                    "1.0.0",
                    PluginSource::FirstParty,
                ),
                registry,
            ))
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new("", "Invalid Plugin", "1.0.0", PluginSource::Project),
                PluginRegistry::new(),
            ));
        let mut events = session.subscribe();

        let outcome = session.load_plugins(options).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["session-plugin"]);
        assert_eq!(outcome.diagnostics.len(), 1);
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(
            emitted_events.iter().any(|event| matches!(
                event,
                CodingAgentEvent::Diagnostic { message, .. }
                    if message.contains("plugin id must not be empty")
            )),
            "{emitted_events:#?}"
        );
        assert!(
            emitted_events
                .iter()
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged))
        );

        session
            .prompt(prompt_options(api, "use plugin"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        let tools = contexts[0].tools.as_ref().unwrap();
        assert!(tools.iter().any(|tool| tool.name == "plugin_echo"));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn reload_plugins_discovers_default_project_and_user_roots() {
        let _guard = crate::test_support::env_lock();
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("project");
        let global = temp.path().join("global");
        let previous_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
        let project_plugin = cwd.join(".pi-rust/plugins/project-lua");
        let user_plugin = global.join("plugins/user-lua");
        fs::create_dir_all(&project_plugin).unwrap();
        fs::create_dir_all(&user_plugin).unwrap();
        fs::write(
            project_plugin.join("plugin.toml"),
            r#"
id = "project-lua"
name = "Project Lua"
version = "0.1.0"
runtime = "lua"
"#,
        )
        .unwrap();
        fs::write(
            user_plugin.join("plugin.toml"),
            r#"
id = "user-lua"
name = "User Lua"
version = "0.1.0"
runtime = "lua"
"#,
        )
        .unwrap();
        unsafe {
            std::env::set_var("PI_RUST_DIR", &global);
        }
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_cwd(&cwd)
                .with_session_id("sess_plugin_reload_defaults")
                .with_session_log_root(temp.path().join("sessions")),
        )
        .await
        .unwrap();
        let mut events = session.subscribe();

        let outcome = session.reload_plugins().await.unwrap();

        assert!(outcome.loaded_plugin_ids.is_empty());
        assert_eq!(outcome.diagnostics.len(), 2);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.plugin_id.as_deref() == Some("project-lua"))
        );
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.plugin_id.as_deref() == Some("user-lua"))
        );
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_eq!(
            emitted_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::Diagnostic { .. }))
                .count(),
            2
        );
        assert!(
            emitted_events
                .iter()
                .any(|event| matches!(event, CodingAgentEvent::CapabilityChanged))
        );
        unsafe {
            match previous_pi_rust_dir {
                Some(value) => std::env::set_var("PI_RUST_DIR", value),
                None => std::env::remove_var("PI_RUST_DIR"),
            }
        }
    }

    #[tokio::test]
    async fn prompt_runs_flow_and_commits_session_events() {
        let api = "coding-session-prompt";
        registry::register(api, Arc::new(FauxProvider::simple_text("session answer")));
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_prompt")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        let leaf_id = match &outcome {
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(leaf_id),
                ..
            } if final_text == "session answer" && session_id == "sess_prompt" => leaf_id.clone(),
            other => panic!("expected successful prompt with committed leaf, got {other:?}"),
        };
        assert!(leaf_id.starts_with("leaf_"));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptStarted { .. })
        ));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTurnStarted { .. })
        ));
        let remaining_events =
            std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &remaining_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_completed",
            ],
        );
        assert_eq!(
            remaining_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptCompleted { .. }))
                .count(),
            1
        );

        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(replay.active_leaf_id.as_deref(), Some(leaf_id.as_str()));
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::UserInput {
                    turn_id,
                    text,
                },
                TranscriptItem::AssistantMessage {
                    content,
                    status: MessageStatus::Completed,
                    ..
                },
            ] if turn_id == outcome_turn_id(&outcome)
                && text == "hello"
                && content == &vec![PersistedContentBlock::Text {
                    text: "session answer".into(),
                }]
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_prompt/events.jsonl")).unwrap();
        assert!(!event_log.contains("\"message.delta\""));
        assert!(event_log.contains("\"kind\":\"message.completed\""));
        assert!(event_log.contains("\"content\""));
        let committed_leaf = event_log
            .lines()
            .filter_map(|line| serde_json::from_str::<SessionEventEnvelope>(line).ok())
            .find_map(|event| match event.data {
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some(leaf_id),
                } => Some(leaf_id),
                _ => None,
            })
            .unwrap();
        assert_eq!(committed_leaf, leaf_id);
        let hydrated = session.hydrate_current().unwrap().unwrap();
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        let summaries = CodingAgentSession::list(options).unwrap();
        assert_eq!(
            summaries[0].active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        assert_eq!(session.view().session_id, "sess_prompt");
        registry::unregister(api);
    }

    #[tokio::test]
    async fn prompt_requires_runtime_backed_options() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_missing_runtime")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        let error = session
            .prompt(PromptTurnOptions::new(PromptInvocation::Text(
                "hello".into(),
            )))
            .await
            .unwrap_err();

        assert_eq!(error.code(), "config");
        assert!(error.to_string().contains("runtime snapshot"));
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .transcript
                .is_empty()
        );
        assert!(
            session
                .hydrate_current()
                .unwrap()
                .unwrap()
                .summary
                .active_leaf_id
                .is_none()
        );
    }

    #[tokio::test]
    async fn non_persistent_constructor_does_not_create_session_files() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        assert!(session.view().session_id.starts_with("runtime_sess_"));
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn non_persistent_prompt_emits_skipped_write_before_completion() {
        let api = "coding-session-non-persistent-prompt";
        registry::register(api, Arc::new(FauxProvider::simple_text("transient answer")));
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: None,
                leaf_id: None,
                ..
            } if final_text == "transient answer"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &["session_write_skipped", "prompt_completed"],
        );
        assert!(emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SessionWriteSkipped { reason, .. }
                if reason == "session persistence disabled"
        )));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn non_persistent_prompt_hydrates_owner_lifetime_transcript() {
        let first_api = "coding-session-non-persistent-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::simple_text("first answer")),
        );
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();

        session
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-non-persistent-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );

        session
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    #[tokio::test]
    async fn prompt_does_not_duplicate_failure_event_from_agent_error() {
        let api = "coding-session-prompt-error";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("partial", StopReason::Error),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_error")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(outcome, PromptTurnOutcome::Failed { .. }));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_failed",
            ],
        );
        assert_eq!(
            emitted_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptFailed { .. }))
                .count(),
            1
        );
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("operation")
                    && diagnostic.message.contains("failed"))
        );
        registry::unregister(api);
    }

    #[tokio::test]
    async fn branch_summary_persistent_session_records_model_summary() {
        let api = "coding-session-branch-summary";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
                FauxProvider::text_call("model branch summary", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_branch_summary_owner")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let root_leaf = match session
            .prompt(prompt_options(api, "root question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };
        let mut events = session.subscribe();

        let outcome = session
            .summarize_branch(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
                Some("keep branch decisions".into()),
            )
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(_),
                ..
            } if final_text.contains("model branch summary")
                && session_id == "sess_branch_summary_owner"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &["session_write_pending", "session_write_committed"],
        );
        let replay = session.persistent_session_service().replay().unwrap();
        assert!(matches!(
            replay.transcript.last(),
            Some(TranscriptItem::BranchSummary {
                summary,
                source_leaf_id,
                target_leaf_id,
            }) if summary.contains("model branch summary")
                && source_leaf_id == &branch_leaf
                && target_leaf_id == &root_leaf
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_branch_summary_owner/events.jsonl"))
                .unwrap();
        assert!(event_log.contains("branch.summary.created"));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn branch_summary_navigation_reuses_existing_summary_without_rewriting_session() {
        let api = "coding-session-branch-summary-navigation-reuse";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("root answer", StopReason::Stop),
                FauxProvider::text_call("branch answer", StopReason::Stop),
                FauxProvider::text_call("model branch summary", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_branch_summary_navigation_reuse")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let root_leaf = match session
            .prompt(prompt_options(api, "root question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected root prompt success, got {other:?}"),
        };
        let branch_leaf = match session
            .prompt(prompt_options(api, "branch question"))
            .await
            .unwrap()
        {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected branch prompt success, got {other:?}"),
        };
        session
            .summarize_branch(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
                None,
            )
            .await
            .unwrap();
        let event_log_path = temp
            .path()
            .join("sess_branch_summary_navigation_reuse/events.jsonl");
        let event_log_before = std::fs::read_to_string(&event_log_path).unwrap();
        let summary_count_before = event_log_before.matches("branch.summary.created").count();
        let mut events = session.subscribe();

        let outcome = session
            .summarize_branch_for_navigation(
                prompt_options(api, ""),
                branch_leaf.clone(),
                root_leaf.clone(),
            )
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(active_leaf),
                ..
            } if final_text.contains("model branch summary")
                && session_id == "sess_branch_summary_navigation_reuse"
                && active_leaf.as_str() == branch_leaf.as_str()
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert!(emitted_events.is_empty(), "{emitted_events:#?}");
        let event_log_after = std::fs::read_to_string(&event_log_path).unwrap();
        assert_eq!(event_log_after, event_log_before);
        assert_eq!(summary_count_before, 1);
        assert_eq!(event_log_after.matches("branch.summary.created").count(), 1);
        registry::unregister(api);
    }

    #[tokio::test]
    async fn compact_persistent_session_records_events_and_replays_summary() {
        let api = "coding-session-compact";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("first answer", StopReason::Stop),
                FauxProvider::text_call("summary from compact", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_compact")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        session
            .prompt(prompt_options(api, "first question"))
            .await
            .unwrap();
        let mut events = session.subscribe();

        let outcome = session
            .compact(compact_options(api, Some("keep decisions")))
            .await
            .unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(_),
                ..
            } if final_text == "summary from compact" && session_id == "sess_compact"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "runtime_compaction_completed",
                "session_write_committed",
                "prompt_completed",
            ],
        );
        assert!(emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::RuntimeCompactionCompleted {
                summary,
                tokens_before,
                ..
            } if summary == "summary from compact" && *tokens_before > 0
        )));

        let replay = session.persistent_session_service().replay().unwrap();
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::CompactionSummary {
                    summary,
                    first_kept_message_id,
                    tokens_before,
                },
                TranscriptItem::AssistantMessage {
                    content,
                    status: MessageStatus::Completed,
                    ..
                },
            ] if summary == "summary from compact"
                && first_kept_message_id.starts_with("msg_")
                && *tokens_before > 0
                && content == &vec![PersistedContentBlock::Text {
                    text: "first answer".into(),
                }]
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_compact/events.jsonl")).unwrap();
        assert!(event_log.contains("session.compaction.started"));
        assert!(event_log.contains("session.compaction.completed"));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn compact_summary_failure_records_failure_without_folding_replay() {
        let api = "coding-session-compact-summary-failure";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("first answer", StopReason::Stop),
                FauxProvider::text_call("", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_compact_failure")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let prompt_outcome = session
            .prompt(prompt_options(api, "first question"))
            .await
            .unwrap();
        let active_leaf_before = match prompt_outcome {
            PromptTurnOutcome::Success {
                leaf_id: Some(leaf_id),
                ..
            } => leaf_id,
            other => panic!("expected prompt success, got {other:?}"),
        };
        let mut events = session.subscribe();

        let outcome = session.compact(compact_options(api, None)).await.unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Failed { error, .. }
                if error.code() == "provider" && error.to_string().contains("empty summary")
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_failed",
            ],
        );
        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(
            replay.active_leaf_id.as_deref(),
            Some(active_leaf_before.as_str())
        );
        assert!(
            replay
                .transcript
                .iter()
                .all(|item| !matches!(item, TranscriptItem::CompactionSummary { .. }))
        );
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::UserInput { text, .. },
                TranscriptItem::AssistantMessage { content, .. },
                TranscriptItem::Diagnostic { message, .. },
            ] if text == "first question"
                && content == &vec![PersistedContentBlock::Text {
                    text: "first answer".into(),
                }]
                && message.contains("empty summary")
        ));
        let event_log =
            std::fs::read_to_string(temp.path().join("sess_compact_failure/events.jsonl")).unwrap();
        assert!(event_log.contains("session.compaction.started"));
        assert!(event_log.contains("operation.failed"));
        assert!(!event_log.contains("session.compaction.completed"));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_transcript_when_opening_session() {
        let first_api = "coding-session-hydrate-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::simple_text("first answer")),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-hydrate-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        let outcome = opened
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        assert!(matches!(
            outcome,
            PromptTurnOutcome::Success { final_text, .. } if final_text == "second answer"
        ));
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_tool_calls_when_opening_session() {
        let first_api = "coding-session-hydrate-tool-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: vec!["I will use echo.".into()],
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "toolu_1".into(),
                            name: "echo".into(),
                            deltas: Vec::new(),
                            final_arguments: serde_json::json!({"text": "hi"}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("tool final", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_tool_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options_with_tools(
                first_api,
                "use the tool",
                vec![echo_tool()],
            ))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-hydrate-tool-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        opened
            .prompt(prompt_options(second_api, "continue"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 5);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "use the tool".into(),
                    text_signature: None,
                }]
        ));
        let tool_call_id = match &contexts[0].messages[1] {
            Message::Assistant { content } => match content.as_slice() {
                [
                    ContentBlock::Text { text, .. },
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    },
                ] => {
                    assert_eq!(text, "I will use echo.");
                    assert_eq!(name, "echo");
                    assert_eq!(arguments, &serde_json::json!({"text": "hi"}));
                    id.clone()
                }
                other => panic!("unexpected assistant content: {other:?}"),
            },
            other => panic!("unexpected hydrated assistant message: {other:?}"),
        };
        assert!(matches!(
            &contexts[0].messages[2],
            Message::ToolResult {
                tool_call_id: result_tool_call_id,
                tool_name: Some(tool_name),
                is_error: Some(false),
                content,
            } if result_tool_call_id == &tool_call_id
                && tool_name == "echo"
                && content == &vec![ContentBlock::Text {
                    text: "echo: hi".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[3],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "tool final".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[4],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "continue".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    #[tokio::test]
    async fn export_current_html_writes_rust_native_session_transcript() {
        let api = "coding-session-export-html";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: vec!["I will use echo.".into()],
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "toolu_export".into(),
                            name: "echo".into(),
                            deltas: Vec::new(),
                            final_arguments: serde_json::json!({"text": "<hi>"}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("tool final <done>", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_export_html")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options).await.unwrap();
        session
            .prompt(prompt_options_with_tools(
                api,
                "use <tool>",
                vec![echo_tool()],
            ))
            .await
            .unwrap();
        let output = temp.path().join("exports/session.html");

        let exported = session.export_current_html(&output).unwrap();

        assert_eq!(exported, output);
        let html = std::fs::read_to_string(&exported).unwrap();
        assert!(html.contains("<!doctype html>"), "{html}");
        assert!(html.contains("sess_export_html"), "{html}");
        assert!(html.contains("use &lt;tool&gt;"), "{html}");
        assert!(html.contains("I will use echo."), "{html}");
        assert!(html.contains("Tool: echo"), "{html}");
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(html.contains("echo: &lt;hi&gt;"), "{html}");
        assert!(html.contains("tool final &lt;done&gt;"), "{html}");
        registry::unregister(api);
    }

    #[tokio::test]
    async fn export_current_html_rejects_jsonl_target() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_export_jsonl")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let output = temp.path().join("session.jsonl");

        let error = session.export_current_html(&output).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: JSONL session export is no longer supported"
        );
        assert!(!output.exists());
    }

    fn outcome_turn_id(outcome: &PromptTurnOutcome) -> &str {
        match outcome {
            PromptTurnOutcome::Success { turn_id, .. } => turn_id,
            _ => panic!("expected success outcome"),
        }
    }

    fn assert_event_order(events: &[CodingAgentEvent], expected: &[&str]) {
        let observed = events.iter().map(event_kind).collect::<Vec<_>>();
        let mut next_index = 0;
        for kind in observed {
            if next_index < expected.len() && kind == expected[next_index] {
                next_index += 1;
            }
        }
        assert_eq!(
            next_index,
            expected.len(),
            "did not observe event order {expected:?}"
        );
    }

    fn event_kind(event: &CodingAgentEvent) -> &'static str {
        match event {
            CodingAgentEvent::SessionOpened { .. } => "session_opened",
            CodingAgentEvent::SessionWritePending { .. } => "session_write_pending",
            CodingAgentEvent::SessionWriteCommitted { .. } => "session_write_committed",
            CodingAgentEvent::SessionWriteSkipped { .. } => "session_write_skipped",
            CodingAgentEvent::PromptStarted { .. } => "prompt_started",
            CodingAgentEvent::AgentTurnStarted { .. } => "agent_turn_started",
            CodingAgentEvent::ProviderRequestStarted { .. } => "provider_request_started",
            CodingAgentEvent::AssistantMessageStarted { .. } => "assistant_message_started",
            CodingAgentEvent::AssistantMessageDelta { .. } => "assistant_message_delta",
            CodingAgentEvent::AssistantThinkingDelta { .. } => "assistant_thinking_delta",
            CodingAgentEvent::AssistantMessageCompleted { .. } => "assistant_message_completed",
            CodingAgentEvent::ToolCallStarted { .. } => "tool_call_started",
            CodingAgentEvent::ToolCallUpdated { .. } => "tool_call_updated",
            CodingAgentEvent::ToolCallCompleted { .. } => "tool_call_completed",
            CodingAgentEvent::ToolCallFailed { .. } => "tool_call_failed",
            CodingAgentEvent::RuntimeCompactionCompleted { .. } => "runtime_compaction_completed",
            CodingAgentEvent::PromptCompleted { .. } => "prompt_completed",
            CodingAgentEvent::PromptFailed { .. } => "prompt_failed",
            CodingAgentEvent::PromptAborted { .. } => "prompt_aborted",
            CodingAgentEvent::Diagnostic { .. } => "diagnostic",
            CodingAgentEvent::CapabilityChanged => "capability_changed",
        }
    }
}
