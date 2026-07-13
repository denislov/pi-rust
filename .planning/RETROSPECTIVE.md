# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — Canonical Operation Runtime Convergence

**Shipped:** 2026-07-13

**Phases:** 5 | **Plans:** 22 | **Tasks:** 45

### What Was Built

- One complete stable `pi_coding_agent::api` operation facade with exhaustive internal conversion and outcome projection.
- Canonical JSON, print, RPC, and interactive adapters using `CodingAgentSession::run` while preserving behavior, controls, events, replay, navigation and `PartialCommit` identity.
- Deletion of the 16 replaced broad workflow methods after production and test migration.
- Recursive receiver-aware source guards and external dependent-crate compile-pass/fail API proofs.
- Reproducible Stage 9 closure, security verification, and cross-phase milestone audit.

### What Worked

- Evidence-first planning corrected stale historical assumptions before implementation.
- Dependency-ordered phases kept facade, adapter, test, deletion and guard changes independently verifiable.
- Typed outcomes and real failure fixtures preserved durability semantics instead of reducing migration tests to compile-only coverage.
- Independent plan checker, code review, security and integration gates found issues before archive.

### What Was Inefficient

- Shared-main-worktree generic executors could not commit through the sandbox and one closure executor stalled, requiring parent recovery.
- Full workspace tests required elevated local-socket permissions; the restricted run initially produced environment-only failures.
- The first boundary scanner implementation was too line-oriented and required a post-review multiline parser fix.

### Patterns Established

- Prefer compiler-visible privacy tests for Rust API boundaries and receiver-aware source guards only where the type system cannot express repository ownership.
- Keep positive public API inventories independent from production metadata and exports.
- Record final-tree evidence with exact commands, timestamps, statuses, counts and explicit report self-reference.
- Treat durable partial commit identity, replay authority and owner restoration as mandatory migration invariants.

### Key Lessons

1. Architectural deletion is safe only after both production and behavior tests migrate and a receiver-aware absence ledger passes.
2. Source scanners need formatting-variant fixtures, including multiline impls/signatures, before they can serve as closure authority.
3. Archive links must be rewritten when phase directories move; historical artifacts can retain original execution paths as evidence.

### Cost Observations

- Agent dispatch used the generic-agent workaround because typed Codex agent dispatch was unavailable.
- Long-running workspace suites and closure documentation dominated execution time.
- No new runtime dependencies were needed for the milestone.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|---|---:|---:|---|
| v1.0 | 5 | 22 | Evidence-first convergence with layered verification gates |

### Cumulative Quality

| Milestone | Requirements | Integration | Open Security Threats |
|---|---:|---:|---:|
| v1.0 | 37/37 | 12/12 wired, 8/8 flows | 0 |

### Top Lessons

1. Preserve semantic evidence while changing architectural entry points.
2. Use compiler boundaries and source guards as complementary controls.
