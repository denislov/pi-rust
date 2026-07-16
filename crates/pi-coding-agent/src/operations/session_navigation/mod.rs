use crate::runtime::facade::{
    CodingAgentSessionHydration, CodingAgentSessionOptions, CodingAgentSessionTree,
    CodingSessionError,
};
use crate::services::session::{ReplayDerivedOwnerState, replay_derived_owner_state};
use crate::session::service::{SessionPersistence, SessionService};

pub(crate) struct ForkTransition {
    pub(crate) session_service: SessionService,
    pub(crate) session_id: String,
    pub(crate) owner_state: ReplayDerivedOwnerState,
}

pub(crate) fn fork(
    persistence: &SessionPersistence,
    target_leaf_id: Option<&str>,
    operation_id: &str,
) -> Result<ForkTransition, CodingSessionError> {
    let SessionPersistence::Persistent(session_service) = persistence else {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "fork requires a persistent Rust-native session".into(),
        });
    };
    let mut forked_service = session_service.fork_current_admitted(target_leaf_id, operation_id)?;
    let session_id = forked_service.session_id().to_owned();
    let owner_state = match replay_derived_owner_state(&mut forked_service) {
        Ok(owner_state) => owner_state,
        Err(error) => {
            return Err(forked_service.cleanup_failed_transition(operation_id, error));
        }
    };
    Ok(ForkTransition {
        session_service: forked_service,
        session_id,
        owner_state,
    })
}

pub(crate) fn switch_active_leaf(
    persistence: &mut SessionPersistence,
    target_leaf_id: &str,
    operation_id: &str,
) -> Result<(), CodingSessionError> {
    let SessionPersistence::Persistent(session_service) = persistence else {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "active leaf navigation requires a persistent Rust-native session".into(),
        });
    };
    session_service.switch_active_leaf(target_leaf_id, operation_id)
}

pub(crate) fn hydrate(
    options: CodingAgentSessionOptions,
) -> Result<CodingAgentSessionHydration, CodingSessionError> {
    SessionService::hydrate(&options)
}

pub(crate) fn tree_view(
    options: CodingAgentSessionOptions,
) -> Result<CodingAgentSessionTree, CodingSessionError> {
    SessionService::tree_view(&options)
}

pub(crate) fn clone_session(
    options: CodingAgentSessionOptions,
) -> Result<CodingAgentSessionHydration, CodingSessionError> {
    SessionService::open(&options)?
        .clone_current()?
        .hydrated_view()
}

pub(crate) fn fork_session(
    options: CodingAgentSessionOptions,
    target_leaf_id: Option<&str>,
) -> Result<CodingAgentSessionHydration, CodingSessionError> {
    SessionService::open(&options)?
        .fork_current(target_leaf_id)?
        .hydrated_view()
}
