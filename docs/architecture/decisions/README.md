# Architecture Decision Register

## Status Rules

- **Accepted**: normative decision; implementation may still be pending and is
  tracked separately.
- **Proposed**: selected train direction awaiting required prototype/evidence or
  final decision review; dependent implementation remains blocked.
- **Scheduled**: owner, alternatives, prototype, and deadline are known, but the
  decision is intentionally owned by a later release.
- **Superseded**: retained with a link to its replacement and migration impact.

An ADR is not accepted merely because a roadmap names its expected result. Its
file must record context, alternatives, selected design, prohibited edges,
failure/security consequences, compatibility, and verification.

## Register

| ADR | Decision | Status | Owner/deadline | Required evidence | Blocking tasks |
| --- | --- | --- | --- | --- | --- |
| [ADR-001](ADR-001-runtime-owner-and-finalization.md) | runtime owner graph and typed finalization handoff | Accepted 2026-07-18 | `0.4.0` | owner-edge, identity, writer, outbox, shutdown matrices | `RIF-001`, `RIF-002`, `RIF-007`–`RIF-009` |
| ADR-002 | instance grants and operation leases | Proposed | `0.4.0`, implement `0.4.2` | revoke/stale-generation disposable Host API prototype | `EKR-003` |
| ADR-003 | isolated per-invocation Wasm Store/Instance | Proposed | `0.4.0`, implement `0.4.2` | offline async cancel/fuel/epoch/memory prototype | `EKR-004` |
| ADR-004 | extension state/fact scopes and transaction boundaries | Proposed | `0.4.0`, complete `0.4.3` | replay-without-code and cross-store failure model | `ESS-001`–`ESS-003`, `ESS-006` |
| ADR-005 | Workbench retained snapshot/patch/state protocol | Proposed | `0.4.0`, freeze `0.4.4` | two thin semantic views plus gap/resync prototype | `WAP-001`, `WAP-002` |
| ADR-006 | Application Profile composition | Scheduled | `0.4.4` before `WAP-002` | base/overlay/conflict/degraded matrix | `WAP-001`, `WAP-004` |
| ADR-007 | package quarantine and coordinated update | Scheduled | accept `0.4.2`, complete `0.4.3` | phase-crash/fencing/recovery prototype | `EKR-001`, `ESS-001`, `ESS-006` |
| ADR-008 | Manifest/WIT/Host API/schema versioning | Scheduled | accept `0.4.2`, complete `0.4.3` | generated hash and compatibility fixture | `EKR-001`, `ESS-001` |
| ADR-009 | performance baselines and budgets | Scheduled | baseline `0.4.0`, accept `0.4.5` | reproducible baselines from every release | `RIF-010`, `DXH-005` |
| ADR-010 | audit/trace/log/diagnostic contract | Scheduled | `0.4.3` before service implementation | redaction, correlation, rate/retention fixture | `ESS-001`, `ESS-007` |
| [ADR-011](ADR-011-recovery-pending-management.md) | `RecoveryPending` ownership and resolution | Accepted 2026-07-18 | `0.4.0` | restart/retry/operator/subsequent-work matrix | `RIF-002`, `RIF-009` |
| ADR-012 | core versus extension handler boundary | Scheduled | `0.4.2` before contribution migration | DTO/dispatch vertical slice | `EKR-006`, then `EKR-005` |

## Traceability

Each accepted ADR must link to its owning plan tasks and named test/evidence
families. Completion records replace planned evidence with commands, test names,
protocol/API versions, and source commits. If an implementation lands before its
blocking ADR is accepted, that work is provisional and cannot close the task.
