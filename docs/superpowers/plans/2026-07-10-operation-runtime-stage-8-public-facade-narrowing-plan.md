# Operation Runtime Stage 8 Public Facade Narrowing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire replaced compatibility surfaces so `pi-coding-agent` exposes one stable operation runtime facade, one snapshot/reconnect facade, one product event stream, and one Rust-native session fact source.

**Architecture:** Build on Stages 1-7 without changing adapter-visible behavior in a single broad cut. Introduce stable public facade types for operation execution, snapshots, and product-event subscription first; migrate first-party adapters/tests onto those surfaces; then deprecate and delete compatibility entrypoints with source guards that prevent fallback paths from returning.

**Tech Stack:** Rust 2024, `pi-coding-agent`, crate-internal operation runtime, public `pi_coding_agent::api` facade, `UiSnapshot`, retained `ProductEvent`s, RPC/interactive adapter guards, deterministic offline tests.

---

## Current Context

Stage 8 starts after the operation runtime reference architecture has completed these prerequisites:

- `CodingAgentSession` owns operation admission through `IntentRouter`.
- Stage 2 introduced internal `ProductEvent` alongside compatibility `CodingAgentEvent`.
- Stage 6 introduced `UiSnapshot`, `ClientConnection`, retained product-event replay, and adapter projection from snapshots plus product events.
- Stage 7 bounded product-event forwarding, added protocol-family negotiation, and made recovery markers visible through snapshot/product-event paths.
- Before this plan was added, `docs/TODO.md` recorded "Stage 8 public facade narrowing/deletion remains future work under the same reference architecture."

Important current compatibility surfaces:

- `crates/pi-coding-agent/src/lib.rs` keeps root modules public but `#[doc(hidden)]`, while `pi_coding_agent::api` is the stated stable facade.
- `pi_coding_agent::api` currently exports `CodingAgentEvent`, `CodingAgentEventReceiver`, and broad `CodingAgentSession` methods.
- `CodingAgentSession::subscribe()` exposes compatibility `CodingAgentEventReceiver`; first-party adapters should prefer product-event projection.
- `CodingAgentSession` has many public workflow methods (`prompt`, `compact`, `summarize_branch`, `invoke_agent`, `invoke_team`, `self_healing_edit_with_options`, export helpers), while the internal owner already routes through `Operation`.
- Crate-internal snapshot helpers (`ui_snapshot`, `connect_client`, `product_events_after`) are the architectural direction but not yet a stable public embedding facade.

## File Structure

- Create: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
  - Stable public operation request/outcome enums that wrap existing option/outcome types without exposing internal `Operation`.
- Create: `crates/pi-coding-agent/src/coding_session/public_projection.rs`
  - Stable public snapshot, client cursor, retained-event replay, and product-event subscription facade.
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Add public `run`, `snapshot`, `connect`, `product_events_after_public`, and `subscribe_product_events_public` facade methods; deprecate compatibility methods after migration.
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
  - Add public product-event facade conversion helpers without exposing storage fields or Flow node internals.
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
  - Add public product-event receiver wrapper and keep compatibility receiver behind a deprecated compatibility boundary.
- Modify: `crates/pi-coding-agent/src/coding_session/client_projection.rs`
  - Keep internal `UiSnapshot` as the owner projection model and convert it into public snapshot types.
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Keep internal `Operation` crate-private; add conversion from public operation requests in `public_operation.rs`.
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Re-export only the new public facade types from the coding-session boundary.
- Modify: `crates/pi-coding-agent/src/lib.rs`
  - Narrow `pi_coding_agent::api` exports to the stable facade groups and mark root compatibility re-exports as deprecated or migration-private.
- Modify: `crates/pi-coding-agent/src/protocol/rpc/*`
  - Ensure RPC consumes public or crate-internal snapshot/product-event facades, not compatibility event receivers.
- Modify: `crates/pi-coding-agent/src/interactive/*`
  - Ensure interactive projection and tests consume snapshot/product-event facades, not compatibility subscriptions.
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
  - Replace broad importability smoke tests with explicit stable facade contract tests.
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`
  - Guard root compatibility exports, deprecated compatibility event receiver, and stable facade export groups.
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
  - Guard adapter projection against `CodingAgentEventReceiver` and compatibility `.subscribe()` usage.
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
  - Guard public operation execution against bypassing `CodingAgentSession::run`.
- Modify: `docs/TODO.md`
  - Track Stage 8 plan start and closure.

## Non-Goals

- Do not remove TypeScript reference code or add TypeScript session JSONL import/export.
- Do not expose internal `Operation`, `OperationAdmission`, `OperationPermit`, `EventService`, `SessionService`, or `FlowService` as public API.
- Do not make RPC `hello` mandatory in this stage.
- Do not change RPC/interactive wire behavior except where tests explicitly cover a stable replacement.
- Do not delete public compatibility methods before first-party adapters and tests have moved to the replacement facade.

## Task 1: Public Facade Inventory And Guard Rails

**Files:**
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`

