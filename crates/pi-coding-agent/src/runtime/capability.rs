use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::control::OperationKind;
use super::snapshot::SnapshotCoordinator;
use crate::plugins::PluginCapabilities;
use crate::profiles::ProfileId;
use crate::runtime::facade::CodingSessionError;
use crate::session::event::PersistedRuntimeGenerationRef;
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

impl ModelCapability {
    pub(crate) fn require<'a>(
        value: Option<&'a ModelCapability>,
        runtime_profile_id: Option<&ProfileId>,
    ) -> Result<&'a ModelCapability, CodingSessionError> {
        let capability = value.ok_or_else(|| CodingSessionError::UnsupportedCapability {
            capability: "model capability is not granted".into(),
        })?;
        if capability.profile_id.as_ref() != runtime_profile_id {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: format!(
                    "model capability profile mismatch: granted={}, runtime={}",
                    capability
                        .profile_id
                        .as_ref()
                        .map(ProfileId::as_str)
                        .unwrap_or("<none>"),
                    runtime_profile_id
                        .map(ProfileId::as_str)
                        .unwrap_or("<none>")
                ),
            });
        }
        Ok(capability)
    }
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
    pub(crate) shell_path: Option<String>,
    pub(crate) command_prefix: Option<String>,
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
        use crate::tools::filesystem::path::resolve_to_cwd;
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
        Self {
            cwd,
            shell_path: None,
            command_prefix: None,
        }
    }

    pub(crate) fn with_configuration(
        cwd: PathBuf,
        shell_path: Option<String>,
        command_prefix: Option<String>,
    ) -> Self {
        Self {
            cwd,
            shell_path,
            command_prefix,
        }
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
    allow_all: bool,
}

impl PluginCapabilitySet {
    pub(crate) fn permissive() -> Self {
        Self { allow_all: true }
    }

    pub(crate) fn is_permissive(&self) -> bool {
        self.allow_all
    }
}

impl From<&PluginCapabilities> for PluginCapabilitySet {
    fn from(_value: &PluginCapabilities) -> Self {
        Self::default()
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
                shell_path: None,
                command_prefix: None,
            }),
            session_read: Some(SessionReadCapability { persistent: true }),
            session_write: Some(SessionWriteCapability { persistent: true }),
            ui: Some(UiCapability),
            plugin: PluginCapabilitySet::permissive(),
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
    RequestCancelOlderOperations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledCapabilityGeneration {
    pub(crate) generation: CapabilityGeneration,
    pub(crate) revocation: CapabilityRevocationPolicy,
    pub(crate) cancellation_requested_operation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapabilitySnapshotInput {
    pub(crate) operation_id: String,
    pub(crate) operation_kind: OperationKind,
    pub(crate) session_access: SessionCapabilityAccess,
    pub(crate) actor: ActorId,
    pub(crate) uses_model: bool,
    pub(crate) model_profile_id: Option<ProfileId>,
    pub(crate) plugin_capabilities: PluginCapabilities,
    pub(crate) persistent_session: bool,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) shell_path: Option<String>,
    pub(crate) shell_command_prefix: Option<String>,
    pub(crate) runtime_tools: Vec<String>,
    pub(crate) profile_tools: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionCapabilityAccess {
    None,
    Read,
    Write,
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
            cancellation_requested_operation_ids: Vec::new(),
        }
    }

    pub(crate) fn snapshot(&self, input: CapabilitySnapshotInput) -> OperationCapabilitySnapshot {
        let writes_session = matches!(input.session_access, SessionCapabilityAccess::Write);
        let reads_session = !matches!(input.session_access, SessionCapabilityAccess::None);
        let model = input.uses_model.then_some(ModelCapability {
            profile_id: input.model_profile_id,
        });
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
            .map(|cwd| {
                ShellCapability::with_configuration(
                    cwd.clone(),
                    input.shell_path,
                    input.shell_command_prefix,
                )
            });
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
    use crate::plugins::PluginCapabilities;
    use crate::profiles::ProfileId;
    use crate::runtime::control::OperationKind;

    fn input(kind: OperationKind) -> CapabilitySnapshotInput {
        CapabilitySnapshotInput {
            operation_id: "op_snapshot".into(),
            operation_kind: kind,
            session_access: match kind {
                OperationKind::Export => SessionCapabilityAccess::Read,
                OperationKind::Prompt
                | OperationKind::Compact
                | OperationKind::BranchSummary
                | OperationKind::ForkSession
                | OperationKind::SwitchActiveLeaf
                | OperationKind::SetSessionTreeLabel
                | OperationKind::SetDefaultAgentProfile
                | OperationKind::SelfHealingEdit => SessionCapabilityAccess::Write,
                OperationKind::PluginCommand
                | OperationKind::PluginLoad
                | OperationKind::DelegationConfirmation
                | OperationKind::AgentInvocation
                | OperationKind::AgentTeam => SessionCapabilityAccess::None,
            },
            actor: ActorId::Client,
            uses_model: matches!(
                kind,
                OperationKind::Prompt
                    | OperationKind::Compact
                    | OperationKind::BranchSummary
                    | OperationKind::AgentInvocation
                    | OperationKind::AgentTeam
                    | OperationKind::SelfHealingEdit
            ),
            model_profile_id: Some(ProfileId::from("reviewer")),
            plugin_capabilities: PluginCapabilities::new(),
            persistent_session: true,
            cwd: Some(std::path::PathBuf::from("/workspace")),
            shell_path: None,
            shell_command_prefix: None,
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
        assert!(!snapshot.plugin.is_permissive());
    }

    #[test]
    fn shell_snapshot_freezes_execution_configuration() {
        let mut input = input(OperationKind::Prompt);
        input.profile_tools.push("bash".into());
        input.shell_path = Some("/custom/bash".into());
        input.shell_command_prefix = Some("source ./env.sh".into());

        let snapshot = CapabilitySnapshotService::new().snapshot(input);
        let shell = snapshot.shell.expect("bash capability should be granted");

        assert_eq!(shell.cwd, PathBuf::from("/workspace"));
        assert_eq!(shell.shell_path.as_deref(), Some("/custom/bash"));
        assert_eq!(shell.command_prefix.as_deref(), Some("source ./env.sh"));
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
    fn explicit_session_access_is_not_inferred_from_operation_kind() {
        let service = CapabilitySnapshotService::new();
        let mut request = input(OperationKind::AgentInvocation);
        request.session_access = SessionCapabilityAccess::Write;

        let snapshot = service.snapshot(request);

        assert!(snapshot.session_read.is_some());
        assert!(snapshot.session_write.is_some());
    }

    #[test]
    fn model_capability_requires_a_grant_for_the_exact_runtime_profile() {
        let reviewer = ProfileId::from("reviewer");
        let builder = ProfileId::from("builder");
        let capability = ModelCapability {
            profile_id: Some(reviewer.clone()),
        };

        assert!(ModelCapability::require(Some(&capability), Some(&reviewer)).is_ok());
        let missing = ModelCapability::require(None, Some(&reviewer)).unwrap_err();
        let mismatch = ModelCapability::require(Some(&capability), Some(&builder)).unwrap_err();

        assert_eq!(missing.code(), "unsupported_capability");
        assert!(
            missing
                .to_string()
                .contains("model capability is not granted")
        );
        assert_eq!(mismatch.code(), "unsupported_capability");
        assert!(
            mismatch
                .to_string()
                .contains("model capability profile mismatch")
        );
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
