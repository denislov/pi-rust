use crate::providers;
use crate::stream::EventStream;
use crate::types::{
    AssistantMessage, AssistantMessageEvent, Context, Model, ProviderAuthDiagnostic, StopReason,
    StreamOptions,
};
use async_stream::stream;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

pub trait ApiProvider: Send + Sync {
    fn stream(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderAuth {
    pub api_key: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub azure_api_version: Option<String>,
    pub azure_resource_name: Option<String>,
    pub azure_base_url: Option<String>,
    pub azure_deployment_name: Option<String>,
    pub bedrock_region: Option<String>,
    pub bedrock_profile: Option<String>,
    pub bedrock_bearer_token: Option<String>,
    pub diagnostics: Vec<ProviderAuthDiagnostic>,
}

pub trait ProviderAuthResolver: Send + Sync {
    fn resolve_api_key(&self, _provider: &str) -> Option<String> {
        None
    }

    fn resolve_auth(&self, provider: &str) -> ProviderAuth {
        ProviderAuth {
            api_key: self.resolve_api_key(provider),
            ..ProviderAuth::default()
        }
    }

    fn resolve_model_auth(&self, model: &Model) -> ProviderAuth {
        self.resolve_auth(&model.provider)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnvProviderAuthResolver;

impl ProviderAuthResolver for EnvProviderAuthResolver {
    fn resolve_api_key(&self, provider: &str) -> Option<String> {
        crate::util::env_keys::env_api_key(provider)
    }

    fn resolve_auth(&self, provider: &str) -> ProviderAuth {
        match crate::util::env_keys::env_api_key_with_source(provider) {
            Some((api_key, source)) => ProviderAuth {
                api_key: Some(api_key),
                diagnostics: vec![auth_diagnostic("api_key", source)],
                ..ProviderAuth::default()
            },
            None => ProviderAuth::default(),
        }
    }

    fn resolve_model_auth(&self, model: &Model) -> ProviderAuth {
        let mut auth = self.resolve_auth(&model.provider);
        if model.provider == "azure-openai-responses" {
            set_auth_from_env(
                &mut auth.azure_api_version,
                &mut auth.diagnostics,
                "azure_api_version",
                "AZURE_OPENAI_API_VERSION",
            );
            set_auth_from_env(
                &mut auth.azure_base_url,
                &mut auth.diagnostics,
                "azure_base_url",
                "AZURE_OPENAI_BASE_URL",
            );
            set_auth_from_env(
                &mut auth.azure_resource_name,
                &mut auth.diagnostics,
                "azure_resource_name",
                "AZURE_OPENAI_RESOURCE_NAME",
            );
            if let Some(deployment_name) = resolve_azure_deployment_name(&model.id) {
                auth.azure_deployment_name = Some(deployment_name);
                auth.diagnostics.push(auth_diagnostic(
                    "azure_deployment_name",
                    "AZURE_OPENAI_DEPLOYMENT_NAME_MAP",
                ));
            }
        }
        auth
    }
}

fn auth_diagnostic(field: impl Into<String>, source: impl Into<String>) -> ProviderAuthDiagnostic {
    ProviderAuthDiagnostic {
        field: field.into(),
        source: source.into(),
    }
}

fn set_auth_from_env(
    target: &mut Option<String>,
    diagnostics: &mut Vec<ProviderAuthDiagnostic>,
    field: &'static str,
    env_name: &'static str,
) {
    if let Some(value) = non_empty_env(env_name) {
        *target = Some(value);
        diagnostics.push(auth_diagnostic(field, env_name));
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn resolve_azure_deployment_name(model_id: &str) -> Option<String> {
    let map = non_empty_env("AZURE_OPENAI_DEPLOYMENT_NAME_MAP")?;
    for entry in map.split(',') {
        let Some((entry_model_id, deployment)) = entry.trim().split_once('=') else {
            continue;
        };
        if entry_model_id.trim() == model_id {
            let deployment = deployment.trim();
            if !deployment.is_empty() {
                return Some(deployment.to_string());
            }
        }
    }
    None
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

fn apply_auth_material(
    mut opts: Option<StreamOptions>,
    auth: ProviderAuth,
) -> Option<StreamOptions> {
    if auth == ProviderAuth::default() {
        return opts;
    }

    let ProviderAuth {
        api_key,
        headers,
        azure_api_version,
        azure_resource_name,
        azure_base_url,
        azure_deployment_name,
        bedrock_region,
        bedrock_profile,
        bedrock_bearer_token,
        diagnostics,
    } = auth;

    let options = opts.get_or_insert_with(StreamOptions::default);
    let mut applied_fields = Vec::new();
    if fill_if_none(&mut options.api_key, api_key) {
        applied_fields.push("api_key");
    }
    if fill_if_none(&mut options.azure_api_version, azure_api_version) {
        applied_fields.push("azure_api_version");
    }
    if fill_if_none(&mut options.azure_resource_name, azure_resource_name) {
        applied_fields.push("azure_resource_name");
    }
    if fill_if_none(&mut options.azure_base_url, azure_base_url) {
        applied_fields.push("azure_base_url");
    }
    if fill_if_none(&mut options.azure_deployment_name, azure_deployment_name) {
        applied_fields.push("azure_deployment_name");
    }
    if fill_if_none(&mut options.bedrock_region, bedrock_region) {
        applied_fields.push("bedrock_region");
    }
    if fill_if_none(&mut options.bedrock_profile, bedrock_profile) {
        applied_fields.push("bedrock_profile");
    }
    if fill_if_none(&mut options.bedrock_bearer_token, bedrock_bearer_token) {
        applied_fields.push("bedrock_bearer_token");
    }
    options.headers = merge_auth_headers(headers, options.headers.take());
    append_applied_auth_diagnostics(&mut options.auth_diagnostics, diagnostics, &applied_fields);
    opts
}

fn fill_if_none(target: &mut Option<String>, value: Option<String>) -> bool {
    if target.is_none() && value.is_some() {
        *target = value;
        true
    } else {
        false
    }
}

fn append_applied_auth_diagnostics(
    target: &mut Vec<ProviderAuthDiagnostic>,
    diagnostics: Vec<ProviderAuthDiagnostic>,
    applied_fields: &[&str],
) {
    target.extend(
        diagnostics
            .into_iter()
            .filter(|diagnostic| applied_fields.contains(&diagnostic.field.as_str())),
    );
}

fn merge_auth_headers(
    auth_headers: Option<serde_json::Value>,
    option_headers: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match (auth_headers, option_headers) {
        (None, explicit) => explicit,
        (auth, None) => auth,
        (Some(serde_json::Value::Object(mut auth)), Some(serde_json::Value::Object(explicit))) => {
            for (key, value) in explicit {
                auth.insert(key, value);
            }
            Some(serde_json::Value::Object(auth))
        }
        (_, explicit @ Some(_)) => explicit,
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

#[deprecated(note = "use AiClient or ProviderRegistry for scoped provider runtime registration")]
pub fn register(api: &str, provider: Arc<dyn ApiProvider>) {
    REGISTRY.register(api, provider);
}

#[deprecated(note = "use AiClient or ProviderRegistry for scoped provider runtime state")]
pub fn unregister(api: &str) {
    REGISTRY.unregister(api);
}

#[deprecated(note = "use AiClient or ProviderRegistry for scoped provider runtime lookup")]
pub fn lookup(api: &str) -> Option<Arc<dyn ApiProvider>> {
    REGISTRY.lookup(api)
}

/// Top-level entry point: resolves provider by model.api, injects env API key
/// if not provided, delegates to provider.stream(). Returns a stream that
/// immediately yields Error on unknown api.
#[deprecated(note = "use AiClient or ProviderRegistry for scoped provider runtime streaming")]
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
#[allow(deprecated)]
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
