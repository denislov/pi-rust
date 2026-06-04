use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::print_mode::PrintModeOptions;
use pi_coding_agent::run_print_mode;
use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
use std::sync::Arc;

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
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[tokio::test]
async fn persists_new_print_mode_session() {
    let api = "session-print-persist";
    registry::register(api, Arc::new(FauxProvider::simple_text("hello")));
    let dir = tempfile::tempdir().unwrap();

    let project_dir = dir.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: None,
    })
    .await
    .unwrap();

    assert_eq!(output, "hello");

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains(r#""type":"session""#));
    assert!(text.contains(r#""role":"user""#));
    assert!(text.contains(r#""role":"assistant""#));
    registry::unregister(api);
}

#[tokio::test]
async fn persists_session_with_name() {
    let api = "session-print-name";
    registry::register(api, Arc::new(FauxProvider::simple_text("named")));
    let dir = tempfile::tempdir().unwrap();

    let project_dir = dir.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: Some("test-session-name".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "named");

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains(r#""type":"session_info""#));
    assert!(text.contains("test-session-name"));
    registry::unregister(api);
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    collect_jsonl_files(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    out.push(path);
                }
            }
        }
    }
}
