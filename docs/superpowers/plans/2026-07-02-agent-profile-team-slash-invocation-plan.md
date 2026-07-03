# Agent Profile And Team Slash Invocation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Goal

Add user-defined `AgentProfile` and `TeamProfile` configuration, slash-command invocation, default session profile semantics, and controlled model-requested delegation without using `@agent` or `@team` mention syntax.

The feature should let a user define one or more agent profiles, combine profiles into a team profile, set a session default profile, explicitly invoke a profile or team for one task, and allow the model to request bounded delegation only through a `CodingAgentSession`-owned authorization path.

## Current Decision Record

- Explicit user invocation uses slash commands, not `@` mentions.
- `CodingAgentSession` remains the durable product owner. It is not a hidden live LLM session agent.
- The default `AgentProfile` is session configuration: model, instructions, tool policy, resource policy, and delegation policy for ordinary prompts.
- Starting a session with a selected profile creates or opens a normal `CodingAgentSession` whose default profile is that profile. It should not create a generic session agent and then spawn the selected agent.
- If no profile is selected, the session uses the built-in default profile.
- A single `AgentProfile` does not require a separate LLM supervisor. It can still declare supervision policy such as `session`, `self_review`, or future `llm_supervisor`.
- A `TeamProfile` must have supervisor semantics. The supervisor can be a deterministic team flow policy or an explicit supervisor `AgentProfile`.
- Model-initiated delegation is a request, not direct spawn authority. The model can request delegation through capability-scoped tools such as `delegate_agent` or `delegate_team`; `CodingAgentSession` authorizes and executes the child operation according to policy, budget, recursion depth, permissions, and confirmation rules.

## Architecture

`CodingAgentSession` owns profile selection, profile loading, operation authorization, event recording, and adapter-visible capabilities. `PromptTurnFlow` runs ordinary turns under the session default `AgentProfile`. Explicit `/agent` commands run a one-off `AgentInvocationFlow`. Explicit `/team` commands run an `AgentTeamFlow` that coordinates supervisor/member execution. Child work produces correlated product events and artifacts; parent/session-owned flows decide what becomes durable session state.

`pi-agent-core` remains the low-level runtime crate. It should not know about product slash commands, session manifests, adapter UX, or team profile storage. It can receive resolved runtime settings and resources after `pi-coding-agent` has applied profile policy.

## Non-Goals

- Do not introduce `@agent` or `@team` syntax.
- Do not expose raw `CodingAgentSession`, `SessionService`, `RuntimeService`, provider internals, filesystem handles, shell access, or Flow graph mutation through profile files or model delegation tools.
- Do not let child agent/team flows direct-commit parent session state.
- Do not require every session to allocate a live LLM-backed session agent.
- Do not make plugins or profile files bypass capability-scoped host APIs.
- Do not revive TypeScript session JSONL compatibility.

## Proposed User Surface

Interactive slash commands:

```text
/agents
/agent use <agent-id>
/agent <agent-id> <task>
/teams
/team <team-id> <task>
```

Semantics:

- `/agents` lists resolved built-in, user, and project `AgentProfile` entries with default/current status.
- `/agent use <agent-id>` changes the session default `AgentProfile` for subsequent ordinary prompts and records the change in session state.
- `/agent <agent-id> <task>` runs one task with the selected profile without changing the session default.
- `/teams` lists resolved `TeamProfile` entries and their supervisor mode.
- `/team <team-id> <task>` runs one task through the selected team profile without changing the session default.
- Ordinary text prompts continue through the session default `AgentProfile`.
- Session creation options can select the initial default profile, for example a future CLI/RPC field equivalent to `--agent <agent-id>`.

## Candidate Profile Format

Use TOML for the first implementation because the rest of the project already uses TOML-like plugin manifests and Rust has mature serde support. Keep the schema typed and versioned.

Agent profile example:

```toml
schema_version = 1
id = "coder"
display_name = "Coder"
description = "Default coding agent profile"
model = "gpt-5-codex"
system_prompt = "You are a pragmatic coding agent."
tools = ["shell", "apply_patch"]
skills = ["superpowers:systematic-debugging", "superpowers:test-driven-development"]
supervision = "session"

[delegation]
allow_delegate_agent = true
allow_delegate_team = false
max_depth = 1
require_confirmation = "writes"
```

Team profile example:

```toml
schema_version = 1
id = "implementation-review-team"
display_name = "Implementation Review Team"
description = "Planner, implementer, and reviewer workflow"
supervisor = "planner"
strategy = "plan_execute_review"
members = ["planner", "implementer", "reviewer"]

[delegation]
max_parallel_children = 2
max_depth = 1
require_confirmation = "writes"
```

Discovery roots:

