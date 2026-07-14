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

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct FlowExtensionRegistrationHost;

#[allow(dead_code)]
pub(crate) trait FlowExtension: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn extension_points(
        &self,
        host: &FlowExtensionRegistrationHost,
    ) -> Result<Vec<FlowExtensionPoint>, PluginError>;
}
