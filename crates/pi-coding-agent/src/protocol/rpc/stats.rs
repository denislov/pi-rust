use crate::coding_session::CodingAgentCapabilities;
use crate::plugins::PluginCapabilities;
use crate::protocol::rpc::state::RpcState;
use crate::protocol::rpc::state::RunningPrompt;
use crate::protocol::types::RpcCapabilities;
use crate::protocol::types::RpcSessionState;
use pi_agent_core::transcript::StoredAgentMessage;
use serde_json::Value;

impl RpcState {
    pub(super) fn session_state(&self) -> RpcSessionState {
        let projection = self.session_projection();

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
            session_id: projection.session_id,
            client_id: self.client_id.as_ref().map(|id| id.as_str().to_owned()),
            snapshot_sequence: projection.snapshot_sequence,
            capability_generation: projection.capability_generation,
            session_name: self.session_name.clone(),
            auto_compaction_enabled: self.auto_compaction_enabled,
            message_count: self.messages.len(),
            pending_message_count: projection.pending_message_count,
            capabilities: projection.capabilities,
        }
    }

    fn session_projection(&self) -> RpcSessionProjection {
        if let Some(session) = self.coding_session.as_ref() {
            let snapshot = session.ui_snapshot(self.client_drafts.clone());
            return RpcSessionProjection {
                session_id: snapshot.session.session_id,
                pending_message_count: snapshot.client_drafts.len(),
                capabilities: snapshot.capabilities.into(),
                snapshot_sequence: snapshot.cursor.last_event_sequence.get(),
                capability_generation: snapshot.cursor.capability_generation.get(),
            };
        }

        RpcSessionProjection {
            session_id: self.fallback_session_id(),
            pending_message_count: self.steering.len() + self.follow_up.len(),
            capabilities: self.capabilities().into(),
            snapshot_sequence: 0,
            capability_generation: 1,
        }
    }

    fn fallback_session_id(&self) -> String {
        self.active_leaf_id
            .clone()
            .or_else(|| {
                self.active_session_path
                    .as_ref()
                    .and_then(|path| path.file_stem())
                    .and_then(|stem| stem.to_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "in-memory".into())
    }

    pub(super) fn capabilities(&self) -> CodingAgentCapabilities {
        let plugin_capabilities = PluginCapabilities::new();
        let active_operation = self.running.as_ref().map(|running| match running {
            RunningPrompt::Coding(running) => running.operation_kind,
        });
        CodingAgentCapabilities::from_runtime_state(
            active_operation,
            &plugin_capabilities,
            self.active_session_path.is_some(),
        )
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

struct RpcSessionProjection {
    session_id: String,
    pending_message_count: usize,
    capabilities: RpcCapabilities,
    snapshot_sequence: u64,
    capability_generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliRunOptions;
    use crate::coding_session::{
        ClientConnectionId, ClientDraft, ClientDraftKind, CodingAgentSession,
        CodingAgentSessionOptions,
    };

    fn serialized_session_state(state: &RpcState) -> Value {
        serde_json::to_value(state.session_state()).expect("session state serializes")
    }

    #[tokio::test]
    async fn rpc_state_includes_snapshot_cursor_and_client_id() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.coding_session = Some(session);
        state.client_id = Some(ClientConnectionId::new("rpc-primary"));

        let value = serialized_session_state(&state);

        assert_eq!(value["clientId"], "rpc-primary");
        assert!(value["snapshotSequence"].as_u64().is_some());
        assert_eq!(value["snapshotSequence"], 0);
        assert!(value["capabilityGeneration"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn rpc_pending_message_count_comes_from_client_drafts() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.coding_session = Some(session);
        state.client_drafts = vec![
            ClientDraft::new(ClientDraftKind::Prompt, "draft one"),
            ClientDraft::new(ClientDraftKind::FollowUp, "draft two"),
        ];
        state.steering = vec!["steer one".into(), "steer two".into()];
        state.follow_up = vec!["follow up".into()];

        let value = serialized_session_state(&state);

        assert_eq!(value["pendingMessageCount"], 2);
    }
}
