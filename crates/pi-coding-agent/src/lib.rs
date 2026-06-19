pub mod args;
pub mod config;
pub mod error;
pub mod interactive;
pub mod print_mode;
pub mod protocol;
pub mod resources;
pub mod runtime;
pub mod session;
pub mod tools;

pub use args::{CliArgs, CliMode, DEFAULT_MAX_TURNS, help_text, parse_args};
pub use error::CliError;
pub use print_mode::{PrintModeOptions, run_print_mode};
pub use runtime::{
    CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, PromptInvocation, SessionMode,
    SessionRunOptions, build_agent_config, select_model,
};
pub use session::{ActiveSession, ResolvedSessionTarget, encode_cwd, open_active_session};
pub use tools::builtin_tools;

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

    if !parsed.print && !parsed.mode_explicit {
        return interactive::run_interactive_mode(parsed, options).await;
    }

    if parsed.mode == CliMode::Rpc {
        return CliOutput::failure(CliError::UnsupportedMode(
            "rpc requires the streaming binary entry point".into(),
        ));
    }

    let prompt = match parsed.prompt.clone() {
        Some(prompt) if !prompt.trim().is_empty() => prompt,
        _ => return CliOutput::failure(CliError::MissingPrompt),
    };

    let (config, config_diags) = config::load_config(&cwd);
    let diag_text = config::drain_diagnostics(&config_diags);
    if !diag_text.is_empty() {
        eprint!("{diag_text}");
    }

    let model = match select_model(
        &parsed,
        config.settings.default_model.as_deref(),
        options.model_override,
    ) {
        Ok(model) => model,
        Err(error) => return CliOutput::failure(error),
    };

    let provider = model.provider.clone();
    let resolved_api_key = {
        let mut key_diags = Vec::new();
        let resolved = config::auth::resolve_api_key(
            &provider,
            parsed.api_key.as_deref(),
            &config.auth,
            &mut key_diags,
        );
        let key_text = config::drain_diagnostics(&key_diags);
        if !key_text.is_empty() {
            eprint!("{key_text}");
        }
        resolved.map(|r| r.value)
    };

    let (skills, templates, diags) =
        match resources::load_cli_resources(&parsed.skills, &parsed.prompt_templates, &cwd) {
            Ok((s, t, d)) => (s, t, d),
            Err(error) => return CliOutput::failure(error),
        };
    resources::print_diagnostics(&diags);

    let invocation = if let Some(ref skill_name) = parsed.skill {
        if resources::find_skill(&skills, skill_name).is_none() {
            return CliOutput::failure(CliError::InvalidInput(format!(
                "skill '{skill_name}' not found in loaded skills"
            )));
        }
        PromptInvocation::Skill {
            name: skill_name.clone(),
            additional_instructions: None,
        }
    } else if let Some(ref template_name) = parsed.prompt_template {
        if resources::find_template(&templates, template_name).is_none() {
            return CliOutput::failure(CliError::InvalidInput(format!(
                "prompt template '{template_name}' not found in loaded templates"
            )));
        }
        PromptInvocation::PromptTemplate {
            name: template_name.clone(),
            args: parsed.template_args.clone(),
        }
    } else {
        PromptInvocation::Text(prompt)
    };

    let agent_resources = resources::build_agent_resources(skills.to_vec(), templates.to_vec());

    let session_enabled = !parsed.no_session;
    let session = if session_enabled {
        let mut session_opts = options.session.clone();
        if let Some(ref dir) = parsed.session_dir {
            session_opts.session_dir = Some(std::path::PathBuf::from(dir));
        }
        Some(session_opts)
    } else {
        None
    };

    let session_target = if parsed.no_session {
        None
    } else if let Some(ref fork_target) = parsed.fork {
        Some(ResolvedSessionTarget::ForkTarget(fork_target.clone()))
    } else if let Some(ref session_target) = parsed.session {
        Some(ResolvedSessionTarget::OpenTarget(session_target.clone()))
    } else if let Some(ref session_id) = parsed.session_id {
        Some(ResolvedSessionTarget::OpenOrCreateId(session_id.clone()))
    } else if parsed.continue_session || parsed.resume {
        Some(ResolvedSessionTarget::ContinueMostRecent)
    } else {
        None
    };

    let session_name = parsed.name.clone();

    let session_prompt_options = protocol::session_runner::SessionPromptOptions {
        prompt: match &invocation {
            PromptInvocation::Text(t) => t.clone(),
            _ => String::new(),
        },
        model,
        api_key: resolved_api_key,
        system_prompt: parsed.system_prompt,
        max_turns: parsed.max_turns,
        tools: options.tools,
        register_builtins: options.register_builtins,
        session,
        session_target,
        session_name,
        thinking_level: parsed.thinking,
        tool_execution: parsed.tool_execution,
        resources: agent_resources,
        invocation,
    };

    match parsed.mode {
        CliMode::Print => {
            match run_print_mode(PrintModeOptions::from(session_prompt_options)).await {
                Ok(text) => CliOutput::success(stdout_with_trailing_newline(text)),
                Err(error) => CliOutput::failure(error),
            }
        }
        CliMode::Json => protocol::json_mode::run_json_mode(session_prompt_options).await,
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
