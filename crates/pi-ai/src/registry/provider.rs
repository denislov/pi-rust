use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_stream::stream;

use super::auth::{EnvProviderAuthResolver, ProviderAuthResolver, apply_auth_material};
use crate::model::Model;
use crate::protocol::stream::EventStream;
use crate::protocol::{
    AssistantMessage, AssistantMessageEvent, Context, StopReason, StreamOptions,
};

pub trait ApiProvider: Send + Sync {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream;
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

        opts = apply_auth_material(opts, auth_resolver.resolve_model_auth(model));

        provider.stream(model, ctx, opts)
    }
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
