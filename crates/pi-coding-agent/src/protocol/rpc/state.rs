use crate::protocol::rpc::events::RpcCodingEventAdapter;
use crate::{
    CliArgs, CliError, CliRunOptions, coding_session::AgentInvocationOutcome,
    coding_session::AgentTeamOutcome, coding_session::CodingAgentSession,
    coding_session::OperationKind, coding_session::ProductEvent,
    coding_session::PromptControlHandle, coding_session::PromptTurnOutcome, config, select_model,
};
use pi_agent_core::transcript::StoredAgentMessage;
use pi_agent_core::{QueueMode, ThinkingLevel};
use pi_ai::types::Model;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub(super) struct RpcState {
    pub(super) options: CliRunOptions,
    pub(super) model: Model,
    pub(super) api_key: Option<String>,
    pub(super) settings: crate::config::Settings,
    pub(super) thinking_level: ThinkingLevel,
    pub(super) steering_mode: QueueMode,
    pub(super) follow_up_mode: QueueMode,
    pub(super) auto_compaction_enabled: bool,
    pub(super) session_name: Option<String>,
    pub(super) active_session_path: Option<PathBuf>,
    pub(super) active_leaf_id: Option<String>,
    pub(super) messages: Vec<StoredAgentMessage>,
    pub(super) coding_session: Option<CodingAgentSession>,
    pub(super) running: Option<RunningPrompt>,
    pub(super) is_compacting: bool,
    pub(super) steering: Vec<String>,
    pub(super) follow_up: Vec<String>,
}

pub(super) enum RunningPrompt {
    Coding(CodingRunningPrompt),
}

pub(super) struct CodingRunningPrompt {
    pub(super) events: mpsc::UnboundedReceiver<ProductEvent>,
    pub(super) done: oneshot::Receiver<CodingOperationTaskResult>,
    pub(super) control: Option<PromptControlHandle>,
    pub(super) operation_kind: OperationKind,
    pub(super) adapter: RpcCodingEventAdapter,
    pub(super) events_closed: bool,
}

pub(super) struct CodingOperationTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) session_root: Option<PathBuf>,
    pub(super) outcome: CodingOperationOutcome,
}

pub(super) enum CodingOperationOutcome {
    Prompt(Result<PromptTurnOutcome, CliError>),
    AgentInvocation(Result<AgentInvocationOutcome, CliError>),
    AgentTeam(Result<AgentTeamOutcome, CliError>),
    DelegationApproval(Result<(), CliError>),
}

impl RpcState {
    pub(super) fn new(options: CliRunOptions) -> Result<Self, CliError> {
        let cwd = options.session.cwd.clone();
        let (config, config_diags) = config::load_config(&cwd);
        let diagnostics = config_diags
            .iter()
            .map(crate::request::CliDiagnostic::from_config)
            .collect::<Vec<_>>();
        let diag_text = crate::request::render_diagnostics(&diagnostics);
        if !diag_text.is_empty() {
            eprint!("{diag_text}");
        }
        let args = CliArgs::default();
        let model = select_model(
            &args,
            config.settings.default_provider.as_deref(),
            config.settings.default_model.as_deref(),
            options.model_override.clone(),
        )?;
        let api_key = {
            let mut key_diags = Vec::new();
            let resolved =
                config::auth::resolve_api_key(&model.provider, None, &config.auth, &mut key_diags);
            let key_diagnostics = key_diags
                .iter()
                .map(crate::request::CliDiagnostic::from_config)
                .collect::<Vec<_>>();
            let key_text = crate::request::render_diagnostics(&key_diagnostics);
            if !key_text.is_empty() {
                eprint!("{key_text}");
            }
            resolved.map(|r| r.value)
        };

        Ok(Self {
            options,
            model,
            api_key,
            settings: config.settings,
            thinking_level: ThinkingLevel::Off,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            auto_compaction_enabled: true,
            session_name: None,
            active_session_path: None,
            active_leaf_id: None,
            messages: Vec::new(),
            coding_session: None,
            running: None,
            is_compacting: false,
            steering: Vec::new(),
            follow_up: Vec::new(),
        })
    }

    pub(super) fn is_streaming(&self) -> bool {
        self.running.is_some()
    }
}
