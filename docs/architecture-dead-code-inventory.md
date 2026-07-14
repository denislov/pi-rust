# Architecture Convergence Dead-Code Inventory

> Baseline: `ae367e2`
>
> Captured: 2026-07-15
>
> Exit target: no unexpected production dead-code warning and no file-level dead-code suppression

## Rules

This inventory covers production and example sources under `crates/*/src` and
`crates/*/examples`. Test-only support is tracked by its owning test and is not a production
suppression.

An entry is removed only when one of these outcomes is proven:

1. `connect`: the modeled path becomes a real production consumer;
2. `delete`: every consumer has migrated to the replacement contract;
3. `narrow`: the symbol becomes test-only or private to its actual owner;
4. `replace`: an equivalent lint, compile-fail guard, or typed contract supersedes it.

Adding a new `allow(dead_code)`, `allow(unused_imports)`, or production `allow(deprecated)` requires
an inventory update with an owner and removal milestone. Crate/file-level suppression is not an
acceptable permanent state.

## Compiler-Visible Baseline Warnings

These warnings are visible in `cargo check --workspace --all-targets`; they are not hidden by an
allow attribute.

| Owner/path | Symbols | Classification | Planned outcome |
|---|---|---|---|
| `coding_session/mod.rs` | `ProductEventReplayHandle`, constructor, replay query | M5 reconnect model is only partially connected | Connect through canonical client replay or delete after Snapshot v2 replacement in M5 |
| `coding_session/mod.rs` | compact cancellation handle | M2 control contract is modeled but not used | Replace with scheduler-owned typed control in M2 |
| `coding_session/mod.rs` | old `load_plugins` wrapper | Replacement operation path exists | Delete after all tests/adapters use `PluginLoad` operation in M2/M6 |
| `client_service.rs` | acknowledge, detach, mark terminal | M5 client lifecycle is partially connected | Connect through ClientConnection v2 in M5 |
| `operation_control.rs` | active operation id/cancellation, compact cancellation types, idle helpers | M2 scheduler inputs are modeled but not consumed | Replace by `OperationScheduler` in M2 |
| `snapshot_coordinator.rs` | accepted status, acknowledged sequence, handle validation, shutdown recovery, stale/capacity errors | M5 Snapshot/client lifecycle is partially connected | Connect or delete against Snapshot v2 state machine in M5 |
| `protocol/rpc/prompt.rs` | imports and reconnect-only test helpers | Disabled `cfg(any())` compatibility tests | Delete obsolete block or migrate tests to public reconnect contract in M5 |
| `protocol/rpc/event_queue.rs` | test-only `try_send_event` | Test helper not used by active tests | Delete or use in M5 bounded-queue tests |
| `tests/agent_profile_session.rs` | deprecated `family` and `kind` fields | v1 wire compatibility assertion | Retain through v1 baseline; delete with ProductEvent v2 in M5 |

## Explicit Suppression Inventory

### M1: Provider Runtime And Agent Loop

| Path | Suppression purpose | Outcome |
|---|---|---|
| `pi-agent-core/src/agent_turn_flow/runtime.rs` | Eight allows hide the retired monolithic turn loop and helpers | Delete in WP1.2 |
| `pi-agent-core/src/loop_runtime/context.rs` | Hides retired request preparation types | Delete in WP1.2; retain active stream option helper |
| `pi-agent-core/src/ai_runtime.rs` | Calls deprecated global `pi_ai` runtime | Replace with injected scoped runtime in WP1.1 |
| `pi-agent-core/examples/loop_example.rs` | Demonstrates deprecated global registry | Rewrite for scoped runtime or delete in WP1.3 |
| `pi-ai/src/lib.rs`, `providers/mod.rs`, `registry.rs` | Keeps global registry compatibility exports working | Delete in WP1.3 |

### M2: Operation Admission And Scheduler

| Path | Suppression purpose | Outcome |
|---|---|---|
| `coding_session/operation.rs` | Metadata, admission, association, runtime generation and dispatch helpers are only partly consumed | Connect through scheduler and descriptor table in M2 |
| `coding_session/operation_control.rs` | Prompt/control generation helper remains broader than production use | Replace with scheduler-owned control contract in M2 |
| `coding_session/intent_router.rs` | Query/control intent surface has an unused branch | Make all adapter intents use router or delete branch in M2 |
| `coding_session/public_operation.rs` | `NotApplicable` association class has no production construction | Delete unless descriptor matrix proves a required operation in M2 |
| `coding_session/mod.rs` operation wrappers | Old snapshot/client/plugin helper methods coexist with `run(Operation)` | Migrate consumers and delete in M2/M6 |

