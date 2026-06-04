# Design: Rust `pi-ai` M2 provider breadth and model registry

- Date: 2026-06-04
- Status: Draft (pending review)
- Scope: M2 of `ROADMAP.md` - widen `pi-ai` beyond Anthropic/DeepSeek PoC and replace the hand-written model table.
- Depends on: `pi-ai` PoC provider abstraction, existing SSE parser, faux provider tests.

## 1. Context

The current Rust `pi-ai` crate has the right core shape: wire-compatible message/event
types, a process-global provider registry keyed by `model.api`, a working Anthropic
provider, a faux provider, and a small DeepSeek path. The model table is still hand-written
and currently encodes token prices directly as `Model.input` / `Model.output`, which does
not match TypeScript `pi`: TS uses `input: ["text", "image"]` for model modalities and
`cost.{input,output,cacheRead,cacheWrite}` for prices.

M2 should make `pi-ai` useful with mainstream non-Anthropic providers while correcting the
model metadata shape before downstream crates build more assumptions on the current PoC
fields. The TypeScript reference is:

- `pi/packages/ai/src/models.ts`
- `pi/packages/ai/src/models.generated.ts`
- `pi/packages/ai/src/env-api-keys.ts`
- `pi/packages/ai/src/providers/openai-completions.ts`
- `pi/packages/ai/src/providers/openai-responses.ts`
- `pi/packages/ai/src/providers/openai-responses-shared.ts`
- `pi/packages/ai/src/providers/google.ts`
- `pi/packages/ai/src/providers/google-shared.ts`

## 2. Goals and success criteria

Build a second `pi-ai` vertical slice that supports OpenAI Chat Completions, OpenAI
Responses, Google Generative AI, a generated model registry, broader env-key resolution,
and shared HTTP retry/timeout controls.

Done when:

1. `cargo fmt --check`, `cargo test -p pi-ai`, `cargo test --workspace`, and
   `cargo check --workspace` pass from `pi-rust/`.
2. Built-in providers registered by `pi_ai::providers::register_builtins()` include:
   - `anthropic-messages`
   - `deepseek-chat-completions` as a legacy Rust provider alias
   - `openai-completions`
   - `openai-responses`
   - `google-generative-ai`
3. `Model` serde matches the TS generated model shape:
   - `input` serializes as model modalities (`["text"]`, `["text","image"]`).
   - token prices serialize under `cost`.
   - `thinkingLevelMap`, `headers`, and `compat` round-trip when present.
4. The generated Rust registry includes the TS model registry data relevant to M2 and keeps
   a repeatable regeneration path. At minimum the registry covers Anthropic, OpenAI,
   Google, and DeepSeek models from `models.generated.ts`; the generator may include all TS
   providers even when their provider implementation remains out of scope.
5. `lookup_model("gpt-4.1")`, `lookup_model("gpt-5")` when present upstream,
   `lookup_model("gemini-2.5-flash")`, `lookup_model("deepseek-v4-flash")`, and
   `lookup_model("claude-sonnet-4-5")` return TS-shaped metadata with the correct `api`
   and `provider`.
6. Offline fixture tests prove that at least one model for each new API can be consumed via
   `complete()` without live provider keys:
   - `openai-completions`: streaming text, tool call, finish reason, and usage.
   - `openai-responses`: streamed text, function call, reasoning summary, finish status,
     and usage.
   - `google-generative-ai`: streamed text, function call, thinking, finish reason, and
     usage.
7. Provider network errors, missing API keys, HTTP errors, JSON/SSE parse failures,
   cancellation, timeout, and exhausted retries terminate through the event protocol as
   `AssistantMessageEvent::Error`, preserving the existing no-throw stream contract.

No live provider tests are required. All tests must be deterministic and offline.

## 3. Non-goals

- OAuth, Claude Code identity, GitHub Copilot auth, Azure auth, Google Vertex ADC, AWS
  Bedrock SigV4, Cloudflare gateway auth, and browser/Vite compatibility.
