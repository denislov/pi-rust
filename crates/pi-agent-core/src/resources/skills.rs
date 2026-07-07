use crate::resources::frontmatter::parse_frontmatter;
use crate::types::{
    DiagnosticSeverity, ResourceDiagnostic, Skill, SourceTag, SourcedResourceDiagnostic,
    SourcedSkill,
};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub fn load_skills(paths: &[PathBuf]) -> (Vec<Skill>, Vec<ResourceDiagnostic>) {
    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();

    for root in paths {
        if !root.exists() {
            continue;
        }

        if root.is_dir() {
            load_skills_from_dir(root, &mut skills, &mut diagnostics);
        } else if root.is_file()
            && let Some(ext) = root.extension()
                && ext == "md" {
                    let path = root.clone();
                    if let Some(skill) = load_skill_file(&path, &mut diagnostics, false) {
                        skills.push(skill);
                    }
                }
    }

    (skills, diagnostics)
}

/// Load skills from sourced inputs. Each entry's input path is loaded with
/// [`load_skills`]; every resulting skill and diagnostic is tagged with the
/// associated [`SourceTag`]. Mirrors TS `loadSourcedSkills`
/// (`pi/packages/agent/src/harness/skills.ts:83`).
pub fn load_sourced_skills(
    inputs: &[(PathBuf, SourceTag)],
) -> (Vec<SourcedSkill>, Vec<SourcedResourceDiagnostic>) {
    let mut sourced_skills = Vec::new();
    let mut sourced_diagnostics = Vec::new();
    for (path, source) in inputs {
        let (skills, diagnostics) = load_skills(std::slice::from_ref(path));
        for skill in skills {
            sourced_skills.push(SourcedSkill {
                skill,
                source: source.clone(),
            });
        }
        for diagnostic in diagnostics {
            sourced_diagnostics.push(SourcedResourceDiagnostic {
                diagnostic,
                source: source.clone(),
            });
        }
    }
    (sourced_skills, sourced_diagnostics)
}

fn load_skills_from_dir(
    root: &PathBuf,
    skills: &mut Vec<Skill>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    // Load SKILL.md at the directory level
    let skill_md = root.join("SKILL.md");
    if skill_md.exists()
        && let Some(skill) = load_skill_file(&skill_md, diagnostics, false) {
            skills.push(skill);
        }

    // Walk subdirectories for additional SKILL.md files
    let walker = WalkBuilder::new(root)
        .git_ignore(true)
        .hidden(false)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        if path == *root || path == skill_md {
            continue;
        }
        if entry.file_name() == "SKILL.md"
            && let Some(skill) = load_skill_file(&path, diagnostics, false) {
                skills.push(skill);
            }
    }

    // Load direct .md files in root
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path == skill_md {
                continue;
            }
            if let Some(ext) = path.extension()
                && ext == "md"
                    && let Some(skill) = load_skill_file(&path, diagnostics, false) {
                        skills.push(skill);
                    }
        }
    }
}

