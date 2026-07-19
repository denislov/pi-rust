#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CommandDefinition {
    pub(crate) id: String,
    pub(crate) description: String,
}

#[allow(dead_code)]
impl CommandDefinition {
    pub(crate) fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
        }
    }
}
