#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchSummaryErrorCode {
    Aborted,
    SummarizationFailed,
    InvalidSession,
}

impl BranchSummaryErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            BranchSummaryErrorCode::Aborted => "aborted",
            BranchSummaryErrorCode::SummarizationFailed => "summarization_failed",
            BranchSummaryErrorCode::InvalidSession => "invalid_session",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct BranchSummaryError {
    pub code: BranchSummaryErrorCode,
    pub message: String,
}

impl BranchSummaryError {
    pub fn new(code: BranchSummaryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
