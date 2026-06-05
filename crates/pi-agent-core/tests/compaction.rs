use pi_agent_core::compaction::estimate::estimate_tokens;
use pi_agent_core::compaction::prepare::{prepare_compaction, should_compact};
use pi_agent_core::{AgentMessage, CompactionSettings};
use pi_ai::types::ContentBlock;

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
    let (to_summarize, keep) = prepare_compaction(&msgs, &settings);
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
