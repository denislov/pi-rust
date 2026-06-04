# pi-ai M2 provider breadth and model registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add M2 provider breadth to `pi-ai`: TS-shaped model metadata, generated model registry, OpenAI Chat Completions, OpenAI Responses, Google Generative AI, broader env-key lookup, and shared retry/timeout behavior.

**Architecture:** Keep the existing `ApiProvider` and event-stream contract. Migrate `Model` to the TS generated shape, commit a generated Rust model registry, and add provider modules where conversion, wire structs, stream processing, and HTTP wrappers are separately testable. Use raw `reqwest` streaming and offline SSE fixtures instead of live provider SDKs.

**Tech Stack:** Rust edition 2024, `reqwest`, `async-stream`, `futures`, `serde`, `serde_json`, `tokio-util::CancellationToken`, Node.js only for the manual registry generator. Behavioral reference: `pi/packages/ai/src/models.generated.ts`, `providers/openai-completions.ts`, `providers/openai-responses.ts`, `providers/openai-responses-shared.ts`, `providers/google.ts`, and `providers/google-shared.ts`.

**Spec:** `docs/superpowers/specs/2026-06-04-pi-ai-m2-provider-breadth-model-registry-design.md`

---

## File Structure

- Modify `crates/pi-ai/src/types.rs` - TS-shaped `Model`, `ModelCost`, `ModelInput`, extra `StreamOptions` fields.
- Modify `crates/pi-ai/src/models.rs` - generated registry lookup API and nested cost calculation.
- Create `crates/pi-ai/src/models_generated.rs` - generated `Vec<Model>` constructors.
- Create `crates/pi-ai/tools/generate_models.cjs` - manual TS model table to Rust generator.
- Modify `crates/pi-ai/src/util/mod.rs` - export shared `sse` and `http`.
- Create `crates/pi-ai/src/util/sse.rs` - shared SSE decoder moved from Anthropic.
- Create `crates/pi-ai/src/util/http.rs` - retry/timeout helper and retry policy.
- Modify `crates/pi-ai/src/util/env_keys.rs` - expanded M2 provider env-key map.
- Modify `crates/pi-ai/src/providers/mod.rs` - register new built-in providers.
- Create `crates/pi-ai/src/providers/openai/mod.rs` - OpenAI provider namespace.
- Create `crates/pi-ai/src/providers/openai/common.rs` - shared OpenAI auth, headers, stop reasons, usage, compat helpers.
- Create `crates/pi-ai/src/providers/openai/completions/{mod.rs,convert.rs,process.rs,wire.rs}`.
- Create `crates/pi-ai/src/providers/openai/responses/{mod.rs,convert.rs,process.rs,wire.rs}`.
- Create `crates/pi-ai/src/providers/google/{mod.rs,convert.rs,process.rs,wire.rs}`.
- Modify existing tests and examples in `crates/pi-ai`, `crates/pi-agent-core`, and `crates/pi-coding-agent` that construct `Model` literals.
- Create tests `crates/pi-ai/tests/{model_registry.rs,env_keys.rs,http_retry.rs,openai_completions.rs,openai_responses.rs,google.rs}`.
- Create fixtures `crates/pi-ai/tests/fixtures/{openai-completions-text-tool.sse,openai-responses-text-tool.sse,google-text-tool.sse}`.

---

## Task 1: Migrate `Model` to the TypeScript generated shape

**Files:**
- Modify: `crates/pi-ai/src/types.rs`
- Modify: `crates/pi-ai/src/models.rs`
- Create: `crates/pi-ai/tests/model_registry.rs`

- [ ] **Step 1: Write failing model-shape tests**

Create `crates/pi-ai/tests/model_registry.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p pi-ai --test model_registry`

Expected: compile failure because `ModelInput`, `ModelCost`, `Model.thinking_level_map`, and `Model.cost` do not exist.

- [ ] **Step 3: Update `types.rs`**

Add `ModelInput` and `ModelCost`, and replace the current flattened `Model` fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    Text,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub reasoning: bool,
    #[serde(rename = "thinkingLevelMap", skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<serde_json::Value>,
    pub input: Vec<ModelInput>,
    pub cost: ModelCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u32,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<serde_json::Value>,
}
```

Extend the crate re-exports in `src/lib.rs` to include `ModelCost` and `ModelInput`.

- [ ] **Step 4: Update `calculate_cost`**

In `crates/pi-ai/src/models.rs`, change the cost reads to:

```rust
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    usage.cost.input = (usage.input as f64 / 1_000_000.0) * model.cost.input;
    usage.cost.output = (usage.output as f64 / 1_000_000.0) * model.cost.output;
    usage.cost.cache_read = (usage.cache_read as f64 / 1_000_000.0) * model.cost.cache_read;
    usage.cost.cache_write = (usage.cache_write as f64 / 1_000_000.0) * model.cost.cache_write;
}
```

- [ ] **Step 5: Migrate existing `Model` literals in `crates/pi-ai`**

Use this pattern for test-only dummy models:

```rust
Model {
    id: "x".into(),
    name: "x".into(),
    api: "test-api".into(),
    provider: "test".into(),
    base_url: "https://example.invalid".into(),
    reasoning: false,
    thinking_level_map: None,
    input: vec![ModelInput::Text],
    cost: ModelCost::default(),
    context_window: 0,
    max_tokens: 4096,
    headers: None,
    compat: None,
}
```

Update assertions that read `model.input` as a price to read `model.cost.input`.

- [ ] **Step 6: Run the focused test**

Run: `cargo test -p pi-ai --test model_registry`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-ai/src/types.rs crates/pi-ai/src/lib.rs crates/pi-ai/src/models.rs crates/pi-ai/tests/model_registry.rs
git commit -m "feat(pi-ai): align model metadata with generated registry shape"
```

