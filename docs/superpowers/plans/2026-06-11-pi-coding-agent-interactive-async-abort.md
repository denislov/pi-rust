# pi-coding-agent Interactive Async Abort Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `pi-coding-agent` interactive mode keep processing input and render events while a prompt is running, with Ctrl+C aborting the active prompt through `Agent::abort()`.

**Architecture:** Add an abortable session prompt runner that returns an abort handle plus event/done channels. Refactor the interactive loop into a small state machine that owns transcript mutation and renders on input, agent events, completion, and abort. Keep print/json/rpc paths unchanged by preserving `run_session_prompt` and adding a new spawned path for interactive mode.

**Tech Stack:** Rust edition 2024, Tokio tasks and `tokio::sync` channels, existing `pi-agent-core::Agent`, existing `pi-ai` provider registry, existing `pi-tui` `VirtualTerminal`, `StdinBuffer`, and `Tui`.

---

## Context

Read these files before editing:

- `docs/superpowers/specs/2026-06-11-pi-coding-agent-interactive-async-abort-design.md`
- `crates/pi-coding-agent/src/interactive/app.rs`
- `crates/pi-coding-agent/src/protocol/session_runner.rs`
- `crates/pi-agent-core/src/agent.rs`
- `crates/pi-agent-core/src/agent_loop.rs`
- `crates/pi-coding-agent/tests/interactive_abort.rs`
- `crates/pi-coding-agent/tests/interactive_mode.rs`
- `crates/pi-coding-agent/tests/interactive_sessions.rs`

Current baseline:

- `run_interactive_mode` starts a real terminal only in TTY contexts.
- `run_interactive_loop` currently awaits `run_session_prompt` inside `submit_prompt`.
- While the await is in progress, no more input chunks are processed.
- `Agent::abort()` exists, but the interactive loop has no handle to the running `Agent`.

## File Structure

- Modify `crates/pi-coding-agent/Cargo.toml`
  - Add Tokio `sync` feature if it is not already enabled.
  - Add `async-stream` as a dev-dependency for the abort-aware test provider.
- Modify `crates/pi-agent-core/src/agent.rs`
  - Make `Agent` cloneable so an abort handle can share the same internal state.
- Modify `crates/pi-coding-agent/src/protocol/session_runner.rs`
  - Add `SessionPromptAbortHandle`, `SpawnedSessionPrompt`, and `spawn_session_prompt`.
  - Extract shared prompt preparation/driving helpers from `run_session_prompt`.
  - Keep existing `run_session_prompt` signature and behavior.
- Modify `crates/pi-coding-agent/src/interactive/app.rs`
  - Replace direct blocking stdin reads with an input pump/channel.
  - Replace blocking prompt await with a running prompt task state.
  - Track an optional running prompt task.
  - Process input, agent events, and prompt completion while the loop is active.
  - Implement running Ctrl+C abort.
- Modify `crates/pi-coding-agent/tests/interactive_abort.rs`
  - Add abort-aware provider and running Ctrl+C test.
- Optionally modify `crates/pi-coding-agent/tests/interactive_mode.rs`
  - Only if the harness function signatures need imports adjusted.

## Task 1: Red Test for Running Ctrl+C Abort

**Files:**
- Modify: `crates/pi-coding-agent/Cargo.toml`
- Modify: `crates/pi-coding-agent/tests/interactive_abort.rs`

- [ ] **Step 1: Add test-only stream helper dependency**

In `crates/pi-coding-agent/Cargo.toml`, ensure dev-dependencies include:

```toml
[dev-dependencies]
async-stream = "0.3"
tempfile = "3"
```

If `[dev-dependencies]` already exists with `tempfile = "3"`, add only `async-stream = "0.3"` under the existing section.

- [ ] **Step 2: Add a failing test and abort-aware provider**

Append this code to `crates/pi-coding-agent/tests/interactive_abort.rs`:

```rust
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use async_stream::stream;
use pi_ai::registry::ApiProvider;
use pi_ai::{
    AssistantMessage, AssistantMessageEvent, Context, EventStream, Model, StopReason,
    StreamOptions,
};
use pi_coding_agent::interactive::test_harness::run_scripted_interactive_with_provider_chunks;

#[derive(Debug)]
struct AbortAwareProvider {
    cancelled: Arc<AtomicBool>,
}

impl AbortAwareProvider {
    fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }
}

impl ApiProvider for AbortAwareProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let cancelled = Arc::clone(&self.cancelled);
        let model_id = model.id.clone();
        let cancel = opts.and_then(|opts| opts.cancel);

        Box::pin(stream! {
            let mut partial = AssistantMessage::empty("faux", &model_id);
            partial.provider = Some("faux".into());
            yield AssistantMessageEvent::Start {
                content_index: None,
                partial: partial.clone(),
            };

            if let Some(cancel) = cancel {
                cancel.cancelled().await;
                cancelled.store(true, Ordering::SeqCst);
            }

            let mut message = AssistantMessage::empty("faux", &model_id);
            message.provider = Some("faux".into());
            message.stop_reason = StopReason::Aborted;
            message.error_message = Some("aborted".to_string());
            yield AssistantMessageEvent::Error {
                reason: StopReason::Aborted,
                message,
            };
        })
    }
}

#[tokio::test]
async fn ctrl_c_aborts_running_prompt_and_keeps_tui_open() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(AbortAwareProvider::new(Arc::clone(&cancelled)));

    let output = tokio::time::timeout(
        Duration::from_millis(500),
        run_scripted_interactive_with_provider_chunks(
            provider,
            vec!["please wait\r", "\x03", "\x03"],
        ),
    )
    .await
    .expect("interactive loop should not hang while aborting")
    .expect("scripted interactive run should succeed");

    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(output.contains("please wait"));
    assert!(output.contains("aborted"));
    assert!(output.contains("status: idle"));
    assert!(cancelled.load(Ordering::SeqCst));
}
```

- [ ] **Step 3: Run the failing test**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_abort ctrl_c_aborts_running_prompt_and_keeps_tui_open
```

Expected: FAIL to compile because `run_scripted_interactive_with_provider_chunks` does not exist.

## Task 2: Add Chunked Scripted Harness API

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Test: `crates/pi-coding-agent/tests/interactive_abort.rs`

- [ ] **Step 1: Change scripted input to accept multiple chunks**

In `crates/pi-coding-agent/src/interactive/app.rs`, replace the current `ScriptedInput::new` implementation with:

```rust
impl ScriptedInput {
    fn new(input: impl Into<String>) -> Self {
        Self::from_chunks(vec![input.into()])
    }

    fn from_chunks(chunks: Vec<String>) -> Self {
        Self {
            chunks: chunks.into(),
        }
    }
}
```

- [ ] **Step 2: Add generic provider harness function**

Inside `pub mod test_harness` in `crates/pi-coding-agent/src/interactive/app.rs`, add:

```rust
pub async fn run_scripted_interactive_with_provider_chunks(
    provider: Arc<dyn pi_ai::registry::ApiProvider>,
    input_chunks: Vec<&str>,
) -> Result<ScriptedInteractiveOutput, CliError> {
    run_scripted_with_provider(provider, input_chunks, None).await
}
```

Then add this helper next to `run_scripted`:

```rust
async fn run_scripted_with_provider(
    provider: Arc<dyn pi_ai::registry::ApiProvider>,
    input_chunks: Vec<&str>,
    session_dir: Option<&Path>,
) -> Result<ScriptedInteractiveOutput, CliError> {
    let api = format!(
        "interactive-harness-{}",
        INTERACTIVE_ID.fetch_add(1, Ordering::SeqCst)
    );
    registry::register(&api, provider);

    let chunks = input_chunks
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut input = ScriptedInput::from_chunks(chunks);
    let parsed = CliArgs::default();
    let session = session_dir
        .map(|dir| SessionRunOptions {
            mode: SessionMode::Enabled,
            cwd: dir.to_path_buf(),
            session_dir: Some(dir.to_path_buf()),
        })
        .unwrap_or_else(|| SessionRunOptions::disabled(PathBuf::from(".")));
    let options = CliRunOptions {
        model_override: Some(faux_model(&api)),
        tools: Vec::new(),
        register_builtins: false,
        session,
    };

    let result =
        run_interactive_loop(parsed, options, VirtualTerminal::new(80, 24), &mut input).await;
    registry::unregister(&api);

    Ok(scripted_output(result?, session_dir))
}
```

Finally, simplify the existing `run_scripted` helper to call the new generic helper:

```rust
async fn run_scripted(
    provider: FauxProvider,
    input: &str,
    session_dir: Option<&Path>,
) -> Result<ScriptedInteractiveOutput, CliError> {
    run_scripted_with_provider(Arc::new(provider), vec![input], session_dir).await
}
```

- [ ] **Step 3: Run the abort test again**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_abort ctrl_c_aborts_running_prompt_and_keeps_tui_open
```

