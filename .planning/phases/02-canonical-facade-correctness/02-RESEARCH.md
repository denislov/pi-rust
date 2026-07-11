# Phase 2: Canonical Facade Correctness - Research

**Researched:** 2026-07-11  
**Domain:** Rust public API closure, typed operation dispatch, outcome projection, and durable-session behavior verification  
**Confidence:** HIGH - conclusions are grounded in the current repository source, tests, Phase 1 audit, and locked Phase 2 context. [VERIFIED: `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md`; `crates/pi-coding-agent/src/coding_session/{public_operation.rs,operation.rs,mod.rs}`]

<user_constraints>
## User Constraints (from CONTEXT.md)

The following content is copied verbatim from `02-CONTEXT.md`. [VERIFIED: `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md:13`]

### Locked Decisions

### Stable API Completeness
- **D-01:** Every public type that appears in the stable operation facade's signatures must be directly re-exported from `pi_coding_agent::api`. Callers must not need to know the type's implementation module or use crate-root compatibility exports.
- **D-02:** The stable API completeness boundary includes the full operation input/outcome/error type closure plus the existing live-session contracts required for create/open, snapshot/query, event subscription, and control.
- **D-03:** Session construction, observation, subscription, and control remain distinct contracts rather than being forced into `CodingAgentOperation`. Their inclusion in `api` is not permission to expose internal services, registries, dispatch metadata, Flow nodes, raw plugin options, or provider internals.
- **D-04:** Crate-root compatibility exports remain available during Phase 2, but all first-party code added or modified in this phase must use `pi_coding_agent::api`. Compatibility deletion remains Phase 4 work.
- **D-05:** Prove API completeness and privacy with Rust visibility plus positive and negative compile/API tests wherever possible. Use source-scanning guards only for boundaries the language cannot express.

### Dispatcher And Projection Exhaustiveness
- **D-06:** Maintain a per-variant contract matrix for all 15 current `CodingAgentOperation` variants. Each entry must independently prove public-to-internal mapping, expected metadata dispatch mode, and public outcome family.
- **D-07:** Keep `CodingAgentOperation::into_internal`, `Operation::metadata`, and `CodingAgentOperationOutcome::from_internal` as exhaustive matches so a new variant creates compiler-visible work.
- **D-08:** Keep the expected operation inventory independent from implementation metadata. Do not generate expected dispatch modes or outcome families from the implementation being tested.
- **D-09:** Use owner unit tests to inspect internal conversion, metadata, and projection. Use public integration tests that import contracts only through `pi_coding_agent::api` and execute work through `CodingAgentSession::run`.
- **D-10:** Prove that `run` follows metadata with direct metadata assertions plus dispatcher-specific behavior tests for async, sync-read-only, and sync-mutable paths. Do not add production dispatcher instrumentation for testing.

### Durable Mutation Verification
- **D-11:** Add focused canonical-facade behavior evidence for every FACADE-05 high-risk operation: session fork, active-leaf switch, branch-summary reuse, plugin load and command, default-profile mutation, and delegation approval and rejection.
- **D-12:** Apply a shared invariant checklist to those operations: correct public outcome, intended state effect, unchanged error semantics, reopen/replay persistence when applicable, semantic product events and sequence continuity when applicable, and explicit `PartialCommit` when a durable operation crosses a partial-commit boundary.
- **D-13:** Add operation-specific assertions beyond the shared checklist, including session-tree and active-leaf state, branch-summary reuse behavior, plugin registry/command effects, default-profile state, and delegation queue/decision effects as applicable.
- **D-14:** Reuse existing deterministic fixtures and behavior assertions, but make canonical correctness tests call `CodingAgentSession::run` directly. Do not make the canonical suite depend on differential execution against workflow-specific methods scheduled for deletion.
- **D-15:** For persistence-sensitive operations, reuse deterministic failure injection to cover failure before append with no durable mutation, failure after append but before manifest/publication with explicit `PartialCommit`, and reopen/replay recovery with the durable log as authority.
- **D-16:** Do not expand Phase 2 into torn JSONL recovery, interrupted manifest replacement, power-loss durability, atomic filesystem transactions, or other independent crash-consistency initiatives.