- Built-in profiles compiled into `pi-coding-agent`.
- Project profiles under `.pi-rust/agents/*.toml` and `.pi-rust/teams/*.toml` relative to the session cwd or configured workspace root.
- User profiles under a `PI_RUST_DIR`-scoped config root, for example `$PI_RUST_DIR/agents/*.toml` and `$PI_RUST_DIR/teams/*.toml`.

## Stage 0: Planning Baseline

Tasks:

- [x] Add this plan to `docs/TODO.md` source documents.
- [x] Update the Phase 6 subagent/supervisor item so it points at profile/team slash invocation as the user-facing entrypoint.
- [x] Record that explicit invocation uses `/agent` and `/team`, not `@agent` or `@team`.
- [x] Record that the default `AgentProfile` is session configuration, not a hidden live agent.
- [x] Record that model self-delegation must go through `CodingAgentSession` authorization.

Acceptance:

- Future subagent/supervisor work can start without reopening the invocation syntax and ownership decisions.

## Stage 1: Profile Data Model And Registry

Files:

- `crates/pi-coding-agent/src/coding_session/profiles.rs`
- `crates/pi-coding-agent/src/coding_session/mod.rs`
- `crates/pi-coding-agent/tests/public_api.rs`
- `crates/pi-coding-agent/tests/profile_registry.rs` or focused module tests

Tasks:

- [x] Add typed `AgentProfile`, `TeamProfile`, `ProfileId`, `ProfileSource`, `ProfileDiagnostic`, and `ProfileRegistry` models.
- [x] Add `SupervisionPolicy` for single-agent profiles with at least `session` and `self_review` variants; reserve `llm_supervisor` behind explicit future implementation.
- [x] Add `TeamSupervisor` for team profiles with deterministic and profile-backed supervisor variants.
- [x] Add `DelegationPolicy` with maximum depth, confirmation mode, parallel child limits, and allowed target ids.
- [x] Add TOML parsing with schema version validation and fail-open diagnostics for invalid profile files.
- [x] Add built-in default `AgentProfile` construction.
- [x] Add deterministic merge precedence: built-in < user < project, with duplicate id diagnostics.
- [x] Add offline unit tests for valid profile load, invalid profile diagnostics, duplicate id precedence, and built-in default fallback.

Acceptance:

- A session can resolve an `AgentProfile` id and a `TeamProfile` id without touching adapters.
- Invalid profile files do not prevent unrelated valid profiles from loading.
- Profile values are structured data, not ad hoc strings passed into runtime internals.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent profile
source ~/.cargo/env && cargo test -p pi-coding-agent public_api
```

## Stage 2: Session Default AgentProfile Semantics

Files:

- `crates/pi-coding-agent/src/coding_session/context.rs`
- `crates/pi-coding-agent/src/coding_session/event.rs`
- `crates/pi-coding-agent/src/coding_session/session_service.rs`
- `crates/pi-coding-agent/src/coding_session/mod.rs`
- `crates/pi-coding-agent/tests/public_api.rs`

Tasks:

- [x] Add default profile id/options to `CodingAgentSessionOptions`.
- [x] Persist the default profile id in the Rust-native session manifest or an explicit typed session event.
- [x] Restore the default profile id when opening or resuming a session.
- [x] Add an owner method for changing the default profile, for example `CodingAgentSession::set_default_agent_profile()`.
- [x] Emit a canonical product event when the default profile changes.
- [x] Keep ordinary prompts using the current default profile without creating a hidden live session agent.
- [x] Add tests for new-session default profile, explicit selected profile, resumed profile restoration, and profile switch persistence.

Acceptance:

- A direct single-agent session is represented as a normal session with `default_agent_profile_id = <id>`.
- The built-in default is used only when no selected profile exists or the selected profile cannot be resolved under documented fallback policy.
- Resume does not silently reset the profile to the built-in default.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent coding_session
source ~/.cargo/env && cargo test -p pi-coding-agent public_api
```

## Stage 3: Apply Profiles To Runtime Construction

Files:

- `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
- `crates/pi-coding-agent/src/coding_session/prompt.rs`
- `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`
- `crates/pi-coding-agent/src/coding_session/flow_service.rs`

Tasks:

- [x] Extend runtime construction inputs so a resolved `AgentProfile` can influence model id, system prompt, tool policy, skill policy, and delegation tool availability.
- [x] Keep adapter parsing outside `PromptTurnFlow`; the flow should receive resolved or resolvable session-owned profile context.
- [x] Ensure profile application is deterministic and visible in tests using faux providers.
- [x] Emit diagnostics when a profile asks for unavailable tools/skills instead of passing silent partial state to the model.
- [x] Preserve existing prompt behavior under the built-in default profile.

Acceptance:

- Ordinary prompts run through the session default profile.
- Profile-specific model/instruction/tool settings are applied through session-owned runtime construction.
- Low-level `pi-agent-core` receives resolved runtime settings rather than product profile metadata.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent prompt
source ~/.cargo/env && cargo test -p pi-coding-agent coding_session
```

