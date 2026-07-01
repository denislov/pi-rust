#![allow(dead_code)]

use pi_agent_core::flow::FlowOutcome;

use super::CodingSessionError;
use super::manual_compaction_flow::{
    ManualCompactionContext, ManualCompactionFlow, ManualCompactionOutcome,
};
use super::prompt::{PromptTurnContext, PromptTurnOutcome};
use super::prompt_flow::PromptTurnFlow;

#[derive(Debug, Default)]
pub(crate) struct FlowService;

impl FlowService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn prompt_turn_flow(&self) -> Result<PromptTurnFlow, CodingSessionError> {
        PromptTurnFlow::new()
    }

    pub(crate) fn manual_compaction_flow(
        &self,
    ) -> Result<ManualCompactionFlow, CodingSessionError> {
        ManualCompactionFlow::new()
    }

    pub(crate) async fn run_manual_compaction_graph(
        &self,
        ctx: &mut ManualCompactionContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.manual_compaction_flow()?.run(ctx).await
    }

    pub(crate) async fn run_manual_compaction(
        &self,
        ctx: &mut ManualCompactionContext,
    ) -> Result<ManualCompactionOutcome, CodingSessionError> {
        match self.run_manual_compaction_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_prompt_turn_graph(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.prompt_turn_flow()?.run(ctx).await
    }

    pub(crate) async fn run_prompt_turn(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        match self.run_prompt_turn_graph(ctx).await {
            Ok(_) => {
                let session_id = ctx.session_id().map(str::to_owned);
                ctx.finish_success(session_id, None)
            }
            Err(error) => Ok(ctx.finish_failure(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pi_agent_core::AgentResources;
    use pi_ai::providers::faux::FauxProvider;
    use pi_ai::registry;
    use pi_ai::types::{Model, ModelCost, ModelInput};

    use super::*;
    use crate::coding_session::prompt::{PromptTurnIds, PromptTurnOptions};
    use crate::coding_session::session_log::replay::SessionReplay;
    use crate::coding_session::session_log::store::{CreateSessionOptions, SessionLogStore};
    use crate::prompt_options::PromptRunOptions;
    use crate::runtime::PromptInvocation;

    fn model(api: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    #[tokio::test]
    async fn flow_service_builds_and_runs_prompt_turn_graph() {
        let api = "flow-service-prompt-turn";
        registry::register(api, Arc::new(FauxProvider::simple_text("done")));
        let service = FlowService::new();
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
                prompt: "hello".into(),
                model: model(api),
                api_key: None,
                system_prompt: None,
                max_turns: Some(2),
                tools: Vec::new(),
                register_builtins: false,
                session: None,
                session_target: None,
                session_name: None,
                thinking_level: None,
                tool_execution: None,
                resources: AgentResources::default(),
                settings: None,
                invocation: PromptInvocation::Text("hello".into()),
            }),
        );
        let temp = tempfile::tempdir().unwrap();
        let store = SessionLogStore::new(temp.path());
        let handle = store
            .create_session(CreateSessionOptions::new(
                "sess_flow_service",
                "2026-06-29T00:00:00Z",
            ))
            .unwrap();
        context.set_replay(SessionReplay {
            session_id: "sess_flow_service".into(),
            cwd: None,
            active_leaf_id: None,
            transcript: Vec::new(),
            diagnostics: Vec::new(),
        });
        context.begin_transaction(&store, handle).unwrap();

        let outcome = service.run_prompt_turn_graph(&mut context).await.unwrap();

        assert_eq!(outcome.last_node.as_str(), "emit_completion");
        assert!(context.final_message().is_some());
        registry::unregister(api);
    }

    #[test]
    fn manual_compaction_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.manual_compaction_flow().unwrap();

        assert_eq!(
            crate::coding_session::manual_compaction_flow::ManualCompactionFlow::node_ids(),
            &[
                "start_compaction",
                "load_session_replay",
                "select_compaction_range",
                "prepare_summary_context",
                "run_summary_model",
                "record_compaction_events",
                "finalize_compaction",
                "emit_completion",
            ]
        );
    }
}
