# Changes

## 0.4.2 - 2026-07-20

### Changed

- Advanced to the workspace `0.4.2` version. Product Extension command and UI
  compatibility paths were removed from `pi-coding-agent`; this crate retains
  only generic terminal, input, rendering, and component behavior.

## 0.3.0 - 2026-07-17

### Added

- Added explicit inline and full-screen terminal modes with alternate-screen,
  mouse-capture, resize, panic-cleanup, and terminal-restoration support.
- Added rectangular layout, bounded surfaces, clipping, dynamic work areas,
  responsive split geometry, focus routing, and generic overlay composition.
- Added normalized SGR mouse input, frame-local hit regions, ANSI-safe width
  handling, and virtual-terminal coverage for focus, resize, clipping, and
  overlay behavior.

### Boundaries

- Product panels, transcript semantics, authorization policy, delegation, and
  plugin actions remain owned by `pi-coding-agent`; this crate exposes only
  generic terminal, rendering, input, focus, and component primitives.

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
