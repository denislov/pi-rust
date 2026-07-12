---
phase: 03
slug: production-adapter-convergence
status: verified
threats_open: 0
asvs_level: 1
block_on: high
register_authored_at_plan_time: true
created: 2026-07-13
verified: 2026-07-13
---

# Phase 03 - Security

ASVS L1 audit of the plan-time threat registers in Plans 03-01 through 03-09.
Implementation and executable guard locations, rather than plan or summary claims,
are the evidence for mitigated threats.

## Threat Register

| Threat ID | Category | Severity | Disposition | Status | Implementation evidence |
|-----------|----------|----------|-------------|--------|-------------------------|
| T-03-01 | Tampering / Elevation of Privilege | high | mitigate | closed | JSON and both print paths call typed `run(Prompt)` and extract only `Prompt`: `src/protocol/json_mode.rs:100-103`, `src/print_mode.rs:129-132,150-153`; source guard: `tests/product_runtime_boundary_guards.rs:323-400`. |
| T-03-02 | Repudiation | medium | mitigate | closed | JSON subscribes before execution and drains events: `src/protocol/json_mode.rs:94,100,201`; parity is enforced by the JSON/print production guard at `tests/product_runtime_boundary_guards.rs:323-400`. |
| T-03-03 | Information Disclosure | medium | mitigate | closed | Exact public Prompt outcome is retained at `src/protocol/json_mode.rs:100-103` and `src/print_mode.rs:129-153`; no replacement projection path is introduced. |
| T-03-04 | Denial of Service | low | accept | documented accepted risk | Closed-enum mismatch remains an internal invariant in the exact outcome matches at `src/protocol/json_mode.rs:101-105` and `src/print_mode.rs:130-155`; rationale and scope are recorded in Accepted Risks Log. |
| T-03-05 | Tampering / Elevation of Privilege | high | mitigate | closed | RPC constructs typed public operations after handler validation: `src/protocol/rpc/prompt.rs:377,601,783,917`; mutation variants at `src/protocol/rpc/commands.rs:615,858,1047,1159`. |
| T-03-06 | Repudiation / Denial of Service | high | mitigate | closed | Bounded queue is instantiated for every background operation at `src/protocol/rpc/prompt.rs:349,577,761,904`; final drains at `1029,1127`; queue implementation at `src/protocol/rpc/event_queue.rs:13-62`. |
| T-03-07 | Tampering / Denial of Service | high | mitigate | closed | `PromptControlHandle` remains adapter-owned in `src/protocol/rpc/state.rs:12,64`; canonical Prompt remains pinned inside the existing select topology at `src/protocol/rpc/prompt.rs:917`. |
| T-03-08 | Information Disclosure | medium | mitigate | closed | RPC operation errors retain `CliError::from` conversion at `src/protocol/rpc/prompt.rs:399,623,808,939`; exact public outcomes are matched before RPC-local projection. |
| T-03-09 | Tampering / Elevation of Privilege | high | mitigate | closed | Validated mutations use only `SelfHealingEdit`, `SetDefaultAgentProfile`, and `RejectDelegation` public operations at `src/protocol/rpc/commands.rs:615,858,1047`. |
| T-03-10 | Elevation of Privilege / Information Disclosure | high | mitigate | closed | Plugin work uses public `PluginLoad`/`PluginCommand` at `src/protocol/rpc/commands.rs:1141,1159,1223`; private-import and canonical-call guard at `tests/product_runtime_boundary_guards.rs:402-466`. |
| T-03-11 | Repudiation | medium | mitigate | closed | Mutation handlers subscribe before execution (`src/protocol/rpc/commands.rs:588,850,1039`) and use shared final drain at `1543`; canonical RPC guard covers all handlers at `tests/product_runtime_boundary_guards.rs:402-466`. |
| T-03-12 | Denial of Service | medium | mitigate | closed | Each mutation exhaustively extracts its expected outcome at `src/protocol/rpc/commands.rs:620,864,1055,1166`; no retry/detached runner is introduced. |
| T-03-13 | Denial of Service / Tampering | high | mitigate | closed | `PromptTaskCompletion::Failed` carries the live owner at `src/interactive/prompt_task.rs:40-49`; shared exactly-one-owner completion is at `778-786`; restoration is at `src/interactive/loop.rs:2315-2319`. |
| T-03-14 | Repudiation / Denial of Service | high | mitigate | closed | Every owner runner subscribes before `run` and performs a final `try_recv` drain, e.g. `src/interactive/prompt_task.rs:821-873,910-964`; per-runner structural guard at `2009-2043`. |
| T-03-15 | Elevation of Privilege | high | mitigate | closed | Interactive plugin actions call public `PluginLoad`/`PluginCommand` at `src/interactive/prompt_task.rs:1375,1454,1484`; private contract imports are rejected by `tests/product_runtime_boundary_guards.rs:468-590`. |
| T-03-16 | Tampering | medium | mitigate | closed | Direct summary explicitly uses `AlwaysCreate` at `src/interactive/prompt_task.rs:1620-1625`; the production interactive guard covers the path at `tests/product_runtime_boundary_guards.rs:468-590`. |
| T-03-17 | Tampering / Elevation of Privilege | high | mitigate | closed | Profile and rejection inputs become only typed public operations at `src/interactive/prompt_task.rs:1104,1158`; exact outcomes are extracted in the same runners. |
| T-03-18 | Denial of Service | high | mitigate | closed | Both mutations use owner-returning `complete_owned_task` at `src/interactive/prompt_task.rs:1138,1202`; failure restores owner before projection at `src/interactive/loop.rs:2315-2319`. |
| T-03-19 | Repudiation | medium | mitigate | closed | Rejection subscribes first, classifies visibility through `CodingEventBridge`, forwards once, and drains at `src/interactive/prompt_task.rs:1153-1202`. |
| T-03-20 | Tampering | medium | mitigate | closed | Runtime profile mutation completes through `run` at `src/interactive/prompt_task.rs:1104`; root/session projection occurs only in completed handling at `src/interactive/loop.rs:2219-2231`. |
| T-03-21 | Tampering / Repudiation | high | mitigate | closed | Navigation runs `BranchSummary(ReuseExisting)` before `ForkSession` on the same mutable owner at `src/interactive/prompt_task.rs:1695-1736`. |
| T-03-22 | Denial of Service / Repudiation | high | mitigate | closed | One receiver is created before summary and retained through fork, with drains after both operations: `src/interactive/prompt_task.rs:1695,1731,1763`. |
| T-03-23 | Tampering | high | mitigate | closed | Navigation returns the same mutated owner through `complete_owned_task` at `src/interactive/prompt_task.rs:1771`; hydration and owner restoration occur at `src/interactive/loop.rs:2151-2175,2295-2313`. |
| T-03-24 | Elevation of Privilege | high | mitigate | closed | JSON/print, RPC, and interactive canonical/private-import/deprecation guards exist at `tests/product_runtime_boundary_guards.rs:323,402,468`; `crate::api` import enforcement is at `573-590`. |
| T-03-25 | Denial of Service | medium | mitigate | closed | Guards sanitize Rust comments/strings through `sanitize_rust_source` at `tests/product_runtime_boundary_guards.rs:1109`; interactive guard explicitly distinguishes allowed local projection/lifecycle calls at `468-590`. |
| T-03-26 | Tampering / Denial of Service | high | mitigate | closed | All operation failures carry one owner through `PromptTaskFailure` and `complete_owned_task`: `src/interactive/prompt_task.rs:40-49,778-786`; restore precedes error projection at `src/interactive/loop.rs:2315-2319`. |
| T-03-27 | Repudiation | high | mitigate | closed | Structured `PartialCommit` preserves operation ID/message at `src/error.rs:23-27` and conversion at `src/coding_session/error.rs:100-105`; real durable-ID tests are at `src/interactive/loop.rs:2775,2839`. |
| T-03-28 | Tampering | high | mitigate | closed | Successful fork updates target only in completed branches at `src/interactive/loop.rs:2151-2154,2295-2297`; failure branch restores owner without changing target at `2315-2319`. |
| T-03-29 | Repudiation / Information Disclosure | medium | mitigate | closed | Fallback uses established projected-visible-UI classification at `src/interactive/prompt_task.rs:789-790,1156-1202`; original ProductEvents are forwarded once. |
| T-03-30 | Tampering | medium | mitigate | closed | Named runner ledger requires each runner body to subscribe and call `complete_owned_task`, with per-function diagnostics at `src/interactive/prompt_task.rs:2009-2043`; adapter guards remain at `tests/product_runtime_boundary_guards.rs:323-590`. |
| T-03-31 | Repudiation | high | mitigate | closed | `CliError::PartialCommit` is structured at `src/error.rs:23-27`; exact conversion and contract test at `src/coding_session/error.rs:100-105,132-150`. |
| T-03-32 | Tampering / Elevation of Privilege | high | mitigate | closed | Specialized bridge methods are directly `cfg(test)` at `src/coding_session/mod.rs:2189-2212`; test-only/store-control and closed-ledger guards at `tests/product_runtime_boundary_guards.rs:25-112,114-319`. |
| T-03-33 | Information Disclosure | low | accept | documented accepted risk | Stable-facade exclusion is enforced by `tests/product_runtime_boundary_guards.rs:25-112,114-319`; rationale and scope are recorded in Accepted Risks Log. |
| T-03-34 | Tampering / Denial of Service | high | mitigate | closed | Real production spawn/done tests for profile, rejection, prompt, and fork are at `src/interactive/loop.rs:2735,2775,2839,2935`; task polling uses the actual done channel at `456`. |
| T-03-35 | Repudiation | high | mitigate | closed | Real rejection and prompt PartialCommit tests compare typed errors to durable transaction identities at `src/interactive/loop.rs:2775,2839`; structured conversion remains at `src/coding_session/error.rs:100-105`. |
| T-03-36 | Tampering | high | mitigate | closed | Real fork failure test verifies source owner/target and rejects replacement `SessionOpened` at `src/interactive/loop.rs:2935-3003`. |
| T-03-37 | Denial of Service | medium | mitigate | closed | The same pre-task receiver is retained and observes `DefaultAgentProfileChanged` after restoration at `src/interactive/loop.rs:2962,3039-3046`. |

