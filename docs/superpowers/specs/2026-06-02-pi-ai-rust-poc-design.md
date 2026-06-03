# Design: Rust port of `pi-ai` core (proof-of-concept)

- Date: 2026-06-02
- Status: Approved (pending spec review)
- Scope: One foundational crate as a proof-of-concept for porting the `pi` monorepo to Rust.

## 1. Context

`pi` is a coding-agent harness (a TypeScript monorepo, ~110K LOC) consisting of four
published packages:

| Package | LOC | Role | Depends on |
|---|---|---|---|
| `@earendil-works/pi-ai` | ~30.5K (16.3K generated) | Unified multi-provider LLM API | — |
| `@earendil-works/pi-agent-core` | ~8K | Agent runtime: tool-calling loop, state | `pi-ai` |
| `@earendil-works/pi-tui` | ~11.7K | Terminal UI with differential rendering | — |
| `@earendil-works/pi-coding-agent` | ~48K | The interactive `pi` CLI | all three |

A full port is a multi-phase effort. To de-risk it, this project ports a single
foundational crate first as a proof-of-concept: the **core of `pi-ai`** plus **one
real provider (Anthropic)** and the **faux provider**. `pi-ai` is the dependency root
(everything else needs it) and exercises the hardest Rust-porting concerns: async SSE
streaming, `serde` JSON round-tripping, trait-based provider polymorphism, and HTTP.

The Rust workspace already exists at `pi-rust/` (a git repo) with empty `cargo new`
stubs for seven crates. This PoC fills in `crates/pi-ai` only; the other stubs are
left untouched and the workspace must keep building.

## 2. Goals & success criteria

Build a `pi-ai` Rust crate that exposes `stream()` / `complete()` over a provider
abstraction, with a working Anthropic provider and a faux provider.

The PoC is **done** when:

1. `cargo build -p pi-ai` and the whole workspace build cleanly on Rust 1.96 / edition 2024.
2. The offline test suite passes with **no network access and no credentials**:
   - SSE decoder unit tests.
   - Streaming-JSON repair / partial-parse unit tests.
   - Anthropic event-mapping tests driven by raw-SSE fixtures (text + thinking + tool_use).
   - Request-building tests (Context -> Anthropic request JSON).
   - Cost-calculation tests.
   - Faux-provider end-to-end test exercising the full streaming pipeline.
   - Serde round-trip tests confirming wire-compatible JSON.
3. An optional offline example (`examples/faux_stream.rs`) consumes the event stream
   and prints deltas, using the faux provider (no key required).

Non-goal for this PoC: live calls to the real Anthropic API. The Anthropic provider's
network path is implemented and structured for testability, but verified offline via
fixtures rather than live requests.

## 3. Key decisions

These were settled during brainstorming and drive the rest of the design:

- **PoC target:** `pi-ai` core + one provider (Anthropic) + faux provider.
- **Streaming shape (idiomatic-first):** providers return an `impl Stream` of events
  built with `async-stream` — no channels, no spawned task, pull-based. The dropped
  `.result()` method (present in the TS `EventStream`) is replaced by: the terminal
  `Done`/`Error` event carries the final `AssistantMessage`, and a free
  `complete()` consumes the stream to return it.
- **Protocol fidelity:** the *event protocol* and *wire JSON* stay faithful to pi.
  Rust code is idiomatic (snake_case fields, enums); `serde` attributes bridge to pi's
  exact JSON (tagged unions, camelCase) so serialized values match pi byte-for-byte.
  This is near-free and de-risks future interop (pi session files, the next phases).
- **Auth scope:** API-key path only (`x-api-key` + `anthropic-version`). OAuth /
  Claude-Code identity / Copilot / Cloudflare / Bedrock are out.
- **Tests:** offline only; correctness proven via faux provider + fixtures + unit tests.
- **Edition:** 2024 (confirmed supported by installed Rust 1.96.0).

## 4. Scope

