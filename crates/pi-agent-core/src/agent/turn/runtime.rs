use async_stream::stream;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, RwLock};

use super::nodes::{AgentTurnDecision, AgentTurnError};
use super::{context::AgentTurnContext, nodes};
use crate::agent::AgentState;
use crate::agent::types::{AgentEvent, AgentStream};

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
                                error.to_string()
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
) -> Result<AgentTurnResult, AgentTurnError> {
    let mut state = AgentTurnState::Start;
    let mut steps = 0usize;
    loop {
        steps += 1;
        if steps > MAX_TYPED_STATE_STEPS {
            return Err(AgentTurnError::Invariant(format!(
                "typed AgentTurn exceeded {MAX_TYPED_STATE_STEPS} state steps"
            )));
        }
        state = match state {
            AgentTurnState::Finish => return Ok(AgentTurnResult::Finish),
            AgentTurnState::Start => {
                let decision = nodes::start_turn(ctx)?;
                transition_from_decision(AgentTurnState::Start, decision)?
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
                let decision = nodes::prepare_provider_request(ctx).await?;
                transition_from_decision(AgentTurnState::PrepareProviderRequest, decision)?
            }
            AgentTurnState::ApplyProviderHook => {
                let decision = nodes::apply_before_provider_request_hook(ctx).await?;
                transition_from_decision(AgentTurnState::ApplyProviderHook, decision)?
            }
            AgentTurnState::ProviderStream => {
                let decision = nodes::stream_provider(ctx).await?;
                transition_from_decision(AgentTurnState::ProviderStream, decision)?
            }
            AgentTurnState::DecideAfterAssistant => {
                let decision = nodes::decide_after_assistant(ctx)?;
                transition_from_decision(AgentTurnState::DecideAfterAssistant, decision)?
            }
            AgentTurnState::ExecuteTools => {
                let decision = nodes::execute_tools(ctx).await?;
                transition_from_decision(AgentTurnState::ExecuteTools, decision)?
            }
            AgentTurnState::PrepareNextTurn => {
                let decision = nodes::maybe_prepare_next_turn(ctx).await?;
                return match decision {
                    AgentTurnDecision::Continue | AgentTurnDecision::ContinueProvider => {
                        Ok(AgentTurnResult::Continue)
                    }
                    AgentTurnDecision::Done
                    | AgentTurnDecision::Error
                    | AgentTurnDecision::Aborted => Ok(AgentTurnResult::Finish),
                    AgentTurnDecision::Next | AgentTurnDecision::Tools => {
                        Err(AgentTurnError::Invariant(format!(
                            "typed AgentTurn transition from PrepareNextTurn has unexpected decision {decision:?}"
                        )))
                    }
                };
            }
        };

        if cancellation.is_cancelled() {
            return Ok(AgentTurnResult::Finish);
        }
    }
}

fn transition_from_decision(
    state: AgentTurnState,
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    match state {
        AgentTurnState::Start => transition_from_start(decision),
        AgentTurnState::PrepareProviderRequest => transition_from_prepare_provider(decision),
        AgentTurnState::ApplyProviderHook => transition_from_provider_hook(decision),
        AgentTurnState::ProviderStream => transition_from_provider_stream(decision),
        AgentTurnState::DecideAfterAssistant => transition_from_assistant(decision),
        AgentTurnState::ExecuteTools => transition_from_tools(decision),
        AgentTurnState::Finish
        | AgentTurnState::DrainQueuedInput
        | AgentTurnState::CompactRuntimeContext
        | AgentTurnState::PrepareNextTurn => unexpected_decision(state, decision),
    }
}

fn transition_from_start(decision: AgentTurnDecision) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Next => Ok(AgentTurnState::DrainQueuedInput),
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Continue
        | AgentTurnDecision::ContinueProvider
        | AgentTurnDecision::Tools
        | AgentTurnDecision::Done => unexpected_decision(AgentTurnState::Start, decision),
    }
}

fn transition_from_prepare_provider(
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Next => Ok(AgentTurnState::ApplyProviderHook),
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Continue
        | AgentTurnDecision::ContinueProvider
        | AgentTurnDecision::Tools
        | AgentTurnDecision::Done => {
            unexpected_decision(AgentTurnState::PrepareProviderRequest, decision)
        }
    }
}

fn transition_from_provider_hook(
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Next => Ok(AgentTurnState::ProviderStream),
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Continue
        | AgentTurnDecision::ContinueProvider
        | AgentTurnDecision::Tools
        | AgentTurnDecision::Done => {
            unexpected_decision(AgentTurnState::ApplyProviderHook, decision)
        }
    }
}

fn transition_from_provider_stream(
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Next => Ok(AgentTurnState::DecideAfterAssistant),
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Continue
        | AgentTurnDecision::ContinueProvider
        | AgentTurnDecision::Tools
        | AgentTurnDecision::Done => unexpected_decision(AgentTurnState::ProviderStream, decision),
    }
}

fn transition_from_assistant(
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Continue => Ok(AgentTurnState::PrepareNextTurn),
        AgentTurnDecision::Tools => Ok(AgentTurnState::ExecuteTools),
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Next | AgentTurnDecision::ContinueProvider | AgentTurnDecision::Done => {
            unexpected_decision(AgentTurnState::DecideAfterAssistant, decision)
        }
    }
}

fn transition_from_tools(decision: AgentTurnDecision) -> Result<AgentTurnState, AgentTurnError> {
    match decision {
        AgentTurnDecision::Continue | AgentTurnDecision::ContinueProvider => {
            Ok(AgentTurnState::PrepareNextTurn)
        }
        AgentTurnDecision::Error | AgentTurnDecision::Aborted => Ok(AgentTurnState::Finish),
        AgentTurnDecision::Next | AgentTurnDecision::Tools | AgentTurnDecision::Done => {
            unexpected_decision(AgentTurnState::ExecuteTools, decision)
        }
    }
}

fn unexpected_decision(
    state: AgentTurnState,
    decision: AgentTurnDecision,
) -> Result<AgentTurnState, AgentTurnError> {
    Err(AgentTurnError::Invariant(format!(
        "typed AgentTurn transition from {state:?} has unexpected decision {decision:?}"
    )))
}
