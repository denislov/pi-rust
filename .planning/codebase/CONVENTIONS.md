# Coding Conventions

**Analysis Date:** 2026-07-10

## Naming Patterns

**Files:**
- Use lowercase `snake_case.rs` for Rust modules and focused implementation units, such as `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs` and `crates/pi-agent-core/src/agent_turn_flow/runtime.rs`.
- Use `mod.rs` only for directory module roots and re-export surfaces, as in `crates/pi-agent-core/src/resources/mod.rs` and `crates/pi-tui/src/components/mod.rs`.
- Name integration-test files after the behavior or boundary they cover, such as `crates/pi-agent-core/tests/agent_loop.rs`, `crates/pi-ai/tests/http_retry.rs`, and `crates/pi-coding-agent/tests/api_boundary_guards.rs`.
- Use explicit suffixes for architectural enforcement tests: `*_boundary_guards.rs`, `public_api.rs`, and `deterministic_boundary.rs` identify contract tests rather than feature examples.

**Functions:**
- Use `snake_case` for functions and methods, with behavior-oriented names such as `parse_retry_after_ms` in `crates/pi-ai/src/util/http.rs` and `resolve_prompt_request` exported by `crates/pi-coding-agent/src/lib.rs`.
- Test functions read as complete behavioral statements, for example `provider_guard_restores_existing_provider_on_drop` in `crates/pi-ai/tests/support_guards.rs` and `root_public_modules_are_marked_migration_private` in `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use `new` for primary constructors and `with_*` consuming builders for optional configuration, as shown by `SelfHealingEditRequest::new`, `with_check_command`, and `with_repair_attempts` in `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`.
- Use verb prefixes consistently: `load_*`, `resolve_*`, `parse_*`, `build_*`, `run_*`, `format_*`, and `register_*` communicate side effects and ownership across `crates/pi-agent-core/src/lib.rs` and `crates/pi-coding-agent/src/lib.rs`.

**Variables:**
- Use short conventional names only when scope is small (`ctx`, `opts`, `cfg`, `req`); otherwise prefer semantic names such as `release_rx`, `provider_streamer`, and `previous_non_empty` in `crates/pi-agent-core/tests/agent_loop.rs` and `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use suffixes to show cloned or adapted ownership in async closures, such as `calls_for_streamer` in `crates/pi-agent-core/tests/agent_loop.rs`.
- Prefix intentionally retained ownership guards with `_`, such as `_lock` and `_guard` in `crates/pi-coding-agent/tests/support/mod.rs` and `crates/pi-ai/tests/support_guards.rs`.
- Name constants in `SCREAMING_SNAKE_CASE`; test-only source snapshots and timing anchors follow this convention in `crates/pi-tui/tests/deterministic_boundary.rs`.

**Types:**
- Use `UpperCamelCase` for structs, enums, traits, aliases, and enum variants: `RetryConfig`, `ApiProvider`, `AssistantMessageEvent::Done`, and `CodingAgentSession` in `crates/pi-ai/src/util/http.rs`, `crates/pi-ai/tests/support_guards.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Use domain suffixes to make roles explicit: `*Config`, `*Options`, `*Request`, `*Outcome`, `*Error`, `*Provider`, `*Guard`, `*Flow`, and `*Service` recur throughout `crates/pi-agent-core/src/` and `crates/pi-coding-agent/src/coding_session/`.
- Derive standard traits explicitly according to value semantics. Configuration/data types commonly derive `Debug`, `Clone`, `PartialEq`, serialization traits, and sometimes `Default`, as in `PartialCompaction` in `crates/pi-coding-agent/src/config/settings.rs`.

## Code Style

**Formatting:**
- Use standard `rustfmt` through `cargo fmt`; no `rustfmt.toml` or `.rustfmt.toml` is present, so default toolchain formatting is authoritative.
- Keep formatter-produced multiline imports, derives, match arms, and builder expressions. Representative formatting appears in `crates/pi-agent-core/src/lib.rs` and `crates/pi-ai/tests/openai_completions.rs`.
- Verify formatting with `cargo fmt --check`; this command is part of the recurring project verification recorded in `docs/TODO.md` and phase plans under `docs/superpowers/plans/`.
- Use trailing commas in multiline structs, calls, enum variants, and collection literals so `rustfmt` produces stable diffs, as in `crates/pi-ai/tests/openai_completions.rs`.

**Linting:**
- Use Clippy for static analysis. The project documents `cargo clippy --fix` across `pi-tui`, `pi-ai`, `pi-agent-core`, and `pi-coding-agent` in `docs/TODO.md`.
- Keep lint suppression narrow and documented by code location. `crates/pi-coding-agent/src/lib.rs` has crate-level allowances for intentionally large error/enum types, argument-heavy APIs, and compatibility conditionals; most other allowances are item- or module-scoped.
- Preserve explicit compatibility allowances such as `#[allow(deprecated)]` only around migration shims and compatibility tests in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/tests/support/mod.rs`.
- Do not add broad warning suppression to new crates. Match the smallest existing scope in the owning module, and add a boundary test when the allowance protects a deliberate migration surface.

