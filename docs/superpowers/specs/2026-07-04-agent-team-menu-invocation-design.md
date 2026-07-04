# Agent And Team Menu Invocation Design

## Purpose

The current interactive agent and team surface uses argument-heavy slash commands:

```text
/agent use <agent-id>
/agent <agent-id> <task>
/team <team-id> <task>
```

This is workable, but it makes users remember profile ids and command shapes even though `pi-rust` already owns a session-scoped `ProfileRegistry`. The interactive UI should use that registry to present discovered agents and teams directly.

This design replaces the interactive space-argument forms with menu-driven `/agent` and `/team` flows. It also keeps colon run shortcuts for users who already know the target id. It keeps the Rust API and RPC command surface unchanged.

## Goals

- Make `/agent` open an interactive agent menu.
- Make `/team` open an interactive team menu.
- Remove interactive support for:
  - `/agent use <agent-id>`
  - `/agent <agent-id> <task>`
  - `/team <team-id> <task>`
- Support `/agent:<agent-id> <task>` as a direct shortcut for the agent menu `Run` action.
- Support `/team:<team-id> <task>` as a direct shortcut for the team menu `Run` action.
- Keep one-off execution available through a menu `Run` action.
- Keep default agent switching available through a menu `Use` action.
- Use the session-owned `ProfileRegistry` as the source for discovered agents and teams.
- Preserve explicit slash-command invocation. Do not add `@agent` or `@team` mention syntax.
- Keep RPC and Rust API invocation commands unchanged:
  - `CodingAgentSession::invoke_agent`
  - `CodingAgentSession::invoke_team`
  - RPC `invoke_agent`
  - RPC `invoke_team`
- Keep profile/team delegated work isolated behind `AgentInvocationFlow` and `AgentTeamFlow`.

## Non-Goals

- Do not keep compatibility for the removed interactive argument forms.
- Do not support `/agent:<id>` or `/team:<id>` without a task as a profile selection command. Bare colon targets without task text should report usage rather than entering pending task mode.
- Do not add a default team profile.
- Do not change model-requested delegation tools.
- Do not change the profile discovery roots or precedence rules.
- Do not rescan profile files on every keypress.
- Do not expose raw session, runtime, provider, filesystem, shell, or Flow internals to menus or plugins.
- Do not change RPC JSONL command shapes.

## Current State

`InteractiveRoot` owns a `ProfileRegistry` and a `default_agent_profile_id`. The registry is loaded from built-in, user, and project profile roots, and is refreshed through existing session/root hydration paths.

The interactive app already has several selector patterns:

- `/model` opens a selector when called without arguments.
- `/resume` opens a selector when called without a direct target.
- `/settings` uses `pi_tui::SettingsList`.
- `pi_tui::SettingsList` supports submenu factories.
- `pi_tui::SelectList` and `SelectorDialog` support directional selection, filtering, confirm, and cancel.

`SettingsList` proves the TUI layer can support nested menus. However, it is shaped around setting values, not workflow actions. The agent/team menu should be a `pi-coding-agent` interactive component rather than a settings-list reuse.

## User Experience

### Agent Menu

Typing and submitting:

```text
/agent
```

opens an agent menu:

```text
Agent
> Info
  Use
  Run
```

Actions:

- `Info` shows discovered agent profiles, the current default profile, and profile diagnostics.
- `Use` opens an agent selector. Confirming a profile sets it as the session default agent profile for later ordinary prompts.
- `Run` opens an agent selector. Confirming a profile enters a pending task mode for that profile. The next submitted editor text becomes a one-off `AgentInvocationFlow` task and does not mutate the default profile.

### Team Menu

Typing and submitting:

```text
/team
```

opens a team menu:

```text
Team
> Info
  Run
```

Actions:

- `Info` shows discovered team profiles, supervisor mode, member count or member ids, and profile diagnostics.
- `Run` opens a team selector. Confirming a team enters a pending task mode for that team. The next submitted editor text becomes a one-off `AgentTeamFlow` task.

