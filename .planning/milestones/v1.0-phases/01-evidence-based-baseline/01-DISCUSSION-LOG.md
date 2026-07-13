# Phase 1: Evidence-Based Baseline - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-07-11
**Phase:** 1-Evidence-Based Baseline
**Areas discussed:** Audit artifact structure, Completion evidence threshold, Status and confidence taxonomy, History and documentation handling

---

## Audit Artifact Structure

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Primary structure | Matrix-first; narrative-first; matrix plus findings | Matrix plus findings |
| Artifact location | Phase-local audit; long-lived docs audit; old plan; dual files | Phase-local `01-AUDIT.md` |
| Matrix row unit | Public operation; workflow; caller path; two matrices | Public operation variant |
| Downstream mapping | Evidence only; requirement; requirement plus phase; implementation tasks | Requirement plus target phase |

**User's choice:** Use one Phase-local audit with an exhaustive public-operation matrix, a separate compatibility inventory, and a findings report mapped to requirements and Phase 2-5.
**Notes:** Phase 1 must not generate implementation tasks.

---

## Completion Evidence Threshold

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Completion evidence | Source; source plus test; layered evidence; full workspace per item | Layered evidence |
| Git authority | Commit required; tree plus Git corroboration; tree only; old plan commits | Current tree plus Git corroboration |
| Unrunnable tests | Incomplete; complete; split implementation/verification; case-by-case | Split implementation and verification |
| Source guards | Always sufficient; compiler only; expressibility-based; defer | Expressibility-based |

**User's choice:** Require source plus focused tests, with additional boundary evidence where applicable. Keep current-tree facts separate from Git corroboration and verification state.
**Notes:** Full workspace verification remains a Phase 5 closure gate.

---

## Status And Confidence Taxonomy

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Main status | Three-state; extended single enum; split implementation/disposition; free text | Split implementation and disposition |
| Confidence | None; high/medium/low; percentage; uncertain-only | High/medium/low on every item |
| Evidence problems | One issue list; gaps vs blockers; all blockers; no blockers | Separate evidence gaps and blockers |
| Finding importance | Severity; processing obligation; target phase; none | Blocking/required/hardening/informational |

**User's choice:** Use explicit orthogonal fields so implementation, scope disposition, verification, confidence, evidence gaps, blockers, and processing obligation remain distinguishable.
**Notes:** Only blockers prevent Phase 1 verification from passing.

---

## History And Documentation Handling

| Decision | Options considered | Selected |
|----------|--------------------|----------|
| Authority order | Tree only; design only; layered; newest file | Layered authority with explicit conflicts |
| Git depth | Full history; old-plan commits; focused/anomaly-driven; hashes only | Stage 9-focused and anomaly-driven |
| Old-doc edits | Update now; audit only; uncheck only; mark superseded | Independent audit only |
| Canonical refs | Minimal; core architecture set; all design refs; live-only | Core architecture set |

**User's choice:** Preserve source, planning, design, and historical documents as distinct authority layers. Phase 1 records conflicts without editing the old plan or TODO.
**Notes:** Phase 5 owns final closure-document updates.

---

## Agent Discretion

- Exact table column ordering and concise evidence notation.
- Whether supporting caller and compatibility tables are separate sections inside the single `01-AUDIT.md` source of truth.

## Deferred Ideas

None.
