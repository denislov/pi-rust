# Codebase Concerns

**Analysis Date:** 2026-07-10

## Tech Debt

**Oversized orchestration and UI modules:**
- Issue: Several production files combine state ownership, protocol adaptation, rendering, command dispatch, and tests in single modules. The largest hotspots are about 5,073 lines in `crates/pi-coding-agent/src/interactive/app.rs`, 4,818 lines in `crates/pi-coding-agent/src/coding_session/mod.rs`, 2,715 lines in `crates/pi-coding-agent/src/interactive/root.rs`, 2,642 lines in `crates/pi-coding-agent/src/coding_session/session_service.rs`, 2,468 lines in `crates/pi-coding-agent/src/interactive/loop.rs`, and 2,409 lines in `crates/pi-coding-agent/src/coding_session/event_service.rs`.
- Files: `crates/pi-coding-agent/src/interactive/app.rs`, `crates/pi-coding-agent/src/coding_session/mod.rs`, `crates/pi-coding-agent/src/interactive/root.rs`, `crates/pi-coding-agent/src/coding_session/session_service.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Impact: Changes have a broad blast radius, reviewers must reason across unrelated responsibilities, and source-scanning boundary tests become substitutes for narrower type/module boundaries.
- Fix approach: Continue extracting stateful services and adapter-specific projections behind the existing operation, snapshot, product-event, and service boundaries. Split only along ownership boundaries already described in `docs/TODO.md` and `docs/superpowers/specs/2026-07-07-operation-runtime-reference-architecture.md`, with focused behavior tests before each move.

**Operation-runtime migration is incomplete:**
- Issue: The canonical operation dispatcher exists, but first-party adapter migration and deletion of broad compatibility methods remain active work. Two top-level checklist items are still marked `[~]`.
- Files: `docs/TODO.md`, `crates/pi-coding-agent/src/coding_session/mod.rs`, `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`, `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- Impact: New code can accidentally use deprecated workflow-specific entrypoints or compatibility event surfaces, extending the period where two public/runtime models must remain coherent.
- Fix approach: Complete Stage 9 Tasks 4-8 from `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md`, migrate adapters to `CodingAgentSession::run`, then remove compatibility methods and their warning suppressions in one guarded change.

**Source-scanning tests encode architecture textually:**
- Issue: Many architectural invariants are enforced by tests that read Rust source and search for forbidden strings or relative ordering instead of compiler-visible ownership rules.
- Files: `crates/pi-coding-agent/tests/api_boundary_guards.rs`, `crates/pi-coding-agent/tests/event_boundary_guards.rs`, `crates/pi-coding-agent/tests/session_boundary_guards.rs`, `crates/pi-agent-core/tests/api_boundary_guards.rs`, `crates/pi-tui/tests/api_boundary_guards.rs`
- Impact: Harmless formatting, renames, or comments can break tests, while semantically equivalent violations can evade string matching.
- Fix approach: Preserve guards during migration, but replace each guard with visibility restrictions, sealed traits, narrow facades, or compile-fail/API tests whenever the type system can express the rule.

**Placeholder workspace members:**
- Issue: Three crates and the root binary still contain Cargo-template `add`/`Hello, world!` implementations rather than domain behavior.
- Files: `crates/pi-web-ui/src/lib.rs`, `crates/pi-mom/src/lib.rs`, `crates/pi-pods/src/lib.rs`, `src/main.rs`
- Impact: Workspace builds imply capabilities that do not exist, crate names can mislead downstream planning, and placeholder tests provide no product confidence.
- Fix approach: Either remove these members until their milestones begin or replace them with explicit crate-level documentation and failing/ignored contract tests that state the intended boundary without pretending the feature is implemented.

## Known Bugs

**A malformed final session-log record makes the whole session unreadable:**
- Symptoms: Session open/replay fails at the first malformed JSONL line instead of recovering all complete preceding records.
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`, `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Trigger: Truncate or partially write the final line of `events.jsonl`, including a process or machine failure between `write_all` and durable storage. `read_events` uses `fs::read_to_string` and returns an error on any line that `serde_json::from_str` cannot parse.
- Workaround: Back up the session directory and remove only the incomplete trailing record before reopening. Do not discard earlier valid records.

