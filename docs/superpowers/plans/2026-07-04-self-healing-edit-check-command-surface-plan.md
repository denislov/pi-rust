# Self-Healing Edit Check Command Surface Plan

## Goal

Expose the existing self-healing edit check-command flow through stable product entry points without exposing raw runner, runtime, provider, or session internals.

## Scope

- Add a public options/request type for `CodingAgentSession` self-healing edits.
- Preserve the existing `self_healing_edit(path, replacements)` API as a compatibility wrapper.
- Allow RPC callers to pass an optional `checkCommand` string on `self_healing_edit`.
- Allow interactive users to pass an optional `--check <command>` suffix on `/self-healing-edit`.
- Keep repair strategy policy out of the public surface for this slice.

## Design

- `SelfHealingEditRequest` owns `path`, replacements, and optional `check_command`.
- `CodingAgentSession::self_healing_edit_with_options(request)` is the canonical API.
- Session internals convert the request into `SelfHealingEditOptions` and install a real check runner only when a check command is present.
- RPC and interactive adapters only pass strings and return/display the existing `SelfHealingEditOutcome.check_output`.

## Verification

- Public API test for check-command execution and persisted successful outcome.
- RPC test for `checkCommand` request serialization and `checkOutput` response data.
- Interactive command parser test for `--check` suffix queuing.
- Focused cargo tests, then workspace formatting/check/test.
