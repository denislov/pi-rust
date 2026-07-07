use crate::coding_session::CodingAgentEvent;
use crate::protocol::events::CodingProtocolEventAdapter;
use crate::protocol::types::ProtocolEvent;

pub(crate) struct RpcCodingEventAdapter {
    inner: CodingProtocolEventAdapter,
}

impl RpcCodingEventAdapter {
    pub(crate) fn new_with_provider(api: String, provider: String, model: String) -> Self {
        Self {
            inner: CodingProtocolEventAdapter::new_with_provider(api, provider, model),
        }
    }

    pub(crate) fn push(&mut self, event: &CodingAgentEvent) -> Vec<ProtocolEvent> {
        self.inner.push(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{CodingAgentEvent, CodingSessionError};
    use pi_agent_core::transcript::StoredAgentMessage;
    use pi_ai::types::{StopReason, Usage};

    fn adapter() -> RpcCodingEventAdapter {
        RpcCodingEventAdapter::new_with_provider(
            "faux".into(),
            "faux-provider".into(),
            "faux-model".into(),
        )
    }

    #[test]
    fn rpc_adapter_maps_product_prompt_stream_to_protocol_events() {
        let mut adapter = adapter();
        let mut events = Vec::new();

        for event in [
            CodingAgentEvent::AgentTurnStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                agent_turn: 1,
            },
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello".into(),
            },
            CodingAgentEvent::AssistantMessageCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                final_text: "hello".into(),
                usage: Usage::default(),
            },
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
            },
        ] {
            events.extend(adapter.push(&event));
        }

        assert!(matches!(events[0], ProtocolEvent::TurnStart));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProtocolEvent::MessageUpdate { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProtocolEvent::TurnEnd { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProtocolEvent::AgentEnd { .. }))
        );
    }

    #[test]
    fn rpc_adapter_maps_product_failure_to_protocol_error_message() {
        let mut adapter = adapter();
        let events = adapter.push(&CodingAgentEvent::PromptFailed {
            operation_id: "op_1".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        });

        assert!(matches!(
            &events[0],
            ProtocolEvent::MessageStart {
                message: StoredAgentMessage::Assistant {
                    provider,
                    stop_reason: StopReason::Error,
                    error_message: Some(error_message),
                    ..
                }
            } if provider == "faux-provider" && error_message == "provider error: provider failed"
        ));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProtocolEvent::AgentEnd { .. }))
        );
    }
}
