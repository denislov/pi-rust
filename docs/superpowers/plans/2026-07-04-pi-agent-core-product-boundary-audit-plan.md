# pi-agent-core Product Boundary Audit Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Confirm and document that `pi-agent-core` remains a low-level agent/runtime crate and does not depend on `pi-coding-agent` product ownership types such as `CodingAgentSession`, `CodingAgentEvent`, protocol adapters, interactive adapters, or session-owner services.

**Architecture:** Keep product/session/workflow ownership in `pi-coding-agent`. `pi-agent-core` may expose low-level agent, tool, hook, runtime, execution environment, and Flow primitives, but must not import or depend on coding-agent product APIs.

**Tech Stack:** Rust 2024 workspace, Cargo metadata dependency graph, ripgrep source audit, focused crate check.

---

### Task 1: Audit Source References

**Files:**
- Inspect: `crates/pi-agent-core/`
- Inspect: `crates/pi-agent-core/Cargo.toml`

- [x] **Step 1: Search for product ownership references**

Search `pi-agent-core` for `pi_coding_agent`, `pi-coding-agent`, `CodingAgentSession`, `CodingAgentEvent`, `coding_session`, `protocol`, and `interactive`.

- [x] **Step 2: Classify findings**

Confirm any hits are documentation comments or generic non-product terms, not imports, dependencies, or runtime ownership code.

### Task 2: Audit Cargo Dependency Boundary

**Files:**
- Inspect: `Cargo.toml`
- Inspect: `crates/pi-agent-core/Cargo.toml`

- [x] **Step 1: Query Cargo metadata**

Run:

```bash
/home/whai/.cargo/bin/cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="pi-agent-core") | .dependencies[].name'
```

Confirm `pi-coding-agent` is absent.

- [x] **Step 2: Run focused crate check**

Run:

```bash
/home/whai/.cargo/bin/cargo check -p pi-agent-core --quiet
```

### Task 3: Update Docs And Verify

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-04-pi-agent-core-product-boundary-audit-plan.md`

- [x] **Step 1: Update TODO**

Mark the cross-cutting `pi-agent-core` product-ownership boundary item complete with the source/dependency audit result.

- [x] **Step 2: Mark plan steps complete**

Update this plan's checkboxes as implementation proceeds.

- [x] **Step 3: Run final checks**

Run:

```bash
/home/whai/.cargo/bin/cargo check -p pi-agent-core --quiet
/home/whai/.cargo/bin/cargo check --workspace --quiet
git diff --check
```
