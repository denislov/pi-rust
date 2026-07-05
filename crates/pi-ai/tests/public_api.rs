use async_stream::stream;
use pi_ai::api::{
    AiClient, AnthropicMessagesCompat, ApiProvider, AssistantImages, AssistantMessage,
    AssistantMessageEvent, CacheControlFormat, ContentBlock, Context, Cost, EventStream,
    ImageContent, ImageInput, ImageOutput, ImagesContext, ImagesModel, ImagesModelCost,
    ImagesModelOutput, ImagesUsage, Message, Model, ModelCompat, ModelCost, ModelInput,
    OpenAICompletionsCompat, OpenAIResponsesCompat, OpenRouterRouting, ProviderAuthResolver,
    ProviderError, ProviderErrorKind, ProviderPayloadHook, ProviderPayloadHookFuture,
    ProviderRegistry, ProviderResponseHook, ProviderResponseHookFuture, ProviderResponseInfo,
    ProviderStreamHooks, RetryConfig, StopReason, StreamOptions, TextContent, ThinkingConfig,
    ThinkingFormat, ThinkingLevelMap, ThinkingLevelValue, Tool, Usage, VercelGatewayRouting,
    all_models, builtin_provider_apis, calculate_cost, complete, env_api_key, get_model,
    get_models, get_providers, is_retryable_status, lookup_model, parse_retry_after_ms,
    register_builtins_into,
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
    let _ = builtin_provider_apis as fn() -> &'static [&'static str];

    fn accepts_types(
        _assistant: Option<AssistantMessage>,
        _event: Option<AssistantMessageEvent>,
        _content: Option<ContentBlock>,
        _context: Option<Context>,
        _cost: Option<Cost>,
        _image_content: Option<ImageContent>,
        _image_input: Option<ImageInput>,
        _image_output: Option<ImageOutput>,
        _images: Option<AssistantImages>,
        _images_context: Option<ImagesContext>,
        _images_cost: Option<ImagesModelCost>,
        _images_model: Option<ImagesModel>,
        _images_output: Option<ImagesModelOutput>,
        _images_usage: Option<ImagesUsage>,
        _message: Option<Message>,
        _model_compat: Option<ModelCompat>,
        _model_cost: Option<ModelCost>,
        _model_input: Option<ModelInput>,
        _openai_completions_compat: Option<OpenAICompletionsCompat>,
        _openrouter_routing: Option<OpenRouterRouting>,
        _provider_payload_hook: Option<ProviderPayloadHook>,
        _provider_payload_future: Option<ProviderPayloadHookFuture>,
        _provider_info: Option<ProviderResponseInfo>,
        _provider_response_hook: Option<ProviderResponseHook>,
        _provider_response_future: Option<ProviderResponseHookFuture>,
        _hooks: Option<ProviderStreamHooks>,
        _stop: Option<StopReason>,
        _options: Option<StreamOptions>,
        _text_content: Option<TextContent>,
        _thinking: Option<ThinkingConfig>,
        _thinking_format: Option<ThinkingFormat>,
        _thinking_level_map: Option<ThinkingLevelMap>,
        _thinking_level_value: Option<ThinkingLevelValue>,
        _tool: Option<Tool>,
        _usage: Option<Usage>,
        _vercel_gateway_routing: Option<VercelGatewayRouting>,
        _cache_control_format: Option<CacheControlFormat>,
        _stream: Option<EventStream>,
    ) {
    }

    accepts_types(
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
    );

    let _ = complete;
}

#[test]
fn retry_policy_helpers_are_importable_from_api_facade() {
    let options = StreamOptions {
        max_retries: Some(3),
        timeout_ms: Some(2_000),
        max_retry_delay_ms: Some(1_500),
        ..StreamOptions::default()
    };
    let retry_config = RetryConfig::from_options(Some(&options));

    assert_eq!(retry_config.max_retries, 3);
    assert_eq!(retry_config.timeout_ms, Some(2_000));
    assert_eq!(retry_config.max_retry_delay_ms, 1_500);
    assert!(is_retryable_status(429));
    assert!(is_retryable_status(503));
    assert!(!is_retryable_status(404));
    assert_eq!(parse_retry_after_ms(Some("1.25"), &retry_config), Ok(1_250));
    assert!(
        parse_retry_after_ms(Some("2"), &retry_config)
            .unwrap_err()
            .contains("exceeds max_retry_delay_ms")
    );
}

#[test]
fn provider_error_surface_is_importable_from_api_facade() {
    let error = ProviderError::http_status(
        "openai-responses",
        "gpt-test",
        "openai",
        429,
        "rate limited".into(),
    );

    assert_eq!(error.kind, ProviderErrorKind::HttpStatus);
    assert_eq!(error.api, "openai-responses");
    assert_eq!(error.provider.as_deref(), Some("openai"));
    assert_eq!(error.model.as_deref(), Some("gpt-test"));
    assert_eq!(error.status, Some(429));
    assert_eq!(error.body.as_deref(), Some("rate limited"));
    assert!(error.to_string().contains("HTTP 429"));
}