## Import Organization

**Order:**
1. Declare crate/module attributes and local module declarations first, as in `crates/pi-coding-agent/src/lib.rs`.
2. Import the Rust standard library (`std::*`) before external crates, separated by a blank line, as in `crates/pi-ai/tests/support_guards.rs`.
3. Import third-party and workspace crates (`async_stream`, `futures`, `pi_ai`, `pi_agent_core`) next, grouping related symbols with brace imports.
4. Import local crate modules with `crate::...` and test support with `mod support;` or `use support::...`; keep each group formatter-compatible.

**Path Aliases:**
- Rust module paths are used directly; no custom path-alias configuration is present.
- Inside a crate, prefer `crate::...` for cross-module references, as in `crates/pi-coding-agent/src/config/settings.rs` and `crates/pi-ai/src/util/http.rs`.
- Downstream and integration-test code should prefer stable `pi_ai::api`, `pi_agent_core::api`, and `pi_coding_agent::api` facades where available. Root-level compatibility exports remain deliberately constrained by tests such as `crates/pi-agent-core/tests/api_boundary_guards.rs`.
- Use `super::*` primarily in small co-located `#[cfg(test)]` modules such as `crates/pi-coding-agent/src/config/paths.rs`; integration tests use explicit public imports.

## Error Handling

**Patterns:**
- Return `Result<T, E>` for recoverable failures and use typed domain errors where the API crosses a component boundary. `thiserror` is used by `pi-agent-core`, `pi-coding-agent`, and `pi-tui` via their `Cargo.toml` manifests.
- Convert lower-level failures at the ownership boundary with `map_err`, attaching domain context. `PluginError::Execution` conversion in `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs` is representative.
- Use early returns for invalid conditions and keep error messages actionable, as in `parse_retry_after_ms` in `crates/pi-ai/src/util/http.rs`.
- In tests, use `expect` with a state-specific message when failure indicates a broken fixture or invariant, and use `unwrap` only for setup that cannot meaningfully recover. Examples appear in `crates/pi-ai/tests/support_guards.rs` and `crates/pi-agent-core/tests/agent_loop.rs`.
- Model provider-stream failures as events where streaming APIs require continued protocol consistency; scripted error events in `crates/pi-agent-core/tests/common/mod.rs` exercise this behavior.

## Logging

**Framework:** Direct CLI output (`println!`/`eprintln!`) and structured product/protocol events; no general-purpose logging facade is detected.

