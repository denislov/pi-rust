use crate::coding_session::{CapabilityStatus, CodingAgentCapabilities};
use crate::protocol::rpc::state::RpcState;
use crate::protocol::rpc::state::RunningPrompt;
use crate::protocol::types::RpcSessionState;
use pi_agent_core::session::StoredAgentMessage;
use serde_json::Value;

impl RpcState {
    pub(super) fn session_state(&self) -> RpcSessionState {
        RpcSessionState {
            model: Some(self.model.clone()),
            thinking_level: self.thinking_level,
            is_streaming: self.is_streaming(),
            is_compacting: self.is_compacting,
            steering_mode: self.steering_mode,
            follow_up_mode: self.follow_up_mode,
            session_file: self
                .active_session_path
                .as_ref()
                .map(|path| path.display().to_string()),
            session_id: self
                .active_leaf_id
                .clone()
                .or_else(|| {
                    self.active_session_path
                        .as_ref()
                        .and_then(|path| path.file_stem())
                        .and_then(|stem| stem.to_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "in-memory".into()),
            session_name: self.session_name.clone(),
            auto_compaction_enabled: self.auto_compaction_enabled,
            message_count: self.messages.len(),
            pending_message_count: self.steering.len() + self.follow_up.len(),
            capabilities: self.capabilities().into(),
        }
    }

    pub(super) fn capabilities(&self) -> CodingAgentCapabilities {
        let mut capabilities =
            CodingAgentCapabilities::phase_3(self.is_streaming().then_some("prompt"));
        let coding_running = matches!(self.running, Some(RunningPrompt::Coding(_)));

        capabilities.abort = if coding_running {
            CapabilityStatus::Disabled {
                reason: "operation abort awaits CodingAgentSession operation handles".into(),
            }
        } else {
            CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            }
        };
        capabilities.steer = if coding_running {
            CapabilityStatus::Disabled {
                reason: "agent turn steering awaits AgentTurnFlow".into(),
            }
        } else {
            CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            }
        };
        capabilities.follow_up = if coding_running {
            CapabilityStatus::Disabled {
                reason: "follow-up controls await AgentTurnFlow".into(),
            }
        } else {
            CapabilityStatus::Available
        };

        capabilities
    }

    pub(super) fn session_stats(&self) -> Value {
        let mut user_messages = 0;
        let mut assistant_messages = 0;
        let mut tool_results = 0;
        for message in &self.messages {
            match message {
                StoredAgentMessage::User { .. } => user_messages += 1,
                StoredAgentMessage::Assistant { .. } => assistant_messages += 1,
                StoredAgentMessage::ToolResult { .. } => tool_results += 1,
                StoredAgentMessage::BashExecution { .. }
                | StoredAgentMessage::Custom { .. }
                | StoredAgentMessage::BranchSummary { .. } => user_messages += 1,
            }
        }
        let session_file = self
            .active_session_path
            .as_ref()
            .map(|path| Value::String(path.display().to_string()))
            .unwrap_or(Value::Null);
        let session_id = self
            .active_leaf_id
            .clone()
            .or_else(|| {
                self.active_session_path
                    .as_ref()
                    .and_then(|path| path.file_stem())
                    .and_then(|stem| stem.to_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "in-memory".into());

        serde_json::json!({
            "sessionFile": session_file,
            "sessionId": session_id,
            "userMessages": user_messages,
            "assistantMessages": assistant_messages,
            "toolCalls": 0,
            "toolResults": tool_results,
            "totalMessages": self.messages.len(),
            "tokens": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
            },
            "cost": 0.0
        })
    }

    pub(super) fn last_assistant_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find_map(|message| match message {
                StoredAgentMessage::Assistant { content, .. } => Some(
                    content
                        .iter()
                        .filter_map(|block| match block {
                            pi_ai::types::ContentBlock::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
    }
}
