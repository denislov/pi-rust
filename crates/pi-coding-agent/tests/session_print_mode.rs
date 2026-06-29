use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::print_mode::PrintModeOptions;
use pi_coding_agent::run_print_mode;
use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
use pi_coding_agent::session::ResolvedSessionTarget;
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
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "hello");

    let session_dirs = session_dirs(&sessions_dir);
    assert_eq!(session_dirs.len(), 1);
    assert!(session_dirs[0].join("session.json").is_file());
    let text = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(text.contains(r#""kind":"session.created""#));
    assert!(text.contains(r#""kind":"turn.input.recorded""#));
    assert!(text.contains(r#""kind":"message.completed""#));
    assert!(!text.contains(r#""type":"session""#));
    registry::unregister(api);
}

#[tokio::test]
async fn enabled_session_with_name_uses_rust_native_log() {
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
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: Some("test-session-name".into()),
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "named");

    let session_dirs = session_dirs(&sessions_dir);
    assert_eq!(session_dirs.len(), 1);
    assert!(session_dirs[0].join("session.json").is_file());
    let text = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert!(text.contains(r#""kind":"session.created""#));
    assert!(!text.contains(r#""type":"session_info""#));
    registry::unregister(api);
}

#[tokio::test]
async fn continues_most_recent_rust_native_session() {
    let api_first = "session-print-compaction-first";
    registry::register(api_first, Arc::new(FauxProvider::simple_text("stored")));
    let dir = tempfile::tempdir().unwrap();

    let project_dir = dir.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let first = run_print_mode(PrintModeOptions {
        prompt: "old context".into(),
        model: faux_model(api_first),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir.clone(),
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("old context".into()),
    })
    .await
    .unwrap();
    assert_eq!(first, "stored");
    registry::unregister(api_first);

    let api_second = "session-print-compaction-second";
    registry::register(api_second, Arc::new(FauxProvider::simple_text("continued")));

    let second = run_print_mode(PrintModeOptions {
        prompt: "continue".into(),
        model: faux_model(api_second),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir.clone()),
        }),
        session_target: Some(ResolvedSessionTarget::ContinueMostRecent),
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("continue".into()),
    })
    .await
    .unwrap();

    assert_eq!(second, "continued");

    let session_dirs = session_dirs(&sessions_dir);
    assert_eq!(session_dirs.len(), 1);
    let text = std::fs::read_to_string(session_dirs[0].join("events.jsonl")).unwrap();
    assert_eq!(text.matches(r#""kind":"turn.input.recorded""#).count(), 2);
    assert_eq!(text.matches(r#""kind":"message.completed""#).count(), 2);
    assert!(!text.contains(r#""type":"compaction""#));
    registry::unregister(api_second);
}

fn session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect()
}
