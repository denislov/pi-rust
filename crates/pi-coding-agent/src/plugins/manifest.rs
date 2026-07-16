#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PluginId(String);

impl PluginId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginSource {
    FirstParty,
    Project,
    User,
    Lua,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginMetadata {
    pub(crate) id: PluginId,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) source: PluginSource,
}

impl PluginMetadata {
    pub(crate) fn new(
        id: PluginId,
        name: impl Into<String>,
        version: impl Into<String>,
        source: PluginSource,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            version: version.into(),
            source,
        }
    }
}