- [x] **Step 1: Add failing root compatibility export guard**

Add this test to `crates/pi-coding-agent/tests/api_boundary_guards.rs`:

```rust
#[test]
fn root_reexports_are_explicit_compatibility_surface() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let before_api = lib_source
        .split("pub mod api {")
        .next()
        .expect("api module should exist");

    let mut violations = Vec::new();
    for (index, line) in before_api.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub use ") {
            continue;
        }
        let before_api_lines = before_api.lines().collect::<Vec<_>>();
        let previous_non_empty = before_api_lines[..index]
            .iter()
            .rev()
            .find(|candidate| !candidate.trim().is_empty())
            .map(|candidate| candidate.trim());
        if previous_non_empty != Some("#[deprecated(note = \"use pi_coding_agent::api instead\")]") {
            violations.push(format!("{}: {}", index + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "root reexports should be explicitly deprecated compatibility surface; stable users should import pi_coding_agent::api:\n{}",
        violations.join("\n")
    );
}
```

- [x] **Step 2: Run RED guard**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards root_reexports_are_explicit_compatibility_surface -- --nocapture
```

Expected: FAIL because root `pub use` lines are not yet marked deprecated.

- [x] **Step 3: Mark root re-exports as compatibility surface**

In `crates/pi-coding-agent/src/lib.rs`, add this attribute before each root-level `pub use` that appears before `pub mod api`:

```rust
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use args::{CliArgs, CliMode, help_text, parse_args};
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use error::CliError;
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use print_mode::{PrintModeOptions, run_print_mode};
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use prompt_options::PromptRunOptions;
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use runtime::{
    CliRunOptions, DEFAULT_MODEL_ID, DEFAULT_SYSTEM_PROMPT, PromptInvocation, SessionMode,
    SessionRunOptions, build_agent_config, effective_no_context_files, effective_session_dir,
    select_model,
};
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use session::{ResolvedSessionTarget, encode_cwd};
#[deprecated(note = "use pi_coding_agent::api instead")]
pub use tools::builtin_tools;
```

Keep the `api` module itself non-deprecated.

- [x] **Step 4: Update public API smoke imports**

In `crates/pi-coding-agent/tests/public_api.rs`, keep importing from `pi_coding_agent::api`. Add a small regression check proving root compatibility is not used by the test:

```rust
#[test]
fn public_api_tests_use_stable_facade_imports() {
    let source = include_str!("public_api.rs");
    assert!(
        !source.contains("use pi_coding_agent::{"),
        "public API tests should import stable symbols through pi_coding_agent::api"
    );
}
```

- [x] **Step 5: Run GREEN guard and public API tests**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards root_reexports_are_explicit_compatibility_surface -- --nocapture
cargo test -p pi-coding-agent --test public_api public_api_tests_use_stable_facade_imports -- --nocapture
```

Expected: both tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/api_boundary_guards.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "chore: mark root coding-agent facade compatibility"
```

## Task 2: Stable Public Operation Facade

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/public_operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Write failing public operation facade test**

Add imports to `crates/pi-coding-agent/tests/public_api.rs`:

```rust
use pi_coding_agent::api::{CodingAgentOperation, CodingAgentOperationOutcome};
```

Add this test:

```rust
#[tokio::test]
async fn coding_session_run_public_operation_facade_is_importable() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_public_run")
            .with_session_log_root(temp.path()),
    )
        .await
        .unwrap();
    let outcome = session
        .run(CodingAgentOperation::ExportCurrent)
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        CodingAgentOperationOutcome::Export(_)
    ));
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent --test public_api coding_session_run_public_operation_facade_is_importable -- --nocapture
```

Expected: FAIL because `CodingAgentOperation`, `CodingAgentOperationOutcome`, and `CodingAgentSession::run` do not exist.

- [x] **Step 3: Add public operation facade types**

Create `crates/pi-coding-agent/src/coding_session/public_operation.rs`:

```rust
use std::path::PathBuf;

