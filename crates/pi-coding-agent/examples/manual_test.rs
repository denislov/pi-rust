// global provider runtime compatibility example.
// Manual test: run pi-coding-agent with a faux provider.
// No API key needed — uses scripted responses.
// This example exercises run_print_mode(), which currently reaches providers
// through pi-coding-agent's runtime compatibility boundary. Direct
// provider-facing examples should prefer scoped pi_ai::api::ProviderRegistry/AiClient.
//
// Usage:
//   cargo run -p pi-coding-agent --example manual_test
//
// Or with custom prompt:
//   cargo run -p pi-coding-agent --example manual_test -- hello world

use pi_ai::AiClient;
use pi_ai::providers::faux::FauxProvider;
use pi_ai::types::{Model, ModelCost, ModelInput};
use pi_coding_agent::{PrintModeOptions, run_print_mode};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let prompt = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let prompt = if prompt.is_empty() {
        "Say hello in one sentence.".to_string()
    } else {
        prompt
    };

    let api = "manual-test";

    // Register a faux provider that returns a scripted text response
    let ai_client = AiClient::new();
    ai_client.register_provider(
        api,
        Arc::new(FauxProvider::new(vec![
            pi_ai::providers::faux::FauxResponse {
                text_deltas: vec![
                    "Hello! This is the faux provider speaking. No real API call was made.".into(),
                ],
                thinking_deltas: vec![],
                tool_calls: vec![],
            },
        ])),
    );

    let model = Model {
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
    };

    let invocation_text = prompt.clone();
    let opts = PrintModeOptions {
        prompt,
        model,
        api_key: None,
        system_prompt: Some("Be brief.".into()),
        max_turns: Some(5),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: Some(ai_client),
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: pi_agent_core::AgentResources::default(),
        settings: None,
        invocation: pi_coding_agent::PromptInvocation::Text(invocation_text),
    };

    match run_print_mode(opts).await {
        Ok(text) => println!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
