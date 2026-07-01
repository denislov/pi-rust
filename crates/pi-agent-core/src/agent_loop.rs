use std::sync::{Arc, RwLock};

use crate::agent::AgentState;
use crate::agent_turn_flow::AgentTurnFlow;
use crate::types::AgentStream;

pub fn run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream {
    AgentTurnFlow::run_state(state)
}
