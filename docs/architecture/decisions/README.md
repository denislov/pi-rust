# Architecture Decision Register

## Status Rules

- **Accepted**: normative decision; implementation may still be pending and is
  tracked separately.
- **Proposed**: selected train direction awaiting required prototype/evidence or
  final decision review; dependent implementation remains blocked.
- **Scheduled**: owner, alternatives, prototype, and deadline are known, but the
  decision is intentionally owned by a later release.
- **Skipped**: the owning scope was removed from the train; the decision is not
  an open obligation or accepted contract and requires a new reviewed plan to
  resume.
- **Superseded**: retained with a link to its replacement and migration impact.

An ADR is not accepted merely because a roadmap names its expected result. Its
file must record context, alternatives, selected design, prohibited edges,
failure/security consequences, compatibility, and verification.

## Register

| ADR | Decision | Status | Owner/deadline | Required evidence | Blocking tasks |
| --- | --- | --- | --- | --- | --- |
| [ADR-001](ADR-001-runtime-owner-and-finalization.md) | runtime owner graph and typed finalization handoff | Accepted 2026-07-18 | `0.4.0` | owner-edge, identity, writer, outbox, shutdown matrices | `RIF-001`, `RIF-002`, `RIF-007`–`RIF-009` |
| [ADR-002](ADR-002-extension-grants-and-leases.md) | instance grants and operation leases | Accepted 2026-07-18 | `0.4.0`, implement `0.4.2` | offline revoke/stale-generation prototype passed | `EKR-003` |
| [ADR-003](ADR-003-isolated-wasm-invocation.md) | isolated per-invocation Wasm Store/Instance | Accepted 2026-07-18 | `0.4.0`, implement `0.4.2` | locked Wasmtime async cancel/fuel/epoch/memory and TypeScript/WIT component fixtures passed | `EKR-004` |
| [ADR-004](ADR-004-extension-state-and-facts.md) | extension state/fact scopes and transaction boundaries | Accepted 2026-07-18 | boundary decision retained; `0.4.3` implementation Skipped | offline state/outbox/activation prototype passed | no current blocking task; former `ESS-001`–`ESS-003`, `ESS-006` are Skipped |
| [ADR-005](ADR-005-workbench-protocol.md) | Workbench retained snapshot/patch/state protocol | Accepted 2026-07-18 | boundary decision retained; `0.4.4` implementation Skipped | two-view revision/gap/resync prototype passed | no current blocking task; former `WAP-001`, `WAP-002` are Skipped |
| ADR-006 | Application Profile composition | Skipped 2026-07-19 | requires a new reviewed plan | no implementation evidence claimed | former `WAP-001`, `WAP-004` are Skipped |
| [ADR-007](ADR-007-extension-package-quarantine.md) | package quarantine and coordinated update | Accepted 2026-07-19 | minimum quarantine implemented in `0.4.2`; coordinated update Skipped | quarantine/integrity matrix passed; phase recovery not claimed | `EKR-001` complete; former `ESS-001`, `ESS-006` are Skipped |
| [ADR-008](ADR-008-extension-contract-versioning.md) | Manifest/WIT/Host API/schema versioning | Accepted 2026-07-19 | minimum contracts implemented in `0.4.2`; service evolution Skipped | generated binding/hash/compatibility fixtures | `EKR-001` complete; former `ESS-001` is Skipped |
| ADR-009 | performance baselines and budgets | Skipped 2026-07-19 | provisional released-path guards retained; final Extension/Workbench budget requires a new plan | released-path baseline evidence only | former `DXH-005` is Skipped |
| ADR-010 | audit/trace/log/diagnostic contract | Skipped 2026-07-19 | requires a new reviewed plan | no Extension observability expansion claimed | former `ESS-001`, `ESS-007` are Skipped |
| [ADR-011](ADR-011-recovery-pending-management.md) | `RecoveryPending` ownership and resolution | Accepted 2026-07-18 | `0.4.0` | restart/retry/operator/subsequent-work matrix | `RIF-002`, `RIF-009` |
| [ADR-012](ADR-012-core-extension-handler-boundary.md) | core versus extension handler boundary | Accepted 2026-07-19 | `0.4.2` `EKR-006`; contribution consumption Skipped | DTO/dispatch slice and package-derived extension refs | `EKR-006` complete; former `EKR-005` is Skipped |
| [ADR-013](ADR-013-generic-flow-ownership.md) | generic Flow ownership and migration | Accepted 2026-07-19 | `0.4.1` | complete Flow inventory and cancellation/max-step/missing-transition matrix | `AWC-002`, `AWC-003` |

## Traceability

Each accepted ADR must link to its owning plan tasks and named test/evidence
families. Completion records replace planned evidence with commands, test names,
protocol/API versions, and source commits. If an implementation lands before its
blocking ADR is accepted, that work is provisional and cannot close the task.
