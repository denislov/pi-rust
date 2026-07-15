use pi_agent_core::api::{Flow, FlowOutcome, FlowRunOptions};

use super::CodingSessionError;
use super::agent_invocation_flow::{
    AgentInvocationContext, AgentInvocationFlow, AgentInvocationOutcome,
};
use super::agent_team_flow::{AgentTeamContext, AgentTeamFlow, AgentTeamOutcome};
use super::branch_summary_flow::{BranchSummaryContext, BranchSummaryFlow, BranchSummaryOutcome};
use super::export_flow::{ExportContext, ExportFlow, ExportOutcome};
use super::manual_compaction_flow::{
    ManualCompactionContext, ManualCompactionFlow, ManualCompactionOutcome,
};
use super::plugin_load_flow::{PluginLoadContext, PluginLoadFlow, PluginLoadOutcome};
use super::prompt::{PromptTurnContext, PromptTurnOutcome};
use super::prompt_flow::PromptTurnFlow;
use super::self_healing_edit_flow::{
    SelfHealingEditContext, SelfHealingEditFlow, SelfHealingEditOutcome,
};

pub(crate) fn add_linear_edges<C>(
    flow: &mut Flow<C>,
    node_ids: &[&str],
) -> Result<(), CodingSessionError> {
    for pair in node_ids.windows(2) {
        flow.edge(pair[0], pair[1])
            .map_err(|error| CodingSessionError::Flow {
                message: format!("flow graph configuration failed: {error}"),
            })?;
    }
    Ok(())
}

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

    pub(crate) fn agent_invocation_flow(&self) -> Result<AgentInvocationFlow, CodingSessionError> {
        AgentInvocationFlow::new()
    }

    pub(crate) fn agent_team_flow(&self) -> Result<AgentTeamFlow, CodingSessionError> {
        AgentTeamFlow::new()
    }

    pub(crate) fn plugin_load_flow(&self) -> Result<PluginLoadFlow, CodingSessionError> {
        PluginLoadFlow::new()
    }

    pub(crate) fn export_flow(&self) -> Result<ExportFlow, CodingSessionError> {
        ExportFlow::new()
    }

    pub(crate) fn self_healing_edit_flow(&self) -> Result<SelfHealingEditFlow, CodingSessionError> {
        SelfHealingEditFlow::new()
    }

    pub(crate) fn run_export_graph(
        &self,
        ctx: &mut ExportContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.export_flow()?.run(ctx)
    }

    pub(crate) fn run_export(
        &self,
        ctx: &mut ExportContext,
    ) -> Result<ExportOutcome, CodingSessionError> {
        match self.run_export_graph(ctx) {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_self_healing_edit_graph(
        &self,
        ctx: &mut SelfHealingEditContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.self_healing_edit_flow()?.run(ctx).await
    }

    pub(crate) async fn run_self_healing_edit(
        &self,
        ctx: &mut SelfHealingEditContext,
    ) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        match self.run_self_healing_edit_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_agent_invocation_graph(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.agent_invocation_flow()?.run(ctx).await
    }

    pub(crate) async fn run_agent_invocation(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        match self.run_agent_invocation_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_agent_team_graph(
        &self,
        ctx: &mut AgentTeamContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.agent_team_flow()?.run(ctx).await
    }

    pub(crate) async fn run_agent_team(
        &self,
        ctx: &mut AgentTeamContext,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        match self.run_agent_team_graph(ctx).await {
            Ok(_) => ctx.finish_success(),
            Err(error) => Err(ctx.take_failure_error().unwrap_or(error)),
        }
    }

    pub(crate) async fn run_prompt_subflow_for_agent_invocation(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.run_prompt_turn_graph(ctx).await
    }

    pub(crate) async fn run_prompt_subflow_for_agent_team_member(
        &self,
        ctx: &mut PromptTurnContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.run_prompt_turn_graph(ctx).await
    }

    pub(crate) async fn run_agent_invocation_subflow(
        &self,
        ctx: &mut AgentInvocationContext,
    ) -> Result<AgentInvocationOutcome, CodingSessionError> {
        self.run_agent_invocation(ctx).await
    }

    pub(crate) async fn run_agent_team_subflow(
        &self,
        ctx: &mut AgentTeamContext,
    ) -> Result<AgentTeamOutcome, CodingSessionError> {
        self.run_agent_team(ctx).await
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
        let cancel = ctx.options().cancellation();
        self.manual_compaction_flow()?
            .run_with_options(
                ctx,
                FlowRunOptions {
                    cancel,
                    ..Default::default()
                },
            )
            .await
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
            Err(error) => match ctx.abort_reason() {
                Some(reason) => {
                    Ok(ctx.finish_abort(reason.to_owned(), ctx.session_id().map(str::to_owned)))
                }
                None => Ok(ctx.finish_failure(error)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        sync::{Arc, Mutex},
    };

    use futures::future::{BoxFuture, FutureExt};
    use pi_agent_core::api::{
        AgentResources, AgentTool, ExecutionOutput, FileSystem, InMemoryExecutionEnv,
    };
    use pi_ai::api::testing::FauxProvider;
    use pi_ai::api::{ContentBlock, Model, ModelCost, ModelInput};

    use super::*;
    use crate::coding_session::capability_snapshot::OperationCapabilitySnapshot;
    use crate::coding_session::export_flow::{ExportContext, ExportOptions};
    use crate::coding_session::plugin_load_flow::{
        PluginLoadCandidate, PluginLoadContext, PluginLoadManifest, PluginLoadOptions,
        PluginLoadOutcome,
    };
    use crate::coding_session::prompt::{PromptTurnIds, PromptTurnOptions};
    use crate::coding_session::self_healing_edit_flow::{
        SelfHealingEditCheckOutput, SelfHealingEditCheckRunner, SelfHealingEditContext,
        SelfHealingEditObserver, SelfHealingEditOptions, SelfHealingEditRepairAttempt,
        SelfHealingEditRepairStrategy, SelfHealingEditReplacement,
    };
    use crate::coding_session::session_log::replay::{SessionReplay, TranscriptItem};
    use crate::coding_session::session_log::store::{CreateSessionOptions, SessionLogStore};
    use crate::coding_session::{CodingAgentSessionExportItem, CodingAgentSessionSummary};
    use crate::plugins::{
        CommandDefinition, CommandProvider, CommandRegistrationHost, PluginError, PluginRegistry,
        PluginSource, PromptHookContext, PromptHookPoint, ToolProvider, ToolRegistrationHost,
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

    struct NoopFlowNode;

    impl pi_agent_core::api::FlowNode<()> for NoopFlowNode {
        fn name(&self) -> &str {
            "NoopFlowNode"
        }

        fn run<'a>(
            &'a self,
            _ctx: &'a mut (),
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<pi_agent_core::api::Action, String>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Ok(pi_agent_core::api::Action::default()) })
        }
    }

    #[test]
    fn add_linear_edges_maps_configuration_errors_to_coding_session_error() {
        let mut flow = pi_agent_core::api::Flow::new("start").unwrap();
        flow.add_node("start", NoopFlowNode).unwrap();

        let error = add_linear_edges(&mut flow, &["start", "missing"]).unwrap_err();

        assert!(matches!(error, CodingSessionError::Flow { .. }));
        assert!(
            error
                .to_string()
                .contains("flow graph configuration failed: unknown flow node: missing"),
            "{error}"
        );
    }

    #[derive(Debug)]
    struct SequenceCheckRunner {
        outputs: Mutex<VecDeque<SelfHealingEditCheckOutput>>,
    }

    impl SequenceCheckRunner {
        fn new(outputs: Vec<SelfHealingEditCheckOutput>) -> Self {
            Self {
                outputs: Mutex::new(outputs.into()),
            }
        }
    }

    impl SelfHealingEditCheckRunner for SequenceCheckRunner {
        fn run_check<'a>(
            &'a self,
            _cwd: &'a std::path::Path,
            _command: &'a str,
        ) -> BoxFuture<'a, Result<SelfHealingEditCheckOutput, String>> {
            async move {
                self.outputs
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| "no check output queued".to_owned())
            }
            .boxed()
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ObservedRepairAttempt {
        path: String,
        attempt: usize,
        old_text: String,
        new_text: String,
        exit_code: Option<i32>,
    }

    #[derive(Debug)]
    struct BlockingRepairObserver {
        sender: tokio::sync::mpsc::UnboundedSender<ObservedRepairAttempt>,
        release: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    }

    impl BlockingRepairObserver {
        fn new(
            sender: tokio::sync::mpsc::UnboundedSender<ObservedRepairAttempt>,
            release: tokio::sync::oneshot::Receiver<()>,
        ) -> Self {
            Self {
                sender,
                release: Mutex::new(Some(release)),
            }
        }
    }

    impl SelfHealingEditObserver for BlockingRepairObserver {
        fn repair_attempted<'a>(
            &'a self,
            path: &'a str,
            repair: &'a SelfHealingEditRepairAttempt,
        ) -> BoxFuture<'a, ()> {
            let replacement = repair
                .replacements
                .first()
                .expect("repair observer test uses one replacement");
            let observed = ObservedRepairAttempt {
                path: path.to_owned(),
                attempt: repair.attempt,
                old_text: replacement.old_text.clone(),
                new_text: replacement.new_text.clone(),
                exit_code: repair.check_output.as_ref().map(|output| output.exit_code),
            };
            let release = self.release.lock().unwrap().take();
            async move {
                self.sender
                    .send(observed)
                    .expect("repair observer receiver should be alive");
                if let Some(release) = release {
                    let _ = release.await;
                }
            }
            .boxed()
        }
    }

    #[derive(Debug)]
    struct FixedRepairStrategy {
        replacements: Vec<SelfHealingEditReplacement>,
    }

    impl FixedRepairStrategy {
        fn new(replacements: Vec<SelfHealingEditReplacement>) -> Self {
            Self { replacements }
        }
    }

    impl SelfHealingEditRepairStrategy for FixedRepairStrategy {
        fn repair<'a>(
            &'a self,
            _attempt: usize,
            _path: &'a str,
            _replacements: &'a [SelfHealingEditReplacement],
            _diagnostics: &'a [crate::coding_session::self_healing_edit_flow::SelfHealingEditDiagnostic],
        ) -> BoxFuture<'a, Result<Vec<SelfHealingEditReplacement>, String>> {
            async move { Ok(self.replacements.clone()) }.boxed()
        }
    }

    fn check_output(
        command: &str,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
    ) -> SelfHealingEditCheckOutput {
        SelfHealingEditCheckOutput {
            command: command.to_owned(),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
            exit_code,
        }
    }

    #[tokio::test]
    async fn flow_service_builds_and_runs_prompt_turn_graph() {
        let api = "flow-service-prompt-turn";
        let _provider_guard = crate::test_support::ProviderGuard::register(
            api,
            Arc::new(FauxProvider::simple_text("done")),
        );
        let service = FlowService::new();
        let mut context = PromptTurnContext::new(
            PromptTurnIds::new("op_1", "turn_1"),
            PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
                prompt: "hello".into(),
                model: model(api),
                api_key: None,
                auth_diagnostics: Vec::new(),
                system_prompt: None,
                max_turns: Some(2),
                tools: Vec::new(),
                register_builtins: false,
                ai_client: Some(_provider_guard.ai_client()),
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
            pending_delegation_confirmations: Vec::new(),
            usage: Default::default(),
            operation_statuses: Default::default(),
        });
        context.begin_transaction(&store, handle).unwrap();
        context.set_capability_snapshot(OperationCapabilitySnapshot::permissive("op_1"));

        let outcome = service.run_prompt_turn_graph(&mut context).await.unwrap();

        assert_eq!(outcome.last_node.as_str(), "emit_completion");
        assert!(context.final_message().is_some());
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
    fn agent_team_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.agent_team_flow().unwrap();

        assert_eq!(
            crate::coding_session::agent_team_flow::AgentTeamFlow::node_ids(),
            &[
                "start_team",
                "plan_subtasks",
                "run_member_agent",
                "collect_member_result",
                "merge_or_reject_result",
                "finalize_team",
            ]
        );
    }

    #[test]
    fn agent_invocation_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.agent_invocation_flow().unwrap();

        assert_eq!(
            crate::coding_session::agent_invocation_flow::AgentInvocationFlow::node_ids(),
            &[
                "start_agent_invocation",
                "resolve_agent_profile",
                "prepare_child_prompt",
                "run_child_agent",
                "finalize_agent_invocation",
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

    #[test]
    fn self_healing_edit_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.self_healing_edit_flow().unwrap();

        assert_eq!(
            crate::coding_session::self_healing_edit_flow::SelfHealingEditFlow::node_ids(),
            &[
                "start_edit_workflow",
                "read_target",
                "propose_patch",
                "validate_patch",
                "apply_patch",
                "run_check",
                "repair_patch",
                "record_result",
            ]
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_applies_successful_edit() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .with_execution_env(env.clone());
        let mut context = SelfHealingEditContext::new(options);

        let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

        assert_eq!(outcome.path, "src/app.txt");
        assert_eq!(outcome.attempts, 1);
        assert!(outcome.message.contains("Successfully replaced 1 block"));
        assert!(outcome.diff.contains("-2 two"), "{}", outcome.diff);
        assert!(outcome.diff.contains("+2 deux"), "{}", outcome.diff);
        assert!(
            outcome.patch.contains("--- src/app.txt"),
            "{}",
            outcome.patch
        );
        assert!(outcome.patch.contains("+deux"), "{}", outcome.patch);
        assert_eq!(outcome.first_changed_line, Some(2));
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "one\ndeux\n"
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_runs_successful_check_command() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        env.set_command(
            "cargo test --quiet",
            ExecutionOutput {
                stdout: "tests passed".to_owned(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .with_execution_env(env.clone())
        .with_check_command("cargo test --quiet");
        let mut context = SelfHealingEditContext::new(options);

        let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

        let check_output = outcome
            .check_output
            .as_ref()
            .expect("check output should be recorded");
        assert_eq!(check_output.command, "cargo test --quiet");
        assert_eq!(check_output.exit_code, 0);
        assert_eq!(check_output.stdout, "tests passed");
        assert!(check_output.stderr.is_empty());
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "one\ndeux\n"
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_fails_when_check_command_fails_without_repair() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        env.set_command(
            "cargo check",
            ExecutionOutput {
                stdout: String::new(),
                stderr: "compile error".to_owned(),
                exit_code: 1,
            },
        );
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .with_execution_env(env.clone())
        .with_check_command("cargo check");
        let mut context = SelfHealingEditContext::new(options);

        let error = service
            .run_self_healing_edit(&mut context)
            .await
            .expect_err("failed check should fail without repair");

        assert!(
            error.to_string().contains("self-healing edit check failed"),
            "{error}"
        );
        assert!(
            context
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("compile error")),
            "{:#?}",
            context.diagnostics()
        );
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "one\ndeux\n"
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_repairs_after_failed_check() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        let check_runner = Arc::new(SequenceCheckRunner::new(vec![
            check_output("cargo check", 1, "", "compile error"),
            check_output("cargo check", 0, "fixed", ""),
        ]));
        let repair_strategy = Arc::new(FixedRepairStrategy::new(vec![
            SelfHealingEditReplacement::new("deux", "dos"),
        ]));
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .with_execution_env(env.clone())
        .with_check_command("cargo check")
        .with_check_runner(check_runner)
        .with_repair_strategy(repair_strategy)
        .with_max_repair_attempts(1);
        let mut context = SelfHealingEditContext::new(options);

        let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

        assert_eq!(outcome.attempts, 2);
        assert_eq!(outcome.repair_attempts.len(), 1);
        let repair = &outcome.repair_attempts[0];
        assert_eq!(repair.attempt, 1);
        assert_eq!(repair.replacements.len(), 1);
        assert_eq!(repair.replacements[0].old_text, "deux");
        assert_eq!(repair.replacements[0].new_text, "dos");
        assert_eq!(repair.check_output.as_ref().unwrap().exit_code, 0);
        assert!(
            repair
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("compile error")),
            "{:#?}",
            repair.diagnostics
        );
        assert_eq!(outcome.check_output.as_ref().unwrap().exit_code, 0);
        assert_eq!(outcome.check_output.as_ref().unwrap().stdout, "fixed");
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("compile error")),
            "{:#?}",
            outcome.diagnostics
        );
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "one\ndos\n"
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_notifies_repair_observer_before_completion() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        let check_runner = Arc::new(SequenceCheckRunner::new(vec![
            check_output("cargo check", 1, "", "compile error"),
            check_output("cargo check", 0, "fixed", ""),
        ]));
        let repair_strategy = Arc::new(FixedRepairStrategy::new(vec![
            SelfHealingEditReplacement::new("deux", "dos"),
        ]));
        let (observed_tx, mut observed_rx) = tokio::sync::mpsc::unbounded_channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let observer = Arc::new(BlockingRepairObserver::new(observed_tx, release_rx));
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("two", "deux")],
        )
        .with_execution_env(env.clone())
        .with_check_command("cargo check")
        .with_check_runner(check_runner)
        .with_repair_strategy(repair_strategy)
        .with_max_repair_attempts(1)
        .with_repair_observer(observer);
        let mut context = SelfHealingEditContext::new(options);
        let run = service.run_self_healing_edit(&mut context);
        tokio::pin!(run);

        let observed = tokio::select! {
            observed = observed_rx.recv() => observed.expect("repair observer should emit before completion"),
            result = &mut run => panic!("flow completed before live repair observer fired: {result:?}"),
        };

        assert_eq!(
            observed,
            ObservedRepairAttempt {
                path: "src/app.txt".to_owned(),
                attempt: 1,
                old_text: "deux".to_owned(),
                new_text: "dos".to_owned(),
                exit_code: Some(0),
            }
        );
        release_tx
            .send(())
            .expect("flow should still be waiting for observer release");
        let outcome = run.await.unwrap();
        assert_eq!(outcome.attempts, 2);
        assert_eq!(outcome.repair_attempts.len(), 1);
    }

    #[tokio::test]
    async fn self_healing_edit_flow_reports_validation_failure_without_write() {
        let service = FlowService::new();
        let env = InMemoryExecutionEnv::new("/workspace");
        env.write_file("src/app.txt", b"one\ntwo\n").await.unwrap();
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("", "deux")],
        )
        .with_execution_env(env.clone());
        let mut context = SelfHealingEditContext::new(options);

        let error = service
            .run_self_healing_edit(&mut context)
            .await
            .expect_err("empty old text should fail validation");

        assert!(
            error.to_string().contains("oldText must not be empty"),
            "{error}"
        );
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "one\ntwo\n"
        );
        assert_eq!(context.diagnostics().len(), 1);
        assert!(
            context.diagnostics()[0]
                .message
                .contains("oldText must not be empty")
        );
    }

    #[tokio::test]
    async fn self_healing_edit_flow_uses_execution_env_operations() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let local_path = temp.path().join("src/app.txt");
        std::fs::create_dir_all(local_path.parent().unwrap()).unwrap();
        std::fs::write(&local_path, "local should not change\n").unwrap();

        let env = InMemoryExecutionEnv::new(temp.path());
        env.write_file("src/app.txt", b"env one\nenv two\n")
            .await
            .unwrap();
        let options = SelfHealingEditOptions::new(
            env.cwd(),
            "src/app.txt",
            vec![SelfHealingEditReplacement::new("env two", "env deux")],
        )
        .with_execution_env(env.clone());
        let mut context = SelfHealingEditContext::new(options);

        let outcome = service.run_self_healing_edit(&mut context).await.unwrap();

        assert_eq!(outcome.first_changed_line, Some(2));
        assert_eq!(
            env.read_text_file("src/app.txt").await.unwrap(),
            "env one\nenv deux\n"
        );
        assert_eq!(
            std::fs::read_to_string(&local_path).unwrap(),
            "local should not change\n"
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
                .contains("Lua plugin entry is required")
        );
        assert!(
            context
                .loaded_plugin_service()
                .unwrap()
                .collect_tools()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_tool_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-hello");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-hello"
name = "Lua Hello"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:tool({
    name = "lua_hello",
    description = "greets from lua",
    input_schema = {
      type = "object",
      properties = {
        name = { type = "string" }
      }
    },
    run = function(input)
      return { content = "hello " .. input.name }
    end
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-hello"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.tool_providers, 1);
        let tools = context.loaded_plugin_service().unwrap().collect_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "lua_hello");
        assert_eq!(tools[0].description, "greets from lua");
        assert_eq!(
            tools[0].parameters,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            })
        );

        let output = (tools[0].execute)(serde_json::json!({"name": "pi"}), None)
            .await
            .unwrap();
        assert_eq!(
            output.content,
            vec![ContentBlock::Text {
                text: "hello pi".to_owned(),
                text_signature: None,
            }]
        );
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_command_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-command");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-command"
name = "Lua Command"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:command({
    id = "lua.say_hello",
    description = "greets from lua command",
    run = function(input)
      return { content = "hello " .. input.name }
    end
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-command"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.command_providers, 1);
        let loaded_service = context.loaded_plugin_service().unwrap();
        let commands = loaded_service.collect_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "lua.say_hello");
        assert_eq!(commands[0].description, "greets from lua command");
        let output = loaded_service
            .run_command("lua.say_hello", serde_json::json!({"name": "pi"}))
            .unwrap();
        assert_eq!(output, "hello pi");
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_hook_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-hook");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-hook"
name = "Lua Hook"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:hook({
    point = "before_agent_turn",
    policy = "fail_open",
    run = function(ctx)
      return { diagnostics = { "lua hook saw " .. ctx.point .. " for " .. ctx.operation_id } }
    end
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-hook"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.hook_providers, 1);
        let loaded_service = context.loaded_plugin_service().unwrap();
        let diagnostics = loaded_service
            .run_prompt_hook(
                PromptHookPoint::BeforeAgentTurn,
                PromptHookContext {
                    operation_id: "op_1".to_owned(),
                    turn_id: "turn_1".to_owned(),
                    session_id: Some("session_1".to_owned()),
                    point: PromptHookPoint::BeforeAgentTurn,
                },
            )
            .unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "lua hook saw before_agent_turn for op_1"
        );
        assert_eq!(diagnostics[0].code.as_deref(), Some("plugin_hook"));
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_ui_action_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-ui");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-ui"
name = "Lua UI"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "lua.open_panel"
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-ui"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.ui_providers, 1);
        let actions = context
            .loaded_plugin_service()
            .unwrap()
            .collect_ui_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "ui.open_panel");
        assert_eq!(actions[0].label, "Open panel");
        assert_eq!(actions[0].description, "opens a Lua panel");
        assert_eq!(actions[0].action_id, "lua.open_panel");
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_dialog_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-dialog");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-dialog"
name = "Lua Dialog"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        default = "pi",
        required = true
      },
      {
        id = "confirmed",
        label = "Confirmed",
        description = "Confirm submission",
        type = "boolean",
        default = true
      }
    }
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-dialog"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.ui_providers, 1);
        let dialogs = context
            .loaded_plugin_service()
            .unwrap()
            .collect_ui_dialogs();
        assert_eq!(dialogs.len(), 1);
        assert_eq!(dialogs[0].id, "dialog.open_panel");
        assert_eq!(dialogs[0].title, "Lua panel");
        assert_eq!(dialogs[0].description, "Panel registered by Lua");
        assert_eq!(dialogs[0].action_id, "lua.submit_panel");
        assert_eq!(dialogs[0].fields.len(), 2);
        assert_eq!(dialogs[0].fields[0].id, "name");
        assert_eq!(dialogs[0].fields[0].label, "Name");
        assert_eq!(dialogs[0].fields[0].description, "Target name");
        assert_eq!(dialogs[0].fields[0].kind, "text");
        assert_eq!(dialogs[0].fields[0].default_value, serde_json::json!("pi"));
        assert!(dialogs[0].fields[0].required);
        assert_eq!(dialogs[0].fields[1].id, "confirmed");
        assert_eq!(dialogs[0].fields[1].kind, "boolean");
        assert_eq!(dialogs[0].fields[1].default_value, serde_json::json!(true));
        assert!(!dialogs[0].fields[1].required);
    }

    #[tokio::test]
    async fn plugin_load_flow_loads_lua_manifest_keybind_provider() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-keybind");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-keybind"