## Stage 4: Interactive Slash Commands

Files:

- `crates/pi-coding-agent/src/interactive/slash.rs`
- `crates/pi-coding-agent/src/interactive/commands.rs`
- `crates/pi-coding-agent/src/interactive/loop.rs`
- `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- `crates/pi-coding-agent/tests/interactive_mode.rs`

Tasks:

- [x] Add built-in slash command definitions for `agents`, `agent`, `teams`, and `team`.
- [x] Implement `/agents` and `/teams` list output from the session-owned profile registry.
- [x] Implement `/agent use <agent-id>` as a session default profile switch.
- [x] Implement `/agent <agent-id> <task>` as a one-off agent invocation operation.
- [x] Implement `/team <team-id> <task>` as a one-off team invocation operation.
- [x] Reject missing ids/tasks with usage text. `/agent use`, one-off `/agent`, and `/team` parser validation are covered.
- [ ] Reject `@agent` and `@team` as normal prompt text; do not add mention parsing.
- [ ] Keep plugin command slash aliases working and avoid id conflicts with built-in slash command names.
- [x] Add completion/suggestion tests for the new slash commands.

Acceptance:

- Explicit agent/team calls are possible from interactive mode only through slash commands.
- A user can switch the default profile without running a task.
- One-off invocation does not mutate the default profile.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent --test interactive_mode agent_profile
source ~/.cargo/env && cargo test -p pi-coding-agent --test interactive_mode team_profile
```

## Stage 5: AgentInvocationFlow

Files:

- `crates/pi-coding-agent/src/coding_session/agent_invocation_flow.rs`
- `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- `crates/pi-coding-agent/src/coding_session/event.rs`
- `crates/pi-coding-agent/src/protocol/events.rs`
- `crates/pi-coding-agent/tests/public_api.rs`

Tasks:

- [x] Add an `AgentInvocationFlow` for one-off `/agent <id> <task>` execution.
- [x] Resolve the target `AgentProfile` through the session registry.
- [x] Create a child operation lineage id correlated with the parent session operation.
- [x] Run the task through the same runtime service boundaries used by ordinary prompts.
- [~] Record product events for invocation started, child output, diagnostics, completion, failure, and cancellation. Start, child output, diagnostics, completion, and failure are covered; abort/cancellation event shape exists, while interactive abort for agent invocation remains unsupported.
- [x] Decide whether invocation output becomes an assistant transcript item, an artifact, or a structured event before durable commit. One-off child output streams as product events and does not commit directly into the parent transcript.
- [x] Enforce busy-state and operation-control rules consistently with compact/export/plugin-load workflows.

Acceptance:

- `/agent <id> <task>` has a visible operation lifecycle and does not bypass `CodingAgentSession`.
- Child work cannot direct-commit arbitrary parent session state.
- Invocation events are stable enough for RPC and interactive adapters.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent agent_invocation
source ~/.cargo/env && cargo test -p pi-coding-agent protocol_events
```

## Stage 6: TeamProfile And AgentTeamFlow

Files:

- `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`
- `crates/pi-coding-agent/src/coding_session/profiles.rs`
- `crates/pi-coding-agent/src/coding_session/flow_service.rs`
- `crates/pi-coding-agent/src/coding_session/event.rs`
- `crates/pi-coding-agent/tests/public_api.rs`

Tasks:

- [x] Add `AgentTeamFlow` with stable nodes for `start_team`, `plan_subtasks`, `run_member_agent`, `collect_member_result`, `merge_or_reject_result`, and `finalize_team`.
- [x] Require every `TeamProfile` to declare supervisor semantics.
- [x] Support an initial conservative strategy such as `plan_execute_review` before adding parallel strategies.
- [x] Resolve member ids to `AgentProfile` values at the session boundary.
- [x] Enforce child operation isolation and parent-controlled commit policy.
- [x] Record team/member lineage in typed product events.
- [~] Add deterministic faux-provider tests for two-member success, member failure, supervisor rejection, and coherent operation lineage. Current coverage includes two-member success, unknown-member failure, profile-backed supervisor execution, parent transcript isolation, and child operation lineage; explicit supervisor rejection and child runtime failure tests remain follow-up.

Acceptance:

- `/team <id> <task>` is implemented as a team workflow, not as a direct adapter loop.
- Supervisor behavior is explicit for every team.
- Member results become events/artifacts until the team flow decides the final session-visible result.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent agent_team_flow
source ~/.cargo/env && cargo test -p pi-coding-agent coding_session
```

## Stage 7: Controlled Model-Requested Delegation

Files:

- `crates/pi-coding-agent/src/coding_session/delegation.rs`
- `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
- `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`
- `crates/pi-coding-agent/src/coding_session/capability_service.rs`
- `crates/pi-coding-agent/tests/public_api.rs`

