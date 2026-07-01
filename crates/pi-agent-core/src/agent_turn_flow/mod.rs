mod context;
mod nodes;
mod runtime;

pub use context::{AgentTurnContext, PendingToolCall, RuntimeCompactionState};
pub use nodes::{
    DecideStopOrToolsNode, ExecuteToolsNode, MaybeCompactRuntimeContextNode, PrepareContextNode,
    ProviderStreamNode, decide_stop_or_tools, execute_tools, maybe_compact_runtime_context,
    prepare_context, stream_provider,
};
pub use runtime::AgentTurnFlow;

#[cfg(test)]
mod tests {
    use super::AgentTurnFlow;

    #[test]
    fn agent_turn_flow_runtime_entrypoint_exists() {
        let _flow = AgentTurnFlow;
    }
}