name = "Lua Keybind"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+shift+p",
    description = "opens the Lua panel",
    action_id = "lua.open_panel"
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(outcome.loaded_plugin_ids, vec!["lua-keybind"]);
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.keybind_providers, 1);
        let keybindings = context
            .loaded_plugin_service()
            .unwrap()
            .collect_keybindings();
        assert_eq!(keybindings.len(), 1);
        assert_eq!(keybindings[0].id, "keybind.open_panel");
        assert_eq!(keybindings[0].key, "ctrl+shift+p");
        assert_eq!(keybindings[0].description, "opens the Lua panel");
        assert_eq!(keybindings[0].action_id, "lua.open_panel");
    }

    #[tokio::test]
    async fn plugin_load_flow_lua_host_capabilities_metadata_is_feature_scoped() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp
            .path()
            .join("project/.pi-rust/plugins/lua-capabilities");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-capabilities"
name = "Lua Capabilities"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
local function assert_feature_scoped(host, capabilities)
  local expected = {
    "api_version",
    "plugin",
    "workspace",
    "capabilities",
    "tool",
    "command",
    "hook",
    "ui_action",
    "dialog",
    "keybind"
  }
  for _, name in ipairs(expected) do
    if capabilities[name] ~= true then
      error("missing Lua host capability " .. name)
    end
  end
  local forbidden = {
    "session",
    "sessionService",
    "sessionStore",
    "sessionLog",
    "eventLog",
    "runtime",
    "runtimeService",
    "provider",
    "providerKey",
    "apiKey",
    "auth",
    "model",
    "filesystem",
    "fs",
    "shell",
    "bash",
    "operation",
    "operationContext",
    "operation_context",
    "flow",
    "flowGraph",
    "graph"
  }
  local function assert_no_privileged_internals(value, label)
    for _, name in ipairs(forbidden) do
      if value[name] ~= nil then
        error(label .. " exposed privileged internal " .. name)
      end
    end
  end
  assert_no_privileged_internals(host, "Lua host")
  assert_no_privileged_internals(capabilities, "Lua host capabilities")
  assert_no_privileged_internals(host:plugin(), "Lua plugin metadata")
  assert_no_privileged_internals(host:workspace(), "Lua workspace metadata")
