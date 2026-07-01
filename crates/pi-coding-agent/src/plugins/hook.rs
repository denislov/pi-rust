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

#[allow(dead_code)]
pub(crate) trait HookProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn hooks(&self, host: &HookRegistrationHost) -> Result<Vec<HookRegistration>, PluginError>;
}
