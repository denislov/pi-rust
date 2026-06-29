use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodingAgentSessionOptions {
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
pub struct CodingAgentCapabilities {
    pub prompt: bool,
    pub session_log: bool,
    pub plugins: bool,
}

impl CodingAgentCapabilities {
    pub(crate) fn phase_1() -> Self {
        Self {
            prompt: false,
            session_log: false,
            plugins: false,
        }
    }
}
