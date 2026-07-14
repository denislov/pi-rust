mod context;
mod nodes;
mod runtime;

pub use context::{
    AgentTurnContext, AgentTurnProviderRequestOverride, PendingToolCall, RuntimeCompactionState,
};
pub use nodes::{
    ApplyBeforeProviderRequestHookNode, DecideAfterAssistantNode, DecideStopOrToolsNode,
    DrainQueuedInputNode, ExecuteToolsNode, MaybeCompactRuntimeContextNode,
    MaybePrepareNextTurnNode, PrepareContextNode, PrepareProviderRequestNode, ProviderStreamNode,
    StartTurnNode, apply_before_provider_request_hook, decide_after_assistant,
    decide_stop_or_tools, drain_queued_input, execute_tools, maybe_compact_runtime_context,
    maybe_prepare_next_turn, prepare_context, prepare_provider_request, start_turn,
    stream_provider,
};
pub use runtime::AgentTurnFlow;

#[cfg(test)]
mod tests {
    use super::AgentTurnFlow;

    #[test]
    fn agent_turn_flow_runtime_entrypoint_exists() {
        let _flow = AgentTurnFlow::new().expect("agent turn flow should build");
        assert_eq!(AgentTurnFlow::node_ids()[0], "start_turn");
    }
}
