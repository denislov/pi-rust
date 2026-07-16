# Changes

## 0.2.0 - 2026-07-16

### Breaking Changes

- Replaced root and implementation-module imports with the categorized
  `pi_tui::api` facade.
- Removed compatibility source paths after migrating production and test
  consumers to the final owner directories.

### API Categories

- Terminal lifecycle, capability negotiation, and cleanup.
- Input normalization and keybinding contracts.
- Rendering, ANSI width/wrapping, cursor, and overlay primitives.
- Editor state, autocomplete, menus, dialogs, markdown, and image components.
- Deterministic virtual-terminal helpers behind `test-support`.

### Boundaries

- Product commands, provider/model selection, sessions, plugins, and
  ProductEvent semantics remain outside this crate.
- `pi-coding-agent` consumes only the documented TUI dependency-edge allowlist.

### Tests

- Consolidated terminal, input, editing, rendering, component, markdown, and
  image tests into eight integration targets.
- Retained virtual-terminal coverage for cleanup, resize, cursor placement,
  Unicode width, and terminal protocol negotiation.
