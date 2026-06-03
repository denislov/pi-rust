use crate::types::{Model, Usage};

/// Static model lookup by id. Returns None for unknown models.
pub fn lookup_model(id: &str) -> Option<Model> {
    all_models().iter().find(|m| m.id == id).cloned()
}

/// Calculate cost for the given usage against a model's rates.
/// Rates are per million tokens. Updates usage.cost in place.
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    let input_cost = (usage.input as f64 / 1_000_000.0) * model.input;
    let output_cost = (usage.output as f64 / 1_000_000.0) * model.output;
    let cache_read_cost = model
        .cache_read
        .map_or(0.0, |rate| (usage.cache_read as f64 / 1_000_000.0) * rate);
    let cache_write_cost = model
        .cache_write
        .map_or(0.0, |rate| (usage.cache_write as f64 / 1_000_000.0) * rate);
    usage.cost.input = input_cost;
    usage.cost.output = output_cost;
    usage.cost.cache_read = cache_read_cost;
    usage.cost.cache_write = cache_write_cost;
}

/// Hand-crafted static model table (subset of Anthropic models).
/// Populated inline via build_models() at first access.
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
        input: f64,
        output: f64,
        cache_read: Option<f64>,
        cache_write: Option<f64>,
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
            input,
            output,
            cache_read,
            cache_write,
            context_window,
            max_tokens: Some(max_tokens),
            headers: None,
        }
    }
    vec![
        m(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            true,
            3.0,
            15.0,
            Some(0.30),
            Some(3.75),
            200_000,
            8192,
        ),
        m(
            "claude-haiku-4-5",
            "Claude Haiku 4.5",
            false,
            1.0,
            5.0,
            Some(0.10),
            Some(1.25),
            200_000,
            8192,
        ),
        m(
            "claude-opus-4-5",
            "Claude Opus 4.5",
            true,
            15.0,
            75.0,
            Some(1.50),
            Some(18.75),
            200_000,
            8192,
        ),
        m(
            "claude-sonnet-4",
            "Claude Sonnet 4",
            true,
            3.0,
            15.0,
            Some(0.30),
            Some(3.75),
            200_000,
            8192,
        ),
        m(
            "claude-opus-4",
            "Claude Opus 4",
            true,
            15.0,
            75.0,
            Some(1.50),
            Some(18.75),
            200_000,
            8192,
        ),
        m(
            "claude-3-5-sonnet-latest",
            "Claude 3.5 Sonnet",
            false,
            3.0,
            15.0,
            Some(0.30),
            Some(3.75),
            200_000,
            8192,
        ),
        m(
            "claude-3-5-haiku-latest",
            "Claude 3.5 Haiku",
            false,
            0.80,
            4.0,
            Some(0.08),
            Some(1.00),
            200_000,
            8192,
        ),
        m(
            "claude-3-opus-latest",
            "Claude 3 Opus",
            false,
            15.0,
            75.0,
            Some(1.50),
            Some(18.75),
            200_000,
            4096,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_model() {
        let m = lookup_model("claude-sonnet-4-5").unwrap();
        assert_eq!(m.id, "claude-sonnet-4-5");
        assert_eq!(m.input, 3.0);
    }

    #[test]
    fn lookup_unknown_model() {
        assert!(lookup_model("nonexistent").is_none());
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
        assert!((usage.cost.input - 1.0).abs() < 0.001); // 1M tokens * $1/M
        assert!((usage.cost.output - 2.5).abs() < 0.001); // 500K tokens * $5/M
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
        assert!((usage.cost.cache_read - 0.30).abs() < 0.001); // $0.30/M
        assert!((usage.cost.cache_write - 7.50).abs() < 0.001); // $3.75/M * 2
    }
}
