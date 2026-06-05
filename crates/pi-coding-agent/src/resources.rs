use crate::CliError;
use pi_agent_core::resources::{
    load_prompt_templates as core_load_templates, load_skills as core_load_skills,
};
use pi_agent_core::{AgentResources, PromptTemplate, ResourceDiagnostic, Skill};
use std::path::{Path, PathBuf};

pub fn resolve_resource_paths(paths: &[String], cwd: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { cwd.join(&p) }
        })
        .collect()
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
