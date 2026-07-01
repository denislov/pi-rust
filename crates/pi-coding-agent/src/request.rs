use crate::config::{self, Config, ConfigPaths};
use crate::input::{self, ProcessedPromptInput};
use crate::prompt_options::PromptRunOptions;
use crate::resources::{self, LoadedResources};
use crate::runtime::{
    CliRunOptions, PromptInvocation, SessionRunOptions, effective_no_context_files,
    effective_session_dir, select_model,
};
use crate::session::ResolvedSessionTarget;
use crate::tools::{self, ToolFilter};
use crate::{CliArgs, CliError};
use pi_agent_core::types::DiagnosticSeverity as ResourceDiagnosticSeverity;
use pi_agent_core::{AgentResources, ResourceDiagnostic};
use pi_ai::types::Model;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliDiagnostic {
    pub severity: CliDiagnosticSeverity,
    pub message: String,
    pub source: Option<PathBuf>,
    pub code: Option<String>,
}

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
    pub diagnostics: Vec<CliDiagnostic>,
}

pub struct ResolvedPromptRequest {
    pub context: ResolvedCliContext,
    pub processed_prompt: ProcessedPromptInput,
    pub invocation: PromptInvocation,
    pub session_options: PromptRunOptions,
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
    let mut diagnostics = config_diags
        .iter()
        .map(CliDiagnostic::from_config)
        .collect::<Vec<_>>();

    let model = select_model(
        &parsed,
        config.settings.default_provider.as_deref(),
        config.settings.default_model.as_deref(),
        options.model_override,
    )?;

    let provider = model.provider.clone();
    let api_key = resolve_api_key(
        &provider,
        parsed.api_key.as_deref(),
        &config,
        &mut diagnostics,
    );
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
            theme_paths: {
                let mut paths = config.settings.themes.clone();
                paths.extend(parsed.theme_paths.iter().cloned());
                paths
            },
            theme: config.settings.theme.clone(),
        },
    )?;
    diagnostics.extend(
        loaded_resources
            .diagnostics
            .iter()
            .map(CliDiagnostic::from_resource),
    );

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
        diagnostics,
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
    let processed_prompt = input::process_at_file_references_with_processing_options(
        &merged_prompt,
        &context.cwd,
        input::ImageProcessingOptions::from_settings(&context.config.settings),
    )?;
    let invocation = resolve_invocation(&context, &processed_prompt);

    let session_options = PromptRunOptions {
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

fn resolve_api_key(
    provider: &str,
    cli_api_key: Option<&str>,
    config: &Config,
    diagnostics: &mut Vec<CliDiagnostic>,
) -> Option<String> {
    let mut key_diags = Vec::new();
    let resolved =
        config::auth::resolve_api_key(provider, cli_api_key, &config.auth, &mut key_diags);
    diagnostics.extend(key_diags.iter().map(CliDiagnostic::from_config));
    resolved.map(|resolved| resolved.value)
}

impl CliDiagnostic {
    pub fn from_config(diagnostic: &config::ConfigDiagnostic) -> Self {
        let severity = match diagnostic.severity {
            config::DiagnosticSeverity::Warn => CliDiagnosticSeverity::Warning,
            config::DiagnosticSeverity::Error => CliDiagnosticSeverity::Error,
        };
        Self {
            severity,
            message: diagnostic.message.clone(),
            source: diagnostic.source.clone(),
            code: Some("config".to_string()),
        }
    }

    pub fn from_resource(diagnostic: &ResourceDiagnostic) -> Self {
        let severity = match diagnostic.severity {
            ResourceDiagnosticSeverity::Info => CliDiagnosticSeverity::Info,
            ResourceDiagnosticSeverity::Warning => CliDiagnosticSeverity::Warning,
            ResourceDiagnosticSeverity::Error => CliDiagnosticSeverity::Error,
        };
        Self {
            severity,
            message: diagnostic.message.clone(),
            source: Some(diagnostic.path.clone()),
            code: Some(diagnostic.code.clone()),
        }
    }
}

pub fn render_diagnostics(diagnostics: &[CliDiagnostic]) -> String {
    let mut out = String::new();
    for diagnostic in diagnostics {
        let label = match diagnostic.severity {
            CliDiagnosticSeverity::Info => "info",
            CliDiagnosticSeverity::Warning => "warning",
            CliDiagnosticSeverity::Error => "error",
        };
        match diagnostic.code.as_deref() {
            Some("config") => match &diagnostic.source {
                Some(path) => out.push_str(&format!(
                    "config {label}: {} ({})\n",
                    diagnostic.message,
                    path.display()
                )),
                None => out.push_str(&format!("config {label}: {}\n", diagnostic.message)),
            },
            Some(code) => match &diagnostic.source {
                Some(path) => out.push_str(&format!(
                    "resource {}: {} (code: {})\n",
                    path.display(),
                    diagnostic.message,
                    code
                )),
                None => out.push_str(&format!(
                    "resource {label}: {} (code: {})\n",
                    diagnostic.message, code
                )),
            },
            None => match &diagnostic.source {
                Some(path) => out.push_str(&format!(
                    "{label}: {} ({})\n",
                    diagnostic.message,
                    path.display()
                )),
                None => out.push_str(&format!("{label}: {}\n", diagnostic.message)),
            },
        }
    }
    out
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
    let has_custom = parsed.system_prompt.is_some();
    let mut system_prompt = parsed.system_prompt.clone();
    if !context_files.is_empty() || !parsed.append_system_prompt.is_empty() {
        let mut parts = Vec::new();
        if let Some(base) = system_prompt.take() {
            parts.push(base);
        }
        // Wrap context files in <project_context> / <project_instructions>,
        // mirroring TS `buildSystemPrompt` in `system-prompt.ts`.
        if !context_files.is_empty() {
            let mut ctx_block = String::from(
                "<project_context>\n\nProject-specific instructions and guidelines:\n\n",
            );
            for file in context_files {
                ctx_block.push_str(&format!(
                    "<project_instructions path=\"{}\">\n{}\n</project_instructions>\n\n",
                    file.path.display(),
                    file.content
                ));
            }
            ctx_block.push_str("</project_context>");
            parts.push(ctx_block);
        }
        parts.extend(parsed.append_system_prompt.clone());
        system_prompt = Some(parts.join("\n\n"));
    }
    // Append cwd suffix, mirroring TS's date and working directory footer.
    if let Some(ref mut prompt) = system_prompt
        && has_custom
    {
        let display_cwd = cwd.display().to_string().replace('\\', "/");
        *prompt = format!("{prompt}\nCurrent working directory: {display_cwd}");
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
