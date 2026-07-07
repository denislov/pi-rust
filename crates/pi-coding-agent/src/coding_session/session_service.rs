use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use pi_agent_core::transcript::{SessionEntry, SessionTreeNode, StoredAgentMessage};
use pi_ai::types::ContentBlock;

use super::event_service::EventService;
use super::export_flow::{ExportContext, ExportOptions};
use super::prompt::{PromptTurnContext, PromptTurnOutcome, PromptTurnTransaction};
use super::session_log::event::{
    OperationKind, PersistedContentBlock, PersistedDelegationRuntimeSeed,
    PersistedDelegationStatus, PersistedPluginDiagnostic, SessionEventData, SessionEventEnvelope,
};
use super::session_log::id::{Clock, IdGenerator, SystemClock, SystemIdGenerator};
use super::session_log::replay::{MessageStatus, SessionReplay, ToolCallStatus, TranscriptItem};
use super::session_log::store::{
    CreateSessionOptions, ManifestPatch, SessionHandle, SessionLogStore, SessionSummary,
};
use super::session_log::transaction::TurnTransaction;
use super::{
    CodingAgentEvent, CodingAgentSessionDiagnostic, CodingAgentSessionHydration,
    CodingAgentSessionOptions, CodingAgentSessionSummary, CodingAgentSessionTranscriptItem,
    CodingAgentSessionTree, CodingAgentSessionView, CodingSessionError, ProfileId, ProfileKind,
    SelfHealingEditOutcome, SelfHealingEditRepairAttempt,
};

#[derive(Debug)]
pub(crate) struct SessionService {
    #[allow(dead_code)]
    store: SessionLogStore,
    handle: SessionHandle,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedSessionWrite {
    pub(crate) events: Vec<CodingAgentEvent>,
    pub(crate) session_id: Option<String>,
    pub(crate) leaf_id: Option<String>,
}

#[derive(Debug)]
pub(crate) enum SessionPersistence {
    Persistent(SessionService),
    NonPersistent(TransientSessionState),
}

#[derive(Debug)]
pub(crate) struct TransientSessionState {
    pub(crate) runtime_id: String,
    pub(crate) transcript: Vec<TranscriptItem>,
    pub(crate) default_agent_profile_id: ProfileId,
}

impl TransientSessionState {
    pub(crate) fn new(default_agent_profile_id: ProfileId) -> Self {
        let mut ids = SystemIdGenerator;
        Self {
            runtime_id: format!("runtime_{}", ids.next_session_id()),
            transcript: Vec::new(),
            default_agent_profile_id,
        }
    }

    pub(crate) fn finalize_prompt_transaction(
        &mut self,
        context: &PromptTurnContext,
        outcome: &PromptTurnOutcome,
    ) -> FinalizedSessionWrite {
        if outcome.is_success() {
            self.transcript.extend(context.completed_transcript_items());
        }
        SessionService::skip_prompt_transaction(
            context.operation_id().to_owned(),
            "session persistence disabled",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionCopyKind {
    Clone,
    Fork,
}

impl SessionService {
    pub(crate) fn create(options: &CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        let session_id = match options.session_id() {
            Some(session_id) => normalize_session_id(session_id, "session id")?,
            None => ids.next_session_id(),
        };
        Self::create_with_id(
            store,
            session_id,
            &mut ids,
            &clock,
            option_cwd_string(options),
            option_default_agent_profile_id(options),
        )
    }

    pub(crate) fn open(options: &CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        let target = open_target(options)?;
        let handle = store.open_session(&target)?;

        Ok(Self { store, handle })
    }

    pub(crate) fn open_or_create(
        options: &CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "open-or-create requires a session id, not a session path".into(),
            });
        }
        let session_id = options
            .session_id()
            .ok_or_else(|| CodingSessionError::Input {
                message: "open-or-create requires a session id".into(),
            })
            .and_then(|session_id| normalize_session_id(session_id, "session id"))?;
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);

        if let Some(handle) = store.try_open_session_id(&session_id)? {
            return Ok(Self { store, handle });
        }

        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        Self::create_with_id(
            store,
            session_id,
            &mut ids,
            &clock,
            option_cwd_string(options),
            option_default_agent_profile_id(options),
        )
    }

    pub(crate) fn list(
        options: &CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        let root = resolve_session_log_root(options)?;
        let store = SessionLogStore::new(root);
        let cwd = option_cwd_string(options);
        let mut summaries = Vec::new();
        for summary in store.list_sessions()? {
            if let Some(cwd) = cwd.as_deref() {
                let handle = store.open_session(&summary.session_dir)?;
                let replay = store.replay_session(&handle)?;
                if replay.cwd.as_deref() != Some(cwd) {
                    continue;
                }
            }
            summaries.push(CodingAgentSessionSummary::from(summary));
        }
        Ok(summaries)
    }

    pub(crate) fn hydrate(
        options: &CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        Self::open(options)?.hydrated_view()
    }

