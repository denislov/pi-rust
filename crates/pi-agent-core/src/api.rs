/// Provider-neutral agent runtime configuration, messages, events, and
/// lifecycle. Product policy and provider construction do not belong here.
pub mod agent {
    pub use crate::agent::types::{
        AgentConfig, AgentEvent, AgentMessage, AgentResources, AgentStream, CompactionConfig,
        CompactionSettings, ProviderRequestSnapshot, ProviderStreamer, QueueMode, ThinkingLevel,
    };
    pub use crate::agent::{Agent, AgentAdmissionError};
    pub use crate::hooks::{
        AfterToolCallContext, AfterToolCallHook, AfterToolCallResult, AgentHooks,
        AgentLoopTurnUpdate, BeforeProviderRequestContext, BeforeProviderRequestResult,
        BeforeToolCallContext, BeforeToolCallHook, BeforeToolCallResult, ConvertToLlmHook,
        HookFuture, PrepareNextTurnContext, PrepareNextTurnHook, ShouldStopAfterTurnContext,
        ShouldStopAfterTurnHook, TransformContextHook,
    };
}

/// Tool definitions and provider-neutral tool execution results.
pub mod tool {
    pub use crate::agent::types::{
        AgentTool, AgentToolDefinitionError, AgentToolOutput, AgentToolResult,
        ToolExecutionContext, ToolExecutionMode, ToolFn, ToolUpdateCallback,
    };
}

/// Generic typed orchestration graph used by product-owned operations.
pub mod flow {
    pub use crate::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome, FlowRunOptions};
}

/// Capability-neutral filesystem and shell execution contracts plus output
/// shaping helpers used by coding tools.
pub mod execution {
    pub use crate::execution::capture::{
        ShellCaptureOptions, ShellCaptureResult, bash_execution_to_text,
        execute_shell_with_capture, sanitize_binary_output,
    };
    pub use crate::execution::truncate::{
        TruncationLimit, TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
    };
    pub use crate::execution::{
        ExecOptions, ExecutionEnv, ExecutionOutput, FileInfo, FileKind, FileSystem, Shell,
    };
    pub use crate::execution::{ExecutionError, ExecutionErrorCode, FileError, FileErrorCode};
}

/// Provider-neutral skills, prompt templates, diagnostics, and parsing.
pub mod resources {
    pub use crate::resources::{
        AgentResources, DiagnosticSeverity, PromptTemplate, ResourceDiagnostic, Skill, SourceTag,
        SourcedPromptTemplate, SourcedResourceDiagnostic, SourcedSkill,
    };
    pub use crate::resources::{
        format_prompt_template_invocation, format_skill_invocation,
        format_skills_for_system_prompt, load_prompt_templates, load_skills,
        load_sourced_prompt_templates, load_sourced_skills, parse_command_args, parse_frontmatter,
        substitute_args,
    };
}

/// Token estimation and summarization primitives. Durable compaction policy
/// remains owned by the product session layer.
pub mod compaction {
    pub use crate::compaction::branch::{
        BranchPreparation, BranchSummaryOptions, BranchSummaryResult, CollectEntriesResult,
        FileOperations, collect_entries_for_branch_summary, generate_branch_summary,
        generate_branch_summary_with_provider_streamer, prepare_branch_entries,
    };
    pub use crate::compaction::branch_error::{BranchSummaryError, BranchSummaryErrorCode};
    pub use crate::compaction::estimate::{
        ContextUsageEstimate, calculate_context_tokens, estimate_context_tokens, estimate_tokens,
    };
    pub use crate::compaction::prepare::{prepare_compaction, should_compact};
    pub use crate::compaction::summarize::{
        build_summarization_context, serialize_conversation, summarize,
        summarize_with_provider_streamer,
    };
}

/// Provider-neutral transcript records, tree projection, and identifiers.
pub mod transcript {
    pub use crate::transcript::{
        SessionEntry, SessionHeader, SessionIdGenerator, SessionMetadata, SessionTreeNode,
        StoredAgentMessage, StoredUsage, StoredUsageCost, TranscriptIdError, TreeFilterMode,
        agent_message_to_stored, create_session_id, create_timestamp, generate_entry_id,
    };
}

/// Deterministic helpers for owner and downstream tests. Production code
/// must not import this category.
#[cfg(any(test, feature = "test-support"))]
pub mod testing {
    pub use crate::agent::turn::{
        AgentTurnContext, AgentTurnFlow, AgentTurnProviderRequestOverride,
        ApplyBeforeProviderRequestHookNode, DecideAfterAssistantNode, DrainQueuedInputNode,
        ExecuteToolsNode, MaybeCompactRuntimeContextNode, MaybePrepareNextTurnNode,
        PendingToolCall, PrepareProviderRequestNode, ProviderStreamNode, RuntimeCompactionState,
        StartTurnNode, apply_before_provider_request_hook, decide_after_assistant,
        drain_queued_input, execute_tools, maybe_compact_runtime_context, maybe_prepare_next_turn,
        prepare_provider_request, start_turn, stream_provider,
    };
    pub use crate::context::conversion::{
        assemble_context, convert_to_context, default_convert_to_llm,
    };
    pub use crate::context::{
        InMemorySessionStorage, SessionContext, SessionError, SessionErrorCode,
        build_session_context,
    };
    pub use crate::execution::capture::bash_execution_to_text;
    pub use crate::flow::{FlowEvent, FlowEventCallback, NodeId};
    pub use crate::testing::environment::InMemoryExecutionEnv;
    pub use crate::testing::harness::{
        AbortResult, AgentHarness, AgentHarnessEvent, AgentHarnessHooks, AgentHarnessPhase,
        BeforeAgentStartHook, BeforeProviderPayload, BeforeProviderPayloadHook,
        BeforeProviderPayloadPatch, BeforeProviderRequest, BeforeProviderRequestHook,
        BeforeProviderRequestPatch, ContextHook, GetApiKeyAndHeadersHook, HarnessContext,
        HarnessHookFuture, HarnessHookKind, HeaderPatch, Observer, Patch, ProviderAuth,
        ProviderResponse, StreamOptionsPatch, SubscriptionGuard, on_kind,
    };
    pub use crate::testing::proxy::{
        ProxyAssistantMessageEvent, ProxyMessageState, ProxyRequest, ProxyStreamOptions,
        ProxyTransportFuture, build_proxy_request_body, process_proxy_event, stream_proxy,
        stream_proxy_with_transport,
    };
}