Tasks:

- [x] Add session-owned delegation request tools such as `delegate_agent` and `delegate_team` only when the active profile policy allows them.
- [~] Make delegation tools return requests into `CodingAgentSession`; do not let the model instantiate child agents directly. Current tools return structured request/rejection envelopes through session-owned runtime construction; converting accepted requests into owner-authorized child flow execution remains follow-up.
- [~] Enforce delegation policy: allowed ids, maximum depth, maximum child count, confirmation mode, and write/tool permissions. Current request-tool boundary enforces allowed agent/team ids and zero-depth rejection; maximum child count, confirmation mode, write/tool permissions, and recursive budget accounting remain follow-up.
- [ ] Add a confirmation boundary for write-capable, team, or high-cost delegation when policy requires it.
- [~] Record delegation requested/approved/rejected/started/completed events. Current event mapping records `DelegationRequested` and `DelegationRejected` from delegation tool envelopes while preserving ordinary tool lifecycle events; approved/started/completed events remain tied to follow-up child execution.
- [ ] Prevent recursive or unbounded delegation loops.
- [~] Keep delegation tools capability-scoped and free of raw session/runtime/provider internals. Current request tools expose only target id/task schemas and structured envelopes; child execution must preserve the same boundary.

Acceptance:

- The model can ask for help, but `CodingAgentSession` decides whether and how to run child work.
- Delegation request/rejection behavior is auditable in the product event stream; approval/execution lifecycle auditing remains follow-up.
- Denied delegation is a normal, deterministic runtime outcome.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent delegation
source ~/.cargo/env && cargo test -p pi-coding-agent capability
```

## Stage 8: RPC, Protocol, And Capability Surface

Files:

- `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- `crates/pi-coding-agent/src/protocol/events.rs`
- `crates/pi-coding-agent/tests/rpc_mode.rs`
- `crates/pi-coding-agent/tests/protocol_events.rs`

Tasks:

- [ ] Expose profile/team availability through `CodingAgentCapabilities`.
- [ ] Add RPC command support for listing profiles, switching default profile, invoking one-off agent work, and invoking team work.
- [ ] Add protocol event mappings for profile changes, agent invocation, team invocation, and delegation lifecycle events.
- [ ] Keep RPC behavior on the same `CodingAgentSession` owner paths used by interactive slash commands.
- [ ] Add serialization tests for all new capability and event fields.

Acceptance:

- Interactive and RPC clients observe the same product semantics.
- Capability status can explain unavailable, disabled, busy, or policy-denied profile/team operations.

Focused checks:

```bash
source ~/.cargo/env && cargo test -p pi-coding-agent --test rpc_mode agent_profile
source ~/.cargo/env && cargo test -p pi-coding-agent --test protocol_events profile
```

## Stage 9: Documentation And Full Verification

Tasks:

- [ ] Update `docs/TODO.md` after each implementation slice.
- [ ] Update the Phase 6 guide if the final node names or team strategy differ from this plan.
- [ ] Document profile file locations, schema, and precedence.
- [ ] Document slash command usage.
- [ ] Document delegation policy and safety behavior.
- [ ] Run focused checks for each slice before broad checks.
- [ ] Run full workspace verification at the end.

Final checks:

```bash
source ~/.cargo/env && cargo fmt --check
source ~/.cargo/env && cargo test -p pi-coding-agent
source ~/.cargo/env && cargo check --workspace
source ~/.cargo/env && cargo test --workspace
source ~/.cargo/env && git diff --check
```

## Acceptance Checklist

- [x] Explicit user invocation uses `/agent` and `/team`, not `@agent` or `@team`.
- [x] `default_agent_profile_id` is durable session configuration and resumes correctly.
- [x] Ordinary prompts use the current default `AgentProfile`.
- [x] `/agent use <id>` switches the default profile without running a task.
- [x] `/agent <id> <task>` runs a one-off agent operation without changing the default profile.
- [x] `/team <id> <task>` runs a supervised team operation without changing the default profile.
- [x] Single-agent sessions do not require a separate LLM supervisor.
- [x] Team profiles always declare supervisor semantics.
- [~] Model-requested delegation goes through session-owned authorization and bounded execution. Request tools are now session-owned and policy-gated; bounded child execution remains follow-up.
- [x] Child operations cannot direct-commit parent session state.
- [~] New product behavior is visible through `CodingAgentEvent` and `CodingAgentCapabilities`. Agent/team lifecycle events and delegation requested/rejected events exist in `CodingAgentEvent`; capability and RPC/protocol surfaces remain Stage 8 follow-up.
- [~] No raw session/runtime/provider internals are exposed through profiles, plugins, or delegation tools. Current delegation request tools preserve this; follow-up child execution must keep the same boundary.