### the agent's Discretion
- Choose the exact organization of the operation contract matrix and test helper names, provided expected values remain independent from implementation metadata and helpers stay test-only.
- Reuse or extend existing fixtures at the owning test layer without creating a new production compatibility facade.
- Select precise negative API-test mechanics according to what stable Rust and the existing test harness can express; retain narrowly scoped source guards only where compiler-visible enforcement is impractical.

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within the Phase 2 facade-correctness scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FACADE-01 | First-party callers can obtain the complete stable `CodingAgentOperation`, outcome, and required support types through `pi_coding_agent::api`. | Build a signature-closure ledger from every public `CodingAgentSession` facade method and every operation/outcome payload, then prove it through one facade-only integration import surface. [VERIFIED: `.planning/REQUIREMENTS.md:16`; `crates/pi-coding-agent/src/lib.rs:64`]
| FACADE-02 | `CodingAgentSession::run` converts every public operation to an internal operation and selects the async, sync-read-only, or sync-mutable dispatcher from operation metadata. | Add an owner-owned 15-row conversion/metadata matrix and retain dispatcher-specific behavior tests without production instrumentation. [VERIFIED: `.planning/REQUIREMENTS.md:17`; `crates/pi-coding-agent/src/coding_session/mod.rs:249`; `crates/pi-coding-agent/src/coding_session/operation.rs:72`]
| FACADE-03 | Every internal operation outcome is projected through one exhaustive mapping into a public operation outcome. | Test every `OperationOutcome` family directly at the owner layer, including the two-path export projection, while preserving the exhaustive match. [VERIFIED: `.planning/REQUIREMENTS.md:18`; `crates/pi-coding-agent/src/coding_session/public_operation.rs:161`]
| FACADE-04 | Internal operations, dispatch metadata, plugin load options, services, and Flow nodes are not exposed through the stable API. | Prefer compiler visibility/compile-fail API checks for named private contracts; retain source guards only for root-module and compatibility annotations that Rust cannot assert as a downstream import. [VERIFIED: `.planning/REQUIREMENTS.md:19`; `crates/pi-coding-agent/tests/api_boundary_guards.rs:4`]
| FACADE-05 | Fork, active-leaf switch, branch-summary reuse, plugin, profile, and delegation operations preserve their persistence, event-continuity, and error semantics. | Reuse current deterministic fixtures, but invoke each behavior through `CodingAgentSession::run` and apply a shared state/event/error/reopen/partial-commit checklist. [VERIFIED: `.planning/REQUIREMENTS.md:20`; `crates/pi-coding-agent/src/coding_session/mod.rs:3187`]
</phase_requirements>

## Summary

Phase 2 should be planned as contract completion and behavior-proof work, not as a dispatcher rewrite. The current implementation already has a single public `CodingAgentSession::run` method that converts a public operation, reads internal metadata, selects one of three dispatchers, and projects the internal result; both conversion directions and metadata are exhaustive `match` expressions. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:249`; `crates/pi-coding-agent/src/coding_session/public_operation.rs:107`; `crates/pi-coding-agent/src/coding_session/operation.rs:72`]

The planning gap is evidence structure. Existing tests prove selected public variants and several high-risk operations, but they do not yet form the locked independent 15-variant conversion/dispatch/outcome matrix, and the public facade import test is a broad hand-maintained list rather than an explicit closure audit tied to facade signatures. [VERIFIED: `crates/pi-coding-agent/tests/public_api.rs:8`; `crates/pi-coding-agent/tests/public_api.rs:127`; `.planning/phases/01-evidence-based-baseline/01-AUDIT.md`]

FACADE-05 should be a focused canonical-path verification slice. Existing owner tests already cover canonical fork, switch, branch-summary reuse, plugin load, profile mutation, delegation error/admission behavior, replay, event sequencing, and partial-commit primitives; the phase should consolidate and extend those assertions through `run`, especially for plugin command success, profile persistence/reopen, delegation approve/reject state effects, and deterministic before/after-append failure boundaries. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:2770`; `crates/pi-coding-agent/src/coding_session/mod.rs:3187`; `crates/pi-coding-agent/src/coding_session/mod.rs:3430`; `crates/pi-coding-agent/src/coding_session/mod.rs:4200`; `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs:1060`]

