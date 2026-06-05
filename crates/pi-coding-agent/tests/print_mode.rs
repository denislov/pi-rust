use pi_agent_core::AgentTool;
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_ai::registry;
use pi_ai::types::{ContentBlock, Model, ModelCost, ModelInput, StopReason};
use pi_coding_agent::{CliError, PrintModeOptions, PromptInvocation, run_print_mode};
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

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

fn echo_tool() -> AgentTool {
    AgentTool {
        name: "echo".into(),
        description: "echoes input".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        execution_mode: None,
        execute: Arc::new(|args| {
            let text = args
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {text}"),
                text_signature: None,
            }];
            Box::pin(async move { Ok(result) })
        }),
    }
}

#[tokio::test]
async fn prints_single_turn_text_response() {
    let api = "pi-coding-print-text";
    registry::register(
        api,
        Arc::new(FauxProvider::new(vec![text_response("Hello")])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Hello");
    registry::unregister(api);
}

#[tokio::test]
async fn treats_length_as_successful_final_text() {
    let api = "pi-coding-print-length";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![text_response("Partial final text")],
            stop_reason: StopReason::Length,
        }])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Partial final text");
    registry::unregister(api);
}

#[tokio::test]
async fn returns_agent_failure_on_error_stop_reason() {
    let api = "pi-coding-print-error";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Error,
        }])),
    );

    let error = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap_err();

    assert_eq!(error, CliError::AgentFailure("LLM error".into()));
    registry::unregister(api);
}

#[tokio::test]
async fn supports_tool_call_loop_with_injected_tool() {
    let api = "pi-coding-print-tool-loop";
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_1".into(),
                        name: "echo".into(),
                        deltas: vec!["{\"text\":".into(), "\"hi\"}".into()],
                        final_arguments: serde_json::json!({"text": "hi"}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("Tool completed")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "echo hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: 5,
        tools: vec![echo_tool()],
        register_builtins: false,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        invocation: PromptInvocation::Text("echo hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Tool completed");
    registry::unregister(api);
}
