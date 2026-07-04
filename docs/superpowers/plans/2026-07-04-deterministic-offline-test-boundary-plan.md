# Deterministic Offline Test Boundary Hardening Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tighten the workspace invariant that tests are deterministic and offline unless explicitly marked smoke/opt-in.

**Architecture:** Keep ordinary unit/integration tests on faux providers, fixtures, injected environment maps, temp directories, and local in-memory execution. Avoid real provider keys, live network calls, wall-clock uniqueness, and unguarded process-wide environment mutation in tests.

**Tech Stack:** Rust 2024, existing test suites, standard-library synchronization, no new test runtime dependency.

---

### Task 1: Audit Test Boundary Risks

**Files:**
- Inspect: `crates/*/tests/**/*.rs`
- Inspect: crate-local `#[cfg(test)]` modules

- [x] **Step 1: Search high-risk test signals**

Search for `#[ignore]`, `smoke`, provider key env vars, `reqwest`, socket/network APIs, literal HTTP URLs, `SystemTime::now`, `Instant::now`, and random APIs.

- [x] **Step 2: Classify findings**

Separate real risks from deterministic fixture values, injected env maps, faux providers, local temp dirs, and implementation-only provider code.

### Task 2: Harden Concrete Determinism Gaps

**Files:**
- Modify: `crates/pi-ai/src/util/env_keys.rs`
- Add: `crates/pi-ai/tests/support/mod.rs`
- Modify: `crates/pi-ai/tests/{azure_openai_responses,bedrock,deepseek,env_keys,mistral,openai_codex_responses,openai_completions}.rs`
- Modify: `crates/pi-tui/tests/autocomplete.rs`

- [x] **Step 1: Guard pi-ai env-key/provider tests**

Add test-local/shared environment guards that serialize env mutation and restore the previous values for env vars touched by each test.

- [x] **Step 2: Remove wall-clock temp suffix from autocomplete tests**

Replace `SystemTime::now()`-based temp directory suffixes with a process-local atomic counter plus process id.

- [x] **Step 3: Run focused tests**

Run:

```bash
/home/whai/.cargo/bin/cargo test -p pi-ai util::env_keys::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test env_keys --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test deepseek --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test openai_completions completions_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test mistral mistral_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test bedrock bedrock_provider_missing_credentials_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test openai_codex_responses codex_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test azure_openai_responses azure_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-tui --test autocomplete --quiet
```

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-deterministic-offline-test-boundary-plan.md`

- [x] **Step 1: Update TODO**

Record the offline/deterministic audit, marking the cross-cutting test-boundary item in progress if remaining cross-crate env/timing risks are found.

- [x] **Step 2: Mark plan steps complete**

Update this plan's checkboxes as implementation proceeds.

- [x] **Step 3: Run final checks**

Run:

```bash
/home/whai/.cargo/bin/cargo fmt --check
/home/whai/.cargo/bin/cargo test -p pi-ai util::env_keys::tests --lib --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test env_keys --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test deepseek --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --test openai_completions completions_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test mistral mistral_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test bedrock bedrock_provider_missing_credentials_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test openai_codex_responses codex_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-ai --test azure_openai_responses azure_provider_missing_key_returns_error_event -- --nocapture
/home/whai/.cargo/bin/cargo test -p pi-tui --test autocomplete --quiet
/home/whai/.cargo/bin/cargo test -p pi-ai --quiet
/home/whai/.cargo/bin/cargo test -p pi-tui --quiet
/home/whai/.cargo/bin/cargo test --workspace --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```
