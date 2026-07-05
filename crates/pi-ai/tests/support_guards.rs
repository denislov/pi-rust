mod support;

use std::sync::Arc;

use async_stream::stream;
use pi_ai::registry::{self, ApiProvider};
use pi_ai::stream::EventStream;
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};

struct GuardTestProvider(&'static str);

impl ApiProvider for GuardTestProvider {
    fn stream(&self, _model: &Model, _ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
        let text = self.0.to_string();
        Box::pin(stream! {
            let mut message = AssistantMessage::empty("guard", "guard-model");
            message.content.push(pi_ai::types::ContentBlock::Text {
                text,
                text_signature: None,
            });
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

#[test]
fn provider_guard_unregisters_new_provider_on_drop() {
    let api = "pi-ai-provider-guard-drop-api";
    registry::unregister(api);

    {
        let _guard = support::ProviderGuard::register(api, Arc::new(GuardTestProvider("temp")));
        assert!(registry::lookup(api).is_some());
    }

    assert!(registry::lookup(api).is_none());
}

#[test]
fn provider_guard_clear_removes_provider_until_drop_then_restores_it() {
    let api = "pi-ai-provider-guard-clear-api";
    registry::unregister(api);
    registry::register(api, Arc::new(GuardTestProvider("original")));
    let original = registry::lookup(api).expect("original provider registered");

    {
        let _guard = support::ProviderGuard::clear(api);
        assert!(registry::lookup(api).is_none());
    }

    let restored = registry::lookup(api).expect("original provider restored");
    assert!(Arc::ptr_eq(&original, &restored));
    registry::unregister(api);
}

#[test]
fn provider_guard_restores_existing_provider_on_drop() {
    let api = "pi-ai-provider-guard-restore-api";
    registry::unregister(api);
    registry::register(api, Arc::new(GuardTestProvider("original")));
    let original = registry::lookup(api).expect("original provider registered");

    {
        let _guard = support::ProviderGuard::register(api, Arc::new(GuardTestProvider("temp")));
        assert!(registry::lookup(api).is_some());
    }

    let restored = registry::lookup(api).expect("original provider restored");
    assert!(Arc::ptr_eq(&original, &restored));
    registry::unregister(api);
}
