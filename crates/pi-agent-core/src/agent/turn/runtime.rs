use async_stream::stream;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, RwLock};

use super::nodes::AgentTurnDecision;
use super::{context::AgentTurnContext, nodes};
use crate::agent::AgentState;
use crate::agent::types::{AgentEvent, AgentStream};

const ACTION_CONTINUE: &str = "continue";
const ACTION_CONTINUE_PROVIDER: &str = "continue_provider";
const ACTION_TOOLS: &str = "tools";
const MAX_TYPED_STATE_STEPS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentTurnState {
    Finish,
    Start,
    DrainQueuedInput,
    CompactRuntimeContext,
    PrepareProviderRequest,
    ApplyProviderHook,
    ProviderStream,
    DecideAfterAssistant,
    ExecuteTools,
    PrepareNextTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentTurnResult {
    Continue,
    Finish,
}

pub struct AgentTurnRunner;

impl AgentTurnRunner {
    pub(crate) fn run_state(state: Arc<RwLock<AgentState>>) -> AgentStream {
        Box::pin(stream! {
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

                let mut run = Box::pin(run_typed_turn(&mut context, cancel)).fuse();
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
                            error: if context.cancel_token.is_cancelled() {
                                "aborted".into()
                            } else {
                                error
                            },
                        };
                        return;
                    }
                };

                turn = context.turn;

                {
                    let mut state = state.write().unwrap();
                    context.apply_to_state(&mut state);
                }

                match outcome {
                    AgentTurnResult::Continue => continue,
                    AgentTurnResult::Finish => return,
                }
            }
        })
    }
}

async fn run_typed_turn(
    ctx: &mut AgentTurnContext,
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<AgentTurnResult, String> {
    let mut state = AgentTurnState::Start;
    let mut steps = 0usize;
    loop {
        steps += 1;
        if steps > MAX_TYPED_STATE_STEPS {
            return Err(format!(
                "typed AgentTurn exceeded {MAX_TYPED_STATE_STEPS} state steps"
            ));
        }
        state = match state {
            AgentTurnState::Finish => return Ok(AgentTurnResult::Finish),
            AgentTurnState::Start => {
                let action = nodes::start_turn(ctx)?;
                transition_from_action(AgentTurnState::Start, action)?
            }
            AgentTurnState::DrainQueuedInput => {
                nodes::drain_queued_input(ctx);
                AgentTurnState::CompactRuntimeContext
            }
            AgentTurnState::CompactRuntimeContext => {
                nodes::maybe_compact_runtime_context(ctx).await?;
                AgentTurnState::PrepareProviderRequest
            }
            AgentTurnState::PrepareProviderRequest => {
                let action = nodes::prepare_provider_request(ctx).await?;
                transition_from_action(AgentTurnState::PrepareProviderRequest, action)?
            }
            AgentTurnState::ApplyProviderHook => {
                let action = nodes::apply_before_provider_request_hook(ctx).await?;
                transition_from_action(AgentTurnState::ApplyProviderHook, action)?
            }
            AgentTurnState::ProviderStream => {
                let action = nodes::stream_provider(ctx).await?;
                transition_from_action(AgentTurnState::ProviderStream, action)?
            }
            AgentTurnState::DecideAfterAssistant => {
                let action = nodes::decide_after_assistant(ctx)?;
                transition_from_action(AgentTurnState::DecideAfterAssistant, action)?
            }
            AgentTurnState::ExecuteTools => {
                let action = nodes::execute_tools(ctx).await?;
                transition_from_action(AgentTurnState::ExecuteTools, action)?
            }
            AgentTurnState::PrepareNextTurn => {
                let action = nodes::maybe_prepare_next_turn(ctx).await?;
                return match action.as_str() {
                    ACTION_CONTINUE | ACTION_CONTINUE_PROVIDER => Ok(AgentTurnResult::Continue),
                    "done" | "error" | "aborted" => Ok(AgentTurnResult::Finish),
                    action => Err(format!(
                        "typed AgentTurn transition from PrepareNextTurn has unknown action '{action}'"
                    )),
                };
            }
        };

        if cancellation.is_cancelled() {
            return Ok(AgentTurnResult::Finish);
        }
    }
}

fn transition_from_action(
    state: AgentTurnState,
    action: AgentTurnDecision,
) -> Result<AgentTurnState, String> {
    let action = action.as_str();
    match (state, action) {
        (AgentTurnState::Start, "default") => Ok(AgentTurnState::DrainQueuedInput),
        (AgentTurnState::PrepareProviderRequest, "default") => {
            Ok(AgentTurnState::ApplyProviderHook)
        }
        (AgentTurnState::ApplyProviderHook, "default") => Ok(AgentTurnState::ProviderStream),
        (AgentTurnState::ProviderStream, "default") => Ok(AgentTurnState::DecideAfterAssistant),
        (AgentTurnState::DecideAfterAssistant, ACTION_TOOLS) => Ok(AgentTurnState::ExecuteTools),
        (AgentTurnState::DecideAfterAssistant, ACTION_CONTINUE) => {
            Ok(AgentTurnState::PrepareNextTurn)
        }
        (AgentTurnState::ExecuteTools, ACTION_CONTINUE) => Ok(AgentTurnState::PrepareNextTurn),
        (_, "error" | "aborted") => Ok(AgentTurnState::Finish),
        (state, action) => Err(format!(
            "typed AgentTurn transition from {state:?} has unknown action '{action}'"
        )),
    }
}
