use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::CodingSessionError;
use super::operation_control::OperationKind;
use super::profiles::ProfileId;
use super::session_log::event::PersistedRuntimeGenerationRef;
use super::snapshot_coordinator::SnapshotCoordinator;
use crate::plugins::PluginCapabilities;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CapabilityGeneration(u64);

impl CapabilityGeneration {
    pub(crate) fn new(value: u64) -> Self {
        Self(value.max(1))
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActorId {
    Client,
    ChildOperation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelCapability {
    pub(crate) profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ToolCapabilitySet {
    allow_all: bool,
    allowed: BTreeSet<String>,
}

impl ToolCapabilitySet {
    pub(crate) fn from_names(names: impl IntoIterator<Item = String>) -> Self {
        Self {
            allow_all: false,
            allowed: names.into_iter().collect(),
        }
    }

    pub(crate) fn allows(&self, name: &str) -> bool {
        self.allow_all || self.allowed.contains(name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CommandCapabilitySet {
    allowed: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCapability {
    pub(crate) cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCapability {
    pub(crate) cwd: PathBuf,
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut stack: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                _ => stack.push(component),
            },
            Component::CurDir => {}
            other => stack.push(other),
        }
    }
    let mut result = PathBuf::new();
    for component in stack {
        result.push(component.as_os_str());
    }
    result
}

pub(crate) fn tool_uses_filesystem(name: &str) -> bool {
    matches!(name, "read" | "write" | "edit" | "grep" | "find" | "ls")
}

impl FilesystemCapability {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub(crate) fn resolve_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        use crate::tools::path::resolve_to_cwd;
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();
        let resolved = resolve_to_cwd(&path_str, &self.cwd);
        let was_relative = !path_ref.is_absolute() && !path_str.starts_with('~');
        if was_relative {
            let normalized = lexically_normalize(&resolved);
            let normalized_cwd = lexically_normalize(&self.cwd);
            if !normalized.starts_with(&normalized_cwd) {
                return Err(CodingSessionError::UnsupportedCapability {
                    capability: format!(
                        "filesystem path escapes granted cwd: {}",
                        normalized.display()
                    ),
                });
            }
            return Ok(normalized);
        }
        Ok(resolved)
    }
}

impl ShellCapability {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReadCapability {
    pub(crate) persistent: bool,
}

impl SessionReadCapability {
    pub(crate) fn require(
        value: Option<&SessionReadCapability>,
    ) -> Result<&SessionReadCapability, CodingSessionError> {
        value.ok_or_else(|| CodingSessionError::UnsupportedCapability {
            capability: "session read capability is not granted".into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionWriteCapability {
    pub(crate) persistent: bool,
}

impl SessionWriteCapability {
    pub(crate) fn require(
        value: Option<&SessionWriteCapability>,
    ) -> Result<&SessionWriteCapability, CodingSessionError> {
        value.ok_or_else(|| CodingSessionError::UnsupportedCapability {
            capability: "session write capability is not granted".into(),
        })
    }
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
            profile_id: self
                .model
                .as_ref()
                .and_then(|model| model.profile_id.clone()),
            capability_generation: Some(self.generation.get()),
        }
    }

    pub(crate) fn permissive(operation_id: impl Into<String>) -> Self {
        Self {
            generation: CapabilityGeneration::new(1),
            operation_id: operation_id.into(),
            actor: ActorId::Client,
            model: Some(ModelCapability { profile_id: None }),
            tools: ToolCapabilitySet {
                allow_all: true,
                allowed: BTreeSet::new(),
            },
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
}

#[cfg(test)]
impl OperationCapabilitySnapshot {
    pub(crate) fn test_without_session_write(operation_id: impl Into<String>) -> Self {
        let mut snapshot = Self::permissive(operation_id);
        snapshot.session_write = None;
        snapshot
    }

    pub(crate) fn test_with_tools(
        operation_id: impl Into<String>,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut snapshot = Self::permissive(operation_id);
        snapshot.tools = ToolCapabilitySet::from_names(names.into_iter().map(Into::into));
        snapshot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRevocationPolicy {
    FutureOnly,
}

impl CapabilityRevocationPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FutureOnly => "future_only",
        }
    }
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
    snapshot_coordinator: Arc<SnapshotCoordinator>,
}

impl CapabilitySnapshotService {
    pub(crate) fn new() -> Self {
        Self::with_snapshot_coordinator(SnapshotCoordinator::new())
    }

    pub(crate) fn with_snapshot_coordinator(
        snapshot_coordinator: Arc<SnapshotCoordinator>,
    ) -> Self {
        Self {
            snapshot_coordinator,
        }
    }

    pub(crate) fn current_generation(&self) -> CapabilityGeneration {
        self.snapshot_coordinator.current_capability_generation()
    }

    pub(crate) fn install_next_generation(
        &mut self,
        revocation: CapabilityRevocationPolicy,
    ) -> InstalledCapabilityGeneration {
        InstalledCapabilityGeneration {
            generation: self
                .snapshot_coordinator
                .install_next_capability_generation(),
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
            | OperationKind::Compact
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
            .filter(|_| allowed_tools.iter().any(|name| tool_uses_filesystem(name)))
            .map(|cwd| FilesystemCapability { cwd: cwd.clone() });
        let shell = cwd
            .as_ref()
            .filter(|_| allowed_tools.iter().any(|name| name == "bash"))
            .map(|cwd| ShellCapability { cwd: cwd.clone() });
        OperationCapabilitySnapshot {
            generation: self.current_generation(),
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
    fn prompt_snapshot_grants_filesystem_for_every_filesystem_tool() {
        let service = CapabilitySnapshotService::new();
        for tool in ["read", "write", "edit", "grep", "find", "ls"] {
            let mut input = input(OperationKind::Prompt);
            input.runtime_tools = vec![tool.into()];
            input.profile_tools = vec![tool.into()];

            let snapshot = service.snapshot(input);

            assert!(
                snapshot.filesystem.is_some(),
                "{tool} should receive filesystem capability"
            );
            assert!(
                snapshot.tools.allows(tool),
                "{tool} should remain tool-authorized"
            );
        }
    }

    #[test]
    fn compact_snapshot_grants_model_and_session_write() {
        let snapshot = CapabilitySnapshotService::new().snapshot(input(OperationKind::Compact));
        assert!(snapshot.model.is_some());
        assert!(snapshot.session_write.is_some());
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

    #[test]
    fn export_snapshot_grants_read_without_write_or_model() {
        let service = CapabilitySnapshotService::new();

        let snapshot = service.snapshot(input(OperationKind::Export));

        assert!(snapshot.session_read.is_some());
        assert!(snapshot.session_write.is_none());
        assert!(snapshot.model.is_none());
        assert!(snapshot.filesystem.is_some());
        assert!(snapshot.shell.is_none());
        assert!(snapshot.tools.allows("read"));
        assert!(snapshot.tools.allows("edit"));
        assert!(!snapshot.tools.allows("bash"));
    }

    #[test]
    fn filesystem_resolve_path_keeps_absolute_paths() {
        let capability = FilesystemCapability {
            cwd: std::path::PathBuf::from("/workspace/project"),
        };
        let resolved = capability.resolve_path("/etc/hosts").unwrap();
        assert_eq!(resolved, std::path::PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn filesystem_resolve_path_rejects_dotdot_escape() {
        let capability = FilesystemCapability {
            cwd: std::path::PathBuf::from("/workspace/project"),
        };
        let error = capability.resolve_path("../outside.txt").unwrap_err();
        assert_eq!(error.code(), "unsupported_capability");
    }

    #[test]
    fn filesystem_resolve_path_allows_relative_within_cwd() {
        let capability = FilesystemCapability {
            cwd: std::path::PathBuf::from("/workspace/project"),
        };
        let resolved = capability.resolve_path("src/main.rs").unwrap();
        assert_eq!(
            resolved,
            std::path::PathBuf::from("/workspace/project/src/main.rs")
        );
    }
}
