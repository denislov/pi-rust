pub mod args;
pub mod error;
pub mod print_mode;
pub mod runtime;

pub use args::{CliArgs, DEFAULT_MAX_TURNS, help_text, parse_args};
pub use error::CliError;
pub use print_mode::{PrintModeOptions, run_print_mode};
pub use runtime::{
    CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, build_agent_config, select_model,
};

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

pub async fn run_cli(args: impl IntoIterator<Item = String>) -> CliOutput {
    run_cli_with_options(args, CliRunOptions::default()).await
}

pub async fn run_cli_with_options(
    args: impl IntoIterator<Item = String>,
    options: CliRunOptions,
) -> CliOutput {
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

    if !parsed.print {
        return CliOutput::failure(CliError::UnsupportedMode("interactive".into()));
    }

    let prompt = match parsed.prompt.clone() {
        Some(prompt) if !prompt.trim().is_empty() => prompt,
        _ => return CliOutput::failure(CliError::MissingPrompt),
    };

    let model = match select_model(&parsed, options.model_override) {
        Ok(model) => model,
        Err(error) => return CliOutput::failure(error),
    };

    match run_print_mode(PrintModeOptions {
        prompt,
        model,
        api_key: parsed.api_key,
        system_prompt: parsed.system_prompt,
        max_turns: parsed.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
    })
    .await
    {
        Ok(text) => CliOutput::success(stdout_with_trailing_newline(text)),
        Err(error) => CliOutput::failure(error),
    }
}
