use futures::StreamExt;
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentTool};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{ContentBlock, Model, StopReason};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let provider = Arc::new(FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("I'll search for that.", StopReason::ToolUse),
        FauxProvider::text_call("Done searching. The answer is 42.", StopReason::Stop),
    ]));
    registry::register("faux-api", provider);

    let model = Model {
        id: "faux-model".into(),
        name: "Faux Model".into(),
        api: "faux-api".into(),
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

    let agent = Agent::new(AgentConfig {
        model,
        system_prompt: Some("You are a helpful assistant.".into()),
        max_turns: 5,
        stream_options: None,
    });

    agent.add_tool(AgentTool {
        name: "search".into(),
        description: "Search the web".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        execute: Arc::new(|_args| {
            Box::pin(async move {
                Ok(vec![ContentBlock::Text {
                    text: "search results: 42 is the answer".into(),
                    text_signature: None,
                }])
            })
        }),
    });

    println!("=== pi-agent-core loop example ===\n");

    let mut stream = agent.prompt("What is the meaning of life?");
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::TurnStart { turn } => {
                println!("--- Turn {} ---", turn);
            }
            AgentEvent::LlmEvent(e) => {
                if let pi_ai::types::AssistantMessageEvent::TextDelta { delta, .. } = &e {
                    print!("{}", delta);
                }
            }
            AgentEvent::ToolCallStart { tool_name, .. } => {
                println!("\n[tool call: {}]", tool_name);
            }
            AgentEvent::ToolCallEnd { result, .. } => match result {
                Ok(blocks) => println!("[tool result: {:?}]", blocks),
                Err(e) => println!("[tool error: {}]", e),
            },
            AgentEvent::AgentDone { message } => {
                println!("\n\nDone — stop reason: {:?}", message.stop_reason);
            }
            AgentEvent::AgentError { error } => {
                eprintln!("\nError: {}", error);
            }
        }
    }

    println!("\n=== Final messages ({}) ===", agent.messages().len());
    for msg in agent.messages() {
        match msg {
            pi_agent_core::AgentMessage::UserText { text, .. } => println!("  User: {}", text),
            pi_agent_core::AgentMessage::Assistant { .. } => println!("  Assistant (response)"),
            pi_agent_core::AgentMessage::ToolResult { tool_call_id, .. } => {
                println!("  ToolResult: {}", tool_call_id)
            }
            pi_agent_core::AgentMessage::SystemPrompt { text, .. } => {
                println!("  System: {}", text)
            }
        }
    }
}
