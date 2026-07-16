use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct PluginCapabilities {
    pub(crate) tool_providers: usize,
    pub(crate) command_providers: usize,
    pub(crate) hook_providers: usize,
    pub(crate) ui_providers: usize,
    pub(crate) keybind_providers: usize,
    pub(crate) diagnostics: usize,
    pub(crate) tool_provider_ids: BTreeSet<String>,
    pub(crate) command_provider_ids: BTreeSet<String>,
    pub(crate) hook_provider_ids: BTreeSet<String>,
    pub(crate) ui_provider_ids: BTreeSet<String>,
    pub(crate) keybind_provider_ids: BTreeSet<String>,
}

impl PluginCapabilities {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}
