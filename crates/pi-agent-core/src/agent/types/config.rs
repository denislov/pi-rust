use pi_ai::api::model::Model;
use pi_ai::api::stream::StreamOptions;

use crate::hooks::AgentHooks;

use super::{AgentResources, ProviderStreamer, QueueMode, ThinkingLevel, ToolExecutionMode};

// ── Compaction types ───────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u32,
    pub keep_recent_tokens: u32,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompactionConfig {
    pub settings: CompactionSettings,
    pub custom_instructions: Option<String>,
}

// ── AgentConfig ────────────────────────────────────

#[derive(Clone)]
pub struct AgentConfig {
    pub model: Model,
    pub system_prompt: Option<String>,
    /// Optional turn ceiling. `None` means no hard cap (the loop only stops
    /// when the model finishes or an explicit hook requests it). Provided to
    /// match the TS `pi/packages/agent` `while (true)` semantics.
    pub max_turns: Option<u32>,
    pub stream_options: Option<StreamOptions>,
    pub thinking_level: ThinkingLevel,
    pub tool_execution: ToolExecutionMode,
    /// Generic caller-owned identity attached to every tool invocation.
    pub tool_execution_scope: Option<String>,
    pub steering_mode: QueueMode,
    pub follow_up_mode: QueueMode,
    pub hooks: AgentHooks,
    pub resources: AgentResources,
    pub compaction: Option<CompactionConfig>,
    pub provider_streamer: Option<ProviderStreamer>,
}

impl std::fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field("model", &self.model)
            .field("system_prompt", &self.system_prompt)
            .field("max_turns", &self.max_turns)
            .field("stream_options", &self.stream_options)
            .field("thinking_level", &self.thinking_level)
            .field("tool_execution", &self.tool_execution)
            .field("tool_execution_scope", &self.tool_execution_scope)
            .field("steering_mode", &self.steering_mode)
            .field("follow_up_mode", &self.follow_up_mode)
            .field("hooks", &self.hooks)
            .field("resources", &self.resources)
            .field("compaction", &self.compaction)
            .field("provider_streamer", &self.provider_streamer.is_some())
            .finish()
    }
}

impl AgentConfig {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            system_prompt: None,
            max_turns: None,
            stream_options: None,
            thinking_level: ThinkingLevel::Off,
            tool_execution: ToolExecutionMode::Parallel,
            tool_execution_scope: None,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            hooks: AgentHooks::default(),
            resources: AgentResources::default(),
            compaction: None,
            provider_streamer: None,
        }
    }
}
