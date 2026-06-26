# Compaction trigger & TUI error wrap fix — execution todos

Source plan: `docs/compaction-trigger-and-tui-error-wrap-fix-plan.md`

## Implementation order (TDD)

- [x] 1. Add `ContextUsageEstimate` + `estimate_context_tokens()` with unit tests (`pi-agent-core` compaction/estimate.rs)
- [x] 2. Gate `compact_before_provider_request()` with `should_compact()` before `prepare_compaction()`
- [x] 3. Add runtime tests proving large-context models do not compact early (update existing runtime compaction tests for new gating)
- [x] 4. Serialize compaction summaries as text-only prompts (`summarize.rs`)
- [x] 5. Add summarization tests for histories containing tool calls + split tool results
- [x] 6. Change `render_error_message()` to wrap to width (`interactive/render.rs`)
- [x] 7. Add interactive render tests for long errors
- [x] 8. Run focused tests, then workspace checks (`cargo fmt --check`, `cargo test --workspace`, `cargo check --workspace`)

## Acceptance criteria (verified)

1. `deepseek-v4-flash` (~1% usage) does not auto-compact. → `runtime_compaction_does_not_trigger_on_large_context_model`
2. Trigger threshold = `model.context_window - reserve_tokens`. → `runtime_compaction_triggers_near_context_limit` + `should_compact` gate in `compact_before_provider_request`
3. `keep_recent_tokens` only controls retained messages after compaction starts. → gate uses only `reserve_tokens`; `prepare_compaction` still uses `keep_recent_tokens`
4. Context estimate does not sum every assistant `usage.total_tokens`. → `estimate_context_tokens` anchors on last valid usage (6 unit tests)
5. Summarization requests contain no provider-level assistant tool calls / tool result messages. → `build_summarization_context` single-user-message tests
6. Long TUI errors wrap to terminal width and retain full text. → 3 render tests
7. Existing session compaction entries + TUI transcript behavior stay compatible. → `persists_compaction_entry_when_continued_session_is_too_large` updated; `render_transcript_lines_colors_error_item_red_bold` unchanged; 1018 workspace tests pass

## Verification commands run

- `cargo test -p pi-agent-core compaction` — 22 passed
- `cargo test -p pi-coding-agent interactive` — 176 lib + interactive passed
- `cargo test -p pi-coding-agent --test session_print_mode` — 3 passed
- `cargo fmt --check` — clean (exit 0)
- `cargo test --workspace` — 1018 passed, 0 failed
- `cargo check --workspace` — clean (exit 0)
- `cargo build --workspace` — no warnings