**Manifest replacement can leave invalid JSON after interruption:**
- Symptoms: A session cannot be opened because `session.json` is empty, truncated, or malformed.
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/manifest.rs`
- Trigger: Interrupt `write_manifest`, which serializes and overwrites the live manifest with `fs::write` rather than writing a temporary file and atomically renaming it.
- Workaround: Restore `session.json` from a backup or reconstruct it from the session directory and event log; semantic startup recovery does not repair a syntactically damaged manifest.

## Security Considerations

**Agent tools are host-level, not workspace-sandboxed:**
- Risk: Tool arguments may target absolute paths, `..` paths, home-directory paths, or arbitrary shell commands. `resolve_to_cwd` intentionally preserves absolute paths, and the bash tool invokes `bash -c` with the selected command.
- Files: `crates/pi-coding-agent/src/tools/path.rs`, `crates/pi-coding-agent/src/tools/bash.rs`, `crates/pi-coding-agent/src/tools/read.rs`, `crates/pi-coding-agent/src/tools/write.rs`, `crates/pi-coding-agent/src/tools/edit.rs`
- Current mitigation: Filesystem and shell access are represented by operation capability snapshots; bash clears the environment and forwards only a small allowlist, applies a timeout, truncates output, and kills the Unix process group on timeout.
- Recommendations: Treat local model/tool access as equivalent to local code execution. Add an opt-in workspace confinement policy using canonicalized paths and symlink-aware containment checks, plus user approval/policy hooks for commands and writes outside the workspace.

**Lua plugins can consume unbounded CPU or memory:**
- Risk: A plugin entrypoint, tool, command, or hook can run an infinite loop or allocate until process exhaustion. Lua execution occurs synchronously, and no instruction hook or memory limit is installed.
- Files: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`, `crates/pi-coding-agent/Cargo.toml`, `docs/lua-plugin-host.md`
- Current mitigation: `create_lua` exposes only table, string, math, and UTF-8 standard libraries; file, process, network, and raw session capabilities are not exposed through the host API.
- Recommendations: Add instruction-count/deadline cancellation and a memory ceiling for every registration and invocation. Run untrusted plugins outside the main interactive/runtime thread, and make plugin trust explicit at discovery time.

**Credentials are plaintext and Unix-only permission hardening is incomplete:**
- Risk: Provider API keys and OAuth tokens are serialized directly into `auth.toml`; on non-Unix platforms no equivalent ACL hardening is applied, and an existing loose Unix mode only produces a warning.
- Files: `crates/pi-coding-agent/src/config/auth.rs`, `crates/pi-coding-agent/src/config/paths.rs`, `crates/pi-coding-agent/src/interactive/app.rs`
- Current mitigation: New Unix writes are changed to mode `0600`, loose Unix permissions emit diagnostics, and raw keys are resolved through the auth store rather than intentionally logged.
- Recommendations: Prefer environment/keychain references, use create-with-restrictive-permissions rather than write-then-chmod, add Windows ACL handling, and add explicit redaction tests for every diagnostic/protocol path that can carry provider errors.

## Performance Bottlenecks

**Full assistant-message cloning on streaming deltas:**
- Problem: Provider processors clone the accumulated `AssistantMessage` into most `TextDelta`, `ThinkingDelta`, and `ToolcallDelta` events. As content grows, later deltas copy all earlier content.
- Files: `crates/pi-ai/src/providers/anthropic/process.rs`, `crates/pi-ai/src/providers/openai/responses/process.rs`, `crates/pi-ai/src/providers/openai/completions/process.rs`, `crates/pi-ai/src/providers/google/process.rs`, `crates/pi-ai/src/providers/mistral/process.rs`, `crates/pi-ai/src/providers/deepseek/process.rs`, `crates/pi-ai/src/providers/bedrock/process.rs`
- Cause: `AssistantMessageEvent` carries both the incremental delta and a complete owned `partial` snapshot.
- Improvement path: Make delta events lightweight and keep the authoritative partial in one accumulator, or use shared immutable storage/copy-on-write snapshots. Add long-stream allocation and latency benchmarks before changing the public event contract.

**Session replay reparses the complete event log:**
- Problem: Replay reads `events.jsonl` into one `String`, parses every line into a vector, then folds the complete event set.
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`, `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Cause: There is no incremental reader, index checkpoint, compacted snapshot, or bounded replay window for persisted sessions.
- Improvement path: Stream records through a buffered reader, tolerate/recover a torn tail, and introduce versioned replay checkpoints or compaction snapshots once large-session measurements justify the added format complexity.

