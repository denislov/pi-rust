# Fullscreen Visual Contract For 0.5.6

Status: completed implementation contract. Baseline source commit:
`15dbb9bd815047d438335fd326c4ee67b6d329db` (`0.5.5`).

This document freezes the product-owned vocabulary and responsive behavior for
the 0.5.6 fullscreen visual convergence. Generic terminal geometry, Unicode
width, overlay composition, terminal lifecycle, and theme primitives remain in
`pi-tui`; every product term and composition rule below remains in
`pi-coding-agent`.

## Layout vocabulary

| Term | Owner and meaning |
| --- | --- |
| Conversation | Dominant transcript viewport, or the active child transcript page |
| Context rail | Persistent wide-mode right column |
| Context drawer | Medium-mode right-side surface with an explicit left edge |
| Context page | Narrow-mode replacement for the Conversation work area |
| Composer | Bounded editor including its top/bottom separators and prompt gutter |
| Status | Final-row runtime/context/profile/model summary |
| Composer assistance | Non-capturing slash/file suggestions attached to Composer |
| Support prompt | Non-capturing actionable prompt above Composer |
| Modal dialog | Capturing settings/model/session/tree/authorization surface |
| Context detail | Detail surface spatially owned by the current Context mode |
| Divider | An explicit structural rectangle owned by `ShellLayout` |
| Header/tabs | Stable first row of a panel/page; focus cannot shift its body |
| Tips | Prioritized actions for the current region, not a second global help page |

## Responsive thresholds

The frozen transition widths are 64 and 100 visible columns. Tests cover 63,
64, 65 and 99, 100, 101 in addition to the release viewports.

| Width | Mode | Context behavior | Composer behavior |
| --- | --- | --- | --- |
| `0..=63` | Narrow | replacement page while open | hidden/disabled while Context owns the page |
| `64..=99` | Medium | right drawer while open | retained unless a capturing modal owns input |
| `100..` | Wide | persistent rail plus Context/Tips divider | full-width bottom owner |

Resize preserves the active page, Context tab/item/detail, focus restore target,
editor text/cursor, transcript selection, and transcript scroll. Only bounded
rectangles and viewport projections are recomputed.

## Semantic style roles

No role introduces RGB values. Roles resolve through the existing theme tokens:

| Role | Existing token family | Plain/no-color signal |
| --- | --- | --- |
| focused header/border | `accent`, `borderAccent` | `▌` and explicit title |
| inactive header/border | `borderMuted`, `muted` | stable separator/title |
| selected row | `selectedBg`, `accent` | `›` in a reserved gutter |
| primary/secondary text | normal foreground, secondary foreground | spacing and label hierarchy |
| muted metadata | `muted` | compact suffix/indentation |
| running | `accent` | spinner plus `running` |
| waiting/warning | warning token | `! waiting permission` |
| success | success token | `✓ completed` |
| failure | error token | `× failed` |
| cancelled | muted/warning token | `- cancelled` ASCII fallback |

## Marker ownership

- `▌` identifies focused panel/page or transcript selection in its reserved
  gutter; it never serves as the composer prompt.
- `›` identifies a selected list/autocomplete row and occupies a reserved
  one-column marker slot followed by one spacing column.
- `>` belongs only to the first visual Composer input line.
- spinner/status glyphs belong only to runtime state.
- ASCII fallbacks preserve the same fixed gutter widths.

## Overlay roles

`TransientOverlayRole` is a product type and is the sole mapping from product
surface semantics to generic `OverlayOptions`:

- Composer assistance: `BottomLeft`, non-capturing, at most 72 columns, command
  content aligned to the Composer text origin, and bottom margin derived from
  actual Composer height plus Status.
- Support prompt: `BottomLeft`, non-capturing, two-column outer margin.
- Modal dialog: centered, capturing, independently bounded.
- Context detail: rail, drawer, and page each map to an owning right-side or
  replacement-page geometry; none inherits modal or Composer placement.

## Baseline evidence

The immutable behavior source is the baseline commit above. Initial tmux
captures are retained locally under
`target/perf-baseline/0.5.6-fullscreen-visual/baseline/tmux-120x32/`; they include
inline/fullscreen startup, 60x18 and 120x32 resize, Unicode input, narrow Context,
wide Context focus, wide/narrow settings modal, scrolling, and terminal cleanup.
The immutable source is the authority for exhaustive reconstruction of the old
appearance; producing every permutation of known defects was deliberately not a
release gate. The complete 60x18, 80x24, 120x32, 160x40 dark/light/basic/no-color
matrix is instead retained for the final candidate and compared with the source
baseline facts plus deterministic virtual-terminal evidence by `FVP-010`.

Known 0.5.5 facts intentionally preserved as defects in the baseline are the
centered completion menu, missing Context/Tips divider, repeated `>` semantics,
session label in Status, and whitespace-collapsed Markdown table rows.
