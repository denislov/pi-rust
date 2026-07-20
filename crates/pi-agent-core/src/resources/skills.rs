use crate::agent::types::{
    DiagnosticSeverity, ResourceDiagnostic, Skill, SourceTag, SourcedResourceDiagnostic,
    SourcedSkill,
};
use crate::resources::{parse_frontmatter_at_path, read_resource_file};
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
            && ext == "md"
        {
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
        && let Some(skill) = load_skill_file(&skill_md, diagnostics, false)
    {
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
            && let Some(skill) = load_skill_file(&path, diagnostics, false)
        {
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
                && let Some(skill) = load_skill_file(&path, diagnostics, false)
            {
                skills.push(skill);
            }
        }
    }
}

fn load_skill_file(
    path: &Path,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    _is_template: bool,
) -> Option<Skill> {
    let content = read_resource_file(path, "skill_read_error", diagnostics)?;

    let (meta, body) = parse_frontmatter_at_path(&content, path, diagnostics);

    let parent_dir_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());

    let frontmatter_name = meta.get("name").and_then(|v| v.as_str()).map(str::to_owned);
    let name = frontmatter_name
        .clone()
        .map(|value| value.chars().take(64).collect())
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
            path: path.to_path_buf(),
        });
    }
    for error in validate_skill_name(frontmatter_name.as_deref().unwrap_or(&name)) {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: error,
            path: path.to_path_buf(),
        });
    }

    let description_raw = meta
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let description = description_raw
        .clone()
        .map(|value| value.chars().take(1024).collect::<String>());

    // Reject skills with empty description (TS behavior).
    if description.as_deref().is_none_or(|d| d.trim().is_empty()) {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: "description is required".into(),
            path: path.to_path_buf(),
        });
        return None;
    }

    if let Some(ref desc) = description_raw
        && desc.chars().count() > 1024
    {
        diagnostics.push(ResourceDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: "invalid_metadata".into(),
            message: format!(
                "description exceeds {} characters ({})",
                1024,
                desc.chars().count()
            ),
            path: path.to_path_buf(),
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
    let character_count = name.chars().count();
    if character_count > 64 {
        errors.push(format!("name exceeds 64 characters ({character_count})"));
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
        && let Some(name) = parent.file_name()
    {
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

    #[test]
    fn validates_unicode_name_and_description_lengths_by_characters() {
        let dir = TempDir::new().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        let long_name = "a".repeat(65);
        let long_description = "界".repeat(1025);
        std::fs::write(
            &skill_md,
            format!("---\nname: {long_name}\ndescription: {long_description}\n---\n\ncontent"),
        )
        .unwrap();
        let (skills, diags) = load_skills(&[dir.path().to_path_buf()]);
        assert_eq!(skills.len(), 1);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("name exceeds 64 characters (65)"))
        );
        assert!(diags.iter().any(|d| {
            d.message
                .contains("description exceeds 1024 characters (1025)")
        }));
        assert_eq!(skills[0].description.chars().count(), 1024);
    }
}
