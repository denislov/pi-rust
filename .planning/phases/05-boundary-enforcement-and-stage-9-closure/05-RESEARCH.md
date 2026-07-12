# Phase 5 Research: Boundary Enforcement and Stage 9 Closure

## User Constraints

Copied from `05-CONTEXT.md` (locked decisions):

- The phase hardens JSON, print, RPC, and interactive adapter guards, proves the stable facade from an external-consumer perspective, verifies the workspace, and records one authoritative Stage 9 closure report.
- Do not restore, rename, or recreate deleted broad workflow methods. Do not implement Stage 10 typed `ProductEvent` payload convergence or compatibility-subscription deletion.
- Use one centralized adapter inventory; recursively scan registered roots, explicitly register single-file entry points, and fail closed on missing/empty/unreadable/duplicate/unowned paths.
- Scan production code only; test callers remain covered by the Phase 4 receiver-aware deleted-method ledger.
- Detect prohibited calls structurally with receiver awareness. Legitimate same-name calls belong in one scoped exception table with exact/max counts and reasons. Unknown receivers fail closed. Scanner fixtures must cover formatting variants and ignored comments/strings/test-only code. No inline suppression directives are allowed.
- Add external-consumer compile-pass and compile-fail fixtures. Negative fixtures cover operation/dispatch, runtime services, plugin options/registries, and Flow contracts through `api`, crate-root, and `#[doc(hidden)]` paths. Assert failure category without exact rustc text. Keep an independent explicit positive facade inventory.
- Produce one authoritative Stage 9 closure report with structured command evidence, source-audit results, verified worktree/commit state, historical-plan superseded marker, current-document links, and a bounded Stage 10 handoff inventory.

## Project Constraints (from AGENTS.md)

- Use Chinese for user communication; technical documents may be English.
- In an indexed repository, use `codegraph explore` before grep/file reads for code understanding.
- Preserve dependency direction (`pi-coding-agent -> pi-agent-core -> pi-ai`, plus `pi-tui`), stable contracts under `pi_coding_agent::api`, typed durable facts/replay ordering, and existing behavior/event/control semantics.
- Verify with `cargo fmt --check`, focused `pi-coding-agent` tests, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check`.

## Current Baseline

- `product_runtime_boundary_guards.rs` already contains source sanitization, recursive helpers, receiver-aware method ledgers, retained-method assertions, and alternate-facade checks. Its 16-method absence ledger is the Phase 4 deletion baseline. [VERIFIED: current source and `04-VERIFICATION.md`]
- `api_boundary_guards.rs` checks root/module visibility, compatibility exports, dispatcher/event boundaries, and internal-contract source restrictions. `public_api.rs` has an independent 15-variant facade signature/behavior inventory. [VERIFIED: current test targets named in `05-CONTEXT.md`]
- Phase 4 completed migration and deletion; `load_plugins(PluginLoadOptions)` remains private for exactly four co-located owner-test calls because the public operation cannot express custom plugin candidates/registries. This must remain an explicit exception, not a compatibility wrapper. [VERIFIED: `04-RESEARCH.md`, `04-VERIFICATION.md`]
- No external package is expected. The implementation should use Rust integration tests, existing sanitization/parser helpers, and Cargo fixtures; adding a parser crate would expand scope and requires separate legitimacy review. [VERIFIED: project manifests and existing guard design]

## Standard Stack

| Concern | Use | Evidence |
|---|---|---|
| Boundary tests | Rust integration targets under `crates/pi-coding-agent/tests/` | [VERIFIED: existing guard targets] |
| Compile contracts | Deterministic offline Cargo/rustc fixture crates or harness invoked by integration tests | [VERIFIED: D-12 through D-16] |
| Source scanning | Existing sanitizer plus structural receiver/token logic, extended for recursive inventory and cfg(test) exclusion | [VERIFIED: `product_runtime_boundary_guards.rs`] |
| Runtime verification | `cargo test`, `cargo check`, rustfmt, git diff checks | [VERIFIED: project constraints and Phase 4 validation] |

## Architecture Patterns

1. **Central inventory as coverage assertion.** Store JSON/print/RPC/interactive roots and single-file entry points in one test-owned table. Discover all `.rs` files recursively, normalize paths, assert every discovered file has exactly one owner, and emit path-specific diagnostics. [VERIFIED: D-01..D-05]
2. **Structural call guard.** Sanitize comments, strings, chars, raw strings, and test-only regions before token recognition; parse receiver shape across line breaks, chains, parentheses, comments, and rustfmt output. Match method + receiver, then consult only the scoped exception table. [VERIFIED: D-06..D-11]
3. **Compiler-first API proof.** Positive fixture imports every public operation variant/outcome/support type through `pi_coding_agent::api`. Compile-fail fixtures attempt each forbidden category through API, crate-root, and doc-hidden module paths, asserting failure class rather than diagnostic wording. [VERIFIED: D-12..D-16]
4. **Independent closure evidence.** A closure report records exact commands, timestamp, status, counts/conclusions, source-audit scope, and commit/worktree identity without embedding full logs. Current authority docs link to this report; the historical plan is marked superseded. [VERIFIED: D-17..D-21]

## Don't Hand-Roll

- Do not add a general-purpose parser dependency solely for this guard; extend the existing deterministic source sanitizer/token logic unless it cannot satisfy the locked fixtures. [VERIFIED: current repository has no parser dependency for this boundary]
- Do not derive expected API coverage from production exports or metadata; keep the positive inventory explicit and independent. [VERIFIED: D-16]
- Do not use method-name substring scans as the authority, inline suppression comments/attributes, or renamed compatibility wrappers. [VERIFIED: D-06, D-11]

## Implementation Plan Inputs

### Adapter guard work

- Consolidate root/file ownership inventory and recursive discovery in `product_runtime_boundary_guards.rs` or a focused support module.
- Preserve the 16 deleted-method receiver-aware ledger and exactly four test-only `load_plugins` exceptions.
- Add positive/negative scanner fixtures for real canonical violations, legitimate same-name receivers, multiline/chained/parenthesized calls, comments/strings/doc comments, and `#[cfg(test)]` code.
- Add checks for local production `#[allow(deprecated)]` and any alternate wrapper/suppression mechanism across all four adapter families.

