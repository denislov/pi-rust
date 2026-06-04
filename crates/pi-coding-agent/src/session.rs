use crate::{CliError, runtime::SessionRunOptions};
use pi_agent_core::session::{self, JsonlSessionRepo, StoredAgentMessage};
use std::path::{Path, PathBuf};

pub fn encode_cwd(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| match c {
            '/' | '\\' => '-',
            '"' | '<' | '>' | ':' | '|' | '?' | '*' | '\'' => '-',
            ' ' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn default_sessions_root() -> Result<PathBuf, CliError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    Ok(PathBuf::from(home)
        .join(".pi")
        .join("agent")
        .join("sessions"))
}

pub fn resolve_session_dir(
    _cwd: &Path,
    cli_session_dir: Option<&str>,
    runtime_session_dir: Option<&Path>,
) -> Result<PathBuf, CliError> {
    if let Some(dir) = cli_session_dir {
        return Ok(PathBuf::from(dir));
    }

    if let Some(dir) = runtime_session_dir {
        return Ok(dir.to_path_buf());
    }

    if let Ok(env_dir) = std::env::var("PI_SESSION_DIR") {
        return Ok(PathBuf::from(env_dir));
    }

    if let Ok(agent_dir) = std::env::var("PI_AGENT_DIR") {
        return Ok(PathBuf::from(agent_dir).join("sessions"));
    }

    default_sessions_root()
}

#[derive(Debug, Clone)]
pub enum ResolvedSessionTarget {
    New,
    ContinueMostRecent,
    OpenTarget(String),
    OpenOrCreateId(String),
    ForkTarget(String),
}

#[derive(Debug)]
pub struct ActiveSession {
    pub storage: session::JsonlSessionStorage,
    pub baseline_messages: usize,
}

pub fn agent_message_to_stored(msg: &pi_agent_core::AgentMessage) -> Option<StoredAgentMessage> {
    session::agent_message_to_stored(msg)
}

pub fn append_agent_message(
    storage: &mut session::JsonlSessionStorage,
    msg: &pi_agent_core::AgentMessage,
    entry_id: String,
    parent_id: Option<String>,
    timestamp: String,
) -> Result<(), CliError> {
    if let Some(stored) = agent_message_to_stored(msg) {
        let entry =
            pi_agent_core::session::SessionEntry::message(entry_id, parent_id, timestamp, stored);
        storage
            .append_entry(entry)
            .map_err(|e| CliError::SessionFailure(e.message))?;
    }
    Ok(())
}

pub fn open_active_session(
    target: ResolvedSessionTarget,
    options: &SessionRunOptions,
) -> Result<ActiveSession, CliError> {
    let cwd_str = options.cwd.display().to_string();
    let sessions_root = match &options.session_dir {
        Some(dir) => dir.clone(),
        None => resolve_session_dir(&options.cwd, None, None)?,
    };

    let repo = JsonlSessionRepo::new(&sessions_root);

    let storage = match target {
        ResolvedSessionTarget::New => repo
            .create(&cwd_str, None)
            .map_err(|e| CliError::SessionFailure(e.message))?,
        ResolvedSessionTarget::ContinueMostRecent => repo
            .most_recent(&cwd_str)
            .map_err(|e| CliError::SessionFailure(e.message))?
            .ok_or_else(|| CliError::SessionFailure("no previous session to continue".into()))?,
        ResolvedSessionTarget::OpenTarget(ref target_val) => repo
            .open_target(&cwd_str, target_val)
            .map_err(|e| CliError::SessionFailure(e.message))?,
        ResolvedSessionTarget::OpenOrCreateId(ref id) => match repo.open_target(&cwd_str, id) {
            Ok(storage) => storage,
            Err(_) => repo
                .create(&cwd_str, Some(id))
                .map_err(|e| CliError::SessionFailure(e.message))?,
        },
        ResolvedSessionTarget::ForkTarget(ref target_val) => {
            let repo = JsonlSessionRepo::new(&sessions_root);
            let source = repo
                .open_target(&cwd_str, target_val)
                .map_err(|e| CliError::SessionFailure(e.message))?;
            let source_path = source.path().to_path_buf();
            repo.fork(&source_path, &cwd_str, None, None)
                .map_err(|e| CliError::SessionFailure(e.message))?
        }
    };

    let entries = storage.get_entries();
    let context = session::build_session_context(&entries, None)
        .map_err(|e| CliError::SessionFailure(e.message))?;
    let baseline = context.messages.len();

    Ok(ActiveSession {
        storage,
        baseline_messages: baseline,
    })
}
