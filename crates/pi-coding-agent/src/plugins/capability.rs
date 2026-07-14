#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct PluginCapabilities {
    pub(crate) tool_providers: usize,
    pub(crate) command_providers: usize,
    pub(crate) hook_providers: usize,
    pub(crate) ui_providers: usize,
    pub(crate) keybind_providers: usize,
    pub(crate) flow_extensions: usize,
    pub(crate) diagnostics: usize,
}

impl PluginCapabilities {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}
