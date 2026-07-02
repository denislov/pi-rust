#![allow(dead_code)]

use pi_agent_core::flow::FlowOutcome;

use super::CodingSessionError;
use super::branch_summary_flow::{BranchSummaryContext, BranchSummaryFlow, BranchSummaryOutcome};
use super::manual_compaction_flow::{
    ManualCompactionContext, ManualCompactionFlow, ManualCompactionOutcome,
};
use super::plugin_load_flow::{PluginLoadContext, PluginLoadFlow, PluginLoadOutcome};
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

    pub(crate) fn branch_summary_flow(&self) -> Result<BranchSummaryFlow, CodingSessionError> {
        BranchSummaryFlow::new()
    }

    pub(crate) fn plugin_load_flow(&self) -> Result<PluginLoadFlow, CodingSessionError> {
        PluginLoadFlow::new()
    }

    pub(crate) async fn run_plugin_load_graph(
        &self,
        ctx: &mut PluginLoadContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.plugin_load_flow()?.run(ctx).await
    }

    pub(crate) async fn run_plugin_load(
        &self,
        ctx: &mut PluginLoadContext,
    ) -> Result<PluginLoadOutcome, CodingSessionError> {
        match self.run_plugin_load_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_branch_summary_graph(
        &self,
        ctx: &mut BranchSummaryContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.branch_summary_flow()?.run(ctx).await
    }

    pub(crate) async fn run_branch_summary(
        &self,
        ctx: &mut BranchSummaryContext,
    ) -> Result<BranchSummaryOutcome, CodingSessionError> {
        match self.run_branch_summary_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
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

    use pi_agent_core::{AgentResources, AgentTool};
    use pi_ai::providers::faux::FauxProvider;
    use pi_ai::registry;
    use pi_ai::types::{Model, ModelCost, ModelInput};

    use super::*;
    use crate::coding_session::plugin_load_flow::{
        PluginLoadCandidate, PluginLoadContext, PluginLoadManifest, PluginLoadOptions,
        PluginLoadOutcome,
    };
    use crate::coding_session::prompt::{PromptTurnIds, PromptTurnOptions};
    use crate::coding_session::session_log::replay::SessionReplay;
    use crate::coding_session::session_log::store::{CreateSessionOptions, SessionLogStore};
    use crate::plugins::{
        CommandDefinition, CommandProvider, CommandRegistrationHost, PluginError, PluginRegistry,
        PluginSource, ToolProvider, ToolRegistrationHost,
    };
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
            leaves: Vec::new(),
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
    fn branch_summary_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.branch_summary_flow().unwrap();

        assert_eq!(
            crate::coding_session::branch_summary_flow::BranchSummaryFlow::node_ids(),
            &[
                "start_branch_summary",
                "load_branch_events",
                "select_abandoned_range",
                "prepare_summary_prompt",
                "run_summary_model",
                "record_branch_summary",
                "finalize_branch_summary",
            ]
        );
    }

    #[test]
    fn plugin_load_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.plugin_load_flow().unwrap();

        assert_eq!(
            crate::coding_session::plugin_load_flow::PluginLoadFlow::node_ids(),
            &[
                "start_plugin_load",
                "discover_plugins",
                "validate_manifests",
                "load_first_party_plugins",
                "load_lua_plugins_later",
                "register_capabilities",
                "emit_diagnostics",
                "finalize_plugin_load",
            ]
        );
    }

    #[tokio::test]
    async fn plugin_load_flow_registers_valid_plugins_and_keeps_invalid_diagnostics() {
        let service = FlowService::new();
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(PluginLoadToolProvider));
        registry.register_command_provider(Arc::new(PluginLoadCommandProvider));
        let options = PluginLoadOptions::new()
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new(
                    "first-party-fixture",
                    "First Party Fixture",
                    "1.0.0",
                    PluginSource::FirstParty,
                ),
                registry,
            ))
            .with_candidate(PluginLoadCandidate::new(
                PluginLoadManifest::new("", "Invalid Fixture", "1.0.0", PluginSource::Project),
                PluginRegistry::new(),
            ));
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["first-party-fixture"]);
        assert!(outcome.capability_changed);
        assert_eq!(outcome.capabilities.tool_providers, 1);
        assert_eq!(outcome.capabilities.command_providers, 1);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("plugin id must not be empty")
        );
        assert!(matches!(
            context.outcome(),
            Some(PluginLoadOutcome {
                loaded_plugin_ids,
                capability_changed: true,
                ..
            }) if loaded_plugin_ids == &["first-party-fixture".to_owned()]
        ));
        let loaded_service = context.loaded_plugin_service().unwrap();
        assert_eq!(loaded_service.collect_tools()[0].name, "plugin_echo");
        assert_eq!(loaded_service.collect_commands()[0].id, "plugin.command");
    }

    #[tokio::test]
    async fn plugin_load_flow_defers_lua_candidates_as_diagnostics() {
        let service = FlowService::new();
        let mut registry = PluginRegistry::new();
        registry.register_tool_provider(Arc::new(PluginLoadToolProvider));
        let options = PluginLoadOptions::new().with_candidate(PluginLoadCandidate::new(
            PluginLoadManifest::new("lua-fixture", "Lua Fixture", "1.0.0", PluginSource::Lua),
            registry,
        ));
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert!(outcome.loaded_plugin_ids.is_empty());
        assert_eq!(outcome.capabilities.tool_providers, 0);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("Lua plugin loading is not implemented yet")
        );
        assert!(
            context
                .loaded_plugin_service()
                .unwrap()
                .collect_tools()
                .is_empty()
        );
    }

    struct PluginLoadToolProvider;

    impl ToolProvider for PluginLoadToolProvider {
        fn metadata(&self) -> crate::plugins::PluginMetadata {
            crate::plugins::PluginMetadata::new(
                crate::plugins::PluginId::new("plugin-load-tool"),
                "Plugin Load Tool",
                "1.0.0",
                PluginSource::FirstParty,
            )
        }

        fn tools(&self, _host: &ToolRegistrationHost) -> Result<Vec<AgentTool>, PluginError> {
            Ok(vec![AgentTool::new_text(
                "plugin_echo",
                "echoes plugin input",
                serde_json::json!({"type": "object"}),
                |_args| async { Ok("plugin echo".to_owned()) },
            )])
        }
    }

    struct PluginLoadCommandProvider;

    impl CommandProvider for PluginLoadCommandProvider {
        fn metadata(&self) -> crate::plugins::PluginMetadata {
            crate::plugins::PluginMetadata::new(
                crate::plugins::PluginId::new("plugin-load-command"),
                "Plugin Load Command",
                "1.0.0",
                PluginSource::FirstParty,
            )
        }

        fn commands(
            &self,
            _host: &CommandRegistrationHost,
        ) -> Result<Vec<CommandDefinition>, PluginError> {
            Ok(vec![CommandDefinition::new(
                "plugin.command",
                "runs a plugin command",
            )])
        }
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