**Primary recommendation:** Plan three implementation waves: stable API closure/privacy, owner-owned 15-variant contract matrix plus dispatcher proof, then canonical FACADE-05 durability scenarios with focused and workspace verification. [VERIFIED: `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md:16`; `.planning/ROADMAP.md:34`]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Stable public operation facade | `pi-coding-agent` crate API facade | External Rust integration tests | Product contracts are curated in `lib.rs::api`; downstream tests prove only stable imports are required. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; `crates/pi-coding-agent/tests/public_api.rs:8`]
| Public-to-internal conversion and outcome projection | `coding_session/public_operation.rs` | `coding_session/mod.rs` owner tests | Conversion belongs beside public operation types; owner tests can inspect crate-private internal variants. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_operation.rs:42`; `crates/pi-coding-agent/src/coding_session/mod.rs:2220`]
| Metadata and dispatch classification | `coding_session/operation.rs` | `CodingAgentSession::run` | Internal metadata remains private and is consumed by the session owner to select the dispatcher. [VERIFIED: `crates/pi-coding-agent/src/coding_session/operation.rs:72`; `crates/pi-coding-agent/src/coding_session/mod.rs:253`]
| Durable mutation semantics | `CodingAgentSession` plus `SessionService`/session log | Public facade integration tests | The session owner coordinates services and events; durable truth remains in typed session facts and replay. [VERIFIED: `.planning/PROJECT.md`; `crates/pi-coding-agent/src/coding_session/session_service.rs`; `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`]
| API privacy enforcement | Rust visibility and integration compilation | Narrow source guards | Private modules/types should fail downstream access; textual structure checks remain useful only for annotations and forbidden source patterns. [VERIFIED: `crates/pi-coding-agent/tests/api_boundary_guards.rs:4`; `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md:21`]

## Project Constraints (from AGENTS.md)

- Use Chinese for user communication; technical documentation may be written entirely in English. [VERIFIED: `AGENTS.md`]
- In this indexed repository, use CodeGraph before grep/find/direct code reads when locating or understanding code. [VERIFIED: `AGENTS.md`; `.codegraph/`]
- Preserve the dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; keep product semantics in `pi-coding-agent`. [VERIFIED: `AGENTS.md`; `.planning/PROJECT.md`]
- New or migrated consumers use `pi_coding_agent::api`; internal operation metadata, plugin options, services, and Flow nodes remain private. [VERIFIED: `AGENTS.md`; `.planning/PROJECT.md`]
- Preserve JSON/print/RPC/interactive behavior, typed durability, replay authority, event ordering, recovery markers, and explicit `PartialCommit`. [VERIFIED: `AGENTS.md`; `.planning/PROJECT.md`]
- Use deterministic offline fixtures and retain behavioral assertions; compile-only checks are insufficient for runtime compatibility. [VERIFIED: `AGENTS.md`; `.planning/PROJECT.md`]
- Production adapters and tests migrate before broad compatibility methods are deleted; Phase 2 does neither broad migration nor deletion. [VERIFIED: `AGENTS.md`; `.planning/ROADMAP.md:49`; `02-CONTEXT.md:9`]
- Required closure commands include formatting, focused crate tests, workspace tests/check, source audits, and `git diff --check`. [VERIFIED: `AGENTS.md`; `.planning/PROJECT.md`]

## Standard Stack

### Core

| Library/Facility | Version | Purpose | Why Standard Here |
|------------------|---------|---------|-------------------|
| Rust | 1.96.0 installed; edition 2024 | Exhaustive enums/matches, visibility, async facade | The workspace is Rust 2024 and compiler exhaustiveness is the strongest guard for operation inventory changes. [VERIFIED: `rustc --version`; `Cargo.toml:9`]
| Cargo test harness | Cargo 1.96.0 installed | Owner unit tests and downstream integration tests | Existing tests are standard `#[test]`/`#[tokio::test]` suites under the owning crate. [VERIFIED: `cargo --version`; `crates/pi-coding-agent/tests/public_api.rs`]
| Tokio | workspace lock resolves 1.52.3 | Async session operations and deterministic async tests | `CodingAgentSession::run` is async and the crate enables `macros`, runtime, sync, and test utilities. [VERIFIED: `Cargo.lock`; `crates/pi-coding-agent/Cargo.toml:28`]
| `tempfile` | workspace lock resolves 3.27.0 | Isolated persistent-session/plugin/profile fixtures | Existing public and owner tests create deterministic session roots and project/global roots with temporary directories. [VERIFIED: `Cargo.lock`; `crates/pi-coding-agent/tests/public_api.rs:179`]
| Faux provider/test support | repository-owned | Offline prompt/agent/team behavior | Existing fixtures avoid live providers while preserving streaming and error behavior. [VERIFIED: `crates/pi-coding-agent/tests/support/mod.rs`; `crates/pi-coding-agent/src/coding_session/mod.rs:2223`]

### Supporting

| Facility | Purpose | When to Use |
|----------|---------|-------------|
| Rust visibility and downstream integration compilation | Positive stable import and negative private-boundary proof | Use for FACADE-01/FACADE-04 whenever a type boundary is expressible by the language. [VERIFIED: `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md:21`]
| Source guards | Verify `#[doc(hidden)]`, deprecation markers, and the shape of `run` | Retain only where a downstream crate cannot express the intended structural rule. [VERIFIED: `crates/pi-coding-agent/tests/api_boundary_guards.rs:4`]
| Session log failure injection and filesystem fixtures | Verify no-commit, in-doubt/partial-commit, and replay recovery | Use only on persistence-sensitive FACADE-05 paths; do not broaden into crash-consistency redesign. [VERIFIED: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs:1060`; `02-CONTEXT.md:35`]

No external package installation is needed for this phase. [VERIFIED: `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md`; `crates/pi-coding-agent/Cargo.toml`]

## Architecture Patterns

### System Architecture Diagram

```text
first-party/downstream caller
        |
        | imports only pi_coding_agent::api
        v
CodingAgentOperation (15 public variants)
        |
        | exhaustive into_internal
        v
private Operation -----------------> private Operation::metadata
        |                                      |
        |                                      v
        |                         Async / SyncReadOnly / SyncMutable
        |                                      |
        +-------------------- CodingAgentSession::run -------------------+
                                      |                                  |
                                      v                                  v
                         owner dispatcher/services              session log/events/state
                                      |                                  |
                                      +---------------+------------------+
                                                      v
                                           private OperationOutcome
                                                      |
                                                      | exhaustive from_internal
                                                      v
                                       CodingAgentOperationOutcome
```

This is the current intended and mostly implemented path; Phase 2 should strengthen its public closure and evidence rather than add another facade. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:249`; `.planning/PROJECT.md`]

### Recommended Test Ownership

```text
crates/pi-coding-agent/src/coding_session/
├── public_operation.rs       # conversion/projection owner tests or test helpers
├── operation.rs              # metadata inventory owner tests
└── mod.rs                    # dispatcher behavior and durable operation scenarios

crates/pi-coding-agent/tests/
├── public_api.rs             # facade-only imports and run-based external behavior
├── api_boundary_guards.rs    # privacy/structural fallback guards
└── support/mod.rs            # deterministic reusable fixtures only
```