There is no `Use` action for teams because the current session model has a default agent profile, not a default team profile.

### Pending Task Mode

After selecting `Agent -> Run -> coder`, the editor returns to normal text input but the root keeps a pending target:

```text
Agent task: coder
```

The user then types a plain task:

```text
refactor module
```

Pressing Enter creates:

```rust
PendingAgentInvocationRequest {
    profile_id: "coder",
    task: "refactor module",
}
```

After selecting `Team -> Run -> implementation`, the same pattern creates:

```rust
PendingAgentTeamRequest {
    team_id: "implementation",
    task: "ship feature",
}
```

Empty task submission should keep the pending target and show a concise usage/error message.

Esc in pending task mode cancels the pending target. Ctrl-C while idle should follow existing editor-clearing behavior, but if a pending target is active and the editor is empty, it should cancel the pending target before exiting the app.

### Colon Run Shortcuts

Users who already know the target id can bypass the menu `Run` selector:

```text
/agent:coder refactor module
/team:implementation ship feature
```

These forms are equivalent to:

```text
/agent
Agent -> Run -> coder -> refactor module

/team
Team -> Run -> implementation -> ship feature
```

They do not mutate the default agent profile. They exist only for one-off run behavior.

## Command Semantics

Interactive slash handling changes as follows:

```text
/agent
```

opens the agent menu.

```text
/team
```

opens the team menu.

Space-style interactive arguments to those commands are invalid:

```text
/agent use coder
/agent coder refactor module
/team implementation ship feature
```

The UI should report the same explicit usage used for malformed colon forms:

```text
Usage: /agent or /agent:<agent-id> <task>
Usage: /team or /team:<team-id> <task>
```

Colon run shortcuts are valid only when both an id and a non-empty task are present:

```text
/agent:coder refactor module
/team:implementation ship feature
```

The parser should normalize these to the same pending request types used by menu `Run`:

```rust
PendingAgentInvocationRequest {
    profile_id: "coder",
    task: "refactor module",
}

PendingAgentTeamRequest {
    team_id: "implementation",
    task: "ship feature",
}
```

Malformed colon forms should report usage:

```text
/agent:
/agent:coder
/team:
/team:implementation
```

The usage text should always make the run shortcut explicit:

```text
Usage: /agent or /agent:<agent-id> <task>
Usage: /team or /team:<team-id> <task>
```

`/agents` and `/teams` may remain as list commands for quick textual listing, but they are no longer the primary discovery path. They should continue to read from the same `ProfileRegistry`.

`@agent` and `@team` remain ordinary prompt text.

Built-in slash commands continue to win over same-named plugin aliases.

## Architecture

Add a dedicated interactive module:

```text
crates/pi-coding-agent/src/interactive/profile_menu.rs
```

The module owns menu state, rendering, filtering, and action selection. It should not own session mutation. It returns explicit outcomes for `InteractiveRoot` to apply.

Suggested state model:

```rust
enum ProfileMenuKind {
    Agent,
    Team,
}

enum ProfileMenuScreen {
    AgentRoot,
    TeamRoot,
    AgentInfo,
    TeamInfo,
    AgentUse,
    AgentRun,
    TeamRun,
}

enum ProfileMenuOutcome {
    None,
    Close,
    SetDefaultAgent(ProfileId),
    BeginAgentTask(ProfileId),
    BeginTeamTask(ProfileId),
}

enum PendingProfileTask {
    Agent { profile_id: ProfileId },
    Team { team_id: ProfileId },
}
```

`InteractiveRoot` should hold:

```rust
profile_menu: Option<ProfileMenuState>,
pending_profile_task: Option<PendingProfileTask>,
```

The exact names can vary, but the implementation should keep menu state and pending task state explicit. Avoid boolean combinations such as `selecting_agent_menu`, `selecting_agent_use`, and `selecting_team_run` because they make invalid states easy to represent.

## Input Flow

Input priority should become:

1. Tree selector.
2. Active plugin dialog.
3. Active profile menu.
4. Active model/session/settings selectors.
5. Normal editor input.

