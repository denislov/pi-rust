use super::error::PluginError;
use super::registry::PluginMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum FlowExtensionPoint {
    PromptBeforePrepare,
    PromptAfterResourcesLoaded,
    PromptBeforeAgentTurn,
    PromptAfterAgentTurn,
    PromptBeforeSessionCommit,
    AgentBeforeProviderRequest,
    AgentAfterToolResult,
}

#[allow(dead_code)]
pub(crate) trait FlowExtension: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn extension_points(&self) -> Result<Vec<FlowExtensionPoint>, PluginError>;
}
