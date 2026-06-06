mod common;
use common::faux_model;
use futures::StreamExt;
use pi_agent_core::{Agent, AgentConfig, AgentTool, ToolExecutionMode};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{ContentBlock, StopReason};
use std::sync::Arc;
use std::time::Duration;

fn delayed_tool(name: &str, delay_ms: u64, output_text: &str) -> AgentTool {
    let name = name.to_string();
    let text = output_text.to_string();
    AgentTool {
        name,
        description: format!("delayed {}ms", delay_ms),
        parameters: serde_json::json!({"type": "object"}),
        execution_mode: None,
        execute: Arc::new(move |_| {
            let text = text.clone();
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                Ok(vec![ContentBlock::Text {
                    text,
                    text_signature: None,
                }])
            })
        }),
    }
}

fn make_two_tool_call_provider(api: &str, call1_name: &str, call2_name: &str) {
    let json_args = serde_json::json!({}).to_string();
    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::single_call(
                vec![pi_ai::providers::faux::FauxResponse {
                    text_deltas: vec![],
                    thinking_deltas: vec![],
                    tool_calls: vec![
                        pi_ai::providers::faux::FauxToolCall {
                            id: "tool_slow".into(),
                            name: call1_name.into(),
                            deltas: vec![json_args.clone()],
                            final_arguments: serde_json::json!({}),
                        },
                        pi_ai::providers::faux::FauxToolCall {
                            id: "tool_fast".into(),
                            name: call2_name.into(),
                            deltas: vec![json_args],
                            final_arguments: serde_json::json!({}),
                        },
                    ],
                }],
                StopReason::ToolUse,
            ),
            FauxProvider::text_call("done.", StopReason::Stop),
        ])),
    );
}

#[tokio::test]
async fn parallel_tools_finish_faster_than_sequential_tools() {
    let api_par = "parallel-faster-par";
    let api_seq = "parallel-faster-seq";

    let mut config_par = AgentConfig::new(faux_model(api_par));
    config_par.tool_execution = ToolExecutionMode::Parallel;
    config_par.max_turns = 5;

    let agent_par = Agent::new(config_par);
    agent_par.add_tool(delayed_tool("slow", 100, "slow_result"));
    agent_par.add_tool(delayed_tool("fast", 100, "fast_result"));

    make_two_tool_call_provider(api_par, "slow", "fast");

    let par_start = tokio::time::Instant::now();
    let mut stream = agent_par.prompt("go");
    while stream.next().await.is_some() {}
    let parallel_ms = par_start.elapsed().as_millis();

    let mut config_seq = AgentConfig::new(faux_model(api_seq));
    config_seq.tool_execution = ToolExecutionMode::Sequential;
    config_seq.max_turns = 5;

    let agent_seq = Agent::new(config_seq);
    agent_seq.add_tool(delayed_tool("slow", 100, "slow_result"));
    agent_seq.add_tool(delayed_tool("fast", 100, "fast_result"));

    make_two_tool_call_provider(api_seq, "slow", "fast");

    let seq_start = tokio::time::Instant::now();
    let mut stream = agent_seq.prompt("go");
    while stream.next().await.is_some() {}
    let sequential_ms = seq_start.elapsed().as_millis();

    assert!(parallel_ms < 180, "parallel took {}ms", parallel_ms);
    assert!(sequential_ms >= 190, "sequential took {}ms", sequential_ms);

    registry::unregister(api_par);
    registry::unregister(api_seq);
}

#[tokio::test]
async fn parallel_tool_results_are_appended_in_assistant_order() {
    let api = "parallel-order";
    let mut config = AgentConfig::new(faux_model(api));
    config.tool_execution = ToolExecutionMode::Parallel;
    config.max_turns = 5;

    let agent = Agent::new(config);
    agent.add_tool(delayed_tool("slow", 100, "slow_result"));
    agent.add_tool(delayed_tool("fast", 10, "fast_result"));

    make_two_tool_call_provider(api, "slow", "fast");

    let mut stream = agent.prompt("go");
    while stream.next().await.is_some() {}

    let results: Vec<_> = agent
        .messages()
        .into_iter()
        .filter_map(|msg| match msg {
            pi_agent_core::AgentMessage::ToolResult { tool_name, .. } => Some(tool_name),
            _ => None,
        })
        .collect();
    assert_eq!(results, vec!["slow", "fast"]);

    registry::unregister(api);
}

#[tokio::test]
async fn parallel_tool_end_events_are_emitted_in_completion_order() {
    let api = "parallel-event-order";
    let mut config = AgentConfig::new(faux_model(api));
    config.tool_execution = ToolExecutionMode::Parallel;
    config.max_turns = 5;

    let agent = Agent::new(config);
    agent.add_tool(delayed_tool("slow", 100, "slow_result"));
    agent.add_tool(delayed_tool("fast", 10, "fast_result"));

    make_two_tool_call_provider(api, "slow", "fast");

    let mut stream = agent.prompt("go");
    let mut end_events = Vec::new();
    while let Some(event) = stream.next().await {
        if let pi_agent_core::AgentEvent::ToolCallEnd { tool_name, .. } = event {
            end_events.push(tool_name);
        }
    }

    assert_eq!(end_events, vec!["fast", "slow"]);

    registry::unregister(api);
}

#[tokio::test]
async fn per_tool_sequential_override_forces_batch_sequential() {
    let api = "parallel-override";
    let mut config = AgentConfig::new(faux_model(api));
    config.tool_execution = ToolExecutionMode::Parallel;
    config.max_turns = 5;

    let agent = Agent::new(config);
    agent.add_tool(delayed_tool("slow", 100, "slow_result"));
    let mut fast_tool = delayed_tool("fast", 100, "fast_result");
    fast_tool.execution_mode = Some(ToolExecutionMode::Sequential);
    agent.add_tool(fast_tool);

    make_two_tool_call_provider(api, "slow", "fast");

    let start = tokio::time::Instant::now();
    let mut stream = agent.prompt("go");
    while stream.next().await.is_some() {}
    let elapsed = start.elapsed().as_millis();

    assert!(elapsed >= 190, "sequential override elapsed {}ms", elapsed);

    registry::unregister(api);
}
