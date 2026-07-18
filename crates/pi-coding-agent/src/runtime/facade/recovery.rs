use super::{
    CodingAgentRecoveryResolutionRequest, CodingAgentRecoveryResolutionResult, CodingAgentSession,
    CodingSessionError,
};
use crate::session::service::SessionPersistence;

impl CodingAgentSession {
    pub fn resolve_recovery(
        &mut self,
        request: CodingAgentRecoveryResolutionRequest,
    ) -> Result<CodingAgentRecoveryResolutionResult, CodingSessionError> {
        self.runtime_host
            .client_projection
            .coordinator
            .ensure_runtime_running()?;
        let SessionPersistence::Persistent(service) =
            &self.runtime_host.session_coordinator.persistence
        else {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "recovery resolution requires a persistent session".into(),
            });
        };
        let commit = service.resolve_recovery(&request)?;
        let operation_kind =
            super::connection::persisted_runtime_operation_kind(commit.operation_kind.clone())
                .ok_or_else(|| CodingSessionError::UnsupportedCapability {
                    capability: "recovery resolution requires a durable root operation family"
                        .into(),
                })?;
        self.runtime_host
            .event_hub
            .service
            .emit_committed_terminal_draft(commit.draft, operation_kind);
        Ok(CodingAgentRecoveryResolutionResult {
            operation_id: commit.operation_id,
            recovery_id: commit.recovery_id,
            resolution: commit.resolution,
        })
    }
}
