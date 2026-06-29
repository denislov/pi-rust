# Phase 2 Print Session Target Convergence Design

## Purpose

Finish the Phase 2 prompt path for print/headless sessions by routing Rust-native session targets through `CodingAgentSession` instead of the old session runner.

This is a convergence step, not a compatibility layer. The old runner may remain only for prompt paths that are not migrated yet, such as no-session execution or adapters explicitly deferred to Phase 3. Once a print session target is supported by `CodingAgentSession`, the print adapter should not keep an equivalent old-runner path for that target.

## Scope

Migrate these print/headless session targets to Rust-native session ownership:

- `ResolvedSessionTarget::New`
- `ResolvedSessionTarget::OpenTarget`
- `ResolvedSessionTarget::OpenOrCreateId`
- `ResolvedSessionTarget::ContinueMostRecent`

Leave `ResolvedSessionTarget::ForkTarget` out of this slice because Rust-native fork and branch semantics are not yet exposed by `SessionService`. The print adapter should return an explicit unsupported-capability error for migrated Rust-native session mode rather than silently falling back to the old JSONL fork path.

No-session print execution can remain on the old runner until the runtime has a direct non-persistent `CodingAgentSession` mode. This is a temporary execution gap, not a session compatibility promise.

## Architecture

`CodingAgentSession` remains the product runtime owner. It opens or creates Rust-native session logs through `SessionService`, owns replay hydration, starts the turn transaction, and runs `PromptTurnFlow`.

`print_mode` remains the adapter boundary. It translates resolved CLI/session request state into either:

- a `CodingAgentSession` plus `PromptTurnOptions` for supported Rust-native session targets; or
- a temporary old-runner execution path only when there is no migrated Rust-native product path.

The adapter should avoid duplicating runtime construction. `PromptTurnOptions::from_session_prompt_options` remains the shared bridge from existing resolved request data into the new prompt runtime.

## Session Target Resolution

`print_mode` should add a small private resolver for Rust-native targets:

- `New`: call `CodingAgentSession::create` with the resolved session log root.
- `OpenTarget(value)`: call `CodingAgentSession::open` with `session_id = value` first. If later user-facing syntax needs path targets, add explicit path handling instead of guessing.
- `OpenOrCreateId(id)`: call `CodingAgentSession::open_or_create` with `session_id = id`.
- `ContinueMostRecent`: call `CodingAgentSession::list`, select the newest summary by `updated_at`, and call `CodingAgentSession::open` with that session id.
- `ForkTarget(_)`: return `CodingSessionError::UnsupportedCapability`.

If `ContinueMostRecent` finds no Rust-native session, return an input/session error equivalent to "no previous session to continue".

The resolver should use `SessionRunOptions.session_dir` when present and otherwise the existing Rust session root resolution. It must not open `JsonlSessionRepo` for migrated targets.

## PromptTurnFlow Boundary

The `open_session` node should stop being a pure no-op. It should validate that the owner prepared the session boundary before agent runtime construction:

- a session id is present;
- replay is attached;
- a turn transaction is active.

The node should not open files directly. File ownership remains in `CodingAgentSession` and `SessionService`, which keeps product/session mutation policy outside individual Flow nodes.

## Error Handling

Unsupported migrated-session actions should fail explicitly. Do not silently route supported Rust-native session mode back to old JSONL storage.

Expected errors:

- missing most-recent Rust-native session for `ContinueMostRecent`;
- missing or invalid id for `OpenTarget`/`OpenOrCreateId`;
- unsupported Rust-native fork target;
- normal `CodingSessionError` propagation from open/create/prompt.

No-session print mode can keep the old runner until a direct non-persistent product runtime is designed. The TODO should identify that as remaining work.

## Tests

Add or extend deterministic offline tests with faux providers:

- explicit `New` still writes Rust-native `session.json` and `events.jsonl`;
- `OpenOrCreateId` creates a Rust-native session, then reopens it on a second prompt;
- `OpenTarget` reuses an existing Rust-native session and hydrates prior transcript into the provider context;
- `ContinueMostRecent` selects the newest Rust-native session;
- `ContinueMostRecent` fails clearly when no Rust-native session exists;
- `ForkTarget` in enabled Rust-native print mode fails as unsupported instead of writing old JSONL;
- no-session print behavior remains covered as temporary old-runner execution.

Focused checks:

```text
cargo fmt --check
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent coding_session
cargo check --workspace
```

Run broader workspace tests if the replay or runtime hydration paths change in shared code.

## Stop Conditions

Stop and redesign if:

- print routing needs to inspect or mutate `SessionService` internals;
- migrated print session targets still write old JSONL session files;
- `PromptTurnFlow` nodes start owning filesystem session resolution;
- old and Rust-native session roots become indistinguishable in user-facing behavior.
