use super::self_healing_edit_flow::{SelfHealingEditCheckOutput, SelfHealingEditDiagnostic};
use crate::error::CliError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodingSessionError {
    #[error("configuration error: {message}")]
    Config { message: String },
    #[error("authentication error: {message}")]
    Auth { message: String },
    #[error("invalid input: {message}")]
    Input { message: String },
    #[error("resource error: {message}")]
    Resource { message: String },
    #[error("session error: {message}")]
    Session { message: String },
    #[error("self-healing edit failed: {message}")]
    SelfHealingEditFailed {
        message: String,
        diagnostics: Vec<SelfHealingEditDiagnostic>,
        check_output: Option<SelfHealingEditCheckOutput>,
    },
    #[error("provider error: {message}")]
    Provider { message: String },
    #[error("tool error: {message}")]
    Tool { message: String },
    #[error("flow error: {message}")]
    Flow { message: String },
    #[error("plugin error: {message}")]
    Plugin { message: String },
    #[error("cancelled")]
    Cancelled,
    #[error("unsupported capability: {capability}")]
    UnsupportedCapability { capability: String },
    #[error("busy: {operation}")]
    Busy { operation: String },
}

impl CodingSessionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config { .. } => "config",
            Self::Auth { .. } => "auth",
            Self::Input { .. } => "input",
            Self::Resource { .. } => "resource",
            Self::Session { .. } => "session",
            Self::SelfHealingEditFailed { .. } => "self_healing_edit_failed",
            Self::Provider { .. } => "provider",
            Self::Tool { .. } => "tool",
            Self::Flow { .. } => "flow",
            Self::Plugin { .. } => "plugin",
            Self::Cancelled => "cancelled",
            Self::UnsupportedCapability { .. } => "unsupported_capability",
            Self::Busy { .. } => "busy",
        }
    }
}

impl From<CodingSessionError> for CliError {
    fn from(error: CodingSessionError) -> Self {
        match error {
            CodingSessionError::Config { message }
            | CodingSessionError::Auth { message }
            | CodingSessionError::Input { message }
            | CodingSessionError::Resource { message }
            | CodingSessionError::Session { message }
            | CodingSessionError::SelfHealingEditFailed { message, .. }
            | CodingSessionError::Provider { message }
            | CodingSessionError::Tool { message }
            | CodingSessionError::Flow { message }
            | CodingSessionError::Plugin { message } => CliError::SessionFailure(message),
            CodingSessionError::Cancelled => CliError::SessionFailure("cancelled".into()),
            CodingSessionError::UnsupportedCapability { capability } => {
                CliError::UnsupportedMode(capability)
            }
            CodingSessionError::Busy { operation } => {
                CliError::SessionFailure(format!("busy: {operation}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_session_error_codes_are_stable() {
        let cases = [
            (
                CodingSessionError::Config {
                    message: "missing setting".into(),
                },
                "config",
            ),
            (
                CodingSessionError::Auth {
                    message: "missing token".into(),
                },
                "auth",
            ),
            (
                CodingSessionError::Input {
                    message: "empty prompt".into(),
                },
                "input",
            ),
            (
                CodingSessionError::Resource {
                    message: "not found".into(),
                },
                "resource",
            ),
            (
                CodingSessionError::Session {
                    message: "not open".into(),
                },
                "session",
            ),
            (
                CodingSessionError::SelfHealingEditFailed {
                    message: "check failed".into(),
                    diagnostics: Vec::new(),
                    check_output: None,
                },
                "self_healing_edit_failed",
            ),
            (
                CodingSessionError::Provider {
                    message: "stream failed".into(),
                },
                "provider",
            ),
            (
                CodingSessionError::Tool {
                    message: "tool failed".into(),
                },
                "tool",
            ),
            (
                CodingSessionError::Flow {
                    message: "node failed".into(),
                },
                "flow",
            ),
            (
                CodingSessionError::Plugin {
                    message: "hook failed".into(),
                },
                "plugin",
            ),
            (CodingSessionError::Cancelled, "cancelled"),
            (
                CodingSessionError::UnsupportedCapability {
                    capability: "prompt".into(),
                },
                "unsupported_capability",
            ),
            (
                CodingSessionError::Busy {
                    operation: "prompt".into(),
                },
                "busy",
            ),
        ];

        for (error, code) in cases {
            assert_eq!(error.code(), code);
        }
    }
}
