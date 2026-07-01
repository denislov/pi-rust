#[derive(Debug, Default)]
pub(crate) struct RuntimeService;

use pi_agent_core::{Agent, AgentMessage};
use pi_ai::types::{AssistantMessage, ContentBlock, StopReason};

use crate::runtime::{SessionMode, build_agent_config};

use super::CodingSessionError;
use super::prompt::RuntimeSnapshot;
use super::session_log::event::PersistedContentBlock;
use super::session_log::replay::{MessageStatus, SessionReplay, ToolCallStatus, TranscriptItem};

impl RuntimeService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn build_agent_runtime(
        &self,
        runtime: &RuntimeSnapshot,
    ) -> Result<Agent, CodingSessionError> {
        if runtime.register_builtins() {
            pi_ai::providers::register_builtins();
        }

        let mut config = build_agent_config(
            runtime.model().clone(),
            runtime.system_prompt().map(str::to_owned),
            runtime.max_turns(),
            runtime.api_key().map(str::to_owned),
            runtime.thinking_level(),
            runtime.tool_execution(),
            runtime.resources().clone(),
            runtime.settings(),
        );
        if matches!(
            runtime
                .session_run_options()
                .map(|session_options| &session_options.mode),
            Some(SessionMode::Enabled)
        ) && runtime.settings().is_none()
        {
            config.compaction = Some(pi_agent_core::CompactionConfig::default());
        }

        let agent = Agent::new(config);
        for tool in runtime.tools() {
            agent.add_tool(tool.clone());
        }
        Ok(agent)
    }

    pub(crate) fn hydrate_agent_runtime(
        &self,
        agent: &Agent,
        runtime: &RuntimeSnapshot,
        replay: &SessionReplay,
    ) {
        let mut pending_assistant: Option<(String, AssistantMessage)> = None;
        let mut pending_tool_results = Vec::new();

        for (index, item) in replay.transcript.iter().enumerate() {
            match item {
                TranscriptItem::UserInput { text, .. } if !text.is_empty() => {
                    flush_replay_hydration_group(
                        agent,
                        &mut pending_assistant,
                        &mut pending_tool_results,
                    );
                    agent.add_message(AgentMessage::UserText {
                        message_id: format!("replay_user_{index}"),
                        text: text.clone(),
                    });
                }
                TranscriptItem::UserInput { .. } => {}
                TranscriptItem::AssistantMessage {
                    message_id,
                    content,
                    status: MessageStatus::Completed,
                } => {
                    flush_replay_hydration_group(
                        agent,
                        &mut pending_assistant,
                        &mut pending_tool_results,
                    );
                    let mut message = replay_assistant_message(runtime);
                    message.content = replay_content_blocks(content);
                    pending_assistant = Some((message_id.clone(), message));
                }
                TranscriptItem::ToolCall {
                    tool_call_id,
                    name,
                    arguments,
                    status: status @ (ToolCallStatus::Completed | ToolCallStatus::Failed),
                    summary,
                } => {
                    pending_replay_assistant_message(&mut pending_assistant, runtime, index)
                        .content
                        .push(ContentBlock::ToolCall {
                            id: tool_call_id.clone(),
                            name: name.clone(),
                            arguments: arguments.clone(),
                            thought_signature: None,
                        });
                    pending_tool_results.push(AgentMessage::ToolResult {
                        message_id: format!("replay_tool_result_{index}"),
                        tool_call_id: tool_call_id.clone(),
                        tool_name: name.clone(),
                        is_error: matches!(status, ToolCallStatus::Failed),
                        content: vec![ContentBlock::Text {
                            text: summary.clone(),
                            text_signature: None,
                        }],
                    });
                }
                TranscriptItem::ToolCall { .. } => {}
                TranscriptItem::CompactionSummary {
                    summary,
                    tokens_before,
                    ..
                } => {
                    flush_replay_hydration_group(
                        agent,
                        &mut pending_assistant,
                        &mut pending_tool_results,
                    );
                    agent.add_message(AgentMessage::CompactionSummary {
                        message_id: format!("replay_compaction_{index}"),
                        summary: summary.clone(),
                        tokens_before: *tokens_before,
                    });
                }
                TranscriptItem::Diagnostic { .. } => {
                    flush_replay_hydration_group(
                        agent,
                        &mut pending_assistant,
                        &mut pending_tool_results,
                    );
                }
                TranscriptItem::AssistantMessage { .. } => {}
            }
        }

        flush_replay_hydration_group(agent, &mut pending_assistant, &mut pending_tool_results);
    }
}

fn replay_assistant_message(runtime: &RuntimeSnapshot) -> AssistantMessage {
    let mut message = AssistantMessage::empty(&runtime.model().api, &runtime.model().id);
    message.provider = Some(runtime.model().provider.clone());
    message.stop_reason = StopReason::Stop;
    message
}

fn replay_content_blocks(content: &[PersistedContentBlock]) -> Vec<ContentBlock> {
    content
        .iter()
        .map(|block| match block {
            PersistedContentBlock::Text { text } => ContentBlock::Text {
                text: text.clone(),
                text_signature: None,
            },
            PersistedContentBlock::Thinking {
                thinking,
                thinking_signature,
                redacted,
            } => ContentBlock::Thinking {
                thinking: thinking.clone(),
                thinking_signature: thinking_signature.clone(),
                redacted: *redacted,
            },
            PersistedContentBlock::Image { mime_type, data } => ContentBlock::Image {
                mime_type: mime_type.clone(),
                data: data.clone(),
            },
        })
        .collect()
}

