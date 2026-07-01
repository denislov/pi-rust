mod capability;
mod command;
mod error;
mod hook;
mod keybind;
mod registry;
mod tool;
mod ui;

pub(crate) use capability::PluginCapabilities;
pub(crate) use command::{CommandDefinition, CommandProvider, CommandRegistrationHost};
pub(crate) use error::PluginError;
#[allow(unused_imports)]
pub(crate) use hook::{
    HookDiagnostic, HookFailurePolicy, HookOutcome, HookProvider, HookRegistration,
    HookRegistrationHost, PromptHookContext, PromptHookPoint,
};
pub(crate) use keybind::{KeybindDefinition, KeybindProvider, KeybindRegistrationHost};
pub(crate) use registry::PluginRegistry;
#[cfg(test)]
pub(crate) use registry::{PluginId, PluginMetadata, PluginSource};
pub(crate) use tool::{ToolProvider, ToolRegistrationHost};
#[allow(unused_imports)]
pub(crate) use ui::{UiActionDefinition, UiDialogDefinition, UiProvider, UiRegistrationHost};
