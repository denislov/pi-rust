use crate::CliError;
use crate::theme::{ResolvedColor, ThemeColor, ThemeJson};
use pi_agent_core::resources::{
    load_prompt_templates as core_load_templates, load_skills as core_load_skills,
};
use pi_agent_core::types::DiagnosticSeverity;
use pi_agent_core::{AgentResources, PromptTemplate, ResourceDiagnostic, Skill};
use pi_tui::{Color, ThemePalette, TuiTheme, dark_theme};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceLoadOptions {
    pub no_skills: bool,
    pub no_prompt_templates: bool,
    pub no_themes: bool,
    pub skill_paths: Vec<String>,
    pub prompt_paths: Vec<String>,
    pub theme_paths: Vec<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedResources {
    pub skills: Vec<Skill>,
    pub prompt_templates: Vec<PromptTemplate>,
    pub themes: Vec<ThemeResource>,
    pub selected_theme: Option<ThemeResource>,
    pub diagnostics: Vec<ResourceDiagnostic>,
}

/// A discovered theme file, parsed into [`ThemeJson`]. The `theme` field holds
/// the structured 51-token model ported from TS `theme/theme.ts`; `name` is
/// the resolved theme name (JSON `name` or file stem), `path` the source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeResource {
    pub name: String,
    pub path: PathBuf,
    pub theme: ThemeJson,
}

/// Build a `pi_tui::TuiTheme` from a loaded theme resource.
///
/// This is a lossy bridge from the 51-token TS theme model to the current
/// `pi-tui` 10-color `ThemePalette` (the contract layer). It maps the most
/// relevant tokens: editor border <- `borderMuted`, menu/overlay border <-
/// `border`, accents/status from their namesake tokens. A theme with invalid
/// `vars` falls back to the dark palette, mirroring TS `setTheme` behavior.
pub fn tui_theme_from_resource(resource: &ThemeResource) -> TuiTheme {
    tui_theme_from_resolved_json(&resource.name, &resource.theme)
}

/// Build a `pi_tui::TuiTheme` from a theme name + parsed [`ThemeJson`].
/// Used by hot reload (which has no `ThemeResource`).
pub(crate) fn tui_theme_from_resolved_json(
    name: &str,
    theme: &crate::theme::ThemeJson,
) -> TuiTheme {
    let palette = match theme.resolve_colors() {
        Ok(resolved) => palette_from_resolved(&resolved),
        Err(_) => dark_theme().palette,
    };
    TuiTheme::custom(name.to_string(), palette)
}

fn palette_from_resolved(resolved: &crate::theme::ResolvedTheme) -> ThemePalette {
    ThemePalette {
        accent: to_color(resolved.fg(ThemeColor::Accent)),
        muted: to_color(resolved.fg(ThemeColor::Muted)),
        text: to_color(resolved.fg(ThemeColor::Text)),
        background: Color::Default,
        error: to_color(resolved.fg(ThemeColor::Error)),
        success: to_color(resolved.fg(ThemeColor::Success)),
        warning: to_color(resolved.fg(ThemeColor::Warning)),
        path: to_color(resolved.fg(ThemeColor::Accent)),
        input_border: to_color(resolved.fg(ThemeColor::BorderMuted)),
        menu_border: to_color(resolved.fg(ThemeColor::Border)),
    }
}

pub(crate) fn to_color(color: ResolvedColor) -> Color {
    match color {
        ResolvedColor::Default => Color::Default,
        ResolvedColor::Hex(r, g, b) => Color::Rgb(r, g, b),
        ResolvedColor::Ansi256(n) => Color::Ansi256(n),
    }
}

pub fn resolve_resource_paths(paths: &[String], cwd: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { cwd.join(&p) }
        })
        .collect()
}

pub fn discover_context_files(cwd: &Path, agent_dir: &Path, disabled: bool) -> Vec<ContextFile> {
    if disabled {
        return Vec::new();
    }

    let mut files = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    if let Some(file) = load_context_file_from_dir(agent_dir)
        && seen.insert(file.path.clone())
    {
        files.push(file);
    }

    let mut ancestors = Vec::new();
    let mut current = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    loop {
        if let Some(file) = load_context_file_from_dir(&current)
            && seen.insert(file.path.clone())
        {
            ancestors.push(file);
        }
        if !current.pop() {
            break;
        }
    }
    ancestors.reverse();
    files.extend(ancestors);
    files
}

fn load_context_file_from_dir(dir: &Path) -> Option<ContextFile> {
    for name in ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"] {
        let path = dir.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Some(ContextFile { path, content });
            }
        }
    }
    None
}

pub fn load_cli_resources(
    skills_dirs: &[String],
    template_paths: &[String],
    cwd: &Path,
) -> Result<(Vec<Skill>, Vec<PromptTemplate>, Vec<ResourceDiagnostic>), CliError> {
    let resolved_skills = resolve_resource_paths(skills_dirs, cwd);
    let resolved_templates = resolve_resource_paths(template_paths, cwd);

    let (skills, skill_diags) = core_load_skills(&resolved_skills);
    let (templates, template_diags) = core_load_templates(&resolved_templates);

    let mut all_diags = skill_diags;
    all_diags.extend(template_diags);

    Ok((skills, templates, all_diags))
}

