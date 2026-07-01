use crate::{CliError, runtime::SessionRunOptions};
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

pub fn session_root_from_run_options(options: &SessionRunOptions) -> Result<PathBuf, CliError> {
    match options.session_dir.as_ref() {
        Some(root) => Ok(root.clone()),
        None => resolve_session_dir(&options.cwd, None, None),
    }
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
