use super::*;

impl CodingAgentSession {
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

    pub(crate) fn subscribe_product_events(&self) -> ProductEventReceiver {
        let receiver = self.event_service.subscribe_product_events();
        self.emit_pending_startup_recovery_markers();
        receiver
    }

    pub fn subscribe_product_events_public(&self) -> CodingAgentProductEventReceiver {
        CodingAgentProductEventReceiver::new(self.subscribe_product_events())
    }

    pub fn runtime_shutdown_handle(&self) -> CodingAgentRuntimeShutdownHandle {
        CodingAgentRuntimeShutdownHandle {
            coordinator: self.snapshot_coordinator.clone(),
        }
    }

    pub fn capability_control(&self) -> CodingAgentCapabilityControl {
        CodingAgentCapabilityControl {
            coordinator: self.snapshot_coordinator.clone(),
            operation_control: self.operation_control.clone(),
            event_service: self.event_service.clone(),
        }
    }

    pub async fn shutdown(&mut self) -> Result<CodingAgentShutdownOutcome, CodingSessionError> {
        if self.snapshot_coordinator.request_shutdown()
            == snapshot_coordinator::RuntimeLifecycle::ShutDown
        {
            return Ok(CodingAgentShutdownOutcome::AlreadyShutDown);
        }
        self.snapshot_coordinator
            .wait_for_active_operation_to_drain()
            .await;
        self.event_service.emit_runtime_shutdown();
        self.snapshot_coordinator.finish_shutdown();
        Ok(CodingAgentShutdownOutcome::ShutDown)
    }

    fn emit_pending_startup_recovery_markers(&self) {
        let markers = {
            let mut markers = self.startup_recovery_markers.lock().unwrap();
            std::mem::take(&mut *markers)
        };
        if !markers.is_empty() {
            self.snapshot_coordinator.mark_recovery_projected();
        }
        for marker in markers {
            self.event_service.emit_operation_recovered(
                marker.operation_id,
                marker.recovery_id,
                marker.reason,
                marker.session_id,
                marker
                    .operation_kind
                    .and_then(recovered_runtime_operation_kind),
                marker.capability_generation,
            );
        }
    }

    pub fn snapshot(&self) -> CodingAgentSnapshot {
        self.emit_pending_startup_recovery_markers();
        self.snapshot_coordinator.snapshot().into()
    }

    pub fn connect(
        &self,
        id: CodingAgentClientId,
    ) -> Result<CodingAgentClientConnection, CodingSessionError> {
        let internal_id = public_projection::internal_client_id(&id);
        let handle = self
            .client_service
            .connect_or_takeover(internal_id)
            .map_err(|error| match error {
                snapshot_coordinator::ClientRegistryError::ClientCapacityExceeded { limit } => {
                    CodingSessionError::ClientCapacityExceeded { limit }
                }
                snapshot_coordinator::ClientRegistryError::Lifecycle(reason) => {
                    CodingSessionError::Lifecycle { reason }
                }
                other => CodingSessionError::Input {
                    message: other.to_string(),
                },
            })?;
        let state = self
            .snapshot_coordinator
            .client_state(&handle)
            .map_err(|error| CodingSessionError::Input {
                message: error.to_string(),
            })?;
        Ok(public_projection::public_client_connection(
            id,
            self.snapshot_coordinator.clone(),
            self.event_service.clone(),
            handle,
            state,
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn ui_snapshot(&self, client_drafts: Vec<ClientDraft>) -> UiSnapshot {
        self.emit_pending_startup_recovery_markers();
        IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
        let mut snapshot = self.snapshot_coordinator.snapshot();
        snapshot.client_drafts = client_drafts;
        snapshot
    }

    pub(in crate::runtime) fn refresh_snapshot_projection(&self) {
        let session = self.view();
        let capabilities = self.capabilities();
        let generation = self.capability_snapshots.current_generation();
        self.snapshot_coordinator
            .install_projection(session, capabilities, generation);
    }

    #[allow(dead_code)]
    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        self.emit_pending_startup_recovery_markers();
        self.event_service.product_events_after(cursor)
    }

    #[cfg(test)]
    pub(crate) fn emit_diagnostic_for_tests(&self, message: impl Into<String>) -> ProductEvent {
        self.event_service.emit_diagnostic(None::<String>, message)
    }
}

fn recovered_runtime_operation_kind(
    kind: crate::session::event::OperationKind,
) -> Option<crate::runtime::control::OperationKind> {
    use crate::runtime::control::OperationKind as RuntimeKind;
    use crate::session::event::OperationKind as SessionKind;
    match kind {
        SessionKind::Prompt => Some(RuntimeKind::Prompt),
        SessionKind::ManualCompaction => Some(RuntimeKind::Compact),
        SessionKind::BranchSummary => Some(RuntimeKind::BranchSummary),
        SessionKind::Export => Some(RuntimeKind::Export),
        SessionKind::PluginLoad => Some(RuntimeKind::PluginLoad),
        SessionKind::SelfHealingEdit => Some(RuntimeKind::SelfHealingEdit),
        SessionKind::Other { .. } => None,
    }
}
