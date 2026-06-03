use std::sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}};
use tokio_util::sync::CancellationToken;
use crate::types::{AgentMessage, AgentTool, AgentConfig, AgentStream};
use crate::agent_loop;

pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
}

struct RunGuard {
    flag: Arc<AtomicBool>,
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

pub struct Agent {
    state: Arc<RwLock<AgentState>>,
    running: Arc<AtomicBool>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(AgentState {
                messages: Vec::new(),
                tools: Vec::new(),
                cancel_token: CancellationToken::new(),
                config,
            })),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn add_tool(&self, tool: AgentTool) {
        self.state.write().unwrap().tools.push(tool);
    }

    pub fn add_message(&self, msg: AgentMessage) {
        self.state.write().unwrap().messages.push(msg);
    }

    pub fn messages(&self) -> Vec<AgentMessage> {
        self.state.read().unwrap().messages.clone()
    }

    /// Adds a UserText message and runs the full tool-calling loop.
    /// Returns an AgentStream that yields events until the model stops
    /// or an error occurs.
    pub fn prompt(&self, text: &str) -> AgentStream {
        if self.running.swap(true, Ordering::SeqCst) {
            panic!("prompt() called while agent is already running");
        }

        {
            let mut state = self.state.write().unwrap();
            state.cancel_token = CancellationToken::new();
            let msg_count = state.messages.len();
            state.messages.push(AgentMessage::UserText {
                message_id: format!("user_{}", msg_count),
                text: text.to_string(),
            });
        }

        let state = self.state.clone();
        let guard = RunGuard { flag: self.running.clone() };
        Box::pin(async_stream::stream! {
            let _guard = guard;
            let mut stream = agent_loop::run_loop(state);
            use futures::StreamExt;
            while let Some(event) = stream.next().await {
                yield event;
            }
        })
    }

    /// Cancels an in-flight loop. Safe to call from another task.
    pub fn abort(&self) {
        self.state.read().unwrap().cancel_token.cancel();
    }
}
