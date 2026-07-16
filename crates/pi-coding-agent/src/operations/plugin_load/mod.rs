use crate::runtime::capability::{OperationCapabilitySnapshot, SessionWriteCapability};
use crate::runtime::control::OperationCancellationHandle;
use crate::runtime::facade::CodingSessionError;
use crate::services::event::EventService;
use crate::services::flow::FlowService;
use crate::services::plugin::{PluginDiagnostic, PluginService};
use crate::session::event::PersistedPluginDiagnostic;
use crate::session::service::{SessionPersistence, SessionService};
use tokio_util::sync::CancellationToken;

pub(crate) mod flow;

use flow::{PluginLoadContext, PluginLoadOptions, PluginLoadOutcome};

pub(crate) struct PluginLoadExecution {
    pub(crate) outcome: PluginLoadOutcome,
    pub(crate) loaded_plugin_service: Option<PluginService>,
}

pub(crate) async fn run(
    persistence: &mut SessionPersistence,
    flow_service: &FlowService,
    event_service: &EventService,
    options: PluginLoadOptions,
    snapshot: &OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
    cancellation_handle: Option<OperationCancellationHandle>,
) -> Result<PluginLoadExecution, CodingSessionError> {
    SessionWriteCapability::require(snapshot.session_write.as_ref())?;
    let mut transaction = match persistence {
        SessionPersistence::Persistent(session_service) => {
            Some(session_service.begin_plugin_load_transaction_with_snapshot(snapshot))
        }
        SessionPersistence::NonPersistent(_) => None,
    };
    let operation_id = transaction
        .as_ref()
        .map(|transaction| transaction.operation_id().to_owned())
        .unwrap_or_else(|| "plugin_load".to_owned());
    let mut context = PluginLoadContext::new(options);
    let outcome = match match cancellation.as_ref() {
        Some(cancellation) => {
            flow_service
                .run_plugin_load_with_cancellation(&mut context, cancellation.clone())
                .await
        }
        None => flow_service.run_plugin_load(&mut context).await,
    } {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Some(transaction) = transaction.take()
                && let SessionPersistence::Persistent(session_service) = persistence
            {
                let finalized = session_service.fail_plugin_load_transaction(
                    Some(transaction),
                    operation_id,
                    error.code(),
                    error.to_string(),
                )?;
                event_service.emit_session_write_events(&finalized);
            }
            return Err(error);
        }
    };
    if let Some(cancellation_handle) = cancellation_handle
        && let Err(error) = cancellation_handle.close()
    {
        if let Some(transaction) = transaction.take()
            && let SessionPersistence::Persistent(session_service) = persistence
        {
            let finalized = session_service.fail_plugin_load_transaction(
                Some(transaction),
                operation_id,
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
        return Err(CodingSessionError::Cancelled);
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
            session_service.commit_plugin_load_transaction(Some(transaction), operation_id)?;
        event_service.emit_session_write_events(&finalized);
    }
    let loaded_plugin_service = context.take_loaded_plugin_service();
    event_service.emit_plugin_load_outcome(&outcome);
    Ok(PluginLoadExecution {
        outcome,
        loaded_plugin_service,
    })
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
