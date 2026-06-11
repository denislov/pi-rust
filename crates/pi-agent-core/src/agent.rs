use crate::agent_loop;
use crate::resources::{format_prompt_template_invocation, format_skill_invocation};
use crate::types::{AgentConfig, AgentMessage, AgentResources, AgentStream, AgentTool};
use std::collections::VecDeque;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};
use tokio_util::sync::CancellationToken;

pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
    pub steering_queue: VecDeque<AgentMessage>,
    pub follow_up_queue: VecDeque<AgentMessage>,
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

impl Clone for Agent {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            running: Arc::clone(&self.running),
        }
    }
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(AgentState {
                messages: Vec::new(),
                tools: Vec::new(),
                cancel_token: CancellationToken::new(),
                config,
                steering_queue: VecDeque::new(),
                follow_up_queue: VecDeque::new(),
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

    pub fn set_resources(&self, resources: AgentResources) {
        self.state.write().unwrap().config.resources = resources;
    }

    pub fn steer(&self, text: impl Into<String>) {
        let msg_count = self.state.read().unwrap().steering_queue.len();
        self.state
            .write()
            .unwrap()
            .steering_queue
            .push_back(AgentMessage::UserText {
                message_id: format!("steer_{}", msg_count),
                text: text.into(),
            });
    }

    pub fn follow_up(&self, text: impl Into<String>) {
        let msg_count = self.state.read().unwrap().follow_up_queue.len();
        self.state
            .write()
            .unwrap()
            .follow_up_queue
            .push_back(AgentMessage::UserText {
                message_id: format!("followup_{}", msg_count),
                text: text.into(),
            });
    }

    pub fn clear_queues(&self) {
        let mut state = self.state.write().unwrap();
        state.steering_queue.clear();
        state.follow_up_queue.clear();
    }

    pub fn skill(
        &self,
        name: &str,
        additional_instructions: Option<&str>,
    ) -> Result<AgentStream, String> {
        let resources = self.state.read().unwrap().config.resources.clone();
        let skill = resources
            .skills
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| format!("skill '{name}' not found"))?;
        let prompt = format_skill_invocation(
            &skill.name,
            &skill.location,
            &skill.content,
            additional_instructions,
        );
        Ok(self.prompt_internal(prompt))
    }

    pub fn prompt_from_template(&self, name: &str, args: &[String]) -> Result<AgentStream, String> {
        let resources = self.state.read().unwrap().config.resources.clone();
        let template = resources
            .prompt_templates
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("prompt template '{name}' not found"))?;
        let prompt = format_prompt_template_invocation(&template.name, &template.content, args);
        Ok(self.prompt_internal(prompt))
    }

    fn prompt_internal(&self, text: String) -> AgentStream {
        if self.running.swap(true, Ordering::SeqCst) {
            panic!("prompt() called while agent is already running");
        }

        {
            let mut state = self.state.write().unwrap();
            state.cancel_token = CancellationToken::new();
            let msg_count = state.messages.len();
            state.messages.push(AgentMessage::UserText {
                message_id: format!("user_{}", msg_count),
                text,
            });
        }

        let state = self.state.clone();
        let guard = RunGuard {
            flag: self.running.clone(),
        };
        Box::pin(async_stream::stream! {
            let _guard = guard;
            let mut stream = agent_loop::run_loop(state);
            use futures::StreamExt;
            while let Some(event) = stream.next().await {
                yield event;
            }
        })
    }

    /// Adds a UserText message and runs the full tool-calling loop.
    /// Returns an AgentStream that yields events until the model stops
    /// or an error occurs.
    pub fn prompt(&self, text: &str) -> AgentStream {
        self.prompt_internal(text.to_string())
    }

    pub fn with_messages(config: AgentConfig, messages: Vec<AgentMessage>) -> Self {
        let agent = Self::new(config);
        agent.replace_messages(messages);
        agent
    }

    pub fn replace_messages(&self, messages: Vec<AgentMessage>) {
        self.state.write().unwrap().messages = messages;
    }

    /// Cancels an in-flight loop. Safe to call from another task.
    pub fn abort(&self) {
        self.state.read().unwrap().cancel_token.cancel();
    }
}
