# Testing Patterns

**Analysis Date:** 2026-07-10

## Test Framework

**Runner:**
- Rust built-in test harness through Cargo (`cargo test`), using standard `#[test]` and Tokio's `#[tokio::test]` for async behavior.
- Tokio 1 supplies async test runtime features in `crates/pi-ai/Cargo.toml`, `crates/pi-agent-core/Cargo.toml`, and `crates/pi-coding-agent/Cargo.toml`.
- Config: workspace/package manifests in `Cargo.toml` and `crates/*/Cargo.toml`; no separate test-runner configuration file is present.

**Assertion Library:**
- Rust standard `assert!`, `assert_eq!`, `assert_ne!`, `matches!`, `panic!`, `expect`, and `unwrap`; no external assertion framework is detected.
- Prefer assertion messages for behavioral or architectural expectations, as demonstrated in `crates/pi-agent-core/tests/agent_loop.rs` and `crates/pi-agent-core/tests/api_boundary_guards.rs`.

**Run Commands:**
```bash
cargo test --workspace                 # Run all unit, integration, and documentation tests
cargo test -p pi-coding-agent          # Run one crate's complete suite
cargo test -p pi-agent-core --test agent_loop  # Run one integration-test target
cargo test -p pi-ai completions_fixture_maps_text_tool_usage_and_done  # Filter by test name
cargo test --workspace --quiet         # Full suite with compact output
cargo fmt --check                      # Required formatting gate used with tests
cargo check --workspace                # Workspace compilation gate used with tests
scripts/tui-smoke.sh                   # Manual tmux-based interactive TUI smoke suite
```

## Test File Organization

**Location:**
- Put public-behavior and cross-module integration tests in each crate's `tests/` directory, for example `crates/pi-ai/tests/`, `crates/pi-agent-core/tests/`, `crates/pi-coding-agent/tests/`, and `crates/pi-tui/tests/`.
- Put small white-box unit tests in a co-located `#[cfg(test)] mod tests` when private implementation access is necessary, as in `crates/pi-coding-agent/src/config/paths.rs` and `crates/pi-tui/src/undo_stack.rs`.
- Put reusable integration-test helpers in `crates/pi-ai/tests/support/mod.rs`, `crates/pi-agent-core/tests/common/mod.rs`, and `crates/pi-coding-agent/tests/support/mod.rs`.
- Store deterministic protocol fixtures under the owning test tree, such as SSE inputs in `crates/pi-ai/tests/fixtures/`.

**Naming:**
- Name files after the tested subsystem or contract: `http_retry.rs`, `agent_loop.rs`, `editor_component.rs`, `public_api.rs`, and `api_boundary_guards.rs`.
- Name tests as behavioral statements in `snake_case`, including the condition and expected outcome: `parse_retry_after_exceeds_max_delay` in `crates/pi-ai/tests/http_retry.rs`.
- Use milestone names only for broad compatibility suites that intentionally preserve a historical contract, such as `crates/pi-agent-core/tests/m9_harness.rs`.

**Structure:**
```text
crates/<crate>/
├── src/
│   └── <module>.rs              # Optional #[cfg(test)] mod tests for private units
└── tests/
    ├── support/mod.rs           # Shared guards/fakes, where needed
    ├── common/mod.rs            # Shared scripted runtime helpers, where needed
    ├── fixtures/                # Checked-in deterministic input data
    ├── <feature>.rs             # Behavior/integration tests
    ├── public_api.rs            # Stable facade compile/behavior checks
    └── *_boundary_guards.rs     # Source-level architecture constraints
```

## Test Structure

**Suite Organization:**
```rust
fn default_cfg() -> RetryConfig {
    RetryConfig {
        max_retries: 2,
        timeout_ms: None,
        max_retry_delay_ms: 10_000,
    }
}

#[test]
fn parse_retry_after_exceeds_max_delay() {
    let cfg = RetryConfig {
        max_retry_delay_ms: 1000,
        ..default_cfg()
    };

    let result = parse_retry_after_ms(Some("5"), &cfg);

    assert!(result.is_err());
}
```
Pattern from `crates/pi-ai/tests/http_retry.rs`: define a small local fixture helper, arrange inputs, invoke one behavior, and assert the public result.

