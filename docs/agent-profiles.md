# Agent Profiles And Teams

This document describes the current user-facing `AgentProfile` and `TeamProfile` surface in `pi-rust`.

The implementation is session-owned. A selected agent profile is durable session configuration, not a hidden live supervisor agent. Explicit one-off work uses `/agent` and `/team` slash commands or the matching RPC commands. `@agent` and `@team` mention syntax is intentionally not part of the design.

## Discovery

Profiles are loaded from three sources with deterministic precedence:

1. Built-in profiles compiled into `pi-coding-agent`.
2. User profiles under `$PI_RUST_DIR/agents/*.toml` and `$PI_RUST_DIR/teams/*.toml`.
3. Project profiles under `.pi-rust/agents/*.toml` and `.pi-rust/teams/*.toml` relative to the session cwd.

When two sources define the same id, the later source wins: built-in < user < project. Invalid files produce diagnostics but do not prevent unrelated valid profiles from loading.

If no profile is selected, the session uses the built-in `default` agent profile. If a session is created or opened with a selected default profile, that profile id is persisted in the Rust-native session manifest as `default_agent_profile_id` and restored on resume.

## Agent Profile TOML

Example:

```toml
schema_version = 1
id = "coder"
display_name = "Coder"
description = "Implementation-focused profile"
model = "gpt-5-codex"
system_prompt = "You are a pragmatic coding agent."
tools = ["shell", "apply_patch"]
skills = ["superpowers:test-driven-development"]
supervision = "session"

[delegation]
allow_delegate_agent = true
allow_delegate_team = false
max_depth = 1
max_parallel_children = 1
require_confirmation = "writes"
allowed_agents = ["reviewer"]
allowed_teams = []
```

Fields:

- `schema_version`: currently `1`.
- `id`: stable profile id. Must be non-empty and have no surrounding whitespace.
- `display_name`: display label.
- `description`: optional text shown by profile listing surfaces.
- `model`: optional model id override for ordinary prompts or one-off invocations using this profile.
- `system_prompt`: optional additional runtime instruction text.
- `tools`: optional allowlist of tool ids. Unavailable requested tools are reported as diagnostics.
- `skills`: optional allowlist of skill ids. Unavailable requested skills are reported as diagnostics.
- `supervision`: `session`, `self_review`, or reserved future `llm_supervisor`.
- `[delegation]`: optional policy block. Defaults disable delegation.

## Team Profile TOML

Example:

```toml
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
description = "Planner and coder workflow"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["planner", "coder"]

[delegation]
max_parallel_children = 2
max_depth = 1
require_confirmation = "always"
```

Fields:

- `schema_version`: currently `1`.
- `id`, `display_name`, `description`: same profile identity fields as agent profiles.
- `supervisor`: `deterministic` or an agent profile id.
- `strategy`: currently `plan_execute_review`.
- `members`: ordered list of agent profile ids.
- `[delegation]`: team-level policy fields for future bounded delegation execution.

Every team profile must declare supervisor semantics. Current execution supports deterministic and profile-backed supervisors.

## Interactive Commands

```text
/agents
/agent use <agent-id>
/agent <agent-id> <task>
/teams
/team <team-id> <task>
/delegations
/delegation list
/delegation approve <tool-call-id>
/delegation approve <operation-id> <tool-call-id>
/delegation reject <tool-call-id> [reason]
```

Semantics:

- `/agents` lists resolved agent profiles and marks the current session default.
- `/agent use <agent-id>` changes the session default profile for later ordinary prompts.
- `/agent <agent-id> <task>` runs one isolated one-off agent invocation without changing the default profile.
- `/teams` lists resolved team profiles.
- `/team <team-id> <task>` runs one supervised team invocation without changing the default profile.
- `/delegations` and `/delegation list` show delegation requests currently waiting for confirmation on the in-memory session owner.
- `/delegation approve <tool-call-id>` approves the unique pending request with that tool call id. Use `/delegation approve <operation-id> <tool-call-id>` when multiple pending requests share a tool call id.
- `/delegation reject <tool-call-id> [reason]` rejects the unique pending request with that tool call id. A rejection emits a product event and does not run child work.
- Ordinary text prompts run with the current session default agent profile.

One-off child work streams events but does not directly commit child transcript state into the parent session.

