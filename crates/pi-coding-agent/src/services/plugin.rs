use pi_agent_core::api::tool::AgentTool;

use crate::operations::prompt::context::CodingDiagnostic;
use crate::plugins::{
    CommandDefinition, KeybindDefinition, PluginCapabilities, PromptHookContext, PromptHookPoint,
    UiActionDefinition, UiDialogDefinition,
};
use crate::runtime::capability::PluginCapabilitySet;
use crate::runtime::facade::CodingSessionError;

/// Empty compatibility owner while the minimum Wasm framework has no product
/// contribution dispatch. Legacy provider registration was intentionally
/// removed; this type disappears when its remaining runtime plumbing is folded.
#[derive(Debug, Clone, Default)]
pub(crate) struct PluginService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginDiagnostic {
    pub(crate) plugin_id: Option<String>,
    pub(crate) message: String,
}

impl PluginService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn collect_tools_with_capabilities(
        &self,
        _capabilities: &PluginCapabilitySet,
    ) -> Vec<AgentTool> {
        Vec::new()
    }

    pub(crate) fn collect_tools(&self) -> Vec<AgentTool> {
        Vec::new()
    }

    pub(crate) fn collect_commands(&self) -> Vec<CommandDefinition> {
        Vec::new()
    }

    pub(crate) fn run_command_with_capabilities(
        &self,
        command_id: &str,
        _args: serde_json::Value,
        capabilities: &PluginCapabilitySet,
    ) -> Result<String, CodingSessionError> {
        if capabilities.is_permissive() {
            Err(CodingSessionError::Plugin {
                message: format!("plugin command not found: {command_id}"),
            })
        } else {
            Err(CodingSessionError::UnsupportedCapability {
                capability: format!("plugin command not granted: {command_id}"),
            })
        }
    }

    pub(crate) fn collect_ui_actions(&self) -> Vec<UiActionDefinition> {
        Vec::new()
    }

    pub(crate) fn collect_ui_dialogs(&self) -> Vec<UiDialogDefinition> {
        Vec::new()
    }

    pub(crate) fn collect_keybindings(&self) -> Vec<KeybindDefinition> {
        Vec::new()
    }

    pub(crate) fn run_prompt_hook_with_capabilities(
        &self,
        _point: PromptHookPoint,
        _ctx: PromptHookContext,
        _capabilities: &PluginCapabilitySet,
    ) -> Result<Vec<CodingDiagnostic>, CodingSessionError> {
        Ok(Vec::new())
    }

    pub(crate) fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::new()
    }
}
