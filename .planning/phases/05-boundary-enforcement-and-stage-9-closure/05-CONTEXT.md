# Phase 5: Boundary Enforcement and Stage 9 Closure - Context

**Gathered:** 2026-07-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 5 makes the canonical live-session operation boundary resistant to
regression, proves the stable facade's positive and negative API contract from
an external consumer's perspective, verifies the final workspace, and closes
Stage 9 with one authoritative evidence record.

The phase hardens guards for JSON, print, RPC, and interactive production
adapters; preserves the Phase 4 deletion boundary; and records the exact final
verification state. It does not restore, rename, or recreate deleted broad
workflow methods. It also does not implement or plan Stage 10 typed
`ProductEvent` payload convergence or compatibility-subscription deletion.

</domain>

<decisions>
## Implementation Decisions

### Recursive Adapter Scan Boundary
- **D-01:** Maintain one centralized inventory of the JSON, print, RPC, and interactive first-party adapters and their production ownership roots.
- **D-02:** Recursively discover every Rust source file below directory roots. Explicitly register single-file adapter entry points. New files below a registered root must enter the guard automatically.
- **D-03:** Guard production code only. Skip code inside `#[cfg(test)]` items or modules; test callers remain governed by the Phase 4 receiver-aware deleted-method absence ledger.
- **D-04:** Every known first-party adapter must belong to exactly one valid ownership root. New top-level adapters require an explicit inventory update.
- **D-05:** Discovery is fail-closed. Missing roots, empty scans, unreadable files or directories, duplicate ownership, and known adapters without ownership all fail with path-specific diagnostics.

### Same-Name Calls And Exception Policy
- **D-06:** Detect prohibited calls structurally with receiver awareness. Do not use method-name substring matching as the authority for distinguishing canonical-boundary violations from legitimate same-name calls.
- **D-07:** Keep all legitimate same-name calls in one narrow exception table keyed by ownership scope or path, receiver shape, method name, and a concrete reason. Unknown receivers fail closed.
- **D-08:** Each exception declares an exact or maximum occurrence count. Moving, copying, or increasing an allowed call requires explicit review and an inventory update.
- **D-09:** Recognition of ordinary Rust method calls must remain correct across line breaks, method chains, parenthesized receivers, inserted comments, and rustfmt changes. Strings, comments, doc comments, and test-only code must not create false positives.
- **D-10:** Lock scanner behavior with positive and negative fixtures covering real calls, legitimate same-name receivers, formatting variants, and ignored non-code text.
- **D-11:** Production source may not use inline comments, custom attributes, or file-level directives to suppress the boundary guard. The centralized exception table is the only permitted exception mechanism.

### Stable API Positive And Negative Proof
- **D-12:** Add real external-consumer compile-pass and compile-fail fixtures. Source inspection alone is insufficient as the final privacy proof.
- **D-13:** Organize negative fixtures by boundary category: internal operation and dispatch metadata, runtime services, plugin load options and registries, and Flow contracts.
- **D-14:** Each negative category must attempt all externally writable access paths, including `pi_coding_agent::api`, crate-root paths, and migration-private `#[doc(hidden)]` public modules. Internal contracts must be inaccessible to an external crate, not merely absent from stable-facade documentation.
- **D-15:** Assert the nature of compilation failure without binding tests to complete rustc diagnostic text.
- **D-16:** Keep an independent, explicit positive facade contract inventory that imports and uses every operation variant, outcome family, and required support type. Expected API closure must not be generated from production exports.

