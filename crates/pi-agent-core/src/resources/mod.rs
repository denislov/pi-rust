mod types;

pub use types::{
    AgentResources, DiagnosticSeverity, PromptTemplate, ResourceDiagnostic, Skill, SourceTag,
    SourcedPromptTemplate, SourcedResourceDiagnostic, SourcedSkill,
};

pub mod frontmatter;
pub mod prompt_templates;
pub mod skills;
pub mod system_prompt;

pub use frontmatter::parse_frontmatter;
pub use prompt_templates::{load_prompt_templates, load_sourced_prompt_templates};
pub use skills::{load_skills, load_sourced_skills};
pub use system_prompt::{
    format_prompt_template_invocation, format_skill_invocation, format_skills_for_system_prompt,
    parse_command_args, substitute_args,
};

pub(crate) fn read_resource_file(
    path: &std::path::Path,
    error_code: &str,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(e) => {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: error_code.into(),
                message: format!("failed to read {}: {}", path.display(), e),
                path: path.to_path_buf(),
            });
            None
        }
    }
}

pub(crate) fn parse_frontmatter_at_path(
    content: &str,
    path: &std::path::Path,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) -> (serde_yaml::Value, String) {
    let (meta, body, mut meta_diags) = parse_frontmatter(content);
    for d in &mut meta_diags {
        d.path = path.to_path_buf();
    }
    diagnostics.append(&mut meta_diags);
    (meta, body)
}