This division matches the locked requirement that private conversion/metadata/projection be inspected by owner tests and stable usability be proven externally. [VERIFIED: `02-CONTEXT.md:27`; existing paths above]

### Pattern 1: Independent Contract Ledger

Define test-only expected rows independently from `Operation::metadata`, with one row for each public variant. Do not derive expected dispatch or outcome values by calling the same implementation under test. [VERIFIED: `02-CONTEXT.md:24`; `02-CONTEXT.md:26`]

| Public Variant | Internal Variant | Expected Dispatch | Public Outcome Family |
|----------------|------------------|-------------------|-----------------------|
| `Prompt` | `Operation::Prompt` | `Async` | `Prompt` |
| `Compact` | `Operation::ManualCompaction` | `Async` | `Compact` |
| `BranchSummary` | `Operation::BranchSummary` | `Async` | `BranchSummary` |
| `SelfHealingEdit` | `Operation::SelfHealingEdit` | `Async` | `SelfHealingEdit` |
| `InvokeAgent` | `Operation::AgentInvocation` | `Async` | `AgentInvocation` |
| `InvokeTeam` | `Operation::AgentTeam` | `Async` | `AgentTeam` |
| `PluginLoad` | `Operation::PluginLoad` | `Async` | `PluginLoad` |
| `PluginCommand` | `Operation::PluginCommand` | `SyncReadOnly` | `PluginCommand` |
| `SetDefaultAgentProfile` | `Operation::SetDefaultAgentProfile` | `SyncMutable` | `DefaultAgentProfileChanged` |
| `ApproveDelegation` | `Operation::ApproveDelegationConfirmation` | `Async` | `DelegationApproved` |
| `RejectDelegation` | `Operation::RejectDelegationConfirmation` | `SyncMutable` | `DelegationRejected` |
| `ForkSession` | `Operation::ForkSession` | `SyncMutable` | `SessionForked` |
| `SwitchActiveLeaf` | `Operation::SwitchActiveLeaf` | `SyncMutable` | `ActiveLeafSwitched` |
| `ExportCurrent` | `Operation::Export(view)` | `SyncReadOnly` | `Export` |
| `ExportCurrentHtml` | `Operation::Export(html)` | `SyncReadOnly` | `ExportHtml` |

All 15 mappings above are directly evidenced by the current exhaustive conversion, metadata, and projection matches. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_operation.rs:107`; `crates/pi-coding-agent/src/coding_session/operation.rs:72`; `crates/pi-coding-agent/src/coding_session/public_operation.rs:161`]

### Pattern 2: Signature Type-Closure Audit

Treat stable API completeness as a transitive type closure: collect every public type named by `CodingAgentSession::{create,open,open_or_create,list,run,snapshot,connect,subscribe_product_events_public,capabilities,view,...}` plus every payload nested in `CodingAgentOperation`, `CodingAgentOperationOutcome`, and `CodingSessionError`, then ensure each caller-facing named type is re-exported directly by `api`. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; `crates/pi-coding-agent/src/coding_session/mod.rs:249`]

The current `api` module already exports the central session, operation, outcome, error, snapshot, client, event, profile, prompt, agent/team, self-healing, and plugin projection types, so the plan should begin with an explicit closure audit and add only confirmed omissions. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:66`]

### Pattern 3: Behavior Triangulation for Durable Operations

For each high-risk operation, assert the public outcome, immediate owner state, semantic event sequence, durable files/facts, and reopened/replayed state where applicable. Add error-path assertions at the same ownership layer. [VERIFIED: `02-CONTEXT.md:31`; `02-CONTEXT.md:32`]

Use a shared test-only checklist helper for common assertions, but keep operation-specific checks visible: forked session identity/tree, selected active leaf, reused summary identity/content, loaded plugin/command output, profile manifest/snapshot, and delegation pending/decision state. [VERIFIED: `02-CONTEXT.md:33`]

### Anti-Patterns to Avoid

