use crate::resources::frontmatter::parse_frontmatter;
use crate::types::{
    DiagnosticSeverity, PromptTemplate, ResourceDiagnostic, SourceTag, SourcedPromptTemplate,
    SourcedResourceDiagnostic,
};
use std::path::PathBuf;

pub fn load_prompt_templates(paths: &[PathBuf]) -> (Vec<PromptTemplate>, Vec<ResourceDiagnostic>) {
    let mut templates = Vec::new();
    let mut diagnostics = Vec::new();

    for path in paths {
        if !path.exists() {
            continue;
        }

        if path.is_file() {
            if path.extension().is_some_and(|e| e == "md")
                && let Some(t) = load_template_file(path, &mut diagnostics) {
                    templates.push(t);
                }
        } else if path.is_dir()
            && let Ok(entries) = std::fs::read_dir(path) {
                let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                files.sort_by_key(|e| e.file_name());
                for entry in files {
                    let p = entry.path();
                    if p.extension().is_some_and(|e| e == "md")
                        && let Some(t) = load_template_file(&p, &mut diagnostics) {
                            templates.push(t);
                        }
                }
            }
    }

    // Deduplicate by name (first wins, TS behavior). Duplicates produce
    // collision diagnostics so users know a later template shadowed an
    // earlier one.
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut deduped: Vec<PromptTemplate> = Vec::new();
    for template in templates {
        if let Some(&existing_idx) = seen.get(&template.name) {
            let existing_loc = deduped[existing_idx].location.clone();
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "prompt_collision".into(),
                message: format!(
                    "name \"/{}\" collision (using {}, ignoring {})",
                    template.name, existing_loc, template.location
                ),
                path: PathBuf::from(&template.location),
            });
        } else {
            let idx = deduped.len();
            seen.insert(template.name.clone(), idx);
            deduped.push(template);
        }
    }

    (deduped, diagnostics)
}

/// Load prompt templates from sourced inputs. Mirrors
/// [`crate::resources::skills::load_sourced_skills`].
pub fn load_sourced_prompt_templates(
    inputs: &[(PathBuf, SourceTag)],
) -> (Vec<SourcedPromptTemplate>, Vec<SourcedResourceDiagnostic>) {
    let mut sourced_templates = Vec::new();
    let mut sourced_diagnostics = Vec::new();
    for (path, source) in inputs {
        let (templates, diagnostics) = load_prompt_templates(std::slice::from_ref(path));
        for template in templates {
            sourced_templates.push(SourcedPromptTemplate {
                template,
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
    (sourced_templates, sourced_diagnostics)
}

fn load_template_file(
    path: &PathBuf,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) -> Option<PromptTemplate> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            diagnostics.push(ResourceDiagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "template_read_error".into(),
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
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        });

    let description = meta
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| {
            let capped: String = s.chars().take(60).collect();
            if s.len() > 60 {
                format!("{}...", capped)
            } else {
                capped
            }
        })
        .unwrap_or_else(|| {
            let first_line = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            let capped: String = first_line.chars().take(60).collect();
            if first_line.len() > 60 {
                format!("{}...", capped)
            } else {
                capped
            }
        });

    Some(PromptTemplate {
        name,
        description,
        content: body,
        location: path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn loads_prompt_template_from_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("review.md");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(
            f,
            "---\ndescription: Review changes\n---\n\nPlease review the following changes: $1"
        )
        .unwrap();

        let (templates, diags) = load_prompt_templates(&[file]);
        assert!(diags.is_empty());
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "review");
        assert_eq!(templates[0].description, "Review changes");
    }

    #[test]
    fn loads_from_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.md"), "content a").unwrap();
        std::fs::write(dir.path().join("b.md"), "content b").unwrap();
        std::fs::write(dir.path().join("c.txt"), "not md").unwrap();

        let (templates, _) = load_prompt_templates(&[dir.path().to_path_buf()]);
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "a");
        assert_eq!(templates[1].name, "b");
    }

    #[test]
    fn deduplicates_by_name_first_wins() {
        let dir = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        std::fs::write(dir.path().join("greet.md"), "hello world").unwrap();
        std::fs::write(dir2.path().join("greet.md"), "hello again").unwrap();

        let (templates, diags) =
            load_prompt_templates(&[dir.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert_eq!(templates.len(), 1, "duplicate name should be removed");
        assert_eq!(templates[0].content, "hello world", "first wins");
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("collision")));
    }
}
