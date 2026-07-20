use crate::app::bootstrap::{SessionMode, SessionRunOptions};
use crate::app::cli::error::CliError;
use crate::app::cli::prompt_options::PromptRunOptions;
use crate::authorization::ToolAuthorizationMode;
use crate::runtime::facade::{
    CodingAgentSession, CodingAgentSessionHydration, CodingAgentSessionOptions,
    CodingAgentSessionTree, CodingSessionError, ProfileId,
};
use pi_ai::api::client::AiClient;
use std::path::{Path, PathBuf};

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

pub(crate) async fn open_headless_prompt_session(
    options: &PromptRunOptions,
) -> Result<CodingAgentSession, CodingSessionError> {
    let Some(session_options) = options.session.as_ref() else {
        ensure_non_persistent_target(options.session_target.as_ref())?;
        return CodingAgentSession::non_persistent(with_ai_client(
            CodingAgentSessionOptions::new(),
            options.ai_client.as_ref(),
        ))
        .await;
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        ensure_non_persistent_target(options.session_target.as_ref())?;
        return CodingAgentSession::non_persistent(with_ai_client(
            CodingAgentSessionOptions::new().with_cwd(session_options.cwd.clone()),
            options.ai_client.as_ref(),
        ))
        .await;
    }

    let session_root = headless_session_root(session_options)?;
    let session_options = with_ai_client(
        CodingAgentSessionOptions::new()
            .with_cwd(session_options.cwd.clone())
            .with_session_log_root(session_root),
        options.ai_client.as_ref(),
    );
    open_persistent_session(session_options, options.session_target.as_ref()).await
}

pub(crate) fn runtime_session_root(
    options: &SessionRunOptions,
) -> Result<Option<PathBuf>, CodingSessionError> {
    if matches!(options.mode, SessionMode::Enabled) {
        headless_session_root(options).map(Some)
    } else {
        Ok(None)
    }
}

pub(crate) async fn open_new_runtime_session(
    options: &SessionRunOptions,
) -> Result<CodingAgentSession, CodingSessionError> {
    match runtime_session_root(options)? {
        Some(session_root) => {
            CodingAgentSession::create(
                CodingAgentSessionOptions::new()
                    .with_cwd(options.cwd.clone())
                    .with_session_log_root(session_root)
                    .with_tool_authorization_mode(ToolAuthorizationMode::Interactive),
            )
            .await
        }
        None => {
            CodingAgentSession::non_persistent(
                CodingAgentSessionOptions::new()
                    .with_cwd(options.cwd.clone())
                    .with_tool_authorization_mode(ToolAuthorizationMode::Interactive),
            )
            .await
        }
    }
}

pub(crate) async fn open_forked_runtime_session(
    options: &SessionRunOptions,
    parent_session_id: &str,
) -> Result<CodingAgentSession, CodingSessionError> {
    if !matches!(options.mode, SessionMode::Enabled) {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "parentSession requires persistent Rust-native sessions".into(),
        });
    }
    let session_root = headless_session_root(options)?;
    let base_options = CodingAgentSessionOptions::new()
        .with_cwd(options.cwd.clone())
        .with_session_log_root(session_root)
        .with_tool_authorization_mode(ToolAuthorizationMode::Interactive);
    let forked = CodingAgentSession::fork_session(
        base_options
            .clone()
            .with_session_id(parent_session_id.to_owned()),
        None,
    )?;
    CodingAgentSession::open(base_options.with_session_id(forked.summary.session_id)).await
}

