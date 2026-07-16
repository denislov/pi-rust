use std::collections::HashMap;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::{Action, FlowError, FlowNode, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowOutcome {
    pub last_node: NodeId,
    pub last_action: Action,
    pub steps: usize,
    pub path: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowEvent {
    Started {
        start: NodeId,
    },
    NodeStart {
        node: NodeId,
        name: String,
        step: usize,
    },
    NodeEnd {
        node: NodeId,
        name: String,
        action: Action,
        step: usize,
    },
    MissingTransition {
        node: NodeId,
        action: Action,
    },
    Completed {
        outcome: FlowOutcome,
    },
    Error {
        error: FlowError,
    },
}

pub type FlowEventCallback = Arc<dyn Fn(FlowEvent) + Send + Sync>;

#[derive(Clone)]
pub struct FlowRunOptions {
    pub max_steps: usize,
    pub strict_missing_transition: bool,
    pub cancel: Option<CancellationToken>,
    pub on_event: Option<FlowEventCallback>,
}

impl Default for FlowRunOptions {
    fn default() -> Self {
        Self {
            max_steps: 1024,
            strict_missing_transition: true,
            cancel: None,
            on_event: None,
        }
    }
}

pub struct Flow<C> {
    start: NodeId,
    nodes: HashMap<NodeId, Box<dyn FlowNode<C>>>,
    transitions: HashMap<(NodeId, Action), NodeId>,
}

impl<C> Flow<C> {
    pub fn new(start: impl Into<String>) -> Result<Self, FlowError> {
        Ok(Self {
            start: NodeId::new(start)?,
            nodes: HashMap::new(),
            transitions: HashMap::new(),
        })
    }

    pub fn add_node(
        &mut self,
        id: impl Into<String>,
        node: impl FlowNode<C> + 'static,
    ) -> Result<&mut Self, FlowError> {
        let id = NodeId::new(id)?;
        if self.nodes.contains_key(&id) {
            return Err(FlowError::DuplicateNode { node: id });
        }
        self.nodes.insert(id, Box::new(node));
        Ok(self)
    }

    pub fn edge(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Result<&mut Self, FlowError> {
        self.edge_on(from, Action::default(), to)
    }

    pub fn edge_on(
        &mut self,
        from: impl Into<String>,
        action: Action,
        to: impl Into<String>,
    ) -> Result<&mut Self, FlowError> {
        let from = NodeId::new(from)?;
        let to = NodeId::new(to)?;
        self.ensure_node_exists(&from)?;
        self.ensure_node_exists(&to)?;
        self.transitions.insert((from, action), to);
        Ok(self)
    }

    pub async fn run(&self, ctx: &mut C) -> Result<FlowOutcome, FlowError> {
        self.run_with_options(ctx, FlowRunOptions::default()).await
    }

    pub async fn run_with_options(
        &self,
        ctx: &mut C,
        options: FlowRunOptions,
    ) -> Result<FlowOutcome, FlowError> {
        if !self.nodes.contains_key(&self.start) {
            return self.fail(
                FlowError::MissingStartNode {
                    node: self.start.clone(),
                },
                &options,
            );
        }

        if is_cancelled(&options) {
            return self.fail(FlowError::Cancelled, &options);
        }

        emit(
            &options,
            FlowEvent::Started {
                start: self.start.clone(),
            },
        );

        let mut current = self.start.clone();
        let mut steps = 0usize;
        let mut path = Vec::new();

        loop {
            if is_cancelled(&options) {
                return self.fail(FlowError::Cancelled, &options);
            }
            if steps >= options.max_steps {
                return self.fail(
                    FlowError::MaxStepsExceeded {
                        max_steps: options.max_steps,
                    },
                    &options,
                );
            }

            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| FlowError::UnknownNode {
                    node: current.clone(),
                })?;
            let name = node.name().to_string();
            let step = steps + 1;
            path.push(current.clone());

            emit(
                &options,
                FlowEvent::NodeStart {
                    node: current.clone(),
                    name: name.clone(),
                    step,
                },
            );

            let action = match node.run(ctx).await {
                Ok(action) => action,
                Err(message) => {
                    return self.fail(
                        FlowError::NodeFailed {
                            node: current,
                            message,
                        },
                        &options,
                    );
                }
            };

            emit(
                &options,
                FlowEvent::NodeEnd {
                    node: current.clone(),
                    name,
                    action: action.clone(),
                    step,
                },
            );
            steps = step;

            if let Some(next) = self.transitions.get(&(current.clone(), action.clone())) {
                current = next.clone();
                continue;
            }

            if self.has_outgoing_transition(&current) && options.strict_missing_transition {
                return self.fail(
                    FlowError::MissingTransition {
                        node: current,
                        action,
                    },
                    &options,
                );
            }

            if self.has_outgoing_transition(&current) {
                emit(
                    &options,
                    FlowEvent::MissingTransition {
                        node: current.clone(),
                        action: action.clone(),
                    },
                );
            }

            let outcome = FlowOutcome {
                last_node: current,
                last_action: action,
                steps,
                path,
            };
            emit(
                &options,
                FlowEvent::Completed {
                    outcome: outcome.clone(),
                },
            );
            return Ok(outcome);
        }
    }

    fn ensure_node_exists(&self, node: &NodeId) -> Result<(), FlowError> {
        if self.nodes.contains_key(node) {
            Ok(())
        } else {
            Err(FlowError::UnknownNode { node: node.clone() })
        }
    }

    fn has_outgoing_transition(&self, node: &NodeId) -> bool {
        self.transitions.keys().any(|(from, _)| from == node)
    }

    fn fail<T>(&self, error: FlowError, options: &FlowRunOptions) -> Result<T, FlowError> {
        emit(
            options,
            FlowEvent::Error {
                error: error.clone(),
            },
        );
        Err(error)
    }
}

fn is_cancelled(options: &FlowRunOptions) -> bool {
    options
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.is_cancelled())
}

fn emit(options: &FlowRunOptions, event: FlowEvent) {
    if let Some(on_event) = &options.on_event {
        on_event(event);
    }
}