When a profile menu is active:

- Up/Down/PageUp/PageDown move selection.
- Enter confirms the selected item.
- Esc returns to the previous menu screen, or closes from the root screen.
- Typing filters profile selector screens.
- Backspace edits the profile selector filter.
- Normal editor text should not change.

When `pending_profile_task` is active and no menu is open:

- Normal editor input edits the task text.
- Enter submits the task through the pending target.
- Empty Enter shows an error and keeps the pending target.
- Esc cancels the pending target and clears the editor.
- Ctrl-C cancels the pending target if the editor is empty; otherwise it preserves existing clear-editor behavior.

## Rendering

`InteractiveRoot::render` should render the profile menu in the same area where model, session, settings, and tree selectors render today. The menu should be quiet and scan-friendly:

- title line;
- selected item marker;
- profile id;
- display name or description;
- short hint line with confirm/cancel controls.

Pending task mode should render a small status line near the editor or footer:

```text
Agent task: coder
Team task: implementation
```

The status line should not be inserted into the transcript. It is transient UI state.

## Data Flow

### Agent Use

1. User submits `/agent`.
2. Root opens `ProfileMenuState::agent_root()`.
3. User selects `Use`.
4. Menu shows agent profiles from `root.profile_registry.agents()`.
5. User confirms a profile.
6. Menu returns `SetDefaultAgent(profile_id)`.
7. Root updates `default_agent_profile_id`, sets the existing `InteractiveAction::AgentProfileUse`, and lets the existing loop/session path persist the default profile.

### Agent Run

1. User submits `/agent`.
2. Root opens `ProfileMenuState::agent_root()`.
3. User selects `Run`.
4. Menu shows agent profiles from `root.profile_registry.agents()`.
5. User confirms a profile.
6. Menu returns `BeginAgentTask(profile_id)`.
7. Root stores `PendingProfileTask::Agent { profile_id }` and clears the editor.
8. User types a task and presses Enter.
9. Root creates `PendingAgentInvocationRequest` and sets `InteractiveAction::AgentInvocation`.
10. Existing `PromptTask::spawn_coding_agent_invocation` runs `CodingAgentSession::invoke_agent`.

### Agent Colon Run

1. User submits `/agent:coder refactor module`.
2. Slash parsing identifies command family `agent`, target id `coder`, and task `refactor module`.
3. Root validates that `coder` exists in `root.profile_registry.agent("coder")`.
4. Root creates `PendingAgentInvocationRequest` and sets `InteractiveAction::AgentInvocation`.
5. Existing `PromptTask::spawn_coding_agent_invocation` runs `CodingAgentSession::invoke_agent`.

### Team Run

1. User submits `/team`.
2. Root opens `ProfileMenuState::team_root()`.
3. User selects `Run`.
4. Menu shows team profiles from `root.profile_registry.teams()`.
5. User confirms a team.
6. Menu returns `BeginTeamTask(team_id)`.
7. Root stores `PendingProfileTask::Team { team_id }` and clears the editor.
8. User types a task and presses Enter.
9. Root creates `PendingAgentTeamRequest` and sets `InteractiveAction::AgentTeam`.
10. Existing `PromptTask::spawn_coding_agent_team` runs `CodingAgentSession::invoke_team`.

### Team Colon Run

1. User submits `/team:implementation ship feature`.
2. Slash parsing identifies command family `team`, target id `implementation`, and task `ship feature`.
3. Root validates that `implementation` exists in `root.profile_registry.team("implementation")`.
4. Root creates `PendingAgentTeamRequest` and sets `InteractiveAction::AgentTeam`.
5. Existing `PromptTask::spawn_coding_agent_team` runs `CodingAgentSession::invoke_team`.

## Error Handling