    pub(crate) fn tree_view(
        options: &CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionTree, CodingSessionError> {
        Self::open(options)?.leaf_tree_view()
    }

    fn leaf_tree_view(&self) -> Result<CodingAgentSessionTree, CodingSessionError> {
        let events = self.store.read_events(&self.handle)?;
        Ok(build_leaf_tree(
            &events,
            self.handle.manifest().active_leaf_id.clone(),
        ))
    }

    pub(crate) fn clone_current(&self) -> Result<Self, CodingSessionError> {
        self.copy_to_new_session(None, SessionCopyKind::Clone)
    }

    pub(crate) fn fork_current(
        &self,
        target_leaf_id: Option<&str>,
    ) -> Result<Self, CodingSessionError> {
        self.copy_to_new_session(target_leaf_id, SessionCopyKind::Fork)
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.handle.manifest().session_id
    }

    pub(crate) fn active_leaf_id(&self) -> Option<&str> {
        self.handle.manifest().active_leaf_id.as_deref()
    }

    pub(crate) fn default_agent_profile_id(&self) -> &ProfileId {
        &self.handle.manifest().default_agent_profile_id
    }

    pub(crate) fn set_default_agent_profile_id(
        &mut self,
        profile_id: ProfileId,
    ) -> Result<(), CodingSessionError> {
        let session_id = self.session_id().to_owned();
        self.store.update_manifest(
            &self.handle,
            ManifestPatch::new()
                .updated_at(SystemClock.now_rfc3339())
                .default_agent_profile_id(profile_id),
        )?;
        self.handle = self.store.open_session_id(&session_id)?;
        Ok(())
    }

    pub(crate) fn branch_summary_for(
        &self,
        source_leaf_id: &str,
        target_leaf_id: &str,
    ) -> Result<Option<String>, CodingSessionError> {
        let source_leaf_id = normalize_leaf_id(source_leaf_id)?;
        let target_leaf_id = normalize_leaf_id(target_leaf_id)?;
        Ok(self
            .replay()?
            .transcript
            .into_iter()
            .rev()
            .find_map(|item| match item {
                TranscriptItem::BranchSummary {
                    summary,
                    source_leaf_id: summary_source_leaf_id,
                    target_leaf_id: summary_target_leaf_id,
                } if summary_source_leaf_id == source_leaf_id
                    && summary_target_leaf_id == target_leaf_id =>
                {
                    Some(summary)
                }
                _ => None,
            }))
    }

    pub(crate) fn record_delegation_confirmation_requested(
        &mut self,
        source_operation_id: String,
        turn_id: String,
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        reason: String,
        runtime_seed: PersistedDelegationRuntimeSeed,
    ) -> Result<(), CodingSessionError> {
        self.append_durable_session_event(
            Some(source_operation_id.clone()),
            Some(turn_id.clone()),
            SessionEventData::DelegationConfirmationRequested {
                source_operation_id,
                turn_id,
                tool_call_id,
                requesting_profile_id,
                target_kind,
                target_id,
                task,
                reason,
                runtime_seed,
            },
        )
    }

    pub(crate) fn record_delegation_confirmation_approved(
        &mut self,
        source_operation_id: String,
        tool_call_id: String,
        approval_operation_id: String,
    ) -> Result<(), CodingSessionError> {
        self.append_durable_session_event(
            Some(source_operation_id.clone()),
            None,
            SessionEventData::DelegationConfirmationApproved {
                source_operation_id,
                tool_call_id,
                approval_operation_id,
            },
        )
    }

    pub(crate) fn record_delegation_confirmation_rejected(
        &mut self,
        source_operation_id: String,
        tool_call_id: String,
        reason: String,
    ) -> Result<(), CodingSessionError> {
        self.append_durable_session_event(
            Some(source_operation_id.clone()),
            None,
            SessionEventData::DelegationConfirmationRejected {
                source_operation_id,
                tool_call_id,
                reason,
            },
        )
    }

    #[allow(dead_code)]
    pub(crate) fn switch_active_leaf(
        &mut self,
        target_leaf_id: &str,
    ) -> Result<(), CodingSessionError> {
        let target_leaf_id = normalize_leaf_id(target_leaf_id)?;
        let events = self.store.read_events(&self.handle)?;
        if committed_leaf_cutoff(&events, &target_leaf_id).is_none() {
            return Err(CodingSessionError::Session {
                message: format!("leaf id not found in session: {target_leaf_id}"),
            });
        }

        let session_id = self.session_id().to_owned();
        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        let updated_at = clock.now_rfc3339();
        let event = SessionEventEnvelope::new(
            session_id.clone(),
            ids.next_event_id(),
            updated_at.clone(),
            SessionEventData::ActiveLeafChanged {
                leaf_id: target_leaf_id.clone(),
            },
        );
        self.store.append_events(&self.handle, &[event])?;
        self.store.update_manifest(
            &self.handle,
            ManifestPatch::new()
                .updated_at(updated_at)
                .active_leaf_id(Some(target_leaf_id)),
        )?;
        self.handle = self.store.open_session_id(&session_id)?;
        Ok(())
    }

    pub(crate) fn begin_prompt_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::Prompt,
        )
    }

    pub(crate) fn begin_manual_compaction_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::ManualCompaction,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn begin_branch_summary_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::BranchSummary,
        )
    }

    pub(crate) fn begin_plugin_load_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::PluginLoad,
        )
    }

    pub(crate) fn begin_self_healing_edit_transaction(&self) -> PromptTurnTransaction {
        TurnTransaction::begin(
            &self.store,
            self.handle.clone(),
            SystemIdGenerator,
            SystemClock,
            OperationKind::SelfHealingEdit,
        )
    }

    pub(crate) fn finalize_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        outcome: &PromptTurnOutcome,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let operation_id = operation_id.into();
        match outcome {
            PromptTurnOutcome::Success { .. } => {
                self.commit_prompt_transaction(transaction, operation_id)
            }
            PromptTurnOutcome::Aborted { reason, .. } => {
                self.abort_prompt_transaction(transaction, operation_id, reason.clone())
            }
            PromptTurnOutcome::Failed { error, .. } => self.fail_prompt_transaction(
                transaction,
                operation_id,
                error.code(),
                error.to_string(),
            ),
        }
    }

    pub(crate) fn commit_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let fallback_operation_id = operation_id.into();
        let Some(mut transaction) = transaction else {
            return Ok(Self::skipped_write(
                fallback_operation_id,
                "no active prompt transaction",
            ));
        };

        let operation_id = transaction.operation_id().to_owned();
        let session_id = self.session_id().to_owned();
        let new_leaf_id = Some(Self::next_leaf_id());
        let mut events = vec![EventService::session_write_pending_event(
            operation_id.clone(),
        )];
        transaction.commit(new_leaf_id.clone())?;
        self.handle = self.store.open_session_id(&session_id)?;
        events.push(EventService::session_write_committed_event(
            operation_id,
            session_id.clone(),
        ));
        Ok(FinalizedSessionWrite {
            events,
            session_id: Some(session_id),
            leaf_id: new_leaf_id,
        })
    }

    pub(crate) fn fail_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let fallback_operation_id = operation_id.into();
        let Some(mut transaction) = transaction else {
            return Ok(Self::skipped_write(
                fallback_operation_id,
                "no active prompt transaction",
            ));
        };

        let operation_id = transaction.operation_id().to_owned();
        let session_id = self.session_id().to_owned();
        let mut events = vec![EventService::session_write_pending_event(
            operation_id.clone(),
        )];
        transaction.fail(error_code, message)?;
        events.push(EventService::session_write_committed_event(
            operation_id,
            session_id.clone(),
        ));
        Ok(FinalizedSessionWrite {
            events,
            session_id: Some(session_id),
            leaf_id: None,
        })
    }

    pub(crate) fn commit_manual_compaction_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.commit_non_leaf_transaction(
            transaction,
            operation_id,
            "no active manual compaction transaction",
        )
    }

    pub(crate) fn commit_branch_summary_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.commit_non_leaf_transaction(
            transaction,
            operation_id,
            "no active branch summary transaction",
        )
    }

    pub(crate) fn commit_plugin_load_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.commit_non_leaf_transaction(
            transaction,
            operation_id,
            "no active plugin load transaction",
        )
    }

    pub(crate) fn fail_plugin_load_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.fail_non_leaf_transaction(
            transaction,
            operation_id,
            error_code,
            message,
            "no active plugin load transaction",
        )
    }

    pub(crate) fn commit_self_healing_edit_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.commit_non_leaf_transaction(
            transaction,
            operation_id,
            "no active self-healing edit transaction",
        )
    }

    pub(crate) fn fail_self_healing_edit_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        self.fail_non_leaf_transaction(
            transaction,
            operation_id,
            error_code,
            message,
            "no active self-healing edit transaction",
        )
    }

    fn commit_non_leaf_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        missing_transaction_reason: &'static str,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let fallback_operation_id = operation_id.into();
        let Some(mut transaction) = transaction else {
            return Ok(Self::skipped_write(
                fallback_operation_id,
                missing_transaction_reason,
            ));
        };

        let operation_id = transaction.operation_id().to_owned();
        let session_id = self.session_id().to_owned();
        let mut events = vec![EventService::session_write_pending_event(
            operation_id.clone(),
        )];
        transaction.commit(None)?;
        self.store.update_manifest(
            &self.handle,
            ManifestPatch::new().updated_at(SystemClock.now_rfc3339()),
        )?;
        self.handle = self.store.open_session_id(&session_id)?;
        events.push(EventService::session_write_committed_event(
            operation_id,
            session_id.clone(),
        ));
        Ok(FinalizedSessionWrite {
            events,
            session_id: Some(session_id),
            leaf_id: self.handle.manifest().active_leaf_id.clone(),
        })
    }

    fn fail_non_leaf_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        error_code: impl Into<String>,
        message: impl Into<String>,
        missing_transaction_reason: &'static str,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let fallback_operation_id = operation_id.into();
        let Some(mut transaction) = transaction else {
            return Ok(Self::skipped_write(
                fallback_operation_id,
                missing_transaction_reason,
            ));
        };

        let operation_id = transaction.operation_id().to_owned();
        let session_id = self.session_id().to_owned();
        let mut events = vec![EventService::session_write_pending_event(
            operation_id.clone(),
        )];
        transaction.fail(error_code, message)?;
        self.store.update_manifest(
            &self.handle,
            ManifestPatch::new().updated_at(SystemClock.now_rfc3339()),
        )?;
        self.handle = self.store.open_session_id(&session_id)?;
        events.push(EventService::session_write_committed_event(
            operation_id,
            session_id.clone(),
        ));
        Ok(FinalizedSessionWrite {
            events,
            session_id: Some(session_id),
            leaf_id: self.handle.manifest().active_leaf_id.clone(),
        })
    }

    pub(crate) fn record_plugin_load_completed(
        transaction: &mut PromptTurnTransaction,
        loaded_plugin_ids: Vec<String>,
        diagnostics: Vec<PersistedPluginDiagnostic>,
        capability_changed: bool,
    ) -> Result<(), CodingSessionError> {
        transaction.record_plugin_load_completed(loaded_plugin_ids, diagnostics, capability_changed)
    }

    pub(crate) fn record_self_healing_edit_started(
        transaction: &mut PromptTurnTransaction,
        path: String,
        replacements: usize,
    ) -> Result<(), CodingSessionError> {
        transaction.record_self_healing_edit_started(path, replacements)
    }

    pub(crate) fn record_self_healing_edit_repair_attempted(
        transaction: &mut PromptTurnTransaction,
        path: &str,
        repair: &SelfHealingEditRepairAttempt,
    ) -> Result<(), CodingSessionError> {
        transaction.record_self_healing_edit_repair_attempted(path, repair)
    }

    pub(crate) fn record_self_healing_edit_completed(
        transaction: &mut PromptTurnTransaction,
        outcome: &SelfHealingEditOutcome,
    ) -> Result<(), CodingSessionError> {
        transaction.record_self_healing_edit_completed(outcome)
    }

    #[allow(dead_code)]
    pub(crate) fn abort_prompt_transaction(
        &mut self,
        transaction: Option<PromptTurnTransaction>,
        operation_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let fallback_operation_id = operation_id.into();
        let Some(mut transaction) = transaction else {
            return Ok(Self::skipped_write(
                fallback_operation_id,
                "no active prompt transaction",
            ));
        };

        let operation_id = transaction.operation_id().to_owned();
        let session_id = self.session_id().to_owned();
        let mut events = vec![EventService::session_write_pending_event(
            operation_id.clone(),
        )];
        transaction.abort(reason)?;
        events.push(EventService::session_write_committed_event(
            operation_id,
            session_id.clone(),
        ));
        Ok(FinalizedSessionWrite {
            events,
            session_id: Some(session_id),
            leaf_id: None,
        })
    }

    pub(crate) fn skip_prompt_transaction(
        operation_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> FinalizedSessionWrite {
        Self::skipped_write(operation_id, reason)
    }

    #[cfg(test)]
    pub(crate) fn session_dir(&self) -> &Path {
        self.handle.session_dir()
    }

    #[allow(dead_code)]
    pub(crate) fn replay(&self) -> Result<SessionReplay, CodingSessionError> {
        self.store.replay_session(&self.handle)
    }

    pub(crate) fn view(&self) -> CodingAgentSessionView {
        CodingAgentSessionView {
            session_id: self.session_id().to_owned(),
            default_agent_profile_id: self.default_agent_profile_id().clone(),
        }
    }

    pub(crate) fn hydrated_view(&self) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        let replay = self.replay()?;
        Ok(CodingAgentSessionHydration {
            summary: self.summary(),
            cwd: replay.cwd.clone(),
            transcript: replay
                .transcript
                .into_iter()
                .map(coding_transcript_item_from_replay)
                .collect(),
            diagnostics: replay
                .diagnostics
                .into_iter()
                .map(|diagnostic| CodingAgentSessionDiagnostic {
                    message: diagnostic.message,
                })
                .collect(),
        })
    }

    pub(crate) fn export_context(
        &self,
        options: ExportOptions,
    ) -> Result<ExportContext, CodingSessionError> {
        Ok(ExportContext::new(options, self.summary(), self.replay()?))
    }

    fn summary(&self) -> CodingAgentSessionSummary {
        CodingAgentSessionSummary {
            session_id: self.handle.manifest().session_id.clone(),
            session_dir: self.handle.session_dir().to_path_buf(),
            created_at: self.handle.manifest().created_at.clone(),
            updated_at: self.handle.manifest().updated_at.clone(),
            active_leaf_id: self.handle.manifest().active_leaf_id.clone(),
        }
    }

    fn copy_to_new_session(
        &self,
        target_leaf_id: Option<&str>,
        kind: SessionCopyKind,
    ) -> Result<Self, CodingSessionError> {
        let target_leaf_id = resolve_copy_target_leaf(self.handle.manifest(), target_leaf_id)?;
        let source_events = self.store.read_events(&self.handle)?;
        let cutoff = committed_leaf_cutoff(&source_events, &target_leaf_id).ok_or_else(|| {
            CodingSessionError::Session {
                message: format!("leaf id not found in source session: {target_leaf_id}"),
            }
        })?;

        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        let replay = self.replay()?;
        let target_session_id = ids.next_session_id();
        let mut target = Self::create_with_id(
            self.store.clone(),
            target_session_id,
            &mut ids,
            &clock,
            replay.cwd,
            self.default_agent_profile_id().clone(),
        )?;

        let provenance = SessionEventEnvelope::new(
            target.session_id().to_owned(),
            ids.next_event_id(),
            clock.now_rfc3339(),
            kind.provenance_event(self.session_id().to_owned(), target_leaf_id.clone()),
        );
        target.store.append_events(&target.handle, &[provenance])?;

        let branch_summary_operations =
            branch_summary_operation_ids_for_target(&source_events[cutoff + 1..], &target_leaf_id);
        let copied_events = source_events[..=cutoff]
            .iter()
            .chain(source_events[cutoff + 1..].iter().filter(|event| {
                should_copy_branch_summary_operation(
                    event,
                    &target_leaf_id,
                    &branch_summary_operations,
                )
            }))
            .filter(|event| should_copy_source_event(event))
            .map(|event| rewrite_event_for_session(event, target.session_id(), &mut ids))
            .collect::<Vec<_>>();
        target.store.append_events(&target.handle, &copied_events)?;
        target.store.update_manifest(
            &target.handle,
            ManifestPatch::new()
                .updated_at(clock.now_rfc3339())
                .active_leaf_id(Some(target_leaf_id)),
        )?;
        let session_id = target.session_id().to_owned();
        target.handle = target.store.open_session_id(&session_id)?;

        Ok(target)
    }

    fn create_with_id(
        store: SessionLogStore,
        session_id: String,
        ids: &mut impl IdGenerator,
        clock: &impl Clock,
        cwd: Option<String>,
        default_agent_profile_id: ProfileId,
    ) -> Result<Self, CodingSessionError> {
        let created_at = clock.now_rfc3339();
        let handle = store.create_session(
            CreateSessionOptions::new(session_id, created_at.clone())
                .default_agent_profile_id(default_agent_profile_id),
        )?;
        let created = SessionEventEnvelope::new(
            handle.manifest().session_id.clone(),
            ids.next_event_id(),
            created_at,
            SessionEventData::SessionCreated { cwd },
        );
        store.append_events(&handle, &[created])?;

        Ok(Self { store, handle })
    }

    fn append_durable_session_event(
        &mut self,
        operation_id: Option<String>,
        turn_id: Option<String>,
        data: SessionEventData,
    ) -> Result<(), CodingSessionError> {
        let session_id = self.session_id().to_owned();
        let mut ids = SystemIdGenerator;
        let clock = SystemClock;
        let updated_at = clock.now_rfc3339();
        let mut event = SessionEventEnvelope::new(
            session_id.clone(),
            ids.next_event_id(),
            updated_at.clone(),
            data,
        );
        event.operation_id = operation_id;
        event.turn_id = turn_id;
        self.store.append_events(&self.handle, &[event])?;
        self.store
            .update_manifest(&self.handle, ManifestPatch::new().updated_at(updated_at))?;
        self.handle = self.store.open_session_id(&session_id)?;
        Ok(())
    }

    fn skipped_write(
        operation_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> FinalizedSessionWrite {
        FinalizedSessionWrite {
            events: vec![EventService::session_write_skipped_event(
                operation_id,
                reason,
            )],
            session_id: None,
            leaf_id: None,
        }
    }

    fn next_leaf_id() -> String {
        let mut ids = SystemIdGenerator;
        ids.next_leaf_id()
    }
}

