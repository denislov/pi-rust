pub mod auth;
pub mod paths;
pub mod settings;

use std::path::PathBuf;

pub use paths::{ConfigPaths, resolve as resolve_paths};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<PathBuf>,
}

impl ConfigDiagnostic {
    pub fn warn(message: impl Into<String>, source: Option<PathBuf>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warn,
            message: message.into(),
            source,
        }
    }

    pub fn error(message: impl Into<String>, source: Option<PathBuf>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            source,
        }
    }
}
