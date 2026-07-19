use crate::extensions::ExtensionPlatformOwner;
use crate::runtime::capability::{OperationCapabilitySnapshot, SessionWriteCapability};
use crate::runtime::control::OperationCancellationHandle;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::plugin::PluginDiagnostic;
use crate::services::workflow::WorkflowService;
use crate::session::event::PersistedPluginDiagnostic;
use crate::session::service::{SessionPersistence, SessionService};
use tokio_util::sync::CancellationToken;

pub(crate) mod runner;

use runner::{PluginLoadContext, PluginLoadOptions, PluginLoadOutcome};

pub(crate) struct PluginLoadExecution {
    pub(crate) outcome: PluginLoadOutcome,
}

pub(crate) async fn run(
    persistence: &mut SessionPersistence,
    workflow_service: &WorkflowService,
    event_service: &EventService,
    options: PluginLoadOptions,
    snapshot: &OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
    cancellation_handle: Option<OperationCancellationHandle>,
    extension_platform: &ExtensionPlatformOwner,
) -> Result<PluginLoadExecution, CodingSessionError> {
    SessionWriteCapability::require(snapshot.session_write.as_ref())?;
    let operation_id = snapshot.operation_id.clone();
    let mut transaction = match persistence {
        SessionPersistence::Persistent(session_service) => {
            Some(session_service.begin_plugin_load_transaction_with_snapshot(snapshot))
        }
        SessionPersistence::NonPersistent(_) => None,
    };
    let mut context = PluginLoadContext::new(options);
    let mut outcome = match match cancellation.as_ref() {
        Some(cancellation) => {
            workflow_service
                .run_plugin_load_with_cancellation(&mut context, cancellation.clone())
                .await
        }
        None => workflow_service.run_plugin_load(&mut context).await,
    } {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Some(transaction) = transaction.take()
                && let SessionPersistence::Persistent(session_service) = persistence
            {
                let finalized = session_service.fail_plugin_load_transaction(
                    Some(transaction),
                    &operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                event_service.emit_session_write_events(&finalized);
            }
            return Err(error);
        }
    };
    let extension_snapshot = match extension_platform.reload_if_configured() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Some(transaction) = transaction.take()
                && let SessionPersistence::Persistent(session_service) = persistence
            {
                let finalized = session_service.fail_plugin_load_transaction(
                    Some(transaction),
                    &operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                event_service.emit_session_write_events(&finalized);
            }
            return Err(error);
        }
    };
    if let Some(extension_activation) = extension_snapshot {
        outcome
            .loaded_plugin_ids
            .extend(extension_activation.snapshot.packages.keys().cloned());
        outcome.loaded_plugin_ids.sort();
        outcome.loaded_plugin_ids.dedup();
        outcome.capability_changed |= extension_activation.capability_changed;
    }
    if let Some(cancellation_handle) = cancellation_handle
        && let Err(error) = cancellation_handle.close()
    {
        if let Some(transaction) = transaction.take()
            && let SessionPersistence::Persistent(session_service) = persistence
        {
            let finalized = session_service.fail_plugin_load_transaction(
                Some(transaction),
                &operation_id,
                error.code(),
                error.to_string(),
            )?;
            event_service.emit_session_write_events(&finalized);
        }
        return Err(error);
    } else if cancellation
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        let error = CodingSessionError::Cancelled;
        if let Some(transaction) = transaction.take()
            && let SessionPersistence::Persistent(session_service) = persistence
        {
            let finalized = session_service.fail_plugin_load_transaction(
                Some(transaction),
                &operation_id,
                error.code(),
                error.to_string(),
            )?;
            event_service.emit_session_write_events(&finalized);
        }
        return Err(error);
    }
    if let Some(transaction) = transaction.as_mut() {
        SessionService::record_plugin_load_completed(
            transaction,
            outcome.loaded_plugin_ids.clone(),
            persisted_plugin_diagnostics(&outcome.diagnostics),
            outcome.capability_changed,
        )?;
    }
    if let Some(transaction) = transaction.take()
        && let SessionPersistence::Persistent(session_service) = persistence
    {
        let finalized =
            session_service.commit_plugin_load_transaction(Some(transaction), &operation_id)?;
        event_service.emit_session_write_events(&finalized);
    }
    Ok(PluginLoadExecution { outcome })
}

fn persisted_plugin_diagnostics(
    diagnostics: &[PluginDiagnostic],
) -> Vec<PersistedPluginDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| PersistedPluginDiagnostic {
            plugin_id: diagnostic.plugin_id.clone(),
            message: diagnostic.message.clone(),
        })
        .collect()
}
