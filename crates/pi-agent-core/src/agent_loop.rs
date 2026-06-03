use std::sync::{Arc, RwLock};
use crate::agent::AgentState;
use crate::types::AgentStream;

pub fn run_loop(_state: Arc<RwLock<AgentState>>) -> AgentStream {
    unimplemented!()
}