- **Testing metadata with metadata:** expected dispatch values generated from `Operation::metadata` cannot detect an incorrect metadata row. [VERIFIED: `02-CONTEXT.md:26`]
- **Compile-only durability claims:** an importable variant does not prove persistence, replay, events, or errors. [VERIFIED: `.planning/PROJECT.md`; `02-CONTEXT.md:32`]
- **Differential tests against compatibility methods:** broad workflow methods are scheduled for deletion and must not become permanent test oracles. [VERIFIED: `02-CONTEXT.md:34`; `.planning/ROADMAP.md:63`]
- **Production test instrumentation:** do not add dispatcher counters/hooks solely to prove routing; owner metadata assertions and behavior-specific dispatcher tests are sufficient. [VERIFIED: `02-CONTEXT.md:28`]
- **Leaking internals to simplify tests:** do not make `Operation`, metadata, services, plugin load options, or Flow nodes public. [VERIFIED: `.planning/REQUIREMENTS.md:19`]
- **Expanding into adapter migration or crash consistency:** these belong to Phase 3/later work or remain out of scope. [VERIFIED: `.planning/ROADMAP.md:49`; `02-CONTEXT.md:36`]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Operation inventory enforcement | Runtime string registry or generated metadata table | Rust enums plus exhaustive `match` owner tests | Compiler-visible work is created when variants change. [VERIFIED: `02-CONTEXT.md:25`]
| Dispatcher observation | Production instrumentation/counters | Direct metadata assertions plus existing busy/error behavior tests for each dispatcher family | Avoids changing runtime semantics for testability. [VERIFIED: `02-CONTEXT.md:28`; `crates/pi-coding-agent/src/coding_session/mod.rs:3105`]
| Stable facade duplication | New compatibility module or wrapper facade | Curated `pi_coding_agent::api` re-exports | The project already defines one stable facade and keeps root exports only as deprecated compatibility. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; `crates/pi-coding-agent/tests/api_boundary_guards.rs:43`]
| Persistence simulator | New in-memory durability model | Existing `SessionService`, session-log transaction, replay, tempfile, and failure fixtures | These exercise the real typed append/manifest/replay ordering. [VERIFIED: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`; `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`]
| Live provider integration | Network-backed model calls | Existing faux providers and scripted streams | Deterministic offline behavior is a project constraint. [VERIFIED: `AGENTS.md`; `crates/pi-coding-agent/tests/support/mod.rs`]

## Runtime State Inventory

Phase 2 is a refactor/migration verification phase, so all five runtime-state categories were checked. [VERIFIED: `.planning/PROJECT.md`; `02-CONTEXT.md`]

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | Rust-native session directories contain `session.json` and `events.jsonl`; FACADE-05 operations can affect active leaf, profile, branch summary, plugin facts, and delegation facts. [VERIFIED: `crates/pi-coding-agent/src/coding_session/session_log`; `.planning/codebase/STACK.md`] | No schema/name migration. Reuse temporary persistent sessions to prove current records replay correctly through canonical operations. |
| Live service config | None required for Phase 2; plugin/profile roots are local fixture directories, not external service configuration. [VERIFIED: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`; `crates/pi-coding-agent/src/coding_session/profiles.rs`] | Create isolated project/global fixture roots only. |
| OS-registered state | None; the product is a local CLI/TUI and this phase does not rename/register OS services. [VERIFIED: `.planning/codebase/STACK.md`; `02-CONTEXT.md`] | None. |
| Secrets/env vars | `PI_RUST_DIR` and provider-related process state exist, but Phase 2 neither renames nor changes them; existing guards serialize/restore mutations. [VERIFIED: `crates/pi-coding-agent/tests/support/mod.rs`; `crates/pi-coding-agent/src/lib.rs:120`] | Reuse `EnvGuard`/provider guards; do not add new global state. |
| Build artifacts/installed packages | Cargo target artifacts may be regenerated; no installed package or binary rename is part of the phase. [VERIFIED: `Cargo.toml`; `02-CONTEXT.md`] | No migration; normal focused/full Cargo verification only. |

## Common Pitfalls

### Pitfall 1: Missing a Support Type in the Stable Closure

**What goes wrong:** `CodingAgentOperation` is exported, but a constructor, nested payload, outcome accessor, event receiver, or control type still requires an implementation-module import. [VERIFIED: `02-CONTEXT.md:17`]

**How to avoid:** derive a written type-closure ledger from public signatures before editing exports, and make `public_api.rs` instantiate or name every closure type through one `pi_coding_agent::api` import. [VERIFIED: `crates/pi-coding-agent/tests/public_api.rs:8`]

### Pitfall 2: Collapsing Two Export Variants

`OperationOutcome::Export` projects to `CodingAgentOperationOutcome::Export` when no path is present and `ExportHtml` when a path is present. A projection matrix that treats this as one public family misses a real branch. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_operation.rs:176`]

### Pitfall 3: Misclassifying Delegation Approval

Approval has dynamic operation kind resolution and uses the async dispatcher, while rejection has a static delegation-confirmation kind and uses the sync-mutable dispatcher. Tests must not assume both decisions share metadata or execution behavior. [VERIFIED: `crates/pi-coding-agent/src/coding_session/operation.rs:98`; `crates/pi-coding-agent/src/coding_session/mod.rs:1591`]

### Pitfall 4: Calling SyncReadOnly “No Mutation Anywhere”

`PluginCommand` is classified `SyncReadOnly` at operation-control/admission level even though the invoked plugin command may have plugin-defined effects outside session durable state. Plan assertions around session-owner dispatch/admission semantics, not a universal purity claim. [VERIFIED: `crates/pi-coding-agent/src/coding_session/operation.rs:92`; `crates/pi-coding-agent/src/coding_session/mod.rs:3498`]

### Pitfall 5: Proving Only the Happy Durable Path

A successful state assertion does not prove the required boundary between no append, appended-but-not-published/manifested, and replay recovery. Use existing deterministic failure mechanisms and assert explicit `PartialCommit` only after durable facts may have landed. [VERIFIED: `02-CONTEXT.md:35`; `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs:1060`]

### Pitfall 6: Duplicating Existing Tests Without Canonicalizing Them

Several behaviors already have strong compatibility-method tests elsewhere. Phase 2 should reuse fixtures/assertions but add focused `run`-based evidence, not copy entire suites or broadly migrate all tests before Phase 4. [VERIFIED: `02-CONTEXT.md:34`; `.planning/ROADMAP.md:62`]

## Code Examples

### Independent Owner Matrix Shape

```rust
// Test-only expected values; do not derive them from Operation::metadata.
struct OperationContractCase {
    name: &'static str,
    public: CodingAgentOperation,
    expected_internal: ExpectedInternalVariant,
    expected_dispatch: OperationDispatchMode,
    expected_outcome: ExpectedPublicOutcomeFamily,
}
```

This is a recommended test-only organization inferred from the locked independent-matrix decision; exact helper names remain discretionary. [VERIFIED: `02-CONTEXT.md:24`; `02-CONTEXT.md:39`]

### Canonical Public Integration Shape

```rust
use pi_coding_agent::api::{
    CodingAgentOperation, CodingAgentOperationOutcome, CodingAgentSession,
    CodingAgentSessionOptions,
};

