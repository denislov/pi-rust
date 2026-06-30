use crate::CliOutput;
use crate::coding_session::{
    CodingAgentEvent, CodingAgentSession, CodingAgentSessionOptions, CodingSessionError,
    PromptTurnOptions, PromptTurnOutcome,
};
use crate::protocol::events::CodingProtocolEventAdapter;
use crate::protocol::jsonl::serialize_json_line;
use crate::protocol::session_runner::SessionPromptOptions;
use crate::protocol::types::ProtocolEvent;
use crate::runtime::{SessionMode, SessionRunOptions};
use crate::session::{ResolvedSessionTarget, resolve_session_dir};
use pi_agent_core::session::{SessionHeader, create_session_id, create_timestamp};
use std::path::PathBuf;

pub async fn run_json_mode(options: SessionPromptOptions) -> CliOutput {
    let header = SessionHeader {
        entry_type: "session".into(),
        version: 3,
        id: create_session_id(),
        timestamp: create_timestamp(),
        cwd: json_header_cwd(&options),
        parent_session: None,
    };

    let mut stdout = match serialize_json_line(&header) {
        Ok(line) => line,
        Err(error) => {
            return CliOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("agent failure: {error}\n"),
            };
        }
    };

    match serialize_json_line(&ProtocolEvent::AgentStart) {
        Ok(line) => stdout.push_str(&line),
        Err(error) => {
            return CliOutput {
                exit_code: 1,
                stdout,
                stderr: format!("agent failure: {error}\n"),
            };
        }
    }

    let mut adapter = CodingProtocolEventAdapter::new_with_provider(
        options.model.api.clone(),
        options.model.provider.clone(),
        options.model.id.clone(),
    );

    match run_json_prompt(options, &mut stdout, &mut adapter).await {
        Ok(PromptTurnOutcome::Success { .. }) => CliOutput {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        },
        Ok(PromptTurnOutcome::Aborted { reason, .. }) => CliOutput {
            exit_code: 1,
            stdout,
            stderr: format!("{reason}\n"),
        },
        Ok(PromptTurnOutcome::Failed { error, .. }) => CliOutput {
            exit_code: 1,
            stdout,
            stderr: format!("{error}\n"),
        },
        Err(error) => {
            let _ = push_coding_protocol_events(
                &mut stdout,
                &mut adapter,
                &CodingAgentEvent::PromptFailed {
                    operation_id: "json_prompt".into(),
                    error: error.clone(),
                },
            );
            CliOutput {
                exit_code: 1,
                stdout,
                stderr: format!("{error}\n"),
            }
        }
    }
}

async fn run_json_prompt(
    options: SessionPromptOptions,
    stdout: &mut String,
    adapter: &mut CodingProtocolEventAdapter,
) -> Result<PromptTurnOutcome, CodingSessionError> {
    let mut session = open_json_coding_session(&options).await?;
    let mut receiver = session.subscribe();
    let prompt_options = PromptTurnOptions::from_session_prompt_options(options);
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let _ = done_tx.send(session.prompt(prompt_options).await);
    });

    loop {
        tokio::select! {
            event = receiver.recv() => match event {
                Ok(event) => push_coding_protocol_events(stdout, adapter, &event)?,
                Err(CodingSessionError::Cancelled) => {
                    return done_rx.await.map_err(|_| CodingSessionError::Cancelled)?;
                }
                Err(error) => return Err(error),
            },
            result = &mut done_rx => {
                drain_json_events(&mut receiver, stdout, adapter)?;
                return result.map_err(|_| CodingSessionError::Cancelled)?;
            }
        }
    }
}

async fn open_json_coding_session(
    options: &SessionPromptOptions,
) -> Result<CodingAgentSession, CodingSessionError> {
    let Some(session_options) = options.session.as_ref() else {
        return CodingAgentSession::non_persistent(CodingAgentSessionOptions::new()).await;
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new().with_cwd(session_options.cwd.clone()),
        )
        .await;
    }

    let session_root = json_coding_session_root(session_options)?;
    let coding_options = CodingAgentSessionOptions::new()
        .with_cwd(session_options.cwd.clone())
        .with_session_log_root(session_root);

    match options
        .session_target
        .as_ref()
        .unwrap_or(&ResolvedSessionTarget::New)
    {
        ResolvedSessionTarget::New => CodingAgentSession::create(coding_options).await,
        ResolvedSessionTarget::OpenTarget(session_id) => {
            CodingAgentSession::open(coding_options.with_session_id(session_id.clone())).await
        }
        ResolvedSessionTarget::OpenOrCreateId(session_id) => {
            CodingAgentSession::open_or_create(coding_options.with_session_id(session_id.clone()))
                .await
        }
        ResolvedSessionTarget::ContinueMostRecent => {
            let session_id = CodingAgentSession::list(coding_options.clone())?
                .into_iter()
                .next()
                .map(|summary| summary.session_id)
                .ok_or_else(|| CodingSessionError::Session {
                    message: "no previous session to continue".into(),
                })?;
            CodingAgentSession::open(coding_options.with_session_id(session_id)).await
        }
        ResolvedSessionTarget::ForkTarget(_) => Err(CodingSessionError::UnsupportedCapability {
            capability: "Rust-native session fork".into(),
        }),
    }
}

fn json_coding_session_root(options: &SessionRunOptions) -> Result<PathBuf, CodingSessionError> {
    match options.session_dir.as_ref() {
        Some(root) => Ok(root.clone()),
        None => resolve_session_dir(&options.cwd, None, None).map_err(|error| {
            CodingSessionError::Session {
                message: error.to_string(),
            }
        }),
    }
}

fn json_header_cwd(options: &SessionPromptOptions) -> String {
    options
        .session
        .as_ref()
        .map(|session| session.cwd.clone())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string()
}

fn drain_json_events(
    receiver: &mut crate::coding_session::CodingAgentEventReceiver,
    stdout: &mut String,
    adapter: &mut CodingProtocolEventAdapter,
) -> Result<(), CodingSessionError> {
    loop {
        match receiver.try_recv() {
            Ok(Some(event)) => push_coding_protocol_events(stdout, adapter, &event)?,
            Ok(None) | Err(CodingSessionError::Cancelled) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn push_coding_protocol_events(
    stdout: &mut String,
    adapter: &mut CodingProtocolEventAdapter,
    event: &CodingAgentEvent,
) -> Result<(), CodingSessionError> {
    for protocol_event in adapter.push(event) {
        stdout.push_str(&serialize_json_line(&protocol_event).map_err(|error| {
            CodingSessionError::Provider {
                message: error.to_string(),
            }
        })?);
    }
    Ok(())
}
