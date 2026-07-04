# Self-Healing Edit Adapter Exposure Plan

## Context

The self-healing edit workflow now exists below the product adapter layer:

- `SelfHealingEditFlow` wraps edit application with stable Flow nodes.
- `CodingAgentSession::self_healing_edit()` exposes a persistent session-owned Rust API.
- Session events persist `self_healing_edit.started` and `self_healing_edit.completed` lifecycle data.
- The provider-visible builtin `edit` tool already routes through the workflow while preserving legacy edit output shape.

The remaining adapter work is to make the workflow directly invokable by product surfaces without routing through a model tool call.

## Recommended Route

1. [x] Add an RPC `self_healing_edit` command.
2. [x] Return a structured response matching the session outcome shape.
3. [x] Preserve capability gating: persistent sessions only, reject while another operation is running.
4. [x] Add an interactive slash command after the RPC shape is stable.
5. [x] Update `docs/TODO.md` and verification notes when each adapter slice lands.

## RPC Command Shape

Request:

```json
{
  "id": "e1",
  "type": "self_healing_edit",
  "path": "src/app.rs",
  "edits": [
    { "oldText": "old", "newText": "new" }
  ]
}
```

Response data:

```json
{
  "path": "src/app.rs",
  "message": "...",
  "diff": "...",
  "patch": "...",
  "firstChangedLine": 1,
  "attempts": 1,
  "diagnostics": [],
  "checkOutput": null
}
```

Errors should be regular RPC error responses with command `self_healing_edit`.

## Interactive Slash Shape

After RPC lands, add a narrow slash entrypoint that queues a pending adapter action rather than running the edit in the command parser:

```text
/self-healing-edit <path> <oldText> => <newText>
```

The `=>` delimiter keeps the first slice simple while allowing spaces in both edit strings. A later richer form can accept JSON edits if multi-edit interactive usage proves important.

## Verification

Use focused tests first:

- `cargo test -p pi-coding-agent --test rpc_mode rpc_self_healing_edit -- --nocapture`
- `cargo test -p pi-coding-agent interactive::app::tests::self_healing_edit_command -- --nocapture`
- `cargo test -p pi-coding-agent interactive::app::tests::slash_ -- --nocapture`
- `cargo test -p pi-coding-agent interactive::app::tests --lib -- --nocapture`

Then run the crate and workspace checks required by the remote project instructions.