**Patterns:**
- Keep setup local to the test unless it is shared across several files; shared stateful helpers belong in `tests/support/mod.rs` or `tests/common/mod.rs`.
- Use RAII guards for environment variables and global provider registries. `EnvGuard` and `ProviderGuard` restore state in `Drop` in `crates/pi-ai/tests/support/mod.rs` and `crates/pi-coding-agent/tests/support/mod.rs`.
- Use `tempfile` for isolated filesystem tests in crates that declare it in `crates/pi-agent-core/Cargo.toml` and `crates/pi-coding-agent/Cargo.toml`.
- Test streamed protocols by collecting or incrementally consuming `EventStream` values and matching exact event variants, as in `crates/pi-ai/tests/openai_completions.rs` and `crates/pi-agent-core/tests/agent_loop.rs`.
- Use source-scanning guard tests to make architectural rules executable. `crates/pi-agent-core/tests/api_boundary_guards.rs` and `crates/pi-tui/tests/deterministic_boundary.rs` read checked-in source and report line-specific violations.
- Use named timing constants and paused Tokio time for deterministic scheduling tests; `#[tokio::test(start_paused = true)]` appears in `crates/pi-agent-core/tests/agent_loop.rs`, and `crates/pi-tui/tests/deterministic_boundary.rs` enforces named time values.

## Mocking

**Framework:** Hand-written fakes, scripted providers, closures, channels, and in-memory implementations; no general mocking crate is detected.

**Patterns:**
```rust
let calls = Arc::new(AtomicUsize::new(0));
let calls_for_streamer = calls.clone();
let mut config = test_config("live-follow-up-provider");
config.provider_streamer = Some(Arc::new(move |_model, context, _opts| {
    let call = calls_for_streamer.fetch_add(1, Ordering::SeqCst) + 1;
    Box::pin(async_stream::stream! {
        // Yield deterministic AssistantMessageEvent values for this turn.
    })
}));
```
Pattern from `crates/pi-agent-core/tests/agent_loop.rs`: inject a closure-backed provider, coordinate concurrency with Tokio channels, and inspect captured calls/state.

**What to Mock:**
- Mock provider/network boundaries with `ApiProvider`, `ProviderStreamer`, `TestProvider`, and scripted `AssistantMessageEvent` sequences from `crates/pi-agent-core/tests/common/mod.rs`.
- Replace external API responses with checked-in SSE fixtures under `crates/pi-ai/tests/fixtures/` and feed them into provider processors as byte streams.
- Use `InMemoryExecutionEnv` and test-specific implementations for filesystem/shell boundaries exported by `crates/pi-agent-core/src/lib.rs` when testing agent behavior.
- Mock process coordination with Tokio `oneshot`, `mpsc`, `Mutex`, and paused time rather than sleeping against wall-clock time.

**What NOT to Mock:**
- Do not mock pure conversion, parsing, layout, or formatting logic; call it directly with concrete values, following `crates/pi-ai/tests/http_retry.rs` and `crates/pi-tui/tests/style.rs`.
- Do not call live model-provider endpoints in the normal suite. Provider tests use missing-key assertions, deterministic fixtures, or faux providers under `crates/pi-ai/tests/`.
- Do not bypass the stable public facade when the purpose is API compatibility; `public_api.rs` and boundary guard suites must import or scan the actual public surface.

## Fixtures and Factories

**Test Data:**
```rust
pub fn text_turn(text: &str) -> ScriptedTurn {
    let text_block = ContentBlock::Text {
        text: text.into(),
        text_signature: None,
    };

    ScriptedTurn {
        events: vec![
            AssistantMessageEvent::Start { /* deterministic partial */ },
            AssistantMessageEvent::TextDelta { /* deterministic delta */ },
        ],
        stop_reason: StopReason::Stop,
        response_id: "resp_1".into(),
        model_name: "test-model".into(),
    }
}
```
Condensed pattern from `crates/pi-agent-core/tests/common/mod.rs`: factories return complete domain objects and deterministic event sequences rather than loosely typed maps.

