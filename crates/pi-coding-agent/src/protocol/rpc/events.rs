use crate::coding_session::ProductEvent;
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

    pub(crate) fn push_product_event(&mut self, event: &ProductEvent) -> Vec<ProtocolEvent> {
        self.inner.push_product_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{
        CodingAgentEvent, CodingAgentProductEventKind, CodingAgentWorkflowProductEvent,
        CodingSessionError, ProductEvent, ProductEventSequence,
    };
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

        for (index, event) in [
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
        ]
        .into_iter()
        .enumerate()
        {
            let product_event =
                ProductEvent::from_compat_event(ProductEventSequence(index as u64 + 1), event);
            events.extend(adapter.push_product_event(&product_event));
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
    fn rpc_adapter_accepts_internal_product_events() {
        let mut adapter = adapter();
        let product_event = ProductEvent::from_compat_event(
            ProductEventSequence(1),
            CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "provider failed".into(),
                },
            },
        );
        assert!(matches!(
            product_event.event(),
            CodingAgentProductEventKind::Workflow(CodingAgentWorkflowProductEvent::PromptFailed {
                operation_id,
                error,
            }) if operation_id == "op_1"
                && error.code == "provider"
                && error.message == "provider error: provider failed"
        ));

        let events = adapter.push_product_event(&product_event);

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

    #[test]
    fn rpc_adapter_maps_product_failure_to_protocol_error_message() {
        let mut adapter = adapter();
        let product_event = ProductEvent::from_compat_event(
            ProductEventSequence(1),
            CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "provider failed".into(),
                },
            },
        );
        let events = adapter.push_product_event(&product_event);

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