### M3: Durable SessionEvent

| Path | Suppression purpose | Outcome |
|---|---|---|
| `coding_session/session_log/mod.rs` | Entire durable module tree is file-level suppressed | Remove suppression after versioned decoder/recovery paths are connected in M3 |
| `coding_session/session_service.rs` | Store field, old transaction wrappers, replay/recovery and compatibility methods | Pair each old method with its snapshot-aware replacement; delete or connect in M3/M6 |

No durable decoder or recovery symbol may be deleted solely because it is not called by the live
adapter path. Old fixture replay and startup recovery are required consumers.

### M4: Capability And Plugin

| Path | Suppression purpose | Outcome |
|---|---|---|
| `coding_session/capability_snapshot.rs` | File-level suppression covers partially integrated capability handles | Remove file-level allow; connect narrow handles and delete only replaced leaves in M4 |
| `coding_session/plugin_service.rs` | Collection/execution helpers for command/hook/UI/keybind/flow extension are not all used by production manifests | Connect supported capabilities; make an explicit product decision for Flow extension in M4 |
| `plugins/{capability,command,error,flow_extension,hook,keybind,registry,tool,ui}.rs` | Declaration/registration APIs have uneven runtime consumption | Do not bulk delete; resolve per capability after loader/host audit in M4 |
| `plugins/mod.rs` | Re-exports supported plugin declarations not all referenced in non-test builds | Remove unused exports after M4 consumer migration |
| `runtime_service.rs` | Deprecated global provider lookup | Removed by M1 scoped runtime, then capability-gated in M4 |

### M5: ProductEvent, Snapshot And Adapters

| Path | Suppression purpose | Outcome |
|---|---|---|
| `coding_session/event.rs` | Flat compatibility event support types | Delete after canonical typed ProductEvent migration in M5 |
| `coding_session/event_service.rs` | Event/replay/backpressure helpers exceed current consumers | Connect through ProductEvent v2 or delete after replacement in M5 |
| `coding_session/client_projection.rs` | Client-local/runtime projection model is partially connected | Connect through Snapshot v2 in M5 |
| `coding_session/public_projection.rs` | Public reconnect/delivery helper is partly unused | Make RPC/interactive use the public client contract or delete in M5 |
| `interactive/event_bridge.rs` | `last_sequence` is production-dead but used by module tests | Narrow to `cfg(test)` while retaining cursor invariant tests |
| `interactive/render.rs` | Helper is used only under tests for some build modes | Narrow or connect based on TUI rendering owner in M5 |
| `protocol/events.rs` | Compatibility adapter method remains unused | Delete when adapters consume canonical ProductEvent v2 |
| `public_event.rs` | Deprecated v1 field construction | Delete with v2 envelope in M5 |

### M6: Facade And Module Cleanup

| Path | Suppression purpose | Outcome |
|---|---|---|
| `pi-coding-agent/src/lib.rs` | Root compatibility exports and internal test support | Delete root compatibility exports; narrow test support in M6 |
| `pi-coding-agent/examples/manual_test.rs` | Uses deprecated root API | Rewrite against `pi_coding_agent::api` before M6 deletion |
| `coding_session/{agent_invocation,branch_summary,flow_service,manual_compaction,plugin_load,prompt,prompt_flow,self_healing_edit}_flow.rs` | File-level suppression hides mixed orchestration/test seams | Split by owner and remove blanket suppressions in M6 after M2--M5 consumers stabilize |

## Guard Query

The inventory is refreshed with:

```bash
rg -n '#!?\[allow\((dead_code|unused_imports|deprecated)\)\]|cfg_attr\([^\n]*allow\(dead_code\)' \
  crates/*/src crates/*/examples --glob '*.rs'
```

At M6 exit, this query must return no file-level dead-code/unused-import suppression and no
deprecated suppression for removed compatibility paths. Any intentionally retained symbol-level
allow must have a narrower documented invariant and explicit approval.