pub(crate) async fn open_interactive_session(
    session_options: Option<&SessionRunOptions>,
    target: Option<&ResolvedSessionTarget>,
    default_agent_profile_id: ProfileId,
) -> Result<CodingAgentSession, CliError> {
    let Some(session_options) = session_options else {
        return Ok(CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new()
                .with_default_agent_profile_id(default_agent_profile_id)
                .with_tool_authorization_mode(ToolAuthorizationMode::Interactive),
        )
        .await?);
    };
    if !matches!(session_options.mode, SessionMode::Enabled) {
        return Ok(CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new()
                .with_cwd(session_options.cwd.clone())
                .with_default_agent_profile_id(default_agent_profile_id)
                .with_tool_authorization_mode(ToolAuthorizationMode::Interactive),
        )
        .await?);
    }

    let session_root = headless_session_root(session_options)?;
    let options = CodingAgentSessionOptions::new()
        .with_cwd(session_options.cwd.clone())
        .with_session_log_root(session_root)
        .with_default_agent_profile_id(default_agent_profile_id)
        .with_tool_authorization_mode(ToolAuthorizationMode::Interactive);
    match target.unwrap_or(&ResolvedSessionTarget::New) {
        ResolvedSessionTarget::New => Ok(CodingAgentSession::create(options).await?),
        ResolvedSessionTarget::OpenOrCreateId(session_id) => Ok(
            CodingAgentSession::open_or_create(options.with_session_id(session_id.clone())).await?,
        ),
        ResolvedSessionTarget::OpenTarget(target) => {
            if target_looks_like_rust_native_session_dir(target) {
                Ok(CodingAgentSession::open(options.with_session_path(target)).await?)
            } else if target_looks_like_legacy_jsonl(target) {
                Err(CodingSessionError::UnsupportedCapability {
                    capability: "legacy JSONL session targets".into(),
                }
                .into())
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
        ResolvedSessionTarget::ForkTarget(source) => {
            let forked = CodingAgentSession::fork_session(
                options.clone().with_session_id(source.clone()),
                None,
            )?;
            Ok(
                CodingAgentSession::open(options.with_session_id(forked.summary.session_id))
                    .await?,
            )
        }
    }
}

fn target_looks_like_rust_native_session_dir(target: &str) -> bool {
    let path = Path::new(target);
    path.is_dir() && path.join("session.json").is_file() && path.join("events.jsonl").is_file()
}

fn target_looks_like_legacy_jsonl(target: &str) -> bool {
    let path = Path::new(target);
    path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") || path.is_file()
}

pub(crate) fn hydrate_interactive_session_target(
    session_options: &Option<SessionRunOptions>,
    target: Option<&ResolvedSessionTarget>,
) -> Result<Option<CodingAgentSessionHydration>, CliError> {
    let Some(session_options) = enabled_session_options(session_options) else {
        return Ok(None);
    };
    let Some(target) = target else {
        return Ok(None);
    };
    let base_options = interactive_navigation_options(session_options)?;
    let hydration = match target {
        ResolvedSessionTarget::New | ResolvedSessionTarget::ForkTarget(_) => return Ok(None),
        ResolvedSessionTarget::ContinueMostRecent => {
            list_interactive_session_hydrations(&Some(session_options.clone()))?
                .into_iter()
                .next()
        }
        ResolvedSessionTarget::OpenOrCreateId(session_id) => {
            match CodingAgentSession::hydrate(base_options.with_session_id(session_id.clone())) {
                Ok(hydration) => Some(hydration),
                Err(_) => return Ok(None),
            }
        }
        ResolvedSessionTarget::OpenTarget(target) => {
            let is_path = target_looks_like_rust_native_session_dir(target);
            let options = if is_path {
                base_options.with_session_path(target)
            } else {
                base_options.with_session_id(target.clone())
            };
            match CodingAgentSession::hydrate(options) {
                Ok(hydration) => Some(hydration),
                Err(error) if is_path => {
                    return Err(CliError::SessionFailure(error.to_string()));
                }
                Err(_) => return Ok(None),
            }
        }
    };
    Ok(hydration.filter(|hydration| hydration_matches_cwd(hydration, &session_options.cwd)))
}

pub(crate) fn list_interactive_session_hydrations(
    session_options: &Option<SessionRunOptions>,
) -> Result<Vec<CodingAgentSessionHydration>, CliError> {
    let Some(session_options) = enabled_session_options(session_options) else {
        return Ok(Vec::new());
    };
    let options = interactive_navigation_options(session_options)?;
    Ok(CodingAgentSession::list(options.clone())?
        .into_iter()
        .filter_map(|summary| {
            CodingAgentSession::hydrate(options.clone().with_session_id(summary.session_id)).ok()
        })
        .filter(|hydration| hydration_matches_cwd(hydration, &session_options.cwd))
        .collect())
}

pub(crate) fn clone_interactive_session(
    session_path: &Path,
    cwd: &Path,
) -> Result<CodingAgentSessionHydration, CodingSessionError> {
    CodingAgentSession::clone_session(interactive_choice_options(session_path, cwd))
}

pub(crate) fn interactive_session_tree(
    session_path: &Path,
    cwd: &Path,
) -> Result<CodingAgentSessionTree, CodingSessionError> {
    CodingAgentSession::tree_view(interactive_choice_options(session_path, cwd))
}

pub(crate) fn export_interactive_session_html(
    session_path: &Path,
    cwd: &Path,
    output_path: &Path,
) -> Result<PathBuf, CodingSessionError> {
    CodingAgentSession::export_session_html(
        interactive_choice_options(session_path, cwd),
        output_path,
    )
}

fn enabled_session_options(
    session_options: &Option<SessionRunOptions>,
) -> Option<&SessionRunOptions> {
    session_options
        .as_ref()
        .filter(|options| matches!(options.mode, SessionMode::Enabled))
}

fn interactive_navigation_options(
    session_options: &SessionRunOptions,
) -> Result<CodingAgentSessionOptions, CodingSessionError> {
    Ok(CodingAgentSessionOptions::new()
        .with_cwd(session_options.cwd.clone())
        .with_session_log_root(headless_session_root(session_options)?))
}

fn interactive_choice_options(session_path: &Path, cwd: &Path) -> CodingAgentSessionOptions {
    let mut options = CodingAgentSessionOptions::new().with_session_path(session_path);
    if let Some(root) = session_path.parent() {
        options = options.with_session_log_root(root);
    }
    if !cwd.as_os_str().is_empty() {
        options = options.with_cwd(cwd);
    }
    options
}

fn hydration_matches_cwd(hydration: &CodingAgentSessionHydration, cwd: &Path) -> bool {
    let expected = normalized_path_string(cwd);
    hydration.cwd.as_deref() == Some(expected.as_str())
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn ensure_non_persistent_target(
    target: Option<&ResolvedSessionTarget>,
) -> Result<(), CodingSessionError> {
    match target {
        None | Some(ResolvedSessionTarget::New) => Ok(()),
        Some(_) => Err(CodingSessionError::UnsupportedCapability {
            capability: "persistent session target in non-persistent headless mode".into(),
        }),
    }
}

async fn open_persistent_session(
    options: CodingAgentSessionOptions,
    target: Option<&ResolvedSessionTarget>,
) -> Result<CodingAgentSession, CodingSessionError> {
    match target.unwrap_or(&ResolvedSessionTarget::New) {
        ResolvedSessionTarget::New => CodingAgentSession::create(options).await,
        ResolvedSessionTarget::OpenTarget(session_id) => {
            CodingAgentSession::open(options.with_session_id(session_id.clone())).await
        }
        ResolvedSessionTarget::OpenOrCreateId(session_id) => {
            CodingAgentSession::open_or_create(options.with_session_id(session_id.clone())).await
        }
        ResolvedSessionTarget::ContinueMostRecent => {
            let session_id = CodingAgentSession::list(options.clone())?
                .into_iter()
                .next()
                .map(|summary| summary.session_id)
                .ok_or_else(|| CodingSessionError::Session {
                    message: "no previous session to continue".into(),
                })?;
            CodingAgentSession::open(options.with_session_id(session_id)).await
        }
        ResolvedSessionTarget::ForkTarget(source) => {
            let forked = CodingAgentSession::fork_session(
                options.clone().with_session_id(source.clone()),
                None,
            )?;
            CodingAgentSession::open(options.with_session_id(forked.summary.session_id)).await
        }
    }
}

fn headless_session_root(options: &SessionRunOptions) -> Result<PathBuf, CodingSessionError> {
    match options.session_dir.as_ref() {
        Some(root) => Ok(root.clone()),
        None => resolve_session_dir(&options.cwd, None, None).map_err(|error| {
            CodingSessionError::Session {
                message: error.to_string(),
            }
        }),
    }
}

fn with_ai_client(
    options: CodingAgentSessionOptions,
    ai_client: Option<&AiClient>,
) -> CodingAgentSessionOptions {
    match ai_client {
        Some(ai_client) => options.with_ai_client(ai_client.clone()),
        None => options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sessions_root_uses_pi_rust_dir() {
        let env = crate::test_support::EnvGuard::new(&["PI_RUST_DIR"]);
        let dir = tempfile::tempdir().unwrap();
        env.set_pi_rust_dir(dir.path());

        let root = default_sessions_root().unwrap();

        assert_eq!(root, dir.path().join("sessions"));
        assert!(
            !root.display().to_string().contains(".pi/agent"),
            "default sessions root must not use the legacy ~/.pi tree: {}",
            root.display()
        );
    }

    #[test]
    fn resolve_session_dir_ignores_legacy_pi_agent_dir() {
        let env =
            crate::test_support::EnvGuard::new(&["PI_RUST_DIR", "PI_AGENT_DIR", "PI_SESSION_DIR"]);
        let global = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        env.set_pi_rust_dir(global.path());
        env.set("PI_AGENT_DIR", legacy.path());
        env.remove("PI_SESSION_DIR");

        let root = resolve_session_dir(Path::new("."), None, None).unwrap();

        assert_eq!(root, global.path().join("sessions"));
    }
}
