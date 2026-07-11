---
phase: 02
slug: canonical-facade-correctness
status: verified
threats_open: 0
asvs_level: 1
block_on: high
register_authored_at_plan_time: true
created: 2026-07-11
verified: 2026-07-11
---

# Phase 02 - Security

> Per-phase security contract for the canonical operation facade, dispatcher,
> public outcome projection, durable mutations, and test-only fault controls.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Downstream caller -> `pi_coding_agent::api` | Independently versioned callers receive only curated product contracts. | Typed operations, outcomes, lifecycle and control contracts |
| Stable facade -> private coding-session runtime | Public re-exports must not expose admission metadata, capability-bearing services, registries, plugin options, or Flow internals. | Public facade types and private runtime ownership |
| Public operation -> private operation | Caller input is converted into private admission and execution contracts. | `CodingAgentOperation` and internal `Operation` |
| Private metadata -> dispatcher | Dispatch classification selects async, sync-read-only, or sync-mutable execution. | Operation metadata and mutation authority |
| Private outcome -> public projection | Internal results and diagnostics are reduced to stable public outcome families. | Operation results, errors, profile and delegation data |
| Canonical operation -> durable session mutation | Operations may append facts, update the manifest, publish events, and replace owner state. | Session events, operation IDs, manifest state, product events |
| Durable log -> reopened owner state | Replay remains authoritative after append succeeds but later commit stages fail. | Typed session facts and replay-derived state |
| Test fault controls -> production runtime | Deterministic failure injection must remain test-only and isolated. | Append/manifest failure controls and shared test state |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation and Evidence | Status |
|-----------|----------|-----------|----------|-------------|-------------------------|--------|
| T-02-01 | Information Disclosure / Elevation of Privilege | `src/lib.rs::api` | high | mitigate | Explicit curated re-exports plus `stable_api_signature_closure_is_importable`; all 15 operations and support types compile through `pi_coding_agent::api` without widening the facade. | closed |
| T-02-02 | Tampering | `tests/api_boundary_guards.rs` | high | mitigate | Rust visibility remains the primary boundary; `stable_api_excludes_internal_runtime_contracts` rejects the current internal operation, metadata, plugin-option, service, registry, and Flow identifiers. | closed |
| T-02-03 | Tampering | `Operation::metadata` / `CodingAgentSession::run` | high | mitigate | Independent 15-row expectations plus `canonical_run_uses_each_metadata_dispatch_family` verify Async, SyncReadOnly, and SyncMutable selection through the public dispatcher. | closed |
| T-02-04 | Information Disclosure / Tampering / Repudiation | `CodingAgentOperationOutcome::from_internal` | high | mitigate | Wildcard-free exhaustive projection and direct fixtures cover every internal outcome family, including separate `ExportCurrent` and `ExportCurrentHtml` branches. | closed |
| T-02-05 | Tampering / Repudiation | Navigation, profile, and delegation persistence | high | mitigate | Named canonical-run tests verify immediate and reopened state, append/manifest ordering, no-mutation failures, explicit `PartialCommit`, matching durable operation IDs, and replay authority. | closed |
| T-02-06 | Denial of Service / Tampering | Failure injection and shared fixture state | high | mitigate | Existing serialized guards are reused; `session_store_failure_controls_remain_test_only` verifies the known definitions and seven call sites remain directly test-gated, with no production fault hook added. | closed |
| T-02-07 | Information Disclosure / Elevation of Privilege | Plugin capability and service projection | high | mitigate | Plugin commands execute through capability-aware `PluginService`; canonical behavior tests verify curated output/diagnostics and operation guard release without exposing service internals. | closed |
| T-02-08 | Repudiation | `ProductEvent` sequence continuity | medium | mitigate | Navigation and high-risk operation tests assert semantic events and strictly increasing sequence values across owner replacement and applicable mutations. | closed |

*Status: open · closed · open below the `high` threshold (non-blocking).*

## Verification Evidence

| Control | Evidence |
|---------|----------|
| Stable facade closure | `cargo test -p pi-coding-agent --test public_api stable_api_signature_closure_is_importable -- --exact` |
| Stable/private boundary | `cargo test -p pi-coding-agent --test api_boundary_guards stable_api_excludes_internal_runtime_contracts -- --exact` |
| Fifteen-row mapping contract | `cargo test -p pi-coding-agent --lib coding_session::public_operation::tests::operation_contract_covers_all_public_variants -- --exact` |
| Dispatcher selection | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_uses_each_metadata_dispatch_family -- --exact` |
| Exhaustive public projection | `cargo test -p pi-coding-agent coding_session::public_operation::tests::operation_outcome_projection_covers_all_families -- --exact` |
| Navigation durability and event continuity | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_navigation_and_branch_summary_durability -- --exact` |
| Navigation failure and replay authority | `cargo test -p pi-coding-agent coding_session::tests::canonical_durable_mutations_distinguish_no_commit_partial_commit_and_replay -- --exact` |
| Plugin/profile/delegation behavior | `cargo test -p pi-coding-agent coding_session::tests::canonical_run_preserves_plugin_profile_and_delegation_contracts -- --exact` |
| Delegation failure and replay authority | `cargo test -p pi-coding-agent coding_session::tests::canonical_delegation_decisions_distinguish_no_commit_partial_commit_and_replay -- --exact` |
| Test-only failure controls | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards session_store_failure_controls_remain_test_only -- --exact` |
| Closed operation facade | `cargo test -p pi-coding-agent --test product_runtime_boundary_guards canonical_operation_facade_has_no_new_workflow_wrappers -- --exact` |
| Phase closure | `cargo fmt --check`, `cargo test --workspace`, `cargo check --workspace`, source audits, and `git diff --check` passed during Phase 02 verification. |

## Residual Hardening

The Phase 02 code review identified four future-bypass weaknesses in source guards:
glob re-export detection, generic fault scanning inside `session_log/store.rs`, lifetime
handling in the handwritten Rust sanitizer, and complete trait-body facade parsing. The
current tree contains no corresponding facade leak, production fault control, or alternate
trait facade. These regression-resistance improvements are assigned to Phase 5 and do not
leave a Phase 02 threat open.

---

## Accepted Risks Log

No accepted risks. Every plan-time threat has disposition `mitigate` and verified implementation evidence.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-07-11 | 8 | 8 | 0 | Codex (`gsd-secure-phase`, ASVS L1) |

The register was authored at plan time. Preliminary classification found zero open
threats, so the ASVS L1 short-circuit applied and no deeper security-auditor pass was
required by the workflow.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-07-11
