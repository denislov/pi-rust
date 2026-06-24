use crate::config::{self, Config, ConfigPaths};
use crate::input::{self, ProcessedPromptInput};
use crate::protocol::session_runner::SessionPromptOptions;
use crate::resources::{self, LoadedResources};
use crate::runtime::{
    CliRunOptions, PromptInvocation, SessionRunOptions, effective_no_context_files,
    effective_session_dir, select_model,
};
use crate::session::ResolvedSessionTarget;
use crate::tools::{self, ToolFilter};
use crate::{CliArgs, CliError};
use pi_agent_core::AgentResources;
use pi_ai::types::Model;
use std::path::PathBuf;

pub struct ResolvedCliContext {
    pub cwd: PathBuf,
    pub parsed: CliArgs,
    pub config: Config,
    pub config_paths: ConfigPaths,
    pub model: Model,
    pub api_key: Option<String>,
    pub loaded_resources: LoadedResources,
    pub system_prompt: Option<String>,
    pub tools: Vec<pi_agent_core::AgentTool>,
    pub register_builtins: bool,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
    pub agent_resources: AgentResources,
}

pub struct ResolvedPromptRequest {
    pub context: ResolvedCliContext,
    pub processed_prompt: ProcessedPromptInput,
    pub invocation: PromptInvocation,
    pub session_options: SessionPromptOptions,
}

pub fn resolve_cli_context(
    parsed: CliArgs,
    options: CliRunOptions,
    cwd: PathBuf,
    global_dir: PathBuf,
) -> Result<ResolvedCliContext, CliError> {
    let config_paths = config::ConfigPaths {
        global_dir,
        project_dir: cwd.join(".pi-rust"),
    };
    let mut config_diags = Vec::new();
    let config = Config {
        settings: config::settings::load_settings(&config_paths, &mut config_diags),
        auth: config::auth::AuthStore::load(&config_paths.global_auth(), &mut config_diags),
    };
    let diag_text = config::drain_diagnostics(&config_diags);
    if !diag_text.is_empty() {
        eprint!("{diag_text}");
    }

    let model = select_model(
        &parsed,
        config.settings.default_provider.as_deref(),
        config.settings.default_model.as_deref(),
        options.model_override,
    )?;

    let provider = model.provider.clone();
    let api_key = resolve_api_key(&provider, parsed.api_key.as_deref(), &config);
    let loaded_resources = resources::load_cli_resources_with_options(
        &parsed.skills,
        &parsed.prompt_templates,
        &cwd,
        &config_paths.global_dir,
        resources::ResourceLoadOptions {
            no_skills: parsed.no_skills,
            no_prompt_templates: parsed.no_prompt_templates,
            no_themes: parsed.no_themes,
            skill_paths: config.settings.skills.clone(),
            prompt_paths: config.settings.prompts.clone(),
            theme_paths: config.settings.themes.clone(),
            theme: config.settings.theme.clone(),
        },
    )?;
    resources::print_diagnostics(&loaded_resources.diagnostics);

    validate_selected_resources(&parsed, &loaded_resources)?;

    let system_prompt = resolve_system_prompt(&parsed, &config, &config_paths, &cwd);
    let tools = tools::filter_tools(
        options.tools,
        &ToolFilter {
            allow: parsed.tools.clone(),
            deny: parsed.exclude_tools.clone(),
            no_tools: parsed.no_tools,
            no_builtin_tools: parsed.no_builtin_tools,
        },
    );

    let session = resolve_session_options(&parsed, &config, options.session);
    let session_target = resolve_session_target(&parsed);
    let session_name = parsed.name.clone();
    let agent_resources = resources::build_agent_resources(
        loaded_resources.skills.clone(),
        loaded_resources.prompt_templates.clone(),
    );

    Ok(ResolvedCliContext {
        cwd,
        parsed,
        config,
        config_paths,
        model,
        api_key,
        loaded_resources,
        system_prompt,
        tools,
        register_builtins: options.register_builtins,
        session,
        session_target,
        session_name,
        agent_resources,
    })
}

