// Manual test: run pi-coding-agent with a faux provider.
// No API key needed — uses scripted responses.
//
// Usage:
//   cargo run -p pi-coding-agent --example manual_test
//
// Or with custom prompt:
//   cargo run -p pi-coding-agent --example manual_test -- hello world

use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::Model;
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
    registry::register(
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
        input: 0.0,
        output: 0.0,
        cache_read: None,
        cache_write: None,
        context_window: 0,
        max_tokens: None,
        headers: None,
    };

    let opts = PrintModeOptions {
        prompt,
        model,
        api_key: None,
        system_prompt: Some("Be brief.".into()),
        max_turns: 5,
        tools: Vec::new(),
        register_builtins: false,
    };

    match run_print_mode(opts).await {
        Ok(text) => println!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }

    registry::unregister(api);
}
