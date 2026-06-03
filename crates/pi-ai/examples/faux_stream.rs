use std::sync::Arc;
use futures::StreamExt;
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_ai::types::*;

#[tokio::main]
async fn main() {
    let provider = Arc::new(FauxProvider::new(vec![
        FauxResponse {
            text_deltas: vec!["Thinking step-by-step...\n".into(), "The answer ".into(), "is 42.".into()],
            thinking_deltas: vec![],
            tool_calls: vec![],
        },
    ]));
    registry::register("faux-api", provider);

    let model = Model {
        id: "faux-model".into(), name: "Faux Model".into(),
        api: "faux-api".into(), provider: "faux".into(),
        base_url: String::new(), reasoning: false,
        input: 0.0, output: 0.0, cache_read: None, cache_write: None,
        context_window: 0, max_tokens: None, headers: None,
    };

    let ctx = Context {
        system_prompt: Some("Answer concisely.".into()),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text {
                text: "What is the meaning of life?".into(),
                text_signature: None,
            }],
        }],
        tools: None,
    };

    let mut stream = registry::stream_model(&model, ctx, None);

    println!("=== faux provider streaming demo ===\n");
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::TextDelta { delta, .. } => {
                print!("{}", delta);
            }
            AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                print!("[think] {}", delta);
            }
            AssistantMessageEvent::ToolcallDelta { delta, .. } => {
                print!("[tool: {}]", delta);
            }
            AssistantMessageEvent::Done { message, .. } => {
                println!("\n\n--- Done ---");
                println!("stop reason: {:?}", message.stop_reason);
                println!("usage: {:?}", message.usage);
            }
            AssistantMessageEvent::Error { message, .. } => {
                eprintln!("\nError: {}", message.error_message.as_deref().unwrap_or("unknown error"));
            }
            _ => {}
        }
    }
    println!("=== end ===");
}