impl SessionCopyKind {
    fn provenance_event(
        self,
        source_session_id: String,
        source_leaf_id: String,
    ) -> SessionEventData {
        match self {
            Self::Clone => SessionEventData::SessionCloned {
                source_session_id,
                source_leaf_id,
            },
            Self::Fork => SessionEventData::SessionForked {
                source_session_id,
                source_leaf_id,
            },
        }
    }
}

fn resolve_copy_target_leaf(
    manifest: &super::session_log::manifest::SessionManifest,
    target_leaf_id: Option<&str>,
) -> Result<String, CodingSessionError> {
    if let Some(target_leaf_id) = target_leaf_id {
        let target_leaf_id = target_leaf_id.trim();
        if target_leaf_id.is_empty() {
            return Err(CodingSessionError::Input {
                message: "target leaf id must not be empty".into(),
            });
        }
        return Ok(target_leaf_id.to_owned());
    }

    manifest
        .active_leaf_id
        .clone()
        .ok_or_else(|| CodingSessionError::Session {
            message: "session has no committed active leaf".into(),
        })
}

fn normalize_leaf_id(value: &str) -> Result<String, CodingSessionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CodingSessionError::Input {
            message: "target leaf id must not be empty".into(),
        });
    }
    Ok(trimmed.to_owned())
}