use super::agent_invocation_flow::{AgentInvocationOptions, AgentInvocationOutcome};
use super::agent_team_flow::{AgentTeamOptions, AgentTeamOutcome};
use super::export::CodingAgentSessionExport;
use super::prompt::{PromptTurnOptions, PromptTurnOutcome};
use super::self_healing_edit_flow::{SelfHealingEditOutcome, SelfHealingEditRequest};

#[derive(Debug)]
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}

#[derive(Debug)]
pub enum CodingAgentOperationOutcome {
    Prompt(PromptTurnOutcome),
    Compact(PromptTurnOutcome),
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}
```

- [x] **Step 4: Wire module exports**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, add the module:

```rust
mod public_operation;
```

Add the public re-export:

```rust
pub use public_operation::{CodingAgentOperation, CodingAgentOperationOutcome};
```

In `crates/pi-coding-agent/src/lib.rs`, add these to `pub mod api`:

```rust
CodingAgentOperation, CodingAgentOperationOutcome,
```

- [x] **Step 5: Implement `CodingAgentSession::run`**

Add this method to the public `impl CodingAgentSession` in `crates/pi-coding-agent/src/coding_session/mod.rs`:

```rust
pub async fn run(
    &mut self,
    operation: CodingAgentOperation,
) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
    match operation {
        CodingAgentOperation::Prompt(options) => self
            .prompt(options)
            .await
            .map(CodingAgentOperationOutcome::Prompt),
        CodingAgentOperation::Compact(options) => self
            .compact(options)
            .await
            .map(CodingAgentOperationOutcome::Compact),
        CodingAgentOperation::BranchSummary {
            options,
            source_leaf_id,
            target_leaf_id,
            custom_instructions,
        } => self
            .summarize_branch(options, source_leaf_id, target_leaf_id, custom_instructions)
            .await
            .map(CodingAgentOperationOutcome::BranchSummary),
        CodingAgentOperation::SelfHealingEdit(request) => self
            .self_healing_edit_with_options(request)
            .await
            .map(CodingAgentOperationOutcome::SelfHealingEdit),
        CodingAgentOperation::InvokeAgent(options) => self
            .invoke_agent(options)
            .await
            .map(CodingAgentOperationOutcome::AgentInvocation),
        CodingAgentOperation::InvokeTeam(options) => self
            .invoke_team(options)
            .await
            .map(CodingAgentOperationOutcome::AgentTeam),
        CodingAgentOperation::ExportCurrent => self
            .export_current()
            .map(CodingAgentOperationOutcome::Export),
        CodingAgentOperation::ExportCurrentHtml(path) => self
            .export_current_html(path)
            .map(CodingAgentOperationOutcome::ExportHtml),
    }
}
```

This first implementation intentionally delegates to existing public methods so behavior remains unchanged. Later tasks move callers to `run` and deprecate broad methods.

- [x] **Step 6: Run GREEN test**

Run:

```bash
cargo test -p pi-coding-agent --test public_api coding_session_run_public_operation_facade_is_importable -- --nocapture
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/public_operation.rs crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "feat: add public coding session operation facade"
```

## Task 3: Stable Snapshot And Client Facade

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/public_projection.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Write failing snapshot facade test**

Add imports in `crates/pi-coding-agent/tests/public_api.rs`:

```rust
use pi_coding_agent::api::{CodingAgentClientId, CodingAgentSnapshot};
```

Add this test:

```rust
#[tokio::test]
async fn coding_session_snapshot_public_facade_is_importable() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();

    let snapshot: CodingAgentSnapshot = session.snapshot();
    let session_id = snapshot.session.session_id.clone();
    assert!(session_id.starts_with("runtime_sess_"));
    assert_eq!(snapshot.cursor.last_event_sequence, 0);

    let client_id = CodingAgentClientId::new("public-client");
    let connected = session.connect(client_id.clone());
    assert_eq!(connected.client_id, client_id);
    assert_eq!(connected.snapshot.session.session_id, session_id);
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent --test public_api coding_session_snapshot_public_facade_is_importable -- --nocapture
```

Expected: FAIL because the public snapshot facade does not exist.

- [x] **Step 3: Add public projection facade types**

Create `crates/pi-coding-agent/src/coding_session/public_projection.rs`:

```rust
use super::client_projection::{ClientConnection, ClientConnectionId, UiSnapshot};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use crate::protocol::version::ProtocolFamilyVersion;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CodingAgentClientId(String);

