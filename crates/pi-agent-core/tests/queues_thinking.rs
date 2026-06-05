mod common;
use common::faux_model;
use futures::StreamExt;
use pi_agent_core::{
    Agent, AgentConfig, AgentEvent, AgentMessage, AgentTool, QueueMode, ThinkingLevel,
};
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_ai::registry;
use pi_ai::types::{ContentBlock, Model, ModelCost, ModelInput, StopReason, StreamOptions};
use std::sync::Arc;

fn reasoning_model(api: &str) -> Model {
    let mut m = faux_model(api);
    m.reasoning = true;
    m
}

fn non_reasoning_model(api: &str) -> Model {
    faux_model(api)
}

#[tokio::test]
async fn steer_injects_user_message_before_next_model_call() {
    let api = "steer-test";
    let mut config = AgentConfig::new(faux_model(api));
    config.steering_mode = QueueMode::All;
    config.max_turns = 5;

    let agent = Agent::new(config);

    // Queue a steer message before prompt
    agent.steer("steered message");

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("I see the steered message.", StopReason::Stop),
        ])),
    );

    let mut stream = agent.prompt("initial");
    let events: Vec<_> = stream.collect().await;

    let texts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::LlmEvent(pi_ai::types::AssistantMessageEvent::TextDelta {
                delta, ..
            }) => Some(delta.clone()),
            _ => None,
        })
        .collect();

    assert!(
        texts.contains(&"I see the steered message.".to_string()),
        "should have response"
    );

    // Verify steered message was injected into messages before model call
    let msgs = agent.messages();
    let steer_pos = msgs.iter().position(
        |m| matches!(m, AgentMessage::UserText { text, .. } if text == "steered message"),
    );
    assert!(steer_pos.is_some(), "steered message should be in messages");

    registry::unregister(api);
}

#[tokio::test]
async fn follow_up_continues_after_stop() {
    let api = "followup-test";
    let mut config = AgentConfig::new(faux_model(api));
    config.follow_up_mode = QueueMode::All;
    config.max_turns = 5;

    let agent = Agent::new(config);

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("First response.", StopReason::Stop),
            FauxProvider::text_call("Follow-up response.", StopReason::Stop),
        ])),
    );

    // Set up follow-up before first prompt
    agent.follow_up("follow up question");

    let mut stream = agent.prompt("initial");
    let events: Vec<_> = stream.collect().await;

    let texts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::LlmEvent(pi_ai::types::AssistantMessageEvent::TextDelta {
                delta, ..
            }) => Some(delta.clone()),
            _ => None,
        })
        .collect();

    assert!(
        texts.contains(&"First response.".to_string()),
        "should have first response"
    );
    assert!(
        texts.contains(&"Follow-up response.".to_string()),
        "should have follow-up response"
    );

    registry::unregister(api);
}

#[tokio::test]
async fn one_at_a_time_drains_one_steering_message() {
    let api = "one-at-a-time";
    let mut config = AgentConfig::new(faux_model(api));
    config.steering_mode = QueueMode::OneAtATime;
    config.max_turns = 5;

    let agent = Agent::new(config);

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("seen steer 1", StopReason::Stop),
            FauxProvider::text_call("seen steer 2", StopReason::Stop),
        ])),
    );

    agent.steer("steer 1");
    agent.steer("steer 2");

    let mut stream = agent.prompt("initial");
    let events: Vec<_> = stream.collect().await;

    let texts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::LlmEvent(pi_ai::types::AssistantMessageEvent::TextDelta {
                delta, ..
            }) => Some(delta.clone()),
            _ => None,
        })
        .collect();

    assert!(texts.contains(&"seen steer 1".to_string()));
    assert!(texts.contains(&"seen steer 2".to_string()));
    assert_eq!(texts.len(), 2);

    registry::unregister(api);
}

#[tokio::test]
async fn clear_queues_removes_all_queued_messages() {
    let api = "clear-queues";
    let mut config = AgentConfig::new(faux_model(api));
    config.max_turns = 3;

    let agent = Agent::new(config);

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("response", StopReason::Stop),
        ])),
    );

    agent.steer("steer msg");
    agent.follow_up("followup msg");
    agent.clear_queues();

    let mut stream = agent.prompt("initial");
    while stream.next().await.is_some() {}

    let msgs = agent.messages();
    let has_steer = msgs
        .iter()
        .any(|m| matches!(m, AgentMessage::UserText { text, .. } if text == "steer msg"));
    let has_followup = msgs
        .iter()
        .any(|m| matches!(m, AgentMessage::UserText { text, .. } if text == "followup msg"));

    assert!(!has_steer, "steer msg should have been cleared");
    assert!(!has_followup, "followup msg should have been cleared");

    registry::unregister(api);
}

#[test]
fn thinking_level_sets_stream_options_for_reasoning_model() {
    let mut config = AgentConfig::new(reasoning_model("thinking-high"));
    config.thinking_level = ThinkingLevel::High;
    let options = stream_options_for_turn(
        &config.model,
        config.stream_options.clone().unwrap_or_default(),
        config.thinking_level,
    );
    assert!(options.thinking.as_ref().unwrap().enabled);
    assert_eq!(
        options.thinking.as_ref().unwrap().effort.as_deref(),
        Some("high")
    );
}

#[test]
fn thinking_level_is_omitted_for_non_reasoning_model() {
    let mut config = AgentConfig::new(non_reasoning_model("thinking-off"));
    config.thinking_level = ThinkingLevel::High;
    let options = stream_options_for_turn(
        &config.model,
        config.stream_options.clone().unwrap_or_default(),
        config.thinking_level,
    );
    assert!(options.thinking.is_none());
}

pub fn stream_options_for_turn(
    model: &Model,
    mut options: StreamOptions,
    thinking_level: ThinkingLevel,
) -> StreamOptions {
    if !model.reasoning {
        options.thinking = None;
        return options;
    }

    match thinking_level {
        ThinkingLevel::Off => {
            options.thinking = None;
        }
        _ => {
            let budget_tokens = match thinking_level {
                ThinkingLevel::Minimal => Some(1024u32),
                ThinkingLevel::Low => Some(2048u32),
                ThinkingLevel::Medium => Some(4096u32),
                ThinkingLevel::High => Some(8192u32),
                ThinkingLevel::XHigh => Some(16384u32),
                ThinkingLevel::Off => None,
            };
            options.thinking = Some(pi_ai::types::ThinkingConfig {
                enabled: true,
                budget_tokens,
                effort: Some(thinking_level.to_string()),
            });
        }
    }

    options
}