Expected: FAIL by timing out at `interactive loop should not hang while aborting`, because the current loop still awaits `run_session_prompt` and never processes the second Ctrl+C chunk.

## Task 3: Add Abortable Session Prompt Runner

**Files:**
- Modify: `crates/pi-agent-core/src/agent.rs`
- Modify: `crates/pi-coding-agent/src/protocol/session_runner.rs`
- Modify: `crates/pi-coding-agent/Cargo.toml`
- Test: headless session and protocol tests

- [ ] **Step 1: Make `Agent` cloneable**

In `crates/pi-agent-core/src/agent.rs`, add this after the `pub struct Agent` definition:

```rust
impl Clone for Agent {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            running: Arc::clone(&self.running),
        }
    }
}
```

- [ ] **Step 2: Add abort handle and spawned task types**

In `crates/pi-coding-agent/src/protocol/session_runner.rs`, add:

```rust
use tokio::sync::{mpsc, oneshot};
```

Add these public types near `SessionPromptResult`:

```rust
#[derive(Clone)]
pub struct SessionPromptAbortHandle {
    agent: Agent,
}

impl SessionPromptAbortHandle {
    pub fn abort(&self) {
        self.agent.abort();
    }
}

pub struct SpawnedSessionPrompt {
    pub abort: SessionPromptAbortHandle,
    pub events: mpsc::UnboundedReceiver<AgentEvent>,
    pub done: oneshot::Receiver<Result<SessionPromptResult, CliError>>,
}
```

- [ ] **Step 3: Extract prompt setup into a prepared run**

In `crates/pi-coding-agent/src/protocol/session_runner.rs`, add this private struct:

```rust
struct PreparedSessionPrompt {
    agent: Agent,
    active_session: Option<ActiveSession>,
    existing_ids: HashSet<String>,
    session_name: Option<String>,
    invocation: PromptInvocation,
}
```

Extract the setup portion of `run_session_prompt` into a private `prepare_session_prompt(options: SessionPromptOptions) -> Result<PreparedSessionPrompt, CliError>` helper. The helper must:

- keep the existing `register_builtins` behavior;
- build `AgentConfig` with `build_agent_config`;
- enable `CompactionConfig::default()` when sessions are enabled;
- open and hydrate active sessions exactly as `run_session_prompt` does now;
- add all tools to the agent;
- move `options.invocation` into `PreparedSessionPrompt`.

- [ ] **Step 4: Extract prompt driving into a reusable async helper**

In `crates/pi-coding-agent/src/protocol/session_runner.rs`, add a private helper:

```rust
async fn drive_prepared_session_prompt(
    mut prepared: PreparedSessionPrompt,
    mut on_event: Option<&mut (dyn FnMut(&AgentEvent) -> Result<(), CliError> + Send)>,
) -> Result<SessionPromptResult, CliError> {
    let mut stream = match &prepared.invocation {
        PromptInvocation::Text(text) if !text.is_empty() => prepared.agent.prompt(text),
        PromptInvocation::Text(_) => return Err(CliError::MissingPrompt),
        PromptInvocation::Skill {
            name,
            additional_instructions,
        } => prepared
            .agent
            .skill(name, additional_instructions.as_deref())
            .map_err(CliError::AgentFailure)?,
        PromptInvocation::PromptTemplate { name, args } => prepared
            .agent
            .prompt_from_template(name, args)
            .map_err(CliError::AgentFailure)?,
    };

    let mut final_message: Option<AssistantMessage> = None;
    let mut pending_compactions = Vec::new();

    while let Some(event) = stream.next().await {
        if let Some(sink) = on_event.as_mut() {
            sink(&event)?;
        }

        match event {
            AgentEvent::AgentDone { message } => final_message = Some(message),
            AgentEvent::AgentError { error } => {
                capture_session_messages(
                    &prepared.agent,
                    &mut prepared.active_session,
                    &mut prepared.existing_ids,
                    &prepared.session_name,
                    &pending_compactions,
                )?;
                return Err(CliError::AgentFailure(error));
            }
            AgentEvent::SessionCompacted {
                summary,
                first_kept_message_id,
                tokens_before,
                details,
            } => pending_compactions.push(PendingCompaction {
                summary,
                first_kept_message_id,
                tokens_before,
                details,
            }),
            _ => {}
        }
    }

    let final_message = final_message.ok_or_else(|| {
        let _ = capture_session_messages(
            &prepared.agent,
            &mut prepared.active_session,
            &mut prepared.existing_ids,
            &prepared.session_name,
            &pending_compactions,
        );
        CliError::AgentFailure("agent stream ended without completion".to_string())
    })?;

    capture_session_messages(
        &prepared.agent,
        &mut prepared.active_session,
        &mut prepared.existing_ids,
        &prepared.session_name,
        &pending_compactions,
    )?;

    Ok(SessionPromptResult {
        final_message,
        messages: prepared.agent.messages(),
    })
}
```

Update `run_session_prompt` to call:

```rust
let prepared = prepare_session_prompt(options)?;
drive_prepared_session_prompt(prepared, on_event).await
```

- [ ] **Step 5: Add spawned session prompt function**

In `crates/pi-coding-agent/src/protocol/session_runner.rs`, add:

```rust
pub fn spawn_session_prompt(options: SessionPromptOptions) -> Result<SpawnedSessionPrompt, CliError> {
    let prepared = prepare_session_prompt(options)?;
    let abort = SessionPromptAbortHandle {
        agent: prepared.agent.clone(),
    };
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = oneshot::channel();

    tokio::spawn(async move {
        let mut on_event = |event: &AgentEvent| {
            let _ = event_tx.send(event.clone());
            Ok(())
        };
        let result = drive_prepared_session_prompt(prepared, Some(&mut on_event)).await;
        let _ = done_tx.send(result);
    });

    Ok(SpawnedSessionPrompt {
        abort,
        events: event_rx,
        done: done_rx,
    })
}
```

- [ ] **Step 6: Enable Tokio sync feature**

In `crates/pi-coding-agent/Cargo.toml`, ensure the Tokio dependency includes `sync`:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "process", "time", "io-util", "io-std", "sync"] }
```

- [ ] **Step 7: Run existing headless tests before changing the UI loop**

Run:

```bash
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test json_mode
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test session_cli
```

Expected: PASS. These tests prove the extraction did not alter existing headless behavior.

## Task 4: Refactor Interactive Loop to Process Input and Prompt Channels

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Test: `crates/pi-coding-agent/tests/interactive_abort.rs`
- Test: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Test: `crates/pi-coding-agent/tests/interactive_sessions.rs`

- [ ] **Step 1: Replace direct input reads with an input pump**

In `crates/pi-coding-agent/src/interactive/app.rs`, remove the `InputSource`, `StdinInput`, and `ScriptedInput` types after Task 2 has served its red-test purpose. Replace them with:

```rust
struct InputPump {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    _reader: Option<std::thread::JoinHandle<()>>,
}

