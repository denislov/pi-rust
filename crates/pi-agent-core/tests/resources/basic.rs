//! Basic skill/template parsing and loading behavior.

use pi_agent_core::api::resources::{load_prompt_templates, load_skills};
use std::io::Write;
use tempfile::TempDir;

#[test]
fn load_skills_from_directory() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("rust");
    std::fs::create_dir(&skill_dir).unwrap();

    let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
    writeln!(
        f,
        "---\nname: rust\ndescription: Rust programming\n---\n\nRust programming guide content."
    )
    .unwrap();

    let (skills, diags) = load_skills(&[skill_dir]);
    assert!(diags.is_empty(), "diags: {:?}", diags);
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "rust");
    assert_eq!(skills[0].description, "Rust programming");
    assert!(!skills[0].disable_model_invocation);
}

#[test]
fn ignored_directories_skipped() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("visible");
    std::fs::create_dir(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: visible\ndescription: A visible skill\n---\n\ncontent",
    )
    .unwrap();

    let hidden_dir = dir.path().join("hidden");
    std::fs::create_dir(&hidden_dir).unwrap();
    std::fs::write(dir.path().join(".gitignore"), "hidden/\n").unwrap();

    let (skills, _) = load_skills(&[dir.path().to_path_buf()]);
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "visible");
}

#[test]
fn load_prompt_templates_from_files() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("review.md"),
        "---\ndescription: Review changes\n---\n\nPlease review $1 and $2.",
    )
    .unwrap();

    let (templates, diags) = load_prompt_templates(&[dir.path().join("review.md")]);
    assert!(diags.is_empty());
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].name, "review");
    assert_eq!(templates[0].description, "Review changes");
}

#[test]
fn loads_from_directory_sorted() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("b.md"), "content b").unwrap();
    std::fs::write(dir.path().join("a.md"), "content a").unwrap();
    std::fs::write(dir.path().join("c.txt"), "not md").unwrap();

    let (templates, _) = load_prompt_templates(&[dir.path().to_path_buf()]);
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].name, "a");
    assert_eq!(templates[1].name, "b");
}
