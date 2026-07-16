use crate::plugins::error::PluginError;
use crate::plugins::manifest::PluginMetadata;
use crate::runtime::facade::PluginCapabilitySet;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CommandDefinition {
    pub(crate) id: String,
    pub(crate) description: String,
}

#[allow(dead_code)]
impl CommandDefinition {
    pub(crate) fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct CommandRegistrationHost {
    capabilities: PluginCapabilitySet,
}

#[allow(dead_code)]
impl CommandRegistrationHost {
    pub(crate) fn new(capabilities: PluginCapabilitySet) -> Self {
        Self { capabilities }
    }

    #[allow(dead_code)]
    pub(crate) fn capabilities(&self) -> &PluginCapabilitySet {
        &self.capabilities
    }
}

#[allow(dead_code)]
pub(crate) trait CommandProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn commands(
        &self,
        host: &CommandRegistrationHost,
    ) -> Result<Vec<CommandDefinition>, PluginError>;

    fn run_command(&self, command_id: &str, args: serde_json::Value)
    -> Result<String, PluginError>;
}
