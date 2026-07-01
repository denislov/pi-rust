use async_stream::stream;
use pi_ai::api::{
    AiClient, ApiProvider, AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Cost,
    EventStream, Message, Model, ModelCost, ModelInput, ProviderAuthResolver, ProviderResponseInfo,
    ProviderStreamHooks, StopReason, StreamOptions, ThinkingConfig, Tool, Usage, all_models,
    calculate_cost, complete, env_api_key, get_model, get_models, get_providers, lookup_model,
    register, stream_model,
};
use std::sync::{Arc, Mutex};

#[test]
fn public_api_symbols_are_importable_from_api_facade() {
    let _ = all_models as fn() -> &'static [Model];
    let _ = get_models as fn(&str) -> Vec<Model>;
    let _ = get_providers as fn() -> Vec<String>;
    let _ = get_model as fn(&str, &str) -> Option<Model>;
    let _ = lookup_model as fn(&str) -> Option<Model>;
    let _ = calculate_cost as fn(&Model, &mut Usage);
    let _ = env_api_key as fn(&str) -> Option<String>;

    fn accepts_types(
        _assistant: Option<AssistantMessage>,
        _event: Option<AssistantMessageEvent>,
        _content: Option<ContentBlock>,
        _context: Option<Context>,
        _cost: Option<Cost>,
        _message: Option<Message>,
        _model_cost: Option<ModelCost>,
        _model_input: Option<ModelInput>,
        _provider_info: Option<ProviderResponseInfo>,
        _hooks: Option<ProviderStreamHooks>,
        _stop: Option<StopReason>,
        _options: Option<StreamOptions>,
        _thinking: Option<ThinkingConfig>,
        _tool: Option<Tool>,
        _usage: Option<Usage>,
        _stream: Option<EventStream>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    );

    let _ = complete;
    let _ = register;
    let _ = stream_model;
}

#[tokio::test]
async fn scoped_ai_client_streams_without_global_registration() {
    let client = AiClient::new();
    let model = scoped_model("scoped-only-api", "scoped-provider");
    client.register_provider(
        "scoped-only-api",
        Arc::new(StaticProvider::new("scoped response")),
    );

    let message = complete(client.stream_model(&model, empty_context(), None))
        .await
        .unwrap();
    assert_eq!(message_text(&message), "scoped response");

    let global_error = complete(stream_model(&model, empty_context(), None))
        .await
        .unwrap_err();
    assert!(global_error.contains("unknown provider api: scoped-only-api"));
}

#[tokio::test]
async fn scoped_ai_client_uses_injected_auth_resolver() {
    let seen_api_key = Arc::new(Mutex::new(None));
    let client = AiClient::with_auth_resolver(Arc::new(StaticAuthResolver));
    let model = scoped_model("scoped-auth-api", "scoped-provider");
    client.register_provider(
        "scoped-auth-api",
        Arc::new(StaticProvider::recording(
            "auth response",
            Arc::clone(&seen_api_key),
        )),
    );

    complete(client.stream_model(&model, empty_context(), Some(StreamOptions::default())))
        .await
        .unwrap();

    assert_eq!(
        seen_api_key.lock().unwrap().as_deref(),
        Some("scoped-key:scoped-provider")
    );
}

struct StaticAuthResolver;

impl ProviderAuthResolver for StaticAuthResolver {
    fn resolve_api_key(&self, provider: &str) -> Option<String> {
        Some(format!("scoped-key:{provider}"))
    }
}

struct StaticProvider {
    text: &'static str,
    seen_api_key: Option<Arc<Mutex<Option<String>>>>,
}

impl StaticProvider {
    fn new(text: &'static str) -> Self {
        Self {
            text,
            seen_api_key: None,
        }
    }

    fn recording(text: &'static str, seen_api_key: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            text,
            seen_api_key: Some(seen_api_key),
        }
    }
}

impl ApiProvider for StaticProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        if let Some(seen_api_key) = &self.seen_api_key {
            *seen_api_key.lock().unwrap() = opts.as_ref().and_then(|opts| opts.api_key.clone());
        }
        let text = self.text.to_string();
        let mut message = AssistantMessage::empty(&model.api, &model.id);
        message.content.push(ContentBlock::Text {
            text,
            text_signature: None,
        });
        Box::pin(stream! {
            yield AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            };
        })
    }
}

fn empty_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    }
}

fn scoped_model(api: &str, provider: &str) -> Model {
    Model {
        id: "scoped-model".into(),
        name: "Scoped Model".into(),
        api: api.into(),
        provider: provider.into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 8_000,
        max_tokens: 1_024,
        headers: None,
        compat: None,
    }
}

fn message_text(message: &AssistantMessage) -> &str {
    match &message.content[0] {
        ContentBlock::Text { text, .. } => text.as_str(),
        _ => panic!("expected text content"),
    }
}
