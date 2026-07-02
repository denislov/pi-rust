use super::error::PluginError;
use super::registry::PluginMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct UiActionDefinition {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) action_id: String,
}

#[allow(dead_code)]
impl UiActionDefinition {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct UiDialogFieldDefinition {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) kind: String,
    pub(crate) default_value: serde_json::Value,
}

#[allow(dead_code)]
impl UiDialogFieldDefinition {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        kind: impl Into<String>,
        default_value: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            kind: kind.into(),
            default_value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct UiDialogDefinition {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) action_id: String,
    pub(crate) fields: Vec<UiDialogFieldDefinition>,
}

#[allow(dead_code)]
impl UiDialogDefinition {
    pub(crate) fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            action_id: action_id.into(),
            fields: Vec::new(),
        }
    }

    pub(crate) fn with_fields(mut self, fields: Vec<UiDialogFieldDefinition>) -> Self {
        self.fields = fields;
        self
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct UiRegistrationHost;

#[allow(dead_code)]
pub(crate) trait UiProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn ui_actions(&self, host: &UiRegistrationHost)
    -> Result<Vec<UiActionDefinition>, PluginError>;

    fn dialogs(&self, _host: &UiRegistrationHost) -> Result<Vec<UiDialogDefinition>, PluginError> {
        Ok(Vec::new())
    }
}
