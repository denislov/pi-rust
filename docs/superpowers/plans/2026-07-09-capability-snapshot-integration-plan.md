# Capability Snapshot Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make operation-local capability snapshots the single permission language for model access, tools, plugins, filesystem, shell, and session read/write behavior.

**Architecture:** Add an internal snapshot model owned by `pi-coding-agent`, freeze one snapshot during operation admission, and pass only snapshot-derived narrow handles into runtime, plugin, tool, and session services. Runtime writes install a new capability generation for future operations; active operations keep their captured snapshot unless an explicit revocation path cancels them.

**Tech Stack:** Rust 2024, `pi-coding-agent`, `pi-agent-core`, typed session events, existing `IntentRouter`, `RuntimeService`, `PluginService`, and Rust-native session log tests.

---

## Current Context

Stage 4 closed with durable `operation.started.runtime_generation` references, but those references currently use a baseline generation. Stage 5 replaces that baseline with a real operation-local `OperationCapabilitySnapshot`.

Important existing boundaries:

- `crates/pi-coding-agent/src/coding_session/operation.rs` owns `Operation`, `OperationMetadata`, and `OperationAdmission`.
- `crates/pi-coding-agent/src/coding_session/intent_router.rs` admits operations and returns `OperationPermit`.
- `crates/pi-coding-agent/src/coding_session/mod.rs` builds admissions for async, sync, sync-mutable, and query paths.
- `crates/pi-coding-agent/src/coding_session/runtime_service.rs` builds `Agent` runtime from `RuntimeSnapshot` plus plugin tools.
- `crates/pi-coding-agent/src/coding_session/plugin_service.rs` collects plugin tools, commands, hooks, UI, keybindings, and Flow extension points through scoped hosts.
- `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs` persists operation-started runtime references.

Stage 5 does not redesign public protocol payloads beyond capability generation metadata needed to explain runtime writes. Stage 6 owns full snapshot/reconnect/client projection semantics.

## File Structure

- Create `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`
  - Internal capability snapshot types, generation state, grants, and helper conversions to persisted generation refs.
- Modify `crates/pi-coding-agent/src/coding_session/mod.rs`
  - Own the capability snapshot service in `CodingAgentSession`.
  - Resolve operation admissions through the snapshot service.
  - Install new generations after runtime writes.
- Modify `crates/pi-coding-agent/src/coding_session/operation.rs`
  - Attach `OperationCapabilitySnapshot` to `OperationAdmission`.
- Modify `crates/pi-coding-agent/src/coding_session/intent_router.rs`
  - Preserve admission behavior while making the frozen snapshot visible through `OperationPermit`.
- Modify `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
  - Persist the admitted snapshot generation/profile references instead of the baseline generation constant.
- Modify `crates/pi-coding-agent/src/coding_session/session_service.rs`
  - Add snapshot-aware transaction entrypoints.
- Modify `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
  - Build model access and tool visibility from the admitted snapshot.
- Modify `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
  - Collect and execute plugin features only through `PluginCapabilitySet`.
- Modify plugin host files under `crates/pi-coding-agent/src/plugins/`
  - Carry narrow plugin capability context in registration hosts.
- Modify builtin tool files under `crates/pi-coding-agent/src/tools/`
  - Gate filesystem and shell operations through snapshot-derived handles.
- Modify service files that use session persistence:
  - `manual_compaction_service.rs`
  - `branch_summary_service.rs`
  - `plugin_load_service.rs`
  - `self_healing_edit_service.rs`
  - `delegation_execution_service.rs`
  - `agent_invocation_flow.rs`
  - `agent_team_flow.rs`

## Task 1: Core Capability Snapshot Model

**Files:**
- Create: `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/event.rs`

- [x] **Step 1: Write failing snapshot model tests**

Add this test module to the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::operation_control::OperationKind;
    use crate::coding_session::profiles::ProfileId;
    use crate::plugins::PluginCapabilities;

    fn input(kind: OperationKind) -> CapabilitySnapshotInput {
        CapabilitySnapshotInput {
            operation_id: "op_snapshot".into(),
            operation_kind: kind,
            actor: ActorId::Client,
            default_profile_id: ProfileId::from("reviewer"),
            plugin_capabilities: PluginCapabilities {
                tool_providers: 1,
                command_providers: 1,
                hook_providers: 1,
                ui_providers: 0,
                keybind_providers: 0,
                flow_extensions: 0,
                diagnostics: 0,
            },
            persistent_session: true,
            cwd: Some(std::path::PathBuf::from("/workspace")),
            runtime_tools: vec!["read".into(), "bash".into(), "edit".into()],
            profile_tools: vec!["read".into(), "edit".into()],
        }
    }

    #[test]
    fn prompt_snapshot_grants_model_tools_session_and_plugin_sets() {
        let service = CapabilitySnapshotService::new();

        let snapshot = service.snapshot(input(OperationKind::Prompt));

        assert_eq!(snapshot.generation, CapabilityGeneration::new(1));
        assert_eq!(snapshot.operation_id, "op_snapshot");
        assert_eq!(snapshot.actor, ActorId::Client);
        assert_eq!(
            snapshot.model,
            Some(ModelCapability {
                profile_id: Some(ProfileId::from("reviewer")),
            })
        );
        assert!(snapshot.tools.allows("read"));
        assert!(snapshot.tools.allows("edit"));
        assert!(!snapshot.tools.allows("bash"));
        assert!(snapshot.filesystem.is_some());
        assert!(snapshot.shell.is_none());
        assert!(snapshot.session_read.is_some());
        assert!(snapshot.session_write.is_some());
        assert_eq!(snapshot.plugin.tool_providers, 1);
    }

    #[test]
    fn runtime_write_install_generation_advances_future_snapshots() {
        let mut service = CapabilitySnapshotService::new();
        let first = service.snapshot(input(OperationKind::Prompt));

        let installed = service.install_next_generation(CapabilityRevocationPolicy::FutureOnly);
        let second = service.snapshot(input(OperationKind::Prompt));

        assert_eq!(first.generation, CapabilityGeneration::new(1));
        assert_eq!(installed.generation, CapabilityGeneration::new(2));
        assert_eq!(second.generation, CapabilityGeneration::new(2));
        assert_eq!(installed.revocation, CapabilityRevocationPolicy::FutureOnly);
    }
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent capability_snapshot --lib
```

Expected: fail because `capability_snapshot.rs` and the snapshot types do not exist.

- [x] **Step 3: Add the model and service**

Create `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`:

