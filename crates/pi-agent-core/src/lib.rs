mod agent;
mod agent_turn_flow;
mod ai_runtime;
mod branch_summary;
mod compaction;
mod convert;
mod env;
mod errors;
mod flow;
mod harness;
mod hooks;
mod loop_runtime;
mod proxy;
mod queues;
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
    pub use crate::agent_turn_flow::{
        AgentTurnContext, AgentTurnFlow, AgentTurnProviderRequestOverride,
        ApplyBeforeProviderRequestHookNode, DecideAfterAssistantNode, DecideStopOrToolsNode,
        DrainQueuedInputNode, ExecuteToolsNode, MaybeCompactRuntimeContextNode,
        MaybePrepareNextTurnNode, PendingToolCall, PrepareContextNode, PrepareProviderRequestNode,
        ProviderStreamNode, RuntimeCompactionState, StartTurnNode,
        apply_before_provider_request_hook, decide_after_assistant, decide_stop_or_tools,
        drain_queued_input, execute_tools, maybe_compact_runtime_context, maybe_prepare_next_turn,
        prepare_context, prepare_provider_request, start_turn, stream_provider,
    };
    pub use crate::branch_summary::{
        BranchPreparation, BranchSummaryOptions, BranchSummaryResult, CollectEntriesResult,
        FileOperations, collect_entries_for_branch_summary, generate_branch_summary,
        generate_branch_summary_with_provider_streamer, prepare_branch_entries,
    };
    pub use crate::compaction::estimate::{
        ContextUsageEstimate, calculate_context_tokens, estimate_context_tokens, estimate_tokens,
    };
    pub use crate::compaction::prepare::{prepare_compaction, should_compact};
    pub use crate::compaction::summarize::{
        build_summarization_context, serialize_conversation, summarize,
        summarize_with_provider_streamer,
    };
    pub use crate::convert::{
        assemble_context, bash_execution_to_text, convert_to_context, default_convert_to_llm,
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
