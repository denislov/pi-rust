# pi-rust compaction trigger and TUI error wrapping fix plan

## Background

Two related user-visible failures were diagnosed in `pi-rust` interactive TUI sessions:

1. Auto compaction can trigger after only a few turns on large-context models such as `deepseek-v4-flash`, even when the footer reports about 1% context usage.
2. Long agent errors, including provider HTTP 400 responses, are rendered as a single truncated line in the interactive TUI instead of wrapping to the terminal width.

The early compaction problem makes the existing summarization request bug much easier to hit: once compaction starts, Rust currently replays historical assistant tool calls as real LLM messages, so a cut between an assistant `toolCall` and its `toolResult` can produce an invalid OpenAI chat-completions payload.

This plan fixes the trigger logic first, preserves the intended compaction semantics, and includes the TUI rendering fix so users can read the full error if a provider still rejects a request.

## Current behavior

### Auto compaction trigger

Relevant files:

- `crates/pi-agent-core/src/agent_loop.rs`
- `crates/pi-agent-core/src/compaction/prepare.rs`
- `crates/pi-agent-core/src/compaction/estimate.rs`
- `crates/pi-coding-agent/src/config/settings.rs`

Current defaults:

```toml
[compaction]
enabled = true
reserve_tokens = 16384
keep_recent_tokens = 20000
```

`CompactionSettings::default()` in `pi-agent-core` and resolved CLI settings in `pi-coding-agent` use the same values.

There is a correct-looking threshold helper:

```rust
should_compact(estimated_tokens, context_window, settings)
```

It returns true when:

```text
estimated_tokens > context_window - reserve_tokens
```

For `deepseek-v4-flash`, the model registry has:

```text
context_window = 1,000,000
reserve_tokens = 16,384
expected threshold = 983,616 tokens
```

However, `compact_before_provider_request()` currently does not call `should_compact`. It calls `prepare_compaction()` directly on every provider turn. `prepare_compaction()` returns a non-empty `to_summarize` once:

```text
estimate_tokens(messages) > reserve_tokens + keep_recent_tokens
```

With defaults, the effective trigger becomes:

```text
16,384 + 20,000 = 36,384 tokens
```

This ignores the active model's context window. On a 1M-token model, this can compact around 3.6% by this flawed estimate, and often earlier relative to the footer because the estimate is also inflated.

### Token estimate inflation

`estimate_tokens()` currently sums `usage.total_tokens` for every historical assistant message that has usage:

```rust
if message.usage.total_tokens > 0 {
    total += message.usage.total_tokens;
    continue;
}
```

Provider `total_tokens` is usually the whole request context plus output for that turn, not the standalone size of that assistant response. Summing every assistant usage double-counts prior context:

```text
turn 1 total_tokens = 4k
turn 2 total_tokens = 8k  (includes turn 1)
turn 3 total_tokens = 12k (includes turns 1-2)

current Rust estimate = 24k
approx current context = 12k
```

The TypeScript implementation avoids this by using the last valid assistant usage as a context anchor, then estimating only messages after that usage.

### Summarization request shape

`crates/pi-agent-core/src/compaction/summarize.rs` currently converts historical `AgentMessage::Assistant` entries back into real LLM assistant messages. If one contains `ContentBlock::ToolCall`, the OpenAI chat-completions converter emits `assistant.tool_calls`.

If compaction cuts a history segment so the corresponding `ToolResult` is outside the summarized slice, the summarization request violates OpenAI's rule that assistant `tool_calls` must be immediately followed by tool messages for each call.

The TypeScript coding-agent avoids this by serializing conversation history into text inside `<conversation>` tags and sending that as a single user message to the summarization model.

### TUI error rendering

Relevant files:

- `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- `crates/pi-coding-agent/src/interactive/transcript.rs`
- `crates/pi-coding-agent/src/interactive/render.rs`

`AgentEvent::AgentError` is stored as `TranscriptItem::Error { text }`.

`render_error_message()` currently calls `fit_line()` for each original line. `fit_line()` truncates over-wide text instead of wrapping:

```rust
if visible_width(line) <= width {
    line.to_string()
} else {
    truncate_to_width(line, width)
}
```

Therefore long provider errors display as incomplete text, for example:

```text
Error: summarization failed: complete failed: HTTP 400 : {"error":{"message":"An assistant message with 'tool_calls' must be followed by tool messages responding to eac...
```

## Target behavior

### Auto compaction

Auto compaction should trigger only when the estimated active context exceeds the active model's context window minus reserved tokens:

```text
trigger_threshold = model.context_window - settings.reserve_tokens
```

For `deepseek-v4-flash`:

```text
trigger_threshold = 1,000,000 - 16,384 = 983,616 tokens
```

`keep_recent_tokens` should not decide whether compaction starts. It should decide how much recent context remains after compaction has already been deemed necessary.

### Context estimate

Compaction and footer logic should agree on the meaning of context usage:

- Prefer the last successful assistant usage as the current context anchor.
- Skip assistant usage for `StopReason::Error` and `StopReason::Aborted`.
- Ignore all-zero usage.
- Add heuristic estimates only for messages after the last valid usage.
- Fall back to heuristic estimation for all messages only when no valid usage exists.

### Summarization safety

Summarization should not replay historical tool calls as provider-level tool calls. It should serialize the conversation into a text prompt, matching TypeScript behavior.

This makes the summarization request valid even if the summarized history contains tool calls, tool results, split turns, or provider-specific content blocks.

### TUI errors

Long errors should wrap to the available transcript width and preserve the error style.

Expected shape:

```text
Error: summarization failed: complete failed: HTTP 400 :
{"error":{"message":"An assistant message with 'tool_calls'
must be followed by tool messages responding to each
tool_call_id", ...}}
```

The first line should include the `Error:` label. Continuation lines should not be truncated. They may either align at column 0 or be indented under the error body; choose the simpler style that matches existing transcript spacing and keeps line widths valid.

## Proposed changes

### 1. Add TS-parity context usage estimation

Add a new helper in `crates/pi-agent-core/src/compaction/estimate.rs`:

```rust
pub struct ContextUsageEstimate {
    pub tokens: u32,
    pub usage_tokens: u32,
    pub trailing_tokens: u32,
    pub last_usage_index: Option<usize>,
}

pub fn estimate_context_tokens(messages: &[AgentMessage]) -> ContextUsageEstimate
```

Rules:

1. Walk messages from newest to oldest.
2. Find the last assistant message with valid usage:
   - stop reason is not `Error`
   - stop reason is not `Aborted`
   - `total_tokens` or component sum is greater than zero
3. If found, return:
   - `usage_tokens = calculate_context_tokens(usage)`
   - `trailing_tokens = heuristic estimate of messages after that assistant`
   - `tokens = usage_tokens + trailing_tokens`
4. If not found, return heuristic estimate across all messages.

Keep the existing `estimate_tokens()` for sizing slices and retained windows, but avoid using it as the top-level auto-compaction trigger when usage data exists.

Testing:

- Last valid assistant usage is used as the anchor.
- Aborted assistant usage is skipped.
- Error assistant usage is skipped.
- All-zero usage is skipped.
- Trailing user/tool/custom/bash messages are added after the anchor.
- No valid usage falls back to heuristic estimate.

### 2. Gate compaction with model context window

Update `compact_before_provider_request()` in `crates/pi-agent-core/src/agent_loop.rs`:

1. Read `model.context_window`.
2. Compute `estimate_context_tokens(&messages).tokens`.
3. Call `should_compact(tokens_before, model.context_window, &config.settings)`.
4. Return `Ok(None)` if false.
5. Only then call `prepare_compaction()`.

Pseudo-code:

```rust
let usage_estimate = estimate_context_tokens(&messages);
let tokens_before = usage_estimate.tokens;

if !should_compact(tokens_before, model.context_window, &config.settings) {
    return Ok(None);
}

let (to_summarize, keep) = prepare_compaction(&messages, &config.settings);
```

This preserves existing `prepare_compaction()` behavior for choosing the cut point once compaction is necessary.

Edge cases:

- If `context_window == 0`, do not auto compact. This matches the existing `should_compact` guard.
- If `reserve_tokens >= context_window`, `saturating_sub` makes the threshold zero. That means any non-empty context can compact. This is consistent with current helper behavior, but tests should document it.
- If `prepare_compaction()` returns empty after the gate, return `Ok(None)` rather than erroring.

Testing:

- A 1M context model with 10k or 36k estimated tokens does not compact.
- A 1M context model with 984k estimated tokens compacts.
- A 128k context model compacts around `128k - 16,384`.
- `context_window == 0` never auto compacts.
- `keep_recent_tokens` affects retained messages after compaction, not trigger timing.

### 3. Serialize compaction summaries as text-only prompts

Update `crates/pi-agent-core/src/compaction/summarize.rs` so it does not build a `Vec<Message>` mirroring the original conversation.

Add a serializer similar to TypeScript `serializeConversation()`:

```rust
fn serialize_conversation(messages: &[AgentMessage]) -> String
```

Recommended output format:

```text
User:
<text>

Assistant:
<text and thinking, if useful>

Tool read (call_abc):
<tool result text>
```

For `ContentBlock::ToolCall`, serialize as plain text:

```text
Assistant tool call read (call_abc):
{"path":"src/lib.rs"}
```

For images, use a placeholder:

```text
[image: image/png]
```

Then build summarization context as:

```rust
Context {
    system_prompt: Some(system_prompt.into()),
    messages: vec![Message::User {
        content: vec![ContentBlock::Text {
            text: format!(
                "<conversation>\n{}\n</conversation>\n\nPlease summarize the conversation history above.",
                serialize_conversation(messages)
            ),
            text_signature: None,
        }],
    }],
    tools: None,
}
```

Testing:

- Summarization request contains exactly one user message.
- Summarization request has no assistant messages with `ToolCall`.
- Summarization request has no `ToolResult` role messages.
- Tool calls and tool results are still represented in the text.
- Existing compaction tests still observe the returned summary and session compaction event.

This change addresses the HTTP 400 `tool_calls` failure even when compaction legitimately triggers near the context limit.

### 4. Wrap TUI error messages

Update `render_error_message()` in `crates/pi-coding-agent/src/interactive/render.rs`.

Use `pi_tui::wrap_text_with_ansi` or a small plain-text wrapping helper. Prefer wrapping before applying color unless ANSI preservation is needed for embedded provider messages.

Simple implementation shape:

```rust
fn render_error_message(...) -> Vec<String> {
    let prefix = "Error: ";
    let prefix_width = visible_width(prefix);
    let first_width = width.saturating_sub(prefix_width).max(1);

    let mut out = Vec::new();
    for (source_line_index, source_line) in text.split('\n').enumerate() {
        let wrap_width = if out.is_empty() { first_width } else { width };
        let wrapped = wrap_text_with_ansi(source_line, wrap_width);

        for (wrapped_index, wrapped_line) in wrapped.into_iter().enumerate() {
            if out.is_empty() {
                out.push(fit_line(&format!(
                    "{}{}",
                    paint_with("Error: ", &styles.error, color),
                    paint_with(&wrapped_line, &styles.error, color)
                ), width));
            } else {
                out.push(fit_line(&paint_with(&wrapped_line, &styles.error, color), width));
            }
        }
    }
    out
}
```

Important: `fit_line()` is acceptable as a final safety clamp after wrapping, but it must not be the primary layout behavior.

Testing:

- A long single-line error at width 40 renders multiple lines.
- Every rendered line has visible width <= width.
- The full error text is recoverable from the rendered lines when ANSI is stripped.
- Multi-line errors preserve explicit newlines and wrap each paragraph.
- Colored output keeps error style on all wrapped lines.

## Implementation order

1. Add `ContextUsageEstimate` and `estimate_context_tokens()` with unit tests in `pi-agent-core`.
2. Change `compact_before_provider_request()` to call `should_compact()` before `prepare_compaction()`.
3. Add runtime tests proving large-context models do not compact early.
4. Change summarization to text-only serialized prompts.
5. Add summarization tests for histories containing tool calls and split tool results.
6. Change `render_error_message()` to wrap.
7. Add interactive render tests for long errors.
8. Run focused tests, then workspace checks.

## Suggested tests

Focused tests:

```bash
cargo test -p pi-agent-core compaction
cargo test -p pi-coding-agent interactive
cargo test -p pi-coding-agent runtime
```

Full checks after the focused tests pass:

```bash
cargo fmt --check
cargo test --workspace
cargo check --workspace
```

## Acceptance criteria

The fix is complete when all of the following are true:

1. With `deepseek-v4-flash` and default compaction settings, auto compaction does not trigger at about 1% context usage.
2. Auto compaction uses `model.context_window - reserve_tokens` as the trigger threshold.
3. `keep_recent_tokens` only controls how much recent history remains after compaction starts.
4. Context usage estimation does not sum every assistant `usage.total_tokens`.
5. When compaction eventually triggers, summarization requests do not contain provider-level assistant tool calls or tool result messages.
6. Long TUI errors wrap to terminal width and retain their full text.
7. Existing session compaction entries and TUI transcript behavior remain compatible with current tests.

## Risks and mitigations

### Risk: compaction triggers too late for providers with inaccurate `context_window`

Mitigation: keep `reserve_tokens` configurable. If a provider has bad metadata, users can raise `reserve_tokens` or disable compaction until the model registry is corrected.

### Risk: heuristic estimate is lower than actual provider tokenization

Mitigation: this already exists in TS. The reserve buffer absorbs normal tokenizer differences. For histories without provider usage, the heuristic remains conservative enough for text and images.

### Risk: serialized summaries lose structured tool-call fidelity

Mitigation: include tool name, call id, arguments, result text, and error status in the serialized text. This is sufficient for summarization and avoids provider protocol constraints.

### Risk: wrapping colored text breaks ANSI spans

Mitigation: wrap plain text before applying style where possible. For already-colored embedded strings, use `wrap_text_with_ansi` and assert visible widths in tests.

## Out of scope

- A full TypeScript parity port of session-entry based compaction, including previous-summary update prompts and split-turn prefix summaries.
- Changing default compaction settings.
- Changing model registry values for `deepseek-v4-flash`.
- Redesigning the TUI transcript layout beyond error wrapping.
