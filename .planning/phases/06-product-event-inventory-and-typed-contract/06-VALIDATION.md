---
phase: 06
slug: product-event-inventory-and-typed-contract
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-13
---

# Phase 06 - Validation Strategy

> Per-phase validation contract for typed public event projection and inventory drift.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness plus deterministic offline integration fixtures |
| **Config file** | Workspace `Cargo.toml` and `crates/pi-coding-agent/Cargo.toml` |
| **Quick run command** | `cargo test -p pi-coding-agent --lib public_event --quiet` |
| **Public contract command** | `cargo test -p pi-coding-agent --test product_event_contract --quiet` |
| **Facade command** | `cargo test -p pi-coding-agent --test public_api --quiet` |
| **Boundary command** | `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` |
| **Full phase command** | `cargo test -p pi-coding-agent --quiet && cargo check -p pi-coding-agent --all-targets` |
| **Full workspace command** | `cargo test --workspace && cargo check --workspace` |

---

## Sampling Rate

- **After every task:** Run that task's focused automated command and `git diff --check`.
- **After Wave 1:** Run the public-event library filter, `public_api`, and all-target crate check before Plan 06-02 starts.
- **After Wave 2:** Run `public_event`, `product_event_contract`, `public_api`, and `event_boundary_guards`, then the full `pi-coding-agent` crate suite/check.
- **Before phase verification:** Run formatting, the full workspace test/check gates, source audits, and `git diff --check` from the final tree.
- **Max feedback latency:** Use the library filter or one integration target after each task; reserve crate/workspace suites for wave and phase boundaries.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Behavior Proved | Test Type | Automated Command | Test Target Exists Before Execution | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------------------------------|--------|
| 06-01-01 | 01 | 1 | EVENT-01, EVENT-02 | T-06-01, T-06-02, T-06-03 | Typed family/payload matching, metadata absence, terminal/durability independence, deterministic Serde | co-located unit/TDD | `cargo test -p pi-coding-agent --lib public_event --quiet` | no - task creates `public_event.rs` tests before implementation | pending |
| 06-01-02 | 01 | 1 | EVENT-01, EVENT-02 | T-06-02, T-06-04 | Public receiver projects one typed event per retained/broadcast ProductEvent and all targets compile | unit/build | `cargo check -p pi-coding-agent --all-targets && cargo test -p pi-coding-agent --lib product_event --quiet` | yes - existing product-event unit tests | pending |
| 06-01-03 | 01 | 1 | EVENT-01, EVENT-02 | T-06-02 | Stable facade exports typed contracts without exposing implementation modules | integration | `cargo test -p pi-coding-agent --test public_api --quiet` | yes | pending |
| 06-02-01 | 02 | 2 | EVENT-01, EVENT-02, EVENT-03 | T-06-05, T-06-07, T-06-08 | Exhaustive 45-variant conversion and exact 11-family distribution | co-located unit/TDD | `cargo test -p pi-coding-agent --lib public_event --quiet` | yes - created by 06-01-01 | pending |
| 06-02-02 | 02 | 2 | EVENT-01, EVENT-02 | T-06-06, T-06-07 | Downstream typed matching, Serde shape, monotonic sequence, and one-to-one projection | integration/TDD | `cargo test -p pi-coding-agent --test product_event_contract --quiet` | no - task creates target before running it | pending |
| 06-02-03 | 02 | 2 | EVENT-03 | T-06-05, T-06-08 | Event and operation/outcome inventories, no-Debug identity, no public legacy payload, no wildcard conversion | integration/source guard | `cargo test -p pi-coding-agent --test event_boundary_guards --quiet && cargo test -p pi-coding-agent --test product_event_contract --quiet` | first target yes; second created by 06-02-02 | pending |

---

## Wave 0 Requirements

No new test framework, dependency, external service, fixture package, or environment setup is required. Two test locations are absent before execution and are explicitly bootstrapped in dependency order:

| Missing Target | Created By | Must Exist Before | Bootstrap Rule |
|----------------|------------|-------------------|----------------|
| `crates/pi-coding-agent/src/coding_session/public_event.rs` co-located tests | 06-01-01 | 06-01-02 and all Plan 06-02 tasks | Task is TDD: write failing typed-contract tests before production conversion in the same file. |
| `crates/pi-coding-agent/tests/product_event_contract.rs` | 06-02-02 | 06-02-03 and final phase gate | Create the integration target and make it pass before the documentation/guard task consumes it. |

`wave_0_complete` remains `false` until these targets have been created and their focused commands pass. The plan dependency graph prevents a later task from requiring either target before its bootstrap task completes.

---

## Feedback Latency

| Check | Expected Use | Latency Class |
|-------|--------------|---------------|
| `cargo test -p pi-coding-agent --lib public_event --quiet` | Inner loop for type/conversion/inventory changes | task-level, target under 60 seconds |
| `cargo test -p pi-coding-agent --test public_api --quiet` | Stable facade changes | task-level, target under 60 seconds |
| `cargo test -p pi-coding-agent --test product_event_contract --quiet` | Public receiver and serialization changes | task-level after target bootstrap |
| `cargo test -p pi-coding-agent --test event_boundary_guards --quiet` | Source contract and inventory drift | task-level, target under 60 seconds |
| Full crate/workspace commands | Cross-module regression | wave/final only |

No watch-mode or interactive command is allowed. If a focused target exceeds 60 seconds in the execution environment, record its measured duration and use the narrowest test-name filter during the task loop while retaining the full target at the wave gate.

---

## Sampling Continuity

- Every implementation task has an automated focused command.
- No sequence of three implementation tasks lacks a direct automated sample.
- Plan 06-02 cannot begin until Plan 06-01's typed contract and facade checks pass.
- Task 06-02-03 reruns both the existing boundary target and the newly bootstrapped public contract target.
- Final verification samples the focused contract, stable facade, source guards, full crate, and full workspace; no requirement relies on manual inspection alone.

---

## Manual-Only Verifications

None. Inventory completeness, public surface, serialization, terminal association, ordering, privacy, source boundaries, and documentation drift all require deterministic automated evidence.

---

## Validation Sign-Off

- [x] Every plan task has an exact automated command.
- [x] Every missing test target has a named bootstrap task before its first dependent use.
- [x] No three consecutive implementation tasks lack automated verification.
- [x] Task-level checks are focused; crate/workspace checks occur at wave/final boundaries.
- [x] No watch-mode flags, network services, external packages, or manual-only gates are used.
- [ ] Wave 0 targets have been created and pass.
- [ ] Focused, crate, and workspace results are recorded in Phase 6 verification evidence.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** pending execution
