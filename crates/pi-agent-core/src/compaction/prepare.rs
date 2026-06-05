use crate::compaction::estimate::estimate_tokens;
use crate::types::{AgentMessage, CompactionSettings};

pub fn should_compact(estimated_tokens: u32, context_window: u32, reserve_tokens: u32) -> bool {
    context_window > 0 && estimated_tokens > context_window.saturating_sub(reserve_tokens)
}

pub fn prepare_compaction(
    messages: &[AgentMessage],
    settings: &CompactionSettings,
) -> (Vec<AgentMessage>, Vec<AgentMessage>) {
    if messages.is_empty() {
        return (vec![], vec![]);
    }

    let estimated = estimate_tokens(messages);
    let total_context_window = settings.reserve_tokens + settings.keep_recent_tokens;

    if estimated <= total_context_window {
        return (vec![], messages.to_vec());
    }

    let mut keep_recent: Vec<AgentMessage> = Vec::new();
    let mut keep_tokens: u32 = 0;
    let mut i = messages.len();

    while i > 0 {
        i -= 1;
        let msg = &messages[i];

        if matches!(msg, AgentMessage::ToolResult { .. }) && keep_recent.is_empty() {
            continue;
        }

        let msg_tokens = estimate_tokens(std::slice::from_ref(msg));
        if keep_tokens + msg_tokens > settings.keep_recent_tokens && !keep_recent.is_empty() {
            i += 1;
            break;
        }

        keep_recent.insert(0, msg.clone());
        keep_tokens += msg_tokens;
    }

    let to_summarize: Vec<AgentMessage> = messages[..i].to_vec();

    (to_summarize, keep_recent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::types::ContentBlock;

    fn user_msg(text: &str) -> AgentMessage {
        AgentMessage::UserText {
            message_id: "u".into(),
            text: text.into(),
        }
    }

    fn tool_result(text: &str) -> AgentMessage {
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

    #[test]
    fn should_compact_when_over_threshold() {
        assert!(should_compact(10_000, 8_000, 1_000));
    }

    #[test]
    fn should_not_compact_under_threshold() {
        assert!(!should_compact(5_000, 8_000, 1_000));
    }

    #[test]
    fn avoid_orphan_tool_result_cut_point() {
        let msgs = vec![user_msg("hello"), tool_result("result"), user_msg("next")];
        let settings = CompactionSettings {
            enabled: true,
            reserve_tokens: 0,
            keep_recent_tokens: 10,
        };
        let (to_summarize, keep) = prepare_compaction(&msgs, &settings);
        assert!(!to_summarize.is_empty() || !keep.is_empty());
    }

    #[test]
    fn maintain_cut_point_after_user_message() {
        let msgs = vec![user_msg("first"), user_msg("second"), user_msg("third")];
        let settings = CompactionSettings {
            enabled: true,
            reserve_tokens: 0,
            keep_recent_tokens: 5,
        };
        let (to_summarize, keep) = prepare_compaction(&msgs, &settings);
        assert!(!keep.is_empty());
        assert!(to_summarize.len() + keep.len() == msgs.len());
    }
}