**Patterns:**
- Send user-facing errors and diagnostics to stderr at executable/interactive boundaries, as in `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, and `crates/pi-coding-agent/src/resources.rs`.
- Keep reusable library logic free of incidental console output. Return typed errors, diagnostics, outcomes, or events from modules under `crates/pi-ai/src/`, `crates/pi-agent-core/src/`, and `crates/pi-coding-agent/src/coding_session/`.
- Emit progress through established event types such as `AgentEvent`, `AssistantMessageEvent`, and `CodingAgentProductEvent`, exported from `crates/pi-agent-core/src/lib.rs`, `crates/pi-ai/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Reserve `println!` in library-adjacent code for examples, benchmarks, and intentional command output; examples include `crates/pi-coding-agent/src/interactive/app.rs` and `crates/pi-agent-core/examples/loop_example.rs`.

## Comments

**When to Comment:**
- Explain architectural intent, compatibility constraints, unsafe/global-state handling, and non-obvious protocol behavior. Stable-facade rationale is documented in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Use short inline comments to explain fixture ordering or protocol structure, such as the message-order comment in `crates/pi-ai/tests/openai_completions.rs`.
- Avoid narrating straightforward assignments or control flow. Prefer expressive type and test names throughout `crates/pi-tui/tests/` and `crates/pi-agent-core/tests/`.

**JSDoc/TSDoc:**
- Not applicable; this is a Rust workspace.
- Use Rust doc comments (`///` and `//!`) for public contracts, module intent, migration guidance, and helper caveats, as shown in `crates/pi-agent-core/src/agent_loop.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-agent-core/tests/common/mod.rs`.
- Use `#[deprecated(note = "...")]` for machine-visible migration guidance rather than relying only on prose comments, as in the crate root facades.

## Function Design

**Size:** Keep leaf helpers narrow and behavior-specific; split orchestration into named flows, services, adapters, and helpers under `crates/pi-coding-agent/src/coding_session/`. Large public operations should expose a small facade even when internal workflows are substantial.

**Parameters:**
- Accept borrowed inputs (`&str`, `&Path`, slices, `Option<&T>`) when ownership is unnecessary, as in `crates/pi-ai/src/util/http.rs` and `crates/pi-coding-agent/src/config/paths.rs`.
- Accept `impl Into<String>` or `impl AsRef<OsStr>` at ergonomic construction/configuration boundaries, as in `SelfHealingEditRequest` and `EnvGuard` helpers.
- Group evolving optional behavior into `*Config` and `*Options` structs rather than extending long positional argument lists. Existing exceptions are explicitly covered by scoped Clippy allowances in `crates/pi-coding-agent/src/lib.rs`.

**Return Values:**
- Return owned domain values from constructors and transforms, `Option<T>` for absence, and `Result<T, E>` for recoverable failure.
- Return borrowed views (`&str`, slices, `Option<&T>`) from accessors, following `SelfHealingEditRequest` in `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`.
- Use iterators and streams for incremental data. `EventStream` and async stream implementations in `crates/pi-ai/src/lib.rs` and `crates/pi-agent-core/tests/common/mod.rs` establish the streaming pattern.

## Module Design

**Exports:**
- Keep implementation modules private by default (`mod`) and expose intentional contracts through crate-root `api` modules and curated `pub use` lists in `crates/pi-ai/src/lib.rs`, `crates/pi-agent-core/src/lib.rs`, and `crates/pi-coding-agent/src/lib.rs`.
- Mark migration-only public modules `#[doc(hidden)]`; boundary tests such as `crates/pi-agent-core/tests/api_boundary_guards.rs` enforce this convention.
- Add compatibility exports only with `#[deprecated(note = "...")]` and a named replacement path. Do not expand root-level facades casually.

**Barrel Files:**
- Rust crate roots and selected `mod.rs` files serve as controlled barrels. Use them to declare modules and re-export a deliberate stable surface, not every internal symbol.
- Add new stable public APIs to the owning crate's `api` module and update public-API/boundary tests under that crate's `tests/` directory.
- Keep test helpers in crate-local support modules such as `crates/pi-ai/tests/support/mod.rs`, `crates/pi-agent-core/tests/common/mod.rs`, and `crates/pi-coding-agent/tests/support/mod.rs`; do not promote them into production facades.

---

*Convention analysis: 2026-07-10*