fn load_skill_file(
    path: &PathBuf,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    _is_template: bool,
) -> Option<Skill> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "skill_read_error".into(),
                message: format!("failed to read {}: {}", path.display(), e),
                path: path.clone(),
            });
            return None;
        }
    };

    let (meta, body, mut meta_diags) = parse_frontmatter(&content);
    for d in &mut meta_diags {
        d.path = path.clone();
    }
    diagnostics.append(&mut meta_diags);

    let parent_dir_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());

    let frontmatter_name = meta.get("name").and_then(|v| v.as_str()).map(|s| {
        let capped: String = s.chars().take(64).collect();
        capped
    });
    let name = frontmatter_name
        .clone()
        .unwrap_or_else(|| fallback_name(path));

    // Validate name against TS `validateName` rules.
    if let Some(ref parent_name) = parent_dir_name
        && let Some(ref fm_name) = frontmatter_name
            && fm_name.as_str() != parent_name.as_str()
        {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "invalid_metadata".into(),
                message: format!(
                    "name \"{fm_name}\" does not match parent directory \"{parent_name}\""
                ),
                path: path.clone(),
            });
        }
    for error in validate_skill_name(&name) {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: error,
            path: path.clone(),
        });
    }

    let description = meta.get("description").and_then(|v| v.as_str()).map(|s| {
        let capped: String = s.chars().take(1024).collect();
        capped
    });

    // Reject skills with empty description (TS behavior).
    if description.as_deref().is_none_or(|d| d.trim().is_empty()) {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: "description is required".into(),
            path: path.clone(),
        });
        return None;
    }

    if let Some(ref desc) = description
        && desc.len() > 1024
    {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: format!("description exceeds {} characters ({})", 1024, desc.len()),
            path: path.clone(),
        });
    }
    let description = description.unwrap(); // safe: we returned None above if None/empty

    let disable_model_invocation = meta
        .get("disable-model-invocation")
        .or_else(|| meta.get("disableModelInvocation"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let location = path.to_string_lossy().to_string();

    Some(Skill {
        name,
        description,
        location,
        content: body,
        disable_model_invocation,
    })
}

/// Validate a skill name against TS `validateName` rules:
/// - only lowercase a-z, 0-9, hyphens
/// - no leading or trailing hyphens
/// - no consecutive hyphens
/// - max 64 characters
fn validate_skill_name(name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if name.len() > 64 {
        errors.push(format!("name exceeds 64 characters ({})", name.len()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        errors.push(
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)".into(),
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        errors.push("name must not start or end with a hyphen".into());
    }
    if name.contains("--") {
        errors.push("name must not contain consecutive hyphens".into());
    }
    errors
}

fn fallback_name(path: &Path) -> String {
    if let Some(stem) = path.file_stem() {
        let s = stem.to_string_lossy();
        let capped: String = s.chars().take(64).collect();
        return capped;
    }
    if let Some(parent) = path.parent()
        && let Some(name) = parent.file_name() {
            let s = name.to_string_lossy();
            let capped: String = s.chars().take(64).collect();
            return capped;
        }
    "unnamed".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn loads_skill_md_from_directory() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("rust");
        std::fs::create_dir(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        let mut f = std::fs::File::create(&skill_md).unwrap();
        writeln!(
            f,
            "---\nname: rust\ndescription: Rust programming\n---\n\nRust skill content."
        )
        .unwrap();

        let (skills, diags) = load_skills(&[skill_dir]);
        assert!(diags.is_empty());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "rust");
        assert_eq!(skills[0].description, "Rust programming");
        assert!(skills[0].content.contains("Rust skill content"));
        assert!(!skills[0].disable_model_invocation);
    }

    #[test]
    fn skips_ignored_directories() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("visible");
        std::fs::create_dir(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: visible\ndescription: A visible skill\n---\n\ncontent",
        )
        .unwrap();

        let hidden_dir = dir.path().join("hidden");
        std::fs::create_dir(&hidden_dir).unwrap();
        let gitignore = dir.path().join(".gitignore");
        std::fs::write(&gitignore, "hidden/").unwrap();

        let (skills, diags) = load_skills(&[dir.path().to_path_buf()]);
        let _ = diags;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "visible");
    }

    #[test]
    fn missing_root_is_skipped() {
        let (skills, diags) = load_skills(&[PathBuf::from("/nonexistent/path/12345")]);
        assert!(diags.is_empty());
        assert!(skills.is_empty());
    }

    #[test]
    fn rejects_skill_with_empty_description() {
        let dir = TempDir::new().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: noskill\n---\n\ncontent").unwrap();
        let (skills, diags) = load_skills(&[dir.path().to_path_buf()]);
        assert!(
            skills.is_empty(),
            "skill with no description should be rejected"
        );
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("description")));
    }

    #[test]
    fn validates_skill_name_rules() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("bad-name");
        std::fs::create_dir(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: BAD_NAME\ndescription: test\n---\n\ncontent",
        )
        .unwrap();
        let (skills, diags) = load_skills(&[dir.path().to_path_buf()]);
        // Name is invalid but skill is still loaded (TS emits warning, not rejection)
        assert_eq!(skills.len(), 1);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("invalid characters"))
        );
    }
}
