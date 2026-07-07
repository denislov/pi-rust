mod support;

use futures::stream;
use pi_ai::EventStream;
use pi_ai::providers::faux::{FauxCall, FauxResponse, FauxToolCall};
use pi_ai::registry::ApiProvider;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
    ModelInput, StopReason, StreamOptions,
};
use pi_coding_agent::{PrintModeOptions, builtin_tools, run_print_mode};
use std::sync::{Arc, Mutex};
use support::ProviderGuard;
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

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

fn read_call(path: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "tool_1".into(),
            name: "read".into(),
            deltas: vec![format!(r#"{{"path":"{path}"}}"#)],
            final_arguments: serde_json::json!({ "path": path }),
        }],
    }
}

fn grep_call(pattern: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![],
        thinking_deltas: vec![],
        tool_calls: vec![FauxToolCall {
            id: "tool_1".into(),
            name: "grep".into(),
            deltas: vec![format!(r#"{{"pattern":"{pattern}","literal":true}}"#)],
            final_arguments: serde_json::json!({ "pattern": pattern, "literal": true }),
        }],
    }
}

fn message_for_call(model_id: &str, call: &FauxCall) -> AssistantMessage {
    let mut message = AssistantMessage::empty("recording-faux", model_id);
    for response in &call.responses {
        if !response.text_deltas.is_empty() {
            message.content.push(ContentBlock::Text {
                text: response.text_deltas.join(""),
                text_signature: None,
            });
        }
        for tool_call in &response.tool_calls {
            message.content.push(ContentBlock::ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.final_arguments.clone(),
                thought_signature: None,
            });
        }
    }
    message.stop_reason = call.stop_reason.clone();
    message
}

struct RecordingProvider {
    calls: Mutex<Vec<FauxCall>>,
    contexts: Arc<Mutex<Vec<Context>>>,
}

impl RecordingProvider {
    fn new(calls: Vec<FauxCall>, contexts: Arc<Mutex<Vec<Context>>>) -> Self {
        Self {
            calls: Mutex::new(calls),
            contexts,
        }
    }
}

impl ApiProvider for RecordingProvider {
    fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        self.contexts.lock().unwrap().push(ctx);
        let call = self.calls.lock().unwrap().remove(0);
        let message = message_for_call(&model.id, &call);
        Box::pin(stream::iter(vec![AssistantMessageEvent::Done {
            reason: call.stop_reason,
            message,
        }]))
    }
}

async fn run_scripted_read(
    api: &str,
    path: &str,
    cwd: std::path::PathBuf,
    contexts: Arc<Mutex<Vec<Context>>>,
) -> String {
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(RecordingProvider::new(
            vec![
                FauxCall {
                    responses: vec![read_call(path)],
                    stop_reason: StopReason::ToolUse,
                },
                FauxCall {
                    responses: vec![text_response("done")],
                    stop_reason: StopReason::Stop,
                },
            ],
            contexts,
        )),
    );
    run_print_mode(PrintModeOptions {
        prompt: "read it".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: builtin_tools(cwd),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("read it".into()),
    })
    .await
    .unwrap()
}

#[test]
fn builtin_tools_has_seven() {
    let tools = pi_coding_agent::builtin_tools(std::path::PathBuf::from("."));
    let names: Vec<_> = tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(
        names,
        vec!["read", "write", "edit", "bash", "grep", "find", "ls"]
    );
}

#[tokio::test]
async fn read_builtin_tool_success_loop_completes() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("input.txt"), "hello").unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));

    let out = run_scripted_read(
        "pi-coding-tools-e2e-success",
        "input.txt",
        dir.path().to_path_buf(),
        contexts.clone(),
    )
    .await;

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    assert_eq!(contexts.len(), 2);
}

#[tokio::test]
async fn read_builtin_tool_error_is_sent_back_to_model_and_loop_completes() {
    let dir = tempdir().unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));

    let out = run_scripted_read(
        "pi-coding-tools-e2e-error",
        "missing.txt",
        dir.path().to_path_buf(),
        contexts.clone(),
    )
    .await;

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    let second_call = contexts
        .get(1)
        .expect("second model call should include tool result");
    let tool_result = second_call
        .messages
        .iter()
        .find_map(|message| match message {
            Message::ToolResult {
                tool_name,
                is_error,
                content,
                ..
            } if tool_name.as_deref() == Some("read") => Some((is_error, content)),
            _ => None,
        })
        .expect("read tool result should be present in second call context");
    assert_eq!(*tool_result.0, Some(true));
    let text = tool_result
        .1
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("read: cannot"), "{text}");
}

#[tokio::test]
async fn grep_builtin_tool_success_is_sent_back_to_model() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("input.txt"), "alpha\nbeta").unwrap();
    let contexts = Arc::new(Mutex::new(Vec::new()));
    let api = "pi-coding-tools-e2e-grep";

    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(RecordingProvider::new(
            vec![
                FauxCall {
                    responses: vec![grep_call("beta")],
                    stop_reason: StopReason::ToolUse,
                },
                FauxCall {
                    responses: vec![text_response("done")],
                    stop_reason: StopReason::Stop,
                },
            ],
            contexts.clone(),
        )),
    );

    let out = run_print_mode(PrintModeOptions {
        prompt: "search".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: builtin_tools(dir.path().to_path_buf()),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text("search".into()),
    })
    .await
    .unwrap();

    assert_eq!(out, "done");
    let contexts = contexts.lock().unwrap();
    let second_call = contexts
        .get(1)
        .expect("second model call should include grep result");
    let text = second_call
        .messages
        .iter()
        .find_map(|message| match message {
            Message::ToolResult {
                tool_name, content, ..
            } if tool_name.as_deref() == Some("grep") => Some(content),
            _ => None,
        })
        .expect("grep tool result should be present")
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("input.txt:2: beta"), "{text}");
}
