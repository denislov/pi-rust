mod capability;
mod contributions;
mod error;
mod manifest;
mod registry;

pub(crate) use capability::PluginCapabilities;
pub(crate) use contributions::command::{
    CommandDefinition, CommandProvider, CommandRegistrationHost,
};
#[allow(unused_imports)]
pub(crate) use contributions::hook::{
    HookDiagnostic, HookFailurePolicy, HookOutcome, HookProvider, HookRegistration,
    HookRegistrationHost, PromptHookContext, PromptHookPoint,
};
pub(crate) use contributions::keybind::{
    KeybindDefinition, KeybindProvider, KeybindRegistrationHost,
};
pub(crate) use contributions::tool::{ToolProvider, ToolRegistrationHost};
#[allow(unused_imports)]
pub(crate) use contributions::ui::{
    UiActionDefinition, UiDialogDefinition, UiDialogFieldDefinition, UiProvider, UiRegistrationHost,
};
pub(crate) use error::PluginError;
pub(crate) use manifest::{PluginId, PluginMetadata, PluginSource};
pub(crate) use registry::PluginRegistry;
