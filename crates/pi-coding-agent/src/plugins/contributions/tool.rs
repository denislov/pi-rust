use pi_agent_core::api::tool::AgentTool;

use crate::plugins::error::PluginError;
use crate::plugins::manifest::PluginMetadata;
use crate::runtime::facade::PluginCapabilitySet;

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
