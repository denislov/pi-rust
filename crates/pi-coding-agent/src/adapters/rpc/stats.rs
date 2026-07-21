use crate::adapters::rpc::state::RpcState;
use crate::protocol::types::RpcCapabilities;
use crate::protocol::types::{RpcSessionNamePersistence, RpcSessionState};
use crate::protocol::types::{RpcSessionStats, RpcSessionTokenStats};
use crate::protocol::version::{ProtocolFamilyVersion, UI_SNAPSHOT_PROTOCOL_VERSION};
use crate::runtime::facade::{
    CodingAgentCapabilities, CodingAgentSessionTranscriptItem, CodingSessionError,
};
use pi_agent_core::api::transcript::StoredAgentMessage;
#[cfg(test)]
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
            event_stream_id: projection.event_stream_id,
            client_id: self
                .client_connection
                .as_ref()
                .map(|connection| connection.client_id.as_str().to_owned()),
            snapshot_sequence: projection.snapshot_sequence,
            capability_generation: projection.capability_generation,
            snapshot_version: projection.snapshot_version,
            negotiated_protocol: self.negotiated_protocol.clone(),
            session_name: self.session_name.clone(),
            session_name_persistence: RpcSessionNamePersistence::AdapterLocal,
            auto_compaction_enabled: self.auto_compaction_enabled,
            message_count: self.messages.len(),
            pending_message_count: projection.pending_message_count,
            pending_tool_authorizations: projection.pending_tool_authorizations,
            capabilities: projection.capabilities,
        }
    }

    fn session_projection(&self) -> RpcSessionProjection {
        if let Some(connection) = self.client_connection.as_ref()
            && let Ok(snapshot) = connection.state()
        {
            return RpcSessionProjection {
                session_id: snapshot.session.session_id,
                event_stream_id: Some(snapshot.cursor.stream_id),
                pending_message_count: snapshot.drafts.len(),
                pending_tool_authorizations: snapshot.pending_authorizations,
                capabilities: if self.is_streaming() {
                    CodingAgentCapabilities::for_session_write_operation(
                        self.foreground
                            .as_ref()
                            .map(|foreground| foreground.operation_kind),
                        self.active_session_path.is_some(),
                    )
                    .into()
                } else {
                    snapshot.capabilities.into()
                },
                snapshot_sequence: snapshot.cursor.last_event_sequence,
                capability_generation: snapshot.cursor.capability_generation,
                snapshot_version: snapshot.version,
            };
        }
        if let Some(session) = self.coding_session.as_ref() {
            let snapshot = session.snapshot();
            return RpcSessionProjection {
                session_id: snapshot.session.session_id,
                event_stream_id: Some(snapshot.cursor.stream_id),
                pending_message_count: snapshot.drafts.len(),
                pending_tool_authorizations: snapshot.pending_authorizations,
                capabilities: snapshot.capabilities.into(),
                snapshot_sequence: snapshot.cursor.last_event_sequence,
                capability_generation: snapshot.cursor.capability_generation,
                snapshot_version: snapshot.version,
            };
        }

        RpcSessionProjection {
            session_id: self.fallback_session_id(),
            event_stream_id: None,
            pending_message_count: self.steering.len() + self.follow_up.len(),
            pending_tool_authorizations: Vec::new(),
            capabilities: CodingAgentCapabilities::idle(self.active_session_path.is_some()).into(),
            snapshot_sequence: 0,
            capability_generation: 1,
            snapshot_version: UI_SNAPSHOT_PROTOCOL_VERSION,
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

    pub(super) fn session_stats(&self) -> Result<RpcSessionStats, CodingSessionError> {
        let session_file = self
            .active_session_path
            .as_ref()
            .map(|path| path.display().to_string());

        if let Some(hydration) = self
            .coding_session
            .as_ref()
            .map(|session| session.hydrate_current())
            .transpose()?
            .flatten()
        {
            let mut counts = RpcSessionMessageCounts::default();
            for item in &hydration.transcript {
                match item {
                    CodingAgentSessionTranscriptItem::User { .. } => counts.user += 1,
                    CodingAgentSessionTranscriptItem::Assistant { .. } => counts.assistant += 1,
                    CodingAgentSessionTranscriptItem::Tool { result, .. } => {
                        counts.tool_calls += 1;
                        counts.tool_results += usize::from(result.is_some());
                    }
                    CodingAgentSessionTranscriptItem::Delegation { .. }
                    | CodingAgentSessionTranscriptItem::CompactionSummary { .. }
                    | CodingAgentSessionTranscriptItem::BranchSummary { .. }
                    | CodingAgentSessionTranscriptItem::Diagnostic { .. } => {}
                }
            }
            let usage = hydration.usage;
            return Ok(RpcSessionStats {
                session_file,
                session_id: hydration.summary.session_id,
                active_leaf_id: hydration.summary.active_leaf_id,
                user_messages: counts.user,
                assistant_messages: counts.assistant,
                tool_calls: counts.tool_calls,
                tool_results: counts.tool_results,
                total_messages: counts.total_messages(),
                tokens: token_stats(
                    usage.input.into(),
                    usage.output.into(),
                    usage.cache_read.into(),
                    usage.cache_write.into(),
                ),
                cost: usage.cost,
                cost_known: usage.cost_known,
            });
        }

        let mut counts = RpcSessionMessageCounts::default();
        let mut input = 0_u64;
        let mut output = 0_u64;
        let mut cache_read = 0_u64;
        let mut cache_write = 0_u64;
        let mut cost = 0.0;
        let mut cost_known = true;
        for message in &self.messages {
            match message {
                StoredAgentMessage::User { .. } => counts.user += 1,
                StoredAgentMessage::Assistant { content, usage, .. } => {
                    counts.assistant += 1;
                    counts.tool_calls += content
                        .iter()
                        .filter(|block| {
                            matches!(
                                block,
                                pi_ai::api::conversation::ContentBlock::ToolCall { .. }
                            )
                        })
                        .count();
                    input += u64::from(usage.input);
                    output += u64::from(usage.output);
                    cache_read += u64::from(usage.cache_read);
                    cache_write += u64::from(usage.cache_write);
                    if usage.cost.known {
                        cost += usage.cost.input
                            + usage.cost.output
                            + usage.cost.cache_read
                            + usage.cost.cache_write;
                    } else {
                        cost_known = false;
                    }
                }
                StoredAgentMessage::ToolResult { .. } => counts.tool_results += 1,
                StoredAgentMessage::BashExecution { .. }
                | StoredAgentMessage::Custom { .. }
                | StoredAgentMessage::BranchSummary { .. } => {}
            }
        }

        Ok(RpcSessionStats {
            session_file,
            session_id: self.fallback_session_id(),
            active_leaf_id: self.active_leaf_id.clone(),
            user_messages: counts.user,
            assistant_messages: counts.assistant,
            tool_calls: counts.tool_calls,
            tool_results: counts.tool_results,
            total_messages: counts.total_messages(),
            tokens: token_stats(input, output, cache_read, cache_write),
            cost,
            cost_known,
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
                            pi_ai::api::conversation::ContentBlock::Text { text, .. } => {
                                Some(text.as_str())
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
    }
}

#[derive(Debug, Default)]
struct RpcSessionMessageCounts {
    user: usize,
    assistant: usize,
    tool_calls: usize,
    tool_results: usize,
}

impl RpcSessionMessageCounts {
    fn total_messages(&self) -> usize {
        self.user + self.assistant + self.tool_results
    }
}

fn token_stats(input: u64, output: u64, cache_read: u64, cache_write: u64) -> RpcSessionTokenStats {
    RpcSessionTokenStats {
        input,
        output,
        cache_read,
        cache_write,
        total: input
            .saturating_add(output)
            .saturating_add(cache_read)
            .saturating_add(cache_write),
    }
}

struct RpcSessionProjection {
    session_id: String,
    event_stream_id: Option<String>,
    pending_message_count: usize,
    pending_tool_authorizations: Vec<crate::authorization::ToolAuthorizationRequest>,
    capabilities: RpcCapabilities,
    snapshot_sequence: u64,
    capability_generation: u64,
    snapshot_version: ProtocolFamilyVersion,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::bootstrap::CliRunOptions;
    use crate::runtime::facade::{
        CodingAgentClientId, CodingAgentDraft, CodingAgentDraftId, CodingAgentDraftKind,
        CodingAgentSession, CodingAgentSessionOptions,
    };
    use pi_agent_core::api::transcript::{StoredUsage, StoredUsageCost};
    use pi_ai::api::conversation::{ContentBlock, StopReason};

    fn serialized_session_state(state: &RpcState) -> Value {
        serde_json::to_value(state.session_state()).expect("session state serializes")
    }

    #[test]
    fn non_persistent_stats_count_only_conversation_messages_and_sum_usage() {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.messages = vec![
            StoredAgentMessage::User {
                content: vec![],
                timestamp: 1,
            },
            StoredAgentMessage::Assistant {
                content: vec![ContentBlock::ToolCall {
                    id: "call_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({}),
                    thought_signature: None,
                }],
                api: "test".into(),
                provider: "test".into(),
                model: "test".into(),
                response_model: None,
                response_id: None,
                usage: StoredUsage {
                    input: 3,
                    output: 4,
                    cache_read: 1,
                    cache_write: 2,
                    total: 10,
                    cost: StoredUsageCost {
                        known: true,
                        input: 0.1,
                        output: 0.2,
                        cache_read: 0.3,
                        cache_write: 0.4,
                    },
                },
                stop_reason: StopReason::ToolUse,
                error_message: None,
                timestamp: 2,
            },
            StoredAgentMessage::ToolResult {
                tool_call_id: "call_1".into(),
                tool_name: "read".into(),
                content: vec![],
                is_error: false,
                timestamp: 3,
            },
            StoredAgentMessage::BashExecution {
                command: "pwd".into(),
                output: String::new(),
                exit_code: Some(0),
                cancelled: false,
                truncated: false,
                full_output_path: None,
                exclude_from_context: None,
                timestamp: 4,
            },
            StoredAgentMessage::Custom {
                custom_type: "note".into(),
                content: vec![],
                display: true,
                details: None,
                timestamp: 5,
            },
            StoredAgentMessage::BranchSummary {
                summary: "summary".into(),
                from_id: "entry_1".into(),
                timestamp: 6,
            },
        ];

        let stats = state.session_stats().unwrap();

        assert_eq!(stats.user_messages, 1);
        assert_eq!(stats.assistant_messages, 1);
        assert_eq!(stats.tool_calls, 1);
        assert_eq!(stats.tool_results, 1);
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.tokens.input, 3);
        assert_eq!(stats.tokens.output, 4);
        assert_eq!(stats.tokens.cache_read, 1);
        assert_eq!(stats.tokens.cache_write, 2);
        assert_eq!(stats.tokens.total, 10);
        assert!((stats.cost - 1.0).abs() < f64::EPSILON);
        assert!(stats.cost_known);
    }

    #[test]
    fn non_persistent_stats_mark_aggregate_cost_unknown() {
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.messages = vec![StoredAgentMessage::Assistant {
            content: Vec::new(),
            api: "test".into(),
            provider: "test".into(),
            model: "dynamic-price".into(),
            response_model: None,
            response_id: None,
            usage: StoredUsage {
                input: 3,
                output: 4,
                cache_read: 0,
                cache_write: 0,
                total: 7,
                cost: StoredUsageCost {
                    known: false,
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                },
            },
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 1,
        }];

        let stats = state.session_stats().unwrap();
        assert_eq!(stats.cost, 0.0);
        assert!(!stats.cost_known);
        let json = serde_json::to_value(stats).unwrap();
        assert_eq!(json["costKnown"], false);
    }

    #[tokio::test]
    async fn rpc_state_includes_snapshot_cursor_and_client_id() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let session_id = session.view().session_id;
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        state.client_connection = Some(
            session
                .connect(CodingAgentClientId::new("rpc-primary"))
                .unwrap(),
        );
        state.coding_session = Some(session);

        let value = serialized_session_state(&state);

        assert_eq!(value["sessionId"], session_id);
        assert!(value["eventStreamId"].as_str().is_some());
        assert_ne!(value["eventStreamId"], value["sessionId"]);
        assert_eq!(value["clientId"], "rpc-primary");
        assert!(value["snapshotSequence"].as_u64().is_some());
        assert_eq!(value["snapshotSequence"], 0);
        assert!(value["capabilityGeneration"].as_u64().unwrap() >= 1);
        assert_eq!(value["snapshotVersion"]["family"], "ui_snapshot");
        assert_eq!(value["snapshotVersion"]["major"], 2);
        assert_eq!(value["snapshotVersion"]["minor"], 2);
    }

    #[tokio::test]
    async fn rpc_pending_message_count_comes_from_client_drafts() {
        let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut state = RpcState::new(CliRunOptions::default()).unwrap();
        let connection = session
            .connect(CodingAgentClientId::new("rpc-primary"))
            .unwrap();
        connection
            .set_prompt_draft(CodingAgentDraftId("draft-one".into()), "draft one")
            .unwrap();
        connection
            .enqueue_control_draft(CodingAgentDraft {
                id: CodingAgentDraftId("draft-two".into()),
                kind: CodingAgentDraftKind::FollowUp,
                text: "draft two".into(),
            })
            .unwrap();
        state.client_connection = Some(connection);
        state.coding_session = Some(session);
        state.steering = vec![
            crate::operations::prompt::context::QueuedPromptInput::Text("steer one".into()),
            crate::operations::prompt::context::QueuedPromptInput::Text("steer two".into()),
        ];
        state.follow_up = vec![crate::operations::prompt::context::QueuedPromptInput::Text(
            "follow up".into(),
        )];

        let value = serialized_session_state(&state);

        assert_eq!(value["pendingMessageCount"], 2);
    }
}
