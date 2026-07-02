use std::path::{Path, PathBuf};

use pi_agent_core::session::SessionTreeNode;

use crate::plugins::PluginCapabilities;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodingAgentSessionOptions {
    cwd: Option<PathBuf>,
    session_id: Option<String>,
    session_log_root: Option<PathBuf>,
    session_path: Option<PathBuf>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSessionView {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingAgentSessionSummary {
    pub session_id: String,
    pub session_dir: PathBuf,
    pub created_at: String,
    pub updated_at: String,
    pub active_leaf_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodingAgentSessionHydration {
    pub(crate) summary: CodingAgentSessionSummary,
    pub(crate) cwd: Option<String>,
    pub(crate) transcript: Vec<CodingAgentSessionTranscriptItem>,
    pub(crate) diagnostics: Vec<CodingAgentSessionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CodingAgentSessionTree {
    pub(crate) tree: Vec<SessionTreeNode>,
    pub(crate) active_leaf_id: Option<String>,
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
    pub(crate) fn phase_5(
        active_operation: Option<&str>,
        plugin_capabilities: &PluginCapabilities,
        persistent_session: bool,
    ) -> Self {
        let prompt = match active_operation {
            Some(operation) => CapabilityStatus::Busy {
                operation: operation.into(),
            },
            None => CapabilityStatus::Available,
        };

        let _ = plugin_capabilities;
        let persistent_session_capability = match (persistent_session, active_operation) {
            (false, _) => CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            },
            (true, Some(operation)) => CapabilityStatus::Busy {
                operation: operation.into(),
            },
            (true, None) => CapabilityStatus::Available,
        };

        Self {
            prompt,
            abort: CapabilityStatus::Unsupported {
                reason: "operation abort is not exposed on CodingAgentSession yet".into(),
            },
            steer: CapabilityStatus::Unsupported {
                reason: "agent turn steering awaits AgentTurnFlow".into(),
            },
            follow_up: CapabilityStatus::Unsupported {
                reason: "follow-up controls await AgentTurnFlow".into(),
            },
            compact: persistent_session_capability.clone(),
            fork: persistent_session_capability.clone(),
            clone_session: persistent_session_capability.clone(),
            branch_summary: persistent_session_capability.clone(),
            switch_session: CapabilityStatus::Unsupported {
                reason: "session switching is not exposed on CodingAgentSession yet".into(),
            },
            export: persistent_session_capability.clone(),
            plugin_reload: persistent_session_capability,
            tools: CapabilityStatus::Available,
            shell: CapabilityStatus::Available,
            plugins: CapabilityStatus::Available,
        }
    }
}