let outcome = session.run(CodingAgentOperation::ExportCurrent).await?;
assert!(matches!(outcome, CodingAgentOperationOutcome::Export(_)));
```

This pattern already exists in the public integration suite and should be used for all new external-consumer evidence. [VERIFIED: `crates/pi-coding-agent/tests/public_api.rs:177`]

### Durable Invariant Shape

```rust
let before = session.snapshot();
let mut events = session.subscribe_product_events_public();
let outcome = session.run(operation).await?;
let after = session.snapshot();

assert_public_outcome(&outcome);
assert_operation_specific_state(&before, &after);
assert_semantic_event_sequence(&mut events);

drop(session);
let reopened = CodingAgentSession::open(options).await?;
assert_replayed_state(&reopened.snapshot());
```

The exact helper APIs may differ, but the assertion sequence follows the locked shared invariant checklist and existing session/event APIs. [VERIFIED: `02-CONTEXT.md:32`; `crates/pi-coding-agent/src/coding_session/mod.rs:273`; `crates/pi-coding-agent/src/coding_session/mod.rs:512`]

## Existing Evidence and Exact Gaps

| Area | Existing Evidence | Planning Gap |
|------|-------------------|--------------|
| Stable facade | `api` exports a large curated surface; `public_api.rs` imports only through `api`. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; `crates/pi-coding-agent/tests/public_api.rs:8`] | Build an explicit operation-facade type closure and add confirmed missing exports/tests rather than assuming the current list is complete. |
| Canonical `run` shape | Source guard checks conversion, metadata selection, all three dispatchers, and centralized projection. [VERIFIED: `crates/pi-coding-agent/tests/api_boundary_guards.rs:79`] | Add behavior/owner proof so FACADE-02 is not supported primarily by text matching. |
| Variant visibility | Public test constructs high-risk variants and outcome families. [VERIFIED: `crates/pi-coding-agent/tests/public_api.rs:127`] | Cover all 15 variants, mapping, dispatch, and outcome family independently. |
| Async dispatcher | Existing owner tests cover agent/team/self-healing/branch/plugin-load/compact/prompt guards and errors. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:3082`; `crates/pi-coding-agent/src/coding_session/mod.rs:3712`] | Tie representative behavior plus direct metadata assertions to the independent matrix. |
| Sync read-only dispatcher | Existing export and plugin-command busy/error tests exist. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:3105`; `crates/pi-coding-agent/src/coding_session/mod.rs:3498`] | Add successful plugin-command public projection and explicit matrix coverage for both export public outcomes. |
| Sync mutable dispatcher | Canonical switch/fork tests and profile/rejection busy tests exist. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:3147`; `crates/pi-coding-agent/src/coding_session/mod.rs:3187`] | Apply the same canonical outcome/state/event/reopen checklist to profile and delegation mutations. |
| Partial commit | Canonical switch partial-commit and transaction in-doubt tests exist. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:3430`; `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs:1060`] | Identify which FACADE-05 operations cross the same boundary and add only operation-relevant before/after append cases. |
| Branch summary reuse | Canonical `run` reuse test exists. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:4200`] | Ensure outcome, no duplicate durable summary, event sequence, and reopen/replay assertions are all explicit. |
| Plugin/profile/delegation | Strong fixture coverage exists across owner and integration suites. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:2770`; `crates/pi-coding-agent/tests/agent_profile_session.rs`; `crates/pi-coding-agent/tests/delegation_execution.rs`] | Create focused canonical `run` tests without broadly migrating those suites in Phase 2. |

## Recommended Plan Decomposition

1. **Stable facade closure and privacy:** inventory the operation/session signature closure, fix `api` re-exports, strengthen facade-only positive tests, and add compiler-visible negative checks where practical. [VERIFIED: FACADE-01; FACADE-04; `02-CONTEXT.md:16`]
2. **Exhaustive contract matrix and dispatcher proof:** add independent owner test rows for all 15 public variants, direct internal mapping/metadata/projection checks, and representative async/read-only/mutable behavior assertions. [VERIFIED: FACADE-02; FACADE-03; `02-CONTEXT.md:23`]
3. **Canonical high-risk behavior:** add or refactor focused `run` tests for fork, switch, branch-summary reuse, plugin load/command, profile mutation, and delegation approve/reject using the shared invariant checklist and deterministic failure boundaries. [VERIFIED: FACADE-05; `02-CONTEXT.md:30`]
4. **Phase verification:** run formatting, focused owner/public/boundary suites, all `pi-coding-agent` tests, workspace tests/check, source audits, and diff checks. [VERIFIED: `.planning/PROJECT.md`; `AGENTS.md`]

Keep these tasks separate enough that an API-export failure, contract-matrix failure, and persistence failure identify distinct ownership boundaries. [VERIFIED: repository architecture and test layout]

## Environment Availability

Step 2.6 is effectively skipped for external dependencies because Phase 2 is code/test-only and introduces no new tool, service, package, or runtime dependency. [VERIFIED: `02-CONTEXT.md`; `crates/pi-coding-agent/Cargo.toml`]

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Build/tests | Yes | 1.96.0 | None required. [VERIFIED: `rustc --version`] |
| Cargo | Build/tests | Yes | 1.96.0 | None required. [VERIFIED: `cargo --version`] |
| CodeGraph CLI/index | Source discovery | Yes | Repository index present | `rg` only after CodeGraph per project rules. [VERIFIED: `.codegraph/`; successful `codegraph explore`] |

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness plus Tokio `#[tokio::test]`. [VERIFIED: `crates/pi-coding-agent/tests/public_api.rs`] |
| Config file | Cargo manifests; no separate test-runner config. [VERIFIED: `Cargo.toml`; `crates/pi-coding-agent/Cargo.toml`] |
| Quick run command | `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards` |
| Owner quick run command | `cargo test -p pi-coding-agent coding_session::tests::` |
| Full crate command | `cargo test -p pi-coding-agent` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FACADE-01 | Complete operation/session facade type closure imports only from `api` | Integration/API compile | `cargo test -p pi-coding-agent --test public_api public_api` | Yes, extend `crates/pi-coding-agent/tests/public_api.rs`. [VERIFIED: file exists] |
| FACADE-02 | All 15 public variants convert and use expected metadata dispatcher | Owner unit plus integration behavior | `cargo test -p pi-coding-agent coding_session::tests::operation_contract` | No exact matrix test yet - Wave 0 addition. [VERIFIED: current test inventory] |
| FACADE-03 | Every internal outcome projects exhaustively, including both export branches | Owner unit | `cargo test -p pi-coding-agent coding_session::tests::operation_outcome_projection` | No exact exhaustive projection test yet - Wave 0 addition. [VERIFIED: current test inventory] |
| FACADE-04 | Internal operations/metadata/plugin options/services/Flow nodes inaccessible through stable facade | Integration compile/API plus narrow source guard | `cargo test -p pi-coding-agent --test api_boundary_guards` | Yes, strengthen existing file and add compile mechanics if feasible. [VERIFIED: file exists] |
| FACADE-05 | High-risk operations preserve outcome/state/error/events/replay/partial commit | Owner integration-style unit tests with real temp session log | `cargo test -p pi-coding-agent canonical_` | Partial; several canonical tests exist, focused gaps remain. [VERIFIED: `crates/pi-coding-agent/src/coding_session/mod.rs:3187`; `:4200`] |

