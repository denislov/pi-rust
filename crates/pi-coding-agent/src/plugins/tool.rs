use pi_agent_core::AgentTool;

use super::error::PluginError;
use super::registry::PluginMetadata;
use crate::coding_session::PluginCapabilitySet;

#[derive(Debug, Clone)]
pub(crate) struct ToolRegistrationHost {
    capabilities: PluginCapabilitySet,
}

impl ToolRegistrationHost {
    pub(crate) fn new(capabilities: PluginCapabilitySet) -> Self {
        Self { capabilities }
    }

    #[allow(dead_code)]
    pub(crate) fn capabilities(&self) -> &PluginCapabilitySet {
        &self.capabilities
    }
}

pub(crate) trait ToolProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn tools(&self, host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError>;
}
