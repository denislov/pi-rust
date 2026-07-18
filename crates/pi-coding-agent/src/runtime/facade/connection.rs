use super::*;

impl CodingAgentSession {
    pub(crate) fn hydrate_current(
        &self,
    ) -> Result<Option<CodingAgentSessionHydration>, CodingSessionError> {
        match &self.runtime_host.session_coordinator.persistence {
            SessionPersistence::Persistent(session_service) => {
                Ok(Some(session_service.hydrated_view()?))
            }
            SessionPersistence::NonPersistent(_) => Ok(None),
        }
    }

    pub(crate) fn subscribe_product_events(&self) -> ProductEventReceiver {
        let receiver = self
            .runtime_host
            .event_hub
            .service
            .subscribe_product_events();
        self.emit_pending_startup_recovery_markers();
        receiver
    }

    pub fn subscribe_product_events_public(&self) -> CodingAgentProductEventReceiver {
        CodingAgentProductEventReceiver::new(self.subscribe_product_events())
    }

    pub fn runtime_shutdown_handle(&self) -> CodingAgentRuntimeShutdownHandle {
        CodingAgentRuntimeShutdownHandle {
            coordinator: self.runtime_host.client_projection.coordinator.clone(),
        }
    }

    pub fn capability_control(&self) -> CodingAgentCapabilityControl {
        CodingAgentCapabilityControl {
            coordinator: self.runtime_host.client_projection.coordinator.clone(),
            operation_control: self.runtime_host.operation_supervisor.control.clone(),
            event_service: self.runtime_host.event_hub.service.clone(),
            authorization_service: self.runtime_host.authorization_service.clone(),
        }
    }

    pub async fn shutdown(&mut self) -> Result<CodingAgentShutdownOutcome, CodingSessionError> {
        if self
            .runtime_host
            .client_projection
            .coordinator
            .request_shutdown()
            == snapshot_coordinator::RuntimeLifecycle::ShutDown
        {
            return Ok(CodingAgentShutdownOutcome::AlreadyShutDown);
        }
        self.runtime_host
            .authorization_service
            .cancel_all("tool authorization cancelled by runtime shutdown");
        self.runtime_host
            .client_projection
            .coordinator
            .wait_for_active_operation_to_drain()
            .await;
        self.runtime_host.session_coordinator.shutdown_writer()?;
        self.runtime_host.event_hub.service.emit_runtime_shutdown();
        self.runtime_host
            .client_projection
            .coordinator
            .finish_shutdown();
        Ok(CodingAgentShutdownOutcome::ShutDown)
    }

    fn emit_pending_startup_recovery_markers(&self) {
        let markers = {
            let mut markers = self
                .runtime_host
                .session_coordinator
                .startup_recovery_markers
                .lock()
                .unwrap();
            std::mem::take(&mut *markers)
        };
        if !markers.is_empty() {
            self.runtime_host
                .client_projection
                .coordinator
                .mark_recovery_projected();
        }
        for marker in markers {
            self.runtime_host
                .event_hub
                .service
                .emit_startup_recovery_pending(
                    marker.operation_id,
                    marker.recovery_id,
                    marker.reason,
                    marker.session_id,
                    marker
                        .operation_kind
                        .and_then(persisted_runtime_operation_kind),
                    marker.capability_generation,
                );
        }
    }

    pub fn snapshot(&self) -> CodingAgentSnapshot {
        self.emit_pending_startup_recovery_markers();
        self.runtime_host
            .client_projection
            .coordinator
            .snapshot()
            .into()
    }

    pub fn connect(
        &self,
        id: CodingAgentClientId,
    ) -> Result<CodingAgentClientConnection, CodingSessionError> {
        let internal_id = public_projection::internal_client_id(&id);
        let handle = self
            .runtime_host
            .client_projection
            .clients
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
            .runtime_host
            .client_projection
            .coordinator
            .client_state(&handle)
            .map_err(|error| CodingSessionError::Input {
                message: error.to_string(),
            })?;
        Ok(public_projection::public_client_connection(
            id,
            self.runtime_host.client_projection.coordinator.clone(),
            self.runtime_host.event_hub.service.clone(),
            self.runtime_host.authorization_service.clone(),
            handle,
            state,
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn ui_snapshot(&self, client_drafts: Vec<ClientDraft>) -> UiSnapshot {
        self.emit_pending_startup_recovery_markers();
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::SessionView,
        );
        let mut snapshot = self.runtime_host.client_projection.coordinator.snapshot();
        snapshot.client_drafts = client_drafts;
        snapshot
    }

    pub(in crate::runtime) fn refresh_snapshot_projection(&self) {
        let session = self.view();
        let capabilities = self.capabilities();
        let generation = self
            .runtime_host
            .operation_supervisor
            .capabilities
            .current_generation();
        let committed_session_sequence = match &self.runtime_host.session_coordinator.persistence {
            SessionPersistence::Persistent(session_service) => {
                session_service.committed_session_sequence()
            }
            SessionPersistence::NonPersistent(_) => 0,
        };
        self.runtime_host
            .client_projection
            .coordinator
            .install_projection(
                session,
                capabilities,
                generation,
                committed_session_sequence,
            );
    }

    #[allow(dead_code)]
    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        self.emit_pending_startup_recovery_markers();
        self.runtime_host
            .event_hub
            .service
            .product_events_after(cursor)
    }

    #[cfg(test)]
    pub(crate) fn emit_diagnostic_for_tests(&self, message: impl Into<String>) -> ProductEvent {
        self.runtime_host
            .event_hub
            .service
            .emit_diagnostic(None::<String>, message)
    }
}

fn persisted_runtime_operation_kind(
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
        SessionKind::SessionTreeLabel => Some(RuntimeKind::SetSessionTreeLabel),
        SessionKind::Other { .. } => None,
    }
}