**Lua source is recompiled for every invocation:**
- Problem: Plugin source is loaded and executed during discovery and again for each tool, command, or hook invocation.
- Files: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`
- Cause: Each call creates a new `Lua` instance, executes the complete plugin source, calls `register(host)`, and extracts one matching callback.
- Improvement path: Cache a validated compiled chunk or use a supervised per-plugin runtime with bounded resources and explicit state/reset semantics.

## Fragile Areas

**Session persistence and recovery ordering:**
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`, `crates/pi-coding-agent/src/coding_session/session_service.rs`, `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`
- Why fragile: Event append, manifest mutation, session-copy publication, and cleanup are separate filesystem operations. The code reports `PartialCommit` and writes semantic recovery markers, but it does not make the underlying multi-file update atomic or durable with `sync_all`.
- Safe modification: Preserve append-before-manifest ordering, add failure tests at every boundary, use temporary files plus atomic rename for manifests, explicitly sync files/directories where durability is promised, and keep replay authoritative when manifest updates fail.
- Test coverage: Failure-injection tests cover several logical error points, but no test simulates a torn JSONL record, truncated manifest, power-loss durability, or filesystem behavior where rename/fsync semantics differ.

**Interactive application state machine:**
- Files: `crates/pi-coding-agent/src/interactive/app.rs`, `crates/pi-coding-agent/src/interactive/loop.rs`, `crates/pi-coding-agent/src/interactive/root.rs`, `crates/pi-coding-agent/src/interactive/event_bridge.rs`, `crates/pi-coding-agent/src/interactive/render.rs`
- Why fragile: UI state, pending operations, session transitions, delegation, plugin actions, and rendering are spread across very large modules with many queues and take/reset methods.
- Safe modification: Start from a behavior test in `crates/pi-coding-agent/tests/interactive_mode.rs` or `crates/pi-coding-agent/tests/interactive_sessions.rs`; preserve the snapshot/product-event projection boundary and keep adapter code independent of internal Flow node IDs.
- Test coverage: Automated integration coverage is broad, but real terminal behavior still depends on the opt-in tmux workflow in `scripts/tui-smoke.sh`; platform terminals and unusual escape-sequence timing remain difficult to reproduce in normal Cargo tests.

**Terminal lifecycle and Windows FFI:**
- Files: `crates/pi-tui/src/terminal.rs`, `crates/pi-tui/tests/terminal_lifecycle.rs`, `crates/pi-tui/tests/terminal.rs`
- Why fragile: Raw mode, keyboard protocol negotiation, bracketed paste, progress threads, cursor state, and Windows console mode restoration must unwind correctly across partial startup and I/O failures.
- Safe modification: Keep cleanup idempotent, never allow an escape-sequence write failure to skip raw-mode restoration, and test startup failure after each state transition.
- Test coverage: Virtual-terminal lifecycle behavior is tested, but native Windows `GetStdHandle`/`GetConsoleMode`/`SetConsoleMode` calls and real terminal negotiation are not exercised by the normal cross-platform unit suite.

## Scaling Limits

**Product-event replay window:**
- Current capacity: Both the session broadcast channel and retained replay buffer default to 128 product events; the RPC forwarding queue also defaults to 128.
- Limit: A slow or disconnected client can fall behind during a high-rate model/tool stream and must rebuild from a fresh snapshot once its cursor predates retained history.
- Scaling path: Keep the existing gap-to-snapshot contract, expose lag metrics, make capacities configurable for deployment modes, and reduce event volume by avoiding full-partial snapshots on every delta.
- Files: `crates/pi-coding-agent/src/coding_session/event_service.rs`, `crates/pi-coding-agent/src/protocol/rpc/event_queue.rs`, `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`

