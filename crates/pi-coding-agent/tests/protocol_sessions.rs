use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{CliRunOptions, SessionRunOptions, protocol::rpc::run_rpc_mode_for_io};
use std::sync::Arc;
use tempfile::tempdir;

fn faux_model(api: &str) -> Model {
    Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

#[tokio::test]
async fn rpc_prompt_persists_session_messages() {
    let dir = tempdir().unwrap();
    let cwd = dir.path().join("project");
    let sessions = dir.path().join("sessions");
    std::fs::create_dir_all(&cwd).unwrap();
    let api = "pi-coding-rpc-session";
    registry::register(api, Arc::new(FauxProvider::simple_text("Hello")));
    let mut session_options = SessionRunOptions::enabled(cwd);
    session_options.session_dir = Some(sessions.clone());

    let input = b"{\"id\":\"n1\",\"type\":\"set_session_name\",\"name\":\"rpc work\"}\n\
                  {\"id\":\"p1\",\"type\":\"prompt\",\"message\":\"hello\"}\n";
    let mut output = Vec::new();
    run_rpc_mode_for_io(
        &input[..],
        &mut output,
        CliRunOptions {
            model_override: Some(faux_model(api)),
            tools: Vec::new(),
            register_builtins: false,
            session: session_options,
        },
    )
    .await
    .unwrap();

    let mut session_files = Vec::new();
    collect_jsonl_files(&sessions, &mut session_files);
    assert_eq!(session_files.len(), 1);
    let contents = std::fs::read_to_string(&session_files[0]).unwrap();
    assert!(contents.contains("\"type\":\"session\""));
    assert!(contents.contains("\"type\":\"session_info\""));
    assert!(contents.contains("\"role\":\"user\""));
    assert!(contents.contains("\"role\":\"assistant\""));
    registry::unregister(api);
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_jsonl_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
}