```rust
use std::collections::BTreeSet;
use std::path::PathBuf;

use super::operation_control::OperationKind;
use super::profiles::ProfileId;
use super::session_log::event::PersistedRuntimeGenerationRef;
use crate::plugins::PluginCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CapabilityGeneration(u64);

impl CapabilityGeneration {
    pub(crate) fn new(value: u64) -> Self {
        Self(value.max(1))
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActorId {
    Client,
    Plugin(String),
    ChildOperation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelCapability {
    pub(crate) profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ToolCapabilitySet {
    allowed: BTreeSet<String>,
}

impl ToolCapabilitySet {
    pub(crate) fn from_names(names: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowed: names.into_iter().collect(),
        }
    }

    pub(crate) fn allows(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.allowed.iter().map(String::as_str)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CommandCapabilitySet {
    allowed: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilesystemCapability {
    pub(crate) cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellCapability {
    pub(crate) cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReadCapability {
    pub(crate) persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionWriteCapability {
    pub(crate) persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiCapability;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PluginCapabilitySet {
    pub(crate) tool_providers: usize,
    pub(crate) command_providers: usize,
    pub(crate) hook_providers: usize,
    pub(crate) ui_providers: usize,
    pub(crate) keybind_providers: usize,
    pub(crate) flow_extensions: usize,
}

impl From<&PluginCapabilities> for PluginCapabilitySet {
    fn from(value: &PluginCapabilities) -> Self {
        Self {
            tool_providers: value.tool_providers,
            command_providers: value.command_providers,
            hook_providers: value.hook_providers,
            ui_providers: value.ui_providers,
            keybind_providers: value.keybind_providers,
            flow_extensions: value.flow_extensions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationCapabilitySnapshot {
    pub(crate) generation: CapabilityGeneration,
    pub(crate) operation_id: String,
    pub(crate) actor: ActorId,
    pub(crate) model: Option<ModelCapability>,
    pub(crate) tools: ToolCapabilitySet,
    pub(crate) commands: CommandCapabilitySet,
    pub(crate) filesystem: Option<FilesystemCapability>,
    pub(crate) shell: Option<ShellCapability>,
    pub(crate) session_read: Option<SessionReadCapability>,
    pub(crate) session_write: Option<SessionWriteCapability>,
    pub(crate) ui: Option<UiCapability>,
    pub(crate) plugin: PluginCapabilitySet,
}

impl OperationCapabilitySnapshot {
    pub(crate) fn persisted_runtime_generation_ref(&self) -> PersistedRuntimeGenerationRef {
        PersistedRuntimeGenerationRef {
            profile_id: self.model.as_ref().and_then(|model| model.profile_id.clone()),
            capability_generation: Some(self.generation.get()),
        }
    }
}

#[cfg(test)]
impl OperationCapabilitySnapshot {
    pub(crate) fn permissive_for_tests(operation_id: impl Into<String>) -> Self {
        Self {
            generation: CapabilityGeneration::new(1),
            operation_id: operation_id.into(),
            actor: ActorId::Client,
            model: Some(ModelCapability { profile_id: None }),
            tools: ToolCapabilitySet::from_names([
                "read".to_string(),
                "write".to_string(),
                "edit".to_string(),
                "bash".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "ls".to_string(),
            ]),
            commands: Default::default(),
            filesystem: Some(FilesystemCapability {
                cwd: std::path::PathBuf::from("."),
            }),
            shell: Some(ShellCapability {
                cwd: std::path::PathBuf::from("."),
            }),
            session_read: Some(SessionReadCapability { persistent: true }),
            session_write: Some(SessionWriteCapability { persistent: true }),
            ui: Some(UiCapability),
            plugin: PluginCapabilitySet {
                tool_providers: usize::MAX,
                command_providers: usize::MAX,
                hook_providers: usize::MAX,
                ui_providers: usize::MAX,
                keybind_providers: usize::MAX,
                flow_extensions: usize::MAX,
            },
        }
    }

    pub(crate) fn test_without_shell(operation_id: impl Into<String>) -> Self {
        let mut snapshot = Self::permissive_for_tests(operation_id);
        snapshot.shell = None;
        snapshot
    }

    pub(crate) fn test_without_session_write(operation_id: impl Into<String>) -> Self {
        let mut snapshot = Self::permissive_for_tests(operation_id);
        snapshot.session_write = None;
        snapshot
    }

    pub(crate) fn test_with_tools(
        operation_id: impl Into<String>,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut snapshot = Self::permissive_for_tests(operation_id);
        snapshot.tools = ToolCapabilitySet::from_names(names.into_iter().map(Into::into));
        snapshot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilityRevocationPolicy {
    FutureOnly,
    CancelMatchingOperations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledCapabilityGeneration {
    pub(crate) generation: CapabilityGeneration,
    pub(crate) revocation: CapabilityRevocationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapabilitySnapshotInput {
    pub(crate) operation_id: String,
    pub(crate) operation_kind: OperationKind,
    pub(crate) actor: ActorId,
    pub(crate) default_profile_id: ProfileId,
    pub(crate) plugin_capabilities: PluginCapabilities,
    pub(crate) persistent_session: bool,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) runtime_tools: Vec<String>,
    pub(crate) profile_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CapabilitySnapshotService {
    current_generation: CapabilityGeneration,
}

impl CapabilitySnapshotService {
    pub(crate) fn new() -> Self {
        Self {
            current_generation: CapabilityGeneration::new(1),
        }
    }

    pub(crate) fn current_generation(&self) -> CapabilityGeneration {
        self.current_generation
    }

    pub(crate) fn install_next_generation(
        &mut self,
        revocation: CapabilityRevocationPolicy,
    ) -> InstalledCapabilityGeneration {
        self.current_generation = self.current_generation.next();
        InstalledCapabilityGeneration {
            generation: self.current_generation,
            revocation,
        }
    }

    pub(crate) fn snapshot(&self, input: CapabilitySnapshotInput) -> OperationCapabilitySnapshot {
        let writes_session = matches!(
            input.operation_kind,
            OperationKind::Prompt
                | OperationKind::Compact
                | OperationKind::BranchSummary
                | OperationKind::ForkSession
                | OperationKind::SelfHealingEdit
        );
        let reads_session = writes_session || matches!(input.operation_kind, OperationKind::Export);
        let model = match input.operation_kind {
            OperationKind::Prompt
            | OperationKind::BranchSummary
            | OperationKind::AgentInvocation
            | OperationKind::AgentTeam
            | OperationKind::SelfHealingEdit => Some(ModelCapability {
                profile_id: Some(input.default_profile_id.clone()),
            }),
            _ => None,
        };
        let allowed_tools = if input.profile_tools.is_empty() {
            Vec::new()
        } else {
            input
                .runtime_tools
                .into_iter()
                .filter(|name| input.profile_tools.iter().any(|allowed| allowed == name))
                .collect::<Vec<_>>()
        };
        let cwd = input.cwd;
        let filesystem = cwd
            .as_ref()
            .filter(|_| allowed_tools.iter().any(|name| name == "read" || name == "edit"))
            .map(|cwd| FilesystemCapability { cwd: cwd.clone() });
        let shell = cwd
            .as_ref()
            .filter(|_| allowed_tools.iter().any(|name| name == "bash"))
            .map(|cwd| ShellCapability { cwd: cwd.clone() });
        OperationCapabilitySnapshot {
            generation: self.current_generation,
            operation_id: input.operation_id,
            actor: input.actor,
            model,
            tools: ToolCapabilitySet::from_names(allowed_tools),
            commands: CommandCapabilitySet::default(),
            filesystem,
            shell,
            session_read: reads_session.then_some(SessionReadCapability {
                persistent: input.persistent_session,
            }),
            session_write: writes_session.then_some(SessionWriteCapability {
                persistent: input.persistent_session,
            }),
            ui: None,
            plugin: PluginCapabilitySet::from(&input.plugin_capabilities),
        }
    }
}

impl Default for CapabilitySnapshotService {
    fn default() -> Self {
        Self::new()
    }
}
```

Add this module declaration near the other `coding_session` modules in `crates/pi-coding-agent/src/coding_session/mod.rs`:

```rust
mod capability_snapshot;
```

- [x] **Step 4: Run GREEN tests**

Run:

```bash
cargo test -p pi-coding-agent capability_snapshot --lib
```

Expected: both snapshot model tests pass.