---

## Task 2: Add generated model registry and lookup API

**Files:**
- Create: `crates/pi-ai/tools/generate_models.cjs`
- Create: `crates/pi-ai/src/models_generated.rs`
- Modify: `crates/pi-ai/src/models.rs`
- Modify: `crates/pi-ai/tests/model_registry.rs`

- [ ] **Step 1: Add registry tests**

Append to `crates/pi-ai/tests/model_registry.rs`:

```rust
use pi_ai::models::{all_models, get_model, get_models, get_providers};

#[test]
fn registry_contains_m2_models_from_ts_reference() {
    let gpt = get_model("openai", "gpt-4.1").unwrap();
    assert_eq!(gpt.api, "openai-responses");
    assert_eq!(gpt.input, vec![ModelInput::Text, ModelInput::Image]);

    let gpt5 = get_model("openai", "gpt-5").unwrap();
    assert_eq!(gpt5.api, "openai-responses");
    assert!(gpt5.reasoning);

    let gemini = get_model("google", "gemini-2.5-flash").unwrap();
    assert_eq!(gemini.api, "google-generative-ai");
    assert!(gemini.input.contains(&ModelInput::Image));

    let deepseek = get_model("deepseek", "deepseek-v4-flash").unwrap();
    assert_eq!(deepseek.api, "openai-completions");
    assert_eq!(deepseek.provider, "deepseek");

    let claude = get_model("anthropic", "claude-sonnet-4-5").unwrap();
    assert_eq!(claude.api, "anthropic-messages");
}

#[test]
fn provider_listing_is_deterministic_and_non_empty() {
    let providers = get_providers();
    assert!(providers.windows(2).all(|w| w[0] <= w[1]));
    assert!(providers.contains(&"anthropic".to_string()));
    assert!(providers.contains(&"openai".to_string()));
    assert!(providers.contains(&"google".to_string()));
}

#[test]
fn provider_model_listing_filters_by_provider() {
    let openai = get_models("openai");
    assert!(openai.iter().any(|m| m.id == "gpt-4.1"));
    assert!(openai.iter().all(|m| m.provider == "openai"));
}

#[test]
fn generated_registry_has_unique_provider_id_pairs() {
    let mut seen = std::collections::BTreeSet::new();
    for model in all_models() {
        assert!(
            seen.insert((model.provider.clone(), model.id.clone())),
            "duplicate model pair: {}/{}",
            model.provider,
            model.id
        );
    }
}
```

- [ ] **Step 2: Run and verify failure**

Run: `cargo test -p pi-ai --test model_registry`

Expected: compile failure for missing lookup APIs and missing generated models.

- [ ] **Step 3: Create the generator**

Create `crates/pi-ai/tools/generate_models.cjs` with these concrete behaviors:

```javascript
#!/usr/bin/env node
const fs = require("node:fs");
const vm = require("node:vm");

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  console.error("usage: node crates/pi-ai/tools/generate_models.cjs <models.generated.ts> <models_generated.rs>");
  process.exit(2);
}

let source = fs.readFileSync(inputPath, "utf8");
source = source.replace(/^import type .*$/gm, "");
source = source.replace(/export const MODELS\s*=\s*/, "const MODELS = ");
source = source.replace(/\s+satisfies\s+Model<[^>]+>/g, "");
source += "\nMODELS;";

const models = vm.runInNewContext(source, {}, { filename: inputPath });

function rustString(value) {
  return JSON.stringify(String(value));
}

function jsonValue(value) {
  return value === undefined ? "None" : `Some(serde_json::json!(${JSON.stringify(value)}))`;
}

function modelInput(values) {
  return `vec![${values.map((v) => v === "image" ? "ModelInput::Image" : "ModelInput::Text").join(", ")}]`;
}

const out = [];
out.push("// This file is generated by tools/generate_models.cjs.");
out.push("// Do not edit by hand.");
out.push("use crate::types::{Model, ModelCost, ModelInput};");
out.push("");
out.push("pub fn generated_models() -> Vec<Model> {");
out.push("    vec![");

for (const provider of Object.keys(models).sort()) {
  for (const id of Object.keys(models[provider]).sort()) {
    const m = models[provider][id];
    out.push("        Model {");
    out.push(`            id: ${rustString(m.id)}.into(),`);
    out.push(`            name: ${rustString(m.name)}.into(),`);
    out.push(`            api: ${rustString(m.api)}.into(),`);
    out.push(`            provider: ${rustString(m.provider)}.into(),`);
    out.push(`            base_url: ${rustString(m.baseUrl)}.into(),`);
    out.push(`            reasoning: ${Boolean(m.reasoning)},`);
    out.push(`            thinking_level_map: ${jsonValue(m.thinkingLevelMap)},`);
    out.push(`            input: ${modelInput(m.input || ["text"])},`);
    out.push("            cost: ModelCost {");
    out.push(`                input: ${Number(m.cost?.input || 0)},`);
    out.push(`                output: ${Number(m.cost?.output || 0)},`);
    out.push(`                cache_read: ${Number(m.cost?.cacheRead || 0)},`);
    out.push(`                cache_write: ${Number(m.cost?.cacheWrite || 0)},`);
    out.push("            },");
    out.push(`            context_window: ${Number(m.contextWindow || 0)},`);
    out.push(`            max_tokens: ${Number(m.maxTokens || 0)},`);
    out.push(`            headers: ${jsonValue(m.headers)},`);
    out.push(`            compat: ${jsonValue(m.compat)},`);
    out.push("        },");
  }
}

out.push("    ]");
out.push("}");
out.push("");

fs.writeFileSync(outputPath, out.join("\n"));
```

