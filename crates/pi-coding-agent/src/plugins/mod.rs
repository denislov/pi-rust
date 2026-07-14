mod capability;
mod command;
mod error;
mod flow_extension;
mod hook;
mod keybind;
mod registry;
mod tool;
mod ui;

pub(crate) use capability::PluginCapabilities;
pub(crate) use command::{CommandDefinition, CommandProvider, CommandRegistrationHost};
pub(crate) use error::PluginError;
pub(crate) use flow_extension::{FlowExtension, FlowExtensionPoint, FlowExtensionRegistrationHost};
#[allow(unused_imports)]
pub(crate) use hook::{
    HookDiagnostic, HookFailurePolicy, HookOutcome, HookProvider, HookRegistration,
    HookRegistrationHost, PromptHookContext, PromptHookPoint,
};
pub(crate) use keybind::{KeybindDefinition, KeybindProvider, KeybindRegistrationHost};
pub(crate) use registry::{PluginId, PluginMetadata, PluginRegistry, PluginSource};
pub(crate) use tool::{ToolProvider, ToolRegistrationHost};
#[allow(unused_imports)]
pub(crate) use ui::{
    UiActionDefinition, UiDialogDefinition, UiDialogFieldDefinition, UiProvider, UiRegistrationHost,
};
