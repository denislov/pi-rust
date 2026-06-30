use std::path::PathBuf;

use crate::CliError;
use crate::coding_session::{
    CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions, CodingSessionError,
    PromptTurnOptions, PromptTurnOutcome,
};
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptOptions, SessionPromptResult, SpawnedSessionPrompt,
    spawn_session_prompt,
};
use crate::runtime::SessionMode;
use crate::session::{ResolvedSessionTarget, resolve_session_dir};
use pi_agent_core::AgentEvent;
use tokio::sync::{mpsc, oneshot};

pub(super) enum PromptTaskEvent {
    Agent(AgentEvent),
    Coding(CodingAgentEvent),
}

pub(super) enum PromptTaskResult {
    Legacy(SessionPromptResult),
    Coding(CodingPromptTaskResult),
}

pub(super) struct CodingPromptTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: PromptTurnOutcome,
}

enum PromptTaskAbortHandle {
    Legacy(SessionPromptAbortHandle),
    Coding(Option<oneshot::Sender<()>>),
}

pub(super) struct PromptTask {
    abort: PromptTaskAbortHandle,
    pub(super) events: mpsc::UnboundedReceiver<PromptTaskEvent>,
    pub(super) done: oneshot::Receiver<Result<PromptTaskResult, CliError>>,
    abort_requested: bool,
    pub(super) events_closed: bool,
}

impl PromptTask {
    pub(super) fn spawn_prompt(
        options: SessionPromptOptions,
        existing_session: Option<CodingAgentSession>,
    ) -> Result<Self, CliError> {
        if should_use_legacy_prompt_task(&options) {
            return Self::spawn_legacy(options);
        }

        Ok(Self::spawn_coding(options, existing_session))
    }

    pub(super) fn spawn_legacy(options: SessionPromptOptions) -> Result<Self, CliError> {
        Self::from_legacy_spawned(spawn_session_prompt(options)?)
    }

    pub(super) fn abort_once(&mut self) {
        if self.abort_requested {
            return;
        }
        match &mut self.abort {
            PromptTaskAbortHandle::Legacy(abort) => abort.abort(),
            PromptTaskAbortHandle::Coding(abort) => {
                if let Some(abort) = abort.take() {
                    let _ = abort.send(());
                }
            }
        }
        self.abort_requested = true;
    }

    fn from_legacy_spawned(spawned: SpawnedSessionPrompt) -> Result<Self, CliError> {
        let SpawnedSessionPrompt {
            abort,
            mut events,
            done,
        } = spawned;
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if event_tx.send(PromptTaskEvent::Agent(event)).is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            let result = done
                .await
                .map_err(|_| {
                    CliError::AgentFailure("prompt task dropped before completion".to_string())
                })
                .and_then(|result| result)
                .map(PromptTaskResult::Legacy);
            let _ = done_tx.send(result);
        });

        Ok(Self {
            abort: PromptTaskAbortHandle::Legacy(abort),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        })
    }

    fn spawn_coding(
        options: SessionPromptOptions,
        existing_session: Option<CodingAgentSession>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();
        let (abort_tx, abort_rx) = oneshot::channel();

        tokio::spawn(async move {
            let result =
                run_coding_prompt_task(options, existing_session, event_tx, abort_rx).await;
            let _ = done_tx.send(result.map(PromptTaskResult::Coding));
        });

        Self {
            abort: PromptTaskAbortHandle::Coding(Some(abort_tx)),
            events: event_rx,
            done: done_rx,
            abort_requested: false,
            events_closed: false,
        }
    }
}

