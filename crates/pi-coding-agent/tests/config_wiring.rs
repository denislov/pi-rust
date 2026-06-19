use pi_coding_agent::config;

#[test]
fn select_model_uses_default_model_when_no_flag() {
    use pi_coding_agent::{CliArgs, select_model};
    let args = CliArgs::default(); // args.model is None
    // default_model resolves via lookup_model; use a known built-in id.
    let model = select_model(&args, Some("claude-sonnet-4-5"), None).expect("model");
    assert_eq!(model.id, "claude-sonnet-4-5");
}

#[test]
fn load_config_from_temp_pi_rust_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "default_model = \"claude-sonnet-4-5\"\n",
    )
    .unwrap();
    // SAFETY: single-threaded integration test.
    unsafe {
        std::env::set_var("PI_RUST_DIR", dir.path().to_str().unwrap());
    }
    let (cfg, diags) = config::load_config(std::path::Path::new("."));
    assert_eq!(
        cfg.settings.default_model.as_deref(),
        Some("claude-sonnet-4-5")
    );
    assert!(diags.is_empty());
    unsafe {
        std::env::remove_var("PI_RUST_DIR");
    }
}
