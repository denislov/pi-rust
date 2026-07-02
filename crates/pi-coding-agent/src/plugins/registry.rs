use std::fmt;
use std::sync::Arc;

use super::command::CommandProvider;
use super::flow_extension::FlowExtension;
use super::hook::HookProvider;
use super::keybind::KeybindProvider;
use super::tool::ToolProvider;
use super::ui::UiProvider;

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
    hook_providers: Vec<Arc<dyn HookProvider>>,
    ui_providers: Vec<Arc<dyn UiProvider>>,
    keybind_providers: Vec<Arc<dyn KeybindProvider>>,
    flow_extensions: Vec<Arc<dyn FlowExtension>>,
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

    #[allow(dead_code)]
    pub(crate) fn register_hook_provider(&mut self, provider: Arc<dyn HookProvider>) {
        self.hook_providers.push(provider);
    }

    #[allow(dead_code)]
    pub(crate) fn register_ui_provider(&mut self, provider: Arc<dyn UiProvider>) {
        self.ui_providers.push(provider);
    }

    #[allow(dead_code)]
    pub(crate) fn register_keybind_provider(&mut self, provider: Arc<dyn KeybindProvider>) {
        self.keybind_providers.push(provider);
    }

    #[allow(dead_code)]
    pub(crate) fn register_flow_extension(&mut self, extension: Arc<dyn FlowExtension>) {
        self.flow_extensions.push(extension);
    }

    pub(crate) fn extend(&mut self, other: PluginRegistry) {
        self.tool_providers.extend(other.tool_providers);
        self.command_providers.extend(other.command_providers);
        self.hook_providers.extend(other.hook_providers);
        self.ui_providers.extend(other.ui_providers);
        self.keybind_providers.extend(other.keybind_providers);
        self.flow_extensions.extend(other.flow_extensions);
    }

    pub(crate) fn tool_providers(&self) -> &[Arc<dyn ToolProvider>] {
        &self.tool_providers
    }

    #[allow(dead_code)]
    pub(crate) fn command_providers(&self) -> &[Arc<dyn CommandProvider>] {
        &self.command_providers
    }

    #[allow(dead_code)]
    pub(crate) fn hook_providers(&self) -> &[Arc<dyn HookProvider>] {
        &self.hook_providers
    }

    #[allow(dead_code)]
    pub(crate) fn ui_providers(&self) -> &[Arc<dyn UiProvider>] {
        &self.ui_providers
    }

    #[allow(dead_code)]
    pub(crate) fn keybind_providers(&self) -> &[Arc<dyn KeybindProvider>] {
        &self.keybind_providers
    }

    #[allow(dead_code)]
    pub(crate) fn flow_extensions(&self) -> &[Arc<dyn FlowExtension>] {
        &self.flow_extensions
    }
}

impl fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("tool_providers_len", &self.tool_providers.len())
            .field("command_providers_len", &self.command_providers.len())
            .field("hook_providers_len", &self.hook_providers.len())
            .field("ui_providers_len", &self.ui_providers.len())
            .field("keybind_providers_len", &self.keybind_providers.len())
            .field("flow_extensions_len", &self.flow_extensions.len())
            .finish()
    }
}
