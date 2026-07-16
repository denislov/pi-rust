use std::collections::VecDeque;
use std::str::FromStr;

use super::types::AgentMessage;

// ── QueueMode ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueMode {
    #[default]
    All,
    OneAtATime,
}

impl std::fmt::Display for QueueMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            QueueMode::All => "all",
            QueueMode::OneAtATime => "one-at-a-time",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for QueueMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(QueueMode::All),
            "one-at-a-time" => Ok(QueueMode::OneAtATime),
            _ => Err(format!("unknown queue mode: {}", s)),
        }
    }
}

pub fn drain_queue(queue: &mut VecDeque<AgentMessage>, mode: QueueMode) -> Vec<AgentMessage> {
    match mode {
        QueueMode::All => queue.drain(..).collect(),
        QueueMode::OneAtATime => queue.pop_front().into_iter().collect(),
    }
}