pub fn load_cli_resources_with_options(
    skills_dirs: &[String],
    template_paths: &[String],
    cwd: &Path,
    agent_dir: &Path,
    options: ResourceLoadOptions,
) -> Result<LoadedResources, CliError> {
    let mut resolved_skills = Vec::new();
    if !options.no_skills {
        resolved_skills.push(agent_dir.join("skills"));
        resolved_skills.push(cwd.join(".pi-rust").join("skills"));
        resolved_skills.extend(resolve_resource_paths(&options.skill_paths, cwd));
        resolved_skills.extend(resolve_resource_paths(skills_dirs, cwd));
    }

    let mut resolved_templates = Vec::new();
    if !options.no_prompt_templates {
        resolved_templates.push(agent_dir.join("prompts"));
        resolved_templates.push(agent_dir.join("prompt-templates"));
        resolved_templates.push(cwd.join(".pi-rust").join("prompts"));
        resolved_templates.push(cwd.join(".pi-rust").join("prompt-templates"));
        resolved_templates.extend(resolve_resource_paths(&options.prompt_paths, cwd));
        resolved_templates.extend(resolve_resource_paths(template_paths, cwd));
    }

    let resolved_themes = if options.no_themes {
        Vec::new()
    } else {
        let mut paths = vec![
            agent_dir.join("themes"),
            cwd.join(".pi-rust").join("themes"),
        ];
        paths.extend(resolve_resource_paths(&options.theme_paths, cwd));
        paths
    };

    let (skills, skill_diags) = if resolved_skills.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        core_load_skills(&resolved_skills)
    };
    let (prompt_templates, template_diags) = if resolved_templates.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        core_load_templates(&resolved_templates)
    };

    let mut diagnostics = skill_diags;
    diagnostics.extend(template_diags);
    let (themes, theme_diags) = load_themes(&resolved_themes);
    diagnostics.extend(theme_diags);
    let selected_theme = options
        .theme
        .as_deref()
        .and_then(|name| themes.iter().find(|theme| theme.name == name).cloned());
    Ok(LoadedResources {
        skills,
        prompt_templates,
        themes,
        selected_theme,
        diagnostics,
    })
}

/// Load themes from the given paths, deduplicating by name (first wins, like
/// TS `getAvailableThemesWithPaths`) and sorting by name. Invalid files and
/// themes with missing required tokens produce diagnostics.
fn load_themes(paths: &[PathBuf]) -> (Vec<ThemeResource>, Vec<ResourceDiagnostic>) {
    let mut themes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "json") {
                load_theme_file(path, &mut themes, &mut diagnostics, &mut seen);
            }
        } else if path.is_dir() {
            let Ok(entries) = std::fs::read_dir(path) else {
                diagnostics.push(ResourceDiagnostic {
                    severity: DiagnosticSeverity::Warning,
                    code: "theme_read_dir_error".into(),
                    message: format!("failed to read theme directory {}", path.display()),
                    path: path.clone(),
                });
                continue;
            };
            let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    load_theme_file(&path, &mut themes, &mut diagnostics, &mut seen);
                }
            }
        }
    }
    // TS `getAvailableThemesWithPaths` sorts available themes by name.
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    (themes, diagnostics)
}

fn load_theme_file(
    path: &Path,
    themes: &mut Vec<ThemeResource>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    seen: &mut std::collections::BTreeSet<String>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "theme_read_error".into(),
                message: format!("failed to read {}: {}", path.display(), error),
                path: path.to_path_buf(),
            });
            return;
        }
    };

    let theme = match serde_json::from_str::<ThemeJson>(&content) {
        Ok(theme) => theme,
        Err(error) => {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "theme_parse_error".into(),
                message: format!("failed to parse {}: {}", path.display(), error),
                path: path.to_path_buf(),
            });
            return;
        }
    };

    let name = if theme.name.is_empty() {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".into())
    } else {
        theme.name.clone()
    };
    if !seen.insert(name.clone()) {
        return; // duplicate theme name; first one wins (TS dedupe)
    }

    // Report missing required color tokens (TS `parseThemeJson` lists these).
    let missing = theme.missing_tokens();
    if !missing.is_empty() {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "theme_missing_tokens".into(),
            message: format!(
                "missing {} color tokens: {}",
                missing.len(),
                missing.join(", ")
            ),
            path: path.to_path_buf(),
        });
    }

    themes.push(ThemeResource {
        name,
        path: path.to_path_buf(),
        theme,
    });
}

pub fn find_skill<'a>(skills: &'a [Skill], name: &str) -> Option<&'a Skill> {
    skills.iter().find(|s| s.name == name)
}

pub fn find_template<'a>(
    templates: &'a [PromptTemplate],
    name: &str,
) -> Option<&'a PromptTemplate> {
    templates.iter().find(|t| t.name == name)
}

pub fn print_diagnostics(diags: &[ResourceDiagnostic]) {
    for d in diags {
        eprintln!(
            "resource {}: {} (code: {})",
            d.path.display(),
            d.message,
            d.code
        );
    }
}

pub fn build_agent_resources(skills: Vec<Skill>, templates: Vec<PromptTemplate>) -> AgentResources {
    AgentResources {
        skills,
        prompt_templates: templates,
    }
}