impl CodingAgentClientId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSnapshotCursor {
    pub last_event_sequence: u64,
    pub capability_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSnapshot {
    pub cursor: CodingAgentSnapshotCursor,
    pub version: ProtocolFamilyVersion,
    pub session: CodingAgentSessionView,
    pub capabilities: CodingAgentCapabilities,
    pub active_operation: Option<String>,
    pub client_draft_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentClientConnection {
    pub client_id: CodingAgentClientId,
    pub snapshot: CodingAgentSnapshot,
}

impl From<UiSnapshot> for CodingAgentSnapshot {
    fn from(snapshot: UiSnapshot) -> Self {
        Self {
            cursor: CodingAgentSnapshotCursor {
                last_event_sequence: snapshot.cursor.last_event_sequence.get(),
                capability_generation: snapshot.cursor.capability_generation.get(),
            },
            version: snapshot.version,
            session: snapshot.session,
            capabilities: snapshot.capabilities,
            active_operation: snapshot.active_operation.map(|kind| kind.as_str().to_owned()),
            client_draft_count: snapshot.client_drafts.len(),
        }
    }
}

pub(crate) fn internal_client_id(id: &CodingAgentClientId) -> ClientConnectionId {
    ClientConnectionId::new(id.as_str())
}

pub(crate) fn public_client_connection(
    id: CodingAgentClientId,
    connection: ClientConnection,
    snapshot: UiSnapshot,
) -> CodingAgentClientConnection {
    debug_assert_eq!(connection.id().as_str(), id.as_str());
    CodingAgentClientConnection {
        client_id: id,
        snapshot: snapshot.into(),
    }
}
```

Add this crate-internal accessor to `ClientConnection` in `crates/pi-coding-agent/src/coding_session/client_projection.rs`:

```rust
pub(crate) fn id(&self) -> &ClientConnectionId {
    &self.id
}
```

- [x] **Step 4: Wire snapshot methods**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, add:

```rust
mod public_projection;
pub use public_projection::{
    CodingAgentClientConnection, CodingAgentClientId, CodingAgentSnapshot,
    CodingAgentSnapshotCursor,
};
```

Add public facade methods:

```rust
pub fn snapshot(&self) -> CodingAgentSnapshot {
    self.ui_snapshot(Vec::new()).into()
}

pub fn connect(
    &self,
    id: CodingAgentClientId,
) -> CodingAgentClientConnection {
    let internal_id = public_projection::internal_client_id(&id);
    let (connection, snapshot) = self.connect_client(internal_id, Vec::new());
    public_projection::public_client_connection(id, connection, snapshot)
}
```

In `crates/pi-coding-agent/src/lib.rs`, add the new types to `pub mod api`:

```rust
CodingAgentClientConnection, CodingAgentClientId, CodingAgentSnapshot,
CodingAgentSnapshotCursor,
```

- [x] **Step 5: Run GREEN test**

Run:

```bash
cargo test -p pi-coding-agent --test public_api coding_session_snapshot_public_facade_is_importable -- --nocapture
```

Expected: PASS.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/public_projection.rs crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "feat: add public coding session snapshot facade"
```

## Task 4: Stable Product Event Subscription Facade

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/public_projection.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [x] **Step 1: Write failing public product-event subscription test**

Add imports in `crates/pi-coding-agent/tests/public_api.rs`:

```rust
use pi_coding_agent::api::{CodingAgentProductEvent, CodingAgentProductEventReceiver};
```

Add this importability test:

```rust
#[test]
fn coding_session_product_event_subscription_public_facade_is_importable() {
    let _event_type_name = std::any::type_name::<CodingAgentProductEvent>();
    let _receiver_type_name = std::any::type_name::<CodingAgentProductEventReceiver>();
}
```

Add this behavior test to the existing `#[cfg(test)] mod tests` in `crates/pi-coding-agent/src/coding_session/mod.rs`:

```rust
#[tokio::test]
async fn public_product_event_receiver_maps_internal_product_events() {
    let session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
        .await
        .unwrap();
    let mut receiver = session.subscribe_product_events_public();
    session.emit_product_event_for_tests(CodingAgentEvent::Diagnostic {
        operation_id: None,
        message: "public event".into(),
    });

    let event = receiver.recv().await.unwrap();
    assert_eq!(event.sequence, 1);
    assert_eq!(event.family, "Diagnostic");
    assert_eq!(event.kind, "Diagnostic(Diagnostic)");
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent coding_session_product_event_subscription_public_facade_is_importable -- --nocapture
```

Expected: FAIL because public product-event facade types and subscription method do not exist.

- [x] **Step 3: Add public product event facade type**

In `crates/pi-coding-agent/src/coding_session/public_projection.rs`, add:

```rust
use super::event::ProductEvent;
use super::event_service::ProductEventReceiver;
use super::error::CodingSessionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentProductEvent {
    pub sequence: u64,
    pub family: String,
    pub kind: String,
}

impl From<ProductEvent> for CodingAgentProductEvent {
    fn from(event: ProductEvent) -> Self {
        Self {
            sequence: event.sequence().get(),
            family: format!("{:?}", event.family()),
            kind: format!("{:?}", event.kind()),
        }
    }
}

#[derive(Debug)]
pub struct CodingAgentProductEventReceiver {
    inner: ProductEventReceiver,
}

impl CodingAgentProductEventReceiver {
    pub(crate) fn new(inner: ProductEventReceiver) -> Self {
        Self { inner }
    }

    pub async fn recv(&mut self) -> Result<CodingAgentProductEvent, CodingSessionError> {
        self.inner.recv().await.map(CodingAgentProductEvent::from)
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentProductEvent>, CodingSessionError> {
        self.inner
            .try_recv()
            .map(|event| event.map(CodingAgentProductEvent::from))
    }
}
```

Use the existing crate-internal `ProductEvent::kind()` accessor when building the public facade value.

- [x] **Step 4: Add public subscription method**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, add:

```rust
pub fn subscribe_product_events_public(&self) -> CodingAgentProductEventReceiver {
    CodingAgentProductEventReceiver::new(self.subscribe_product_events())
}
```

Add `CodingAgentProductEvent` and `CodingAgentProductEventReceiver` to the public re-export list and `pi_coding_agent::api`.

- [x] **Step 5: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent coding_session_product_event_subscription_public_facade_is_importable -- --nocapture
cargo test -p pi-coding-agent --test public_api coding_session_public_api_symbols_are_importable -- --nocapture
```

Expected: both tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/event.rs crates/pi-coding-agent/src/coding_session/event_service.rs crates/pi-coding-agent/src/coding_session/public_projection.rs crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "feat: expose public product event facade"
```

## Task 5: Migrate First-Party Compatibility Subscription Callers

**Files:**
- Modify: `crates/pi-coding-agent/tests/agent_invocation.rs`
- Modify: `crates/pi-coding-agent/tests/agent_team_flow.rs`
- Modify: `crates/pi-coding-agent/tests/delegation_execution.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs` tests
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`

- [ ] **Step 1: Add failing source guard against first-party compatibility subscriptions**

Add this test to `crates/pi-coding-agent/tests/event_boundary_guards.rs`:

```rust
#[test]
fn first_party_code_does_not_consume_compatibility_event_subscription() {
    let scan_roots = [
        "crates/pi-coding-agent/src/protocol",
        "crates/pi-coding-agent/src/interactive",
        "crates/pi-coding-agent/tests",
    ];
    let repo_root = workspace_path("");
    let allowed = [
        "crates/pi-coding-agent/tests/public_api.rs",
        "crates/pi-coding-agent/tests/event_boundary_guards.rs",
    ];
    let mut violations = Vec::new();

    for root in scan_roots {
        collect_source_violations(
            &repo_root,
            &repo_root.join(root),
            &allowed,
            &mut violations,
            |line| line.contains(".subscribe()") || line.contains("CodingAgentEventReceiver"),
        );
    }

    assert!(
        violations.is_empty(),
        "first-party code should consume ProductEvent or public product-event facades instead of compatibility CodingAgentEventReceiver:\n{}",
        violations.join("\n")
    );
}
```

Add this helper to `event_boundary_guards.rs` below the new test:

```rust
fn collect_source_violations(
    repo_root: &std::path::Path,
    path: &std::path::Path,
    allowed_files: &[&str],
    violations: &mut Vec<String>,
    is_violation: impl Copy + Fn(&str) -> bool,
) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .expect("read source directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read source entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_source_violations(
                repo_root,
                &entry.path(),
                allowed_files,
                violations,
                is_violation,
            );
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }
    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    if allowed_files.contains(&relative.as_str()) {
        return;
    }
    let content = std::fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}