After creating the file, run:

```bash
chmod +x crates/pi-ai/tools/generate_models.cjs
```

- [ ] **Step 4: Generate `models_generated.rs`**

Run from `pi-rust/`:

```bash
node crates/pi-ai/tools/generate_models.cjs ../pi/packages/ai/src/models.generated.ts crates/pi-ai/src/models_generated.rs
```

Expected: `crates/pi-ai/src/models_generated.rs` exists and starts with `pub fn generated_models() -> Vec<Model>`.

- [ ] **Step 5: Wire `models.rs` to generated data**

Replace the hand-written table in `crates/pi-ai/src/models.rs` with:

```rust
#[path = "models_generated.rs"]
mod models_generated;

use crate::types::{Model, Usage};
use std::collections::BTreeSet;
use std::sync::LazyLock;

pub fn all_models() -> &'static [Model] {
    static MODELS: LazyLock<Vec<Model>> = LazyLock::new(models_generated::generated_models);
    &MODELS
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

pub fn lookup_model(id: &str) -> Option<Model> {
    const PRIORITY: &[&str] = &["anthropic", "openai", "google", "deepseek"];
    for provider in PRIORITY {
        if let Some(model) = get_model(provider, id) {
            return Some(model);
        }
    }
    all_models().iter().find(|model| model.id == id).cloned()
}
```

Keep the `calculate_cost` function from Task 1 below the lookup functions.

- [ ] **Step 6: Re-export lookup APIs**

In `crates/pi-ai/src/lib.rs`, export:

```rust
pub use models::{all_models, calculate_cost, get_model, get_models, get_providers, lookup_model};
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p pi-ai --test model_registry`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/pi-ai/src/models.rs crates/pi-ai/src/models_generated.rs crates/pi-ai/src/lib.rs crates/pi-ai/tools/generate_models.cjs crates/pi-ai/tests/model_registry.rs
git commit -m "feat(pi-ai): generate model registry from TypeScript metadata"
```

---

## Task 3: Expand env-key resolution for M2 providers

**Files:**
- Modify: `crates/pi-ai/src/util/env_keys.rs`
- Create: `crates/pi-ai/tests/env_keys.rs`

- [ ] **Step 1: Add env-key tests**

Create `crates/pi-ai/tests/env_keys.rs`:

```rust
use pi_ai::util::env_keys::env_api_key;

fn with_env_var(name: &str, value: &str, f: impl FnOnce()) {
    unsafe {
        std::env::set_var(name, value);
    }
    f();
    unsafe {
        std::env::remove_var(name);
    }
}

#[test]
fn resolves_openai_google_and_deepseek_keys() {
    with_env_var("OPENAI_API_KEY", "sk-openai", || {
        assert_eq!(env_api_key("openai").as_deref(), Some("sk-openai"));
    });
    with_env_var("GEMINI_API_KEY", "sk-google", || {
        assert_eq!(env_api_key("google").as_deref(), Some("sk-google"));
    });
    with_env_var("DEEPSEEK_API_KEY", "sk-deepseek", || {
        assert_eq!(env_api_key("deepseek").as_deref(), Some("sk-deepseek"));
    });
}

#[test]
fn resolves_openai_compatible_provider_keys() {
    with_env_var("GROQ_API_KEY", "sk-groq", || {
        assert_eq!(env_api_key("groq").as_deref(), Some("sk-groq"));
    });
    with_env_var("XAI_API_KEY", "sk-xai", || {
        assert_eq!(env_api_key("xai").as_deref(), Some("sk-xai"));
    });
    with_env_var("OPENROUTER_API_KEY", "sk-openrouter", || {
        assert_eq!(env_api_key("openrouter").as_deref(), Some("sk-openrouter"));
    });
    with_env_var("AI_GATEWAY_API_KEY", "sk-gateway", || {
        assert_eq!(env_api_key("vercel-ai-gateway").as_deref(), Some("sk-gateway"));
    });
}

#[test]
fn unknown_provider_returns_none() {
    assert_eq!(env_api_key("unknown-provider"), None);
}
```

- [ ] **Step 2: Run and verify failure**

Run: `cargo test -p pi-ai --test env_keys`

Expected: failures for providers not currently mapped.

- [ ] **Step 3: Replace env var mapping**

Use a single match table in `env_keys.rs`:

```rust
fn provider_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "anthropic" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY", "ANTHROPIC_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY", "DEEPSEEK_KEY"],
        "google" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "cerebras" => &["CEREBRAS_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => &["AI_GATEWAY_API_KEY"],
        "zai" => &["ZAI_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "moonshotai" | "moonshotai-cn" => &["MOONSHOT_API_KEY"],
        "huggingface" => &["HF_TOKEN"],
        "fireworks" => &["FIREWORKS_API_KEY"],
        "together" => &["TOGETHER_API_KEY"],
        "opencode" | "opencode-go" => &["OPENCODE_API_KEY"],
        "kimi-coding" => &["KIMI_API_KEY"],
        "cloudflare-workers-ai" | "cloudflare-ai-gateway" => &["CLOUDFLARE_API_KEY"],
        _ => &[],
    }
}
```

Keep `env_api_key(provider)` as the public function and iterate `provider_env_vars(provider)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pi-ai --test env_keys`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pi-ai/src/util/env_keys.rs crates/pi-ai/tests/env_keys.rs
git commit -m "feat(pi-ai): expand provider API key environment lookup"
```

