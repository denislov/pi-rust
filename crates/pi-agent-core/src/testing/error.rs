#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHarnessErrorCode {
    Busy,
    InvalidState,
    InvalidArgument,
    Session,
    Hook,
    Auth,
    Compaction,
    BranchSummary,
    Unknown,
}

impl AgentHarnessErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentHarnessErrorCode::Busy => "busy",
            AgentHarnessErrorCode::InvalidState => "invalid_state",
            AgentHarnessErrorCode::InvalidArgument => "invalid_argument",
            AgentHarnessErrorCode::Session => "session",
            AgentHarnessErrorCode::Hook => "hook",
            AgentHarnessErrorCode::Auth => "auth",
            AgentHarnessErrorCode::Compaction => "compaction",
            AgentHarnessErrorCode::BranchSummary => "branch_summary",
            AgentHarnessErrorCode::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AgentHarnessError {
    pub code: AgentHarnessErrorCode,
    pub message: String,
}

impl AgentHarnessError {
    pub fn new(code: AgentHarnessErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