```

- [ ] **Step 2: Run RED guard**

Run:

```bash
cargo test -p pi-coding-agent --test event_boundary_guards first_party_code_does_not_consume_compatibility_event_subscription -- --nocapture
```

Expected: FAIL and list existing `.subscribe()` or `CodingAgentEventReceiver` usages.

- [ ] **Step 3: Replace integration-test compatibility receiver helpers**

For tests that only wait for product-visible lifecycle events, replace:

```rust
let mut events = session.subscribe();
```

with:

```rust
let mut events = session.subscribe_product_events_public();
```

Replace helpers typed as:

```rust
receiver: &mut pi_coding_agent::api::CodingAgentEventReceiver,
```

with:

```rust
receiver: &mut pi_coding_agent::api::CodingAgentProductEventReceiver,
```

Update assertions to inspect public product-event `family` and `kind` strings first. Keep payload-specific compatibility-event assertions only in focused compatibility tests.

- [ ] **Step 4: Keep adapter production paths on internal ProductEvent**

Verify production adapter files already use internal `ProductEvent` paths:

```bash
rg -n "\\.subscribe\\(\\)|CodingAgentEventReceiver" crates/pi-coding-agent/src/protocol crates/pi-coding-agent/src/interactive
```

Expected after edits: no matches, except comments in boundary tests.

- [ ] **Step 5: Run GREEN migration checks**

Run:

```bash
cargo test -p pi-coding-agent --test agent_invocation
cargo test -p pi-coding-agent --test agent_team_flow
cargo test -p pi-coding-agent --test delegation_execution
cargo test -p pi-coding-agent --test event_boundary_guards first_party_code_does_not_consume_compatibility_event_subscription -- --nocapture
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/tests/agent_invocation.rs crates/pi-coding-agent/tests/agent_team_flow.rs crates/pi-coding-agent/tests/delegation_execution.rs crates/pi-coding-agent/tests/event_boundary_guards.rs
git commit -m "test: migrate first-party event subscriptions"
```

## Task 6: Deprecate Broad Session Workflow Methods

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`

