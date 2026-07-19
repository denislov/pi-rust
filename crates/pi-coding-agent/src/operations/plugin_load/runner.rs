use tokio_util::sync::CancellationToken;

use crate::plugins::PluginCapabilities;
use crate::runtime::facade::CodingSessionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginDiagnostic {
    pub(crate) plugin_id: Option<String>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PluginLoadOptions;

impl PluginLoadOptions {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginLoadOutcome {
    pub(crate) loaded_plugin_ids: Vec<String>,
    pub(crate) diagnostics: Vec<PluginDiagnostic>,
    pub(crate) capabilities: PluginCapabilities,
    pub(crate) capability_changed: bool,
}

pub(crate) struct PluginLoadContext {
    outcome: Option<PluginLoadOutcome>,
    failure_error: Option<CodingSessionError>,
}

impl PluginLoadContext {
    pub(crate) fn new(_options: PluginLoadOptions) -> Self {
        Self {
            outcome: None,
            failure_error: None,
        }
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<PluginLoadOutcome, CodingSessionError> {
        self.outcome
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "plugin load cannot finish without an outcome".into(),
            })
    }
}

pub(crate) struct PluginLoadRunner;

impl PluginLoadRunner {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        Ok(Self)
    }

    pub(crate) async fn run_typed(
        &self,
        ctx: &mut PluginLoadContext,
        cancellation: Option<CancellationToken>,
    ) -> Result<(), CodingSessionError> {
        if cancellation
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
        {
            ctx.failure_error = Some(CodingSessionError::Cancelled);
            return Err(CodingSessionError::Cancelled);
        }
        ctx.outcome = Some(PluginLoadOutcome {
            loaded_plugin_ids: Vec::new(),
            diagnostics: Vec::new(),
            capabilities: PluginCapabilities::new(),
            capability_changed: false,
        });
        Ok(())
    }
}
