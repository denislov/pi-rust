use crate::internal_tests::support;

use crate::internal_tests::cli_fixture::command::{
    ResolvedSessionTarget, parse_args, resolve_cli_context, resolve_prompt_request,
};
use crate::internal_tests::cli_fixture::resources::builtin_tools;
use pi_coding_agent::api::operation::PromptInvocation;
use pi_coding_agent::api::runtime::{CliRunOptions, SessionRunOptions};
use support::EnvGuard;

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
            session_dir.display().to_string().replace('\\', "/")
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
            ai_client: None,
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
fn resolve_prompt_request_preserves_oauth_auth_source_diagnostic() {
    let env = EnvGuard::new(&["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"]);
    env.remove("ANTHROPIC_API_KEY");
    env.remove("CLAUDE_API_KEY");
    env.remove("ANTHROPIC_KEY");

    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let cwd = temp.path().join("work");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(
        global.join("settings.toml"),
        "default_model = \"claude-haiku-4-5\"\n",
    )
    .unwrap();
    std::fs::write(
        global.join("auth.toml"),
        "[anthropic]\ntype = \"oauth\"\naccess = \"oauth-access\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            global.join("auth.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }

    let parsed = parse_args(vec!["-p".into(), "hello".into()]).unwrap();

    let resolved = resolve_prompt_request(
        parsed,
        CliRunOptions {
            tools: builtin_tools(cwd.clone()),
            register_builtins: false,
            ai_client: None,
            session: SessionRunOptions::enabled(cwd.clone()),
            ..CliRunOptions::default()
        },
        None,
        cwd,
        global,
    )
    .unwrap();

    assert_eq!(
        resolved.session_options.api_key.as_deref(),
        Some("oauth-access")
    );
    let diagnostics = resolved
        .session_options
        .auth_diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.field.as_str(), diagnostic.source.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics, vec![("api_key", "auth.toml:oauth")]);
    let diagnostic_json =
        serde_json::to_string(&resolved.session_options.auth_diagnostics).unwrap();
    assert!(!diagnostic_json.contains("oauth-access"));
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
            ai_client: None,
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

#[test]
fn resolve_cli_context_returns_config_and_resource_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let cwd = temp.path().join("work");
    let theme_dir = temp.path().join("themes");
    std::fs::create_dir_all(&global).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&theme_dir).unwrap();
    std::fs::write(
        global.join("settings.toml"),
        format!(
            "themes = [\"{}\"]\n",
            theme_dir.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    std::fs::write(global.join("auth.toml"), "not valid toml").unwrap();
    std::fs::write(theme_dir.join("bad.json"), "{not json").unwrap();

    let parsed = parse_args(vec!["-p".into(), "hello".into()]).unwrap();

    let resolved = resolve_prompt_request(
        parsed,
        CliRunOptions {
            tools: builtin_tools(cwd.clone()),
            register_builtins: false,
            ai_client: None,
            session: SessionRunOptions::enabled(cwd.clone()),
            ..CliRunOptions::default()
        },
        None,
        cwd,
        global,
    )
    .unwrap();

    let messages: Vec<&str> = resolved
        .context
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to parse auth")),
        "{messages:?}"
    );
    assert!(
        resolved
            .context
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("theme_parse_error")),
        "{:?}",
        resolved.context.diagnostics
    );
}
