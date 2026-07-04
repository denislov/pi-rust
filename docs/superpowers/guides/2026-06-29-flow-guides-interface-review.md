# Flow-Centered Runtime Guides Interface Review

## Review Scope

Reviewed guide set:

- `2026-06-29-phase-1-coding-session-and-session-log-guide.md`
- `2026-06-29-phase-2-prompt-turn-flow-guide.md`
- `2026-06-29-phase-3-adapter-convergence-guide.md`
- `2026-06-29-phase-4-agent-turn-flow-guide.md`
- `2026-06-29-phase-5-plugin-kernel-guide.md`
- `2026-06-29-phase-6-advanced-flow-workflows-guide.md`

This review checks:

- phase handoff contracts;
- owner boundaries;
- event model continuity;
- session persistence continuity;
- plugin/Flow extension restrictions;
- adapter migration safety;
- naming conflicts.

## Summary

No blocking conflicts were found.

The guides intentionally keep two transitional systems alive:

- old `JsonlSessionStorage` for unmigrated adapters;
- new Rust-native session event log for `CodingAgentSession`.

This is not a contradiction as long as:

- new `CodingAgentSession` paths never write old TS-compatible JSONL;
- old paths are explicitly transitional;
- adapters move to `CodingAgentSession` in Phase 3.

## Cross-Phase Handoff Matrix

| Producer | Output | Consumer | Status |
| --- | --- | --- | --- |
| Phase 1 | `CodingAgentSession` shell | Phase 2 prompt API | aligned |
| Phase 1 | `CodingAgentEvent` base enum | Phase 2 event mapping, Phase 3 adapters | aligned |
| Phase 1 | `SessionService` | Phase 2 transaction commit, Phase 3 session actions | aligned |
| Phase 1 | `TurnTransaction` | Phase 2 `RunAgentTurn`, Phase 6 workflows | aligned |
| Phase 1 | Rust-native session log | Phase 2 prompt persistence, Phase 6 replay/export | aligned |
| Phase 2 | `PromptTurnFlow` | Phase 3 adapters, Phase 5 hooks | aligned |
| Phase 2 | `RunAgentTurn` bridge | Phase 4 replacement with `AgentTurnFlow` | aligned |
| Phase 2 | `CodingAgentSession::prompt()` | Phase 3 RPC/TUI migration | aligned |
| Phase 3 | `CapabilityService` concrete model | Phase 5 plugins, Phase 6 advanced flows | aligned |
| Phase 4 | `AgentTurnFlow` | Phase 5 agent-level hook points | aligned |
| Phase 5 | plugin host/capabilities | Phase 6 plugin load and workflow extension | aligned |

## Owner Boundary Review

### `CodingAgentSession`

All phase guides keep `CodingAgentSession` as product owner.

No guide gives plugins, adapters, or Flow nodes direct mutable access to the owner.

### Services

Services remain internal:

- `SessionService`;
- `RuntimeService`;
- `FlowService`;
- `EventService`;
- `CapabilityService`;
- `PluginService`.

Phase 3 adapter migration correctly calls owner methods instead of exposing services.

### `pi-agent-core`

Phase 4 keeps `AgentTurnFlow` in `pi-agent-core` and avoids product concepts.

No guide moves:

- Rust-native session log;
- `CodingAgentEvent`;
- CLI/RPC/TUI concepts;
- plugin Lua host;

into `pi-agent-core`.

## Event Interface Review

Event layers remain distinct:

```text
AgentEvent / FlowEvent
  -> EventService
  -> CodingAgentEvent
  -> CLI/RPC/TUI adapters
```

No guide requires RPC/TUI to consume `FlowEvent`.

Potential tension:

- Phase 3 RPC may need current JSON wire compatibility.
- Phase 2 says `CodingAgentEvent` is the adapter input.

Resolution:

- Keep JSON/RPC wire shape stable by adapting from `CodingAgentEvent`.
- Do not expose `CodingAgentEvent` directly as wire unless explicitly versioned.

## Session Interface Review

Session layers remain distinct:

```text
SessionEventEnvelope / SessionEventData
  persisted facts

CodingAgentEvent
  runtime/product events

Transcript replay
  derived view from session events
```

No guide requires TypeScript session compatibility.

Potential tension:

- Phase 3 says old `JsonlSessionStorage` can remain for old paths.
- Design removes TS session compatibility from new runtime.

Resolution:

- Old storage is transitional only.
- New `CodingAgentSession` paths must create/write Rust-native session logs only.
- No automatic import/export between old and new formats.

## Transaction Interface Review

All mutating phases use:

```text
operation context
  -> TurnTransaction
  -> SessionService finalize
```

No guide allows:

- Flow node direct storage append;
- plugin direct commit;
- adapter direct active leaf mutation for migrated paths;
- delegated flow direct parent session mutation.

Potential gap:

- Phase 1 needs a clear recovery policy for operations without final markers.

Resolution:

- Phase 1 guide states replay must treat those operations as incomplete.
- Implementation should encode this in replay tests before Phase 2 uses the log.

## Prompt Flow to Agent Flow Review

The bridge is consistent:

```text
Phase 2:
  RunAgentTurn -> existing Agent::run()

Phase 4:
  Agent::run() -> AgentTurnFlow wrapper
```

No product code is required to call `AgentTurnFlow` directly.

This keeps Phase 4 isolated to `pi-agent-core`.

## Plugin Interface Review

Plugin restrictions are consistent:

- no raw `CodingAgentSession`;
- no raw `SessionService`;
- no raw auth/provider internals;
- no direct session commit;
- no arbitrary Lua node/subflow in Phase 5.

`FlowExtension` is reserved/first-party first, then restricted Lua later.

No conflict found between Phase 5 and Phase 6. Phase 6 plugin load flow uses the same capability-scoped host model.

## Naming Review

Names are consistent:

- `CodingAgentSession`: product runtime owner.
- `CodingAgentEvent`: product event.
- `SessionEventEnvelope`: persisted event wrapper.
- `SessionEventData`: typed persisted facts.
- `TurnTransaction`: prompt-turn transaction.
- `PromptTurnContext`: product prompt operation context.
- `AgentTurnContext`: low-level agent-loop context.
- `PromptTurnFlow`: product prompt flow.
- `AgentTurnFlow`: core agent loop flow.

Potential naming risk:

- `SessionService` may be confused with old `session.rs`.

Resolution:

- Keep new code under `coding_session/session_service.rs`.
- Treat old `src/session.rs` as transitional legacy product helper until adapters migrate.

## Test Interface Review

The test plan layers are consistent:

1. Phase 1 tests session log, replay, transaction, public API.
2. Phase 2 tests prompt flow and print/json paths.
3. Phase 3 tests RPC/TUI adapters and capabilities.
4. Phase 4 tests agent-core behavior parity.
5. Phase 5 tests plugin capability and failure isolation.
6. Phase 6 tests individual advanced workflows.

No phase requires network or real provider keys.

## Required Implementation Discipline

When implementation starts, preserve these interface rules:

- Add new runtime beside old paths before replacing adapters.
- Promote only `CodingAgentSession` and `CodingAgentEvent` through `api`.
- Keep operation contexts private.
- Keep session event schema typed and versioned.
- Keep `RunAgentTurn` as the only Phase 2 bridge to current agent loop.
- Do not expose Flow node names in product or RPC protocols.

## Final Review Result

The six phase guides are mutually consistent.

The only deliberate transitional overlap is old session storage versus new Rust-native session log. The guides contain enough boundaries to prevent that overlap from becoming a long-term architectural conflict.