### In scope
- Core types (content blocks, messages, events, model, options, usage, stop reason).
- Idiomatic streaming (`EventStream`, `stream()`, `complete()`), with cancellation.
- Provider trait + registry keyed by `api`.
- Anthropic provider (API-key path):
  - Request building: system prompt (with optional `cache_control`), messages
    (user / assistant / toolResult conversion, **consecutive tool-result coalescing**),
    tools (`input_schema`), `max_tokens`, `temperature` gating, basic thinking
    (enabled / budget / adaptive effort), `tool_choice`, image input, tool-call id
    normalization.
  - SSE parsing (ported from pi's decoder).
  - Event mapping for all block types: `text`, `thinking`, `redacted_thinking`, `tool_use`.
  - Usage accumulation + cost calculation; stop-reason mapping.
- Streaming-JSON repair + partial-parse (for incremental tool-call arguments).
- Env-key resolution (`ANTHROPIC_API_KEY` and common aliases).
- A small hand-written Anthropic model table.
- The faux provider (offline end-to-end).
- Offline tests + fixtures; optional offline example.

### Out of scope (explicitly)
- OAuth, Claude-Code identity headers, GitHub Copilot, Cloudflare AI Gateway, AWS Bedrock.
- The provider compat-flag matrix (Fireworks, z.ai, OpenRouter, Vercel AI Gateway, Qwen, etc.).
- Other providers (OpenAI completions/responses, Google, Mistral, …).
- Image *generation* API (`images.ts`).
- The 16K-line generated model registry (`models.generated.ts`).
- Live network tests.

## 5. Architecture

### 5.1 Crate layout (`crates/pi-ai`)
```
crates/pi-ai/
  Cargo.toml
  src/
    lib.rs              # public API: stream(), complete(), re-exports
    types.rs            # wire-compatible types
    stream.rs           # EventStream type alias; complete()
    registry.rs         # trait ApiProvider + registry (by `api`)
    models.rs           # static Anthropic model table + calculate_cost()
    util/
      mod.rs
      json_repair.rs    # repair_json + parse_streaming_json
      env_keys.rs       # env_api_key(provider)
    providers/
      mod.rs            # register_builtins()
      faux.rs           # faux provider
      anthropic/
        mod.rs          # ApiProvider impl; reqwest POST; hands body to process.rs
        sse.rs          # SSE line decoder
        wire.rs         # serde structs for Anthropic request + stream events
        convert.rs      # Context -> request JSON; stop-reason map; id normalization
        process.rs      # core: Stream<Bytes> -> AssistantMessageEvent (no reqwest)
  tests/
    sse.rs
    json_repair.rs
    anthropic_mapping.rs
    request_building.rs
    cost.rs
    faux.rs
    serde_roundtrip.rs
    fixtures/
      anthropic-text.sse
      anthropic-thinking-tooluse.sse
  examples/
    faux_stream.rs      # optional, offline
```

### 5.2 Core types (`types.rs`)
Idiomatic Rust with `serde` bridging to pi's JSON. Representative shapes:

- `enum ContentBlock` (tag = `type`):
  - `Text { text: String, text_signature: Option<String> }`
  - `Thinking { thinking: String, thinking_signature: Option<String>, redacted: Option<bool> }`
  - `Image { data: String, mime_type: String }`
  - `ToolCall { id: String, name: String, arguments: serde_json::Value, thought_signature: Option<String> }`
    (serialized tag value `toolCall`)
- `enum Message` (tag = `role`): `User`, `Assistant`, `ToolResult`.
- `struct AssistantMessage { content, api, provider, model, response_model?, response_id?,
  usage, stop_reason, error_message?, timestamp, … }`.
- `struct Usage { input, output, cache_read, cache_write, total_tokens, cost: Cost }`.
- `enum StopReason`: `Stop`, `Length`, `ToolUse`, `Error`, `Aborted`.
- `enum AssistantMessageEvent` (tag = `type`): `Start`, `TextStart/Delta/End`,
  `ThinkingStart/Delta/End`, `ToolcallStart/Delta/End`, `Done { reason, message }`,
  `Error { reason, error }`. Partial/streaming variants also carry
  `partial: AssistantMessage`, matching pi.
- `struct Context { system_prompt: Option<String>, messages: Vec<Message>, tools: Option<Vec<Tool>> }`.
- `struct Tool { name, description, parameters: serde_json::Value }` (JSON Schema as `Value`).
- `struct Model { id, name, api, provider, base_url, reasoning, input, cost, context_window,
  max_tokens, headers?, thinking_level_map? }`.
- `struct StreamOptions { temperature?, max_tokens?, api_key?, cache_retention?, thinking…,
  tool_choice?, headers?, cancel: Option<CancellationToken>, … }` (subset of pi's options).

### 5.3 Streaming model (`stream.rs`)
- `pub type EventStream = Pin<Box<dyn Stream<Item = AssistantMessageEvent> + Send>>`.
- Providers construct it with `async_stream::stream! { … yield … }.boxed()`.
- Contract: once invoked, request/model/runtime failures are **encoded as the terminal
  `Error` event**, never returned as `Err` or panicked across the stream boundary.
  Terminal events are exactly one of `Done` (reason `stop`/`length`/`toolUse`) or
  `Error` (reason `aborted`/`error`).
- `pub async fn complete(model, ctx, opts) -> AssistantMessage` consumes the stream and
  returns the message from the terminal event.
- Cancellation: `StreamOptions.cancel: Option<CancellationToken>`. The provider loop
  checks it and, when triggered, yields `Error { reason: aborted, … }` then ends.
  Dropping the stream also cancels the in-flight request (the `async-stream` future and
  the reqwest response are dropped).

### 5.4 Provider abstraction (`registry.rs`)
- `trait ApiProvider: Send + Sync { fn stream(&self, model: &Model, ctx: Context,
  opts: Option<StreamOptions>) -> EventStream; }`
- Registry: a process-global `HashMap<String /* api */, Arc<dyn ApiProvider>>` behind a
  `OnceLock<RwLock<…>>`. `register_builtins()` registers Anthropic under
  `"anthropic-messages"`. The faux provider registers under a caller-chosen api id.
- Top-level `stream(model, ctx, opts)` resolves the provider by `model.api`, injects the
  env API key when `opts.api_key` is absent, and delegates. Unknown api -> a stream that
  immediately yields an `Error` event (consistent with the no-throw contract).

### 5.5 Anthropic provider (`providers/anthropic/`)
- `convert.rs`
  - `build_request(model, ctx, opts) -> serde_json::Value` (or a typed `wire::Request`):
    system array with optional `cache_control`; message conversion with consecutive
    tool-result coalescing into a single `user` turn; tools mapped to `input_schema`;
    `max_tokens` (opts or model cap); `temperature` only when set, thinking disabled, and
    supported; thinking config (adaptive `effort` vs budget `budget_tokens`);
    `tool_choice`. Tool-call ids normalized to `^[a-zA-Z0-9_-]{1,64}$`.
  - `map_stop_reason(&str) -> StopReason` (ported from pi; unknown -> `Error`).
- `sse.rs`: a line decoder ported from pi (`decode_sse_line` + an async `iterate_sse`
  over `Stream<Bytes>`): handles `\n` / `\r\n`, `:`-comment lines, multi-line `data`,
  and events split across read boundaries; yields `ServerSentEvent { event, data }`.
- `wire.rs`: `serde` structs for the request and for stream events: `message_start`,
  `content_block_start` (text / thinking / redacted_thinking / tool_use),
  `content_block_delta` (text_delta / thinking_delta / input_json_delta / signature_delta),
  `content_block_stop`, `message_delta` (stop_reason + usage), `message_stop`.
- `process.rs`: the **testable core**. Signature roughly:
  `fn process(body: impl Stream<Item = Bytes>, model: Model, cancel: Option<CancellationToken>) -> EventStream`.
  It runs SSE decode -> wire parse -> maps to `AssistantMessageEvent`s, accumulating
  partial content, usage, cost, and stop reason. All provider logic lives here so it is
  covered by offline fixture tests without reqwest.
- `mod.rs`: the `ApiProvider` impl. Builds the reqwest `POST {base_url}/v1/messages`
  with headers (`x-api-key`, `anthropic-version: 2023-06-01`, `content-type`,
  `accept`, optional beta headers), `stream: true`, then passes
  `response.bytes_stream()` to `process()`. This network shim is intentionally thin.

### 5.6 Supporting utilities
- `util/json_repair.rs`: `repair_json` (escape raw control chars, fix invalid escapes)
  and `parse_streaming_json` (try strict parse -> repair -> in-house partial completion
  that closes open strings/arrays/objects -> `{}` fallback). No external partial-JSON dep.
- `util/env_keys.rs`: `env_api_key(provider) -> Option<String>` mapping `anthropic` ->
  `ANTHROPIC_API_KEY` (+ common aliases).
- `models.rs`: a small static table of current Anthropic models (id, name, cost,
  context window, max tokens, reasoning flag) and `calculate_cost(&Model, &mut Usage)`
  ported exactly (per-million-token rates).

## 6. Data flow

```
caller
  -> stream(model, ctx, opts)
       -> registry.lookup(model.api) -> Arc<dyn ApiProvider>
       -> provider.stream(...)                      [anthropic]
            -> build reqwest POST /v1/messages (stream)
            -> process(response.bytes_stream(), model, cancel)
                 -> sse decode -> wire parse -> map -> yield AssistantMessageEvent*
            => Start, (Text|Thinking|Toolcall)(Start|Delta|End)*, (Done|Error)
  -> consumer iterates events  (or)  complete() drains to the terminal message
```

The faux provider replaces the reqwest+process portion with a scripted response that is
re-emitted as deltas, exercising the same event protocol and consumer path offline.

## 7. Error handling

- All failures after `stream()` is invoked are surfaced as a terminal `Error` event whose
  `AssistantMessage` has `stop_reason` `error` or `aborted` and an `error_message`.
- Cancellation produces `Error { reason: aborted }`.
- SSE/JSON parse failures inside `process()` terminate the stream with an `Error` event
  (matching pi, which throws internally and converts to the error event).
- Unknown Anthropic stop reasons or block types are handled defensively (mapped to
  `Error` or skipped) rather than panicking.

## 8. Testing strategy (all offline)

1. **SSE decoder** — synthetic byte chunks: LF vs CRLF, `:`-comments, multi-line `data`,
   events split across read boundaries, trailing data without final blank line.
2. **Streaming JSON** — repair cases (raw control chars, bad escapes) and partial-parse
   cases (truncated objects/strings) produce the expected values.
3. **Anthropic mapping** — feed `tests/fixtures/*.sse` through `process()`; assert the
   exact `AssistantMessageEvent` sequence and the final `AssistantMessage` (covering
   text, thinking, tool_use, usage, and stop reason).
4. **Request building** — `Context` -> expected request JSON, including tool-result
   coalescing and `cache_control` placement.
5. **Cost calc** — known usage + model rates -> expected costs.
6. **Faux provider** — queue responses, consume `stream()`, assert delta events, the
   final message, and the usage estimate.
7. **Serde round-trip** — types serialize to pi-matching JSON and deserialize back.

Fixtures are authored by hand from the documented Anthropic SSE format (and may be
seeded from real captures if available); no fixture requires a credential to produce.

## 9. Dependencies & toolchain

- Toolchain: Rust 1.96.0, edition 2024 (confirmed available).
- Crates: `tokio` (rt-multi-thread, macros, sync, time), `futures` / `futures-util`,
  `async-stream`, `reqwest` (json + stream, rustls-tls), `serde` (derive), `serde_json`,
  `bytes`, `tokio-util` (CancellationToken), `thiserror`.
- Dev: `tokio` test macros; fixtures under `tests/fixtures/`.

## 10. Risks

- **In-house partial-JSON parser** may diverge from npm `partial-json` on exotic input.
  Mitigation: tests pin the behavior the provider actually needs (incremental tool args).
  Low risk.
- **Anthropic wire types lag the API** (new effort levels, block types). Mitigation:
  defensive mapping (skip/route-to-error) as pi does.
- **Global mutable registry** needs care for test isolation. Mitigation: faux provider
  registers under unique api ids and unregisters; tests avoid relying on global order.

## 11. Future phases (not part of this PoC)

In dependency order, once the approach is validated: remaining `pi-ai` providers and the
generated model registry; `pi-agent-core` (the tool-calling loop, building on this crate);
`pi-tui`; then `pi-coding-agent`. Each gets its own spec -> plan -> implementation cycle.
```