- New providers beyond the M2 set: Mistral, Azure OpenAI Responses, OpenAI Codex Responses,
  Bedrock, Google Vertex, Cloudflare Workers AI, OpenRouter image generation, and image
  generation APIs.
- Full TS compatibility matrix for every OpenAI-compatible provider. M2 stores generated
  `compat` metadata and implements only the compatibility flags needed by the M2 fixture
  set and major OpenAI/DeepSeek paths.
- Full prompt caching/session-affinity behavior. M2 keeps `cacheRetention` metadata in the
  model table but only implements retry/timeout and custom headers from `StreamOptions`.
- Live network validation or credentials in CI.

## 4. Approach

### Recommended approach: raw HTTP providers plus generated Rust registry

Use `reqwest` directly for OpenAI and Google instead of adding provider SDK crates. The TS
implementation uses SDKs, but Rust can keep the crate small, make SSE processing testable,
and reuse the no-throw event contract by separating each provider into:

- `convert.rs` - `Context` and `StreamOptions` to provider request JSON.
- `wire.rs` - request/response/event structs.
- `process.rs` - provider stream/fixture JSON to `AssistantMessageEvent`.
- `mod.rs` - thin HTTP wrapper with auth, headers, retry, timeout, and cancellation.

Generate a committed `models_generated.rs` file from the TS `models.generated.ts` reference
with a manual script under `crates/pi-ai/tools/`. Build and tests should not depend on
Node.js or on the sibling `pi/` checkout; regeneration is an explicit maintainer action.

### Rejected alternatives

1. Port all TS providers in one M2. This would mix provider breadth, auth, SDK behavior, and
   model registry churn in one large change. It does not match the roadmap's "priority"
   wording and would delay usable OpenAI/Google support.
2. Keep the current hand-written model table and only add providers. This would make the
   new providers usable for a few hard-coded models but leave the main M2 risk
   unresolved: model metadata drift.
3. Depend on Node.js in `build.rs` to parse TS at compile time. That makes `pi-rust`
   non-standalone and introduces a build dependency on the sibling TypeScript repo.

## 5. Scope

### In scope

- `Model` shape migration:
  - add `ModelInput` and `ModelCost`.
  - move token prices from flattened `Model.input/output/cache_read/cache_write` to
    `Model.cost`.
  - add `thinking_level_map: Option<serde_json::Value>` and
    `compat: Option<serde_json::Value>`.
  - keep `headers` as model metadata and serialize it as TS `headers`.
- Model registry:
  - add `get_model(provider, id)`, `get_models(provider)`, `get_providers()`.
  - keep `lookup_model(id)` for existing callers, with deterministic provider-order search.
  - add a committed generated source file and a repeatable generator script.
- Shared provider utilities:
  - move or copy the existing SSE line decoder into a shared utility module.
  - add `util::http` retry/timeout helper for reqwest providers.
  - extend `StreamOptions` with `timeout_ms`, `max_retries`, and `max_retry_delay_ms`.
  - extend `env_api_key` provider mappings for M2 providers.
- New providers:
  - `openai-completions` using `/chat/completions` streaming SSE.
  - `openai-responses` using `/responses` streaming SSE.
  - `google-generative-ai` using Gemini REST `:streamGenerateContent?alt=sse`.
- Offline fixture tests for conversion, stream processing, env keys, retry/timeout, and
  registry behavior.

### Out of scope

See Section 3.

## 6. Architecture

### 6.1 Crate layout

```text
crates/pi-ai/
  Cargo.toml
  tools/
    generate_models.cjs
  src/
    lib.rs
    types.rs
    models.rs
    models_generated.rs
    registry.rs
    stream.rs
    util/
      mod.rs
      env_keys.rs
      http.rs
      json_repair.rs
      sse.rs
    providers/
      mod.rs
      faux.rs
      anthropic/
      deepseek/
      openai/
        mod.rs
        common.rs
        completions/
          mod.rs
          convert.rs
          process.rs
          wire.rs
        responses/
          mod.rs
          convert.rs
          process.rs
          wire.rs
      google/
        mod.rs
        convert.rs
        process.rs
        wire.rs
  tests/
    model_registry.rs
    env_keys.rs
    http_retry.rs
    openai_completions.rs
    openai_responses.rs
    google.rs
    fixtures/
      openai-completions-text-tool.sse
      openai-responses-text-tool.sse
      google-text-tool.sse
```

