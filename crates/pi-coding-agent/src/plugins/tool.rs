use pi_agent_core::AgentTool;

use super::error::PluginError;
use super::registry::PluginMetadata;

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolRegistrationHost;

pub(crate) trait ToolProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn tools(&self, host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError>;
}
