use std::sync::Mutex;

use super::capability::CapabilityGeneration;
use super::facade::CodingSessionError;
use crate::operations::delegation::PendingDelegationConfirmationQueue;
use crate::profiles::ProfileId;
use crate::services::session::{ReplayDerivedOwnerState, replay_derived_owner_state};
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

    pub(super) fn switch_active_leaf(
        operation_id: impl Into<String>,
        capability_generation: CapabilityGeneration,
        target_leaf_id: impl Into<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            capability_generation,
            mutation: SessionMutation::SwitchActiveLeaf {
                target_leaf_id: target_leaf_id.into(),
            },
        }
    }

    pub(super) fn set_session_tree_label(
        operation_id: impl Into<String>,
        capability_generation: CapabilityGeneration,
        entry_id: impl Into<String>,
        label: Option<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            capability_generation,
            mutation: SessionMutation::SetSessionTreeLabel {
                entry_id: entry_id.into(),
                label,
            },
        }
    }

    pub(super) fn fork_session(
        operation_id: impl Into<String>,
        capability_generation: CapabilityGeneration,
        target_leaf_id: Option<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            capability_generation,
            mutation: SessionMutation::ForkSession { target_leaf_id },
        }
    }
}

#[derive(Debug)]
pub(super) enum SessionMutation {
    SetDefaultAgentProfile {
        profile_id: ProfileId,
    },
    SwitchActiveLeaf {
        target_leaf_id: String,
    },
    SetSessionTreeLabel {
        entry_id: String,
        label: Option<String>,
    },
    ForkSession {
        target_leaf_id: Option<String>,
    },
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
            SessionWriterReply::DefaultAgentProfile {
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
    DefaultAgentProfile {
        profile_id: ProfileId,
    },
    ActiveLeaf,
    SessionTreeLabel {
        entry_id: String,
        label: Option<String>,
        updated_at: String,
    },
    ForkedSession {
        session_id: String,
    },
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
                Ok(SessionWriterReply::DefaultAgentProfile { profile_id })
            }
            SessionMutation::SwitchActiveLeaf { target_leaf_id } => {
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability:
                            "active leaf navigation requires a persistent Rust-native session"
                                .into(),
                    });
                };
                session_service.switch_active_leaf(&target_leaf_id, &command.operation_id)?;
                Ok(SessionWriterReply::ActiveLeaf)
            }
            SessionMutation::SetSessionTreeLabel { entry_id, label } => {
                let SessionPersistence::Persistent(session_service) = &mut self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "session tree labels require a persistent Rust-native session"
                            .into(),
                    });
                };
                let update =
                    session_service.set_tree_label(&entry_id, label, &command.operation_id)?;
                Ok(SessionWriterReply::SessionTreeLabel {
                    entry_id: update.entry_id,
                    label: update.label,
                    updated_at: update.updated_at,
                })
            }
            SessionMutation::ForkSession { target_leaf_id } => {
                let SessionPersistence::Persistent(session_service) = &self.persistence else {
                    return Err(CodingSessionError::UnsupportedCapability {
                        capability: "fork requires a persistent Rust-native session".into(),
                    });
                };
                let mut forked_service = session_service
                    .fork_current_admitted(target_leaf_id.as_deref(), &command.operation_id)?;
                let owner_state = match replay_derived_owner_state(&mut forked_service) {
                    Ok(owner_state) => owner_state,
                    Err(error) => {
                        return Err(
                            forked_service.cleanup_failed_transition(&command.operation_id, error)
                        );
                    }
                };
                let session_id = forked_service.session_id().to_owned();
                self.install_forked_session(forked_service, owner_state);
                Ok(SessionWriterReply::ForkedSession { session_id })
            }
        }
    }

    fn install_forked_session(
        &mut self,
        session_service: crate::session::service::SessionService,
        owner_state: ReplayDerivedOwnerState,
    ) {
        self.persistence = SessionPersistence::Persistent(session_service);
        self.pending_delegation_confirmations = owner_state.pending_delegation_confirmations;
        *self.startup_recovery_markers.lock().unwrap() = owner_state.startup_recovery_markers;
    }
}
