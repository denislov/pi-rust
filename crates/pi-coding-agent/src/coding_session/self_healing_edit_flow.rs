#![allow(dead_code)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use futures::{
    StreamExt,
    future::{BoxFuture, FutureExt},
};
use pi_agent_core::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome};
use pi_agent_core::{ExecOptions, ExecutionEnv, FileError};
use pi_ai::types::{
    AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, StopReason,
    StreamOptions,
};
use serde::Deserialize;
use tokio::process::Command;

use crate::tools::edit::{EditOperations, RealEditOperations, edit_execute_with_operations};

use super::CodingSessionError;
use super::FilesystemCapability;
use super::prompt::{PromptTurnOptions, RuntimeSnapshot};
use super::runtime_service::stream_model_for_scoped_runtime;

const DEFAULT_ACTION: &str = "default";

pub(crate) const SELF_HEALING_EDIT_NODE_IDS: &[&str] = &[
    "start_edit_workflow",
    "read_target",
    "propose_patch",
    "validate_patch",
    "apply_patch",
    "run_check",
    "repair_patch",
    "record_result",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditReplacement {
    pub old_text: String,
    pub new_text: String,
}

impl SelfHealingEditReplacement {
    pub fn new(old_text: impl Into<String>, new_text: impl Into<String>) -> Self {
        Self {
            old_text: old_text.into(),
            new_text: new_text.into(),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "oldText": self.old_text,
            "newText": self.new_text,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SelfHealingEditModelRepairOptions {
    prompt_options: PromptTurnOptions,
    max_attempts: usize,
}

impl SelfHealingEditModelRepairOptions {
    pub fn new(prompt_options: PromptTurnOptions) -> Self {
        Self {
            prompt_options,
            max_attempts: 1,
        }
    }

    pub fn with_max_attempts(mut self, attempts: usize) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }

    pub fn prompt_options(&self) -> &PromptTurnOptions {
        &self.prompt_options
    }

    pub(crate) fn prompt_options_mut(&mut self) -> &mut PromptTurnOptions {
        &mut self.prompt_options
    }

    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }

    pub(crate) fn into_parts(self) -> (PromptTurnOptions, usize) {
        (self.prompt_options, self.max_attempts)
    }
}

#[derive(Debug, Clone)]
pub struct SelfHealingEditRequest {
    path: String,
    replacements: Vec<SelfHealingEditReplacement>,
    check_command: Option<String>,
    repair_attempts: Vec<Vec<SelfHealingEditReplacement>>,
    model_repair: Option<SelfHealingEditModelRepairOptions>,
}

impl SelfHealingEditRequest {
    pub fn new(path: impl Into<String>, replacements: Vec<SelfHealingEditReplacement>) -> Self {
        Self {
            path: path.into(),
            replacements,
            check_command: None,
            repair_attempts: Vec::new(),
            model_repair: None,
        }
    }

    pub fn with_check_command(mut self, command: impl Into<String>) -> Self {
        self.check_command = Some(command.into());
        self
    }

    pub fn with_repair_attempts(mut self, attempts: Vec<Vec<SelfHealingEditReplacement>>) -> Self {
        self.repair_attempts = attempts;
        self
    }

    pub fn with_model_repair(mut self, options: SelfHealingEditModelRepairOptions) -> Self {
        self.model_repair = Some(options);
        self
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn replacements(&self) -> &[SelfHealingEditReplacement] {
        &self.replacements
    }

    pub fn check_command(&self) -> Option<&str> {
        self.check_command.as_deref()
    }

    pub fn repair_attempts(&self) -> &[Vec<SelfHealingEditReplacement>] {
        &self.repair_attempts
    }

    pub fn model_repair(&self) -> Option<&SelfHealingEditModelRepairOptions> {
        self.model_repair.as_ref()
    }

    pub(crate) fn model_repair_mut(&mut self) -> Option<&mut SelfHealingEditModelRepairOptions> {
        self.model_repair.as_mut()
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        String,
        Vec<SelfHealingEditReplacement>,
        Option<String>,
        Vec<Vec<SelfHealingEditReplacement>>,
        Option<SelfHealingEditModelRepairOptions>,
    ) {
        (
            self.path,
            self.replacements,
            self.check_command,
            self.repair_attempts,
            self.model_repair,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditDiagnostic {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditCheckOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditRepairAttempt {
    pub attempt: usize,
    pub replacements: Vec<SelfHealingEditReplacement>,
    pub diagnostics: Vec<SelfHealingEditDiagnostic>,
    pub check_output: Option<SelfHealingEditCheckOutput>,
}

pub(crate) trait SelfHealingEditCheckRunner: Send + Sync {
    fn run_check<'a>(
        &'a self,
        cwd: &'a Path,
        command: &'a str,
    ) -> BoxFuture<'a, Result<SelfHealingEditCheckOutput, String>>;
}

pub(crate) trait SelfHealingEditRepairStrategy: Send + Sync {
    fn repair<'a>(
        &'a self,
        attempt: usize,
        path: &'a str,
        replacements: &'a [SelfHealingEditReplacement],
        diagnostics: &'a [SelfHealingEditDiagnostic],
    ) -> BoxFuture<'a, Result<Vec<SelfHealingEditReplacement>, String>>;
}

pub(crate) trait SelfHealingEditObserver: Send + Sync {
    fn repair_attempted<'a>(
        &'a self,
        path: &'a str,
        repair: &'a SelfHealingEditRepairAttempt,
    ) -> BoxFuture<'a, ()>;
}

pub(crate) struct PlannedSelfHealingEditRepairStrategy {
    attempts: Vec<Vec<SelfHealingEditReplacement>>,
}

impl PlannedSelfHealingEditRepairStrategy {
    pub(crate) fn new(attempts: Vec<Vec<SelfHealingEditReplacement>>) -> Self {
        Self { attempts }
    }
}

impl SelfHealingEditRepairStrategy for PlannedSelfHealingEditRepairStrategy {
    fn repair<'a>(
        &'a self,
        attempt: usize,
        _path: &'a str,
        _replacements: &'a [SelfHealingEditReplacement],
        _diagnostics: &'a [SelfHealingEditDiagnostic],
    ) -> BoxFuture<'a, Result<Vec<SelfHealingEditReplacement>, String>> {
        async move {
            let index = attempt.saturating_sub(1);
            self.attempts.get(index).cloned().ok_or_else(|| {
                format!("self-healing edit repair attempt {attempt} was not configured")
            })
        }
        .boxed()
    }
}

pub(crate) struct ModelSelfHealingEditRepairStrategy {
    runtime: RuntimeSnapshot,
}

impl ModelSelfHealingEditRepairStrategy {
    pub(crate) fn new(runtime: RuntimeSnapshot) -> Self {
        Self { runtime }
    }
}

impl SelfHealingEditRepairStrategy for ModelSelfHealingEditRepairStrategy {
    fn repair<'a>(
        &'a self,
        attempt: usize,
        path: &'a str,
        replacements: &'a [SelfHealingEditReplacement],
        diagnostics: &'a [SelfHealingEditDiagnostic],
    ) -> BoxFuture<'a, Result<Vec<SelfHealingEditReplacement>, String>> {
        async move {
            let prompt = model_repair_prompt(attempt, path, replacements, diagnostics);
            let response = stream_model_repair(&self.runtime, prompt).await?;
            parse_model_repair_response(&response)
        }
        .boxed()
    }
}

