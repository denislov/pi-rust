use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{Model, ModelCost, ModelInput, StopReason, Usage};
use pi_coding_agent::print_mode::PrintModeOptions;
use pi_coding_agent::run_print_mode;
use pi_coding_agent::runtime::{SessionMode, SessionRunOptions};
use pi_coding_agent::session::ResolvedSessionTarget;
use std::sync::Arc;

fn faux_model(api: &str) -> Model {
    faux_model_with_window(api, 0)
}

/// Like [`faux_model`] but with an explicit `context_window`. The default
/// [`faux_model`] keeps `context_window: 0` (never auto-compacts under the
/// context-window-gated trigger); tests that exercise auto compaction should
/// pick a window appropriate to their scenario.
fn faux_model_with_window(api: &str, context_window: u32) -> Model {
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
        context_window,
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

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains(r#""type":"session_info""#));
    assert!(text.contains("test-session-name"));
    registry::unregister(api);
}

#[tokio::test]
async fn persists_compaction_entry_when_continued_session_is_too_large() {
    // 60k context window; default reserve_tokens=16_384 → trigger threshold
    // 43_616. The first session's single long user prompt (~36k heuristic
    // tokens, no assistant usage yet) stays under the threshold, so it does
    // NOT compact. The first session's provider call reports a large
    // `usage.total_tokens` (50k) to mirror a real provider reporting the
    // accumulated context size; when the second session loads that assistant
    // message, `estimate_context_tokens` anchors on it (50k + trailing) and
    // exceeds 43_616, so compaction fires and the entry is persisted.
    let api_first = "session-print-compaction-first";
    registry::register(
        api_first,
        Arc::new(
            FauxProvider::simple_text("stored").with_default_usage(Usage {
                input: 49_980,
                output: 20,
                total_tokens: 50_000,
                ..Default::default()
            }),
        ),
    );
    let dir = tempfile::tempdir().unwrap();

    let project_dir = dir.path().join("project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let sessions_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let long_prompt = "old context ".repeat(12_120);
    let first = run_print_mode(PrintModeOptions {
        prompt: long_prompt.clone(),
        model: faux_model_with_window(api_first, 60_000),
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
        invocation: pi_coding_agent::PromptInvocation::Text(long_prompt),
    })
    .await
    .unwrap();
    assert_eq!(first, "stored");
    registry::unregister(api_first);

    let api_second = "session-print-compaction-second";
    registry::register(
        api_second,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("compact summary", StopReason::Stop),
            FauxProvider::text_call("after compaction", StopReason::Stop),
        ])),
    );

    let second = run_print_mode(PrintModeOptions {
        prompt: "continue".into(),
        model: faux_model_with_window(api_second, 60_000),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        session: Some(SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: project_dir,
            session_dir: Some(sessions_dir),
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

    assert_eq!(second, "after compaction");

    let mut files = Vec::new();
    collect_jsonl_files(dir.path(), &mut files);
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.contains(r#""type":"compaction""#));
    assert!(text.contains(r#""summary":"compact summary""#));
    assert!(text.contains(r#""firstKeptEntryId""#));
    registry::unregister(api_second);
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
