use super::error::PluginError;
use super::registry::PluginMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PromptHookPoint {
    BeforePromptPrepare,
    AfterInputPrepared,
    AfterResourcesLoaded,
    BeforeAgentTurn,
    AfterAgentTurn,
    BeforeSessionCommit,
    AfterSessionCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum HookFailurePolicy {
    FailOpen,
    FailClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct HookRegistration {
    pub(crate) point: PromptHookPoint,
    pub(crate) policy: HookFailurePolicy,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct HookRegistrationHost;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct PromptHookContext {
    pub(crate) operation_id: String,
    pub(crate) turn_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) point: PromptHookPoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct HookDiagnostic {
    pub(crate) message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct HookOutcome {
    pub(crate) diagnostics: Vec<HookDiagnostic>,
}

#[allow(dead_code)]
pub(crate) trait HookProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn hooks(&self, host: &HookRegistrationHost) -> Result<Vec<HookRegistration>, PluginError>;

    fn run_hook(&self, _ctx: &PromptHookContext) -> Result<HookOutcome, PluginError> {
        Ok(HookOutcome::default())
    }
}
