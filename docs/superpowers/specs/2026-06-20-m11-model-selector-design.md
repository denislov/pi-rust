# M11 Model Selector Design

## Goal

Turn Rust interactive `/model` from a recognized placeholder into the first real M11 selector vertical slice.

## Scope

This slice implements:

- `/model <id>` direct model switching.
- `/model` opening an in-TUI model selector.
- selector fuzzy filtering, keyboard navigation, confirmation, and cancellation.
- footer/current session model updates.
- subsequent prompts using the selected model.

This slice intentionally does not implement `/scoped-models`, `/settings`, provider login/logout, theme selection, session tree navigation, or M13 peripheral commands.

## Architecture

The current interactive loop keeps command parsing, transcript state, footer state, and prompt submission in `crates/pi-coding-agent/src/interactive/app.rs`. The first slice will keep dispatch there, but isolate selector-specific state in a small helper module if the code would otherwise make `app.rs` harder to read.

`InteractiveRoot` will own the displayed active model id and a modal state for the selector. Slash command handling will produce local UI actions rather than submitting a prompt. The outer loop will update `PromptContext.model` when the root reports a confirmed model selection, so the next `SessionPromptOptions` uses the new model.

## Interaction

`/model <id>` resolves the id with `pi_ai::lookup_model`. On success it updates the active model and appends a system transcript item. On failure it appends a clear system error and leaves the active model unchanged.

`/model` creates a selector backed by `pi_ai::all_models()`, sorted by provider and id for deterministic tests. Rows show the model id with provider/name as secondary text. Typing filters through the existing `pi_tui::SelectList` fuzzy behavior. Enter confirms and Escape/Ctrl+C cancels. Cancel appends no model-change message and leaves the model unchanged.

## Error Handling

Invalid model ids are non-fatal local UI errors. They are never sent to the provider. Selector cancellation is also local and non-fatal. If the selector is open, normal prompt submission is captured by the selector until it is confirmed or canceled.

## Testing

Tests stay offline. Scripted interactive tests use the faux provider and assert rendered transcript/footer behavior. Unit tests cover slash dispatch and selector state where practical. The key acceptance check is that a prompt submitted after switching models reaches the faux provider with the new model id.
