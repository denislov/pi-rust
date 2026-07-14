// global provider runtime compatibility example.
// This example exercises Agent::run()/Agent::prompt(), which currently reaches
// providers through pi-agent-core's global AI runtime compatibility boundary.
// Direct provider-facing examples should prefer scoped
// pi_ai::api::ProviderRegistry/AiClient.

use futures::StreamExt;
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentTool, AgentToolOutput};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{ContentBlock, Model, ModelCost, ModelInput, StopReason};
use std::sync::Arc;

#[allow(deprecated)]
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

    let mut config = AgentConfig::new(model);
    config.system_prompt = Some("You are a helpful assistant.".into());
    config.max_turns = Some(5);

    let agent = Agent::new(config);

    agent.add_tool(AgentTool {
        name: "search".into(),
        description: "Search the web".into(),
        parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        execution_mode: None,
        execute: Arc::new(|_args, _on_update| {
            Box::pin(async move {
                Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                    text: "search results: 42 is the answer".into(),
                    text_signature: None,
                }]))
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
            AgentEvent::BeforeProviderRequest { .. } => {}
            AgentEvent::LlmEvent(e) => {
                if let pi_ai::types::AssistantMessageEvent::TextDelta { delta, .. } = &e {
                    print!("{}", delta);
                }
            }
            AgentEvent::ToolCallStart { tool_name, .. } => {
                println!("\n[tool call: {}]", tool_name);
            }
            AgentEvent::ToolCallUpdate { update, .. } => {
                println!("[tool update: {:?}]", update.content);
            }
            AgentEvent::ToolCallEnd { result, .. } => {
                if result.is_error {
                    println!("[tool error: {:?}]", result.content);
                } else {
                    println!("[tool result: {:?}]", result.content);
                }
            }
            AgentEvent::AgentDone { message } => {
                println!("\n\nDone — stop reason: {:?}", message.stop_reason);
            }
            AgentEvent::AgentError { error } => {
                eprintln!("\nError: {}", error);
            }
            AgentEvent::SessionCompacted { summary, .. } => {
                println!("\n[compacted: {}]", summary);
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
            pi_agent_core::AgentMessage::CompactionSummary { summary, .. } => {
                println!("  Compaction: {}", summary)
            }
            pi_agent_core::AgentMessage::BashExecution { command, .. } => {
                println!("  BashExecution: {}", command)
            }
            pi_agent_core::AgentMessage::Custom { custom_type, .. } => {
                println!("  Custom: {}", custom_type)
            }
            pi_agent_core::AgentMessage::BranchSummary { from_id, .. } => {
                println!("  BranchSummary from {}", from_id)
            }
        }
    }
}