end

function register(host)
  local capabilities = host:capabilities()
  assert_feature_scoped(host, capabilities)
  host:command({
    id = "lua.capabilities_info",
    description = "capabilities " .. tostring(capabilities.tool) .. " " .. tostring(capabilities.command),
    run = function(input)
      local live_capabilities = host:capabilities()
      assert_feature_scoped(host, live_capabilities)
      return { content = "capabilities " .. tostring(live_capabilities.tool) .. " " .. tostring(live_capabilities.command) }
    end
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(
            outcome.loaded_plugin_ids,
            vec!["lua-capabilities"],
            "{:#?}",
            outcome.diagnostics
        );
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.command_providers, 1);
        let loaded_service = context.loaded_plugin_service().unwrap();
        let commands = loaded_service.collect_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "lua.capabilities_info");
        assert_eq!(commands[0].description, "capabilities true true");
        assert_eq!(
            loaded_service
                .run_command("lua.capabilities_info", serde_json::json!({}))
                .unwrap(),
            "capabilities true true"
        );
    }

    #[tokio::test]
    async fn plugin_load_flow_lua_host_workspace_metadata_is_read_only_and_path_scoped() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp.path().join("project/.pi-rust/plugins/lua-workspace");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "lua-workspace"
name = "Lua Workspace"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.lua"),
            r#"
