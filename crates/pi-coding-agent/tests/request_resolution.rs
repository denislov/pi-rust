use pi_coding_agent::api::{CliRunOptions, SessionRunOptions, builtin_tools, parse_args};
use pi_coding_agent::request::{resolve_cli_context, resolve_prompt_request};
use pi_coding_agent::runtime::PromptInvocation;
use pi_coding_agent::session::ResolvedSessionTarget;

#[test]
fn resolve_prompt_request_builds_common_headless_context() {
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let cwd = temp.path().join("work");
    let session_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(
        global.join("settings.toml"),
        format!(
            "default_model = \"claude-haiku-4-5\"\nsession_dir = \"{}\"\n",
            session_dir.display()
        ),
    )
    .unwrap();
    std::fs::write(global.join("AGENTS.md"), "global instructions").unwrap();

    let parsed = parse_args(vec![
        "-p".into(),
        "hello".into(),
        "--tools".into(),
        "read".into(),
        "--session-id".into(),
        "shared-session".into(),
    ])
    .unwrap();

    let resolved = resolve_prompt_request(
        parsed,
        CliRunOptions {
            tools: builtin_tools(cwd.clone()),
            register_builtins: false,
            session: SessionRunOptions::enabled(cwd.clone()),
            ..CliRunOptions::default()
        },
        Some("from stdin".into()),
        cwd,
        global,
    )
    .unwrap();

    assert_eq!(resolved.context.model.id, "claude-haiku-4-5");
    assert_eq!(resolved.processed_prompt.text, "hello\n\nfrom stdin");
    assert!(matches!(
        resolved.invocation,
        PromptInvocation::Text(ref text) if text == "hello\n\nfrom stdin"
    ));
    assert_eq!(resolved.session_options.tools.len(), 1);
    assert_eq!(resolved.session_options.tools[0].name, "read");
    assert!(matches!(
        resolved.context.session_target,
        Some(ResolvedSessionTarget::OpenOrCreateId(ref id)) if id == "shared-session"
    ));
    assert_eq!(
        resolved
            .context
            .session
            .as_ref()
            .unwrap()
            .session_dir
            .as_ref()
            .unwrap(),
        &session_dir
    );
    assert!(
        resolved
            .context
            .system_prompt
            .as_ref()
            .unwrap()
            .contains("global instructions")
    );
}

#[test]
fn resolve_cli_context_validates_loaded_skill_names_without_prompt() {
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let cwd = temp.path().join("work");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    let parsed = parse_args(vec!["--skill".into(), "missing".into()]).unwrap();
    let err = match resolve_cli_context(
        parsed,
        CliRunOptions {
            tools: builtin_tools(cwd.clone()),
            session: SessionRunOptions::enabled(cwd.clone()),
            ..CliRunOptions::default()
        },
        cwd,
        global,
    ) {
        Ok(_) => panic!("missing skill should fail"),
        Err(error) => error,
    };

    assert_eq!(
        err.to_string(),
        "skill 'missing' not found in loaded skills"
    );
}
