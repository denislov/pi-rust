#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    #[error("missing value for {0}")]
    MissingValue(String),
    #[error("unknown flag: {0}")]
    UnknownFlag(String),
    #[error("unsupported mode: {0}")]
    UnsupportedMode(String),
    #[error("missing prompt")]
    MissingPrompt,
    #[error("unknown model: {0}")]
    UnknownModel(String),
    #[error("invalid max turns: {0}")]
    InvalidMaxTurns(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("agent failure: {0}")]
    AgentFailure(String),
    #[error("{0}")]
    InvalidSessionFlags(String),
    #[error("{0}")]
    SessionFailure(String),
}