- [ ] **Step 1: Add failing guard for broad workflow method deprecations**

Add this test to `crates/pi-coding-agent/tests/api_boundary_guards.rs`:

```rust
#[test]
fn broad_session_workflow_methods_are_deprecated_in_favor_of_run() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner should be readable");
    for signature in [
        "pub async fn prompt(",
        "pub async fn compact(",
        "pub async fn summarize_branch(",
        "pub async fn self_healing_edit_with_options(",
        "pub async fn invoke_agent(",
        "pub async fn invoke_team(",
        "pub fn export_current_html(",
        "pub fn export_current(",
    ] {
        let preceding = preceding_non_blank_line(&source, signature)
            .unwrap_or_else(|| panic!("missing method signature: {signature}"));
        assert_eq!(
            preceding.trim(),
            "#[deprecated(note = \"use CodingAgentSession::run instead\")]",
            "{signature} should be deprecated after CodingAgentSession::run is available"
        );
    }
}
```

Add or reuse this helper in the same test file:

```rust
fn preceding_non_blank_line<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
    let lines: Vec<&str> = source.lines().collect();
    let idx = lines.iter().position(|line| line.contains(signature))?;
    if idx == 0 {
        return Some("");
    }
    let mut i = idx - 1;
    while i > 0 && lines[i].trim().is_empty() {
        i -= 1;
    }
    Some(lines[i])
}
```

- [ ] **Step 2: Run RED guard**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards broad_session_workflow_methods_are_deprecated_in_favor_of_run -- --nocapture
```

Expected: FAIL because methods are not yet deprecated.

- [ ] **Step 3: Add deprecation attributes**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, add this attribute to each broad public workflow method listed in the test:

```rust
#[deprecated(note = "use CodingAgentSession::run instead")]
```

Because the first `run` implementation delegates to these wrappers, add `#[allow(deprecated)]` to `CodingAgentSession::run` in the same edit. This keeps the transitional bridge warning-free while preserving the later cleanup path toward direct `Operation` service routing.

Do not deprecate `create`, `open`, `open_or_create`, `non_persistent`, `list`, `capabilities`, `view`, `snapshot`, `connect`, or public product-event subscription methods.

- [ ] **Step 4: Update public API tests to use `run` for operation smoke coverage**

In `crates/pi-coding-agent/tests/public_api.rs`, replace the direct export workflow smoke call with `CodingAgentSession::run`:

```rust
let outcome = session
    .run(CodingAgentOperation::ExportCurrent)
    .await
    .unwrap();
assert!(matches!(outcome, CodingAgentOperationOutcome::Export(_)));
```

For compatibility-specific tests that still intentionally call deprecated methods, add:

```rust
#[allow(deprecated)]
```

to the smallest function scope.

