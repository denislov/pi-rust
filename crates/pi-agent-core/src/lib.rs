#[doc(hidden)]
pub mod agent;
#[doc(hidden)]
pub mod agent_turn_flow;
mod ai_runtime;
mod branch_summary;
#[doc(hidden)]
pub mod compaction;
#[doc(hidden)]
pub mod convert;
#[doc(hidden)]
pub mod env;
#[doc(hidden)]
pub mod errors;
mod flow;
#[doc(hidden)]
pub mod harness;
#[doc(hidden)]
pub mod hooks;
mod loop_runtime;
mod proxy;
#[doc(hidden)]
pub mod queues;
mod resources;
mod session_context;
mod shell_output;
mod transcript;
mod truncate;
mod types;

/// Stable low-level runtime facade for `pi-agent-core`.
///
/// Product session ownership, adapter wire events, and workflow ownership belong
/// in `pi-coding-agent`. This module intentionally exposes low-level agent,
/// tool, hook, resource, and environment contracts.
pub mod api {
    pub use crate::agent::Agent;
    pub use crate::branch_summary::{
        BranchPreparation, BranchSummaryOptions, BranchSummaryResult, CollectEntriesResult,
        FileOperations, collect_entries_for_branch_summary, generate_branch_summary,
        generate_branch_summary_with_provider_streamer, prepare_branch_entries,
    };
    pub use crate::env::{
        ExecOptions, ExecutionEnv, ExecutionOutput, FileInfo, FileKind, FileSystem,
        InMemoryExecutionEnv, Shell,
    };
    pub use crate::errors::{
        BranchSummaryError, BranchSummaryErrorCode, ExecutionError, ExecutionErrorCode, FileError,
        FileErrorCode,
    };
    pub use crate::flow::{
        Action, Flow, FlowError, FlowEvent, FlowEventCallback, FlowNode, FlowOutcome,
        FlowRunOptions, NodeId,
    };
    pub use crate::harness::{
        AbortResult, AgentHarness, AgentHarnessEvent, AgentHarnessHooks, AgentHarnessPhase,
        BeforeAgentStartHook, BeforeProviderPayload, BeforeProviderPayloadHook,
        BeforeProviderPayloadPatch, BeforeProviderRequest, BeforeProviderRequestHook,
        BeforeProviderRequestPatch, ContextHook, GetApiKeyAndHeadersHook, HarnessContext,
        HarnessHookFuture, HarnessHookKind, HeaderPatch, Observer, Patch, ProviderAuth,
        ProviderResponse, StreamOptionsPatch, SubscriptionGuard, on_kind,
    };
    pub use crate::hooks::{
        AfterToolCallContext, AfterToolCallHook, AfterToolCallResult, AgentHooks,
        AgentLoopTurnUpdate, BeforeProviderRequestContext, BeforeProviderRequestResult,
        BeforeToolCallContext, BeforeToolCallHook, BeforeToolCallResult, ConvertToLlmHook,
        HookFuture, PrepareNextTurnContext, PrepareNextTurnHook, ShouldStopAfterTurnContext,
        ShouldStopAfterTurnHook, TransformContextHook,
    };
    pub use crate::proxy::{
        ProxyAssistantMessageEvent, ProxyMessageState, ProxyRequest, ProxyStreamOptions,
        ProxyTransportFuture, build_proxy_request_body, process_proxy_event, stream_proxy,
        stream_proxy_with_transport,
    };
    pub use crate::resources::{
        format_prompt_template_invocation, format_skill_invocation,
        format_skills_for_system_prompt, load_prompt_templates, load_skills,
        load_sourced_prompt_templates, load_sourced_skills, parse_command_args, parse_frontmatter,
        substitute_args,
    };
    pub use crate::session_context::{
        InMemorySessionStorage, SessionContext, SessionError, SessionErrorCode,
        build_session_context,
    };
    pub use crate::shell_output::{
        ShellCaptureOptions, ShellCaptureResult, execute_shell_with_capture, sanitize_binary_output,
    };
    pub use crate::transcript::{
        SessionEntry, SessionHeader, SessionIdGenerator, SessionMetadata, SessionTreeNode,
        StoredAgentMessage, StoredUsage, StoredUsageCost, TranscriptIdError, TreeFilterMode,
        agent_message_to_stored, create_session_id, create_timestamp, generate_entry_id,
    };
    pub use crate::truncate::{
        TruncationLimit, TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
    };
    pub use crate::types::{
        AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentStream, AgentTool,
        AgentToolDefinitionError, AgentToolOutput, AgentToolResult, CompactionConfig,
        CompactionSettings, DiagnosticSeverity, PromptTemplate, ProviderRequestSnapshot,
        ProviderStreamer, QueueMode, ResourceDiagnostic, Skill, SourceTag, SourcedPromptTemplate,
        SourcedResourceDiagnostic, SourcedSkill, ThinkingLevel, ToolExecutionMode, ToolFn,
        ToolUpdateCallback,
    };
}
