use pi_ai::api::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Cost, EventStream, Message,
    Model, ModelCost, ModelInput, ProviderResponseInfo, ProviderStreamHooks, StopReason,
    StreamOptions, ThinkingConfig, Tool, Usage, all_models, calculate_cost, complete, env_api_key,
    get_model, get_models, get_providers, lookup_model, register, stream_model,
};

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