## RPC Commands

RPC mode currently supports:

```jsonl
{"id":"a1","type":"list_agent_profiles"}
{"id":"t1","type":"list_team_profiles"}
{"id":"s1","type":"set_default_agent_profile","profileId":"coder"}
{"id":"i1","type":"invoke_agent","profileId":"coder","task":"implement parser"}
{"id":"g1","type":"invoke_team","teamId":"implementation","task":"ship feature"}
{"id":"d1","type":"list_delegation_confirmations"}
{"id":"d2","type":"approve_delegation","operationId":"op_...","toolCallId":"tool_..."}
{"id":"d3","type":"reject_delegation","operationId":"op_...","toolCallId":"tool_...","reason":"not now"}
```

`set_default_agent_profile` emits a `default_agent_profile_changed` protocol event after the command response. `invoke_agent` and `invoke_team` run through the same background operation path as prompts and stream semantic lifecycle protocol events such as `agent_invocation_start`, `agent_team_start`, `agent_team_member_start`, and matching end/error/abort events.

`list_delegation_confirmations` returns confirmation-held delegation requests for the current session owner. In persistent sessions, the pending list is derived from typed delegation confirmation request/resolution events in the Rust-native session log, so unresolved confirmations are restored after reopening the session. `approve_delegation` records an approval resolution, removes the matching pending request, and executes it through the same session-owned child agent/team flow used by auto-approved delegation. `reject_delegation` records a rejection resolution, removes the pending request, and emits a rejection event. Both commands identify a pending request by the `operationId` and `toolCallId` from the original `delegation_confirmation_required` event.

RPC `get_state.capabilities` includes `agentProfiles`, `teamProfiles`, and `delegation`. Profile/team operations report `busy` while an agent or team invocation is running. Delegation reports `available` when the session owner can run bounded, policy-gated delegation and `busy` while another owner operation is active. Individual `delegate_agent` and `delegate_team` tools are still exposed only when the active `AgentProfile` policy allows them.

## Delegation Boundary

Delegation is model-requested, not model-authorized. A model can only ask through session-owned tools that are exposed by the active profile policy:

- `delegate_agent`
- `delegate_team`

Current behavior:

- Delegation tools are only visible when the active profile policy allows them.
- Allowed target ids and zero-depth policy are enforced at the request boundary.
- Accepted requests return structured envelopes and emit `DelegationRequested` product events.
- Rejected requests return structured rejection envelopes and emit `DelegationRejected` product events.
- Auto-approved requests run through session-owned child agent/team flows and emit `DelegationApproved`, `DelegationStarted`, and either `DelegationCompleted` or `DelegationFailed` product events.
- Delegated child prompts authorize their own queued delegation requests at child depth. Exhausted nested requests emit `DelegationRejected` instead of being silently dropped.
- Requests that require confirmation emit `DelegationConfirmationRequired`, are held in the current session owner's pending confirmation queue, and can be listed, approved, or rejected through interactive slash commands or RPC. Persistent sessions rebuild unresolved pending confirmations from the typed session event log on reopen; non-persistent sessions keep only process-local pending state.
- Pending-confirmation approval emits `DelegationApproved`, starts the child agent/team flow, and then emits `DelegationStarted` plus `DelegationCompleted` or `DelegationFailed`. Pending-confirmation rejection emits `DelegationRejected` and does not run child work.
- The protocol adapter serializes these as `delegation_requested`, `delegation_rejected`, `delegation_confirmation_required`, `delegation_approved`, `delegation_started`, `delegation_completed`, and `delegation_failed`.
- The interactive event bridge renders confirmation-required, approval, rejection, start, completion, and failure delegation events as system notices in the transcript. Confirmation-required notices include the exact approve/reject slash commands.

Still follow-up:

- Richer interactive confirmation prompts/approval UI for write-capable, team, or high-cost delegation beyond the current slash-command path.
- Expiry and stale-request policy for durable pending confirmations.
- Recursive child execution and inherited budget accounting beyond the current child-depth authorization boundary.
- Capability release policy for delegated child work.

Profiles and delegation tools do not expose raw `CodingAgentSession`, session storage, runtime service, provider internals, filesystem handles, shell handles, or Flow graph mutation APIs.