### Stable facade work

- Keep `public_api.rs` as an explicit positive contract inventory; expand it only with required canonical variants/outcomes/support types.
- Add external-consumer compile fixtures grouped as: internal `Operation`/dispatch metadata; runtime services; plugin load options/registries; Flow contracts. Exercise `api`, crate-root, and doc-hidden paths for each group.
- Prefer Rust visibility/sealed contracts and exhaustive matching; retain source scans only for calls, suppression directives, ownership, and synonym detection that Rust cannot express.

### Closure/documentation work

- Run focused guards first, then `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, source audits, `cargo test --workspace`, `cargo check --workspace`, and `git diff --check`.
- Write one closure report under this phase directory containing GUARD/CLOSE requirement mapping, deleted-method and retained-API conclusions, structured verification evidence, commit/worktree state, and Stage 10 handoff (untyped `ProductEvent` families plus compatibility subscriptions and source locations).
- Update `.planning/PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, current architecture/design docs, and mark `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` superseded with a closure-report link. [VERIFIED: D-17..D-21]

## Runtime State Inventory

This is a rename/refactor-style boundary closure, so the five required state categories are explicit:

1. **Identity:** `CodingAgentSession::run(CodingAgentOperation)` and `pi_coding_agent::api` are the sole canonical live-session operation identity; deleted broad method names must remain absent. [VERIFIED: Phase 4 ledger]
2. **Behavior:** JSON/print/RPC/interactive outputs, errors, event ordering, control handling, replay, navigation continuity, and `PartialCommit` operation IDs remain unchanged. [VERIFIED: project constraints and Phase 2-4 verification]
3. **Data/durability:** typed session facts, append/manifest ordering, operation identifiers, replay authority, recovery markers, and pending delegation state remain authoritative. [VERIFIED: project architecture and Phase 2 durable tests]
4. **Dependencies/ownership:** adapters own projection only; product runtime owns operations/services; lower crates remain product-neutral; every adapter source file has exactly one inventory root. [VERIFIED: architecture docs and D-01..D-05]
5. **Validation/evidence:** guard/API fixtures, focused and workspace Cargo commands, source audits, formatting/diff checks, closure report, and Stage 10 handoff are the required final proof set. [VERIFIED: CLOSE-01..04 and D-17..D-21]

## Security Domain and ASVS Applicability

Security enforcement is applicable. The relevant OWASP ASVS Level 1 domains are:

| Category | Applicability | Required control |
|---|---|---|
| V3 Session Management | Yes | Preserve replay authority, typed transactions, event sequence continuity, and navigation/session recovery assertions. [VERIFIED: Phase 2-4 evidence] |
| V4 Access Control | Yes | External consumers can use admitted public operations only; services, dispatch metadata, plugin options/registries, Flow nodes, and test fault controls remain inaccessible. [VERIFIED: D-12..D-16] |
| V5 Input Validation | Yes | Structural receiver recognition and typed operation/API fixture inputs reject unknown or malformed access paths fail-closed. [VERIFIED: D-05..D-10] |
| V2 Authentication | No behavior change | Preserve existing auth/provider fixtures; no credential semantics are changed. [VERIFIED: phase boundary] |
| V6 Cryptography | Not applicable | No cryptographic or credential-storage changes are planned. [VERIFIED: phase boundary] |

Threats to keep in each plan's verification register: a private/broad path bypassing admission; old facade recreated under a synonym; scanner false positives deleting legitimate same-name calls; scanner false negatives from formatting/test-only code; and durable evidence being weakened while guards are refactored. Mitigations are the compiler-first fixtures, receiver-aware fail-closed scanner, explicit exception counts, and preserved behavior/durability tests. [VERIFIED: prior phase threat register and locked decisions]

## Validation Architecture

| Layer | Command/fixture | Purpose |
|---|---|---|
| Scanner/API focused | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test api_boundary_guards --test public_api -- --nocapture` | Recursive adapter ownership/call/suppression guards and explicit facade contracts |
| Crate regression | `cargo test -p pi-coding-agent`; `cargo check -p pi-coding-agent` | Preserve all product behavior and compile visibility |
| Formatting/diff | `cargo fmt --check`; `git diff --check` | Repository hygiene |
| Workspace closure | `cargo test --workspace`; `cargo check --workspace` | Cross-crate compatibility and dependency-direction proof |
| Source audit | Receiver-aware old-definition/call/synonym audit plus production suppression audit | CLOSE-01 and deletion boundary |
| Evidence | Closure report with command, timestamp, status, counts/conclusion, commit/worktree state | CLOSE-04 reproducibility |

No manual-only verification is required; every phase requirement has an automated or structured evidence check. [VERIFIED: Phase 4 validation pattern and CLOSE-01..04]

## Common Pitfalls

- Recursive scans that silently skip a new adapter file or accept duplicate ownership violate fail-closed coverage. [VERIFIED: D-02..D-05]
- Name-only matching confuses `Agent::prompt`/service methods with deleted `CodingAgentSession` methods. [VERIFIED: prior receiver-aware ledger]
- Scanning comments, strings, doc examples, or `#[cfg(test)]` code creates false positives; fixtures must lock these exclusions. [VERIFIED: D-09]
- Compile-fail tests that inspect only `api` miss crate-root/doc-hidden escape paths. [VERIFIED: D-13..D-15]
- Generating expected API variants from exports makes the test self-approving. [VERIFIED: D-16]
- Rewriting the historical plan or embedding megabytes of Cargo output makes closure evidence non-auditable. [VERIFIED: D-18..D-20]
- Treating Stage 10 event payload/subscription work as Phase 5 scope violates the explicit deferral. [VERIFIED: phase boundary and D-21]

## Research Gaps / Assumptions

- Exact compile-fail harness layout and scanner token implementation are intentionally discretionary; planner should select the smallest deterministic fixture mechanism that covers every locked access path. [VERIFIED: D-12..D-16]
- No external packages are expected; if implementation discovers a parser dependency is unavoidable, pause for package-legitimacy verification before adding it. [VERIFIED: manifests and current guard implementation]

## Sources

- `.planning/phases/05-boundary-enforcement-and-stage-9-closure/05-CONTEXT.md` (locked decisions and canonical references).
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md` (phase scope and completion criteria).
- `.planning/phases/04-test-convergence-and-compatibility-deletion/04-RESEARCH.md` and `04-VALIDATION.md` (deletion baseline, security and validation patterns).
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`, `api_boundary_guards.rs`, `public_api.rs` (current executable guards and facade inventory).
- `crates/pi-coding-agent/src/lib.rs`, `src/coding_session/mod.rs` (stable facade and session owner boundary).
- `AGENTS.md` and project architecture/stack documents (repository constraints).

## RESEARCH COMPLETE
