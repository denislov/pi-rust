use crate::plugins::error::PluginError;
use crate::plugins::manifest::PluginMetadata;
use crate::runtime::facade::PluginCapabilitySet;

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
    pub(crate) required: bool,
    pub(crate) options: Vec<String>,
}

#[allow(dead_code)]
impl UiDialogFieldDefinition {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        kind: impl Into<String>,
        default_value: serde_json::Value,
        required: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            kind: kind.into(),
            default_value,
            required,
            options: Vec::new(),
        }
    }

    pub(crate) fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct UiRegistrationHost {
    capabilities: PluginCapabilitySet,
}

#[allow(dead_code)]
impl UiRegistrationHost {
    pub(crate) fn new(capabilities: PluginCapabilitySet) -> Self {
        Self { capabilities }
    }

    #[allow(dead_code)]
    pub(crate) fn capabilities(&self) -> &PluginCapabilitySet {
        &self.capabilities
    }
}

#[allow(dead_code)]
pub(crate) trait UiProvider: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn ui_actions(&self, host: &UiRegistrationHost)
    -> Result<Vec<UiActionDefinition>, PluginError>;

    fn dialogs(&self, _host: &UiRegistrationHost) -> Result<Vec<UiDialogDefinition>, PluginError> {
        Ok(Vec::new())
    }
}