#[test]
fn compat_surface_is_importable_from_api_facade() {
    let anthropic = ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
        supports_temperature: Some(true),
        supports_cache_control_on_tools: Some(false),
        ..AnthropicMessagesCompat::default()
    });
    let ModelCompat::AnthropicMessages(anthropic_compat) = anthropic else {
        panic!("expected anthropic messages compat");
    };
    assert_eq!(anthropic_compat.supports_temperature, Some(true));
    assert_eq!(
        anthropic_compat.supports_cache_control_on_tools,
        Some(false)
    );

    let responses = ModelCompat::OpenAIResponses(OpenAIResponsesCompat {
        send_session_id_header: Some(true),
        supports_long_cache_retention: Some(true),
    });
    let ModelCompat::OpenAIResponses(responses_compat) = responses else {
        panic!("expected openai responses compat");
    };
    assert_eq!(responses_compat.send_session_id_header, Some(true));
    assert_eq!(responses_compat.supports_long_cache_retention, Some(true));

    let thinking_map = ThinkingLevelMap {
        high: Some(ThinkingLevelValue::String("provider-high".into())),
        low: Some(ThinkingLevelValue::Null),
        ..ThinkingLevelMap::default()
    };
    assert_eq!(thinking_map.resolve("high"), Some("provider-high".into()));
    assert_eq!(thinking_map.resolve("low"), None);
    assert_eq!(thinking_map.resolve("medium"), Some("medium".into()));

    let compat = ModelCompat::OpenAICompletions(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::OpenRouter),
        cache_control_format: Some(CacheControlFormat::Anthropic),
        open_router_routing: Some(OpenRouterRouting {
            allow_fallbacks: Some(false),
            ..OpenRouterRouting::default()
        }),
        vercel_gateway_routing: Some(VercelGatewayRouting {
            only: Some(vec!["openai".into()]),
            ..VercelGatewayRouting::default()
        }),
        ..OpenAICompletionsCompat::default()
    });
    let ModelCompat::OpenAICompletions(openai_compat) = compat else {
        panic!("expected openai completions compat");
    };
    assert_eq!(
        openai_compat.thinking_format,
        Some(ThinkingFormat::OpenRouter)
    );
    assert_eq!(
        openai_compat.cache_control_format,
        Some(CacheControlFormat::Anthropic)
    );
    assert_eq!(
        openai_compat
            .open_router_routing
            .and_then(|routing| routing.allow_fallbacks),
        Some(false)
    );
    assert_eq!(
        openai_compat
            .vercel_gateway_routing
            .and_then(|routing| routing.only)
            .unwrap(),
        vec!["openai".to_string()]
    );
}

#[test]
fn builtins_can_register_into_scoped_provider_registry() {
    let registry = ProviderRegistry::new();
    assert!(registry.lookup("openai-responses").is_none());

    register_builtins_into(&registry);

    for api in builtin_provider_apis() {
        assert!(
            registry.lookup(api).is_some(),
            "expected built-in api {api} to be registered into the scoped registry"
        );
    }
}

#[test]
fn builtin_provider_catalog_matches_scoped_registration_set() {
    let registry = ProviderRegistry::new();
    register_builtins_into(&registry);

    let expected = builtin_provider_apis()
        .iter()
        .map(|api| (*api).to_string())
        .collect::<Vec<_>>();
    assert_eq!(registry.registered_apis(), expected);
}

#[test]
fn scoped_ai_client_can_register_builtin_providers_directly() {
    let client = AiClient::new();
    assert!(client.lookup_provider("openai-responses").is_none());

    client.register_builtins();

    let expected = builtin_provider_apis()
        .iter()
        .map(|api| (*api).to_string())
        .collect::<Vec<_>>();
    assert_eq!(client.provider_registry().registered_apis(), expected);
    for api in builtin_provider_apis() {
        assert!(
            client.lookup_provider(api).is_some(),
            "expected built-in api {api} to be registered into the scoped AiClient"
        );
    }
}

#[test]
fn builtin_provider_catalog_is_stable_and_sorted() {
    assert_eq!(
        builtin_provider_apis(),
        &[
            "anthropic-messages",
            "azure-openai-responses",
            "bedrock-converse-stream",
            "deepseek-chat-completions",
            "google-generative-ai",
            "mistral-conversations",
            "openai-codex-responses",
            "openai-completions",
            "openai-responses",
        ]
    );
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

    let global_error = complete(pi_ai::stream_model(&model, empty_context(), None))
        .await
        .unwrap_err();
    assert!(global_error.contains("unknown provider api: scoped-only-api"));
}

#[tokio::test]
async fn scoped_ai_client_uses_injected_auth_resolver_without_stream_options() {
    let seen_api_key = Arc::new(Mutex::new(None));
    let client = AiClient::with_auth_resolver(Arc::new(StaticAuthResolver));
    let model = scoped_model("scoped-auth-none-api", "scoped-provider");
    client.register_provider(
        "scoped-auth-none-api",
        Arc::new(StaticProvider::recording(
            "auth response",
            Arc::clone(&seen_api_key),
        )),
    );

    complete(client.stream_model(&model, empty_context(), None))
        .await
        .unwrap();

    assert_eq!(
        seen_api_key.lock().unwrap().as_deref(),
        Some("scoped-key:scoped-provider")
    );
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
