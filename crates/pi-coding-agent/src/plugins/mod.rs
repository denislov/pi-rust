mod capability;
mod command;
mod error;
mod hook;
mod registry;
mod tool;

pub(crate) use capability::PluginCapabilities;
pub(crate) use command::{CommandDefinition, CommandProvider, CommandRegistrationHost};
pub(crate) use error::PluginError;
#[cfg(test)]
pub(crate) use hook::{HookFailurePolicy, PromptHookPoint};
pub(crate) use hook::{HookProvider, HookRegistration, HookRegistrationHost};
pub(crate) use registry::PluginRegistry;
#[cfg(test)]
pub(crate) use registry::{PluginId, PluginMetadata, PluginSource};
pub(crate) use tool::{ToolProvider, ToolRegistrationHost};
