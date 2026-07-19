mod capability;
mod contributions;

pub(crate) use capability::PluginCapabilities;
pub(crate) use contributions::command::CommandDefinition;
pub(crate) use contributions::keybind::KeybindDefinition;
pub(crate) use contributions::ui::{UiActionDefinition, UiDialogDefinition};