### Sampling Rate

- **Per task commit:** run the directly owned test target plus `cargo fmt --check`. [VERIFIED: project verification convention]
- **Per wave merge:** run `cargo test -p pi-coding-agent` and `cargo check -p pi-coding-agent`. [VERIFIED: `.planning/PROJECT.md`]
- **Phase gate:** run `cargo fmt --check`, `cargo test --workspace`, `cargo check --workspace`, focused source audits, and `git diff --check`. [VERIFIED: `.planning/PROJECT.md`; `AGENTS.md`]

### Wave 0 Gaps

- [ ] Add a test-only 15-row operation contract matrix at the owner layer; no new production API is required. [VERIFIED: `02-CONTEXT.md:24`; current test inventory]
- [ ] Add direct exhaustive internal-outcome projection tests, explicitly covering `Export` and `ExportHtml`. [VERIFIED: `crates/pi-coding-agent/src/coding_session/public_operation.rs:176`]
- [ ] Add an explicit stable facade signature-closure test/ledger in `public_api.rs`; extend exports only after the ledger identifies omissions. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; current broad import test]
- [ ] Add focused canonical success/state/reopen/event tests for plugin command, profile mutation, and delegation approval/rejection where current evidence is compatibility-path or error-path focused. [VERIFIED: existing test inventory]
- [ ] Reuse or expose existing test-only deterministic failure controls at their current owner boundary for no-append/partial-commit/replay scenarios; do not create production hooks. [VERIFIED: `02-CONTEXT.md:35`; session-log tests]

## Security Domain

