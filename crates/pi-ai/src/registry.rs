use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, StopReason, StreamOptions,
};
use async_stream::stream;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

pub trait ApiProvider: Send + Sync {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream;
}

static REGISTRY: LazyLock<RwLock<HashMap<String, Arc<dyn ApiProvider>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn register(api: &str, provider: Arc<dyn ApiProvider>) {
    REGISTRY.write().unwrap().insert(api.to_string(), provider);
}

pub fn unregister(api: &str) {
    REGISTRY.write().unwrap().remove(api);
}

pub fn lookup(api: &str) -> Option<Arc<dyn ApiProvider>> {
    REGISTRY.read().unwrap().get(api).cloned()
}

/// Top-level entry point: resolves provider by model.api, injects env API key
/// if not provided, delegates to provider.stream(). Returns a stream that
/// immediately yields Error on unknown api.
pub fn stream_model(model: &Model, ctx: Context, mut opts: Option<StreamOptions>) -> EventStream {
    let api = model.api.clone();
    let provider = match lookup(&api) {
        Some(p) => p,
        None => {
            return Box::pin(stream! {
                let mut msg = AssistantMessage::empty("registry", "");
                msg.error_message = Some(format!("unknown provider api: {}", api));
                msg.stop_reason = StopReason::Error;
                yield AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message: msg,
                };
            });
        }
    };

    if let Some(ref mut o) = opts {
        if o.api_key.is_none() {
            o.api_key = crate::util::env_keys::env_api_key(&model.provider);
        }
    }

    provider.stream(model, ctx, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessage, ModelCost, ModelInput};
    use futures::StreamExt;
    use std::sync::Arc;

    struct DummyProvider;
    impl ApiProvider for DummyProvider {
        fn stream(
            &self,
            _model: &Model,
            _ctx: Context,
            _opts: Option<StreamOptions>,
        ) -> EventStream {
            Box::pin(stream! {
                let mut msg = AssistantMessage::empty("dummy", "dummy");
                msg.content.push(crate::types::ContentBlock::Text {
                    text: "dummy response".into(), text_signature: None,
                });
                yield AssistantMessageEvent::Done { reason: StopReason::Stop, message: msg };
            })
        }
    }

    #[tokio::test]
    async fn registry_register_and_lookup() {
        register("reg-test-api", Arc::new(DummyProvider));
        let found = lookup("reg-test-api");
        assert!(found.is_some());
        unregister("reg-test-api");
        assert!(lookup("reg-test-api").is_none());
    }

    #[tokio::test]
    async fn stream_model_unknown_api_returns_error() {
        let model = Model {
            id: "x".into(),
            name: "x".into(),
            api: "nonexistent".into(),
            provider: "none".into(),
            base_url: "".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        };
        let mut stream = stream_model(
            &model,
            Context {
                system_prompt: None,
                messages: vec![],
                tools: None,
            },
            None,
        );
        let event = stream.next().await.unwrap();
        assert!(matches!(event, AssistantMessageEvent::Error { .. }));
    }

    #[tokio::test]
    async fn stream_model_delegates_to_provider() {
        register("test-api", Arc::new(DummyProvider));
        let model = Model {
            id: "x".into(),
            name: "x".into(),
            api: "test-api".into(),
            provider: "test".into(),
            base_url: "".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        };
        let mut stream = stream_model(
            &model,
            Context {
                system_prompt: None,
                messages: vec![],
                tools: None,
            },
            None,
        );
        let event = stream.next().await.unwrap();
        assert!(matches!(event, AssistantMessageEvent::Done { .. }));
        unregister("test-api");
    }
}
