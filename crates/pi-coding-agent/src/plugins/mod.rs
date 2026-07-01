mod capability;
mod command;
mod error;
mod hook;
mod registry;
mod tool;

pub(crate) use capability::PluginCapabilities;
pub(crate) use command::{CommandDefinition, CommandProvider, CommandRegistrationHost};
pub(crate) use error::PluginError;
#[allow(unused_imports)]
pub(crate) use hook::{
    HookDiagnostic, HookFailurePolicy, HookOutcome, HookProvider, HookRegistration,
    HookRegistrationHost, PromptHookContext, PromptHookPoint,
};
pub(crate) use registry::PluginRegistry;
#[cfg(test)]
pub(crate) use registry::{PluginId, PluginMetadata, PluginSource};
pub(crate) use tool::{ToolProvider, ToolRegistrationHost};