- [ ] **Step 5: Run GREEN checks**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards broad_session_workflow_methods_are_deprecated_in_favor_of_run -- --nocapture
cargo test -p pi-coding-agent --test public_api
```

Expected: both pass without new warnings promoted to errors.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/tests/api_boundary_guards.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "chore: deprecate broad session workflow methods"
```

## Task 7: Remove Compatibility Event Receiver From Stable Facade

**Files:**
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Modify: `crates/pi-coding-agent/tests/public_api.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`

- [ ] **Step 1: Add failing guard against exporting compatibility receiver from `api`**

Add this test to `crates/pi-coding-agent/tests/api_boundary_guards.rs`:

```rust
#[test]
fn stable_api_does_not_export_compatibility_event_receiver() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let api_module = lib_source
        .split("pub mod api {")
        .nth(1)
        .expect("api module should exist")
        .split("\n}\n\n#[cfg")
        .next()
        .expect("api module should end before test support");

    assert!(
        !api_module.contains("CodingAgentEventReceiver"),
        "stable api should export CodingAgentProductEventReceiver instead of compatibility CodingAgentEventReceiver"
    );
}
```

- [ ] **Step 2: Run RED guard**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards stable_api_does_not_export_compatibility_event_receiver -- --nocapture
```

Expected: FAIL because `CodingAgentEventReceiver` is currently exported from `pi_coding_agent::api`.

- [ ] **Step 3: Remove compatibility receiver from stable facade**

In `crates/pi-coding-agent/src/lib.rs`, remove `CodingAgentEventReceiver` from the `pub mod api` `pub use crate::coding_session::{ ... }` list.

Keep `CodingAgentEvent` exported in this stage so compatibility payload matching remains available. Do not remove it until a later plan expands `CodingAgentProductEvent` beyond family/kind metadata.

- [ ] **Step 4: Update public API test imports**

In `crates/pi-coding-agent/tests/public_api.rs`, remove `CodingAgentEventReceiver` from the `use pi_coding_agent::api::{ ... }` list and replace receiver smoke checks with:

```rust
let _receiver_type_name = std::any::type_name::<CodingAgentProductEventReceiver>();
```

- [ ] **Step 5: Run GREEN checks**

Run:

```bash
cargo test -p pi-coding-agent --test api_boundary_guards stable_api_does_not_export_compatibility_event_receiver -- --nocapture
cargo test -p pi-coding-agent --test public_api
```

Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/lib.rs crates/pi-coding-agent/tests/api_boundary_guards.rs crates/pi-coding-agent/tests/public_api.rs
git commit -m "chore: narrow stable event receiver facade"
```