---

## Task 4: Add shared SSE and HTTP retry/timeout utilities

**Files:**
- Create: `crates/pi-ai/src/util/sse.rs`
- Create: `crates/pi-ai/src/util/http.rs`
- Modify: `crates/pi-ai/src/util/mod.rs`
- Modify: `crates/pi-ai/src/types.rs`
- Modify: `crates/pi-ai/src/providers/anthropic/process.rs`
- Modify: `crates/pi-ai/src/providers/anthropic/mod.rs`
- Create: `crates/pi-ai/tests/http_retry.rs`

- [ ] **Step 1: Extend `StreamOptions` tests indirectly**

Add this assertion to an existing `types.rs` test or a new unit test:

```rust
#[test]
fn stream_options_serializes_retry_timeout_fields() {
    let opts = StreamOptions {
        timeout_ms: Some(1500),
        max_retries: Some(2),
        max_retry_delay_ms: Some(10_000),
        ..Default::default()
    };
    let json = serde_json::to_value(opts).unwrap();
    assert_eq!(json["timeoutMs"], 1500);
    assert_eq!(json["maxRetries"], 2);
    assert_eq!(json["maxRetryDelayMs"], 10_000);
}
```

- [ ] **Step 2: Add fields to `StreamOptions`**

In `types.rs`:

```rust
#[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
pub timeout_ms: Option<u64>,
#[serde(rename = "maxRetries", skip_serializing_if = "Option::is_none")]
pub max_retries: Option<u32>,
#[serde(rename = "maxRetryDelayMs", skip_serializing_if = "Option::is_none")]
pub max_retry_delay_ms: Option<u64>,
```

- [ ] **Step 3: Move SSE decoder to shared utility**

Copy the Anthropic SSE decoder from `providers/anthropic/sse.rs` into `util/sse.rs`, preserving the tests and public types. Export it:

```rust
pub mod env_keys;
pub mod http;
pub mod json_repair;
pub mod sse;
```

Update Anthropic `process.rs` imports from `super::sse` to `crate::util::sse`.

- [ ] **Step 4: Add retry helper skeleton and tests**

Create `crates/pi-ai/tests/http_retry.rs` with unit tests against pure policy functions:

```rust
use pi_ai::util::http::{is_retryable_status, parse_retry_after_ms, RetryConfig};

#[test]
fn retryable_statuses_match_provider_policy() {
    for status in [408, 409, 429, 500, 502, 503, 504] {
        assert!(is_retryable_status(status), "{status} should retry");
    }
    for status in [400, 401, 403, 404, 422] {
        assert!(!is_retryable_status(status), "{status} should not retry");
    }
}

#[test]
fn retry_after_respects_delay_cap() {
    let cfg = RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 1000,
    };
    assert_eq!(parse_retry_after_ms(Some("1"), &cfg).unwrap(), 1000);
    assert!(parse_retry_after_ms(Some("5"), &cfg).is_err());
}
```

- [ ] **Step 5: Implement `util/http.rs`**

Add:

```rust
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub timeout_ms: Option<u64>,
    pub max_retry_delay_ms: u64,
}

impl RetryConfig {
    pub fn from_options(opts: Option<&crate::types::StreamOptions>) -> Self {
        Self {
            max_retries: opts.and_then(|o| o.max_retries).unwrap_or(0),
            timeout_ms: opts.and_then(|o| o.timeout_ms),
            max_retry_delay_ms: opts.and_then(|o| o.max_retry_delay_ms).unwrap_or(60_000),
        }
    }
}

pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500..=599)
}
```

Add `parse_retry_after_ms(header, cfg)` for integer-second `Retry-After`. If parsed delay exceeds `cfg.max_retry_delay_ms`, return an error string containing both values.

Providers will use this helper in later tasks; do not change provider retry behavior in this task beyond Anthropic import cleanup.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p pi-ai --test http_retry
cargo test -p pi-ai sse
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-ai/src/types.rs crates/pi-ai/src/util/mod.rs crates/pi-ai/src/util/sse.rs crates/pi-ai/src/util/http.rs crates/pi-ai/src/providers/anthropic/process.rs crates/pi-ai/src/providers/anthropic/mod.rs crates/pi-ai/tests/http_retry.rs
git commit -m "feat(pi-ai): add shared SSE and HTTP retry utilities"
```

---

## Task 5: Implement OpenAI Chat Completions conversion and stream processing

**Files:**
- Create: `crates/pi-ai/src/providers/openai/mod.rs`
- Create: `crates/pi-ai/src/providers/openai/common.rs`
- Create: `crates/pi-ai/src/providers/openai/completions/{mod.rs,convert.rs,process.rs,wire.rs}`
- Create: `crates/pi-ai/tests/openai_completions.rs`
- Create: `crates/pi-ai/tests/fixtures/openai-completions-text-tool.sse`

- [ ] **Step 1: Add a conversion and processing test**

Create `crates/pi-ai/tests/openai_completions.rs`:

```rust
use futures::StreamExt;
use pi_ai::providers::openai::completions::convert::build_request;
use pi_ai::providers::openai::completions::process::process;
use pi_ai::types::{AssistantMessageEvent, ContentBlock, Context, Message, StopReason, Tool};

