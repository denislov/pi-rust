mod coding_session;
mod plugins;

pub mod args;
pub mod config;
pub mod error;
pub mod input;
pub mod interactive;
mod list_models;
pub mod models;
pub mod print_mode;
pub mod prompt_options;
pub mod protocol;
pub mod request;
pub mod resources;
pub mod runtime;
pub mod session;
pub mod theme;
pub mod tools;

pub use args::{CliArgs, CliMode, help_text, parse_args};
pub use error::CliError;
pub use print_mode::{PrintModeOptions, run_print_mode};
pub use prompt_options::PromptRunOptions;
pub use runtime::{
    CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, PromptInvocation, SessionMode,
    SessionRunOptions, build_agent_config, effective_no_context_files, effective_session_dir,
    select_model,
};
pub use session::{ResolvedSessionTarget, encode_cwd};
pub use tools::builtin_tools;

/// Stable library facade for embedding or scripting `pi-coding-agent`.
///
/// The root modules remain public during the migration, but downstream crates
/// should prefer this module for APIs that are intended to stay stable.
pub mod api {
    pub use crate::args::{CliArgs, CliMode, help_text, parse_args};
    pub use crate::coding_session::{
        AgentProfile, CapabilityStatus, CodingAgentCapabilities, CodingAgentEvent,
        CodingAgentEventReceiver, CodingAgentSession, CodingAgentSessionExport,
        CodingAgentSessionExportItem, CodingAgentSessionOptions, CodingAgentSessionSummary,
        CodingAgentSessionView, CodingDiagnostic, CodingDiagnosticSeverity, CodingSessionError,
        DelegationConfirmationMode, DelegationPolicy, ProfileDiagnostic, ProfileId, ProfileKind,
        ProfileRegistry, ProfileRegistryOptions, ProfileSource, PromptTurnMode, PromptTurnOptions,
        PromptTurnOutcome, SupervisionPolicy, TeamProfile, TeamStrategy, TeamSupervisor,
    };
    pub use crate::error::CliError;
    pub use crate::print_mode::{PrintModeOptions, run_print_mode};
    pub use crate::prompt_options::PromptRunOptions;
    pub use crate::request::{
        CliDiagnostic, CliDiagnosticSeverity, ResolvedCliContext, ResolvedPromptRequest,
        render_diagnostics, resolve_cli_context, resolve_prompt_request, resolve_session_target,
    };
    pub use crate::runtime::{
        CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, PromptInvocation, SessionMode,
        SessionRunOptions, build_agent_config, effective_no_context_files, effective_session_dir,
        select_model,
    };
    pub use crate::session::{ResolvedSessionTarget, encode_cwd};
    pub use crate::tools::{ToolFilter, builtin_tools, filter_tools};
    pub use crate::{CliOutput, run_cli, run_cli_with_options, run_cli_with_options_and_stdin};
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    fn success(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(error: CliError) -> Self {
        Self {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        }
    }

    fn with_stderr(mut self, stderr: String) -> Self {
        if !stderr.is_empty() {
            self.stderr = format!("{stderr}{}", self.stderr);
        }
        self
    }
}

fn stdout_with_trailing_newline(text: String) -> String {
    if text.is_empty() {
        String::new()
    } else if text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    }
}

fn default_cli_options(cwd: std::path::PathBuf) -> CliRunOptions {
    CliRunOptions {
        model_override: None,
        tools: builtin_tools(cwd.clone()),
        register_builtins: true,
        session: SessionRunOptions::enabled(cwd),
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    run_cli_with_options(args, default_cli_options(cwd)).await
}

pub async fn run_cli_with_options(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
) -> CliOutput {
    run_cli_with_options_and_stdin(args, options, None).await
}

pub async fn run_cli_with_options_and_stdin(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
    stdin: Option<String>,
) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return CliOutput::failure(error),
    };

    if parsed.help {
        return CliOutput::success(help_text());
    }

    if parsed.version {
        return CliOutput::success(format!("{}\n", env!("CARGO_PKG_VERSION")));
    }

    if let Some(search) = parsed.list_models.as_ref() {
        return match list_models::list_models_output(
            search.as_deref(),
            parsed.provider.as_deref(),
            parsed.json,
        ) {
            Ok(stdout) => CliOutput::success(stdout),
            Err(error) => CliOutput::failure(error),
        };
    }

    if !parsed.print && !parsed.mode_explicit {
        return interactive::run_interactive_mode(parsed, options).await;
    }

    if parsed.mode == CliMode::Rpc {
        return CliOutput::failure(CliError::UnsupportedMode(
            "rpc requires the streaming binary entry point".into(),
        ));
    }

    if let Some(models) = parsed.models.as_deref()
        && let Err(error) = models::parse_model_rotation(models)
    {
        return CliOutput::failure(error);
    }

    let config_paths = config::resolve_paths(&cwd);
    let resolved = match request::resolve_prompt_request(
        parsed.clone(),
        options,
        stdin,
        cwd,
        config_paths.global_dir,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return CliOutput::failure(error),
    };
    let session_prompt_options = resolved.session_options;
    let diagnostic_text = request::render_diagnostics(&resolved.context.diagnostics);

    match parsed.mode {
        CliMode::Print => {
            match run_print_mode(PrintModeOptions::from(session_prompt_options)).await {
                Ok(text) => CliOutput::success(stdout_with_trailing_newline(text))
                    .with_stderr(diagnostic_text),
                Err(error) => CliOutput::failure(error).with_stderr(diagnostic_text),
            }
        }
        CliMode::Json => protocol::json_mode::run_json_mode(session_prompt_options)
            .await
            .with_stderr(diagnostic_text),
        CliMode::Rpc => CliOutput::failure(CliError::UnsupportedMode(
            "rpc requires the streaming binary entry point".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cli_options_include_builtin_tools() {
        let options = default_cli_options(std::path::PathBuf::from("."));
        let names: Vec<_> = options.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["read", "write", "edit", "bash", "grep", "find", "ls"]
        );
        assert!(options.register_builtins);
        assert!(options.model_override.is_none());
    }
}
