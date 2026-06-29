# Phase 2 ResolveRequest Node Design

## Purpose

Finish the remaining Phase 2 prompt-flow node gap by making `resolve_request` a real `PromptTurnFlow` node.

This node should not parse CLI arguments or duplicate adapter request resolution. Its job is to validate and lock in the request contract that adapters pass to `PromptTurnOptions`, so later flow nodes can depend on a resolved request boundary instead of discovering missing request state indirectly.

## Scope

Implement a narrow request-resolution stage inside `PromptTurnContext` and `PromptTurnFlow`:

- add internal request-resolved state to `PromptTurnContext`;
- make `ResolveRequest` validate the `PromptTurnOptions` contract;
- make `PrepareInput` and `ResolveRuntime` require that request resolution already happened;
- keep adapter parsing and runtime construction outside this node;
- update tests and TODO status for the Phase 2 flow-node slice.

This slice does not add non-persistent sessions, RPC routing, TUI routing, or manual compaction support.

## Node Contract

`ResolveRequest` consumes `PromptTurnOptions` from the context and produces an internal "request resolved" state.

Required validations:

- text prompts must not be empty;
- content prompts must not be empty;
- `Skill` and `PromptTemplate` invocations are accepted as already resolved product invocations;
- `Compact` is rejected with `CodingSessionError::UnsupportedCapability`;
- a runtime snapshot must be present in `PromptTurnOptions`, because Phase 2 adapters are expected to pass runtime-backed options into the flow.

On success, the node marks the request as resolved. Running it more than once should be idempotent.

## Context Changes

`PromptTurnContext` should expose narrow methods:

```rust
resolve_request()
request_is_resolved()
```

The resolved state should remain internal to `pi-coding-agent`. Public APIs should not expose a request-resolution flag.

`prepare_input()` should fail clearly if called before `resolve_request()`.

`resolve_runtime_from_options()` should fail clearly if called before `resolve_request()`. It should still own the runtime attachment from `PromptTurnOptions` into `PromptTurnContext`.

## Error Handling

Use existing product error variants:

- `CodingSessionError::Input` for empty text/content prompts;
- `CodingSessionError::Config` for missing runtime snapshot;
- `CodingSessionError::UnsupportedCapability` for manual compaction in `PromptTurnFlow`;
- `CodingSessionError::Session` for node-order invariant failures such as preparing input before request resolution.

Flow execution will wrap these in `FlowError::NodeFailed`; tests should assert on stable message fragments rather than exact wrapper formatting.

## Tests

Add focused tests for:

- `ResolveRequest` marks a runtime-backed text prompt as resolved;
- `ResolveRequest` rejects missing runtime snapshot;
- `ResolveRequest` rejects empty text input;
- `ResolveRequest` rejects empty content input;
- `ResolveRequest` rejects `PromptInvocation::Compact`;
- `ResolveRequest` is idempotent;
- `PrepareInput` fails clearly when run before request resolution;
- `ResolveRuntime` fails clearly when run before request resolution;
- the full `PromptTurnFlow` still runs through the real node sequence.

Existing focused checks should remain green:

```text
cargo fmt --check
cargo test -p pi-coding-agent coding_session
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test session_print_mode
cargo test -p pi-coding-agent --test session_cli
cargo check --workspace
cargo test --workspace
```

## Stop Conditions

Stop and redesign if:

- `ResolveRequest` needs CLI args, RPC wire structs, or interactive UI state directly;
- request resolution starts building `Agent` or loading resources;
- adapters must mutate `PromptTurnContext` internals instead of constructing `PromptTurnOptions`;
- manual compaction support becomes necessary for this Phase 2 slice.
