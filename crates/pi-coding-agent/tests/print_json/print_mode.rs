use crate::support;

use pi_agent_core::api::tool::{AgentTool, AgentToolOutput};
use pi_ai::api::conversation::{ContentBlock, StopReason};
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::testing::{FauxCall, FauxProvider, FauxResponse, FauxToolCall};
use pi_coding_agent::api::operation::PromptInvocation;
use pi_coding_agent::api::protocol::{PrintModeOptions, run_print_mode};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::ProviderGuard;

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
            known: true,
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
        execute: Arc::new(|_context, args, _on_update| {
            let text = args
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let result = vec![ContentBlock::Text {
                text: format!("echo: {text}"),
                text_signature: None,
            }];
            Box::pin(async move { Ok(AgentToolOutput::new(result)) })
        }),
    }
}

fn mutation_tool(executions: Arc<AtomicUsize>) -> AgentTool {
    AgentTool {
        name: "mutate_external_state".into(),
        description: "mutates external state".into(),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_context, _args, _on_update| {
            let executions = executions.clone();
            Box::pin(async move {
                executions.fetch_add(1, Ordering::SeqCst);
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "mutated".into(),
                    text_signature: None,
                }]))
            })
        }),
    }
}

#[tokio::test]
async fn prints_single_turn_text_response() {
    let api = "pi-coding-print-text";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::new(vec![text_response("Hello")])),
    );

    let output = run_print_mode(PrintModeOptions {
        prompt: "hi".into(),
        model: faux_model(api),
        api_key: None,
        system_prompt: None,
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::api::resources::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Hello");
}

#[tokio::test]
async fn treats_length_as_successful_final_text() {
    let api = "pi-coding-print-length";
    let _provider_guard = ProviderGuard::register(
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
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::api::resources::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Partial final text");
}

#[tokio::test]
async fn returns_agent_failure_on_error_stop_reason() {
    let api = "pi-coding-print-error";
    let _provider_guard = ProviderGuard::register(
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
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::api::resources::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hi".into()),
    })
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "agent failure: LLM error");
}

#[tokio::test]
async fn supports_tool_call_loop_with_injected_tool() {
    let api = "pi-coding-print-tool-loop";
    let _provider_guard = ProviderGuard::register(
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
        max_turns: Some(5),
        tools: vec![echo_tool()],
        register_builtins: false,
        ai_client: Some(_provider_guard.ai_client()),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::api::resources::AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("echo hi".into()),
    })
    .await
    .unwrap();

    assert_eq!(output, "Tool completed");
}

#[tokio::test]
async fn print_mode_denies_unknown_mutation_without_waiting_or_executing() {
    let api = "pi-coding-print-tool-authorization-deny";
    let _provider_guard = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxCall {
                responses: vec![FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![FauxToolCall {
                        id: "tool_mutate".into(),
                        name: "mutate_external_state".into(),
                        deltas: vec!["{}".into()],
                        final_arguments: serde_json::json!({}),
                    }],
                }],
                stop_reason: StopReason::ToolUse,
            },
            FauxCall {
                responses: vec![text_response("Mutation was denied; continuing safely")],
                stop_reason: StopReason::Stop,
            },
        ])),
    );
    let executions = Arc::new(AtomicUsize::new(0));

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        run_print_mode(PrintModeOptions {
            prompt: "mutate".into(),
            model: faux_model(api),
            api_key: None,
            system_prompt: None,
            max_turns: Some(5),
            tools: vec![mutation_tool(executions.clone())],
            register_builtins: false,
            ai_client: Some(_provider_guard.ai_client()),
            session: None,
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: pi_agent_core::api::resources::AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text("mutate".into()),
        }),
    )
    .await
    .expect("print mode must not wait for interactive authorization")
    .unwrap();

    assert_eq!(output, "Mutation was denied; continuing safely");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}
