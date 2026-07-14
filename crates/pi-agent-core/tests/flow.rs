use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use pi_agent_core::flow::{Action, Flow, FlowError, FlowEvent, FlowNode, FlowRunOptions, NodeId};
use tokio_util::sync::CancellationToken;

struct PushNode {
    name: &'static str,
    value: &'static str,
    action: &'static str,
}

impl FlowNode<Vec<&'static str>> for PushNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut Vec<&'static str>,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            ctx.push(self.value);
            Action::new(self.action).map_err(|err| err.to_string())
        })
    }
}

#[derive(Default)]
struct RetryContext {
    attempts: usize,
    log: Vec<&'static str>,
}

struct RetryOnceNode;

impl FlowNode<RetryContext> for RetryOnceNode {
    fn name(&self) -> &str {
        "retry-once"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut RetryContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            if ctx.attempts == 0 {
                ctx.attempts += 1;
                ctx.log.push("retry");
                Action::new("retry").map_err(|err| err.to_string())
            } else {
                ctx.log.push("done");
                Action::new("done").map_err(|err| err.to_string())
            }
        })
    }
}

struct RetryFinishNode;

impl FlowNode<RetryContext> for RetryFinishNode {
    fn name(&self) -> &str {
        "finish"
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut RetryContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            ctx.log.push("finish");
            Ok(Action::default())
        })
    }
}

struct FailingNode;

impl FlowNode<Vec<&'static str>> for FailingNode {
    fn name(&self) -> &str {
        "failing"
    }

    fn run<'a>(
        &'a self,
        _ctx: &'a mut Vec<&'static str>,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async { Err("boom".into()) })
    }
}

#[tokio::test]
async fn linear_flow_mutates_typed_context_and_records_path() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start-node",
            value: "a",
            action: "default",
        },
    )
    .unwrap()
    .add_node(
        "second",
        PushNode {
            name: "second-node",
            value: "b",
            action: "done",
        },
    )
    .unwrap()
    .edge("start", "second")
    .unwrap();

    let mut ctx = Vec::new();
    let outcome = flow.run(&mut ctx).await.unwrap();

    assert_eq!(ctx, vec!["a", "b"]);
    assert_eq!(outcome.last_node.as_str(), "second");
    assert_eq!(outcome.last_action.as_str(), "done");
    assert_eq!(outcome.steps, 2);
    assert_eq!(
        outcome.path.iter().map(NodeId::as_str).collect::<Vec<_>>(),
        vec!["start", "second"]
    );
}

#[tokio::test]
async fn conditional_transition_routes_by_action() {
    let mut flow = Flow::new("decide").unwrap();
    flow.add_node("decide", RetryOnceNode)
        .unwrap()
        .add_node("finish", RetryFinishNode)
        .unwrap()
        .edge_on("decide", Action::new("retry").unwrap(), "decide")
        .unwrap()
        .edge_on("decide", Action::new("done").unwrap(), "finish")
        .unwrap();

    let mut ctx = RetryContext::default();
    let outcome = flow.run(&mut ctx).await.unwrap();

    assert_eq!(ctx.attempts, 1);
    assert_eq!(ctx.log, vec!["retry", "done", "finish"]);
    assert_eq!(
        outcome.path.iter().map(NodeId::as_str).collect::<Vec<_>>(),
        vec!["decide", "decide", "finish"]
    );
}

#[test]
fn duplicate_node_is_rejected() {
    let mut flow = Flow::<Vec<&'static str>>::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "one",
            value: "a",
            action: "default",
        },
    )
    .unwrap();

    let err = flow
        .add_node(
            "start",
            PushNode {
                name: "two",
                value: "b",
                action: "default",
            },
        )
        .err()
        .unwrap();

    assert_eq!(
        err,
        FlowError::DuplicateNode {
            node: NodeId::new("start").unwrap()
        }
    );
}

#[test]
fn unknown_edge_endpoint_is_rejected() {
    let mut flow = Flow::<Vec<&'static str>>::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start",
            value: "a",
            action: "default",
        },
    )
    .unwrap();

    let err = flow.edge("start", "missing").err().unwrap();

    assert_eq!(
        err,
        FlowError::UnknownNode {
            node: NodeId::new("missing").unwrap()
        }
    );
}

#[tokio::test]
async fn strict_missing_transition_returns_error() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start",
            value: "a",
            action: "unexpected",
        },
    )
    .unwrap()
    .add_node(
        "end",
        PushNode {
            name: "end",
            value: "b",
            action: "default",
        },
    )
    .unwrap()
    .edge("start", "end")
    .unwrap();

    let mut ctx = Vec::new();
    let err = flow.run(&mut ctx).await.unwrap_err();

    assert_eq!(
        err,
        FlowError::MissingTransition {
            node: NodeId::new("start").unwrap(),
            action: Action::new("unexpected").unwrap()
        }
    );
}

#[tokio::test]
async fn lenient_missing_transition_completes_and_emits_event() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start",
            value: "a",
            action: "unexpected",
        },
    )
    .unwrap()
    .add_node(
        "end",
        PushNode {
            name: "end",
            value: "b",
            action: "default",
        },
    )
    .unwrap()
    .edge("start", "end")
    .unwrap();

    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let options = FlowRunOptions {
        strict_missing_transition: false,
        on_event: Some(Arc::new(move |event| {
            captured.lock().unwrap().push(event);
        })),
        ..Default::default()
    };

    let mut ctx = Vec::new();
    let outcome = flow.run_with_options(&mut ctx, options).await.unwrap();

    assert_eq!(outcome.last_node.as_str(), "start");
    assert_eq!(outcome.last_action.as_str(), "unexpected");
    assert!(events.lock().unwrap().iter().any(
        |event| matches!(event, FlowEvent::MissingTransition { node, action }
                if node.as_str() == "start" && action.as_str() == "unexpected")
    ));
}

#[tokio::test]
async fn self_loop_stops_at_max_steps() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start",
            value: "tick",
            action: "default",
        },
    )
    .unwrap()
    .edge("start", "start")
    .unwrap();

    let mut ctx = Vec::new();
    let err = flow
        .run_with_options(
            &mut ctx,
            FlowRunOptions {
                max_steps: 3,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();

    assert_eq!(err, FlowError::MaxStepsExceeded { max_steps: 3 });
    assert_eq!(ctx, vec!["tick", "tick", "tick"]);
}

#[tokio::test]
async fn node_failure_is_wrapped_with_node_id() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node("start", FailingNode).unwrap();

    let mut ctx = Vec::new();
    let err = flow.run(&mut ctx).await.unwrap_err();

    assert_eq!(
        err,
        FlowError::NodeFailed {
            node: NodeId::new("start").unwrap(),
            message: "boom".into()
        }
    );
}

#[tokio::test]
async fn cancelled_token_stops_before_execution() {
    let mut flow = Flow::new("start").unwrap();
    flow.add_node(
        "start",
        PushNode {
            name: "start",
            value: "should-not-run",
            action: "default",
        },
    )
    .unwrap();

    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut ctx = Vec::new();
    let err = flow
        .run_with_options(
            &mut ctx,
            FlowRunOptions {
                cancel: Some(cancel),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();

    assert_eq!(err, FlowError::Cancelled);
    assert!(ctx.is_empty());
}