### Stage 9 Closure Evidence And Documentation
- **D-17:** Create one formal Stage 9 closure report as the authoritative record of the final operation boundary, deleted compatibility surface, guard coverage, source audit, verification results, remaining work, and Stage 10 handoff. Other current documents link to this report.
- **D-18:** Record reproducible structured evidence for every required verification command: exact command, execution time, final status, key counts or conclusion, source-audit scope and zero-violation result, and the verified Git commit or worktree state. Do not embed complete Cargo logs.
- **D-19:** Preserve the body of `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` as historical evidence. Add a clear superseded status and closure-report link rather than rewriting its checklist or moving it to the archive.
- **D-20:** Update current architecture, design, project, requirements, roadmap, and state authority documents to match the verified Stage 9 implementation and point to the closure report where appropriate.
- **D-21:** Include a bounded Stage 10 handoff inventory: remaining compatibility event subscriptions, untyped `ProductEvent` payload families, why they were deferred, source locations that must be re-verified, and behavior constraints that Stage 10 must preserve. Do not create a Stage 10 implementation plan in this phase.

### The Agent's Discretion
- Choose the scanner or parser implementation, provided it satisfies the structural, formatting-independent, fixture-backed, and fail-closed decisions above.
- Choose the exact Rust compile-test harness and fixture layout, provided it is deterministic, offline, external-consumer accurate, and does not weaken category or access-path coverage.
- Choose the adapter inventory data structure, exception-table representation, diagnostic wording, and internal helper organization.
- Choose the closure report filename and section layout, provided there is one clearly identified authoritative Stage 9 report containing all evidence required by D-17 through D-21.
- Choose focused verification command ordering and batching. All commands required by the roadmap and project constraints remain mandatory.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone And Phase Authority
- `.planning/PROJECT.md` - Defines the canonical runtime goal, architecture and compatibility constraints, deletion order, verification requirements, and Stage 10 exclusions.
- `.planning/REQUIREMENTS.md` - Defines GUARD-01 through GUARD-04 and CLOSE-01 through CLOSE-04 as the complete Phase 5 requirement set.
- `.planning/ROADMAP.md` - Fixes the Phase 5 goal and five observable success criteria.
- `.planning/STATE.md` - Carries the completed Phase 1-4 decisions and current Phase 5 position.
- `.planning/phases/05-boundary-enforcement-and-stage-9-closure/05-CONTEXT.md` - Locks the implementation and closure decisions from this discussion.

### Prior Phase Contracts And Evidence
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md` - Source-backed operation, caller, compatibility-method, guard-gap, and Stage 10 inventories; current source remains the completion authority.
- `.planning/phases/01-evidence-based-baseline/01-CONTEXT.md` - Defines the evidence hierarchy and the rule that historical checklist state is not completion proof.
- `.planning/phases/02-canonical-facade-correctness/02-CONTEXT.md` - Locks facade closure, private runtime boundaries, independent contract expectations, and compiler-visible enforcement preference.
- `.planning/phases/02-canonical-facade-correctness/02-VERIFICATION.md` - Verifies the canonical facade and high-risk durable behavior Phase 5 guards must not disturb.
- `.planning/phases/03-production-adapter-convergence/03-CONTEXT.md` - Defines the adapter ownership boundaries, behavior-preservation rules, and prohibition on alternate compatibility facades.
- `.planning/phases/03-production-adapter-convergence/03-VERIFICATION.md` - Records the converged production adapter baseline.
- `.planning/phases/04-test-convergence-and-compatibility-deletion/04-CONTEXT.md` - Locks public-first test migration, deletion order, retained API boundary, helper restrictions, and the Phase 5 hardening handoff.
- `.planning/phases/04-test-convergence-and-compatibility-deletion/04-VERIFICATION.md` - Records the final deleted-method and migrated-test baseline that Phase 5 must enforce.

### Stage 9 Design And Architecture
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` - Historical migration targets and verification ideas; preserve as superseded design input rather than rewriting it as execution truth.
- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md` - Defines the intended one-facade convergence contract, behavior requirements, and non-goals.
- `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md` - Defines runtime ownership, adapter boundaries, event/control paths, persistence, and service layering.
- `docs/superpowers/ARCHITECTURE.md` - Current architecture documentation that must agree with the verified Stage 9 boundary.

### Current Boundary And API Evidence
- `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` - Existing receiver-aware 16-method absence ledger, adapter guards, source sanitization, recursive file discovery, retained method inventory, and alternate-facade checks.
- `crates/pi-coding-agent/tests/api_boundary_guards.rs` - Existing root-module, compatibility re-export, canonical dispatcher, event receiver, and internal-contract source guards.
- `crates/pi-coding-agent/tests/public_api.rs` - Existing independent 15-variant stable facade import and behavior evidence.
- `crates/pi-coding-agent/src/lib.rs` - Owns the stable `pi_coding_agent::api` facade and migration compatibility surface.
- `crates/pi-coding-agent/src/coding_session/mod.rs` - Owns `CodingAgentSession::run`, retained lifecycle/query/event/control methods, and the final session-owner method boundary.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SourceScan`, `rust_files_under`, `sanitize_rust_source`, `line_is_cfg_test_gated`, and the receiver-aware method ledger in `product_runtime_boundary_guards.rs` provide an existing guard framework to harden rather than replace blindly.
- The 16-name deleted compatibility inventory and retained public/pub(crate) method expectations already encode the Phase 4 owner boundary.
- Adapter-specific JSON/print, RPC, and interactive tests already contain the banned method inventories and production deprecation checks that can be consolidated behind the new adapter inventory.
- `stable_api_signature_closure_is_importable` in `public_api.rs` already supplies an independent 15-operation positive contract baseline.
- `stable_api_excludes_internal_runtime_contracts` in `api_boundary_guards.rs` provides the current forbidden-type categories that external compile-fail fixtures must strengthen.

