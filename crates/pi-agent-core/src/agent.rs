use crate::agent_loop;
use crate::convert::convert_to_context;
use crate::hooks::BeforeProviderRequestHook;
use crate::resources::{format_prompt_template_invocation, format_skill_invocation};
use crate::types::{AgentConfig, AgentMessage, AgentResources, AgentStream, AgentTool};
use pi_ai::types::{Context, StreamOptions};
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
    pub(crate) provider_request_override: Option<ProviderRequestOverride>,
}

pub(crate) struct ProviderRequestOverride {
    pub context: Context,
    pub stream_options: Option<StreamOptions>,
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
                provider_request_override: None,
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

    pub fn before_provider_request_hook(&self) -> Option<BeforeProviderRequestHook> {
        self.state
            .read()
            .unwrap()
            .config
            .hooks
            .before_provider_request
            .clone()
    }

    pub fn set_before_provider_request_hook(&self, hook: Option<BeforeProviderRequestHook>) {
        self.state
            .write()
            .unwrap()
            .config
            .hooks
            .before_provider_request = hook;
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

        self.run_locked()
    }

    fn run_locked(&self) -> AgentStream {
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

    /// Runs the model/tool loop with the messages already present on the agent.
    /// Harness code uses this when it needs to transform or patch messages before
    /// starting a turn.
    pub fn run(&self) -> AgentStream {
        if self.running.swap(true, Ordering::SeqCst) {
            panic!("run() called while agent is already running");
        }

        {
            self.state.write().unwrap().cancel_token = CancellationToken::new();
        }

        self.run_locked()
    }

    pub fn with_messages(config: AgentConfig, messages: Vec<AgentMessage>) -> Self {
        let agent = Self::new(config);
        agent.replace_messages(messages);
        agent
    }

    pub fn replace_messages(&self, messages: Vec<AgentMessage>) {
        self.state.write().unwrap().messages = messages;
    }

    pub fn provider_request_snapshot(&self) -> (Context, Option<StreamOptions>) {
        let state = self.state.read().unwrap();
        let context = convert_to_context(
            &state.config.system_prompt,
            &state.messages,
            &state.tools,
            &state.config.resources,
        );
        (context, state.config.stream_options.clone())
    }

    pub fn set_provider_request_override(
        &self,
        context: Context,
        stream_options: Option<StreamOptions>,
    ) {
        self.state.write().unwrap().provider_request_override = Some(ProviderRequestOverride {
            context,
            stream_options,
        });
    }

    /// Cancels an in-flight loop. Safe to call from another task.
    pub fn abort(&self) {
        self.state.read().unwrap().cancel_token.cancel();
    }
}
