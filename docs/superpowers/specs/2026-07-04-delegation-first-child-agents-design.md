# Delegation-First Child-Agent Design

Date: 2026-07-04

## Decision

`pi-rust` should not introduce a standalone child-agent product concept with its
own profile type, command namespace, Flow family, capability family, or protocol
event family.

Top-level sessions remain `AgentProfile`-owned. Users are interacting with one
primary agent. When that agent needs help, it requests bounded work through the
existing session-owned delegation boundary:

```text
delegate_agent
delegate_team
```

This keeps the product model small:

- ordinary prompts use the current default `AgentProfile`;
- `/agent` and `/agent:<id>` are explicit user-triggered agent runs;
- `/team` and `/team:<id>` are explicit user-triggered team runs;
- model-requested helper work uses delegation and remains policy-controlled.

`TeamProfile` remains the explicit team workflow model and a possible
`delegate_team` target. It is not the default session model.

## Product Model

The session manifest continues to use:

```text
default_agent_profile_id
```

The built-in `default` agent profile may expose a small read-only helper roster
through delegation. The first helper set should stay conservative:

```text
explore
review
check
```

These helpers are auto-approved by policy because they do not receive write
tools by default. They are intended for context gathering, review, and safe
validation, not direct mutation.

Custom `AgentProfile` entries do not implicitly inherit the built-in helper
roster. A user or project profile must explicitly declare its delegation policy
and allowed agent/team ids. This keeps specialist profiles predictable and
auditable.

## Delegation Policy

The existing `[delegation]` policy remains the product boundary. It should be
documented as a helper roster plus authorization policy:

```toml
[delegation]
allow_delegate_agent = true
allow_delegate_team = false
max_depth = 1
max_parallel_children = 1
require_confirmation = "never"
allowed_agents = ["explore", "review", "check"]
allowed_teams = []
```

Default policy for the built-in `default` profile:

- read-only helpers only;
- auto-approval for those helpers;
- no inherited parent write tools;
- initial `max_depth = 1`;
- no implicit custom-profile inheritance.

Write-capable helpers should require explicit user/project configuration and can
use the existing confirmation modes.

## Data Flow

Delegated helper runs receive a minimal context packet by default. They do not
receive the full parent transcript or full parent runtime state.

The packet contains:

```text
parent_operation_id
tool_call_id
requesting_profile_id
target_agent_id or target_team_id
task
cwd or workspace identity
optional short parent summary
explicit evidence list
target profile capability scope
delegation depth
delegation lineage
```

The required parts are task, target id, operation correlation, depth/lineage,
workspace identity, and target capability scope. Summary and evidence are
optional. Automatic relevance selection can be considered later, but it is not
part of the first version.

The delegated run returns a structured result:

```text
status
summary
final_text
diagnostics
artifacts or references
child_operation_id
```

The parent receives this result summary. The full child token stream is not
appended as ordinary parent transcript content.

## UI Behavior

Interactive UI and transcript rendering should represent delegation as folded
blocks in the parent conversation.

Running:

```text
Explore agent
Task: inspect replay/session-log edge cases
Status: running
```

Completed:

```text
Explore agent completed
Summary: found 3 relevant replay paths and 2 missing tests
```

Failed:

```text
Review agent failed
Reason: provider returned an error
```

Confirmation-required:

```text
Delegation requested
Target: Review agent
Task: review write-capable edit plan
Reason: policy requires confirmation

Approve: /delegation approve <operation-id> <tool-call-id>
Reject:  /delegation reject <operation-id> <tool-call-id> [reason]
```

The parent transcript should show status and summary by default. Detailed child
work can be exposed through child operation correlation or future expansion
hooks. Multiple child runs must not stream raw nested token output into the main
conversation.

## Protocol And API

No new command, capability, or event family is introduced for child-agent work.
The existing delegation event family remains canonical:

```text
delegation_requested
delegation_confirmation_required
delegation_approved
delegation_started
delegation_completed
delegation_failed
delegation_rejected
```

RPC and interactive adapters should add any folded-block display metadata to
delegation event payloads rather than introducing parallel event types.

Explicit user-triggered runs stay separate:

```text
invoke_agent
invoke_team
/agent:<id> <task>
/team:<id> <task>
```

These are not model-requested delegation; they are user commands.

## Session Log

Durable events should preserve the delegation lifecycle and enough information
to rebuild pending confirmations and folded transcript blocks:

- request target and task;
- authorization/confirmation status;
- child operation id;
- result summary;
- diagnostics;
- approval/rejection resolution.

The parent session log should not treat the child transcript as ordinary parent
messages. If detailed child traces are retained, they should be linked artifacts
or child operation records.

## Non-Goals

- Add a standalone child-agent profile type.
- Add a `/child-agent` or equivalent command namespace.
- Add a separate child-agent capability name.
- Add protocol events outside the delegation family for model-requested helper
  work.
- Make `TeamProfile` the default top-level session model.
- Share the full parent transcript with delegated helpers by default.
- Let delegated helpers inherit parent tools or skills without target-profile
  allowlist release.

## Testing Requirements

- Built-in default profile exposes read-only helper delegation.
- Custom profiles expose no helpers unless explicitly configured.
- Read-only helpers are auto-approved.
- Write-capable helper policy requires explicit configuration.
- Delegated helper context omits the full parent transcript by default.
- Delegated helper tools come from the target profile, not the parent profile.
- Completed helper work records a folded delegation result in the parent view.
- RPC uses delegation events only for model-requested helper work.
- Confirmation-required delegation still uses the existing pending queue.