impl InputPump {
    fn from_stdin() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let reader = std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            loop {
                let mut buffer = [0_u8; 1024];
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let chunk = String::from_utf8_lossy(&buffer[..count]).to_string();
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            rx,
            _reader: Some(reader),
        }
    }

    fn from_chunks(chunks: Vec<String>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for chunk in chunks {
            let _ = tx.send(chunk);
        }
        drop(tx);
        Self { rx, _reader: None }
    }

    async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await
    }
}
```

Update `run_interactive_mode` to create the pump:

```rust
let terminal = ProcessTerminal::new();
let mut input = InputPump::from_stdin();
match run_interactive_loop(parsed, options, terminal, &mut input).await {
    Ok(result) => CliOutput {
        exit_code: result.exit_code,
        stdout: String::new(),
        stderr: String::new(),
    },
    Err(error) => CliOutput {
        exit_code: 1,
        stdout: String::new(),
        stderr: format!("{error}\n"),
    },
}
```

Update `run_interactive_loop` and `run_started_interactive_loop` signatures:

```rust
async fn run_interactive_loop<T: Terminal>(
    parsed: CliArgs,
    options: CliRunOptions,
    mut terminal: T,
    input: &mut InputPump,
) -> Result<LoopResult<T>, CliError>
```

```rust
async fn run_started_interactive_loop<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    input: &mut InputPump,
    prompt_context: PromptContext,
) -> Result<i32, CliError>
```

Update scripted harness call sites to use `InputPump::from_chunks(...)` instead of `ScriptedInput`.

- [ ] **Step 2: Import the spawned session runner**

In `crates/pi-coding-agent/src/interactive/app.rs`, change:

```rust
use crate::protocol::session_runner::{SessionPromptOptions, run_session_prompt};
```

to:

```rust
use crate::protocol::session_runner::{
    SessionPromptAbortHandle, SessionPromptOptions, SessionPromptResult, SpawnedSessionPrompt,
    spawn_session_prompt,
};
```

- [ ] **Step 3: Add prompt task state**

In `crates/pi-coding-agent/src/interactive/app.rs`, add near `LoopResult`:

```rust
struct PromptTask {
    abort: SessionPromptAbortHandle,
    events: tokio::sync::mpsc::UnboundedReceiver<pi_agent_core::AgentEvent>,
    done: tokio::sync::oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    abort_requested: bool,
}

impl PromptTask {
    fn new(spawned: SpawnedSessionPrompt) -> Self {
        Self {
            abort: spawned.abort,
            events: spawned.events,
            done: spawned.done,
            abort_requested: false,
        }
    }

    fn abort_once(&mut self) {
        if !self.abort_requested {
            self.abort.abort();
            self.abort_requested = true;
        }
    }
}
```

- [ ] **Step 4: Add running abort action**

Update `InteractiveAction`:

```rust
enum InteractiveAction {
    None,
    Submit,
    AbortRunning,
    Exit,
}
```

Update `InteractiveRoot::handle_input` so Ctrl+C while running requests abort:

```rust
fn handle_input(&mut self, event: &InputEvent) {
    if matches_key(event, "ctrl+c") {
        match self.status {
            InteractiveStatus::Running => {
                self.action = InteractiveAction::AbortRunning;
                return;
            }
            InteractiveStatus::Idle => {
                if self.editor.text().is_empty() {
                    self.action = InteractiveAction::Exit;
                } else {
                    self.editor.set_text("");
                }
                return;
            }
        }
    }

    if self.status == InteractiveStatus::Idle {
        self.editor.handle_input(event);
        if let Some(prompt) = self.take_submitted() {
            self.pending_submit = Some(prompt);
            self.action = InteractiveAction::Submit;
        }
    }
}
```

- [ ] **Step 5: Replace synchronous submit with task spawning**

Remove the old `submit_prompt` function. Add:

```rust
fn start_prompt_task<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    prompt: String,
    prompt_context: &PromptContext,
) -> Result<PromptTask, CliError> {
    {
        let root = root_mut(tui, root_id)?;
        root.push_user(prompt.clone());
        root.set_status(InteractiveStatus::Running);
    }
    render_tui(tui)?;

    let options = SessionPromptOptions {
        prompt: prompt.clone(),
        model: prompt_context.model.clone(),
        api_key: prompt_context.api_key.clone(),
        system_prompt: prompt_context.system_prompt.clone(),
        max_turns: prompt_context.max_turns,
        tools: prompt_context.tools.clone(),
        register_builtins: prompt_context.register_builtins,
        session: prompt_context.session.clone(),
        session_target: prompt_context.session_target.clone(),
        session_name: prompt_context.session_name.clone(),
        thinking_level: prompt_context.thinking_level,
        tool_execution: prompt_context.tool_execution,
        resources: prompt_context.resources.clone(),
        invocation: PromptInvocation::Text(prompt),
    };

    spawn_session_prompt(options).map(PromptTask::new)
}
```

- [ ] **Step 6: Add agent event and finish helpers**

Add:

```rust
fn apply_agent_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    bridge: &mut InteractiveEventBridge,
    event: pi_agent_core::AgentEvent,
) -> Result<(), CliError> {
    let ui_events = bridge.handle(&event);
    let root = root_mut(tui, root_id)?;
    root.apply_events(ui_events);
    Ok(())
}