**Location:**
- Cross-test factories and fake providers: `crates/pi-agent-core/tests/common/mod.rs`.
- Environment/provider-state guards: `crates/pi-ai/tests/support/mod.rs` and `crates/pi-coding-agent/tests/support/mod.rs`.
- Provider wire fixtures: `crates/pi-ai/tests/fixtures/*.sse`.
- Test-local builders such as `default_cfg` and `test_model`: keep beside the tests in `crates/pi-ai/tests/http_retry.rs` and `crates/pi-ai/tests/openai_completions.rs`.

## Coverage

**Requirements:** No numeric line/branch coverage target or coverage configuration is enforced in the repository. Coverage quality is enforced behaviorally through broad workspace tests, stable-API tests, deterministic boundary guards, and focused verification commands documented in `docs/TODO.md`.

**View Coverage:**
```bash
cargo test --workspace
```
No repository-defined `cargo llvm-cov`, Tarpaulin, or equivalent coverage command is detected. Introduce a coverage tool only as an explicit project decision rather than assuming it is part of the current gate.

## Test Types

**Unit Tests:**
- Exercise pure functions, value types, formatting, parsing, and private helpers with direct assertions. Examples include `crates/pi-ai/tests/http_retry.rs`, `crates/pi-tui/tests/style.rs`, and co-located tests in `crates/pi-coding-agent/src/config/paths.rs`.
- Prefer table-like sequences of small named tests where each edge case deserves an independently searchable failure, as in `crates/pi-ai/tests/http_retry.rs`.

**Integration Tests:**
- Exercise public crate behavior, async agent/provider flows, session/runtime boundaries, resource loading, protocol conversion, and stable API imports under each crate's `tests/` directory.
- Treat architectural source guards as integration tests. They enforce allowed imports, public facade shape, deterministic-test rules, and migration boundaries in files such as `crates/pi-agent-core/tests/api_boundary_guards.rs` and `crates/pi-tui/tests/api_boundary_guards.rs`.
- Use crate-specific runs during development and `cargo test --workspace --quiet` as the broad regression gate, matching `docs/TODO.md`.

**E2E Tests:**
- No browser or service-level E2E framework is used.
- Interactive terminal behavior has a tmux-based smoke workflow in `scripts/tui-smoke.sh`, documented by `docs/tui-smoke.md`; it builds `pi-coding-agent`, launches interactive mode, drives input, and stores captures under `target/tui-smoke/`.
- Keep the default smoke path offline/deterministic; the optional real-provider prompt is explicitly opt-in through the environment variable named in `docs/tui-smoke.md`.

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn llm_events_stream_before_provider_done() {
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let agent = Agent::new(test_config("live-stream-provider"));
    let mut stream = agent.prompt("hi");

    tokio::time::timeout(Duration::from_millis(200), async {
        while let Some(event) = stream.next().await {
            if matches!(event, AgentEvent::LlmEvent(_)) {
                return;
            }
        }
        panic!("stream ended before expected event");
    })
    .await
    .expect("event should arrive before provider completes");

    release_tx.send(()).unwrap();
}
```
Adapted from `crates/pi-agent-core/tests/agent_loop.rs`: coordinate explicitly, assert intermediate streaming behavior with a timeout, then release the fake provider. Use named timing constants when extending timing-sensitive TUI suites, per `crates/pi-tui/tests/deterministic_boundary.rs`.

**Error Testing:**
```rust
#[test]
fn parse_retry_after_invalid_header() {
    let result = parse_retry_after_ms(Some("not-a-number"), &default_cfg());
    assert!(result.is_err());
}

#[test]
fn provider_guard_unregisters_new_provider_on_drop() {
    let api = "pi-ai-provider-guard-drop-api";
    registry::unregister(api);
    {
        let _guard = support::ProviderGuard::register(api, Arc::new(GuardTestProvider("temp")));
        assert!(registry::lookup(api).is_some());
    }
    assert!(registry::lookup(api).is_none());
}
```
Patterns from `crates/pi-ai/tests/http_retry.rs` and `crates/pi-ai/tests/support_guards.rs`: assert returned failure for invalid input and assert cleanup/restoration for stateful failure boundaries.

---

*Testing analysis: 2026-07-10*
