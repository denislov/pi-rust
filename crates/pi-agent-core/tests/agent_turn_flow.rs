mod common;

use pi_agent_core::agent_turn_flow::{AgentTurnContext, PrepareContextNode};
use pi_agent_core::flow::Flow;
use pi_agent_core::{
    Agent, AgentConfig, AgentMessage, AgentResources, AgentTool, PromptTemplate, Skill,
};
use pi_ai::types::StreamOptions;

#[test]
fn agent_turn_context_snapshots_agent_state_without_draining_queues() {
    let resources = AgentResources {
        skills: vec![Skill {
            name: "rust".into(),
            description: "Rust guidance".into(),
            location: "/skills/rust/SKILL.md".into(),
            content: "Use Rust idioms.".into(),
            disable_model_invocation: false,
        }],
        prompt_templates: vec![PromptTemplate {
            name: "review".into(),
            description: "Review code".into(),
            content: "Review $1".into(),
            location: "/prompts/review.md".into(),
        }],
    };
    let mut config = AgentConfig::new(common::faux_model("test-api"));
    config.system_prompt = Some("system rules".into());
    config.max_turns = Some(3);
    config.resources = resources;

    let agent = Agent::new(config);
    agent.add_message(AgentMessage::UserText {
        message_id: "user_0".into(),
        text: "hello".into(),
    });
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    ));
    agent.steer("steer this turn");
    agent.follow_up("follow up next");

    let context = AgentTurnContext::from_agent(&agent);

    assert_eq!(
        context.config.system_prompt.as_deref(),
        Some("system rules")
    );
    assert_eq!(context.config.max_turns, Some(3));
    assert_eq!(context.messages.len(), 1);
    assert!(matches!(
        &context.messages[0],
        AgentMessage::UserText { text, .. } if text == "hello"
    ));
    assert_eq!(context.tools.len(), 1);
    assert_eq!(context.tools[0].name, "echo");
    assert_eq!(context.resources.skills.len(), 1);
    assert_eq!(context.resources.skills[0].name, "rust");
    assert_eq!(context.resources.prompt_templates.len(), 1);
    assert_eq!(context.resources.prompt_templates[0].name, "review");
    assert_eq!(context.steering_queue.len(), 1);
    assert_eq!(context.follow_up_queue.len(), 1);
    assert_eq!(context.turn, 0);
    assert!(context.provider_request.is_none());
    assert!(context.assistant_message.is_none());
    assert!(context.pending_tool_calls.is_empty());
    assert!(context.tool_results.is_empty());
    assert!(context.events.is_empty());
    assert!(!context.cancel_token.is_cancelled());

    let drained = agent.drain_steering_queue();
    assert_eq!(drained.len(), 1);
}

#[tokio::test]
async fn prepare_context_node_builds_provider_request_from_context_snapshot() {
    let mut config = AgentConfig::new(common::faux_model("test-api"));
    config.system_prompt = Some("system rules".into());
    config.stream_options = Some(StreamOptions {
        temperature: Some(0.2),
        max_tokens: Some(123),
        ..Default::default()
    });

    let agent = Agent::new(config);
    agent.add_message(AgentMessage::UserText {
        message_id: "user_0".into(),
        text: "hello".into(),
    });
    agent.add_tool(AgentTool::new_text(
        "echo",
        "echo input",
        serde_json::json!({"type": "object"}),
        |_| async { Ok("ok".to_string()) },
    ));

    let (expected_context, expected_options) = agent.provider_request_snapshot();
    let expected_options = expected_options.expect("stream options should be configured");

    let mut context = AgentTurnContext::from_agent(&agent);
    let mut flow = Flow::new("prepare_context").unwrap();
    flow.add_node("prepare_context", PrepareContextNode)
        .unwrap();

    let outcome = flow.run(&mut context).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "prepare_context");
    let request = context
        .provider_request
        .as_ref()
        .expect("node should attach provider request");
    assert_eq!(request.model.id, "faux-model");
    assert_eq!(request.context, expected_context);
    assert_eq!(
        request.stream_options.temperature,
        expected_options.temperature
    );
    assert_eq!(
        request.stream_options.max_tokens,
        expected_options.max_tokens
    );
    assert!(request.stream_options.cancel.is_some());
}
