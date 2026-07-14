use pi_agent_core::Skill;
use pi_agent_core::resources::{
    format_prompt_template_invocation, format_skill_invocation, format_skills_for_system_prompt,
    load_prompt_templates, load_skills, parse_frontmatter,
};
use std::io::Write;
use tempfile::TempDir;

#[test]
fn frontmatter_parses_name_and_description() {
    let input = "---\nname: rust\ndescription: Rust programming\ndisable-model-invocation: true\n---\n\n# Rust Skill\n\nSome content.";
    let (meta, body, diags) = parse_frontmatter(input);
    assert!(diags.is_empty());
    assert_eq!(meta["name"].as_str().unwrap(), "rust");
    assert_eq!(meta["description"].as_str().unwrap(), "Rust programming");
    assert!(meta["disable-model-invocation"].as_bool().unwrap());
    assert!(body.contains("# Rust Skill"));
}

#[test]
fn invalid_yaml_returns_warning_diagnostic() {
    let input = "---\ninvalid: [bad yaml\n---\nbody";
    let (_, _, diags) = parse_frontmatter(input);
    assert!(!diags.is_empty());
    assert!(diags[0].code.contains("parse_error"));
}

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
fn formats_skills_for_system_prompt() {
    let skills = vec![
        Skill {
            name: "rust".into(),
            description: "Rust programming".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: String::new(),
            disable_model_invocation: false,
        },
        Skill {
            name: "hidden".into(),
            description: "hidden".into(),
            location: "/h".into(),
            content: String::new(),
            disable_model_invocation: true,
        },
    ];
    let formatted = format_skills_for_system_prompt(&skills);
    assert!(formatted.contains("<available_skills>"));
    assert!(formatted.contains("<name>rust</name>"));
    assert!(!formatted.contains("hidden"));
    assert!(formatted.contains("</available_skills>"));
}

#[test]
fn format_skill_invocation_includes_skill_content() {
    let result = format_skill_invocation(
        "rust",
        "/skills/rust/SKILL.md",
        "Rust programming guide",
        Some("use best practices"),
    );
    assert!(result.contains("rust"));
    assert!(result.contains("/skills/rust/SKILL.md"));
    assert!(result.contains("Rust programming guide"));
    assert!(result.contains("use best practices"));
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
fn format_prompt_template_replaces_args() {
    let result = format_prompt_template_invocation(
        "review",
        "Review $1 and $2",
        &["foo".into(), "bar".into()],
    );
    assert_eq!(result, "Review foo and bar");
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