### 6.2 Model metadata

`Model` becomes TS-shaped while staying idiomatic internally:

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

`calculate_cost(model, usage)` reads rates from `model.cost`. Existing test fixtures and
downstream dummy models must be migrated with a helper constructor in tests to avoid
repeating model literals.

### 6.3 Model registry

`models.rs` owns lookup behavior and includes generated data:

- `all_models() -> &'static [Model]`
- `get_model(provider: &str, id: &str) -> Option<Model>`
- `get_models(provider: &str) -> Vec<Model>`
- `get_providers() -> Vec<String>`
- `lookup_model(id: &str) -> Option<Model>`

`lookup_model` remains for existing Rust callers and searches in a deterministic priority:
`anthropic`, `openai`, `google`, `deepseek`, then lexical provider order. `get_model` is the
preferred API when a provider is known.

The generator script reads `pi/packages/ai/src/models.generated.ts`, removes TS-only
syntax such as `satisfies Model<...>`, evaluates the object in a Node `vm` context, and
writes Rust constructors into `src/models_generated.rs`. The generated file is committed;
Cargo does not run the script automatically.

Generated data should preserve `compat`, `headers`, and `thinkingLevelMap` as
`serde_json::Value`, so M2 does not need to type every TS compatibility option before the
providers need it.

### 6.4 Shared HTTP policy

Add a small retry helper around `reqwest::RequestBuilder` or a closure that creates a fresh
request for each attempt:

- `timeout_ms`: per-attempt request timeout.
- `max_retries`: retry count after the initial attempt, default `0` to preserve PoC
  behavior.
- `max_retry_delay_ms`: cap for server `Retry-After`, default `60_000`.

Retry only:

- transport/connect errors when the request body can be recreated,
- HTTP 408, 409, 429, and 5xx.

Do not retry:

- 4xx other than 408/409/429,
- cancellation,
- JSON/SSE parse failures after a response body starts.

Errors are converted by providers into terminal `AssistantMessageEvent::Error`.

### 6.5 OpenAI Chat Completions provider

API key:

- provider `openai` uses `OPENAI_API_KEY`.
- OpenAI-compatible generated models use their provider env var where present, for example
  `DEEPSEEK_API_KEY`, `XAI_API_KEY`, `GROQ_API_KEY`, and `OPENROUTER_API_KEY`.

Request:

- `POST {model.base_url}/chat/completions`
- `Authorization: Bearer <api_key>`
- `stream: true`
- `stream_options: { include_usage: true }` unless compat disables it
- `messages`: system/developer prompt, user text/images, assistant text/thinking/tool calls,
  and tool results
- `tools`: JSON-schema functions
- `tool_choice`, `temperature`, and `max_tokens`/`max_completion_tokens`

Processing:

- SSE `data: [DONE]` terminates body consumption.
- `choices[0].delta.content` maps to text events.
- `reasoning_content`, `reasoning`, and `reasoning_text` map to thinking events.
- `choices[0].delta.tool_calls[*].function.arguments` accumulates with the existing
  streaming JSON parser.
- `finish_reason` maps `stop -> Stop`, `length -> Length`,
  `function_call/tool_calls -> ToolUse`, and content-filter/provider errors to `Error`.
- usage maps OpenAI cache tokens to `Usage.input/cache_read/cache_write/output`.

### 6.6 OpenAI Responses provider

Request:

- `POST {model.base_url}/responses`
- `Authorization: Bearer <api_key>`
- `stream: true`
- `store: false`
- `input`: Responses item list converted from pi messages
- `tools`: function tools
- `temperature`, `max_output_tokens`, and reasoning options when enabled