- `/agent` with unsupported space arguments reports `Usage: /agent or /agent:<agent-id> <task>`.
- `/team` with unsupported space arguments reports `Usage: /team or /team:<team-id> <task>`.
- `/agent:<id>` without task reports `Usage: /agent or /agent:<agent-id> <task>`.
- `/team:<id>` without task reports `Usage: /team or /team:<team-id> <task>`.
- `/agent:<id> <task>` with an unknown id reports `Unknown agent profile: <id>`.
- `/team:<id> <task>` with an unknown id reports `Unknown team profile: <id>`.
- Agent `Use` with no profiles should show an empty-state menu. The built-in `default` profile normally prevents this, but the UI should still handle an empty registry defensively.
- Agent `Run` with no profiles should show an empty-state menu.
- Team `Run` with no teams should show an empty-state menu.
- Profile diagnostics should be visible from `Info`, but invalid files should not block valid profile selection.
- If a selected profile disappears before session execution, the existing flow validation should emit the current unknown-profile failure.
- Pending task submit with empty text should not clear the pending target.
- Esc should always provide a way back or out from menu and pending task modes.

## Testing

Add focused tests around the root/input layer:

- `/agent` opens the agent root menu.
- `/team` opens the team root menu.
- `/agent use coder` reports `Usage: /agent or /agent:<agent-id> <task>` and does not change the default profile.
- `/agent coder refactor module` reports `Usage: /agent or /agent:<agent-id> <task>` and does not queue invocation.
- `/agent:coder refactor module` queues `PendingAgentInvocationRequest`.
- `/agent:coder` reports `Usage: /agent or /agent:<agent-id> <task>`.
- `/team implementation ship feature` reports `Usage: /team or /team:<team-id> <task>` and does not queue team invocation.
- `/team:implementation ship feature` queues `PendingAgentTeamRequest`.
- `/team:implementation` reports `Usage: /team or /team:<team-id> <task>`.
- `@agent coder refactor module` remains ordinary prompt text.
- `@team implementation ship feature` remains ordinary prompt text.
- Agent `Info` renders the current default profile and discovered profile ids.
- Agent `Use -> coder` sets the default profile through the existing agent-profile-use action path.
- Agent `Run -> coder -> task` queues `PendingAgentInvocationRequest`.
- Team `Info` renders discovered team ids and supervisor/member information.
- Team `Run -> implementation -> task` queues `PendingAgentTeamRequest`.
- Esc returns from a selector screen to the root menu.
- Esc closes the root menu.
- Esc cancels pending task mode.
- Filtering in agent/team selectors narrows candidate profiles.
- Built-in `/agent` and `/team` continue to win over same-named plugin aliases.

Add scripted interactive tests:

- menu-driven agent `Run` executes a one-off agent invocation.
- menu-driven team `Run` executes a team invocation.
- menu-driven agent `Use` affects the following ordinary prompt.
- `/agent:<id> <task>` executes a one-off agent invocation.
- `/team:<id> <task>` executes a team invocation.

Suggested verification after implementation:

```bash
cargo fmt --check
cargo test -p pi-coding-agent agent_profile
cargo test -p pi-coding-agent team_profile
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_abort
cargo test --workspace
cargo check --workspace
```

## Documentation Updates

After implementation, update:

- `docs/agent-profiles.md`
- `docs/TODO.md`
- `docs/superpowers/plans/2026-07-02-agent-profile-team-slash-invocation-plan.md`

The user-facing docs should describe:

```text
/agent
/team
```

as menu entrypoints, and should remove the old argument examples from the interactive command list. RPC examples should remain unchanged.

They should also document colon run shortcuts:

```text
/agent:<agent-id> <task>
/team:<team-id> <task>
```

## Migration Notes

This is an intentional interactive breaking change. Existing TUI scripts using `/agent <id> <task>`, `/agent use <id>`, or `/team <id> <task>` must move to the menu flow, the colon run shortcut, or RPC for non-interactive invocation.

The Rust API and RPC remain stable, so automation should prefer:

```jsonl
{"type":"invoke_agent","profileId":"coder","task":"..."}
{"type":"invoke_team","teamId":"implementation","task":"..."}
```

for explicit programmatic execution.