fn model() -> pi_ai::Model {
    pi_ai::get_model("deepseek", "deepseek-v4-flash").unwrap()
}

#[test]
fn completions_request_maps_context_tools_and_options() {
    let ctx = Context {
        system_prompt: Some("Be concise.".into()),
        messages: vec![
            Message::User {
                content: vec![ContentBlock::Text { text: "Read x".into(), text_signature: None }],
            },
            Message::ToolResult {
                tool_call_id: "call_1".into(),
                tool_name: Some("read".into()),
                is_error: Some(false),
                content: vec![ContentBlock::Text { text: "file text".into(), text_signature: None }],
            },
        ],
        tools: Some(vec![Tool {
            name: "read".into(),
            description: Some("read a file".into()),
            parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}}}),
        }]),
    };
    let opts = Some(pi_ai::StreamOptions {
        max_tokens: Some(128),
        temperature: Some(0.2),
        tool_choice: Some(serde_json::json!("auto")),
        ..Default::default()
    });

    let req = build_request(&model(), &ctx, &opts);
    let json = serde_json::to_value(req).unwrap();
    assert_eq!(json["model"], "deepseek-v4-flash");
    assert_eq!(json["stream"], true);
    assert_eq!(json["stream_options"]["include_usage"], true);
    assert_eq!(json["max_completion_tokens"], 128);
    assert_eq!(json["temperature"], 0.2);
    assert_eq!(json["messages"][0]["role"], "system");
    assert_eq!(json["tools"][0]["type"], "function");
}

#[tokio::test]
async fn completions_fixture_maps_text_tool_usage_and_done() {
    let fixture = include_str!("fixtures/openai-completions-text-tool.sse");
    let chunks = futures::stream::iter(vec![Ok(bytes::Bytes::from(fixture.to_string()))]);
    let mut events = process(chunks, model(), None);
    let mut seen_tool_delta = false;
    let mut terminal = None;

    while let Some(event) = events.next().await {
        if matches!(event, AssistantMessageEvent::ToolcallDelta { .. }) {
            seen_tool_delta = true;
        }
        if matches!(event, AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }) {
            terminal = Some(event);
        }
    }

    assert!(seen_tool_delta);
    match terminal.unwrap() {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(reason, StopReason::ToolUse);
            assert_eq!(message.provider.as_deref(), Some("deepseek"));
            assert_eq!(message.usage.input, 10);
            assert_eq!(message.usage.output, 4);
        }
        other => panic!("expected done, got {other:?}"),
    }
}
```

- [ ] **Step 2: Add fixture**

Create `crates/pi-ai/tests/fixtures/openai-completions-text-tool.sse`:

```text
data: {"id":"chatcmpl_1","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"content":"Use "},"finish_reason":null}]}

data: {"id":"chatcmpl_1","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read","arguments":"{\"path\""}}]},"finish_reason":null}]}

data: {"id":"chatcmpl_1","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"src/lib.rs\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}}

data: [DONE]

```

- [ ] **Step 3: Run and verify failure**

Run: `cargo test -p pi-ai --test openai_completions`

Expected: compile failure because OpenAI modules do not exist.

- [ ] **Step 4: Implement wire structs**

In `wire.rs`, define serde structs for:

- `ChatCompletionRequest`
- `ChatMessage`
- `ChatContentPart`
- `ChatTool`
- `ChatCompletionChunk`
- `ChoiceDelta`
- `ToolCallDelta`
- `ChatUsage`

Use `serde_json::Value` for provider-specific optional fields such as `tool_choice` and partial tool arguments.

- [ ] **Step 5: Implement `convert.rs`**

`build_request(model, ctx, opts)` must:

- resolve completions compatibility with `openai::common::resolve_completions_compat(model)`, using provider/base-url detection first and generated `model.compat` as overrides;
- use role `developer` when `model.reasoning` and resolved compat `supportsDeveloperRole` is true; otherwise `system`;
- map user text to string content and images to `image_url` data URLs;
- map assistant text and tool-call blocks to assistant messages with `tool_calls`;
- map tool results to role `tool` with `tool_call_id`;
- map tools to OpenAI function tools with `strict: false` unless compat `supportsStrictMode` is false;
- use `max_completion_tokens` by default and `max_tokens` when compat says `maxTokensField: "max_tokens"`;
- include `stream_options: {"include_usage": true}` unless compat `supportsUsageInStreaming` is false.

- [ ] **Step 6: Implement `process.rs`**

`process(body_stream, model, cancel)` must:

- use `crate::util::sse` to parse events;
- create `AssistantMessage::empty("openai-completions", &model.id)` and set `provider`;
- emit `Start`;
- accumulate text, thinking, and tool-call blocks;
- parse tool-call arguments with `crate::util::json_repair::parse_streaming_json`;
- map finish reasons with the table in the spec;
- map usage with cache token subtraction;
- emit exactly one terminal `Done` or `Error`.

- [ ] **Step 7: Run tests**

Run: `cargo test -p pi-ai --test openai_completions`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/pi-ai/src/providers/openai crates/pi-ai/tests/openai_completions.rs crates/pi-ai/tests/fixtures/openai-completions-text-tool.sse
git commit -m "feat(pi-ai): process OpenAI chat completions streams offline"
```

