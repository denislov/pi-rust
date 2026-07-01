use super::error::PluginError;
use super::registry::PluginMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct KeybindDefinition {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) description: String,
    pub(crate) action_id: String,
}

#[allow(dead_code)]
impl KeybindDefinition {
    pub(crate) fn new(
        id: impl Into<String>,
        key: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            key: key.into(),
            description: description.into(),
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct KeybindRegistrationHost;

#[allow(dead_code)]
pub(crate) trait KeybindProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn keybindings(
        &self,
        host: &KeybindRegistrationHost,
    ) -> Result<Vec<KeybindDefinition>, PluginError>;
}