- [x] **Step 5: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/capability_snapshot.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: add operation capability snapshot model"
```

## Task 2: Freeze Snapshots During Operation Admission

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/operation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/intent_router.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Write failing admission tests**

Add this test to `crates/pi-coding-agent/src/coding_session/operation.rs`:

```rust
#[test]
fn operation_admission_carries_frozen_capability_snapshot() {
    use crate::coding_session::capability_snapshot::{
        ActorId, CapabilityGeneration, ModelCapability, OperationCapabilitySnapshot,
        PluginCapabilitySet, ToolCapabilitySet,
    };

    let metadata = OperationMetadata {
        static_kind: Some(OperationKind::Prompt),
        origin: OperationOrigin::ClientRoot,
        class: OperationClass::SessionWriteRoot,
        dispatch_mode: OperationDispatchMode::Async,
    };
    let snapshot = OperationCapabilitySnapshot {
        generation: CapabilityGeneration::new(7),
        operation_id: "op_admitted".into(),
        actor: ActorId::Client,
        model: Some(ModelCapability { profile_id: None }),
        tools: ToolCapabilitySet::from_names(["read".to_string()]),
        commands: Default::default(),
        filesystem: None,
        shell: None,
        session_read: None,
        session_write: None,
        ui: None,
        plugin: PluginCapabilitySet::default(),
    };

    let admission = OperationAdmission::new(
        OperationKind::Prompt,
        metadata,
        Some("2026-07-09T00:00:00Z".into()),
        snapshot.clone(),
    );

    assert_eq!(admission.capability_snapshot, snapshot);
}
```

Add this test to `crates/pi-coding-agent/src/coding_session/intent_router.rs`:

```rust
#[test]
fn operation_permit_exposes_the_frozen_snapshot_for_execution() {
    use crate::coding_session::capability_snapshot::{
        ActorId, CapabilityGeneration, OperationCapabilitySnapshot, PluginCapabilitySet,
        ToolCapabilitySet,
    };

    let control = OperationControl::new();
    let metadata = OperationMetadata {
        static_kind: Some(OperationKind::Export),
        origin: OperationOrigin::ClientRoot,
        class: OperationClass::ReadOnly,
        dispatch_mode: OperationDispatchMode::SyncReadOnly,
    };
    let snapshot = OperationCapabilitySnapshot {
        generation: CapabilityGeneration::new(3),
        operation_id: "op_export".into(),
        actor: ActorId::Client,
        model: None,
        tools: ToolCapabilitySet::default(),
        commands: Default::default(),
        filesystem: None,
        shell: None,
        session_read: None,
        session_write: None,
        ui: None,
        plugin: PluginCapabilitySet::default(),
    };
    let admission = OperationAdmission::new(
        OperationKind::Export,
        metadata,
        None,
        snapshot.clone(),
    );

    let permit = IntentRouter::admit_operation(
        &control,
        &admission,
        OperationDispatchMode::SyncReadOnly,
    )
    .unwrap();

    assert_eq!(permit.capability_snapshot(), &snapshot);
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent operation_admission_carries_frozen_capability_snapshot --lib
cargo test -p pi-coding-agent operation_permit_exposes_the_frozen_snapshot_for_execution --lib
```

Expected: fail because `OperationAdmission::new()` does not accept a snapshot and `OperationPermit` has no snapshot accessor.

- [x] **Step 3: Attach snapshots to admission and permit**

Update `OperationAdmission` in `operation.rs`:

```rust
use super::capability_snapshot::OperationCapabilitySnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationAdmission {
    pub(crate) kind: OperationKind,
    pub(crate) metadata: OperationMetadata,
    pub(crate) admitted_at: Option<String>,
    pub(crate) capability_snapshot: OperationCapabilitySnapshot,
}

impl OperationAdmission {
    pub(crate) fn new(
        kind: OperationKind,
        metadata: OperationMetadata,
        admitted_at: Option<String>,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        Self {
            kind,
            metadata,
            admitted_at,
            capability_snapshot,
        }
    }
}
```

Update `OperationPermit` in `intent_router.rs`:

```rust
use super::capability_snapshot::OperationCapabilitySnapshot;

pub(crate) struct OperationPermit {
    guard: Option<OperationGuard>,
    capability_snapshot: OperationCapabilitySnapshot,
    #[cfg(test)]
    kind: OperationKind,
    #[cfg(test)]
    class: OperationClass,
}

impl OperationPermit {
    fn guarded(
        kind: OperationKind,
        class: OperationClass,
        guard: OperationGuard,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);
        Self {
            guard: Some(guard),
            capability_snapshot,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    fn unguarded(
        kind: OperationKind,
        class: OperationClass,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);
        Self {
            guard: None,
            capability_snapshot,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    pub(crate) fn capability_snapshot(&self) -> &OperationCapabilitySnapshot {
        &self.capability_snapshot
    }
}
```

Update `IntentRouter::admit_operation()` to clone `admission.capability_snapshot` into the permit:

```rust
if admission.metadata.class == OperationClass::ReadOnly {
    return Ok(OperationPermit::unguarded(
        admission.kind,
        admission.metadata.class,
        admission.capability_snapshot.clone(),
    ));
}

control.begin(admission.kind).map(|guard| {
    OperationPermit::guarded(
        admission.kind,
        admission.metadata.class,
        guard,
        admission.capability_snapshot.clone(),
    )
})
```

- [x] **Step 4: Route `CodingAgentSession` admission through snapshots**

Add a `capability_snapshots` field to `CodingAgentSession` in `mod.rs`:

```rust
capability_snapshots: CapabilitySnapshotService,
```

Initialize it in every constructor path:

```rust
capability_snapshots: CapabilitySnapshotService::new(),
```

Add this helper near `resolve_operation_admission()`:

```rust
fn snapshot_input_for_operation(
    &self,
    operation_id: String,
    operation: &Operation,
) -> CapabilitySnapshotInput {
    let plugin_capabilities = self.plugin_service.capabilities();
    let default_profile_id = self.default_agent_profile_id();
    CapabilitySnapshotInput {
        operation_id,
        operation_kind: operation.kind(),
        actor: ActorId::Client,
        default_profile_id,
        plugin_capabilities,
        persistent_session: matches!(self.persistence, SessionPersistence::Persistent(_)),
        cwd: self.cwd(),
        runtime_tools: self.current_runtime_tool_names(),
        profile_tools: self.current_profile_tool_names(),
    }
}
```

Add these helpers with concrete return values based on current runtime state:

```rust
fn current_runtime_tool_names(&self) -> Vec<String> {
    vec![
        "read".into(),
        "write".into(),
        "edit".into(),
        "bash".into(),
        "grep".into(),
        "find".into(),
        "ls".into(),
    ]
}

fn current_profile_tool_names(&self) -> Vec<String> {
    match self.active_agent_profile() {
        Some(profile) if !profile.tools.is_empty() => profile.tools.clone(),
        _ => self.current_runtime_tool_names(),
    }
}
```

Update `run_sync_operation()` and `run_sync_mut_operation()` to call `self.resolve_operation_admission(&operation)?` instead of `IntentRouter::static_admission(&operation)?`.

Update `resolve_operation_admission()` so both dynamic and static operations create a snapshot before calling `OperationAdmission::new()`:

```rust
let operation_id = self.next_operation_admission_id(operation);
let snapshot = self
    .capability_snapshots
    .snapshot(self.snapshot_input_for_operation(operation_id, operation));
Ok(OperationAdmission::new(kind, metadata, admitted_at, snapshot))
```

- [x] **Step 5: Run GREEN and regression tests**

Run:

```bash
cargo test -p pi-coding-agent operation_admission_carries_frozen_capability_snapshot --lib
cargo test -p pi-coding-agent operation_permit_exposes_the_frozen_snapshot_for_execution --lib
cargo test -p pi-coding-agent intent_router --lib
cargo check -p pi-coding-agent
```

Expected: selected tests pass and the crate checks.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/operation.rs crates/pi-coding-agent/src/coding_session/intent_router.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: freeze capability snapshots at operation admission"
```

## Task 3: Persist Admitted Snapshot Generations In Session Events

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_log/transaction.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Write failing persistence tests**

Add this test to `session_service.rs`:

```rust
#[test]
fn prompt_transaction_persists_admitted_snapshot_generation() {
    let temp = tempfile::tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_snapshot_generation")
        .with_default_agent_profile_id("reviewer")
        .with_session_log_root(temp.path());
    let mut service = SessionService::create(&options).unwrap();
    let snapshot = OperationCapabilitySnapshot {
        generation: CapabilityGeneration::new(9),
        operation_id: "op_snapshot".into(),
        actor: ActorId::Client,
        model: Some(ModelCapability {
            profile_id: Some(ProfileId::from("reviewer")),
        }),
        tools: ToolCapabilitySet::default(),
        commands: Default::default(),
        filesystem: None,
        shell: None,
        session_read: None,
        session_write: None,
        ui: None,
        plugin: PluginCapabilitySet::default(),
    };
    let mut transaction = service.begin_prompt_transaction_with_snapshot(&snapshot);
    let operation_id = transaction.operation_id().to_owned();
    transaction
        .record_user_input(vec![PersistedContentBlock::Text {
            text: "hello".into(),
        }])
        .unwrap();

    service
        .commit_prompt_transaction(Some(transaction), operation_id)
        .unwrap();

    let events = service.store.read_events(&service.handle).unwrap();
    let persisted = events
        .iter()
        .find_map(|event| match &event.data {
            SessionEventData::OperationStarted {
                operation: OperationKind::Prompt,
                runtime_generation,
            } => Some(runtime_generation),
            _ => None,
        })
        .unwrap();
    assert_eq!(persisted.profile_id, Some(ProfileId::from("reviewer")));
    assert_eq!(persisted.capability_generation, Some(9));
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent prompt_transaction_persists_admitted_snapshot_generation --lib
```

Expected: fail because `begin_prompt_transaction_with_snapshot()` does not exist.

- [x] **Step 3: Add snapshot-aware transaction constructors**

In `TurnTransaction`, replace the baseline generation helper with an explicit constructor:

```rust
pub(crate) fn begin_with_runtime_generation(
    store: &SessionLogStore,
    handle: SessionHandle,
    ids: G,
    clock: C,
    operation: OperationKind,
    runtime_generation: PersistedRuntimeGenerationRef,
) -> Self {
    let mut transaction = Self::begin_without_start_event(store, handle, ids, clock);
    transaction.push_event(SessionEventData::OperationStarted {
        operation,
        runtime_generation,
    });
    transaction.push_event(SessionEventData::TurnStarted {});
    transaction
}
```

Keep `begin()` as a compatibility helper for tests and legacy internal paths:

```rust
pub(crate) fn begin(
    store: &SessionLogStore,
    handle: SessionHandle,
    ids: G,
    clock: C,
    operation: OperationKind,
) -> Self {
    Self::begin_with_runtime_generation(
        store,
        handle,
        ids,
        clock,
        operation,
        PersistedRuntimeGenerationRef::default(),
    )
}
```

In `SessionService`, add snapshot-aware prompt and plugin-load constructors:

```rust
pub(crate) fn begin_prompt_transaction_with_snapshot(
    &self,
    snapshot: &OperationCapabilitySnapshot,
) -> PromptTurnTransaction {
    TurnTransaction::begin_with_runtime_generation(
        &self.store,
        self.handle.clone(),
        SystemIdGenerator,
        SystemClock,
        OperationKind::Prompt,
        snapshot.persisted_runtime_generation_ref(),
    )
}

pub(crate) fn begin_plugin_load_transaction_with_snapshot(
    &self,
    snapshot: &OperationCapabilitySnapshot,
) -> PromptTurnTransaction {
    TurnTransaction::begin_with_runtime_generation(
        &self.store,
        self.handle.clone(),
        SystemIdGenerator,
        SystemClock,
        OperationKind::PluginLoad,
        snapshot.persisted_runtime_generation_ref(),
    )
}
```

- [x] **Step 4: Route owner operation paths through snapshot-aware constructors**

In `CodingAgentSession`, when a prompt or plugin-load operation has an `OperationPermit`, pass `permit.capability_snapshot()` into `SessionService`:

```rust
let mut transaction = session_service
    .begin_prompt_transaction_with_snapshot(operation_permit.capability_snapshot());
```

For plugin load:

```rust
let mut transaction = session_service
    .begin_plugin_load_transaction_with_snapshot(operation_permit.capability_snapshot());
```

- [x] **Step 5: Run GREEN and regression tests**

Run:

```bash
cargo test -p pi-coding-agent prompt_transaction_persists_admitted_snapshot_generation --lib
cargo test -p pi-coding-agent session_events_record_runtime_generation_references --lib
cargo test -p pi-coding-agent session_events_record_capability_generation_references --lib
cargo test -p pi-coding-agent session_service --lib
```

Expected: all selected tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_log/transaction.rs crates/pi-coding-agent/src/coding_session/session_service.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: persist admitted capability snapshot generation"
```

## Task 4: RuntimeService Uses Model And Tool Capabilities

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/prompt_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/prompt.rs`

- [x] **Step 1: Write failing runtime gating tests**

Add these tests to `runtime_service.rs`:

```rust
#[test]
fn runtime_build_rejects_missing_model_capability() {
    let runtime = runtime_snapshot("test-api");
    let snapshot = OperationCapabilitySnapshot {
        generation: CapabilityGeneration::new(1),
        operation_id: "op_runtime".into(),
        actor: ActorId::Client,
        model: None,
        tools: ToolCapabilitySet::default(),
        commands: Default::default(),
        filesystem: None,
        shell: None,
        session_read: None,
        session_write: None,
        ui: None,
        plugin: PluginCapabilitySet::default(),
    };

    let error = RuntimeService::new()
        .build_agent_runtime_with_capabilities(&runtime, &PluginService::new(), &snapshot)
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
    assert!(error.to_string().contains("model capability"));
}

#[test]
fn runtime_build_filters_tools_through_capability_snapshot() {
    let runtime = runtime_snapshot_with_tools(["read", "bash"]);
    let snapshot = OperationCapabilitySnapshot::test_with_tools("op_runtime", ["read"]);

    let build = RuntimeService::new()
        .build_agent_runtime_with_capabilities(&runtime, &PluginService::new(), &snapshot)
        .unwrap();

    assert_eq!(build.tool_names_for_tests(), vec!["read".to_string()]);
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent runtime_build_rejects_missing_model_capability --lib
cargo test -p pi-coding-agent runtime_build_filters_tools_through_capability_snapshot --lib
```

Expected: fail because the capability-aware runtime build method and `AgentRuntimeBuild::tool_names_for_tests()` do not exist.

- [x] **Step 3: Add capability-aware runtime build**

Add this method in `RuntimeService`:

```rust
pub(crate) fn build_agent_runtime_with_capabilities(
    &self,
    runtime: &RuntimeSnapshot,
    plugin_service: &PluginService,
    snapshot: &OperationCapabilitySnapshot,
) -> Result<AgentRuntimeBuild, CodingSessionError> {
    if snapshot.model.is_none() {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: "model capability is required to build agent runtime".into(),
        });
    }

    let provider_streamer = scoped_provider_streamer_for_runtime(runtime);
    let mut diagnostics = runtime.profile_diagnostics().to_vec();
    let resources = apply_skill_policy(runtime, &mut diagnostics);
    let policy_tools = delegation_tools(runtime.profile_id(), runtime.profile_delegation_policy());
    let plugin_tools = plugin_service.collect_tools_with_capabilities(&snapshot.plugin);
    let tools = apply_tool_policy(runtime, plugin_tools, &policy_tools, &mut diagnostics)
        .into_iter()
        .filter(|tool| snapshot.tools.allows(&tool.name))
        .collect::<Vec<_>>();

    let mut config = build_agent_config_with_auth_diagnostics(
        runtime.model().clone(),
        runtime.system_prompt().map(str::to_owned),
        runtime.max_turns(),
        runtime.api_key().map(str::to_owned),
        runtime.auth_diagnostics().to_vec(),
        runtime.thinking_level(),
        runtime.tool_execution(),
        resources,
        runtime.settings(),
    );
    config.provider_streamer = Some(provider_streamer);
    let agent = Agent::new(config);
    for tool in tools.into_iter().chain(policy_tools) {
        if snapshot.tools.allows(&tool.name) {
            agent.try_add_tool(tool).map_err(|error| CodingSessionError::Tool {
                message: error.to_string(),
            })?;
        }
    }
    Ok(AgentRuntimeBuild {
        agent,
        diagnostics,
        #[cfg(test)]
        tool_names,
    })
}
```

Add this test helper field to `AgentRuntimeBuild`:

```rust
#[cfg(test)]
tool_names: Vec<String>,
```

Before consuming `tools`, compute the test-visible names:

```rust
#[cfg(test)]
let tool_names = tools.iter().map(|tool| tool.name.clone()).collect::<Vec<_>>();
```

Add this method:

```rust
#[cfg(test)]
impl AgentRuntimeBuild {
    fn tool_names_for_tests(&self) -> Vec<String> {
        self.tool_names.clone()
    }
}
```

Add this test helper next to the existing `runtime_snapshot()` helper:

```rust
fn runtime_snapshot_with_tools<const N: usize>(tools: [&str; N]) -> RuntimeSnapshot {
    RuntimeSnapshot::from_prompt_run_options(PromptRunOptions {
        prompt: "hello".into(),
        model: model("runtime-service-tools"),
        api_key: Some("key".into()),
        auth_diagnostics: Vec::new(),
        system_prompt: Some("system".into()),
        max_turns: Some(2),
        tools: tools
            .into_iter()
            .map(|name| {
                AgentTool::new_text(
                    name,
                    "test tool",
                    serde_json::json!({"type": "object"}),
                    |_args| async { Ok("ok".to_string()) },
                )
            })
            .collect(),
        register_builtins: false,
        session: Some(SessionRunOptions::disabled(".".into())),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: Some(ToolExecutionMode::Sequential),
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text("hello".into()),
    })
}
```

Keep `build_agent_runtime_with_plugins_and_diagnostics()` as a compatibility wrapper that constructs a permissive snapshot only for current tests and migration paths. Add a source guard in Task 10 to prevent new production calls to the compatibility wrapper.

- [x] **Step 4: Thread the admitted snapshot into `PromptTurnContext`**

Add a field and accessor in `PromptTurnContext`:

```rust
capability_snapshot: Option<OperationCapabilitySnapshot>,
```

Add methods:

```rust
pub(crate) fn set_capability_snapshot(&mut self, snapshot: OperationCapabilitySnapshot) {
    self.capability_snapshot = Some(snapshot);
}

pub(crate) fn capability_snapshot(&self) -> Option<&OperationCapabilitySnapshot> {
    self.capability_snapshot.as_ref()
}
```

In `prompt_flow.rs`, change runtime build:

```rust
let snapshot = ctx.capability_snapshot().ok_or_else(|| {
    CodingSessionError::UnsupportedCapability {
        capability: "prompt runtime build requires operation capability snapshot".into(),
    }
    .to_string()
})?;
let build = service
    .build_agent_runtime_with_capabilities(&runtime, ctx.plugin_service(), snapshot)
    .map_err(|error| error.to_string())?;
```

- [x] **Step 5: Run GREEN and prompt regression tests**

Run:

```bash
cargo test -p pi-coding-agent runtime_build_rejects_missing_model_capability --lib
cargo test -p pi-coding-agent runtime_build_filters_tools_through_capability_snapshot --lib
cargo test -p pi-coding-agent prompt_flow --lib
cargo test -p pi-coding-agent agent_profile_runtime
```

Expected: selected runtime and prompt tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/runtime_service.rs crates/pi-coding-agent/src/coding_session/prompt_flow.rs crates/pi-coding-agent/src/coding_session/prompt.rs
git commit -m "feat: build agent runtime from capability snapshots"
```

## Task 5: Plugin Features Use `PluginCapabilitySet`

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_service.rs`
- Modify: `crates/pi-coding-agent/src/plugins/tool.rs`
- Modify: `crates/pi-coding-agent/src/plugins/command.rs`
- Modify: `crates/pi-coding-agent/src/plugins/hook.rs`
- Modify: `crates/pi-coding-agent/src/plugins/ui.rs`
- Modify: `crates/pi-coding-agent/src/plugins/keybind.rs`

- [x] **Step 1: Write failing plugin capability tests**

Add tests to `plugin_service.rs`:

```rust
#[test]
fn collect_tools_with_capabilities_suppresses_plugin_tools_when_not_granted() {
    let mut registry = PluginRegistry::new();
    registry.register_tool_provider(Arc::new(StaticToolProvider {
        plugin_id: "tools-plugin",
        tool_name: "plugin_echo",
    }));
    let service = PluginService::with_registry(registry);
    let capabilities = PluginCapabilitySet::default();

    let tools = service.collect_tools_with_capabilities(&capabilities);

    assert!(tools.is_empty());
}

#[test]
fn run_command_with_capabilities_rejects_ungranted_commands() {
    let mut registry = PluginRegistry::new();
    registry.register_command_provider(Arc::new(StaticCommandProvider));
    let service = PluginService::with_registry(registry);
    let capabilities = PluginCapabilitySet::default();

    let error = service
        .run_command_with_capabilities("static.command", serde_json::json!({}), &capabilities)
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent collect_tools_with_capabilities_suppresses_plugin_tools_when_not_granted --lib
cargo test -p pi-coding-agent run_command_with_capabilities_rejects_ungranted_commands --lib
```

Expected: fail because capability-aware plugin methods do not exist.

- [x] **Step 3: Add capability-aware plugin collection and execution**

Add methods to `PluginService`:

```rust
pub(crate) fn collect_tools_with_capabilities(
    &self,
    capabilities: &PluginCapabilitySet,
) -> Vec<AgentTool> {
    if capabilities.tool_providers == 0 {
        return Vec::new();
    }
    self.collect_tools()
}

pub(crate) fn run_command_with_capabilities(
    &self,
    command_id: &str,
    args: serde_json::Value,
    capabilities: &PluginCapabilitySet,
) -> Result<String, CodingSessionError> {
    if capabilities.command_providers == 0 {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: format!("plugin command not granted: {command_id}"),
        });
    }
    self.run_command(command_id, args)
}
```

Update hosts so each host stores the capability set:

```rust
#[derive(Debug, Clone)]
pub(crate) struct ToolRegistrationHost {
    capabilities: PluginCapabilitySet,
}

impl ToolRegistrationHost {
    pub(crate) fn new(capabilities: PluginCapabilitySet) -> Self {
        Self { capabilities }
    }

    pub(crate) fn capabilities(&self) -> &PluginCapabilitySet {
        &self.capabilities
    }
}
```

Apply the same `new()` and `capabilities()` shape to command, hook, UI, and keybind hosts.

- [x] **Step 4: Route plugin command operation through the admitted snapshot**

In `CodingAgentSession::run_sync_operation()`, replace:

```rust
self.plugin_service.run_command(&command_id, args)
```

with:

```rust
self.plugin_service.run_command_with_capabilities(
    &command_id,
    args,
    &operation_permit.capability_snapshot().plugin,
)
```

- [x] **Step 5: Run GREEN and plugin tests**

Run:

```bash
cargo test -p pi-coding-agent collect_tools_with_capabilities_suppresses_plugin_tools_when_not_granted --lib
cargo test -p pi-coding-agent run_command_with_capabilities_rejects_ungranted_commands --lib
cargo test -p pi-coding-agent plugin_service --lib
cargo test -p pi-coding-agent rpc_mode plugin --test rpc_mode
```

Expected: selected plugin tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/plugin_service.rs crates/pi-coding-agent/src/plugins/tool.rs crates/pi-coding-agent/src/plugins/command.rs crates/pi-coding-agent/src/plugins/hook.rs crates/pi-coding-agent/src/plugins/ui.rs crates/pi-coding-agent/src/plugins/keybind.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: gate plugin features with capability snapshots"
```

## Task 6: Filesystem And Shell Narrow Handles

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`
- Modify: `crates/pi-coding-agent/src/tools/bash.rs`
- Modify: `crates/pi-coding-agent/src/tools/read.rs`
- Modify: `crates/pi-coding-agent/src/tools/write.rs`
- Modify: `crates/pi-coding-agent/src/tools/edit.rs`
- Modify: `crates/pi-coding-agent/src/tools/grep.rs`
- Modify: `crates/pi-coding-agent/src/tools/find.rs`
- Modify: `crates/pi-coding-agent/src/tools/ls.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs`

- [x] **Step 1: Write failing filesystem and shell tests**

Add tests in `capability_snapshot.rs`:

```rust
#[test]
fn filesystem_handle_rejects_paths_outside_granted_cwd() {
    let capability = FilesystemCapability {
        cwd: std::path::PathBuf::from("/workspace/project"),
    };

    let error = capability.resolve_path("../outside.txt").unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
}

#[test]
fn shell_handle_requires_shell_capability() {
    let snapshot = OperationCapabilitySnapshot::test_without_shell("op_shell");

    let error = ShellCapability::require(snapshot.shell.as_ref()).unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent filesystem_handle_rejects_paths_outside_granted_cwd --lib
cargo test -p pi-coding-agent shell_handle_requires_shell_capability --lib
```

Expected: fail because narrow handle methods do not exist.

- [x] **Step 3: Add narrow handle helpers**

Add to `FilesystemCapability`:

```rust
pub(crate) fn resolve_path(&self, path: impl AsRef<std::path::Path>) -> Result<std::path::PathBuf, CodingSessionError> {
    let joined = self.cwd.join(path);
    let normalized = joined.components().collect::<std::path::PathBuf>();
    if !normalized.starts_with(&self.cwd) {
        return Err(CodingSessionError::UnsupportedCapability {
            capability: format!("filesystem path outside granted cwd: {}", normalized.display()),
        });
    }
    Ok(normalized)
}
```

Add to `ShellCapability`:

```rust
pub(crate) fn require(value: Option<&ShellCapability>) -> Result<&ShellCapability, CodingSessionError> {
    value.ok_or_else(|| CodingSessionError::UnsupportedCapability {
        capability: "shell capability is not granted".into(),
    })
}
```

- [x] **Step 4: Thread handles into builtin tools**

Change builtin tool constructors so tool closures capture the relevant capability:

```rust
pub(crate) fn bash_tool(shell: ShellCapability, options: BashOptions) -> AgentTool {
    AgentTool::new_json("bash", DESCRIPTION, schema(), move |args, on_update| {
        let shell = shell.clone();
        let options = options.clone();
        async move {
            RealBashOperations
                .execute(&shell.cwd, args, &options, on_update)
                .await
                .map(AgentToolOutput::new)
                .map_err(AgentToolResult::error)
        }
    })
}
```

Apply the same pattern to read/write/edit/grep/find/ls: pass `FilesystemCapability`, call `resolve_path()`, and reject ungranted filesystem access with `unsupported_capability`.

- [x] **Step 5: Update self-healing edit operations**

Change `ExecutionEnvEditOperations` to store `FilesystemCapability`:

```rust
struct ExecutionEnvEditOperations<E> {
    env: E,
    filesystem: FilesystemCapability,
}
```

Construct it from the operation snapshot:

```rust
let filesystem = snapshot.filesystem.clone().ok_or_else(|| CodingSessionError::UnsupportedCapability {
    capability: "self-healing edit requires filesystem capability".into(),
})?;
```

- [x] **Step 6: Run GREEN and tool tests**

Run:

```bash
cargo test -p pi-coding-agent filesystem_handle_rejects_paths_outside_granted_cwd --lib
cargo test -p pi-coding-agent shell_handle_requires_shell_capability --lib
cargo test -p pi-coding-agent tool_read
cargo test -p pi-coding-agent tool_write
cargo test -p pi-coding-agent tool_edit
cargo test -p pi-coding-agent tool_bash
cargo test -p pi-coding-agent self_healing_edit --lib
```

Expected: selected capability and tool tests pass.

- [x] **Step 7: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/capability_snapshot.rs crates/pi-coding-agent/src/tools/bash.rs crates/pi-coding-agent/src/tools/read.rs crates/pi-coding-agent/src/tools/write.rs crates/pi-coding-agent/src/tools/edit.rs crates/pi-coding-agent/src/tools/grep.rs crates/pi-coding-agent/src/tools/find.rs crates/pi-coding-agent/src/tools/ls.rs crates/pi-coding-agent/src/coding_session/self_healing_edit_flow.rs
git commit -m "feat: gate filesystem and shell tools with capability handles"
```

## Task 7: Session Read And Write Capabilities

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/session_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/manual_compaction_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/branch_summary_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/self_healing_edit_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`

- [x] **Step 1: Write failing session capability tests**

Add tests to `session_service.rs`:

```rust
#[test]
fn session_write_requires_session_write_capability() {
    let temp = tempfile::tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_write_capability")
        .with_session_log_root(temp.path());
    let mut service = SessionService::create(&options).unwrap();
    let snapshot = OperationCapabilitySnapshot::test_without_session_write("op_write");
    let transaction = service.begin_prompt_transaction_with_snapshot(&snapshot);
    let operation_id = transaction.operation_id().to_owned();

    let error = service
        .commit_prompt_transaction_with_snapshot(Some(transaction), operation_id, &snapshot)
        .unwrap_err();

    assert_eq!(error.code(), "unsupported_capability");
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent session_write_requires_session_write_capability --lib
```

Expected: fail because snapshot-aware finalization does not exist.

- [x] **Step 3: Add read/write guards**

Add helper methods to `SessionReadCapability` and `SessionWriteCapability`:

```rust
pub(crate) fn require(value: Option<&SessionWriteCapability>) -> Result<&SessionWriteCapability, CodingSessionError> {
    value.ok_or_else(|| CodingSessionError::UnsupportedCapability {
        capability: "session write capability is not granted".into(),
    })
}
```

Add snapshot-aware finalization methods:

```rust
pub(crate) fn commit_prompt_transaction_with_snapshot(
    &mut self,
    transaction: Option<PromptTurnTransaction>,
    operation_id: impl Into<String>,
    snapshot: &OperationCapabilitySnapshot,
) -> Result<FinalizedSessionWrite, CodingSessionError> {
    SessionWriteCapability::require(snapshot.session_write.as_ref())?;
    self.commit_prompt_transaction(transaction, operation_id)
}
```

Add read guards to export/tree/hydration entrypoints:

```rust
SessionReadCapability::require(snapshot.session_read.as_ref())?;
```

- [x] **Step 4: Route workflow services through guarded methods**

Update manual compaction, branch summary, self-healing edit, prompt, and fork paths so they call the snapshot-aware methods when an `OperationPermit` is available.

For prompt:

```rust
let finalized = session_service.commit_prompt_transaction_with_snapshot(
    Some(transaction),
    operation_id,
    operation_permit.capability_snapshot(),
)?;
```

For read-only export:

```rust
let replay = session_service.replay_with_snapshot(operation_permit.capability_snapshot())?;
```

- [x] **Step 5: Run GREEN and workflow tests**

Run:

```bash
cargo test -p pi-coding-agent session_write_requires_session_write_capability --lib
cargo test -p pi-coding-agent session_service --lib
cargo test -p pi-coding-agent interactive_sessions
cargo test -p pi-coding-agent rpc_mode
```

Expected: session service, interactive session, and RPC mode tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/session_service.rs crates/pi-coding-agent/src/coding_session/manual_compaction_service.rs crates/pi-coding-agent/src/coding_session/branch_summary_service.rs crates/pi-coding-agent/src/coding_session/self_healing_edit_service.rs crates/pi-coding-agent/src/coding_session/mod.rs
git commit -m "feat: gate session access with capability snapshots"
```

## Task 8: Runtime Writes Install Generations And Emit Revocation Semantics

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `crates/pi-coding-agent/src/protocol/events.rs`
- Modify: `crates/pi-coding-agent/src/protocol/types.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/plugin_load_service.rs`

- [x] **Step 1: Write failing runtime-write generation tests**

Add tests to `coding_session/mod.rs`:

```rust
#[tokio::test]
async fn set_default_profile_installs_future_capability_generation() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_generation_profile")
            .with_session_log_root(temp.path()),
    )
    .await
    .unwrap();
    let first = session.current_capability_generation_for_tests();

    session.set_default_agent_profile_id("reviewer").unwrap();
    let second = session.current_capability_generation_for_tests();

    assert_eq!(first.get() + 1, second.get());
}
```

Add tests to `event_service.rs`:

```rust
#[test]
fn capability_changed_event_carries_generation_and_revocation_policy() {
    let event = CodingAgentEvent::CapabilityChanged {
        generation: 2,
        revocation: CapabilityRevocationPolicy::FutureOnly,
    };

    assert_eq!(
        event.classification().family,
        ProductEventFamily::Capability
    );
}
```

- [x] **Step 2: Run RED tests**

Run:

```bash
cargo test -p pi-coding-agent set_default_profile_installs_future_capability_generation --lib
cargo test -p pi-coding-agent capability_changed_event_carries_generation_and_revocation_policy --lib
```

Expected: fail because generation installation and payloaded capability events do not exist.

- [x] **Step 3: Install generations for RuntimeWrite operations**

In `CodingAgentSession::run_sync_mut_operation()`, after `SetDefaultAgentProfile` succeeds:

```rust
let installed = self
    .capability_snapshots
    .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
self.event_service.emit_capability_changed(installed);
```

In plugin load success path, when `outcome.capability_changed` is true:

```rust
let installed = self
    .capability_snapshots
    .install_next_generation(CapabilityRevocationPolicy::FutureOnly);
self.event_service.emit_capability_changed(installed);
```

- [x] **Step 4: Update product and protocol events**

Change `CodingAgentEvent::CapabilityChanged` to:

```rust
CapabilityChanged {
    generation: u64,
    revocation: CapabilityRevocationPolicy,
},
```

Add `EventService::emit_capability_changed()`:

```rust
pub(crate) fn emit_capability_changed(&self, installed: InstalledCapabilityGeneration) {
    self.emit(CodingAgentEvent::CapabilityChanged {
        generation: installed.generation.get(),
        revocation: installed.revocation,
    });
}
```

Update protocol mapping so existing clients receive the same semantic event name plus additive fields:

```rust
ProtocolEvent::CapabilityChanged {
    generation,
    revocation: revocation.as_str().to_owned(),
}
```

- [x] **Step 5: Run GREEN and protocol tests**

Run:

```bash
cargo test -p pi-coding-agent set_default_profile_installs_future_capability_generation --lib
cargo test -p pi-coding-agent capability_changed_event_carries_generation_and_revocation_policy --lib
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent protocol_events
```

Expected: event service and protocol event tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/mod.rs crates/pi-coding-agent/src/coding_session/event.rs crates/pi-coding-agent/src/coding_session/event_service.rs crates/pi-coding-agent/src/protocol/events.rs crates/pi-coding-agent/src/protocol/types.rs crates/pi-coding-agent/src/coding_session/plugin_load_service.rs
git commit -m "feat: emit capability generation changes"
```

## Task 9: Delegation And Child Operation Capability Release

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/delegation.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/delegation_execution_service.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/agent_invocation_flow.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/agent_team_flow.rs`

- [x] **Step 1: Write failing delegation release tests**

Add test to `delegation_execution_service.rs`:

```rust
#[tokio::test]
async fn delegated_operation_receives_released_tool_capabilities_only() {
    let parent = OperationCapabilitySnapshot::test_with_tools("op_parent", ["read", "bash"]);
    let target_profile = AgentProfile {
        schema_version: 1,
        id: ProfileId::from("coder"),
        display_name: "Coder".into(),
        description: None,
        model: None,
        system_prompt: None,
        tools: vec!["read".into()],
        skills: Vec::new(),
        supervision: SupervisionPolicy::Session,
        delegation: DelegationPolicy::default(),
        source: ProfileSource::BuiltIn,
        path: None,
    };

    let child = capability_snapshot_for_delegated_profile(
        &parent,
        "op_child",
        &target_profile,
        ActorId::ChildOperation("op_parent".into()),
    );

    assert!(child.tools.allows("read"));
    assert!(!child.tools.allows("bash"));
    assert_eq!(child.generation, parent.generation);
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
cargo test -p pi-coding-agent delegated_operation_receives_released_tool_capabilities_only --lib
```

Expected: fail because child snapshot derivation does not exist.

- [x] **Step 3: Add child snapshot derivation**

Add this helper in `delegation.rs`:

```rust
pub(crate) fn capability_snapshot_for_delegated_profile(
    parent: &OperationCapabilitySnapshot,
    operation_id: impl Into<String>,
    profile: &AgentProfile,
    actor: ActorId,
) -> OperationCapabilitySnapshot {
    let released_tools = parent
        .tools
        .names()
        .filter(|name| profile.tools.iter().any(|allowed| allowed == name))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    OperationCapabilitySnapshot {
        generation: parent.generation,
        operation_id: operation_id.into(),
        actor,
        model: Some(ModelCapability {
            profile_id: Some(profile.id.clone()),
        }),
        tools: ToolCapabilitySet::from_names(released_tools),
        commands: Default::default(),
        filesystem: parent.filesystem.clone(),
        shell: parent.shell.clone().filter(|_| profile.tools.iter().any(|name| name == "bash")),
        session_read: None,
        session_write: None,
        ui: None,
        plugin: parent.plugin.clone(),
    }
}
```

- [x] **Step 4: Thread child snapshots into agent/team flows**

When invoking delegated agent/team members, derive the child snapshot from the parent snapshot and store it in the delegated `PromptTurnContext`:

```rust
let child_snapshot = capability_snapshot_for_delegated_profile(
    parent_snapshot,
    child_operation_id.clone(),
    target_profile,
    ActorId::ChildOperation(parent_snapshot.operation_id.clone()),
);
context.set_capability_snapshot(child_snapshot);
```

- [x] **Step 5: Run GREEN and delegation tests**

Run:

```bash
cargo test -p pi-coding-agent delegated_operation_receives_released_tool_capabilities_only --lib
cargo test -p pi-coding-agent delegation_execution
cargo test -p pi-coding-agent agent_invocation
cargo test -p pi-coding-agent agent_team_flow
```

Expected: delegation, agent invocation, and team flow tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/src/coding_session/delegation.rs crates/pi-coding-agent/src/coding_session/delegation_execution_service.rs crates/pi-coding-agent/src/coding_session/agent_invocation_flow.rs crates/pi-coding-agent/src/coding_session/agent_team_flow.rs
git commit -m "feat: derive delegated capability snapshots"
```

## Task 10: Boundary Guards And Compatibility Removal

**Files:**
- Modify: `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/tool_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/tests/session_boundary_guards.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/runtime_service.rs`
- Modify: `docs/TODO.md`

- [x] **Step 1: Write failing source guards**

Add guards:

```rust
#[test]
fn runtime_service_production_paths_require_capability_snapshot() {
    let source = include_str!("../src/coding_session/runtime_service.rs");
    assert!(
        !source.contains("build_agent_runtime_with_plugins_and_diagnostics(&runtime, ctx.plugin_service())"),
        "prompt runtime build must pass OperationCapabilitySnapshot"
    );
}

#[test]
fn plugin_command_paths_use_capability_aware_execution() {
    let source = include_str!("../src/coding_session/mod.rs");
    assert!(
        !source.contains(".run_command(&command_id, args)"),
        "plugin command execution must use run_command_with_capabilities"
    );
}
```

- [x] **Step 2: Run RED guards**

Run:

```bash
cargo test -p pi-coding-agent runtime_service_production_paths_require_capability_snapshot --test product_runtime_boundary_guards
cargo test -p pi-coding-agent plugin_command_paths_use_capability_aware_execution --test product_runtime_boundary_guards
```

Expected: fail if any production path still bypasses snapshots.

- [x] **Step 3: Remove compatibility production usage**

Keep compatibility helpers only under tests or mark them `#[cfg(test)]`. Production runtime build, plugin command execution, and session writes must use snapshot-aware methods.

Use this source shape:

```rust
#[cfg(test)]
pub(crate) fn build_agent_runtime_with_plugins_and_diagnostics(
    &self,
    runtime: &RuntimeSnapshot,
    plugin_service: &PluginService,
) -> Result<AgentRuntimeBuild, CodingSessionError> {
    let snapshot = OperationCapabilitySnapshot::permissive_for_tests("op_test");
    self.build_agent_runtime_with_capabilities(runtime, plugin_service, &snapshot)
}
```

- [x] **Step 4: Update TODO progress**

Add this progress log entry to `docs/TODO.md`:

```markdown
- 2026-07-09: Stage 5 capability snapshot integration completed. Operation admission now freezes `OperationCapabilitySnapshot`, runtime and plugin execution consume snapshot-derived capabilities, filesystem/shell/session access uses narrow handles, runtime writes install new capability generations with capability-change events, delegated child operations inherit explicitly released capabilities, and boundary guards prevent production paths from bypassing the snapshot model.
```

- [x] **Step 5: Run boundary and focused regression tests**

Run:

```bash
cargo test -p pi-coding-agent product_runtime_boundary_guards
cargo test -p pi-coding-agent tool_boundary_guards
cargo test -p pi-coding-agent session_boundary_guards
cargo test -p pi-coding-agent capability_snapshot --lib
```

Expected: boundary guards and snapshot model tests pass.

- [x] **Step 6: Commit**

```bash
git add crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs crates/pi-coding-agent/tests/tool_boundary_guards.rs crates/pi-coding-agent/tests/session_boundary_guards.rs crates/pi-coding-agent/src/coding_session/runtime_service.rs docs/TODO.md
git commit -m "test: guard capability snapshot boundaries"
```

## Task 11: Stage 5 Verification And Closure

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-09-capability-snapshot-integration-plan.md`

- [x] **Step 1: Run full Stage 5 verification**

Run:

```bash
cargo fmt --check
cargo test -p pi-coding-agent capability_snapshot --lib
cargo test -p pi-coding-agent intent_router --lib
cargo test -p pi-coding-agent runtime_service --lib
cargo test -p pi-coding-agent plugin_service --lib
cargo test -p pi-coding-agent session_service --lib
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent delegation_execution
cargo test -p pi-coding-agent agent_invocation
cargo test -p pi-coding-agent agent_team_flow
cargo test -p pi-coding-agent tool_bash
cargo test -p pi-coding-agent tool_edit
cargo check -p pi-coding-agent
git diff --check
```

Expected: every command exits with code 0.

- [x] **Step 2: Update this plan's verification checklist**

After the commands pass, mark these checkboxes:

```markdown
- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent capability_snapshot --lib`
- [x] `cargo test -p pi-coding-agent intent_router --lib`
- [x] `cargo test -p pi-coding-agent runtime_service --lib`
- [x] `cargo test -p pi-coding-agent plugin_service --lib`
- [x] `cargo test -p pi-coding-agent session_service --lib`
- [x] `cargo test -p pi-coding-agent event_service --lib`
- [x] `cargo test -p pi-coding-agent protocol_events`
- [x] `cargo test -p pi-coding-agent delegation_execution`
- [x] `cargo test -p pi-coding-agent agent_invocation`
- [x] `cargo test -p pi-coding-agent agent_team_flow`
- [x] `cargo test -p pi-coding-agent tool_bash`
- [x] `cargo test -p pi-coding-agent tool_edit`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`
```

- [x] **Step 3: Update `docs/TODO.md` top-level architecture status**

Replace the Stage 5 portion of the active architecture item with:

```markdown
Stage 5 capability snapshot integration is complete: operation admission freezes `OperationCapabilitySnapshot`; model/provider, tool, plugin, filesystem, shell, and session access consume snapshot-derived narrow handles; runtime writes install capability generations and emit capability-change events; delegated child operations inherit only explicitly released capabilities; and source guards keep production paths from bypassing snapshots.
```

- [x] **Step 4: Commit closure documentation**

```bash
git add docs/TODO.md docs/superpowers/plans/2026-07-09-capability-snapshot-integration-plan.md
git commit -m "docs: close capability snapshot integration stage"
```

## Verification Checklist

- [x] `cargo fmt --check`
- [x] `cargo test -p pi-coding-agent capability_snapshot --lib`
- [x] `cargo test -p pi-coding-agent intent_router --lib`
- [x] `cargo test -p pi-coding-agent runtime_service --lib`
- [x] `cargo test -p pi-coding-agent plugin_service --lib`
- [x] `cargo test -p pi-coding-agent session_service --lib`
- [x] `cargo test -p pi-coding-agent event_service --lib`
- [x] `cargo test -p pi-coding-agent protocol_events`
- [x] `cargo test -p pi-coding-agent delegation_execution`
- [x] `cargo test -p pi-coding-agent agent_invocation`
- [x] `cargo test -p pi-coding-agent agent_team_flow`
- [x] `cargo test -p pi-coding-agent tool_bash`
- [x] `cargo test -p pi-coding-agent tool_edit`
- [x] `cargo check -p pi-coding-agent`
- [x] `git diff --check`

## Post-Closure Quality Review

2026-07-09 quality review found and fixed three authorization consistency gaps
without changing the Stage 5 contract:

- Filesystem capability classification now covers every builtin filesystem tool:
  `read`, `write`, `edit`, `grep`, `find`, and `ls`.
- Delegated child snapshots no longer inherit a filesystem handle unless at least
  one released target-profile tool uses filesystem access.
- Branch-summary navigation reuse now obtains an admitted branch-summary snapshot
  before checking for reusable cached summaries, and cached-summary reads require
  `SessionReadCapability`.

Focused regression coverage:

- `prompt_snapshot_grants_filesystem_for_every_filesystem_tool`
- `delegated_operation_does_not_release_filesystem_without_filesystem_tools`
- `reused_outcome_requires_session_read_capability`

## Spec Coverage

- Model/provider access uses `ModelCapability`: Tasks 1, 4.
- Filesystem access uses `FilesystemCapability`: Tasks 1, 6.
- Shell access uses `ShellCapability`: Tasks 1, 6.
- Tool execution uses `ToolCapabilitySet`: Tasks 1, 4, 6.
- Plugin host calls use `PluginCapabilitySet`: Tasks 1, 5.
- Session read/write uses `SessionReadCapability` and `SessionWriteCapability`: Tasks 1, 7.
- Runtime mutations emit capability generation changes and revocation semantics: Task 8.
- Active operations do not silently observe mid-run capability changes: Tasks 2, 8, 9.
- Plugins/tools do not receive raw runtime/session/provider/auth services: Tasks 5, 6, 10.