## Task 8: Delete Or Test-Gate Internal Compatibility Shims

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/tests/event_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/api_boundary_guards.rs`

- [ ] **Step 1: Add failing guard that compatibility subscribe is test-gated or deprecated**

Add this test to `crates/pi-coding-agent/tests/event_boundary_guards.rs`:

```rust
#[test]
fn compatibility_subscribe_is_not_a_stable_runtime_path() {
    let owner_source = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/mod.rs",
    ))
    .expect("read coding session owner");
    let event_service_source = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/event_service.rs",
    ))
    .expect("read event service");

    assert!(
        owner_source.contains("#[deprecated(note = \"use subscribe_product_events_public instead\")]")
            || owner_source.contains("#[cfg(test)]\n    pub fn subscribe("),
        "CodingAgentSession::subscribe compatibility path should be deprecated or test-gated"
    );
    assert!(
        event_service_source.contains("#[deprecated(note = \"use ProductEventReceiver instead\")]")
            || event_service_source.contains("#[cfg(test)]\n    pub(crate) fn subscribe("),
        "EventService compatibility CodingAgentEvent subscribe path should be deprecated or test-gated"
    );
}
```

- [ ] **Step 2: Run RED guard**

Run:

```bash
cargo test -p pi-coding-agent --test event_boundary_guards compatibility_subscribe_is_not_a_stable_runtime_path -- --nocapture
```

Expected: FAIL because compatibility subscribe is still a normal public method.

- [ ] **Step 3: Deprecate compatibility subscribe**

In `crates/pi-coding-agent/src/coding_session/mod.rs`, change:

```rust
pub fn subscribe(&self) -> CodingAgentEventReceiver {
```

to:

```rust
#[deprecated(note = "use subscribe_product_events_public instead")]
pub fn subscribe(&self) -> CodingAgentEventReceiver {
```

In `crates/pi-coding-agent/src/coding_session/event_service.rs`, change:

```rust
pub(crate) fn subscribe(&self) -> CodingAgentEventReceiver {
```

to:

```rust
#[deprecated(note = "use ProductEventReceiver instead")]
pub(crate) fn subscribe(&self) -> CodingAgentEventReceiver {
```

Add `#[allow(deprecated)]` only around the owner compatibility wrapper that delegates to `event_service.subscribe()`.

- [ ] **Step 4: Remove or gate compatibility receiver when remaining callers are gone**

Run:

```bash
rg -n "CodingAgentEventReceiver|\\.subscribe\\(\\)" crates/pi-coding-agent/src crates/pi-coding-agent/tests
```

Keep `CodingAgentEventReceiver` and `CodingAgentSession::subscribe()` deprecated in this stage. Record this deletion stop condition in `docs/TODO.md`: remove both after a later product-event payload plan lets compatibility tests stop matching `CodingAgentEvent` payloads directly.

- [ ] **Step 5: Run GREEN checks**

Run:

```bash
cargo test -p pi-coding-agent --test event_boundary_guards compatibility_subscribe_is_not_a_stable_runtime_path -- --nocapture
cargo test -p pi-coding-agent --test api_boundary_guards
cargo test -p pi-coding-agent --test public_api
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/event_service.rs crates/pi-coding-agent/tests/event_boundary_guards.rs docs/TODO.md
git commit -m "chore: deprecate compatibility event subscription"
```

## Task 9: Closure Audit And Documentation

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-10-operation-runtime-stage-8-public-facade-narrowing-plan.md`

- [ ] **Step 1: Run full Stage 8 verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent --test api_boundary_guards
cargo test -p pi-coding-agent --test event_boundary_guards
cargo test -p pi-coding-agent --test product_runtime_boundary_guards
cargo test -p pi-coding-agent --test public_api
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test interactive_mode
cargo check --workspace
cargo test --workspace
git diff --check
```

Expected: every command exits with code 0.

- [ ] **Step 2: Update this plan's verification checklist**

After the commands pass, mark the corresponding checklist entries below.

- [ ] **Step 3: Update the project checklist**

Update the active operation-runtime item in `docs/TODO.md` so the Stage 8 portion says:

```markdown
Stage 8 public facade narrowing/deletion is complete: root compatibility re-exports are deprecated in favor of `pi_coding_agent::api`, public operation execution goes through `CodingAgentSession::run(CodingAgentOperation)`, embedding clients can consume stable snapshot and product-event facades, first-party adapters/tests no longer consume compatibility `CodingAgentEventReceiver`, broad session workflow methods are deprecated in favor of the operation facade, and compatibility event subscription is deprecated or test-gated with deletion stop conditions recorded.
```

Add a progress log entry:

```markdown
- 2026-07-10: Stage 8 public facade narrowing/deletion completed. The stable facade now exposes operation execution, snapshot/client, and product-event subscription boundaries; root compatibility re-exports and broad session workflow methods are deprecated; first-party adapter/test consumers use snapshot plus product-event surfaces; and remaining compatibility event subscription has an explicit deletion boundary.
```

- [ ] **Step 4: Commit closure documentation**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-10-operation-runtime-stage-8-public-facade-narrowing-plan.md
git commit -m "docs: close runtime facade narrowing stage"
```

## Verification Checklist

- [ ] `cargo fmt --check`
- [ ] `cargo test -p pi-coding-agent --test api_boundary_guards`
- [ ] `cargo test -p pi-coding-agent --test event_boundary_guards`
- [ ] `cargo test -p pi-coding-agent --test product_runtime_boundary_guards`
- [ ] `cargo test -p pi-coding-agent --test public_api`
- [ ] `cargo test -p pi-coding-agent --test rpc_mode`
- [ ] `cargo test -p pi-coding-agent --test interactive_mode`
- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `git diff --check`

## Spec Coverage

- Promote `run(Operation)`: Task 2 adds public `CodingAgentSession::run(CodingAgentOperation)`.
- Promote snapshot/view verbs: Task 3 exposes stable snapshot and client connection facades.
- Promote subscribe: Task 4 exposes a public product-event subscription facade.
- Retire broad public methods after adapters/tests migrate: Tasks 5 and 6 migrate first-party consumers and deprecate broad workflow methods.
- Remove compatibility shims after last caller migrates: Tasks 7 and 8 narrow or deprecate compatibility receiver paths with explicit deletion guards.
- Keep TypeScript session JSONL compatibility rejected: no task adds TypeScript session import/export, and closure checks should preserve existing legacy-session boundary guards.
