use crate::CliError;
use pi_agent_core::resources::{
    load_prompt_templates as core_load_templates, load_skills as core_load_skills,
};
use pi_agent_core::types::DiagnosticSeverity;
use pi_agent_core::{AgentResources, PromptTemplate, ResourceDiagnostic, Skill};
use pi_tui::{Color, ThemePalette, TuiTheme, dark_theme, light_theme};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeResource {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}

pub fn tui_theme_from_resource(resource: &ThemeResource) -> TuiTheme {
    let value = serde_json::from_str::<serde_json::Value>(&resource.content).ok();
    let mode = value
        .as_ref()
        .and_then(|value| string_field(value, "mode"))
        .unwrap_or("dark");
    let base = if mode.eq_ignore_ascii_case("light") {
        light_theme().palette
    } else {
        dark_theme().palette
    };
    let palette = value
        .as_ref()
        .and_then(|value| value.get("palette"))
        .map(|palette| merge_palette(base, palette))
        .unwrap_or(base);
    TuiTheme::custom(resource.name.clone(), palette)
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

fn load_themes(paths: &[PathBuf]) -> (Vec<ThemeResource>, Vec<ResourceDiagnostic>) {
    let mut themes = Vec::new();
    let mut diagnostics = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "json") {
                load_theme_file(path, &mut themes, &mut diagnostics);
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
                    load_theme_file(&path, &mut themes, &mut diagnostics);
                }
            }
        }
    }
    (themes, diagnostics)
}

fn load_theme_file(
    path: &Path,
    themes: &mut Vec<ThemeResource>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
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

    let value = match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(value) => value,
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

    let name = value
        .get("name")
        .and_then(|name| name.as_str())
        .map(ToString::to_string)
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "unnamed".into());
    themes.push(ThemeResource {
        name,
        path: path.to_path_buf(),
        content,
    });
}

fn merge_palette(mut palette: ThemePalette, value: &serde_json::Value) -> ThemePalette {
    if let Some(color) = color_field(value, "accent") {
        palette.accent = color;
    }
    if let Some(color) = color_field(value, "muted") {
        palette.muted = color;
    }
    if let Some(color) = color_field(value, "text") {
        palette.text = color;
    }
    if let Some(color) = color_field(value, "background") {
        palette.background = color;
    }
    if let Some(color) = color_field(value, "error") {
        palette.error = color;
    }
    if let Some(color) = color_field(value, "success") {
        palette.success = color;
    }
    if let Some(color) = color_field(value, "warning") {
        palette.warning = color;
    }
    if let Some(color) = color_field(value, "path") {
        palette.path = color;
    }
    if let Some(color) =
        color_field(value, "input_border").or_else(|| color_field(value, "inputBorder"))
    {
        palette.input_border = color;
    }
    if let Some(color) =
        color_field(value, "menu_border").or_else(|| color_field(value, "menuBorder"))
    {
        palette.menu_border = color;
    }
    palette
}

fn color_field(value: &serde_json::Value, field: &str) -> Option<Color> {
    value.get(field).and_then(parse_color)
}

fn string_field<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(|value| value.as_str())
}

fn parse_color(value: &serde_json::Value) -> Option<Color> {
    if let Some(index) = value.as_u64() {
        return u8::try_from(index).ok().map(Color::Ansi256);
    }
    let text = value.as_str()?.trim();
    match text.to_ascii_lowercase().as_str() {
        "default" => Some(Color::Default),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "cyan" => Some(Color::Cyan),
        "magenta" | "purple" => Some(Color::Magenta),
        "white" => Some(Color::White),
        text if text.starts_with('#') => parse_hex_color(text),
        text if text.starts_with("ansi256:") => text
            .trim_start_matches("ansi256:")
            .parse::<u8>()
            .ok()
            .map(Color::Ansi256),
        text if text.starts_with("ansi:") => text
            .trim_start_matches("ansi:")
            .parse::<u8>()
            .ok()
            .map(Color::Ansi256),
        _ => None,
    }
}

fn parse_hex_color(text: &str) -> Option<Color> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(red, green, blue))
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
