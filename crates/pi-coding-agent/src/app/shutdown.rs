use crate::runtime::facade::{CodingAgentSession, CodingSessionError};

pub(crate) async fn shutdown_session(
    session: Option<CodingAgentSession>,
) -> Result<(), CodingSessionError> {
    let Some(mut session) = session else {
        return Ok(());
    };
    session.shutdown().await.map(|_| ())
}
