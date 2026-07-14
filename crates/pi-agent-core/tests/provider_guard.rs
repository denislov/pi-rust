mod common;

use std::sync::Arc;

use async_stream::stream;
use pi_ai::registry::ApiProvider;
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};

struct GuardTestProvider;

impl ApiProvider for GuardTestProvider {
    fn stream(&self, _model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        Box::pin(stream! {
            let message = AssistantMessage::empty("guard", "guard response");
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

#[test]
fn provider_guard_registers_only_its_scoped_client() {
    let api = "pi-agent-core-provider-guard-drop-api";
    let guard = common::ProviderGuard::register(api, Arc::new(GuardTestProvider));
    assert!(guard.ai_client().lookup_provider(api).is_some());
    assert!(pi_ai::AiClient::new().lookup_provider(api).is_none());
}