Processing:

- `response.created` captures response id.
- `response.output_item.added` starts text, thinking, or tool-call blocks.
- `response.output_text.delta` maps to text deltas.
- `response.reasoning_summary_text.delta` and `response.reasoning_text.delta` map to
  thinking deltas.
- `response.function_call_arguments.delta` accumulates tool-call JSON.
- `response.output_item.done` finalizes block signatures and parsed arguments.
- `response.completed` maps usage and stop reason.
- `response.failed` and `error` terminate as `Error`.

### 6.7 Google Generative AI provider

Request:

- `POST {model.base_url}/models/{model.id}:streamGenerateContent?alt=sse&key=<api_key>`
- `contents`: user/model/tool-result turns converted to Gemini `Content`.
- `systemInstruction`: sanitized system prompt.
- `tools`: function declarations.
- `toolConfig`: function calling mode for `auto`, `none`, or `any`.
- `generationConfig`: `temperature`, `maxOutputTokens`.
- `thinkingConfig`: `includeThoughts`, `thinkingBudget`, or disabled thinking settings for
  supported Gemini models.

Processing:

- text parts map to text events unless `thought: true`, which maps to thinking events.
- `functionCall` parts map to a full tool-call block with JSON arguments.
- `finishReason` maps `STOP -> Stop`, `MAX_TOKENS -> Length`, safety/blocklist/prohibited
  reasons to `Error`, and tool calls force `ToolUse`.
- `usageMetadata` maps prompt, cached, candidate, and thought token counts into `Usage`.

## 7. Error handling

All new providers keep the existing `ApiProvider::stream` contract:

- Missing key: terminal `Error` with provider-specific message naming the env var.
- Unknown provider API: existing registry terminal `Error`.
- HTTP failure: terminal `Error` with status and body.
- Network failure after retry exhaustion: terminal `Error` with attempt count and last error.
- Timeout: terminal `Error` with `Request timed out after N ms`.
- Cancellation before or during request: terminal `Error` with `StopReason::Aborted`.
- Provider stream parse failure: terminal `Error`; do not panic.

## 8. Testing strategy

All tests are offline.

- Model tests:
  - TS-shaped serde for `Model`.
  - registry lookup for Anthropic, OpenAI, Google, and DeepSeek.
  - cost calculation reads nested `cost`.
  - generated registry contains no duplicate `(provider, id)` pairs.
- Env-key tests:
  - OpenAI, Google, DeepSeek, Anthropic, Groq, xAI, OpenRouter, Vercel AI Gateway, Mistral.
  - unknown provider returns `None`.
- HTTP retry tests:
  - retryable status retries then succeeds.
  - non-retryable 400 does not retry.
  - `Retry-After` exceeding cap returns an error.
  - timeout produces a provider error event.
- Provider conversion tests:
  - OpenAI completions request with text, image, tools, tool results, max tokens, temperature.
  - OpenAI responses request with text, image, tools, tool results, reasoning.
  - Google request with system prompt, image input, function declarations, tool result, thinking.
- Provider processing tests:
  - fixture SSE to exact event sequence and terminal message.
  - tool-call arguments are parsed through streaming JSON.
  - usage cost is calculated.
  - error events map to terminal `Error`.

## 9. Migration notes

- `DeepSeekProvider` remains registered under `deepseek-chat-completions` to avoid deleting
  working PoC code. Generated DeepSeek models from TS use `api: "openai-completions"`,
  so tests for the legacy DeepSeek provider should construct an explicit model with the
  legacy API instead of relying on `lookup_model`.
- Existing model literals across `pi-ai`, `pi-agent-core`, and `pi-coding-agent` must be
  updated to the TS-shaped `Model`.
- `pi-coding-agent` default model lookup should keep working because
  `claude-sonnet-4-5` remains in the generated registry.
- The generated registry may include providers without Rust implementations. Calling
  `stream_model` for such a model should return the existing unknown-provider error until a
  later milestone registers the provider.