#[derive(Clone)]
pub(crate) struct SelfHealingEditOptions {
    filesystem: FilesystemCapability,
    path: String,
    replacements: Vec<SelfHealingEditReplacement>,
    operations: Arc<dyn EditOperations>,
    check_command: Option<String>,
    check_runner: Option<Arc<dyn SelfHealingEditCheckRunner>>,
    repair_strategy: Option<Arc<dyn SelfHealingEditRepairStrategy>>,
    max_repair_attempts: usize,
    repair_observer: Option<Arc<dyn SelfHealingEditObserver>>,
}

impl SelfHealingEditOptions {
    pub(crate) fn new(
        cwd: impl Into<PathBuf>,
        path: impl Into<String>,
        replacements: Vec<SelfHealingEditReplacement>,
    ) -> Self {
        Self {
            filesystem: FilesystemCapability { cwd: cwd.into() },
            path: path.into(),
            replacements,
            operations: Arc::new(RealEditOperations),
            check_command: None,
            check_runner: None,
            repair_strategy: None,
            max_repair_attempts: 0,
            repair_observer: None,
        }
    }

    pub(crate) fn with_operations(mut self, operations: Arc<dyn EditOperations>) -> Self {
        self.operations = operations;
        self
    }

    pub(crate) fn with_check_command(mut self, command: impl Into<String>) -> Self {
        self.check_command = Some(command.into());
        self
    }

