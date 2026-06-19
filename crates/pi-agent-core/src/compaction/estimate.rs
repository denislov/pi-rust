use crate::types::AgentMessage;
use pi_ai::types::ContentBlock;

pub fn estimate_tokens(messages: &[AgentMessage]) -> u32 {
    let mut total: u32 = 0;

    for msg in messages {
        match msg {
            AgentMessage::UserText { text, .. } => {
                total += (text.len() as u32) / 4;
            }
            AgentMessage::Assistant { message, .. } => {
                if message.usage.total_tokens > 0 {
                    total += message.usage.total_tokens;
                    continue;
                }
                for block in &message.content {
                    total += estimate_block_tokens(block);
                }
            }
            AgentMessage::ToolResult { content, .. } => {
                for block in content {
                    total += estimate_block_tokens(block);
                }
            }
            AgentMessage::SystemPrompt { text, .. } => {
                total += (text.len() as u32) / 4;
            }
            AgentMessage::CompactionSummary { summary, .. } => {
                total += (summary.len() as u32) / 4;
            }
            AgentMessage::BashExecution {
                command,
                output,
                exclude_from_context,
                ..
            } => {
                if !exclude_from_context {
                    total += (command.len() as u32) / 4 + (output.len() as u32) / 4;
                }
            }
            AgentMessage::Custom { content, .. } => {
                for block in content {
                    total += estimate_block_tokens(block);
                }
            }
            AgentMessage::BranchSummary { summary, .. } => {
                total += (summary.len() as u32) / 4;
            }
        }
    }

    total
}

fn estimate_block_tokens(block: &ContentBlock) -> u32 {
    match block {
        ContentBlock::Text { text, .. } => (text.len() as u32) / 4,
        ContentBlock::ToolCall {
            name, arguments, ..
        } => (name.len() as u32) / 4 + (arguments.to_string().len() as u32) / 4,
        ContentBlock::Thinking { thinking, .. } => (thinking.len() as u32) / 4,
        ContentBlock::Image { .. } => 4800 / 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_text_from_chars() {
        let msgs = vec![AgentMessage::UserText {
            message_id: "1".into(),
            text: "hello world this is a test".into(),
        }];
        let tokens = estimate_tokens(&msgs);
        assert!(tokens > 0);
    }

    #[test]
    fn uses_assistant_usage_when_available() {
        use pi_ai::types::AssistantMessage;
        let mut msg = AssistantMessage::empty("test", "test-model");
        msg.usage.total_tokens = 42;
        let msgs = vec![AgentMessage::Assistant {
            message_id: "2".into(),
            message: msg,
        }];
        let tokens = estimate_tokens(&msgs);
        assert_eq!(tokens, 42);
    }
}