**Persistent session size:**
- Current capacity: No explicit record-count or byte-size ceiling is enforced for `events.jsonl`; replay retains parsed events and reconstructed transcript state in memory.
- Limit: Startup/open latency and memory grow with complete session history, and a single malformed record blocks replay.
- Scaling path: Add observable session-size thresholds, streaming replay, durable compaction/checkpoints, and recovery of a single torn trailing record before supporting very long-lived sessions.
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/replay.rs`, `crates/pi-coding-agent/src/coding_session/manual_compaction_service.rs`

## Dependencies at Risk

**Workspace dependency supply chain:**
- Risk: The repository has a lockfile but no checked-in CI pipeline, `cargo audit`, `cargo deny`, license policy, or automated advisory gate. Native/runtime-sensitive dependencies include vendored Lua through `mlua`, cryptography through `ring`, terminal control through `crossterm`, filesystem watching through `notify`, and HTTP/TLS through `reqwest`/rustls.
- Impact: Vulnerable or incompatible transitive versions can remain unnoticed until a manual update or platform failure.
- Migration plan: Add a CI gate for `cargo test --workspace`, `cargo fmt --check`, `cargo check --workspace`, `cargo audit`, and `cargo deny check`; review lockfile changes explicitly and test Linux, macOS, and Windows for terminal/process/filesystem-sensitive crates.
- Files: `Cargo.toml`, `Cargo.lock`, `crates/pi-coding-agent/Cargo.toml`, `crates/pi-ai/Cargo.toml`, `crates/pi-tui/Cargo.toml`

## Missing Critical Features

**Web UI, pod runtime, and MoM functionality are not implemented:**
- Problem: The named crates expose only the template `add` function and template unit test.
- Blocks: Any plan that assumes a functional web interface, pod/container orchestration, or MoM subsystem cannot build on current code.
- Files: `crates/pi-web-ui/src/lib.rs`, `crates/pi-pods/src/lib.rs`, `crates/pi-mom/src/lib.rs`

**Root executable is not a product entrypoint:**
- Problem: The workspace root binary only prints `Hello, world!`; the usable coding-agent entrypoint lives in the `pi-coding-agent` crate.
- Blocks: Installing or running the root `pi-rust` package does not launch the actual application and can confuse packaging/release automation.
- Files: `src/main.rs`, `Cargo.toml`, `crates/pi-coding-agent/src/bin/pi-coding-agent.rs`

**Canonical operation convergence is not closed:**
- Problem: First-party adapters still need to complete migration to the canonical public operation surface, and broad compatibility methods remain pending deletion.
- Blocks: A single stable embedder/runtime contract and the next typed ProductEvent payload stage remain incomplete.
- Files: `docs/TODO.md`, `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md`, `crates/pi-coding-agent/src/coding_session/public_operation.rs`, `crates/pi-coding-agent/src/coding_session/mod.rs`

## Test Coverage Gaps

**Crash-consistency and corrupt-session recovery:**
- What's not tested: Torn final JSONL writes, truncated/empty manifests, power loss after flush but before durable sync, and cross-filesystem atomicity assumptions.
- Files: `crates/pi-coding-agent/src/coding_session/session_log/store.rs`, `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`, `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Risk: A session can become unreadable or ambiguously committed despite logical failure-injection tests passing.
- Priority: High

**Long provider-stream performance:**
- What's not tested: Allocation growth, clone volume, and end-to-end latency for thousands of small text/thinking/tool deltas carrying complete partial-message snapshots.
- Files: `crates/pi-ai/src/providers/anthropic/process.rs`, `crates/pi-ai/src/providers/openai/responses/process.rs`, `crates/pi-ai/src/providers/google/process.rs`, `crates/pi-ai/src/providers/bedrock/process.rs`
- Risk: Large model responses can show quadratic copying behavior without a functional test failing.
- Priority: High

**Runaway Lua plugin containment:**
- What's not tested: Infinite loops, excessive allocation, cancellation, deadlines, panics/errors during registration, and isolation from the interactive event loop.
- Files: `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`, `crates/pi-coding-agent/tests/plugin_ui_boundary_guards.rs`, `crates/pi-coding-agent/tests/runtime.rs`
- Risk: One untrusted or defective plugin can hang or exhaust the whole process.
- Priority: High

**Platform-specific host boundaries:**
- What's not tested: Native Windows terminal mode restoration, Windows credential ACLs, and Windows process-tree termination under timeout.
- Files: `crates/pi-tui/src/terminal.rs`, `crates/pi-coding-agent/src/config/auth.rs`, `crates/pi-coding-agent/src/tools/bash.rs`
- Risk: Security and cleanup behavior can regress on a platform not represented by the local test environment.
- Priority: Medium

**No enforced coverage or CI baseline:**
- What's not tested: There is no numeric line/branch coverage threshold and no checked-in CI workflow guaranteeing the documented workspace gates run on every change.
- Files: `Cargo.toml`, `docs/TODO.md`, `scripts/tui-smoke.sh`
- Risk: Broad local suites can still omit newly added branches or platform configurations, and regressions depend on contributors running the full command set manually.
- Priority: Medium

**Placeholder crates:**
- What's not tested: No domain behavior, public contract, integration path, or negative capability assertion exists for the future web UI, pod, and MoM crates.
- Files: `crates/pi-web-ui/src/lib.rs`, `crates/pi-pods/src/lib.rs`, `crates/pi-mom/src/lib.rs`
- Risk: Template tests pass while the named subsystems remain entirely absent.
- Priority: Low until those milestones become active; High before advertising or depending on those crates.

---

*Concerns audit: 2026-07-10*