---

## Task 6: Add OpenAI Chat Completions HTTP provider

**Files:**
- Modify: `crates/pi-ai/src/providers/openai/completions/mod.rs`
- Modify: `crates/pi-ai/src/providers/openai/mod.rs`
- Modify: `crates/pi-ai/src/providers/mod.rs`
- Modify: `crates/pi-ai/tests/openai_completions.rs`

- [ ] **Step 1: Add missing-key and registration tests**

Append to `openai_completions.rs`:

```rust
use futures::StreamExt;
use pi_ai::registry::{lookup, ApiProvider};

#[tokio::test]
async fn completions_provider_missing_key_returns_error_event() {
    let provider = pi_ai::providers::openai::completions::OpenAICompletionsProvider::new(None);
    let mut stream = provider.stream(
        &model(),
        Context { system_prompt: None, messages: vec![], tools: None },
        None,
    );
    let event = stream.next().await.unwrap();
    assert!(matches!(event, AssistantMessageEvent::Error { reason: StopReason::Error, .. }));
}

#[test]
fn builtins_register_openai_completions_api() {
    pi_ai::providers::register_builtins();
    assert!(lookup("openai-completions").is_some());
}
```

- [ ] **Step 2: Run and verify failure**

Run: `cargo test -p pi-ai --test openai_completions`

Expected: compile failure for missing provider type or failed registration assertion.

- [ ] **Step 3: Implement provider wrapper**

`OpenAICompletionsProvider` should mirror `AnthropicProvider`:

- `new(api_key: Option<String>) -> Self`
- resolve API key from `opts.api_key`, provider override, then `env_api_key(&model.provider)`
- on missing key, return terminal `Error` that names the provider
- build `POST {base_url}/chat/completions`
- set `Authorization: Bearer <key>`, `content-type: application/json`, `accept: text/event-stream`
- merge `model.headers` then `opts.headers`
- pass response byte stream to `completions::process::process`
- apply `timeout_ms` and retry policy from `util::http`
- check cancellation before each attempt and while forwarding processed events

- [ ] **Step 4: Register built-in**

In `providers/mod.rs`:

```rust
pub mod openai;

registry::register(
    "openai-completions",
    Arc::new(openai::completions::OpenAICompletionsProvider::new(None)),
);
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p pi-ai --test openai_completions`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-ai/src/providers/openai crates/pi-ai/src/providers/mod.rs crates/pi-ai/tests/openai_completions.rs
git commit -m "feat(pi-ai): register OpenAI chat completions provider"
```

---

## Task 7: Implement OpenAI Responses conversion, stream processing, and provider

**Files:**
- Create: `crates/pi-ai/src/providers/openai/responses/{mod.rs,convert.rs,process.rs,wire.rs}`
- Modify: `crates/pi-ai/src/providers/openai/mod.rs`
- Modify: `crates/pi-ai/src/providers/mod.rs`
- Create: `crates/pi-ai/tests/openai_responses.rs`
- Create: `crates/pi-ai/tests/fixtures/openai-responses-text-tool.sse`

- [ ] **Step 1: Add fixture**

Create `crates/pi-ai/tests/fixtures/openai-responses-text-tool.sse`:

```text
event: response.created
data: {"type":"response.created","response":{"id":"resp_1"}}

event: response.output_item.added
data: {"type":"response.output_item.added","item":{"id":"msg_1","type":"message","role":"assistant","content":[]}}

event: response.content_part.added
data: {"type":"response.content_part.added","part":{"type":"output_text","text":"","annotations":[]}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"hello"}

event: response.output_item.done
data: {"type":"response.output_item.done","item":{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"hello","annotations":[]}],"status":"completed"}}

