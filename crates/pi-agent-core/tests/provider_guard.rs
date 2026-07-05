mod common;

use std::sync::Arc;

use async_stream::stream;
use pi_ai::registry::{self, ApiProvider};
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
fn provider_guard_unregisters_provider_on_drop() {
    let api = "pi-agent-core-provider-guard-drop-api";
    registry::unregister(api);

    {
        let _guard = common::ProviderGuard::register(api, Arc::new(GuardTestProvider));
        assert!(registry::lookup(api).is_some());
    }

    assert!(registry::lookup(api).is_none());
}
