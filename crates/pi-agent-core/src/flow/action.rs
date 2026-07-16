use super::FlowError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Action(String);

impl Action {
    pub fn new(value: impl Into<String>) -> Result<Self, FlowError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FlowError::EmptyAction);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Action {
    fn default() -> Self {
        Self("default".into())
    }
}

impl TryFrom<&str> for Action {
    type Error = FlowError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
