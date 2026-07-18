use std::sync::Mutex;

use super::capability::CapabilityGeneration;
use super::facade::CodingSessionError;
use crate::operations::delegation::PendingDelegationConfirmationQueue;
use crate::profiles::ProfileId;
#[cfg(test)]
use crate::session::id::{Clock, SystemClock};
use crate::session::service::{SessionPersistence, StartupRecoveryMarker};

/// Sole mutable owner of product session state.
#[derive(Debug)]
pub(super) struct SessionCoordinator {
    pub(super) persistence: SessionPersistence,
    pub(super) pending_delegation_confirmations: PendingDelegationConfirmationQueue,
    pub(super) startup_recovery_markers: Mutex<Vec<StartupRecoveryMarker>>,
}

/// Identity-bearing command accepted by the per-session writer.
#[derive(Debug)]
pub(super) struct SessionWriterCommand {
    pub(super) operation_id: String,
    pub(super) capability_generation: CapabilityGeneration,
    pub(super) mutation: SessionMutation,
}

impl SessionWriterCommand {
    pub(super) fn set_default_agent_profile(
        operation_id: impl Into<String>,
        capability_generation: CapabilityGeneration,
        profile_id: ProfileId,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            capability_generation,
            mutation: SessionMutation::SetDefaultAgentProfile { profile_id },
        }
    }
}

#[derive(Debug)]
pub(super) enum SessionMutation {
    SetDefaultAgentProfile { profile_id: ProfileId },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::service::TransientSessionState;

    fn transient_coordinator() -> SessionCoordinator {
        SessionCoordinator {
            persistence: SessionPersistence::NonPersistent(TransientSessionState::new(
                ProfileId::from("default"),
            )),
            pending_delegation_confirmations: Default::default(),
            startup_recovery_markers: Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn writer_command_mutates_session_owner_and_returns_typed_reply() {
        let mut coordinator = transient_coordinator();

        let reply = coordinator
            .execute_writer_command(SessionWriterCommand::set_default_agent_profile(
                "op_profile",
                CapabilityGeneration::new(7),
                ProfileId::from("reviewer"),
            ))
            .unwrap();

        assert_eq!(
            reply,
            SessionWriterReply::DefaultAgentProfileChanged {
                profile_id: ProfileId::from("reviewer"),
            }
        );
        let SessionPersistence::NonPersistent(state) = &coordinator.persistence else {
            unreachable!("fixture is transient")
        };
        assert_eq!(state.default_agent_profile_id.as_str(), "reviewer");
    }

    #[test]
    fn writer_command_rejects_missing_admitted_identity_without_mutation() {
        let mut coordinator = transient_coordinator();

        let error = coordinator
            .execute_writer_command(SessionWriterCommand::set_default_agent_profile(
                "  ",
                CapabilityGeneration::new(1),
                ProfileId::from("reviewer"),
            ))
            .unwrap_err();

        assert!(matches!(error, CodingSessionError::Session { .. }));
        let SessionPersistence::NonPersistent(state) = &coordinator.persistence else {
            unreachable!("fixture is transient")
        };
        assert_eq!(state.default_agent_profile_id.as_str(), "default");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SessionWriterReply {
    DefaultAgentProfileChanged { profile_id: ProfileId },
}

impl SessionCoordinator {
    #[cfg(test)]
    pub(super) fn pending_delegation_confirmations(
        &self,
    ) -> Vec<crate::operations::delegation::PendingDelegationConfirmation> {
        crate::operations::delegation::confirmation::active_views(
            &self.pending_delegation_confirmations,
            &SystemClock.now_rfc3339(),
        )
    }

    /// Execute one validated writer command.
    ///
    /// This synchronous entry point is the first migration stage of the writer
    /// protocol. Its `&mut self` authority guarantees one logical writer; the
    /// bounded command transport is added without changing workflow contracts.
    pub(super) fn execute_writer_command(
        &mut self,
        command: SessionWriterCommand,
    ) -> Result<SessionWriterReply, CodingSessionError> {
        if command.operation_id.trim().is_empty() {
            return Err(CodingSessionError::Session {
                message: "session writer command requires an admitted operation identity".into(),
            });
        }
        let _capability_generation = command.capability_generation;
        match command.mutation {
            SessionMutation::SetDefaultAgentProfile { profile_id } => {
                match &mut self.persistence {
                    SessionPersistence::Persistent(session_service) => {
                        session_service.set_default_agent_profile_id(profile_id.clone())?;
                    }
                    SessionPersistence::NonPersistent(state) => {
                        state.default_agent_profile_id = profile_id.clone();
                    }
                }
                Ok(SessionWriterReply::DefaultAgentProfileChanged { profile_id })
            }
        }
    }
}
