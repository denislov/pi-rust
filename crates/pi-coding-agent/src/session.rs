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
    let global_dir = match std::env::var_os("PI_RUST_DIR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pi-rust"),
    };
    Ok(global_dir.join("sessions"))
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
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    session::agent_message_to_stored(msg, timestamp_ms)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sessions_root_uses_pi_rust_dir() {
        let _guard = crate::test_support::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let prior_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
        unsafe {
            std::env::set_var("PI_RUST_DIR", dir.path());
        }

        let root = default_sessions_root().unwrap();

        unsafe {
            match prior_pi_rust_dir {
                Some(value) => std::env::set_var("PI_RUST_DIR", value),
                None => std::env::remove_var("PI_RUST_DIR"),
            }
        }

        assert_eq!(root, dir.path().join("sessions"));
        assert!(
            !root.display().to_string().contains(".pi/agent"),
            "default sessions root must not use the legacy ~/.pi tree: {}",
            root.display()
        );
    }

    #[test]
    fn resolve_session_dir_ignores_legacy_pi_agent_dir() {
        let _guard = crate::test_support::env_lock();
        let global = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        let prior_pi_rust_dir = std::env::var_os("PI_RUST_DIR");
        let prior_pi_agent_dir = std::env::var_os("PI_AGENT_DIR");
        let prior_pi_session_dir = std::env::var_os("PI_SESSION_DIR");
        unsafe {
            std::env::set_var("PI_RUST_DIR", global.path());
            std::env::set_var("PI_AGENT_DIR", legacy.path());
            std::env::remove_var("PI_SESSION_DIR");
        }

        let root = resolve_session_dir(Path::new("."), None, None).unwrap();

        unsafe {
            match prior_pi_rust_dir {
                Some(value) => std::env::set_var("PI_RUST_DIR", value),
                None => std::env::remove_var("PI_RUST_DIR"),
            }
            match prior_pi_agent_dir {
                Some(value) => std::env::set_var("PI_AGENT_DIR", value),
                None => std::env::remove_var("PI_AGENT_DIR"),
            }
            match prior_pi_session_dir {
                Some(value) => std::env::set_var("PI_SESSION_DIR", value),
                None => std::env::remove_var("PI_SESSION_DIR"),
            }
        }

        assert_eq!(root, global.path().join("sessions"));
    }
}
