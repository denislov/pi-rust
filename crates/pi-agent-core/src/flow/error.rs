use super::{Action, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    EmptyAction,
    EmptyNodeId,
    DuplicateNode { node: NodeId },
    UnknownNode { node: NodeId },
    MissingStartNode { node: NodeId },
    MissingTransition { node: NodeId, action: Action },
    MaxStepsExceeded { max_steps: usize },
    Cancelled,
    NodeFailed { node: NodeId, message: String },
}

impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlowError::EmptyAction => f.write_str("flow action must not be empty"),
            FlowError::EmptyNodeId => f.write_str("flow node id must not be empty"),
            FlowError::DuplicateNode { node } => write!(f, "duplicate flow node: {node}"),
            FlowError::UnknownNode { node } => write!(f, "unknown flow node: {node}"),
            FlowError::MissingStartNode { node } => {
                write!(f, "flow start node does not exist: {node}")
            }
            FlowError::MissingTransition { node, action } => {
                write!(
                    f,
                    "missing transition from node '{node}' on action '{action}'"
                )
            }
            FlowError::MaxStepsExceeded { max_steps } => {
                write!(f, "flow exceeded max steps ({max_steps})")
            }
            FlowError::Cancelled => f.write_str("flow cancelled"),
            FlowError::NodeFailed { node, message } => {
                write!(f, "flow node '{node}' failed: {message}")
            }
        }
    }
}

impl std::error::Error for FlowError {}
