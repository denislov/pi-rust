use super::CodingSessionError;
use super::capability_snapshot::OperationCapabilitySnapshot;
use super::event_service::EventService;
use super::flow_service::FlowService;
use super::plugin_load_flow::{PluginLoadContext, PluginLoadOptions, PluginLoadOutcome};
use super::plugin_service::{self, PluginService};
use super::session_log::event::PersistedPluginDiagnostic;
use super::session_service::{SessionPersistence, SessionService};

#[derive(Debug, Default)]
pub(crate) struct PluginLoadService;

pub(crate) struct PluginLoadExecution {
    pub(crate) outcome: PluginLoadOutcome,
    pub(crate) loaded_plugin_service: Option<PluginService>,
}

impl PluginLoadService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn load(
        &self,
        persistence: &mut SessionPersistence,
        flow_service: &FlowService,
        event_service: &EventService,
        options: PluginLoadOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PluginLoadExecution, CodingSessionError> {
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
        let outcome = match flow_service.run_plugin_load(&mut context).await {
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
}

fn persisted_plugin_diagnostics(
    diagnostics: &[plugin_service::PluginDiagnostic],
) -> Vec<PersistedPluginDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| PersistedPluginDiagnostic {
            plugin_id: diagnostic.plugin_id.clone(),
            message: diagnostic.message.clone(),
        })
        .collect()
}

use super::*;

impl CodingAgentSession {
    pub(super) async fn load_plugins_inner(
        &mut self,
        options: PluginLoadOptions,
        snapshot: &OperationCapabilitySnapshot,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        let execution = self
            .plugin_load_service
            .load(
                &mut self.persistence,
                &self.flow_service,
                &self.event_service,
                options,
                snapshot,
            )
            .await?;
        if let Some(plugin_service) = execution.loaded_plugin_service {
            self.plugin_service = plugin_service;
        }
        if execution.outcome.capability_changed {
            let installed = self
                .capability_snapshots
                .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
            self.refresh_snapshot_projection();
            self.event_service.emit_capability_changed(installed);
        }
        Ok(execution.outcome)
    }
}
