#[doc(hidden)]
pub mod agent;
#[deprecated(
    note = "use Agent::run() for the public low-level stream or AgentTurnFlow for the internal flow runtime"
)]
#[doc(hidden)]
pub mod agent_loop;
#[doc(hidden)]
pub mod agent_turn_flow;
mod ai_runtime;
#[doc(hidden)]
pub mod branch_summary;
#[doc(hidden)]
pub mod compaction;
#[doc(hidden)]
pub mod convert;
#[doc(hidden)]
pub mod env;
#[doc(hidden)]
pub mod errors;
#[doc(hidden)]
pub mod flow;
#[doc(hidden)]
pub mod harness;
#[doc(hidden)]
pub mod hooks;
mod loop_runtime;
#[doc(hidden)]
pub mod proxy;
#[doc(hidden)]
pub mod queues;
#[doc(hidden)]
pub mod resources;
#[doc(hidden)]
pub mod session_context;
#[doc(hidden)]
pub mod shell_output;
#[doc(hidden)]
pub mod transcript;
#[doc(hidden)]
pub mod truncate;
#[doc(hidden)]
pub mod types;

pub use agent::Agent;
pub use env::{
    ExecOptions, ExecutionEnv, ExecutionOutput, FileInfo, FileKind, FileSystem,
    InMemoryExecutionEnv, Shell,
};
pub use errors::{
    AgentHarnessError, AgentHarnessErrorCode, BranchSummaryError, BranchSummaryErrorCode,
    ExecutionError, ExecutionErrorCode, FileError, FileErrorCode,
};
pub use harness::{
    AbortResult, AgentHarness, AgentHarnessEvent, AgentHarnessHooks, AgentHarnessPhase,
    BeforeAgentStartHook, BeforeProviderPayload, BeforeProviderPayloadHook,
    BeforeProviderPayloadPatch, BeforeProviderRequest, BeforeProviderRequestHook,
    BeforeProviderRequestPatch, ContextHook, GetApiKeyAndHeadersHook, HarnessContext,
    HarnessHookFuture, HarnessHookKind, HeaderPatch, Observer, Patch, ProviderAuth,
    ProviderResponse, StreamOptionsPatch, SubscriptionGuard, on_kind,
};
pub use hooks::{
    AfterToolCallContext, AfterToolCallHook, AfterToolCallResult, AgentHooks,
    BeforeToolCallContext, BeforeToolCallHook, BeforeToolCallResult, ConvertToLlmHook,
    ShouldStopAfterTurnHook, TransformContextHook,
};
pub use resources::{parse_command_args, substitute_args};
pub use types::{
    AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentStream, AgentTool,
    AgentToolDefinitionError, AgentToolOutput, AgentToolResult, CompactionConfig,
    CompactionSettings, DiagnosticSeverity, PromptTemplate, ProviderRequestSnapshot, QueueMode,
    ResourceDiagnostic, Skill, SourceTag, SourcedPromptTemplate, SourcedResourceDiagnostic,
    SourcedSkill, ThinkingLevel, ToolExecutionMode, ToolFn, ToolUpdateCallback,
};

/// Stable low-level runtime facade for `pi-agent-core`.
///
/// Product session ownership, adapter wire events, and workflow ownership belong
/// in `pi-coding-agent`. This module intentionally exposes low-level agent,
/// tool, hook, resource, and environment contracts.
pub mod api {
    pub use crate::agent::Agent;
    pub use crate::env::{
        ExecOptions, ExecutionEnv, ExecutionOutput, FileInfo, FileKind, FileSystem,
        InMemoryExecutionEnv, Shell,
    };
    pub use crate::errors::{ExecutionError, ExecutionErrorCode, FileError, FileErrorCode};
    pub use crate::hooks::{
        AfterToolCallContext, AfterToolCallHook, AfterToolCallResult, AgentHooks,
        AgentLoopTurnUpdate, BeforeProviderRequestContext, BeforeProviderRequestHook,
        BeforeProviderRequestResult, BeforeToolCallContext, BeforeToolCallHook,
        BeforeToolCallResult, ConvertToLlmHook, HookFuture, PrepareNextTurnContext,
        PrepareNextTurnHook, ShouldStopAfterTurnContext, ShouldStopAfterTurnHook,
        TransformContextHook,
    };
    pub use crate::resources::{
        format_prompt_template_invocation, format_skill_invocation,
        format_skills_for_system_prompt, load_prompt_templates, load_skills,
        load_sourced_prompt_templates, load_sourced_skills, parse_command_args, parse_frontmatter,
        substitute_args,
    };
    pub use crate::shell_output::{
        ShellCaptureOptions, ShellCaptureResult, sanitize_binary_output,
    };
    pub use crate::truncate::{
        TruncationLimit, TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
    };
    pub use crate::types::{
        AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentStream, AgentTool,
        AgentToolDefinitionError, AgentToolOutput, AgentToolResult, CompactionConfig,
        CompactionSettings, DiagnosticSeverity, PromptTemplate, ProviderRequestSnapshot, QueueMode,
        ResourceDiagnostic, Skill, SourceTag, SourcedPromptTemplate, SourcedResourceDiagnostic,
        SourcedSkill, ThinkingLevel, ToolExecutionMode, ToolFn, ToolUpdateCallback,
    };
}