event: response.output_item.added
data: {"type":"response.output_item.added","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"read","arguments":""}}

event: response.function_call_arguments.delta
data: {"type":"response.function_call_arguments.delta","delta":"{\"path\":\"Cargo.toml\"}"}

event: response.output_item.done
data: {"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"read","arguments":"{\"path\":\"Cargo.toml\"}"}}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","status":"completed","usage":{"input_tokens":12,"output_tokens":5,"total_tokens":17,"input_tokens_details":{"cached_tokens":2}}}}

```

- [ ] **Step 2: Add tests**

Create `crates/pi-ai/tests/openai_responses.rs` with:

- `responses_request_maps_context_tools_and_options`
- `responses_fixture_maps_text_tool_usage_and_done`
- `responses_provider_missing_key_returns_error_event`
- `builtins_register_openai_responses_api`

Use `pi_ai::get_model("openai", "gpt-4.1")` and assert:

```rust
assert_eq!(json["model"], "gpt-4.1");
assert_eq!(json["stream"], true);
assert_eq!(json["store"], false);
assert_eq!(json["tools"][0]["type"], "function");
assert_eq!(reason, StopReason::ToolUse);
assert_eq!(message.response_id.as_deref(), Some("resp_1"));
assert_eq!(message.usage.input, 10);
assert_eq!(message.usage.cache_read, 2);
```

- [ ] **Step 3: Run and verify failure**

Run: `cargo test -p pi-ai --test openai_responses`

Expected: compile failure because Responses modules do not exist.

- [ ] **Step 4: Implement Responses modules**

Implement:

- `wire.rs` with `ResponseCreateRequest`, `ResponseInputItem`, `ResponseTool`, and `ResponseStreamEvent`.
- `convert.rs` with `build_request(model, ctx, opts)` mirroring TS `convertResponsesMessages` for text, image, assistant text, assistant tool calls, and tool results.
- `process.rs` with event handling for `response.created`, `response.output_item.added`, text deltas, reasoning deltas, function-call deltas/done, `response.completed`, `response.failed`, and `error`.
- `mod.rs` with `OpenAIResponsesProvider` using `/responses`, bearer auth, custom headers, retry/timeout, cancellation, and missing-key error events.

- [ ] **Step 5: Register provider**

In `providers/mod.rs`:

```rust
registry::register(
    "openai-responses",
    Arc::new(openai::responses::OpenAIResponsesProvider::new(None)),
);
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p pi-ai --test openai_responses`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-ai/src/providers/openai crates/pi-ai/src/providers/mod.rs crates/pi-ai/tests/openai_responses.rs crates/pi-ai/tests/fixtures/openai-responses-text-tool.sse
git commit -m "feat(pi-ai): add OpenAI responses provider"
```

---

## Task 8: Implement Google Generative AI conversion, stream processing, and provider

**Files:**
- Create: `crates/pi-ai/src/providers/google/{mod.rs,convert.rs,process.rs,wire.rs}`
- Modify: `crates/pi-ai/src/providers/mod.rs`
- Create: `crates/pi-ai/tests/google.rs`
- Create: `crates/pi-ai/tests/fixtures/google-text-tool.sse`

- [ ] **Step 1: Add fixture**

Create `crates/pi-ai/tests/fixtures/google-text-tool.sse`:

```text
data: {"responseId":"google_resp_1","candidates":[{"content":{"parts":[{"text":"thinking","thought":true},{"functionCall":{"id":"call_1","name":"read","args":{"path":"Cargo.toml"}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":11,"cachedContentTokenCount":1,"candidatesTokenCount":3,"thoughtsTokenCount":2,"totalTokenCount":16}}

```

- [ ] **Step 2: Add tests**

Create `crates/pi-ai/tests/google.rs` with:

- `google_request_maps_context_tools_and_options`
- `google_fixture_maps_thinking_tool_usage_and_done`
- `google_provider_missing_key_returns_error_event`
- `builtins_register_google_api`

Assert request fields:

```rust
assert_eq!(request.model, "gemini-2.5-flash");
let json = serde_json::to_value(&request.body).unwrap();
assert_eq!(json["config"]["maxOutputTokens"], 128);
assert_eq!(json["config"]["temperature"], 0.2);
assert_eq!(json["config"]["tools"][0]["functionDeclarations"][0]["name"], "read");
assert_eq!(json["config"]["toolConfig"]["functionCallingConfig"]["mode"], "AUTO");
```

Assert processed event terminal:

```rust
assert_eq!(reason, StopReason::ToolUse);
assert_eq!(message.response_id.as_deref(), Some("google_resp_1"));
assert_eq!(message.provider.as_deref(), Some("google"));
assert_eq!(message.usage.input, 10);
assert_eq!(message.usage.cache_read, 1);
assert_eq!(message.usage.output, 5);
```

- [ ] **Step 3: Run and verify failure**

Run: `cargo test -p pi-ai --test google`

Expected: compile failure because Google modules do not exist.

- [ ] **Step 4: Implement Google modules**

Implement:

- `wire.rs` with Gemini request/response structs using serde rename rules.
- `convert.rs` mapping:
  - return `GenerateContentRequest { model, body }`, where `model` is used for the URL
    and only `body` is serialized into the HTTP request;
  - user text/images to `Content { role: "user", parts }`;
  - assistant text/thinking/tool calls to `role: "model"`;
  - tool results to `functionResponse`;
  - tools to `functionDeclarations`;
  - tool choice `"auto"`, `"none"`, `"any"` to `"AUTO"`, `"NONE"`, `"ANY"`;
  - `StreamOptions.thinking` to `thinkingConfig`.
- `process.rs` mapping:
  - `part.text` with `thought == true` to thinking events;
  - other `part.text` to text events;
  - `part.functionCall` to tool-call start/delta/end;
  - `finishReason` to `StopReason`;
  - `usageMetadata` to `Usage`.
- `mod.rs` with `GoogleGenerativeAiProvider` using:
  - URL `{base_url}/models/{model.id}:streamGenerateContent?alt=sse&key=<key>`;
  - API key from `opts.api_key`, provider override, then `env_api_key("google")`;
  - retry/timeout helper;
  - terminal error events.

- [ ] **Step 5: Register provider**

In `providers/mod.rs`:

```rust
pub mod google;

registry::register(
    "google-generative-ai",
    Arc::new(google::GoogleGenerativeAiProvider::new(None)),
);
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p pi-ai --test google`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/pi-ai/src/providers/google crates/pi-ai/src/providers/mod.rs crates/pi-ai/tests/google.rs crates/pi-ai/tests/fixtures/google-text-tool.sse
git commit -m "feat(pi-ai): add Google Generative AI provider"
```

---

## Task 9: Update downstream model literals and DeepSeek legacy tests

**Files:**
- Modify: `crates/pi-agent-core/examples/loop_example.rs`
- Modify: `crates/pi-agent-core/tests/agent_loop.rs`
- Modify: `crates/pi-coding-agent/examples/manual_test.rs`
- Modify: `crates/pi-coding-agent/tests/{cli.rs,print_mode.rs,public_api.rs,tools_e2e.rs}`
- Modify: `crates/pi-ai/examples/faux_stream.rs`
- Modify: `crates/pi-ai/tests/{anthropic_mapping.rs,deepseek.rs,faux.rs,request_building.rs,cost.rs}`

- [ ] **Step 1: Run workspace tests and collect compile errors**

Run: `cargo test --workspace`

Expected: compile failures at remaining `Model` literals that still use flattened cost fields or `max_tokens: Some(...)`.

- [ ] **Step 2: Update all dummy model literals**

Use the Task 1 model literal pattern:

- `input: vec![ModelInput::Text]`
- `cost: ModelCost::default()` for faux/test providers
- `max_tokens: 4096` instead of `Some(4096)`
- `thinking_level_map: None`
- `compat: None`

- [ ] **Step 3: Update cost assertions**

In `crates/pi-ai/tests/cost.rs` and `models.rs` unit tests, read nested costs:

```rust
assert!((usage.cost.input - 1.0).abs() < 0.01);
assert!((usage.cost.output - 5.0).abs() < 0.01);
```

Use generated registry models:

- `claude-haiku-4-5` for Anthropic low-cost cache tests when present.
- `gpt-4.1` for OpenAI cost tests.
- `deepseek-v4-flash` for OpenAI-compatible DeepSeek cost tests.

- [ ] **Step 4: Adjust DeepSeek tests for canonical registry**

`lookup_model("deepseek-v4-flash")` should now return `api: "openai-completions"` from TS metadata. For tests that exercise the legacy `DeepSeekProvider`, construct the model explicitly:

```rust
fn legacy_deepseek_model() -> Model {
    let mut model = pi_ai::get_model("deepseek", "deepseek-v4-flash").unwrap();
    model.api = "deepseek-chat-completions".into();
    model
}
```

Use `legacy_deepseek_model()` only in tests that call `pi_ai::providers::deepseek::*`.

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-ai crates/pi-agent-core crates/pi-coding-agent
git commit -m "chore: migrate workspace tests to generated model metadata"
```

---

## Task 10: End-to-end provider registry smoke tests

**Files:**
- Modify: `crates/pi-ai/tests/model_registry.rs`
- Modify: `crates/pi-ai/tests/openai_completions.rs`
- Modify: `crates/pi-ai/tests/openai_responses.rs`
- Modify: `crates/pi-ai/tests/google.rs`

- [ ] **Step 1: Add registry provider presence test**

In `model_registry.rs`:

```rust
#[test]
fn m2_provider_apis_are_registered_by_builtins() {
    pi_ai::providers::register_builtins();
    for api in [
        "anthropic-messages",
        "deepseek-chat-completions",
        "openai-completions",
        "openai-responses",
        "google-generative-ai",
    ] {
        assert!(pi_ai::registry::lookup(api).is_some(), "{api} was not registered");
    }
}
```

- [ ] **Step 2: Add `complete()` smoke tests over fixture processors**

For each provider test file, add a test that drains the provider fixture stream with
`pi_ai::complete(stream).await` and asserts the terminal `AssistantMessage`.

OpenAI completions expected:

```rust
assert_eq!(message.stop_reason, StopReason::ToolUse);
assert!(message.content.iter().any(|block| matches!(block, ContentBlock::ToolCall { name, .. } if name == "read")));
```

OpenAI responses expected:

```rust
assert_eq!(message.response_id.as_deref(), Some("resp_1"));
assert_eq!(message.stop_reason, StopReason::ToolUse);
```

Google expected:

```rust
assert_eq!(message.response_id.as_deref(), Some("google_resp_1"));
assert_eq!(message.stop_reason, StopReason::ToolUse);
```

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test -p pi-ai --test model_registry
cargo test -p pi-ai --test openai_completions
cargo test -p pi-ai --test openai_responses
cargo test -p pi-ai --test google
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/pi-ai/tests
git commit -m "test(pi-ai): add M2 provider registry smoke coverage"
```

---

## Task 11: Final verification

**Files:** No planned source edits.

- [ ] **Step 1: Format**

Run: `cargo fmt --check`

Expected: PASS.

- [ ] **Step 2: Test workspace**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 3: Check workspace**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 4: Regeneration determinism check**

Run:

```bash
node crates/pi-ai/tools/generate_models.cjs ../pi/packages/ai/src/models.generated.ts /tmp/pi-ai-models-generated-check.rs
diff -u crates/pi-ai/src/models_generated.rs /tmp/pi-ai-models-generated-check.rs
```

Expected: no diff.

- [ ] **Step 5: Inspect final diff**

Run: `git diff --stat`

Expected: changes are limited to `pi-ai` provider/model work plus required downstream model literal migration.

- [ ] **Step 6: Commit verification fixes if any**

If formatting or test fixes were needed:

```bash
git add -A
git commit -m "fix(pi-ai): finish M2 provider verification"
```

If no fixes were needed, do not create an empty commit.
