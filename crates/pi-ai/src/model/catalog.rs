use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::model::Model;

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
        serde_json::from_str(include_str!("generated.json"))
            .expect("generated model registry JSON should be valid")
    });
    &MODELS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_model() {
        let model = lookup_model("claude-sonnet-4-5").unwrap();
        assert_eq!(model.id, "claude-sonnet-4-5");
        assert!((model.cost.input - 3.0).abs() < 0.001);
    }

    #[test]
    fn lookup_unknown_model() {
        assert!(lookup_model("nonexistent").is_none());
    }

    #[test]
    fn lookup_deepseek_model() {
        let model = lookup_model("deepseek-v4-flash").unwrap();
        assert_eq!(model.provider, "deepseek");
    }
}
