pub(crate) mod args;
pub(crate) mod error;
pub(crate) mod input;
pub(crate) mod list_models;
pub(crate) mod models;
pub(crate) mod prompt_options;
pub(crate) mod request;

use crate::adapters;
use crate::app::bootstrap::{CliRunOptions, SessionRunOptions};
use crate::config;
use crate::tools::builtin_tools;
use args::{CliMode, help_text, parse_args};
use error::CliError;
use std::io::{IsTerminal, Read};

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
        ai_client: None,
        session: SessionRunOptions::enabled(cwd),
    }
}

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    run_cli_with_options(args, default_cli_options(cwd)).await
}

pub async fn run_cli_stdio(args: impl IntoIterator<Item = String>) -> CliOutput {
    let args = args.into_iter().collect::<Vec<_>>();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let options = default_cli_options(cwd);

    if parse_args(args.clone()).is_ok_and(|parsed| parsed.mode == CliMode::Rpc) {
        return match adapters::rpc::run_rpc_mode_stdio(options).await {
            Ok(()) => CliOutput::success(String::new()),
            Err(error) => CliOutput::failure(error),
        };
    }

    let stdin = if std::io::stdin().is_terminal() {
        None
    } else {
        let mut input = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut input) {
            return CliOutput::failure(CliError::InvalidInput(format!(
                "failed to read stdin: {error}"
            )));
        }
        Some(input)
    };
    run_cli_with_options_and_stdin(args, options, stdin).await
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
        return adapters::interactive::run_interactive_mode(parsed, options).await;
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
            match adapters::print::run_print_prompt_options(session_prompt_options).await {
                Ok(text) => CliOutput::success(stdout_with_trailing_newline(text))
                    .with_stderr(diagnostic_text),
                Err(error) => CliOutput::failure(error).with_stderr(diagnostic_text),
            }
        }
        CliMode::Json => adapters::json::run_json_mode(session_prompt_options)
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
        let names: Vec<_> = options
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["read", "write", "edit", "bash", "grep", "find", "ls"]
        );
        assert!(options.register_builtins);
        assert!(options.model_override.is_none());
    }
}
