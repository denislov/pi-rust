# Design: pi-coding-agent interactive async abort loop

- Date: 2026-06-11
- Status: Ready for implementation
- Scope: M6 follow-up slice for `pi-coding-agent` interactive mode.
- Depends on: current M6 vertical slice in `pi-tui` and `pi-coding-agent`, especially `InteractiveRoot`, `StdinBuffer`, `VirtualTerminal`, `InteractiveEventBridge`, `Transcript`, and `run_session_prompt`.

## 1. Context

The current Rust M6 interactive route has a working production entry point and deterministic scripted harness:

- default non-print invocations route to `interactive::run_interactive_mode`;
- non-TTY contexts still return `interactive mode requires a TTY`;
- real TTY contexts start `ProcessTerminal`, create a `Tui`, parse stdin with `StdinBuffer`, submit prompts through `run_session_prompt`, render transcript rows, and restore terminal state;
- scripted tests cover prompt rendering, idle Ctrl+C exit, and JSONL session writes.

The remaining gap is that prompt execution is still awaited synchronously inside the input handler. While the model/tool loop is running, the UI loop cannot keep reading input, cannot render events as they arrive, and cannot process Ctrl+C until `run_session_prompt` returns. This violates the M6 target behavior for streaming UI updates and running-state abort.

Relevant current files:

- `crates/pi-coding-agent/src/interactive/app.rs`
- `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- `crates/pi-coding-agent/src/interactive/transcript.rs`
- `crates/pi-coding-agent/src/protocol/session_runner.rs`
- `crates/pi-agent-core/src/agent.rs`
- `crates/pi-agent-core/src/agent_loop.rs`
- `crates/pi-coding-agent/tests/interactive_abort.rs`
- `crates/pi-coding-agent/tests/interactive_mode.rs`

## 2. Goal

Turn the interactive loop into an asynchronous event loop that can:

1. spawn the prompt/session runner in the background;
2. continue processing terminal input while the prompt is running;
3. stream `AgentEvent` values through `InteractiveEventBridge` into the transcript as they arrive;
4. process Ctrl+C during a running prompt by calling `Agent::abort()`;
5. leave the TUI open and return to idle after abort;
6. keep the existing idle Ctrl+C behavior: empty editor exits, non-empty editor clears;
7. preserve existing print/json/rpc/session behavior.

## 3. Non-goals

This slice does not implement full TypeScript interactive parity.

Deferred to separate slices:

- transcript viewport scrolling;
- component split into dedicated footer/user/assistant/tool components;
- Markdown component integration for assistant rows;
- real provider manual smoke beyond the existing TTY route;
- tool-level process interruption for running bash children beyond the current agent cancellation path;
- extension UI and slash commands.

## 4. Behavior requirements

### 4.1 Prompt execution

Submitting a prompt while idle must:

- append a `TranscriptItem::User`;
- set status to `running`;
- render immediately;
- create an abort handle backed by the same `Agent` that runs the prompt;
- spawn the prompt run on a background Tokio task;
- receive agent events over a channel and apply them on the UI loop thread;
- mark status `idle` when the prompt finishes, errors, or is aborted.

Only one prompt may run at a time. While running, Enter in the editor must not start another prompt.

### 4.2 Event streaming

The background task must not mutate `Transcript` or `Tui` directly. It sends `AgentEvent` values to the UI loop. The UI loop owns transcript mutation:

```text
run_session_prompt / abortable runner
  -> AgentEvent channel
  -> InteractiveEventBridge
  -> UiEvent
  -> Transcript::apply_event
  -> render_tui()