fn committed_leaf_cutoff(events: &[SessionEventEnvelope], target_leaf_id: &str) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            &event.data,
            SessionEventData::OperationCommitted {
                new_leaf_id: Some(new_leaf_id),
            } if new_leaf_id == target_leaf_id
        )
    })
}

fn branch_summary_operation_ids_for_target(
    events: &[SessionEventEnvelope],
    target_leaf_id: &str,
) -> HashSet<String> {
    events
        .iter()
        .filter_map(|event| match &event.data {
            SessionEventData::BranchSummaryCreated {
                target_leaf_id: summary_target_leaf_id,
                ..
            } if summary_target_leaf_id == target_leaf_id => event.operation_id.clone(),
            _ => None,
        })
        .collect()
}

fn should_copy_branch_summary_operation(
    event: &SessionEventEnvelope,
    target_leaf_id: &str,
    operation_ids: &HashSet<String>,
) -> bool {
    if event
        .operation_id
        .as_ref()
        .is_some_and(|operation_id| operation_ids.contains(operation_id))
    {
        return true;
    }

    matches!(
        &event.data,
        SessionEventData::BranchSummaryCreated {
            target_leaf_id: summary_target_leaf_id,
            ..
        } if event.operation_id.is_none() && summary_target_leaf_id == target_leaf_id
    )
}

