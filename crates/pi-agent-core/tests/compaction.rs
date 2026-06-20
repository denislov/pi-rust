mod common;

use common::faux_model;
use futures::StreamExt;
use pi_agent_core::compaction::estimate::estimate_tokens;
use pi_agent_core::compaction::prepare::{prepare_compaction, should_compact};
use pi_agent_core::{
    Agent, AgentConfig, AgentEvent, AgentMessage, CompactionConfig, CompactionSettings,
};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use pi_ai::types::{ContentBlock, StopReason};
use std::sync::Arc;

fn user_msg(text: &str) -> AgentMessage {
    AgentMessage::UserText {
        message_id: "u".into(),
        text: text.into(),
    }
}

fn tool_msg(text: &str) -> AgentMessage {
    AgentMessage::ToolResult {
        message_id: "t".into(),
        tool_call_id: "call".into(),
        tool_name: "test".into(),
        is_error: false,
        content: vec![ContentBlock::Text {
            text: text.into(),
            text_signature: None,
        }],
    }
}

fn compaction_msg(summary: &str, tokens: u32) -> AgentMessage {
    AgentMessage::CompactionSummary {
        message_id: "cs".into(),
        summary: summary.into(),
        tokens_before: tokens,
    }
}

#[test]
fn estimate_text_tokens() {
    let msgs = vec![user_msg("hello world this is about twenty five")];
    let tokens = estimate_tokens(&msgs);
    assert!(tokens > 5, "should be >5 tokens, got {}", tokens);
}

#[test]
fn estimate_tokens_accumulates_assistant_usage_with_other_messages() {
    let mut assistant = pi_ai::types::AssistantMessage::empty("test", "test-model");
    assistant.usage.total_tokens = 30;
    let msgs = vec![
        user_msg(&"old context ".repeat(100)),
        AgentMessage::Assistant {
            message_id: "a".into(),
            message: assistant,
        },
    ];

    assert!(estimate_tokens(&msgs) > 30);
}

#[test]
fn should_compact_applies_threshold() {
    assert!(should_compact(10_000, 8_000, 1_000));
    assert!(!should_compact(5_000, 8_000, 1_000));
    assert!(!should_compact(100, 0, 0));
}

#[test]
fn prepare_compaction_keeps_recent_messages() {
    let msgs = vec![
        user_msg("first message"),
        user_msg("second message"),
        user_msg("third message"),
        user_msg("fourth message"),
    ];
    let settings = CompactionSettings {
        enabled: true,
        reserve_tokens: 0,
        keep_recent_tokens: 5,
    };
    let (to_summarize, keep) = prepare_compaction(&msgs, &settings);
    assert!(!keep.is_empty(), "should keep some recent messages");
    assert!(
        to_summarize.len() + keep.len() == msgs.len(),
        "all messages accounted for: {} + {} != {}",
        to_summarize.len(),
        keep.len(),
        msgs.len()
    );
}

#[test]
fn prepare_compaction_no_split_for_small_conversation() {
    let msgs = vec![user_msg("hi")];
    let settings = CompactionSettings {
        enabled: true,
        reserve_tokens: 16_384,
        keep_recent_tokens: 20_000,
    };
    let (to_summarize, keep) = prepare_compaction(&msgs, &settings);
    assert!(to_summarize.is_empty());
    assert_eq!(keep.len(), 1);
}

#[test]
fn prepare_compaction_handles_compaction_summary() {
    let msgs = vec![
        compaction_msg("previous summary", 100),
        user_msg("hello after compaction"),
    ];
    let settings = CompactionSettings {
        enabled: true,
        reserve_tokens: 0,
        keep_recent_tokens: 10,
    };
    let (_to_summarize, keep) = prepare_compaction(&msgs, &settings);
    // Should handle gracefully
    assert!(!keep.is_empty());
}

#[test]
fn empty_messages_no_split() {
    let settings = CompactionSettings::default();
    let (to_summarize, keep) = prepare_compaction(&[], &settings);
    assert!(to_summarize.is_empty());
    assert!(keep.is_empty());
}

#[tokio::test]
async fn runtime_compaction_summarizes_before_provider_request() {
    let api = "runtime-compaction";
    let mut config = AgentConfig::new(faux_model(api));
    config.max_turns = 3;
    config.compaction = Some(CompactionConfig {
        settings: CompactionSettings {
            enabled: true,
            reserve_tokens: 0,
            keep_recent_tokens: 8,
        },
        custom_instructions: None,
    });

    let agent = Agent::new(config);
    agent.add_message(user_msg(&"old context ".repeat(40)));
    agent.add_message(user_msg(&"more old context ".repeat(40)));

    registry::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("summary of old context", StopReason::Stop),
            FauxProvider::text_call("final answer", StopReason::Stop),
        ])),
    );

    let events: Vec<_> = agent.prompt("new prompt").collect().await;

    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::SessionCompacted { summary, .. } if summary == "summary of old context"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::AgentDone { message }
            if message.content.iter().any(|block| matches!(
                block,
                ContentBlock::Text { text, .. } if text == "final answer"
            ))
    )));
    assert!(agent.messages().iter().any(|message| matches!(
        message,
        AgentMessage::CompactionSummary { summary, .. } if summary == "summary of old context"
    )));

    registry::unregister(api);
}
