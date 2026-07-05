use std::fs;

use pi_coding_agent::api::{
    DelegationConfirmationMode, ProfileRegistry, ProfileRegistryOptions, ProfileSource,
    SupervisionPolicy, TeamStrategy, TeamSupervisor,
};
use tempfile::tempdir;

#[test]
fn built_in_default_agent_profile_exposes_read_only_helper_roster() {
    let registry = ProfileRegistry::load(ProfileRegistryOptions::new()).unwrap();

    let profile = registry
        .agent("default")
        .expect("built-in default profile should resolve");

    assert_eq!(profile.id.as_str(), "default");
    assert_eq!(profile.display_name, "Default");
    assert_eq!(profile.source, ProfileSource::BuiltIn);
    assert_eq!(profile.supervision, SupervisionPolicy::Session);
    assert!(profile.delegation.allow_delegate_agent);
    assert!(!profile.delegation.allow_delegate_team);
    assert_eq!(profile.delegation.max_depth, 1);
    assert_eq!(profile.delegation.max_parallel_children, 1);
    assert_eq!(
        profile.delegation.require_confirmation,
        DelegationConfirmationMode::Never
    );
    assert_eq!(
        profile
            .delegation
            .allowed_agents
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        vec!["explore", "review", "check"]
    );

    for helper_id in ["explore", "review", "check"] {
        let helper = registry
            .agent(helper_id)
            .expect("built-in helper profile should resolve");
        assert_eq!(helper.source, ProfileSource::BuiltIn);
        assert!(
            helper.tools.is_empty(),
            "built-in helper {helper_id} must not carry write tools by default"
        );
        assert!(
            helper.skills.is_empty(),
            "built-in helper {helper_id} must not carry privileged skills by default"
        );
        assert!(!helper.delegation.allow_delegate_agent);
        assert!(!helper.delegation.allow_delegate_team);
    }
}

#[test]
fn custom_default_profile_does_not_inherit_built_in_helper_roster() {
    let root = tempdir().unwrap();
    write_file(
        root.path().join("agents/default.toml"),
        r#"
schema_version = 1
id = "default"
display_name = "Project Default"
"#,
    );

    let registry =
        ProfileRegistry::load(ProfileRegistryOptions::new().with_project_root(root.path()))
            .unwrap();
    let profile = registry.agent("default").unwrap();

    assert_eq!(profile.display_name, "Project Default");
    assert_eq!(profile.source, ProfileSource::Project);
    assert!(!profile.delegation.allow_delegate_agent);
    assert!(profile.delegation.allowed_agents.is_empty());
}

#[test]
fn loads_agent_and_team_profiles_from_toml_roots() {
    let root = tempdir().unwrap();
    write_file(
        root.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
description = "Implementation agent"
model = "gpt-5-codex"
system_prompt = "You write code."
tools = ["shell", "apply_patch"]
skills = ["superpowers:test-driven-development"]
supervision = "self_review"

[delegation]
allow_delegate_agent = true
allow_delegate_team = false
max_depth = 1
require_confirmation = "writes"
allowed_agents = ["reviewer"]
"#,
    );
    write_file(
        root.path().join("teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
description = "Planner, coder, reviewer"
supervisor = "planner"
strategy = "plan_execute_review"
members = ["planner", "coder", "reviewer"]

[delegation]
max_parallel_children = 2
max_depth = 1
require_confirmation = "writes"
"#,
    );

    let registry =
        ProfileRegistry::load(ProfileRegistryOptions::new().with_project_root(root.path()))
            .unwrap();

    let coder = registry.agent("coder").expect("agent should load");
    assert_eq!(coder.display_name, "Coder");
    assert_eq!(coder.description.as_deref(), Some("Implementation agent"));
    assert_eq!(coder.model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(coder.system_prompt.as_deref(), Some("You write code."));
    assert_eq!(coder.tools, ["shell", "apply_patch"]);
    assert_eq!(
        coder.skills,
        ["superpowers:test-driven-development".to_string()]
    );
    assert_eq!(coder.supervision, SupervisionPolicy::SelfReview);
    assert!(coder.delegation.allow_delegate_agent);
    assert!(!coder.delegation.allow_delegate_team);
    assert_eq!(coder.delegation.max_depth, 1);
    assert_eq!(coder.delegation.allowed_agents[0].as_str(), "reviewer");
    assert_eq!(coder.source, ProfileSource::Project);

    let team = registry.team("implementation").expect("team should load");
    assert_eq!(team.display_name, "Implementation Team");
    assert_eq!(team.supervisor, TeamSupervisor::Agent("planner".into()));
    assert_eq!(team.strategy, TeamStrategy::PlanExecuteReview);
    assert_eq!(team.members.len(), 3);
    assert_eq!(team.members[1].as_str(), "coder");
    assert_eq!(team.delegation.max_parallel_children, 2);
    assert_eq!(team.source, ProfileSource::Project);
    assert!(
        registry.diagnostics().is_empty(),
        "unexpected diagnostics: {registry:#?}"
    );
}

#[test]
fn invalid_profile_files_are_diagnostics_without_blocking_valid_profiles() {
    let root = tempdir().unwrap();
    write_file(
        root.path().join("agents/valid.toml"),
        r#"
schema_version = 1
id = "valid"
display_name = "Valid"
"#,
    );
    write_file(
        root.path().join("agents/invalid.toml"),
        r#"
schema_version = 99
id = "invalid"
display_name = "Invalid"
"#,
    );

    let registry =
        ProfileRegistry::load(ProfileRegistryOptions::new().with_project_root(root.path()))
            .unwrap();

    assert!(registry.agent("valid").is_some());
    assert!(registry.agent("invalid").is_none());
    assert!(
        registry.diagnostics().iter().any(|diagnostic| diagnostic
            .message
            .contains("unsupported agent profile schema_version 99")),
        "expected unsupported schema diagnostic, got {:#?}",
        registry.diagnostics()
    );
}

#[test]
fn duplicate_ids_use_project_over_user_over_builtin_with_diagnostics() {
    let user_root = tempdir().unwrap();
    let project_root = tempdir().unwrap();
    write_file(
        user_root.path().join("agents/default.toml"),
        r#"
schema_version = 1
id = "default"
display_name = "User Default"
"#,
    );
    write_file(
        project_root.path().join("agents/default.toml"),
        r#"
schema_version = 1
id = "default"
display_name = "Project Default"
"#,
    );

    let registry = ProfileRegistry::load(
        ProfileRegistryOptions::new()
            .with_user_root(user_root.path())
            .with_project_root(project_root.path()),
    )
    .unwrap();

    let profile = registry.agent("default").unwrap();
    assert_eq!(profile.display_name, "Project Default");
    assert_eq!(profile.source, ProfileSource::Project);
    assert_eq!(
        registry
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic
                .message
                .contains("duplicate agent profile id default"))
            .count(),
        2,
        "expected diagnostics for user overriding built-in and project overriding user"
    );
}

fn write_file(path: impl AsRef<std::path::Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content.trim_start()).unwrap();
}
