use super::CodingSessionError;
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
    ) -> Result<PluginLoadExecution, CodingSessionError> {
        let mut transaction = match persistence {
            SessionPersistence::Persistent(session_service) => {
                Some(session_service.begin_plugin_load_transaction())
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