### Established Patterns
- Architectural source guards are Rust integration tests with path-specific diagnostics and deterministic offline execution.
- Public API behavior tests import through `pi_coding_agent::api`; co-located owner tests retain the narrow private access required for metadata, dispatcher, and fault-boundary evidence.
- Rust visibility and exhaustive enum matching are preferred enforcement mechanisms. Source scanning remains appropriate for receiver calls, alternate wrappers, local suppressions, and repository ownership rules that the type system cannot express.
- Boundary expectations are independent from the implementation under test; inventories must not be derived from the production facade or operation metadata.
- Verification is layered: focused guard/API tests first, then crate-wide tests and checks, followed by full workspace tests/checks, formatting, source audits, and diff cleanliness.

### Integration Points
- Consolidate adapter ownership and recursive scanning in `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` or a focused support module owned by that boundary suite.
- Strengthen facade privacy in `crates/pi-coding-agent/tests/api_boundary_guards.rs` and external-consumer compile fixtures without exposing production test hooks.
- Preserve and extend the positive facade inventory in `crates/pi-coding-agent/tests/public_api.rs`.
- Audit `crates/pi-coding-agent/src/lib.rs` and public module visibility as the compile-fail fixture access surface.
- Produce the formal closure report under `.planning/phases/05-boundary-enforcement-and-stage-9-closure/` and update current Stage 9 documentation to link to it.

</code_context>

<specifics>
## Specific Ideas

- Treat the centralized adapter inventory as both the recursive scan definition and a coverage assertion: a known adapter cannot silently exist outside all roots.
- Treat exception-count drift as a review signal. A legitimate call copied to a second location is not automatically legitimate.
- Use compile-fail fixtures to prove actual external inaccessibility through every public-looking path, not merely absence from the curated `api` module.
- Keep the closure report concise and auditable: structured command evidence and conclusions, with raw build output left out.
- Preserve historical disagreement between the original plan and actual execution; the superseded marker explains authority without rewriting history.

</specifics>

<deferred>
## Deferred Ideas

- Typed `ProductEvent` payload convergence and compatibility event-subscription deletion remain Stage 10 work. Phase 5 records their current inventory and constraints but does not plan or implement them.

</deferred>

---

*Phase: 5-Boundary Enforcement and Stage 9 Closure*
*Context gathered: 2026-07-13*
