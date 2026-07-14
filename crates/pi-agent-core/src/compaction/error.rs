use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompactionError {
    #[error("compaction aborted")]
    Aborted,
    #[error("summarization failed: {0}")]
    SummarizationFailed(String),
    #[error("invalid session: {0}")]
    InvalidSession(String),
    #[error("unknown error: {0}")]
    Unknown(String),
}
