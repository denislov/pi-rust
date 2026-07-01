use std::fmt;
use std::sync::Arc;

use super::command::CommandProvider;
use super::tool::ToolProvider;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PluginId(String);

impl PluginId {
    #[allow(dead_code)]
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PluginSource {
    FirstParty,
    Project,
    User,
    Lua,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginMetadata {
    pub(crate) id: PluginId,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) source: PluginSource,
}

impl PluginMetadata {
    #[allow(dead_code)]
    pub(crate) fn new(
        id: PluginId,
        name: impl Into<String>,
        version: impl Into<String>,
        source: PluginSource,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            version: version.into(),
            source,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct PluginRegistry {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    command_providers: Vec<Arc<dyn CommandProvider>>,
}

impl PluginRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub(crate) fn register_tool_provider(&mut self, provider: Arc<dyn ToolProvider>) {
        self.tool_providers.push(provider);
    }

    #[allow(dead_code)]
    pub(crate) fn register_command_provider(&mut self, provider: Arc<dyn CommandProvider>) {
        self.command_providers.push(provider);
    }

    pub(crate) fn tool_providers(&self) -> &[Arc<dyn ToolProvider>] {
        &self.tool_providers
    }

    #[allow(dead_code)]
    pub(crate) fn command_providers(&self) -> &[Arc<dyn CommandProvider>] {
        &self.command_providers
    }
}

impl fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("tool_providers_len", &self.tool_providers.len())
            .field("command_providers_len", &self.command_providers.len())
            .finish()
    }
}