fn pending_replay_assistant_message<'a>(
    pending_assistant: &'a mut Option<(String, AssistantMessage)>,
    runtime: &RuntimeSnapshot,
    index: usize,
) -> &'a mut AssistantMessage {
    if pending_assistant.is_none() {
        *pending_assistant = Some((
            format!("replay_assistant_tool_{index}"),
            replay_assistant_message(runtime),
        ));
    }
    &mut pending_assistant.as_mut().expect("pending assistant set").1
}

fn flush_replay_hydration_group(
    agent: &Agent,
    pending_assistant: &mut Option<(String, AssistantMessage)>,
    pending_tool_results: &mut Vec<AgentMessage>,
) {
    if let Some((message_id, message)) = pending_assistant.take() {
        agent.add_message(AgentMessage::Assistant {
            message_id,
            message,
        });
    }
    for message in pending_tool_results.drain(..) {
        agent.add_message(message);
    }
}

#[cfg(test)]
mod tests {
    use pi_agent_core::{AgentResources, ToolExecutionMode};
    use pi_ai::types::{Model, ModelCost, ModelInput};

    use super::*;
    use crate::coding_session::session_log::event::DiagnosticLevel;
    use crate::coding_session::session_log::replay::{
        MessageStatus, ReplayDiagnostic, SessionReplay, ToolCallStatus, TranscriptItem,
    };
    use crate::prompt_options::PromptRunOptions;
    use crate::runtime::{PromptInvocation, SessionRunOptions};

    fn model(api: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    fn runtime_snapshot(api: &str) -> RuntimeSnapshot {
        RuntimeSnapshot::from_prompt_run_options(PromptRunOptions {
            prompt: "hello".into(),
            model: model(api),
            api_key: Some("key".into()),
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools: Vec::new(),
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: Some(ToolExecutionMode::Sequential),
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text("hello".into()),
        })
    }

    #[test]
    fn builds_agent_from_runtime_snapshot() {
        let service = RuntimeService::new();

        let agent = service
            .build_agent_runtime(&runtime_snapshot("runtime-service-build"))
            .unwrap();

        let (context, stream_options) = agent.provider_request_snapshot();
        assert_eq!(context.system_prompt.as_deref(), Some("system"));
        assert_eq!(
            stream_options.and_then(|options| options.api_key),
            Some("key".into())
        );
    }

    #[test]
    fn hydrates_agent_runtime_from_completed_replay_items() {
        let service = RuntimeService::new();
        let runtime = runtime_snapshot("runtime-service-hydrate");
        let agent = service.build_agent_runtime(&runtime).unwrap();
        let replay = SessionReplay {
            session_id: "sess_replay".into(),
            cwd: None,
            active_leaf_id: None,
            transcript: vec![
                TranscriptItem::CompactionSummary {
                    summary: "summary of earlier work".into(),
                    first_kept_message_id: "turn_1".into(),
                    tokens_before: 1200,
                },
                TranscriptItem::UserInput {
                    turn_id: "turn_1".into(),
                    text: "previous question".into(),
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_1".into(),
                    content: vec![
                        PersistedContentBlock::Thinking {
                            thinking: "previous thought".into(),
                            thinking_signature: None,
                            redacted: None,
                        },
                        PersistedContentBlock::Text {
                            text: "previous answer".into(),
                        },
                    ],
                    status: MessageStatus::Completed,
                },
                TranscriptItem::ToolCall {
                    tool_call_id: "tool_1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                    status: ToolCallStatus::Completed,
                    summary: "tool output".into(),
                },
                TranscriptItem::AssistantMessage {
                    message_id: "msg_cancelled".into(),
                    content: vec![PersistedContentBlock::Text {
                        text: "cancelled answer".into(),
                    }],
                    status: MessageStatus::Cancelled,
                },
                TranscriptItem::Diagnostic {
                    level: DiagnosticLevel::Warn,
                    message: "ignored".into(),
                },
            ],
            diagnostics: vec![ReplayDiagnostic {
                level: DiagnosticLevel::Warn,
                message: "ignored".into(),
            }],
        };

        service.hydrate_agent_runtime(&agent, &runtime, &replay);

        let messages = agent.messages();
        assert_eq!(messages.len(), 4);
        assert!(matches!(
            &messages[0],
            AgentMessage::CompactionSummary {
                summary,
                tokens_before,
                ..
            } if summary == "summary of earlier work" && *tokens_before == 1200
        ));
        assert!(matches!(
            &messages[1],
            AgentMessage::UserText { text, .. } if text == "previous question"
        ));
        assert!(matches!(
            &messages[2],
            AgentMessage::Assistant { message_id, message }
                if message_id == "msg_1"
                    && message.content == vec![ContentBlock::Thinking {
                        thinking: "previous thought".into(),
                        thinking_signature: None,
                        redacted: None,
                    }, ContentBlock::Text {
                        text: "previous answer".into(),
                        text_signature: None,
                    }, ContentBlock::ToolCall {
                        id: "tool_1".into(),
                        name: "read".into(),
                        arguments: serde_json::json!({"path": "src/lib.rs"}),
                        thought_signature: None,
                }]
        ));
        assert!(matches!(
            &messages[3],
            AgentMessage::ToolResult {
                message_id,
                tool_call_id,
                tool_name,
                is_error,
                content,
            } if message_id == "replay_tool_result_3"
                && tool_call_id == "tool_1"
                && tool_name == "read"
                && !is_error
                && *content == vec![ContentBlock::Text {
                    text: "tool output".into(),
                    text_signature: None,
                }]
        ));
    }
}