pub fn resolve_prompt_request(
    parsed: CliArgs,
    options: CliRunOptions,
    stdin: Option<String>,
    cwd: PathBuf,
    global_dir: PathBuf,
) -> Result<ResolvedPromptRequest, CliError> {
    let prompt = match parsed.prompt.clone() {
        Some(prompt) if !prompt.trim().is_empty() => prompt,
        _ if stdin
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()) =>
        {
            String::new()
        }
        _ => return Err(CliError::MissingPrompt),
    };
    let merged_prompt = input::merge_stdin_prompt(&prompt, stdin.as_deref());
    let context = resolve_cli_context(parsed, options, cwd, global_dir)?;
    let processed_prompt = input::process_at_file_references(&merged_prompt, &context.cwd)?;
    let invocation = resolve_invocation(&context, &processed_prompt);

    let session_options = SessionPromptOptions {
        prompt: match &invocation {
            PromptInvocation::Text(text) => text.clone(),
            PromptInvocation::Content(_) => processed_prompt.text.clone(),
            _ => String::new(),
        },
        model: context.model.clone(),
        api_key: context.api_key.clone(),
        system_prompt: context.system_prompt.clone(),
        max_turns: context.parsed.max_turns,
        tools: context.tools.clone(),
        register_builtins: context.register_builtins,
        session: context.session.clone(),
        session_target: context.session_target.clone(),
        session_name: context.session_name.clone(),
        thinking_level: context.parsed.thinking,
        tool_execution: context.parsed.tool_execution,
        resources: context.agent_resources.clone(),
        settings: Some(context.config.settings.clone()),
        invocation: invocation.clone(),
    };

    Ok(ResolvedPromptRequest {
        context,
        processed_prompt,
        invocation,
        session_options,
    })
}

pub fn resolve_session_target(parsed: &CliArgs) -> Option<ResolvedSessionTarget> {
    if parsed.no_session {
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
    }
}

fn resolve_api_key(provider: &str, cli_api_key: Option<&str>, config: &Config) -> Option<String> {
    let mut key_diags = Vec::new();
    let resolved =
        config::auth::resolve_api_key(provider, cli_api_key, &config.auth, &mut key_diags);
    let key_text = config::drain_diagnostics(&key_diags);
    if !key_text.is_empty() {
        eprint!("{key_text}");
    }
    resolved.map(|resolved| resolved.value)
}

fn validate_selected_resources(parsed: &CliArgs, loaded: &LoadedResources) -> Result<(), CliError> {
    if let Some(ref skill_name) = parsed.skill {
        if resources::find_skill(&loaded.skills, skill_name).is_none() {
            return Err(CliError::InvalidInput(format!(
                "skill '{skill_name}' not found in loaded skills"
            )));
        }
    }
    if let Some(ref template_name) = parsed.prompt_template {
        if resources::find_template(&loaded.prompt_templates, template_name).is_none() {
            return Err(CliError::InvalidInput(format!(
                "prompt template '{template_name}' not found in loaded templates"
            )));
        }
    }
    Ok(())
}

fn resolve_system_prompt(
    parsed: &CliArgs,
    config: &Config,
    config_paths: &ConfigPaths,
    cwd: &std::path::Path,
) -> Option<String> {
    let context_files = resources::discover_context_files(
        cwd,
        &config_paths.global_dir,
        effective_no_context_files(parsed, &config.settings),
    );
    let mut system_prompt = parsed.system_prompt.clone();
    if !context_files.is_empty() || !parsed.append_system_prompt.is_empty() {
        let mut parts = Vec::new();
        if let Some(base) = system_prompt.take() {
            parts.push(base);
        }
        for file in context_files {
            parts.push(format!(
                "# Context file: {}\n{}",
                file.path.display(),
                file.content
            ));
        }
        parts.extend(parsed.append_system_prompt.clone());
        system_prompt = Some(parts.join("\n\n"));
    }
    system_prompt
}

fn resolve_session_options(
    parsed: &CliArgs,
    config: &Config,
    mut session_options: SessionRunOptions,
) -> Option<SessionRunOptions> {
    if parsed.no_session {
        return None;
    }
    if let Some(dir) = effective_session_dir(parsed, &config.settings) {
        session_options.session_dir = Some(dir);
    }
    Some(session_options)
}

fn resolve_invocation(
    context: &ResolvedCliContext,
    processed_prompt: &ProcessedPromptInput,
) -> PromptInvocation {
    if let Some(ref skill_name) = context.parsed.skill {
        PromptInvocation::Skill {
            name: skill_name.clone(),
            additional_instructions: None,
        }
    } else if let Some(ref template_name) = context.parsed.prompt_template {
        PromptInvocation::PromptTemplate {
            name: template_name.clone(),
            args: context.parsed.template_args.clone(),
        }
    } else if processed_prompt.images.is_empty() {
        PromptInvocation::Text(processed_prompt.text.clone())
    } else {
        PromptInvocation::Content(processed_prompt.content.clone())
    }
}
