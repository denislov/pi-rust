mod support;

use pi_coding_agent::config;
use support::EnvGuard;

#[test]
fn select_model_uses_default_model_when_no_flag() {
    use pi_coding_agent::api::{CliArgs, select_model};
    let args = CliArgs::default(); // args.model is None
    // default_model resolves via lookup_model; use a known built-in id.
    let model = select_model(&args, None, Some("claude-sonnet-4-5"), None).expect("model");
    assert_eq!(model.id, "claude-sonnet-4-5");
}

#[test]
fn load_config_from_temp_pi_rust_dir() {
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "default_model = \"claude-sonnet-4-5\"\n",
    )
    .unwrap();
    env.set_pi_rust_dir(dir.path());
    let (cfg, diags) = config::load_config(std::path::Path::new("."));
    assert_eq!(
        cfg.settings.default_model.as_deref(),
        Some("claude-sonnet-4-5")
    );
    assert!(diags.is_empty());
}

#[test]
fn config_auth_resolution_prefers_env_over_auth_file() {
    let env = EnvGuard::new(&["PI_RUST_DIR", "ANTHROPIC_API_KEY"]);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("auth.toml"),
        "[anthropic]\ntype = \"api_key\"\nkey = \"from-auth\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("auth.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    env.set_pi_rust_dir(dir.path());
    env.set("ANTHROPIC_API_KEY", "from-env");

    let (cfg, diags) = config::load_config(std::path::Path::new("."));
    let mut key_diags = Vec::new();
    let key =
        config::auth::resolve_api_key("anthropic", None, &cfg.auth, &mut key_diags).expect("key");

    assert_eq!(key.value, "from-env");
    assert_eq!(key.source, config::auth::KeySource::Env);
    assert!(diags.is_empty());
    assert!(key_diags.is_empty());
}

#[test]
fn runtime_setting_helpers_consume_session_dir_and_context_flag() {
    use pi_coding_agent::api::{effective_no_context_files, effective_session_dir, parse_args};

    let args = parse_args(vec!["-p".to_string(), "hello".to_string()]).unwrap();
    let mut settings = config::settings::PartialSettings {
        session_dir: Some("/tmp/pi-sessions".into()),
        no_context_files: Some(true),
        ..Default::default()
    }
    .resolve();

    assert_eq!(
        effective_session_dir(&args, &settings).as_deref(),
        Some(std::path::Path::new("/tmp/pi-sessions"))
    );
    assert!(effective_no_context_files(&args, &settings));

    let args = parse_args(vec![
        "--session-dir".to_string(),
        "/tmp/cli-sessions".to_string(),
        "-p".to_string(),
        "hello".to_string(),
    ])
    .unwrap();
    settings.no_context_files = false;
    assert_eq!(
        effective_session_dir(&args, &settings).as_deref(),
        Some(std::path::Path::new("/tmp/cli-sessions"))
    );
    assert!(!effective_no_context_files(&args, &settings));
}
