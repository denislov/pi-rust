//! Compatibility wrapper for the legacy agent loop entrypoint.
//!
//! Use `Agent::run()` for the public low-level event stream and `AgentTurnFlow`
//! for the internal Flow runtime entrypoint.

use std::sync::{Arc, RwLock};

use crate::agent::AgentState;
use crate::agent_turn_flow::AgentTurnFlow;
use crate::types::AgentStream;

pub fn run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream {
    AgentTurnFlow::run_state(state)
}
