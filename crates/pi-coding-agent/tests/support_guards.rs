mod support;

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
    let api = "support-provider-guard-drop-api";
    registry::unregister(api);

    {
        let _guard = support::ProviderGuard::register(api, Arc::new(GuardTestProvider));
        assert!(registry::lookup(api).is_some());
    }

    assert!(registry::lookup(api).is_none());
}

#[test]
fn provider_guard_register_many_unregisters_all_providers_on_drop() {
    let first_api = "support-provider-guard-many-first";
    let second_api = "support-provider-guard-many-second";
    registry::unregister(first_api);
    registry::unregister(second_api);

    {
        let providers: Vec<(String, Arc<dyn ApiProvider>)> = vec![
            (first_api.to_string(), Arc::new(GuardTestProvider)),
            (second_api.to_string(), Arc::new(GuardTestProvider)),
        ];
        let _guard = support::ProviderGuard::register_many(providers);
        assert!(registry::lookup(first_api).is_some());
        assert!(registry::lookup(second_api).is_some());
    }

    assert!(registry::lookup(first_api).is_none());
    assert!(registry::lookup(second_api).is_none());
}