```

This keeps UI state deterministic and testable with `VirtualTerminal`.

### 4.3 Abort

Ctrl+C while status is `running` must:

- call the current prompt task abort handle exactly once;
- keep the TUI open;
- render a running or aborting state immediately;
- allow the background task to complete via the existing agent cancellation path;
- show an error row containing `aborted` or equivalent;
- return status to `idle`.

The implementation must not abort by dropping the `JoinHandle` alone. It must call `Agent::abort()` so provider streams receive the existing cancellation token.

### 4.4 Session persistence

If session persistence is enabled, the background prompt task must still write JSONL entries through the same capture path used by print/json/rpc modes. Aborted prompts may persist the submitted user message and any messages captured before abort. The session file must remain valid JSONL v3.

### 4.5 Existing route behavior

These behaviors must remain unchanged:

- non-TTY default invocation returns `interactive mode requires a TTY`;
- print mode still requires a prompt;
- scripted prompt rendering still shows the user prompt and assistant text;
- scripted session test still appends prompt and response to JSONL;
- `cargo test -p pi-coding-agent` stays green.

## 5. Architecture

### 5.1 Abortable session runner

Add a narrow abortable runner around the existing session runner. The runner should reuse the existing setup and capture logic rather than duplicating session semantics.

Recommended shape:

```rust
#[derive(Clone)]
pub struct SessionPromptAbortHandle {
    agent: pi_agent_core::Agent,
}

impl SessionPromptAbortHandle {
    pub fn abort(&self) {
        self.agent.abort();
    }
}

pub struct SpawnedSessionPrompt {
    pub abort: SessionPromptAbortHandle,
    pub events: tokio::sync::mpsc::UnboundedReceiver<pi_agent_core::AgentEvent>,
    pub done: tokio::sync::oneshot::Receiver<Result<SessionPromptResult, CliError>>,
}

pub fn spawn_session_prompt(options: SessionPromptOptions) -> Result<SpawnedSessionPrompt, CliError>;
```

`pi_agent_core::Agent` currently owns `Arc` state and an `Arc<AtomicBool>`. It can be made `Clone` so the UI loop can keep an abort handle while the background task drives the same agent.

### 5.2 UI event loop

Replace the current synchronous `submit_prompt(...).await` path with a state machine:

```rust
enum RunningPrompt {
    None,
    Active(PromptTask),
}

struct PromptTask {
    abort: SessionPromptAbortHandle,
    events: UnboundedReceiver<AgentEvent>,
    done: oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    abort_requested: bool,
}
```

The loop waits on input chunks and prompt channels. In test mode, scripted chunks should be delivered through the same mechanism as real input chunks so running Ctrl+C is testable.

### 5.3 Input pump

The current `InputSource::read_chunk()` is blocking. For production-quality async behavior, raw input should be pumped into a channel:

```rust
struct InputPump {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
}
```

Production may use a blocking stdin reader thread that sends chunks into the channel. Scripted tests can use a channel preloaded with chunks.

### 5.4 Test provider

Add an abort-aware provider in `tests/interactive_abort.rs`. It should emit a start event, wait on `StreamOptions.cancel`, then emit an error/aborted terminal event. This proves the UI loop can process Ctrl+C while the provider is still waiting.

## 6. Acceptance tests

Required new test:

- `crates/pi-coding-agent/tests/interactive_abort.rs::ctrl_c_aborts_running_prompt_and_keeps_tui_open`

The test must:

- run the interactive harness with chunked input: first chunk submits a prompt, second chunk sends Ctrl+C while the provider is waiting, third chunk sends Ctrl+C while idle to exit;
- use a timeout so the current synchronous implementation fails quickly instead of hanging;
- assert exit code 0;
- assert terminal restored;
- assert rendered output contains the submitted prompt;
- assert rendered output contains `aborted`;
- assert rendered output contains `status: idle`;
- assert the provider observed cancellation.

Existing tests must continue passing:

- `cargo test -p pi-coding-agent --test interactive_abort`
- `cargo test -p pi-coding-agent --test interactive_mode`
- `cargo test -p pi-coding-agent --test interactive_sessions`
- `cargo test -p pi-coding-agent`

Final workspace verification:

- `cargo fmt --check`
- `cargo test -p pi-tui`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`
- `git diff --check`

## 7. Risks

- Refactoring `session_runner` can accidentally change print/json/rpc session persistence. Keep the public `run_session_prompt` behavior unchanged and implement `spawn_session_prompt` as an additional path.
- If the UI loop drops the prompt task instead of calling `Agent::abort()`, provider cancellation will not be tested correctly.
- A blocking stdin read inside the async loop will still prevent event streaming in real TTY mode. Use an input pump/channel for production input chunks.
- The test provider must emit a terminal event after cancellation; otherwise the background task will never finish and the test only proves timeout behavior.
