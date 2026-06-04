use pi_ai::models::{calculate_cost, lookup_model};
use pi_ai::types::{Model, ModelCost, ModelInput, Usage};

#[test]
fn model_serializes_like_ts_generated_shape() {
    let model = Model {
        id: "gpt-4.1".into(),
        name: "GPT-4.1".into(),
        api: "openai-responses".into(),
        provider: "openai".into(),
        base_url: "https://api.openai.com/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_047_576,
        max_tokens: 32_768,
        headers: None,
        compat: None,
    };

    let json = serde_json::to_value(&model).unwrap();
    assert_eq!(json["baseUrl"], "https://api.openai.com/v1");
    assert_eq!(json["input"], serde_json::json!(["text", "image"]));
    assert_eq!(json["cost"]["input"], 2.0);
    assert_eq!(json["cost"]["cacheRead"], 0.5);
    assert!(json.get("cacheRead").is_none());
}

#[test]
fn cost_calculation_uses_nested_model_cost() {
    let model = Model {
        id: "unit-test".into(),
        name: "Unit Test".into(),
        api: "test-api".into(),
        provider: "test".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.25,
            cache_write: 0.75,
        },
        context_window: 1000,
        max_tokens: 100,
        headers: None,
        compat: None,
    };
    let mut usage = Usage {
        input: 1_000_000,
        output: 500_000,
        cache_read: 2_000_000,
        cache_write: 4_000_000,
        total_tokens: 7_500_000,
        cost: Default::default(),
    };

    calculate_cost(&model, &mut usage);

    assert_eq!(usage.cost.input, 1.0);
    assert_eq!(usage.cost.output, 1.0);
    assert_eq!(usage.cost.cache_read, 0.5);
    assert_eq!(usage.cost.cache_write, 3.0);
}

#[test]
fn lookup_default_anthropic_model_still_works() {
    let model = lookup_model("claude-sonnet-4-5").unwrap();
    assert_eq!(model.provider, "anthropic");
    assert_eq!(model.api, "anthropic-messages");
}
