use crate::resources::frontmatter::parse_frontmatter;
use crate::types::{
    DiagnosticSeverity, ResourceDiagnostic, Skill, SourceTag, SourcedResourceDiagnostic,
    SourcedSkill,
};
use ignore::WalkBuilder;
use std::path::PathBuf;

pub fn load_skills(paths: &[PathBuf]) -> (Vec<Skill>, Vec<ResourceDiagnostic>) {
    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();

    for root in paths {
        if !root.exists() {
            continue;
        }

        if root.is_dir() {
            load_skills_from_dir(root, &mut skills, &mut diagnostics);
        } else if root.is_file() {
            if let Some(ext) = root.extension() {
                if ext == "md" {
                    let path = root.clone();
                    if let Some(skill) = load_skill_file(&path, &mut diagnostics, false) {
                        skills.push(skill);
                    }
                }
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
    if skill_md.exists() {
        if let Some(skill) = load_skill_file(&skill_md, diagnostics, false) {
            skills.push(skill);
        }
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
        if entry.file_name() == "SKILL.md" {
            if let Some(skill) = load_skill_file(&path, diagnostics, false) {
                skills.push(skill);
            }
        }
    }

    // Load direct .md files in root
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path == skill_md {
                continue;
            }
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    if let Some(skill) = load_skill_file(&path, diagnostics, false) {
                        skills.push(skill);
                    }
                }
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

    let name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| {
            let capped: String = s.chars().take(64).collect();
            capped
        })
        .unwrap_or_else(|| fallback_name(path));

    let description = meta
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| {
            let capped: String = s.chars().take(1024).collect();
            capped
        })
        .unwrap_or_else(|| fallback_description(&body));

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

fn fallback_name(path: &PathBuf) -> String {
    if let Some(stem) = path.file_stem() {
        let s = stem.to_string_lossy();
        let capped: String = s.chars().take(64).collect();
        return capped;
    }
    if let Some(parent) = path.parent() {
        if let Some(name) = parent.file_name() {
            let s = name.to_string_lossy();
            let capped: String = s.chars().take(64).collect();
            return capped;
        }
    }
    "unnamed".to_string()
}

fn fallback_description(body: &str) -> String {
    let first_line = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let capped: String = first_line.chars().take(1024).collect();
    capped
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
        std::fs::write(&skill_md, "---\nname: visible\n---\n\ncontent").unwrap();

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
}
