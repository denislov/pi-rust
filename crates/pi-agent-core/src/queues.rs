use crate::types::{AgentMessage, QueueMode};
use std::collections::VecDeque;

pub fn drain_queue(queue: &mut VecDeque<AgentMessage>, mode: QueueMode) -> Vec<AgentMessage> {
    match mode {
        QueueMode::All => queue.drain(..).collect(),
        QueueMode::OneAtATime => queue.pop_front().into_iter().collect(),
    }
}