    pub(crate) fn with_check_runner(mut self, runner: Arc<dyn SelfHealingEditCheckRunner>) -> Self {
        self.check_runner = Some(runner);
        self
    }

    pub(crate) fn with_real_check_runner(mut self) -> Self {
        self.check_runner = Some(Arc::new(RealSelfHealingEditCheckRunner));
        self
    }

    pub(crate) fn with_repair_strategy(
        mut self,
        strategy: Arc<dyn SelfHealingEditRepairStrategy>,
    ) -> Self {
        self.repair_strategy = Some(strategy);
        self
    }

    pub(crate) fn with_max_repair_attempts(mut self, attempts: usize) -> Self {
        self.max_repair_attempts = attempts;
        self
    }

    pub(crate) fn with_repair_observer(
        mut self,
        observer: Arc<dyn SelfHealingEditObserver>,
    ) -> Self {
        self.repair_observer = Some(observer);
        self
    }

    pub(crate) fn with_execution_env<E>(mut self, env: E) -> Self
    where
        E: ExecutionEnv + Clone + 'static,
    {
        self.operations = Arc::new(ExecutionEnvEditOperations::new(env.clone()));
        self.check_runner = Some(Arc::new(ExecutionEnvCheckRunner::new(env)));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHealingEditOutcome {
    pub path: String,
    pub message: String,
    pub diff: String,
    pub patch: String,
    pub first_changed_line: Option<usize>,
    pub attempts: usize,
    pub diagnostics: Vec<SelfHealingEditDiagnostic>,
    pub check_output: Option<SelfHealingEditCheckOutput>,
    pub repair_attempts: Vec<SelfHealingEditRepairAttempt>,
}

pub(crate) struct SelfHealingEditContext {
    options: SelfHealingEditOptions,
    target_was_read: bool,
    proposal_ready: bool,
    apply_output: Option<pi_agent_core::AgentToolOutput>,
    outcome: Option<SelfHealingEditOutcome>,
    diagnostics: Vec<SelfHealingEditDiagnostic>,
    attempts: usize,
    repair_attempts: usize,
    repair_attempt_records: Vec<SelfHealingEditRepairAttempt>,
    check_output: Option<SelfHealingEditCheckOutput>,
    check_failed: bool,
    failure_error: Option<CodingSessionError>,
}

impl SelfHealingEditContext {
    pub(crate) fn new(options: SelfHealingEditOptions) -> Self {
        Self {
            options,
            target_was_read: false,
            proposal_ready: false,
            apply_output: None,
            outcome: None,
            diagnostics: Vec::new(),
            attempts: 0,
            repair_attempts: 0,
            repair_attempt_records: Vec::new(),
            check_output: None,
            check_failed: false,
            failure_error: None,
        }
    }

    pub(crate) fn diagnostics(&self) -> &[SelfHealingEditDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn repair_attempts(&self) -> &[SelfHealingEditRepairAttempt] {
        &self.repair_attempt_records
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<SelfHealingEditOutcome, CodingSessionError> {
        self.outcome
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "self-healing edit cannot finish without a recorded result".into(),
            })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.diagnostics.push(SelfHealingEditDiagnostic {
            message: message.clone(),
        });
        self.failure_error = Some(error);
        message
    }

    fn start_edit_workflow(&mut self) -> Result<(), CodingSessionError> {
        if self.options.path.trim().is_empty() {
            return Err(session_error("self-healing edit path must not be empty"));
        }
        if self.options.replacements.is_empty() {
            return Err(session_error(
                "self-healing edit requires at least one replacement",
            ));
        }
        self.target_was_read = false;
        self.proposal_ready = false;
        self.apply_output = None;
        self.outcome = None;
        self.attempts = 0;
        self.repair_attempts = 0;
        self.repair_attempt_records.clear();
        self.check_output = None;
        self.check_failed = false;
        Ok(())
    }

    async fn read_target(&mut self) -> Result<(), CodingSessionError> {
        let resolved = self.options.filesystem.resolve_path(&self.options.path)?;
        self.options
            .operations
            .read_file(&resolved)
            .await
            .map_err(session_error)?;
        self.target_was_read = true;
        Ok(())
    }

    fn propose_patch(&mut self) -> Result<(), CodingSessionError> {
        if !self.target_was_read {
            return Err(session_error(
                "self-healing edit cannot propose before reading target",
            ));
        }
        self.proposal_ready = true;
        Ok(())
    }

    fn validate_patch(&mut self) -> Result<(), CodingSessionError> {
        if !self.proposal_ready {
            return Err(session_error(
                "self-healing edit cannot validate before proposal",
            ));
        }
        for replacement in &self.options.replacements {
            if replacement.old_text.is_empty() {
                return Err(session_error(format!(
                    "oldText must not be empty in {}.",
                    self.options.path
                )));
            }
        }
        Ok(())
    }

    async fn apply_patch(&mut self) -> Result<(), CodingSessionError> {
        self.attempts += 1;
        let args = serde_json::json!({
            "path": self.options.path,
            "edits": self
                .options
                .replacements
                .iter()
                .map(SelfHealingEditReplacement::to_json)
                .collect::<Vec<_>>(),
        });
        let output = edit_execute_with_operations(
            &self.options.filesystem.cwd,
            args,
            self.options.operations.clone(),
        )
        .await
        .map_err(session_error)?;
        self.apply_output = Some(output);
        Ok(())
    }

    async fn run_check(&mut self) -> Result<(), CodingSessionError> {
        self.check_failed = false;
        let Some(command) = self.options.check_command.as_deref() else {
            return Ok(());
        };
        if command.trim().is_empty() {
            return Err(session_error(
                "self-healing edit check command must not be empty",
            ));
        }
        let runner = self.options.check_runner.clone().ok_or_else(|| {
            session_error("self-healing edit check command requires a check runner")
        })?;
        let output = runner
            .run_check(&self.options.filesystem.cwd, command)
            .await
            .map_err(|error| {
                session_error(format!("self-healing edit check failed to run: {error}"))
            })?;
        self.check_failed = output.exit_code != 0;
        if self.check_failed {
            self.diagnostics.push(SelfHealingEditDiagnostic {
                message: check_failure_message(&output),
            });
        }
        self.check_output = Some(output);
        Ok(())
    }

    async fn repair_patch(&mut self) -> Result<(), CodingSessionError> {
        if !self.check_failed {
            return Ok(());
        }
        let Some(strategy) = self.options.repair_strategy.clone() else {
            return Err(self.check_failure_error());
        };
        if self.options.max_repair_attempts == 0 {
            return Err(self.check_failure_error());
        }

        while self.check_failed && self.repair_attempts < self.options.max_repair_attempts {
            self.repair_attempts += 1;
            let replacements = match strategy
                .repair(
                    self.repair_attempts,
                    &self.options.path,
                    &self.options.replacements,
                    &self.diagnostics,
                )
                .await
            {
                Ok(replacements) => replacements,
                Err(error) => return Err(self.repair_failure_error(error)),
            };
            if replacements.is_empty() {
                return Err(session_error(
                    "self-healing edit repair produced no replacements",
                ));
            }
            let applied_replacements = replacements.clone();
            self.options.replacements = replacements;
            self.proposal_ready = true;
            self.validate_patch()?;
            self.apply_patch().await?;
            self.run_check().await?;
            let repair = SelfHealingEditRepairAttempt {
                attempt: self.repair_attempts,
                replacements: applied_replacements,
                diagnostics: self.diagnostics.clone(),
                check_output: self.check_output.clone(),
            };
            self.notify_repair_attempted(&repair).await;
            self.repair_attempt_records.push(repair);
        }

        if self.check_failed {
            return Err(self.check_failure_error());
        }
        Ok(())
    }

    async fn notify_repair_attempted(&self, repair: &SelfHealingEditRepairAttempt) {
        if let Some(observer) = self.options.repair_observer.as_ref() {
            observer.repair_attempted(&self.options.path, repair).await;
        }
    }

    fn check_failure_error(&self) -> CodingSessionError {
        CodingSessionError::SelfHealingEditFailed {
            message: self.latest_check_failure_message(),
            diagnostics: self.diagnostics.clone(),
            check_output: self.check_output.clone(),
            repair_attempts: self.repair_attempt_records.clone(),
        }
    }

    fn repair_failure_error(&self, error: impl std::fmt::Display) -> CodingSessionError {
        let message = format!("self-healing edit repair failed: {error}");
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.push(SelfHealingEditDiagnostic {
            message: message.clone(),
        });
        CodingSessionError::SelfHealingEditFailed {
            message,
            diagnostics,
            check_output: self.check_output.clone(),
            repair_attempts: self.repair_attempt_records.clone(),
        }
    }

    fn latest_check_failure_message(&self) -> String {
        self.check_output
            .as_ref()
            .map(check_failure_message)
            .unwrap_or_else(|| "self-healing edit check failed".to_owned())
    }

    fn record_result(&mut self) -> Result<(), CodingSessionError> {
        let output = self
            .apply_output
            .as_ref()
            .ok_or_else(|| session_error("self-healing edit cannot record result before apply"))?;
        let message = output
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let details = output.details.as_ref().ok_or_else(|| {
            session_error("self-healing edit output did not include edit details")
        })?;
        self.outcome = Some(SelfHealingEditOutcome {
            path: self.options.path.clone(),
            message,
            diff: details
                .get("diff")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            patch: details
                .get("patch")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            first_changed_line: details
                .get("firstChangedLine")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            attempts: self.attempts,
            diagnostics: self.diagnostics.clone(),
            check_output: self.check_output.clone(),
            repair_attempts: self.repair_attempt_records.clone(),
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelfHealingEditNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: SelfHealingEditNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelfHealingEditNodeKind {
    StartEditWorkflow,
    ReadTarget,
    ProposePatch,
    ValidatePatch,
    ApplyPatch,
    RunCheck,
    RepairPatch,
    RecordResult,
}

const SELF_HEALING_EDIT_NODE_SPECS: &[SelfHealingEditNodeSpec] = &[
    SelfHealingEditNodeSpec {
        id: "start_edit_workflow",
        name: "StartEditWorkflow",
        kind: SelfHealingEditNodeKind::StartEditWorkflow,
    },
    SelfHealingEditNodeSpec {
        id: "read_target",
        name: "ReadTarget",
        kind: SelfHealingEditNodeKind::ReadTarget,
    },
    SelfHealingEditNodeSpec {
        id: "propose_patch",
        name: "ProposePatch",
        kind: SelfHealingEditNodeKind::ProposePatch,
    },
    SelfHealingEditNodeSpec {
        id: "validate_patch",
        name: "ValidatePatch",
        kind: SelfHealingEditNodeKind::ValidatePatch,
    },
    SelfHealingEditNodeSpec {
        id: "apply_patch",
        name: "ApplyPatch",
        kind: SelfHealingEditNodeKind::ApplyPatch,
    },
    SelfHealingEditNodeSpec {
        id: "run_check",
        name: "RunCheck",
        kind: SelfHealingEditNodeKind::RunCheck,
    },
    SelfHealingEditNodeSpec {
        id: "repair_patch",
        name: "RepairPatch",
        kind: SelfHealingEditNodeKind::RepairPatch,
    },
    SelfHealingEditNodeSpec {
        id: "record_result",
        name: "RecordResult",
        kind: SelfHealingEditNodeKind::RecordResult,
    },
];

pub(crate) struct SelfHealingEditFlow {
    flow: Flow<SelfHealingEditContext>,
}

impl SelfHealingEditFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(SELF_HEALING_EDIT_NODE_IDS[0]).map_err(flow_error)?;
        for spec in SELF_HEALING_EDIT_NODE_SPECS {
            flow.add_node(spec.id, SelfHealingEditNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        for pair in SELF_HEALING_EDIT_NODE_IDS.windows(2) {
            flow.edge(pair[0], pair[1]).map_err(flow_error)?;
        }
        Ok(Self { flow })
    }

    #[cfg(test)]
    pub(crate) fn node_ids() -> &'static [&'static str] {
        SELF_HEALING_EDIT_NODE_IDS
    }

    pub(crate) async fn run(
        &self,
        ctx: &mut SelfHealingEditContext,
    ) -> Result<FlowOutcome, CodingSessionError> {
        self.flow.run(ctx).await.map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct SelfHealingEditNode {
    name: &'static str,
    kind: SelfHealingEditNodeKind,
}

impl SelfHealingEditNode {
    fn new(name: &'static str, kind: SelfHealingEditNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<SelfHealingEditContext> for SelfHealingEditNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut SelfHealingEditContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                SelfHealingEditNodeKind::StartEditWorkflow => ctx.start_edit_workflow(),
                SelfHealingEditNodeKind::ReadTarget => ctx.read_target().await,
                SelfHealingEditNodeKind::ProposePatch => ctx.propose_patch(),
                SelfHealingEditNodeKind::ValidatePatch => ctx.validate_patch(),
                SelfHealingEditNodeKind::ApplyPatch => ctx.apply_patch().await,
                SelfHealingEditNodeKind::RunCheck => ctx.run_check().await,
                SelfHealingEditNodeKind::RepairPatch => ctx.repair_patch().await,
                SelfHealingEditNodeKind::RecordResult => ctx.record_result(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

struct RealSelfHealingEditCheckRunner;

impl SelfHealingEditCheckRunner for RealSelfHealingEditCheckRunner {
    fn run_check<'a>(
        &'a self,
        cwd: &'a Path,
        command: &'a str,
    ) -> BoxFuture<'a, Result<SelfHealingEditCheckOutput, String>> {
        async move {
            let output = shell_check_command(command)
                .current_dir(cwd)
                .kill_on_drop(true)
                .output()
                .await
                .map_err(|error| error.to_string())?;
            Ok(SelfHealingEditCheckOutput {
                command: command.to_owned(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code().unwrap_or(-1),
            })
        }
        .boxed()
    }
}

fn shell_check_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut shell = Command::new("cmd");
        shell.arg("/C").arg(command);
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        shell.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        shell
    }
    #[cfg(not(windows))]
    {
        let mut shell = Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    }
}

struct ExecutionEnvCheckRunner<E> {
    env: E,
}

impl<E> ExecutionEnvCheckRunner<E> {
    fn new(env: E) -> Self {
        Self { env }
    }
}

impl<E> SelfHealingEditCheckRunner for ExecutionEnvCheckRunner<E>
where
    E: ExecutionEnv + Clone + 'static,
{
    fn run_check<'a>(
        &'a self,
        cwd: &'a Path,
        command: &'a str,
    ) -> BoxFuture<'a, Result<SelfHealingEditCheckOutput, String>> {
        async move {
            let output = self
                .env
                .exec(
                    command,
                    Some(ExecOptions {
                        cwd: Some(cwd.to_path_buf()),
                    }),
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(SelfHealingEditCheckOutput {
                command: command.to_owned(),
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
            })
        }
        .boxed()
    }
}

struct ExecutionEnvEditOperations<E> {
    env: E,
}

impl<E> ExecutionEnvEditOperations<E> {
    fn new(env: E) -> Self {
        Self { env }
    }
}

impl<E> EditOperations for ExecutionEnvEditOperations<E>
where
    E: ExecutionEnv + Clone + 'static,
{
    fn read_file<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        async move {
            self.env
                .read_binary_file(path.to_string_lossy().as_ref())
                .await
                .map_err(file_error_message)
        }
        .boxed()
    }

    fn write_file<'a>(
        &'a self,
        path: &'a Path,
        content: &'a [u8],
    ) -> BoxFuture<'a, Result<(), String>> {
        async move {
            self.env
                .write_file(path.to_string_lossy().as_ref(), content)
                .await
                .map_err(file_error_message)
        }
        .boxed()
    }
}

#[derive(Deserialize)]
struct ModelRepairResponse {
    edits: Vec<ModelRepairEdit>,
}

#[derive(Deserialize)]
struct ModelRepairEdit {
    #[serde(rename = "oldText")]
    old_text: String,
    #[serde(rename = "newText")]
    new_text: String,
}

fn model_repair_prompt(
    attempt: usize,
    path: &str,
    replacements: &[SelfHealingEditReplacement],
    diagnostics: &[SelfHealingEditDiagnostic],
) -> String {
    let replacement_values = replacements
        .iter()
        .map(SelfHealingEditReplacement::to_json)
        .collect::<Vec<_>>();
    let replacements_json =
        serde_json::to_string(&replacement_values).unwrap_or_else(|_| "[]".to_string());
    let diagnostic_messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    let diagnostics_json =
        serde_json::to_string(&diagnostic_messages).unwrap_or_else(|_| "[]".to_string());
    format!(
        "A self-healing edit check failed. Return only JSON shaped as {{\"edits\":[{{\"oldText\":\"...\",\"newText\":\"...\"}}]}} with replacements to apply to the current file.\nPath: {path}\nRepair attempt: {attempt}\nCurrent edits: {replacements_json}\nDiagnostics: {diagnostics_json}"
    )
}

async fn stream_model_repair(runtime: &RuntimeSnapshot, prompt: String) -> Result<String, String> {
    let context = Context {
        system_prompt: runtime.system_prompt().map(str::to_owned),
        messages: vec![Message::User {
            content: vec![ContentBlock::Text {
                text: prompt,
                text_signature: None,
            }],
        }],
        tools: None,
    };
    let mut stream =
        stream_model_for_scoped_runtime(runtime, context, model_repair_stream_options(runtime));
    let mut final_text = None;
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::Done { message, .. } => {
                if matches!(message.stop_reason, StopReason::Error) {
                    return Err(message.error_message.unwrap_or_else(|| {
                        "self-healing edit model repair returned an error".into()
                    }));
                }
                final_text = Some(assistant_message_text(&message));
            }
            AssistantMessageEvent::Error { message, .. } => {
                return Err(message
                    .error_message
                    .unwrap_or_else(|| "self-healing edit model repair stream failed".into()));
            }
            AssistantMessageEvent::Start { .. }
            | AssistantMessageEvent::TextStart { .. }
            | AssistantMessageEvent::TextDelta { .. }
            | AssistantMessageEvent::TextEnd { .. }
            | AssistantMessageEvent::ThinkingStart { .. }
            | AssistantMessageEvent::ThinkingDelta { .. }
            | AssistantMessageEvent::ThinkingEnd { .. }
            | AssistantMessageEvent::ToolcallStart { .. }
            | AssistantMessageEvent::ToolcallDelta { .. }
            | AssistantMessageEvent::ToolcallEnd { .. } => {}
        }
    }
    let text = final_text.ok_or_else(|| {
        "self-healing edit model repair did not return a final message".to_string()
    })?;
    if text.trim().is_empty() {
        return Err("self-healing edit model repair returned empty text".into());
    }
    Ok(text)
}

fn model_repair_stream_options(runtime: &RuntimeSnapshot) -> Option<StreamOptions> {
    crate::runtime::build_agent_config_with_auth_diagnostics(
        runtime.model().clone(),
        runtime.system_prompt().map(str::to_owned),
        runtime.max_turns(),
        runtime.api_key().map(str::to_owned),
        runtime.auth_diagnostics().to_vec(),
        runtime.thinking_level(),
        runtime.tool_execution(),
        runtime.resources().clone(),
        runtime.settings(),
    )
    .stream_options
}

fn assistant_message_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_model_repair_response(text: &str) -> Result<Vec<SelfHealingEditReplacement>, String> {
    let response: ModelRepairResponse = serde_json::from_str(text.trim()).map_err(|error| {
        format!("self-healing edit model repair response was not valid JSON edits: {error}")
    })?;
    if response.edits.is_empty() {
        return Err("self-healing edit model repair response contained no edits".into());
    }
    Ok(response
        .edits
        .into_iter()
        .map(|edit| SelfHealingEditReplacement::new(edit.old_text, edit.new_text))
        .collect())
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn session_error(message: impl Into<String>) -> CodingSessionError {
    CodingSessionError::Session {
        message: message.into(),
    }
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}

fn check_failure_message(output: &SelfHealingEditCheckOutput) -> String {
    let mut message = format!(
        "self-healing edit check failed: `{}` exited with {}",
        output.command, output.exit_code
    );
    let stderr = output.stderr.trim();
    let stdout = output.stdout.trim();
    if !stderr.is_empty() {
        message.push_str(&format!("; stderr: {}", compact_check_text(stderr)));
    } else if !stdout.is_empty() {
        message.push_str(&format!("; stdout: {}", compact_check_text(stdout)));
    }
    message
}

fn compact_check_text(text: &str) -> String {
    const MAX_LEN: usize = 240;
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_LEN {
        compact
    } else {
        format!("{}...", compact.chars().take(MAX_LEN).collect::<String>())
    }
}

fn file_error_message(error: FileError) -> String {
    error.to_string()
}
