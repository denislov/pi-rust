#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(dead_code)]
pub(crate) enum PluginError {
    #[error("plugin registration error in {plugin_id}: {message}")]
    Registration { plugin_id: String, message: String },
    #[error("plugin execution error in {plugin_id}: {message}")]
    Execution { plugin_id: String, message: String },
    #[error("plugin {plugin_id} denied capability {capability}")]
    PermissionDenied {
        plugin_id: String,
        capability: String,
    },
    #[error("plugin panic in {plugin_id}: {message}")]
    Panic { plugin_id: String, message: String },
}

impl PluginError {
    pub(crate) fn plugin_id(&self) -> &str {
        match self {
            Self::Registration { plugin_id, .. }
            | Self::Execution { plugin_id, .. }
            | Self::PermissionDenied { plugin_id, .. }
            | Self::Panic { plugin_id, .. } => plugin_id,
        }
    }
}
