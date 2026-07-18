#[cfg(any(test, feature = "test-support"))]
use crate::agent::turn::AgentTurnContext;
use crate::agent::turn::AgentTurnFlow;
use crate::agent::types::{
    AgentConfig, AgentMessage, AgentResources, AgentStream, AgentTool, AgentToolDefinitionError,
};
use crate::context::conversion::convert_to_context;
use crate::hooks::BeforeProviderRequestHook;
use crate::resources::{format_prompt_template_invocation, format_skill_invocation};
use pi_ai::api::conversation::Context;
use pi_ai::api::stream::StreamOptions;
use std::collections::VecDeque;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentAdmissionError {
    #[error("agent is busy while starting {operation}")]
    Busy { operation: &'static str },
    #[error("cannot continue: no messages in context")]
    EmptyContext,
    #[error("cannot continue from message role: assistant")]
    AssistantTail,
}

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

    pub fn try_add_tool(&self, tool: AgentTool) -> Result<(), AgentToolDefinitionError> {
        tool.validate()?;
        self.add_tool(tool);
        Ok(())
    }

    pub fn add_message(&self, msg: AgentMessage) {
        self.state.write().unwrap().messages.push(msg);
    }

    pub fn messages(&self) -> Vec<AgentMessage> {
        self.state.read().unwrap().messages.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn agent_turn_context_snapshot(&self) -> AgentTurnContext {
        let state = self.state.read().unwrap();
        AgentTurnContext::from_state(&state)
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
        let mut state = self.state.write().unwrap();
        let message_id = next_message_id(&state, "steer");
        state.steering_queue.push_back(AgentMessage::UserText {
            message_id,
            text: text.into(),
        });
    }

    pub fn steer_content(&self, content: Vec<pi_ai::api::conversation::ContentBlock>) {
        let mut state = self.state.write().unwrap();
        let message_id = next_message_id(&state, "steer");
        state.steering_queue.push_back(AgentMessage::Custom {
            message_id,
            custom_type: "input".into(),
            content,
            display: true,
            details: None,
            timestamp: 0,
        });
    }

    pub fn follow_up(&self, text: impl Into<String>) {
        let mut state = self.state.write().unwrap();
        let message_id = next_message_id(&state, "followup");
        state.follow_up_queue.push_back(AgentMessage::UserText {
            message_id,
            text: text.into(),
        });
    }

    pub fn follow_up_content(&self, content: Vec<pi_ai::api::conversation::ContentBlock>) {
        let mut state = self.state.write().unwrap();
        let message_id = next_message_id(&state, "followup");
        state.follow_up_queue.push_back(AgentMessage::Custom {
            message_id,
            custom_type: "input".into(),
            content,
            display: true,
            details: None,
            timestamp: 0,
        });
    }

    pub fn clear_queues(&self) {
        let mut state = self.state.write().unwrap();
        state.steering_queue.clear();
        state.follow_up_queue.clear();
    }

    /// Drain and return all queued steering messages.
    pub fn drain_steering_queue(&self) -> Vec<AgentMessage> {
        let mut state = self.state.write().unwrap();
        state.steering_queue.drain(..).collect()
    }

    /// Drain and return all queued follow-up messages.
    pub fn drain_follow_up_queue(&self) -> Vec<AgentMessage> {
        let mut state = self.state.write().unwrap();
        state.follow_up_queue.drain(..).collect()
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
        self.try_prompt_internal(prompt)
            .map_err(|error| error.to_string())
    }

    pub fn prompt_from_template(&self, name: &str, args: &[String]) -> Result<AgentStream, String> {
        let resources = self.state.read().unwrap().config.resources.clone();
        let template = resources
            .prompt_templates
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("prompt template '{name}' not found"))?;
        let prompt = format_prompt_template_invocation(&template.name, &template.content, args);
        self.try_prompt_internal(prompt)
            .map_err(|error| error.to_string())
    }

    fn try_prompt_internal(&self, text: String) -> Result<AgentStream, AgentAdmissionError> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| AgentAdmissionError::Busy {
                operation: "prompt",
            })?;
        {
            let mut state = self.state.write().unwrap();
            state.cancel_token = CancellationToken::new();
            let message_id = next_message_id(&state, "user");
            state
                .messages
                .push(AgentMessage::UserText { message_id, text });
        }

        Ok(self.run_locked())
    }

    fn run_locked(&self) -> AgentStream {
        let state = self.state.clone();
        let guard = RunGuard {
            flag: self.running.clone(),
        };
        Box::pin(async_stream::stream! {
            let _guard = guard;
            let mut stream = AgentTurnFlow::run_state(state);
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
        match self.try_prompt(text) {
            Ok(stream) => stream,
            Err(error) => error_stream(error.to_string()),
        }
    }

    pub fn try_prompt(&self, text: &str) -> Result<AgentStream, AgentAdmissionError> {
        self.try_prompt_internal(text.to_string())
    }

    /// Runs the model/tool loop with the messages already present on the agent.
    /// Harness code uses this when it needs to transform or patch messages before
    /// starting a turn.
    ///
    /// Mirrors TS `agentLoopContinue`: returns `Err` if `messages` is empty or
    /// the last message is an assistant message.
    pub fn run(&self) -> Result<AgentStream, String> {
        self.try_run().map_err(|error| error.to_string())
    }

    pub fn try_run(&self) -> Result<AgentStream, AgentAdmissionError> {
        {
            let s = self.state.read().unwrap();
            if s.messages.is_empty() {
                return Err(AgentAdmissionError::EmptyContext);
            }
            if matches!(s.messages.last(), Some(AgentMessage::Assistant { .. })) {
                return Err(AgentAdmissionError::AssistantTail);
            }
        }

        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| AgentAdmissionError::Busy { operation: "run" })?;

        {
            self.state.write().unwrap().cancel_token = CancellationToken::new();
        }

        Ok(self.run_locked())
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

fn error_stream(error: String) -> AgentStream {
    Box::pin(async_stream::stream! {
        yield crate::agent::types::AgentEvent::AgentError { error };
    })
}

fn next_message_id(state: &AgentState, prefix: &str) -> String {
    let mut index = 0u64;
    loop {
        let candidate = format!("{prefix}_{index}");
        let used = state
            .messages
            .iter()
            .chain(state.steering_queue.iter())
            .chain(state.follow_up_queue.iter())
            .any(|message| message.message_id() == candidate);
        if !used {
            return candidate;
        }
        index += 1;
    }
}
