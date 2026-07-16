use async_stream::stream;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, RwLock};

use super::{context::AgentTurnContext, nodes};
use crate::agent::AgentState;
use crate::agent::types::{AgentEvent, AgentStream};
use crate::flow::{Action, Flow, FlowError, FlowOutcome, FlowRunOptions};

pub const AGENT_TURN_NODE_IDS: &[&str] = &[
    "start_turn",
    "drain_queued_input",
    "maybe_compact_runtime_context",
    "prepare_provider_request",
    "apply_before_provider_request_hook",
    "provider_stream",
    "decide_after_assistant",
    "maybe_prepare_next_turn",
    "execute_tools",
];

const ACTION_CONTINUE: &str = "continue";
const ACTION_CONTINUE_PROVIDER: &str = "continue_provider";
const ACTION_TOOLS: &str = "tools";

pub struct AgentTurnFlow {
    flow: Flow<AgentTurnContext>,
}

impl AgentTurnFlow {
    pub fn new() -> Result<Self, FlowError> {
        let mut flow = Flow::new(AGENT_TURN_NODE_IDS[0])?;
        flow.add_node("start_turn", nodes::StartTurnNode)?
            .add_node("drain_queued_input", nodes::DrainQueuedInputNode)?
            .add_node(
                "maybe_compact_runtime_context",
                nodes::MaybeCompactRuntimeContextNode,
            )?
            .add_node(
                "prepare_provider_request",
                nodes::PrepareProviderRequestNode,
            )?
            .add_node(
                "apply_before_provider_request_hook",
                nodes::ApplyBeforeProviderRequestHookNode,
            )?
            .add_node("provider_stream", nodes::ProviderStreamNode)?
            .add_node("decide_after_assistant", nodes::DecideAfterAssistantNode)?
            .add_node("maybe_prepare_next_turn", nodes::MaybePrepareNextTurnNode)?
            .add_node("execute_tools", nodes::ExecuteToolsNode)?
            .edge("start_turn", "drain_queued_input")?
            .edge("drain_queued_input", "maybe_compact_runtime_context")?
            .edge("maybe_compact_runtime_context", "prepare_provider_request")?
            .edge(
                "prepare_provider_request",
                "apply_before_provider_request_hook",
            )?
            .edge("apply_before_provider_request_hook", "provider_stream")?
            .edge("provider_stream", "decide_after_assistant")?
            .edge_on(
                "decide_after_assistant",
                Action::new(ACTION_TOOLS)?,
                "execute_tools",
            )?
            .edge_on(
                "decide_after_assistant",
                Action::new(ACTION_CONTINUE)?,
                "maybe_prepare_next_turn",
            )?
            .edge_on(
                "execute_tools",
                Action::new(ACTION_CONTINUE)?,
                "maybe_prepare_next_turn",
            )?;

        Ok(Self { flow })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub async fn run(&self, ctx: &mut AgentTurnContext) -> Result<FlowOutcome, FlowError> {
        self.run_with_options(ctx, FlowRunOptions::default()).await
    }

    pub async fn run_with_options(
        &self,
        ctx: &mut AgentTurnContext,
        mut options: FlowRunOptions,
    ) -> Result<FlowOutcome, FlowError> {
        options.strict_missing_transition = false;
        self.flow.run_with_options(ctx, options).await
    }

    pub(crate) fn run_state(state: Arc<RwLock<AgentState>>) -> AgentStream {
        Box::pin(stream! {
            let flow = match AgentTurnFlow::new() {
                Ok(flow) => flow,
                Err(error) => {
                    yield AgentEvent::AgentError {
                        error: error.to_string(),
                    };
                    return;
                }
            };

            let mut turn: u32 = 0;

            loop {
                let mut context = {
                    let mut state = state.write().unwrap();
                    let context = AgentTurnContext::from_state(&state);
                    state.steering_queue.clear();
                    state.follow_up_queue.clear();
                    context
                };
                context.turn = turn;
                let cancel = context.cancel_token.clone();
                let (event_sender, mut event_receiver) = mpsc::unbounded();
                context.attach_runtime(Arc::clone(&state), event_sender);

                let mut run = Box::pin(flow.run_with_options(
                    &mut context,
                    FlowRunOptions {
                        cancel: Some(cancel),
                        ..FlowRunOptions::default()
                    },
                ))
                .fuse();
                let outcome_result = loop {
                    futures::select! {
                        event = event_receiver.next().fuse() => {
                            if let Some(event) = event {
                                yield event;
                            }
                        }
                        outcome = &mut run => break outcome,
                    }
                };
                drop(run);
                while let Some(Some(event)) = event_receiver.next().now_or_never() {
                    yield event;
                }

                let outcome = match outcome_result {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        {
                            let mut state = state.write().unwrap();
                            context.apply_to_state(&mut state);
                        }
                        yield AgentEvent::AgentError {
                            error: flow_error_message(error),
                        };
                        return;
                    }
                };

                turn = context.turn;

                {
                    let mut state = state.write().unwrap();
                    context.apply_to_state(&mut state);
                }

                match outcome.last_action.as_str() {
                    ACTION_CONTINUE | ACTION_CONTINUE_PROVIDER => continue,
                    _ => return,
                }
            }
        })
    }
}

fn flow_error_message(error: FlowError) -> String {
    match error {
        FlowError::Cancelled => "aborted".into(),
        error => error.to_string(),
    }
}
