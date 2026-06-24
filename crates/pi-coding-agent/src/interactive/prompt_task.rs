use crate::CliError;
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptResult, SpawnedSessionPrompt,
};

pub(super) struct PromptTask {
    abort: SessionPromptAbortHandle,
    pub(super) events: tokio::sync::mpsc::UnboundedReceiver<pi_agent_core::AgentEvent>,
    pub(super) done: tokio::sync::oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    abort_requested: bool,
    pub(super) events_closed: bool,
}

impl PromptTask {
    pub(super) fn new(spawned: SpawnedSessionPrompt) -> Self {
        Self {
            abort: spawned.abort,
            events: spawned.events,
            done: spawned.done,
            abort_requested: false,
            events_closed: false,
        }
    }

    pub(super) fn abort_once(&mut self) {
        if !self.abort_requested {
            self.abort.abort();
            self.abort_requested = true;
        }
    }
}
