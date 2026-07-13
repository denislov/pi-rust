# Milestones

## v1.0 Canonical Operation Runtime Convergence (Shipped: 2026-07-13)

**Phases completed:** 5 phases, 22 plans, 45 tasks

**Closeout:** Verified with non-blocking technical debt; 37/37 requirements, 12/12 integration surfaces, and 8/8 end-to-end flows passed.

**Key accomplishments:**

- Structural audit scaffold with 15-row Operation Matrix seeded from live source, three-mode Bash validator enforcing locked taxonomies, and concrete Nyquist task map across 3 plans and 7 tasks
- Populated 15-row Operation Matrix from live source plus 46 evidence IDs, 26 production callers, 32 test callers, 16 compatibility methods, 4 authority conflicts, and 8 findings with validator bug fixes
- Final audit closure with 9 findings spanning completed baseline through Stage 10 deferral, AUDIT-01/02/03 traceability marked complete, and Nyquist validation signed off with validator blocking-finding bug fix
- Complete facade-only signature evidence plus exact privacy enforcement that removes registry implementation types without disturbing crate-root compatibility.
- Independent 15-row operation ownership, exhaustive outcome projection fixtures, and behavior-backed proof of all three metadata-selected dispatch families.
- Canonical high-risk operations now have outcome, state, event, replay, and PartialCommit evidence, backed by a closed facade ledger and fully approved Phase 2 validation.
- JSON and print prompt paths now route through CodingAgentSession::run(CodingAgentOperation::Prompt) with exhaustive outcome extraction, locked by a narrow production source guard.
- All four select-driven RPC background operations (prompt, agent, team, delegation approval) now route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction and unchanged concurrency, control, queue, replay, idempotency, and session semantics.
- All five short-lived RPC mutation commands (self-healing edit, default-profile mutation, delegation rejection, plugin load, plugin command) now route through CodingAgentSession::run(CodingAgentOperation) with exhaustive outcome extraction, and a narrow source guard locks canonical operations across src/protocol/rpc/.
- Every ordinary interactive background workflow (prompt, agent, team, approval, compact, self-heal, plugin reload/command, direct branch summary) now routes through CodingAgentSession::run(CodingAgentOperation) with unchanged TUI controls, events, timing, projections, and owner state.
- Interactive default-profile mutation and delegation rejection now execute asynchronously through CodingAgentSession::run(CodingAgentOperation) with preserved menus, dialogs, queues, errors, events, persistence, projections, and owner state.
- Direct /fork and summary-before-fork tree navigation now execute through CodingAgentSession::run(CodingAgentOperation) with one receiver spanning both operations, and complete narrow production source guards close the Phase 3 adapter convergence gate.
- Owner-bearing interactive tasks now return the live CodingAgentSession on failure, fork completions synchronize the next request target, and delegation fallback follows visible UiEvent projection.
- Structured partial-commit identity now survives the CLI adapter boundary, and interactive unit tests have a narrowly guarded owner-local bridge to real persistence faults and pending delegation state.
- Four deterministic real-runner tests now close both strict UAT gaps through actual PromptTask channels, durable persistence failures, finish_prompt restoration, and post-failure owner use.
- Agent, team, and export behavior now enters the canonical typed dispatcher, with receiver-aware proof and deletion of four obsolete live-session wrappers.
- Prompt, profile, self-healing, compaction, and delegation setup coverage now enters the canonical typed dispatcher, with seven G2 wrappers deleted and custom plugin options confined to four owner tests.
- Delegation decisions now use admitted typed operations with durable evidence preserved, and both public approval/rejection compatibility methods are deleted without shims.
- Canonical navigation and branch-summary operations now carry the final behavior tests, while the complete 16-method compatibility surface is deleted and Phase 4 closure gates pass.
- A locked offline Cargo consumer now proves both the complete canonical facade and the compiler-enforced privacy of every internal runtime category.

**Deferred hardening:**

- Make top-level first-party adapter ownership independently discoverable from the guard that enforces its closed-world inventory.
- Bind negative external compile fixtures to the expected forbidden symbol or span, beyond checking `E0432`/`E0603` categories.
- Revisit Phase 3's `wave_0_complete: false` metadata if a future Nyquist contract requires it.

**Audit:** `.planning/milestones/v1.0-MILESTONE-AUDIT.md` (`TECH_DEBT`, no blockers)

---
