use crate::types::{Model, Usage};
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// Static model lookup by id. Searches deterministic priority then lexical.
pub fn lookup_model(id: &str) -> Option<Model> {
    const PRIORITY: &[&str] = &["anthropic", "openai", "google", "deepseek"];
    for provider in PRIORITY {
        if let Some(model) = get_model(provider, id) {
            return Some(model);
        }
    }
    all_models().iter().find(|model| model.id == id).cloned()
}

pub fn get_model(provider: &str, id: &str) -> Option<Model> {
    all_models()
        .iter()
        .find(|model| model.provider == provider && model.id == id)
        .cloned()
}

pub fn get_models(provider: &str) -> Vec<Model> {
    all_models()
        .iter()
        .filter(|model| model.provider == provider)
        .cloned()
        .collect()
}

pub fn get_providers() -> Vec<String> {
    let mut providers = BTreeSet::new();
    for model in all_models() {
        providers.insert(model.provider.clone());
    }
    providers.into_iter().collect()
}

pub fn all_models() -> &'static [Model] {
    static MODELS: LazyLock<Vec<Model>> = LazyLock::new(|| {
        serde_json::from_str(include_str!("models_generated.json"))
            .expect("generated model registry JSON should be valid")
    });
    &MODELS
}

/// Calculate cost for the given usage against a model's rates.
/// Rates are per million tokens. Updates usage.cost in place.
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    usage.cost.input = (usage.input as f64 / 1_000_000.0) * model.cost.input;
    usage.cost.output = (usage.output as f64 / 1_000_000.0) * model.cost.output;
    usage.cost.cache_read = (usage.cache_read as f64 / 1_000_000.0) * model.cost.cache_read;
    usage.cost.cache_write = (usage.cache_write as f64 / 1_000_000.0) * model.cost.cache_write;
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