Phase 2 changes no authentication, cryptography, provider transport, or credential format. Security relevance is limited to preserving access boundaries and preventing internal capability/service exposure. [VERIFIED: `02-CONTEXT.md`; `.planning/PROJECT.md`]

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No direct change | Preserve existing auth contracts; no facade expansion into provider/auth internals. [VERIFIED: `.planning/PROJECT.md`] |
| V3 Session Management | Yes, product-session integrity rather than web cookies | Typed session facts, replay authority, operation admission, and explicit partial-commit handling. [VERIFIED: `crates/pi-coding-agent/src/coding_session/session_log`; `CodingSessionError::PartialCommit`] |
| V4 Access Control | Yes | Rust visibility, curated `api` re-exports, capability snapshots, and private services/metadata. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`; `operation.rs`] |
| V5 Input Validation | Yes at existing operation boundaries | Preserve typed IDs/options and existing validation/error semantics; do not replace with untyped facade payloads. [VERIFIED: `public_operation.rs`; `CodingSessionError`] |
| V6 Cryptography | No direct change | Do not introduce or modify cryptographic code. [VERIFIED: Phase 2 boundary] |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Accidental exposure of internal services or capability metadata | Information Disclosure / Elevation of Privilege | Curated facade, crate-private types, downstream negative API checks. [VERIFIED: FACADE-04] |
| Mutation routed through the wrong admission/dispatcher class | Tampering | Independent metadata matrix and dispatcher behavior tests. [VERIFIED: FACADE-02] |
| Durable mutation reported as ordinary failure after append | Repudiation / Integrity | Preserve explicit `PartialCommit`, recovery markers, and replay authority. [VERIFIED: FACADE-05; session-log transaction tests] |
| Test fixtures mutating global environment/provider state concurrently | Integrity / Availability | Reuse serialized environment/provider guards. [VERIFIED: `crates/pi-coding-agent/tests/support/mod.rs`; `src/lib.rs:120`] |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| - | None. All implementation and planning claims in this research were verified against current repository artifacts or command output. | All | No user confirmation is required before planning. |

## Open Questions

1. **Which named support types, if any, are currently absent from `pi_coding_agent::api`?**
   - What we know: the facade already exports the central operation/session closure. [VERIFIED: `crates/pi-coding-agent/src/lib.rs:64`]
   - What remains: the planner should make the signature-closure audit the first task rather than presume an omission list from Phase 1. [VERIFIED: FACADE-01]
   - Recommendation: generate a test-owned ledger from current public signatures, then make the smallest export changes proven necessary.

2. **Which FACADE-05 mutations genuinely support before-append and after-append failure injection at their present owner boundary?**
   - What we know: transaction-level and switch-navigation partial-commit evidence exists. [VERIFIED: `session_log/transaction.rs:1060`; `coding_session/mod.rs:3430`]
   - What remains: not every runtime-only operation has a durable append boundary, so the planner must apply D-15 only where persistence is applicable. [VERIFIED: `02-CONTEXT.md:32`]
   - Recommendation: add a per-operation applicability column before assigning failure tests; do not force `PartialCommit` onto plugin command or other non-durable paths.

3. **What compile-negative mechanism best fits the existing stable Rust harness?**
   - What we know: current privacy enforcement is mostly Rust visibility plus source guards; no dedicated compile-fail framework is evident. [VERIFIED: `api_boundary_guards.rs`; Cargo manifests]
   - Recommendation: first test what can be proven by ordinary integration compilation and type naming; add no external package, and retain precise source guards for the remainder. [VERIFIED: `02-CONTEXT.md:41`]

## Sources

### Primary (HIGH confidence)

- `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md` - locked Phase 2 decisions, scope, and test ownership. [VERIFIED: repository]
- `.planning/REQUIREMENTS.md` and `.planning/ROADMAP.md` - FACADE-01..05 and phase success criteria. [VERIFIED: repository]
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - source-backed baseline and gap classification. [VERIFIED: repository]
- `crates/pi-coding-agent/src/lib.rs` - stable facade exports. [VERIFIED: current source]
- `crates/pi-coding-agent/src/coding_session/public_operation.rs` - public variants, conversion, and projection. [VERIFIED: current source]
- `crates/pi-coding-agent/src/coding_session/operation.rs` - internal variants and metadata. [VERIFIED: current source]
- `crates/pi-coding-agent/src/coding_session/mod.rs` - canonical run path, dispatchers, and owner tests. [VERIFIED: current source]
- `crates/pi-coding-agent/tests/public_api.rs` and `api_boundary_guards.rs` - current public and boundary evidence. [VERIFIED: current tests]
- `crates/pi-coding-agent/src/coding_session/session_log/{transaction.rs,replay.rs}` - append/manifest ordering, in-doubt semantics, and replay authority. [VERIFIED: current source/tests]

### Secondary (MEDIUM confidence)

- None. No external documentation or ecosystem research was needed because the phase uses the existing repository stack and installs no packages. [VERIFIED: Phase 2 scope]

### Tertiary (LOW confidence)

- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions and facilities were read from installed tools, Cargo manifests, and the lockfile. [VERIFIED: command output; manifests]
- Architecture: HIGH - CodeGraph and direct source inspection agree on conversion, metadata dispatch, outcome projection, and service/log ownership. [VERIFIED: `.codegraph/`; source files]
- Pitfalls: HIGH - each pitfall follows a locked decision or a current implementation asymmetry. [VERIFIED: `02-CONTEXT.md`; source/tests]
- Validation architecture: HIGH - existing Cargo targets and test ownership are present; only the exact new test names remain discretionary. [VERIFIED: Cargo manifests; test files]

**Research date:** 2026-07-11  
**Valid until:** The operation enum, stable facade exports, or session durability implementation materially changes; otherwise through Phase 2 planning and execution. [VERIFIED: phase scope]