function register(host)
  local workspace = host:workspace()
  if workspace.session ~= nil or workspace.runtime ~= nil or workspace.provider ~= nil or workspace.sessionService ~= nil then
    error("workspace metadata exposed privileged internals")
  end
  host:command({
    id = "lua.workspace_info",
    description = "workspace " .. workspace.pluginRoot .. " entry " .. workspace.entryPath,
    run = function(input)
      local live_workspace = host:workspace()
      if live_workspace.session ~= nil or live_workspace.runtime ~= nil or live_workspace.provider ~= nil or live_workspace.sessionService ~= nil then
        error("workspace metadata exposed privileged internals at execution")
      end
      return { content = "workspace " .. live_workspace.pluginRoot .. " entry " .. live_workspace.entryPath }
    end
  })
end
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new().with_discovery_root(
            temp.path().join("project/.pi-rust/plugins"),
            PluginSource::Project,
        );
        let mut context = PluginLoadContext::new(options);
        let plugin_root = plugin_dir.display().to_string();
        let entry_path = plugin_dir.join("plugin.lua").display().to_string();
        let expected = format!("workspace {plugin_root} entry {entry_path}");

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert_eq!(
            outcome.loaded_plugin_ids,
            vec!["lua-workspace"],
            "{:#?}",
            outcome.diagnostics
        );
        assert!(outcome.diagnostics.is_empty(), "{:#?}", outcome.diagnostics);
        assert_eq!(outcome.capabilities.command_providers, 1);
        let loaded_service = context.loaded_plugin_service().unwrap();
        let commands = loaded_service.collect_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "lua.workspace_info");
        assert_eq!(commands[0].description, expected);
        assert_eq!(
            loaded_service
                .run_command("lua.workspace_info", serde_json::json!({}))
                .unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn plugin_load_flow_discovers_project_and_user_manifest_files() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let project_plugins = temp.path().join("project/.pi-rust/plugins");
        let user_plugins = temp.path().join("user/plugins");
        fs::create_dir_all(project_plugins.join("project-lua")).unwrap();
        fs::create_dir_all(user_plugins.join("bad-plugin")).unwrap();
        fs::write(
            project_plugins.join("project-lua/plugin.toml"),
            r#"
id = "project-lua"
name = "Project Lua"
version = "0.1.0"
runtime = "lua"
"#,
        )
        .unwrap();
        fs::write(
            user_plugins.join("bad-plugin/plugin.toml"),
            r#"
id = ""
name = "Broken Plugin"
version = "0.1.0"
"#,
        )
        .unwrap();
        let options = PluginLoadOptions::new()
            .with_discovery_root(&project_plugins, PluginSource::Project)
            .with_discovery_root(&user_plugins, PluginSource::User);
        let mut context = PluginLoadContext::new(options);

        let outcome = service.run_plugin_load(&mut context).await.unwrap();

        assert!(outcome.loaded_plugin_ids.is_empty());
        assert_eq!(outcome.capabilities.tool_providers, 0);
        assert_eq!(outcome.diagnostics.len(), 2);
        assert!(outcome.diagnostics.iter().any(|diagnostic| {
            diagnostic.plugin_id.as_deref() == Some("project-lua")
                && diagnostic.message.contains("Lua plugin entry is required")
        }));
        assert!(outcome.diagnostics.iter().any(|diagnostic| {
            diagnostic.plugin_id.as_deref() == Some("")
                && diagnostic.message.contains("plugin id must not be empty")
        }));
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

        fn run_command(
            &self,
            command_id: &str,
            _args: serde_json::Value,
        ) -> Result<String, PluginError> {
            assert_eq!(command_id, "plugin.command");
            Ok("plugin command".to_owned())
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

    #[test]
    fn export_flow_node_ids_are_stable() {
        let service = FlowService::new();

        service.export_flow().unwrap();

        assert_eq!(
            crate::coding_session::export_flow::ExportFlow::node_ids(),
            &[
                "start_export",
                "load_session_replay",
                "select_export_view",
                "render_export",
                "write_export",
                "emit_completion",
            ]
        );
    }

    #[test]
    fn export_flow_writes_html_from_session_replay() {
        let service = FlowService::new();
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("exports/session.html");
        let summary = CodingAgentSessionSummary {
            session_id: "sess_export_flow".into(),
            session_dir: temp.path().join("sess_export_flow"),
            created_at: "2026-07-02T00:00:00Z".into(),
            updated_at: "2026-07-02T00:00:00Z".into(),
            active_leaf_id: Some("leaf_1".into()),
        };
        let replay = SessionReplay {
            session_id: "sess_export_flow".into(),
            cwd: Some("/workspace/pi-rust".into()),
            active_leaf_id: Some("leaf_1".into()),
            leaves: Vec::new(),
            transcript: vec![TranscriptItem::UserInput {
                turn_id: "turn_1".into(),
                text: "hello <flow>".into(),
            }],
            diagnostics: Vec::new(),
            pending_delegation_confirmations: Vec::new(),
            usage: Default::default(),
            operation_statuses: Default::default(),
        };
        let mut context = ExportContext::new(ExportOptions::html(output.clone()), summary, replay);

        let outcome = service.run_export(&mut context).unwrap();

        assert_eq!(outcome.path.as_deref(), Some(output.as_path()));
        assert_eq!(outcome.export.summary.session_id, "sess_export_flow");
        assert_eq!(
            outcome.export.transcript,
            vec![CodingAgentSessionExportItem::User {
                text: "hello <flow>".into(),
            }]
        );
        let html = fs::read_to_string(&output).unwrap();
        assert!(html.contains("sess_export_flow"), "{html}");
        assert!(html.contains("hello &lt;flow&gt;"), "{html}");
        assert!(html.contains("/workspace/pi-rust"), "{html}");
    }
}