async fn run_coding_prompt_task(
    options: SessionPromptOptions,
    existing_session: Option<CodingAgentSession>,
    event_tx: mpsc::UnboundedSender<PromptTaskEvent>,
    mut abort_rx: oneshot::Receiver<()>,
) -> Result<CodingPromptTaskResult, CliError> {
    let mut session = match existing_session {
        Some(session) => session,
        None => {
            open_interactive_coding_session(
                options.session.as_ref(),
                options.session_target.as_ref(),
            )
            .await?
        }
    };
    let mut receiver = session.subscribe();
    let prompt_options = PromptTurnOptions::from_session_prompt_options(options);

    let outcome = {
        let mut prompt = Box::pin(session.prompt(prompt_options));
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    break Err(CliError::UnsupportedMode(
                        "interactive prompt abort awaits AgentTurnFlow".into(),
                    ));
                }
                event = receiver.recv() => {
                    if let Ok(event) = event {
                        let _ = event_tx.send(PromptTaskEvent::Coding(event));
                    }
                }
                outcome = &mut prompt => {
                    break outcome.map_err(CliError::from);
                }
            }
        }
    }?;

    while let Ok(Some(event)) = receiver.try_recv() {
        let _ = event_tx.send(PromptTaskEvent::Coding(event));
    }

    Ok(CodingPromptTaskResult { session, outcome })
}

fn should_use_legacy_prompt_task(options: &SessionPromptOptions) -> bool {
    if matches!(
        options.invocation,
        crate::runtime::PromptInvocation::Compact { .. }
    ) {
        return true;
    }

    match options.session_target.as_ref() {
        Some(ResolvedSessionTarget::ForkTarget(_)) => true,
        Some(ResolvedSessionTarget::OpenTarget(target)) => target_looks_like_legacy_jsonl(target),
        _ => false,
    }
}

fn target_looks_like_legacy_jsonl(target: &str) -> bool {
    let path = std::path::Path::new(target);
    path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") || path.is_file()
}

fn interactive_coding_session_root(
    options: Option<&crate::runtime::SessionRunOptions>,
) -> Result<Option<PathBuf>, CliError> {
    let Some(options) = options else {
        return Ok(None);
    };
    if !matches!(options.mode, SessionMode::Enabled) {
        return Ok(None);
    }
    match options.session_dir.as_ref() {
        Some(root) => Ok(Some(root.clone())),
        None => Ok(Some(resolve_session_dir(&options.cwd, None, None)?)),
    }
}

async fn open_interactive_coding_session(
    session_options: Option<&crate::runtime::SessionRunOptions>,
    target: Option<&ResolvedSessionTarget>,
) -> Result<CodingAgentSession, CliError> {
    let Some(session_options) = session_options else {
        return Ok(CodingAgentSession::non_persistent(CodingAgentSessionOptions::new()).await?);
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return Ok(CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new().with_cwd(session_options.cwd.clone()),
        )
        .await?);
    }

    let session_root =
        interactive_coding_session_root(Some(session_options))?.ok_or_else(|| {
            CodingSessionError::Session {
                message: "enabled interactive session is missing a session root".into(),
            }
        })?;

    let options = CodingAgentSessionOptions::new()
        .with_cwd(session_options.cwd.clone())
        .with_session_log_root(session_root);
    match target.unwrap_or(&ResolvedSessionTarget::New) {
        ResolvedSessionTarget::New => Ok(CodingAgentSession::create(options).await?),
        ResolvedSessionTarget::OpenOrCreateId(session_id) => Ok(
            CodingAgentSession::open_or_create(options.with_session_id(session_id.clone())).await?,
        ),
        ResolvedSessionTarget::OpenTarget(target) => {
            if target_looks_like_session_dir(target) {
                Ok(CodingAgentSession::open(options.with_session_path(target)).await?)
            } else {
                Ok(CodingAgentSession::open(options.with_session_id(target.clone())).await?)
            }
        }
        ResolvedSessionTarget::ContinueMostRecent => {
            let session_id = CodingAgentSession::list(options.clone())?
                .into_iter()
                .next()
                .map(|summary| summary.session_id)
                .ok_or_else(|| CodingSessionError::Session {
                    message: "no previous session to continue".into(),
                })?;
            Ok(CodingAgentSession::open(options.with_session_id(session_id)).await?)
        }
        ResolvedSessionTarget::ForkTarget(_) => Err(CodingSessionError::UnsupportedCapability {
            capability: "Rust-native session fork".into(),
        }
        .into()),
    }
}

fn target_looks_like_session_dir(target: &str) -> bool {
    let path = std::path::Path::new(target);
    path.is_dir() && path.join("session.json").is_file()
}