fn finish_prompt<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    result: Result<SessionPromptResult, CliError>,
) -> Result<(), CliError> {
    let root = root_mut(tui, root_id)?;
    if let Err(error) = result {
        root.apply_events(vec![UiEvent::AgentError {
            error: error.to_string(),
        }]);
    }
    root.set_status(InteractiveStatus::Idle);
    Ok(())
}
```

- [ ] **Step 7: Add loop control helper**

Add:

```rust
enum LoopControl {
    Continue,
    Exit,
}

fn handle_input_event<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    event: InputEvent,
    prompt_context: &PromptContext,
    running: &mut Option<PromptTask>,
) -> Result<LoopControl, CliError> {
    let (action, prompt) = {
        let root = root_mut(tui, root_id)?;
        root.handle_input(&event);
        let action = root.take_action();
        let prompt = if action == InteractiveAction::Submit {
            root.take_pending_submit()
        } else {
            None
        };
        (action, prompt)
    };

    match action {
        InteractiveAction::None => {
            render_tui(tui)?;
            Ok(LoopControl::Continue)
        }
        InteractiveAction::Exit => Ok(LoopControl::Exit),
        InteractiveAction::AbortRunning => {
            if let Some(task) = running.as_mut() {
                task.abort_once();
            }
            render_tui(tui)?;
            Ok(LoopControl::Continue)
        }
        InteractiveAction::Submit => {
            if running.is_some() {
                render_tui(tui)?;
                return Ok(LoopControl::Continue);
            }
            let Some(prompt) = prompt else {
                render_tui(tui)?;
                return Ok(LoopControl::Continue);
            };
            if prompt.trim().is_empty() {
                render_tui(tui)?;
                return Ok(LoopControl::Continue);
            }
            *running = Some(start_prompt_task(tui, root_id, prompt, prompt_context)?);
            Ok(LoopControl::Continue)
        }
    }
}
```

- [ ] **Step 8: Rewrite `run_started_interactive_loop`**

Replace `run_started_interactive_loop` with:

```rust
async fn run_started_interactive_loop<T: Terminal>(
    tui: &mut Tui<T>,
    root_id: usize,
    input: &mut InputPump,
    prompt_context: PromptContext,
) -> Result<i32, CliError> {
    let mut stdin_buffer = StdinBuffer::new();
    let mut running: Option<PromptTask> = None;
    let mut bridge = InteractiveEventBridge::new();
    render_tui(tui)?;

    loop {
        if let Some(mut task) = running.take() {
            tokio::select! {
                chunk = input.recv() => {
                    let Some(chunk) = chunk else {
                        running = Some(task);
                        continue;
                    };
                    running = Some(task);
                    for event in stdin_buffer.process(&chunk) {
                        match handle_input_event(tui, root_id, event, &prompt_context, &mut running)? {
                            LoopControl::Continue => {}
                            LoopControl::Exit => return Ok(0),
                        }
                    }
                }
                maybe_event = task.events.recv() => {
                    if let Some(event) = maybe_event {
                        apply_agent_event(tui, root_id, &mut bridge, event)?;
                        render_tui(tui)?;
                    }
                    running = Some(task);
                }
                done = &mut task.done => {
                    let result = done.unwrap_or_else(|_| {
                        Err(CliError::AgentFailure(
                            "prompt task dropped before completion".to_string(),
                        ))
                    });
                    finish_prompt(tui, root_id, result)?;
                    render_tui(tui)?;
                    running = None;
                }
            }
        } else {
            let Some(chunk) = input.recv().await else {
                return Ok(0);
            };
            for event in stdin_buffer.process(&chunk) {
                match handle_input_event(tui, root_id, event, &prompt_context, &mut running)? {
                    LoopControl::Continue => {}
                    LoopControl::Exit => return Ok(0),
                }
            }
        }
    }
}
```

If the compiler reports that `task` was moved while borrowed, keep the same state machine but split the running branch into smaller helper functions. Preserve this behavior:

- input chunks can be processed while `running` is `Some`;
- agent events are applied as they arrive;
- done sets status idle and clears `running`;
- abort does not exit the loop.

- [ ] **Step 9: Run the abort test**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_abort ctrl_c_aborts_running_prompt_and_keeps_tui_open
```