fn should_copy_source_event(event: &SessionEventEnvelope) -> bool {
    !matches!(
        event.data,
        SessionEventData::SessionCreated { .. }
            | SessionEventData::SessionCloned { .. }
            | SessionEventData::SessionForked { .. }
    )
}

fn rewrite_event_for_session(
    event: &SessionEventEnvelope,
    target_session_id: &str,
    ids: &mut impl IdGenerator,
) -> SessionEventEnvelope {
    let mut copied = event.clone();
    copied.session_id = target_session_id.to_owned();
    copied.event_id = ids.next_event_id();
    copied.parent_event_id = None;
    copied
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LeafTreeEntry {
    leaf_id: String,
    parent_leaf_id: Option<String>,
    timestamp: String,
    text: String,
}

fn build_leaf_tree(
    events: &[SessionEventEnvelope],
    active_leaf_id: Option<String>,
) -> CodingAgentSessionTree {
    let mut operation_kinds = HashMap::new();
    let mut operation_inputs = HashMap::new();
    let mut leaves = Vec::new();
    let mut current_parent_leaf_id: Option<String> = None;

    for event in events {
        if let SessionEventData::ActiveLeafChanged { leaf_id } = &event.data {
            current_parent_leaf_id = Some(leaf_id.clone());
            continue;
        }
        let Some(operation_id) = event.operation_id.as_deref() else {
            continue;
        };
        match &event.data {
            SessionEventData::OperationStarted { operation } => {
                operation_kinds.insert(operation_id.to_owned(), operation.clone());
            }
            SessionEventData::TurnInputRecorded { content } => {
                operation_inputs
                    .entry(operation_id.to_owned())
                    .or_insert_with(|| text_from_persisted_content(content));
            }
            SessionEventData::OperationCommitted {
                new_leaf_id: Some(leaf_id),
            } if operation_kinds.get(operation_id) == Some(&OperationKind::Prompt) => {
                leaves.push(LeafTreeEntry {
                    leaf_id: leaf_id.clone(),
                    parent_leaf_id: current_parent_leaf_id.clone(),
                    timestamp: event.created_at.clone(),
                    text: operation_inputs
                        .get(operation_id)
                        .filter(|text| !text.trim().is_empty())
                        .cloned()
                        .unwrap_or_else(|| leaf_id.clone()),
                });
                current_parent_leaf_id = Some(leaf_id.clone());
            }
            _ => {}
        }
    }

    CodingAgentSessionTree {
        tree: leaf_tree(leaves),
        active_leaf_id,
    }
}

fn text_from_persisted_content(content: &[PersistedContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            PersistedContentBlock::Text { text } => Some(text.trim()),
            PersistedContentBlock::Thinking { thinking, .. } => Some(thinking.trim()),
            PersistedContentBlock::Image { .. } => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn leaf_tree(leaves: Vec<LeafTreeEntry>) -> Vec<SessionTreeNode> {
    let known_leaf_ids = leaves
        .iter()
        .map(|leaf| leaf.leaf_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut children_by_parent: HashMap<Option<String>, Vec<LeafTreeEntry>> = HashMap::new();
    for mut leaf in leaves {
        if leaf
            .parent_leaf_id
            .as_ref()
            .is_some_and(|parent| !known_leaf_ids.contains(parent))
        {
            leaf.parent_leaf_id = None;
        }
        children_by_parent
            .entry(leaf.parent_leaf_id.clone())
            .or_default()
            .push(leaf);
    }
    build_leaf_children(None, &mut children_by_parent)
}

fn build_leaf_children(
    parent_leaf_id: Option<&str>,
    children_by_parent: &mut HashMap<Option<String>, Vec<LeafTreeEntry>>,
) -> Vec<SessionTreeNode> {
    let key = parent_leaf_id.map(str::to_owned);
    let leaves = children_by_parent.remove(&key).unwrap_or_default();
    leaves
        .into_iter()
        .map(|leaf| {
            let leaf_id = leaf.leaf_id.clone();
            let mut node = SessionTreeNode {
                entry: SessionEntry::message(
                    leaf.leaf_id,
                    leaf.parent_leaf_id,
                    leaf.timestamp,
                    StoredAgentMessage::User {
                        content: vec![ContentBlock::Text {
                            text: leaf.text,
                            text_signature: None,
                        }],
                        timestamp: 0,
                    },
                ),
                children: Vec::new(),
                label: None,
                label_timestamp: None,
            };
            node.children = build_leaf_children(Some(&leaf_id), children_by_parent);
            node
        })
        .collect()
}

fn coding_transcript_item_from_replay(item: TranscriptItem) -> CodingAgentSessionTranscriptItem {
    match item {
        TranscriptItem::UserInput { text, .. } => CodingAgentSessionTranscriptItem::User { text },
        TranscriptItem::AssistantMessage {
            message_id,
            content,
            status,
        } => CodingAgentSessionTranscriptItem::Assistant {
            id: message_id,
            text: persisted_content_blocks_text(&content),
            done: !matches!(status, MessageStatus::Started),
        },
        TranscriptItem::ToolCall {
            tool_call_id,
            name,
            arguments,
            status,
            summary,
        } => CodingAgentSessionTranscriptItem::Tool {
            call_id: tool_call_id,
            name,
            args: arguments,
            result: if summary.is_empty() {
                None
            } else {
                Some(summary)
            },
            is_error: matches!(status, ToolCallStatus::Failed),
        },
        TranscriptItem::DelegationBlock {
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            status,
            child_operation_id,
            summary,
        } => CodingAgentSessionTranscriptItem::Delegation {
            tool_call_id,
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            status: delegation_status_label(status).into(),
            child_operation_id,
            summary,
        },
        TranscriptItem::CompactionSummary { summary, .. } => {
            CodingAgentSessionTranscriptItem::CompactionSummary { summary }
        }
        TranscriptItem::BranchSummary { summary, .. } => {
            CodingAgentSessionTranscriptItem::BranchSummary { summary }
        }
        TranscriptItem::Diagnostic { message, .. } => {
            CodingAgentSessionTranscriptItem::Diagnostic { message }
        }
    }
}

fn delegation_status_label(status: PersistedDelegationStatus) -> &'static str {
    match status {
        PersistedDelegationStatus::Requested => "requested",
        PersistedDelegationStatus::Running => "running",
        PersistedDelegationStatus::Completed => "completed",
        PersistedDelegationStatus::Failed => "failed",
        PersistedDelegationStatus::Rejected => "rejected",
        PersistedDelegationStatus::ConfirmationRequired => "confirmation_required",
    }
}

fn persisted_content_blocks_text(
    content: &[super::session_log::event::PersistedContentBlock],
) -> String {
    content
        .iter()
        .map(|block| match block {
            super::session_log::event::PersistedContentBlock::Text { text } => text.clone(),
            super::session_log::event::PersistedContentBlock::Thinking { thinking, .. } => {
                thinking.clone()
            }
            super::session_log::event::PersistedContentBlock::Image { mime_type, .. } => {
                format!("[image:{mime_type}]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl From<SessionSummary> for CodingAgentSessionSummary {
    fn from(summary: SessionSummary) -> Self {
        Self {
            session_id: summary.session_id,
            session_dir: summary.session_dir,
            created_at: summary.created_at,
            updated_at: summary.updated_at,
            active_leaf_id: summary.active_leaf_id,
        }
    }
}

fn resolve_session_log_root(
    options: &CodingAgentSessionOptions,
) -> Result<PathBuf, CodingSessionError> {
    if let Some(root) = options.session_log_root() {
        return Ok(root.to_path_buf());
    }
    crate::session::default_sessions_root().map_err(|error| CodingSessionError::Session {
        message: error.to_string(),
    })
}

fn open_target(options: &CodingAgentSessionOptions) -> Result<PathBuf, CodingSessionError> {
    if let Some(path) = options.session_path() {
        return Ok(path.to_path_buf());
    }
    let session_id = options
        .session_id()
        .ok_or_else(|| CodingSessionError::Input {
            message: "opening a coding session requires a session id or session path".into(),
        })?;
    Ok(PathBuf::from(normalize_session_id(
        session_id,
        "session id",
    )?))
}

fn normalize_session_id(value: &str, label: &str) -> Result<String, CodingSessionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CodingSessionError::Input {
            message: format!("{label} must not be empty"),
        });
    }
    Ok(trimmed.to_owned())
}

fn option_cwd_string(options: &CodingAgentSessionOptions) -> Option<String> {
    options.cwd().map(normalized_path_string)
}

fn option_default_agent_profile_id(options: &CodingAgentSessionOptions) -> ProfileId {
    options
        .default_agent_profile_id()
        .cloned()
        .unwrap_or_else(|| ProfileId::from("default"))
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::session_log::event::PersistedContentBlock;

    #[test]
    fn create_uses_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id(" sess_test ")
            .with_session_log_root(temp.path());
        let service = SessionService::create(&options).unwrap();

        assert_eq!(service.session_id(), "sess_test");
        assert!(service.session_dir().join("session.json").is_file());
        assert!(service.session_dir().join("events.jsonl").is_file());

        let replay = service.replay().unwrap();
        assert_eq!(replay.session_id, "sess_test");
        assert!(replay.transcript.is_empty());
    }

    #[test]
    fn create_records_adapter_cwd() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_cwd")
            .with_cwd(&cwd)
            .with_session_log_root(temp.path().join("sessions"));

        let service = SessionService::create(&options).unwrap();

        let replay = service.replay().unwrap();
        assert_eq!(replay.cwd.as_deref(), Some(cwd.to_str().unwrap()));
    }

    #[test]
    fn open_reads_rust_native_session_by_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_open")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&options).unwrap();

        let opened = SessionService::open(&options).unwrap();

        assert_eq!(opened.session_id(), "sess_open");
        assert_eq!(opened.session_dir(), created.session_dir());
    }

    #[test]
    fn open_reads_rust_native_session_by_path() {
        let temp = tempfile::tempdir().unwrap();
        let create_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_path")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&create_options).unwrap();
        let open_options = CodingAgentSessionOptions::new()
            .with_session_log_root(temp.path())
            .with_session_path(created.session_dir());

        let opened = SessionService::open(&open_options).unwrap();

        assert_eq!(opened.session_id(), "sess_path");
    }

    #[test]
    fn open_or_create_creates_missing_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_open_or_create")
            .with_session_log_root(temp.path());

        let service = SessionService::open_or_create(&options).unwrap();

        assert_eq!(service.session_id(), "sess_open_or_create");
        assert!(service.session_dir().join("session.json").is_file());
        let events = service.store.read_events(&service.handle).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].data,
            SessionEventData::SessionCreated { .. }
        ));
    }

    #[test]
    fn open_or_create_reopens_existing_explicit_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_reopen")
            .with_session_log_root(temp.path());
        let created = SessionService::create(&options).unwrap();

        let opened = SessionService::open_or_create(&options).unwrap();

        assert_eq!(opened.session_id(), "sess_reopen");
        assert_eq!(opened.session_dir(), created.session_dir());
        let events = opened.store.read_events(&opened.handle).unwrap();
        assert_eq!(events.len(), 1, "open-or-create must not recreate sessions");
        assert!(matches!(
            events[0].data,
            SessionEventData::SessionCreated { .. }
        ));
    }

    #[test]
    fn open_or_create_requires_session_id() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new().with_session_log_root(temp.path());

        let error = SessionService::open_or_create(&options).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: open-or-create requires a session id"
        );
    }

    #[test]
    fn list_returns_session_summaries_sorted_by_updated_at() {
        let temp = tempfile::tempdir().unwrap();
        let old_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_list_old")
            .with_session_log_root(temp.path());
        let new_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_list_new")
            .with_session_log_root(temp.path());
        let old = SessionService::create(&old_options).unwrap();
        let new = SessionService::create(&new_options).unwrap();
        old.store
            .update_manifest(
                &old.handle,
                super::super::session_log::store::ManifestPatch::new()
                    .updated_at("2999-01-01T00:00:00Z")
                    .active_leaf_id(Some("leaf_list_old".into())),
            )
            .unwrap();

        let summaries = SessionService::list(
            &CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
        )
        .unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].session_id, "sess_list_old");
        assert_eq!(summaries[0].session_dir, old.session_dir());
        assert_eq!(summaries[0].updated_at, "2999-01-01T00:00:00Z");
        assert_eq!(
            summaries[0].active_leaf_id.as_deref(),
            Some("leaf_list_old")
        );
        assert_eq!(summaries[1].session_id, "sess_list_new");
        assert_eq!(summaries[1].session_dir, new.session_dir());
    }

    #[test]
    fn list_filters_session_summaries_by_adapter_cwd() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let other = temp.path().join("other");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let root = temp.path().join("sessions");
        let project_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_project")
            .with_cwd(&project)
            .with_session_log_root(&root);
        let other_options = CodingAgentSessionOptions::new()
            .with_session_id("sess_other")
            .with_cwd(&other)
            .with_session_log_root(&root);
        SessionService::create(&project_options).unwrap();
        SessionService::create(&other_options).unwrap();

        let summaries = SessionService::list(
            &CodingAgentSessionOptions::new()
                .with_cwd(&project)
                .with_session_log_root(&root),
        )
        .unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].session_id, "sess_project");
    }

    #[test]
    fn open_requires_session_id() {
        let error = SessionService::open(&CodingAgentSessionOptions::new()).unwrap_err();

        assert_eq!(error.code(), "input");
        assert_eq!(
            error.to_string(),
            "invalid input: opening a coding session requires a session id or session path"
        );
    }

    #[test]
    fn commit_prompt_transaction_emits_pending_and_committed_events() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_commit_prompt")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let mut transaction = service.begin_prompt_transaction();
        let operation_id = transaction.operation_id().to_owned();
        transaction
            .record_user_input(vec![
                crate::coding_session::session_log::event::PersistedContentBlock::Text {
                    text: "hello".into(),
                },
            ])
            .unwrap();

        let finalized = service
            .commit_prompt_transaction(Some(transaction), operation_id.clone())
            .unwrap();

        assert_eq!(
            finalized.events,
            vec![
                CodingAgentEvent::SessionWritePending {
                    operation_id: operation_id.clone(),
                },
                CodingAgentEvent::SessionWriteCommitted {
                    operation_id: operation_id.clone(),
                    session_id: "sess_commit_prompt".into(),
                },
            ]
        );
        assert_eq!(finalized.session_id.as_deref(), Some("sess_commit_prompt"));
        assert!(
            finalized
                .leaf_id
                .as_deref()
                .is_some_and(|leaf_id| leaf_id.starts_with("leaf_"))
        );
        let replay = service.replay().unwrap();
        assert_eq!(replay.transcript.len(), 1);
        assert_eq!(replay.active_leaf_id, finalized.leaf_id);
        assert_eq!(
            service
                .hydrated_view()
                .unwrap()
                .summary
                .active_leaf_id
                .as_deref(),
            replay.active_leaf_id.as_deref()
        );
    }

    #[test]
    fn clone_current_copies_committed_history_to_new_session() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_clone_source")
            .with_cwd(&cwd)
            .with_session_log_root(temp.path().join("sessions"));
        let mut service = SessionService::create(&options).unwrap();
        let source_session_id = service.session_id().to_owned();
        record_prompt(&mut service, "first prompt");
        let target_leaf = record_prompt(&mut service, "second prompt");

        let cloned = service.clone_current().unwrap();

        assert_ne!(cloned.session_id(), source_session_id);
        assert!(cloned.session_dir().join("session.json").is_file());
        assert!(cloned.session_dir().join("events.jsonl").is_file());
        let hydrated = cloned.hydrated_view().unwrap();
        assert_eq!(hydrated.cwd.as_deref(), Some(cwd.to_str().unwrap()));
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(target_leaf.as_str())
        );
        assert_eq!(
            hydrated.transcript,
            vec![
                CodingAgentSessionTranscriptItem::User {
                    text: "first prompt".into()
                },
                CodingAgentSessionTranscriptItem::User {
                    text: "second prompt".into()
                },
            ]
        );

        let cloned_session_id = cloned.session_id().to_owned();
        let events = cloned.store.read_events(&cloned.handle).unwrap();
        assert!(
            matches!(
                &events[1].data,
                SessionEventData::SessionCloned {
                    source_session_id: actual_source_session_id,
                    source_leaf_id,
                } if actual_source_session_id == &source_session_id
                    && source_leaf_id == &target_leaf
            ),
            "{events:#?}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.data, SessionEventData::SessionCreated { .. }))
                .count(),
            1
        );
        assert!(
            events
                .iter()
                .all(|event| event.session_id == cloned_session_id
                    && event.parent_event_id.is_none())
        );
    }

    #[test]
    fn fork_current_uses_requested_committed_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_fork_source")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let source_session_id = service.session_id().to_owned();
        let target_leaf = record_prompt(&mut service, "keep prompt");
        record_prompt(&mut service, "drop prompt");

        let forked = service.fork_current(Some(&target_leaf)).unwrap();

        assert_ne!(forked.session_id(), source_session_id);
        let hydrated = forked.hydrated_view().unwrap();
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(target_leaf.as_str())
        );
        assert_eq!(
            hydrated.transcript,
            vec![CodingAgentSessionTranscriptItem::User {
                text: "keep prompt".into()
            }]
        );
        let events = forked.store.read_events(&forked.handle).unwrap();
        assert!(
            events.iter().any(|event| matches!(
                &event.data,
                SessionEventData::SessionForked {
                    source_session_id: actual_source_session_id,
                    source_leaf_id,
                } if actual_source_session_id == &source_session_id
                    && source_leaf_id == &target_leaf
            )),
            "{events:#?}"
        );
        let event_log_text =
            std::fs::read_to_string(forked.handle.event_log_path().unwrap()).unwrap();
        assert!(event_log_text.contains("keep prompt"), "{event_log_text}");
        assert!(!event_log_text.contains("drop prompt"), "{event_log_text}");
    }

    #[test]
    fn fork_current_copies_branch_summary_for_requested_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_fork_branch_summary")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let target_leaf = record_prompt(&mut service, "keep prompt");
        let abandoned_leaf = record_prompt(&mut service, "drop prompt");
        let mut transaction = service.begin_branch_summary_transaction();
        let operation_id = transaction.operation_id().to_owned();
        transaction
            .record_branch_summary_created(
                "model branch summary",
                abandoned_leaf.clone(),
                target_leaf.clone(),
            )
            .unwrap();
        service
            .commit_branch_summary_transaction(Some(transaction), operation_id)
            .unwrap();

        let forked = service.fork_current(Some(&target_leaf)).unwrap();

        let event_log_text =
            std::fs::read_to_string(forked.handle.event_log_path().unwrap()).unwrap();
        let hydrated = forked.hydrated_view().unwrap();
        assert_eq!(
            hydrated.transcript,
            vec![
                CodingAgentSessionTranscriptItem::User {
                    text: "keep prompt".into()
                },
                CodingAgentSessionTranscriptItem::BranchSummary {
                    summary: "model branch summary".into()
                },
            ],
            "{event_log_text}"
        );
        assert!(
            event_log_text.contains(r#""kind":"branch.summary.created""#),
            "{event_log_text}"
        );
        assert!(
            event_log_text.contains("model branch summary"),
            "{event_log_text}"
        );
        assert!(!event_log_text.contains("drop prompt"), "{event_log_text}");
    }

    #[test]
    fn fork_current_rejects_unknown_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_fork_unknown")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        record_prompt(&mut service, "known prompt");

        let error = service.fork_current(Some("leaf_missing")).unwrap_err();

        assert_eq!(error.code(), "session");
        assert_eq!(
            error.to_string(),
            "session error: leaf id not found in source session: leaf_missing"
        );
    }

    #[test]
    fn switch_active_leaf_records_event_and_updates_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_switch_leaf")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let root_leaf = record_prompt(&mut service, "root prompt");
        let branch_leaf = record_prompt(&mut service, "branch prompt");

        service.switch_active_leaf(&root_leaf).unwrap();

        assert_eq!(service.active_leaf_id(), Some(root_leaf.as_str()));
        assert_eq!(
            service.replay().unwrap().active_leaf_id.as_deref(),
            Some(root_leaf.as_str())
        );
        let events = service.store.read_events(&service.handle).unwrap();
        assert!(matches!(
            events.last().map(|event| &event.data),
            Some(SessionEventData::ActiveLeafChanged { leaf_id }) if leaf_id == &root_leaf
        ));
        assert!(events.last().unwrap().operation_id.is_none());

        let alternate_leaf = record_prompt(&mut service, "alternate prompt");
        let tree = SessionService::tree_view(&options).unwrap();

        assert_eq!(
            tree.active_leaf_id.as_deref(),
            Some(alternate_leaf.as_str())
        );
        assert_eq!(tree.tree.len(), 1);
        assert_eq!(tree.tree[0].entry.id, root_leaf);
        let child_ids = tree.tree[0]
            .children
            .iter()
            .map(|child| child.entry.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            child_ids,
            vec![branch_leaf.as_str(), alternate_leaf.as_str()]
        );
    }

    #[test]
    fn switch_active_leaf_rejects_unknown_leaf_without_mutating_session() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_switch_unknown")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let known_leaf = record_prompt(&mut service, "known prompt");
        let before_events = service.store.read_events(&service.handle).unwrap();

        let error = service.switch_active_leaf("leaf_missing").unwrap_err();

        assert_eq!(error.code(), "session");
        assert_eq!(
            error.to_string(),
            "session error: leaf id not found in session: leaf_missing"
        );
        assert_eq!(service.active_leaf_id(), Some(known_leaf.as_str()));
        assert_eq!(
            service.store.read_events(&service.handle).unwrap(),
            before_events
        );
    }

    #[test]
    fn fail_prompt_transaction_emits_pending_and_committed_events() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_fail_prompt")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let transaction = service.begin_prompt_transaction();
        let operation_id = transaction.operation_id().to_owned();

        let finalized = service
            .fail_prompt_transaction(
                Some(transaction),
                operation_id.clone(),
                "provider",
                "stream failed",
            )
            .unwrap();

        assert_eq!(
            finalized.events,
            vec![
                CodingAgentEvent::SessionWritePending {
                    operation_id: operation_id.clone(),
                },
                CodingAgentEvent::SessionWriteCommitted {
                    operation_id: operation_id.clone(),
                    session_id: "sess_fail_prompt".into(),
                },
            ]
        );
        let replay = service.replay().unwrap();
        assert!(
            replay
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("stream failed"))
        );
    }

    #[test]
    fn skip_prompt_transaction_emits_skipped_event() {
        let finalized =
            SessionService::skip_prompt_transaction("op_skip", "no active prompt transaction");

        assert_eq!(
            finalized.events,
            vec![CodingAgentEvent::SessionWriteSkipped {
                operation_id: "op_skip".into(),
                reason: "no active prompt transaction".into(),
            }]
        );
        assert!(finalized.session_id.is_none());
        assert!(finalized.leaf_id.is_none());
    }

    #[test]
    fn tree_view_uses_active_leaf_changes_as_prompt_parent() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_tree_branch")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let root_leaf = record_prompt(&mut service, "root prompt");
        let branch_leaf = record_prompt(&mut service, "branch prompt");
        let switch_event = SessionEventEnvelope::new(
            service.session_id().to_owned(),
            "evt_switch_root",
            "2026-06-29T00:00:00Z",
            SessionEventData::ActiveLeafChanged {
                leaf_id: root_leaf.clone(),
            },
        );
        service
            .store
            .append_events(&service.handle, &[switch_event])
            .unwrap();
        let alternate_leaf = record_prompt(&mut service, "alternate prompt");

        let tree = SessionService::tree_view(&options).unwrap();

        assert_eq!(
            tree.active_leaf_id.as_deref(),
            Some(alternate_leaf.as_str())
        );
        assert_eq!(tree.tree.len(), 1);
        assert_eq!(tree.tree[0].entry.id, root_leaf);
        let child_ids = tree.tree[0]
            .children
            .iter()
            .map(|child| child.entry.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            child_ids,
            vec![branch_leaf.as_str(), alternate_leaf.as_str()]
        );
    }

    #[test]
    fn tree_view_uses_committed_leaf_ids() {
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_tree")
            .with_session_log_root(temp.path());
        let mut service = SessionService::create(&options).unwrap();
        let first_leaf = record_prompt(&mut service, "first prompt");
        let second_leaf = record_prompt(&mut service, "second prompt");

        let tree = SessionService::tree_view(&options).unwrap();

        assert_eq!(tree.active_leaf_id.as_deref(), Some(second_leaf.as_str()));
        assert_eq!(tree.tree.len(), 1);
        assert_eq!(tree.tree[0].entry.id, first_leaf);
        assert_eq!(tree.tree[0].children.len(), 1);
        assert_eq!(tree.tree[0].children[0].entry.id, second_leaf);
        assert_eq!(
            tree.tree[0]
                .entry
                .field("message")
                .and_then(|message| message.get("content"))
                .and_then(|content| content.as_array())
                .and_then(|blocks| blocks.first())
                .and_then(|block| block.get("text"))
                .and_then(|text| text.as_str()),
            Some("first prompt")
        );
    }

    fn record_prompt(service: &mut SessionService, text: &str) -> String {
        let mut transaction = service.begin_prompt_transaction();
        let operation_id = transaction.operation_id().to_owned();
        transaction
            .record_user_input(vec![PersistedContentBlock::Text {
                text: text.to_owned(),
            }])
            .unwrap();
        service
            .commit_prompt_transaction(Some(transaction), operation_id)
            .unwrap()
            .leaf_id
            .unwrap()
    }
}
