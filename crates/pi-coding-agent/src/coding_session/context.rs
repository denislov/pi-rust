use std::path::{Path, PathBuf};

use pi_agent_core::api::SessionTreeNode;
use pi_ai::api::AiClient;

use crate::coding_session::operation_control::OperationKind;
use crate::coding_session::profiles::{ProfileId, ProfileKind};
use crate::plugins::PluginCapabilities;

#[derive(Clone, Default)]
pub struct CodingAgentSessionOptions {
    cwd: Option<PathBuf>,
    session_id: Option<String>,
    session_log_root: Option<PathBuf>,
    session_path: Option<PathBuf>,
    default_agent_profile_id: Option<ProfileId>,
    ai_client: Option<AiClient>,
}

impl std::fmt::Debug for CodingAgentSessionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingAgentSessionOptions")
            .field("cwd", &self.cwd)
            .field("session_id", &self.session_id)
            .field("session_log_root", &self.session_log_root)
            .field("session_path", &self.session_path)
            .field("default_agent_profile_id", &self.default_agent_profile_id)
            .field("has_scoped_ai_client", &self.ai_client.is_some())
            .finish()
    }
}

impl CodingAgentSessionOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_session_log_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.session_log_root = Some(root.into());
        self
    }

    pub fn with_session_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.session_path = Some(path.into());
        self
    }

    pub fn with_default_agent_profile_id(mut self, profile_id: impl Into<ProfileId>) -> Self {
        self.default_agent_profile_id = Some(profile_id.into());
        self
    }

    pub fn with_ai_client(mut self, ai_client: AiClient) -> Self {
        self.ai_client = Some(ai_client);
        self
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    pub fn session_log_root(&self) -> Option<&Path> {
        self.session_log_root.as_deref()
    }

    pub fn session_path(&self) -> Option<&Path> {
        self.session_path.as_deref()
    }

    pub fn default_agent_profile_id(&self) -> Option<&ProfileId> {
        self.default_agent_profile_id.as_ref()
    }

    pub(crate) fn ai_client(&self) -> Option<&AiClient> {
        self.ai_client.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSessionView {
    pub session_id: String,
    pub default_agent_profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSessionSummary {
    pub session_id: String,
    pub session_dir: PathBuf,
    pub created_at: String,
    pub updated_at: String,
    pub active_leaf_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CodingAgentSessionHydration {
    pub(crate) summary: CodingAgentSessionSummary,
    pub(crate) cwd: Option<String>,
    pub(crate) transcript: Vec<CodingAgentSessionTranscriptItem>,
    pub(crate) diagnostics: Vec<CodingAgentSessionDiagnostic>,
    pub(crate) usage: CodingAgentSessionUsageSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CodingAgentSessionTree {
    pub(crate) tree: Vec<SessionTreeNode>,
    pub(crate) active_leaf_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CodingAgentSessionUsageSummary {
    pub(crate) input: u32,
    pub(crate) output: u32,
    pub(crate) cache_read: u32,
    pub(crate) cache_write: u32,
    pub(crate) cost: f64,
    pub(crate) last_context_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodingAgentSessionTranscriptItem {
    User {
        text: String,
    },
    Assistant {
        id: String,
        text: String,
        done: bool,
    },
    Tool {
        call_id: String,
        name: String,
        args: serde_json::Value,
        result: Option<String>,
        is_error: bool,
    },
    Delegation {
        tool_call_id: String,
        requesting_profile_id: ProfileId,
        target_kind: ProfileKind,
        target_id: ProfileId,
        task: String,
        status: String,
        child_operation_id: Option<String>,
        summary: Option<String>,
    },
    CompactionSummary {
        summary: String,
    },
    BranchSummary {
        summary: String,
    },
    Diagnostic {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodingAgentSessionDiagnostic {
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentCapabilities {
    pub prompt: CapabilityStatus,
    pub abort: CapabilityStatus,
    pub steer: CapabilityStatus,
    pub follow_up: CapabilityStatus,
    pub compact: CapabilityStatus,
    pub fork: CapabilityStatus,
    pub clone_session: CapabilityStatus,
    pub branch_summary: CapabilityStatus,
    pub switch_session: CapabilityStatus,
    pub export: CapabilityStatus,
    pub plugin_reload: CapabilityStatus,
    pub self_healing_edit: CapabilityStatus,
    pub agent_profiles: CapabilityStatus,
    pub team_profiles: CapabilityStatus,
    pub delegation: CapabilityStatus,
    pub tools: CapabilityStatus,
    pub shell: CapabilityStatus,
    pub plugins: CapabilityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Available,
    Disabled { reason: String },
    Unsupported { reason: String },
    Busy { operation: String },
}

impl CodingAgentCapabilities {
    pub(crate) fn from_runtime_state(
        active_operation: Option<OperationKind>,
        _plugin_capabilities: &PluginCapabilities,
        persistent_session: bool,
    ) -> Self {
        let prompt = match active_operation {
            Some(operation) => CapabilityStatus::Busy {
                operation: operation.as_str().into(),
            },
            None => CapabilityStatus::Available,
        };

        let persistent_session_capability = match (persistent_session, active_operation) {
            (false, _) => CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            },
            (true, Some(operation)) => CapabilityStatus::Busy {
                operation: operation.as_str().into(),
            },
            (true, None) => CapabilityStatus::Available,
        };
        let prompt_control_capability = match active_operation {
            Some(OperationKind::Prompt) => CapabilityStatus::Available,
            _ => CapabilityStatus::Disabled {
                reason: "no prompt is running".into(),
            },
        };
        let profile_operation_capability = match active_operation {
            Some(operation) => CapabilityStatus::Busy {
                operation: operation.as_str().into(),
            },
            None => CapabilityStatus::Available,
        };

        Self {
            prompt,
            abort: prompt_control_capability.clone(),
            steer: prompt_control_capability.clone(),
            follow_up: prompt_control_capability,
            compact: persistent_session_capability.clone(),
            fork: persistent_session_capability.clone(),
            clone_session: persistent_session_capability.clone(),
            branch_summary: persistent_session_capability.clone(),
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: persistent_session_capability.clone(),
            plugin_reload: persistent_session_capability.clone(),
            self_healing_edit: persistent_session_capability,
            agent_profiles: profile_operation_capability.clone(),
            team_profiles: profile_operation_capability.clone(),
            delegation: profile_operation_capability,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }
}
