use crate::providers;
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

pub trait ProviderAuthResolver: Send + Sync {
    fn resolve_api_key(&self, provider: &str) -> Option<String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnvProviderAuthResolver;

impl ProviderAuthResolver for EnvProviderAuthResolver {
    fn resolve_api_key(&self, provider: &str) -> Option<String> {
        crate::util::env_keys::env_api_key(provider)
    }
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn ApiProvider>>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, api: impl Into<String>, provider: Arc<dyn ApiProvider>) {
        self.providers.write().unwrap().insert(api.into(), provider);
    }

    pub fn unregister(&self, api: &str) {
        self.providers.write().unwrap().remove(api);
    }

    pub fn lookup(&self, api: &str) -> Option<Arc<dyn ApiProvider>> {
        self.providers.read().unwrap().get(api).cloned()
    }

    pub fn registered_apis(&self) -> Vec<String> {
        let mut apis = self
            .providers
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        apis.sort();
        apis
    }

    pub fn stream_model(
        &self,
        model: &Model,
        ctx: Context,
        opts: Option<StreamOptions>,
    ) -> EventStream {
        self.stream_model_with_auth(model, ctx, opts, &EnvProviderAuthResolver)
    }

    pub fn stream_model_with_auth(
        &self,
        model: &Model,
        ctx: Context,
        mut opts: Option<StreamOptions>,
        auth_resolver: &dyn ProviderAuthResolver,
    ) -> EventStream {
        let api = model.api.clone();
        let provider = match self.lookup(&api) {
            Some(p) => p,
            None => return unknown_provider_stream(api),
        };

        match opts.as_mut() {
            Some(options) if options.api_key.is_none() => {
                options.api_key = auth_resolver.resolve_api_key(&model.provider);
            }
            None => {
                if let Some(api_key) = auth_resolver.resolve_api_key(&model.provider) {
                    opts = Some(StreamOptions {
                        api_key: Some(api_key),
                        ..StreamOptions::default()
                    });
                }
            }
            _ => {}
        }

        provider.stream(model, ctx, opts)
    }
}

#[derive(Clone)]
pub struct AiClient {
    registry: ProviderRegistry,
    auth_resolver: Arc<dyn ProviderAuthResolver>,
}

impl Default for AiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AiClient {
    pub fn new() -> Self {
        Self::with_auth_resolver(Arc::new(EnvProviderAuthResolver))
    }

    pub fn with_auth_resolver(auth_resolver: Arc<dyn ProviderAuthResolver>) -> Self {
        Self {
            registry: ProviderRegistry::new(),
            auth_resolver,
        }
    }

    pub fn with_registry(
        registry: ProviderRegistry,
        auth_resolver: Arc<dyn ProviderAuthResolver>,
    ) -> Self {
        Self {
            registry,
            auth_resolver,
        }
    }

    pub fn provider_registry(&self) -> ProviderRegistry {
        self.registry.clone()
    }

    pub fn register_provider(&self, api: impl Into<String>, provider: Arc<dyn ApiProvider>) {
        self.registry.register(api, provider);
    }

    pub fn register_builtins(&self) {
        providers::register_builtins_into(&self.registry);
    }

    pub fn unregister_provider(&self, api: &str) {
        self.registry.unregister(api);
    }

    pub fn lookup_provider(&self, api: &str) -> Option<Arc<dyn ApiProvider>> {
        self.registry.lookup(api)
    }

    pub fn stream_model(
        &self,
        model: &Model,
        ctx: Context,
        opts: Option<StreamOptions>,
    ) -> EventStream {
        self.registry
            .stream_model_with_auth(model, ctx, opts, self.auth_resolver.as_ref())
    }
}

static REGISTRY: LazyLock<ProviderRegistry> = LazyLock::new(ProviderRegistry::new);

pub fn register(api: &str, provider: Arc<dyn ApiProvider>) {
    REGISTRY.register(api, provider);
}

pub fn unregister(api: &str) {
    REGISTRY.unregister(api);
}

pub fn lookup(api: &str) -> Option<Arc<dyn ApiProvider>> {
    REGISTRY.lookup(api)
}

/// Top-level entry point: resolves provider by model.api, injects env API key
/// if not provided, delegates to provider.stream(). Returns a stream that
/// immediately yields Error on unknown api.
pub fn stream_model(model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream {
    REGISTRY.stream_model(model, ctx, opts)
}

fn unknown_provider_stream(api: String) -> EventStream {
    Box::pin(stream! {
        let mut msg = AssistantMessage::empty("registry", "");
        msg.error_message = Some(format!("unknown provider api: {}", api));
        msg.stop_reason = StopReason::Error;
        yield AssistantMessageEvent::Error {
            reason: StopReason::Error,
            message: msg,
        };
    })
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
