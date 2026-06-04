use crate::types::{Model, ModelCost, ModelInput, Usage};

/// Static model lookup by id. Returns None for unknown models.
pub fn lookup_model(id: &str) -> Option<Model> {
    all_models().iter().find(|m| m.id == id).cloned()
}

/// Calculate cost for the given usage against a model's rates.
/// Rates are per million tokens. Updates usage.cost in place.
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    usage.cost.input = (usage.input as f64 / 1_000_000.0) * model.cost.input;
    usage.cost.output = (usage.output as f64 / 1_000_000.0) * model.cost.output;
    usage.cost.cache_read = (usage.cache_read as f64 / 1_000_000.0) * model.cost.cache_read;
    usage.cost.cache_write = (usage.cache_write as f64 / 1_000_000.0) * model.cost.cache_write;
}

/// Static model table.
pub fn all_models() -> &'static [Model] {
    use std::sync::LazyLock;
    static MODELS: LazyLock<Vec<Model>> = LazyLock::new(build_models);
    &MODELS
}

fn build_models() -> Vec<Model> {
    fn m(
        id: &str,
        name: &str,
        reasoning: bool,
        input_price: f64,
        output_price: f64,
        cache_read: f64,
        cache_write: f64,
        context_window: u32,
        max_tokens: u32,
    ) -> Model {
        Model {
            id: id.into(),
            name: name.into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            reasoning,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: input_price,
                output: output_price,
                cache_read,
                cache_write,
            },
            context_window,
            max_tokens,
            headers: None,
            compat: None,
        }
    }
    fn deepseek_m(id: &str, name: &str, reasoning: bool) -> Model {
        Model {
            id: id.into(),
            name: name.into(),
            api: "deepseek-chat-completions".into(),
            provider: "deepseek".into(),
            base_url: "https://api.deepseek.com".into(),
            reasoning,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 64_000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        }
    }
    vec![
        m(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            true,
            3.0,
            15.0,
            0.30,
            3.75,
            200_000,
            8192,
        ),
        m(
            "claude-haiku-4-5",
            "Claude Haiku 4.5",
            false,
            1.0,
            5.0,
            0.10,
            1.25,
            200_000,
            8192,
        ),
        m(
            "claude-opus-4-5",
            "Claude Opus 4.5",
            true,
            15.0,
            75.0,
            1.50,
            18.75,
            200_000,
            8192,
        ),
        m(
            "claude-sonnet-4",
            "Claude Sonnet 4",
            true,
            3.0,
            15.0,
            0.30,
            3.75,
            200_000,
            8192,
        ),
        m(
            "claude-opus-4",
            "Claude Opus 4",
            true,
            15.0,
            75.0,
            1.50,
            18.75,
            200_000,
            8192,
        ),
        m(
            "claude-3-5-sonnet-latest",
            "Claude 3.5 Sonnet",
            false,
            3.0,
            15.0,
            0.30,
            3.75,
            200_000,
            8192,
        ),
        m(
            "claude-3-5-haiku-latest",
            "Claude 3.5 Haiku",
            false,
            0.80,
            4.0,
            0.08,
            1.00,
            200_000,
            8192,
        ),
        m(
            "claude-3-opus-latest",
            "Claude 3 Opus",
            false,
            15.0,
            75.0,
            1.50,
            18.75,
            200_000,
            4096,
        ),
        deepseek_m("deepseek-v4-flash", "DeepSeek V4 Flash", false),
        deepseek_m("deepseek-v4-pro", "DeepSeek V4 Pro", true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_model() {
        let m = lookup_model("claude-sonnet-4-5").unwrap();
        assert_eq!(m.id, "claude-sonnet-4-5");
        assert!((m.cost.input - 3.0).abs() < 0.001);
    }

    #[test]
    fn lookup_unknown_model() {
        assert!(lookup_model("nonexistent").is_none());
    }

    #[test]
    fn lookup_deepseek_model() {
        let m = lookup_model("deepseek-v4-flash").unwrap();
        assert_eq!(m.provider, "deepseek");
        assert_eq!(m.api, "deepseek-chat-completions");
    }

    #[test]
    fn cost_calculation_basic() {
        let model = lookup_model("claude-haiku-4-5").unwrap();
        let mut usage = Usage {
            input: 1_000_000,
            output: 500_000,
            cache_read: 0,
            cache_write: 0,
            total_tokens: 1_500_000,
            cost: Default::default(),
        };
        calculate_cost(&model, &mut usage);
        assert!((usage.cost.input - 1.0).abs() < 0.001);
        assert!((usage.cost.output - 2.5).abs() < 0.001);
    }

    #[test]
    fn cost_calculation_with_cache() {
        let model = lookup_model("claude-sonnet-4-5").unwrap();
        let mut usage = Usage {
            input: 0,
            output: 0,
            cache_read: 1_000_000,
            cache_write: 2_000_000,
            total_tokens: 3_000_000,
            cost: Default::default(),
        };
        calculate_cost(&model, &mut usage);
        assert!((usage.cost.cache_read - 0.30).abs() < 0.001);
        assert!((usage.cost.cache_write - 7.50).abs() < 0.001);
    }
}