Status vocabulary: `closed`, `documented accepted risk`, `open`, or
`open below the high threshold (non-blocking)`.

## Accepted Risks Log

| Threat ID | Severity | Accepted risk and rationale | Compensating evidence |
|-----------|----------|-----------------------------|-----------------------|
| T-03-04 | low | `CodingAgentOperationOutcome` is a closed enum and an adapter/outcome mismatch denotes an internal programming invariant. The phase accepts process/task failure at this unreachable branch instead of adding retry or protocol behavior that would alter established output contracts. | Every adapter performs exact variant extraction; canonical-operation source guards reject workflow bypasses. |
| T-03-33 | low | Test fixture helpers are privileged crate-test conveniences. The residual risk of accidental disclosure is accepted because they are directly `cfg(test)`, `pub(crate)`, specialized rather than generic, omitted from `pi_coding_agent::api`, and guarded by executable source ledgers. | `src/coding_session/mod.rs:2189-2212`; `tests/product_runtime_boundary_guards.rs:25-319`. |

No threat has disposition `transfer`.

## Threat Flags

All nine phase summaries report `## Threat Flags: None`. No unregistered flag
requires mapping.

## Gate Computation

- Total threats: 37
- Mitigated and closed: 35
- Documented accepted risks: 2
- Open blocking (high or critical): 0
- Open non-blocking (below high): 0
- Frontmatter `threats_open`: 0

## Verification Basis

- ASVS level: L1, presence in the plan-cited implementation boundary.
- Configuration: `block_on: high`; accepted risks are documented rather than open.
- Planning verification: `03-VERIFICATION.md` reports 5/5 observable truths and
  all 13 production boundary guards passing. This report was used to identify
  expected artifacts, not as a substitute for the implementation evidence above.
- No implementation file or pre-existing planning artifact was modified by this audit.

## Security Audit Trail

| Audit Date | Threats Total | Closed / accepted | Open blocking | Run By |
|------------|---------------|-------------------|---------------|--------|
| 2026-07-13 | 37 | 37 | 0 | Codex (`gsd-secure-phase`, ASVS L1) |

## Sign-Off

- [x] Every plan-time threat is represented exactly once.
- [x] Every mitigate disposition has implementation or executable-guard evidence.
- [x] Every accept disposition is recorded in Accepted Risks Log.
- [x] Summary Threat Flags were incorporated.
- [x] `threats_open: 0` matches the `high` blocking threshold.

**Approval:** verified 2026-07-13
