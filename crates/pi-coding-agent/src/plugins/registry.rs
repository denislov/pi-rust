use std::fmt;
use std::sync::Arc;

use super::contributions::command::CommandProvider;
use super::contributions::hook::HookProvider;
use super::contributions::keybind::KeybindProvider;
use super::contributions::tool::ToolProvider;
use super::contributions::ui::UiProvider;

#[derive(Clone, Default)]
pub(crate) struct PluginRegistry {
    tool_providers: Vec<Arc<dyn ToolProvider>>,
    command_providers: Vec<Arc<dyn CommandProvider>>,
    hook_providers: Vec<Arc<dyn HookProvider>>,
    ui_providers: Vec<Arc<dyn UiProvider>>,
    keybind_providers: Vec<Arc<dyn KeybindProvider>>,
}

impl PluginRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register_tool_provider(&mut self, provider: Arc<dyn ToolProvider>) {
        self.tool_providers.push(provider);
    }

    pub(crate) fn register_command_provider(&mut self, provider: Arc<dyn CommandProvider>) {
        self.command_providers.push(provider);
    }

    pub(crate) fn register_hook_provider(&mut self, provider: Arc<dyn HookProvider>) {
        self.hook_providers.push(provider);
    }

    pub(crate) fn register_ui_provider(&mut self, provider: Arc<dyn UiProvider>) {
        self.ui_providers.push(provider);
    }

    pub(crate) fn register_keybind_provider(&mut self, provider: Arc<dyn KeybindProvider>) {
        self.keybind_providers.push(provider);
    }

    pub(crate) fn extend(&mut self, other: PluginRegistry) {
        self.tool_providers.extend(other.tool_providers);
        self.command_providers.extend(other.command_providers);
        self.hook_providers.extend(other.hook_providers);
        self.ui_providers.extend(other.ui_providers);
        self.keybind_providers.extend(other.keybind_providers);
    }

    pub(crate) fn tool_providers(&self) -> &[Arc<dyn ToolProvider>] {
        &self.tool_providers
    }

    pub(crate) fn command_providers(&self) -> &[Arc<dyn CommandProvider>] {
        &self.command_providers
    }

    pub(crate) fn hook_providers(&self) -> &[Arc<dyn HookProvider>] {
        &self.hook_providers
    }

    pub(crate) fn ui_providers(&self) -> &[Arc<dyn UiProvider>] {
        &self.ui_providers
    }

    pub(crate) fn keybind_providers(&self) -> &[Arc<dyn KeybindProvider>] {
        &self.keybind_providers
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
            .finish()
    }
}
