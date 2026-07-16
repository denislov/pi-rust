use std::future::Future;
use std::pin::Pin;

use super::{Action, FlowError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(value: impl Into<String>) -> Result<Self, FlowError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FlowError::EmptyNodeId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for NodeId {
    type Error = FlowError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub trait FlowNode<C>: Send + Sync {
    fn name(&self) -> &str;

    fn run<'a>(
        &'a self,
        ctx: &'a mut C,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>>;
}