Expected: PASS.

## Task 5: Regression Tests for Existing Interactive Behavior

**Files:**
- Test: `crates/pi-coding-agent/tests/interactive_abort.rs`
- Test: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Test: `crates/pi-coding-agent/tests/interactive_sessions.rs`

- [ ] **Step 1: Run focused interactive tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_abort
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_sessions
```

Expected: PASS.

- [ ] **Step 2: Run full coding-agent tests**

Run:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS.

## Task 6: Spec Compliance Review

**Files:**
- Review: `docs/superpowers/specs/2026-06-11-pi-coding-agent-interactive-async-abort-design.md`
- Review: changed Rust files

- [ ] **Step 1: Check behavior requirements manually**

Confirm each item is true:

- The prompt runner is spawned instead of awaited inside input handling.
- Agent events are sent over a channel and applied by the UI loop.
- Ctrl+C while running calls `SessionPromptAbortHandle::abort()`.
- Ctrl+C while idle with an empty editor exits.
- Ctrl+C while idle with text clears the editor.
- Non-TTY route still returns `interactive mode requires a TTY`.
- Print/json/rpc tests still pass.

- [ ] **Step 2: Search for accidental behavior leaks**

Run:

```bash
rg -n "run_session_prompt\\(|spawn_session_prompt|AbortRunning|abort_once|interactive mode requires a TTY" crates/pi-coding-agent/src crates/pi-agent-core/src
```

Expected:

- `run_session_prompt` remains available for existing headless modes.
- `spawn_session_prompt` is used by interactive mode only.
- `AbortRunning` and `abort_once` are local to interactive app state.
- The non-TTY error string remains unchanged.

## Task 7: Code Quality Review

**Files:**
- Review: `crates/pi-coding-agent/src/interactive/app.rs`
- Review: `crates/pi-coding-agent/src/protocol/session_runner.rs`
- Review: `crates/pi-agent-core/src/agent.rs`

- [ ] **Step 1: Check ownership boundaries**

Confirm:

- `Transcript` and `Tui` are mutated only from the interactive loop.
- The background task sends `AgentEvent` values and completion result only.
- `SessionPromptAbortHandle` does not expose `Agent` directly.
- `spawn_session_prompt` does not change `run_session_prompt` public behavior.

- [ ] **Step 2: Check channel termination paths**

Confirm:

- If event receiver is closed, the prompt task still completes and sends `done`.
- If done sender is dropped, the UI reports `prompt task dropped before completion`.
- If abort is requested twice, `Agent::abort()` is called only once through `abort_once`.
- Terminal stop still runs after loop errors.

## Task 8: Final Verification

**Files:**
- No required file changes unless verification exposes bugs.

- [ ] **Step 1: Format check**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Focused crate tests**

Run:

```bash
cargo test -p pi-tui
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 3: Workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS. Existing warning output from unrelated test files is acceptable if exit code is 0.

- [ ] **Step 4: Workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Whitespace check**

Run:

```bash
git diff --check
```

Expected: PASS.

## Implementation Notes for Subagents

- Work from `pi-rust/`, not the TypeScript repo.
- Do not modify `pi/`.
- Do not revert the existing uncommitted M6 files.
- Use TDD: add the abort test first and observe it fail before production changes.
- Keep this slice narrow. Do not implement scrolling, Markdown rendering, component split, slash commands, or themes in this plan.
- Do not commit unless the user explicitly asks for a commit.
