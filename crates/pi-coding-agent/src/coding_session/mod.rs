mod capability_service;
mod context;
mod error;
mod event;
mod event_service;
mod flow_service;
mod plugin_service;
mod prompt;
mod prompt_flow;
mod runtime_service;
mod session_log;
mod session_service;

pub use context::{
    CapabilityStatus, CodingAgentCapabilities, CodingAgentSessionOptions,
    CodingAgentSessionSummary, CodingAgentSessionView,
};
pub(crate) use context::{
    CodingAgentSessionDiagnostic, CodingAgentSessionHydration, CodingAgentSessionTranscriptItem,
};
pub use error::CodingSessionError;
pub use event::CodingAgentEvent;
pub use event_service::CodingAgentEventReceiver;
pub use prompt::{
    CodingDiagnostic, CodingDiagnosticSeverity, PromptTurnMode, PromptTurnOptions,
    PromptTurnOutcome,
};

use capability_service::CapabilityService;
use event_service::EventService;
use flow_service::FlowService;
use plugin_service::PluginService;
use prompt::{PromptTurnContext, PromptTurnIds};
use runtime_service::RuntimeService;
use session_log::id::{IdGenerator, SystemIdGenerator};
use session_log::replay::TranscriptItem;
use session_service::{FinalizedSessionWrite, SessionService};

#[derive(Debug)]
pub struct CodingAgentSession {
    persistence: SessionPersistence,
    runtime_service: RuntimeService,
    flow_service: FlowService,
    event_service: EventService,
    capability_service: CapabilityService,
    plugin_service: PluginService,
    active_operation: Option<String>,
}

#[derive(Debug)]
enum SessionPersistence {
    Persistent(SessionService),
    NonPersistent(TransientSessionState),
}

#[derive(Debug)]
struct TransientSessionState {
    runtime_id: String,
    transcript: Vec<TranscriptItem>,
}

impl TransientSessionState {
    fn new() -> Self {
        let mut ids = SystemIdGenerator;
        Self {
            runtime_id: format!("runtime_{}", ids.next_session_id()),
            transcript: Vec::new(),
        }
    }
}

impl CodingAgentSession {
    pub async fn create(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::create(&options)?;
        Self::from_services(session_service)
    }

    pub async fn open(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        Self::from_services(session_service)
    }

    pub async fn open_or_create(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open_or_create(&options)?;
        Self::from_services(session_service)
    }

    pub async fn non_persistent(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_id().is_some() || options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "non-persistent coding sessions do not accept a session id or path".into(),
            });
        }
        Self::from_transient(TransientSessionState::new())
    }

    pub fn list(
        options: CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        SessionService::list(&options)
    }

    pub(crate) fn hydrate(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::hydrate(&options)
    }

    pub(crate) fn clone_session(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .clone_current()?
            .hydrated_view()
    }

    pub(crate) fn fork_session(
        options: CodingAgentSessionOptions,
        target_leaf_id: Option<&str>,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .fork_current(target_leaf_id)?
            .hydrated_view()
    }

    pub(crate) fn hydrate_current(
        &self,
    ) -> Result<Option<CodingAgentSessionHydration>, CodingSessionError> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => {
                Ok(Some(session_service.hydrated_view()?))
            }
            SessionPersistence::NonPersistent(_) => Ok(None),
        }
    }

    pub fn subscribe(&self) -> CodingAgentEventReceiver {
        self.event_service.subscribe()
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        self.capability_service
            .capabilities(self.active_operation.as_deref())
    }

    pub fn view(&self) -> CodingAgentSessionView {
        let _ = (
            &self.runtime_service,
            &self.flow_service,
            &self.plugin_service,
        );
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service.view(),
            SessionPersistence::NonPersistent(state) => CodingAgentSessionView {
                session_id: state.runtime_id.clone(),
            },
        }
    }

    pub async fn prompt(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if self.active_operation.is_some() {
            return Err(CodingSessionError::Busy {
                operation: "prompt".into(),
            });
        }
        self.active_operation = Some("prompt".into());
        let result = self.prompt_inner(options).await;
        self.active_operation = None;
        result
    }

    fn from_services(session_service: SessionService) -> Result<Self, CodingSessionError> {
        let event_service = EventService::new();
        event_service.emit(CodingAgentEvent::SessionOpened {
            session_id: session_service.session_id().to_owned(),
        });

        Ok(Self {
            persistence: SessionPersistence::Persistent(session_service),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            active_operation: None,
        })
    }

    fn from_transient(state: TransientSessionState) -> Result<Self, CodingSessionError> {
        Ok(Self {
            persistence: SessionPersistence::NonPersistent(state),
            runtime_service: RuntimeService::new(),
            flow_service: FlowService::new(),
            event_service: EventService::new(),
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            active_operation: None,
        })
    }

    async fn prompt_inner(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnOutcome, CodingSessionError> {
        if options.runtime().is_none() {
            return Err(CodingSessionError::Config {
                message: "prompt turn options do not include a runtime snapshot".into(),
            });
        }
        let mut context = self.prepare_prompt_context(options)?;
        let operation_id = context.operation_id().to_owned();
        let turn_id = context.turn_id().to_owned();

        self.event_service.emit(CodingAgentEvent::PromptStarted {
            operation_id,
            turn_id,
        });
        let mut outcome = match self.flow_service.run_prompt_turn(&mut context).await {
            Ok(outcome) => outcome,
            Err(error) => context.finish_failure(error),
        };
        let finalized = match self.finalize_prompt_transaction(&mut context, &outcome) {
            Ok(finalized) => finalized,
            Err(error) => {
                outcome = context.finish_failure(error.clone());
                SessionService::skip_prompt_transaction(
                    context.operation_id().to_owned(),
                    format!("session write finalization failed: {error}"),
                )
            }
        };
        apply_finalized_session_write(&mut outcome, &finalized);

        self.emit_coding_events_before_prompt_outcome(context.coding_events());
        self.emit_session_write_events(&finalized);
        self.emit_prompt_outcome_event(&outcome);
        Ok(outcome)
    }

    fn prepare_prompt_context(
        &mut self,
        options: PromptTurnOptions,
    ) -> Result<PromptTurnContext, CodingSessionError> {
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => {
                let replay = session_service.replay()?;
                let transaction = session_service.begin_prompt_transaction();
                let operation_id = transaction.operation_id().to_owned();
                let turn_id = transaction.turn_id().to_owned();
                let mut context =
                    PromptTurnContext::new(PromptTurnIds::new(operation_id, turn_id), options);
                context.set_session_id(session_service.session_id().to_owned());
                context.set_replay(replay);
                context.set_transaction(transaction);
                Ok(context)
            }
            SessionPersistence::NonPersistent(state) => {
                let mut ids = SystemIdGenerator;
                let mut context = PromptTurnContext::new(
                    PromptTurnIds::new(ids.next_operation_id(), ids.next_turn_id()),
                    options,
                );
                context
                    .set_non_persistent_session(state.runtime_id.clone(), state.transcript.clone());
                Ok(context)
            }
        }
    }

    fn finalize_prompt_transaction(
        &mut self,
        context: &mut PromptTurnContext,
        outcome: &PromptTurnOutcome,
    ) -> Result<FinalizedSessionWrite, CodingSessionError> {
        let operation_id = context.operation_id().to_owned();
        let transaction = context.take_transaction();
        match &mut self.persistence {
            SessionPersistence::Persistent(session_service) => match outcome {
                PromptTurnOutcome::Success { .. } => {
                    session_service.commit_prompt_transaction(transaction, operation_id)
                }
                PromptTurnOutcome::Aborted { reason, .. } => session_service
                    .abort_prompt_transaction(transaction, operation_id, reason.clone()),
                PromptTurnOutcome::Failed { error, .. } => session_service.fail_prompt_transaction(
                    transaction,
                    operation_id,
                    error.code(),
                    error.to_string(),
                ),
            },
            SessionPersistence::NonPersistent(state) => {
                if matches!(outcome, PromptTurnOutcome::Success { .. }) {
                    state
                        .transcript
                        .extend(context.completed_transcript_items());
                }
                Ok(SessionService::skip_prompt_transaction(
                    operation_id,
                    "session persistence disabled",
                ))
            }
        }
    }

    fn emit_coding_events_before_prompt_outcome(&self, events: &[CodingAgentEvent]) {
        for event in events {
            if is_prompt_outcome_event(event) {
                continue;
            }
            self.event_service.emit(event.clone());
        }
    }

    fn emit_session_write_events(&self, finalized: &FinalizedSessionWrite) {
        for event in &finalized.events {
            self.event_service.emit(event.clone());
        }
    }

    fn emit_prompt_outcome_event(&self, outcome: &PromptTurnOutcome) {
        match outcome {
            PromptTurnOutcome::Success {
                operation_id,
                turn_id,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptCompleted {
                operation_id: operation_id.clone(),
                turn_id: turn_id.clone(),
            }),
            PromptTurnOutcome::Aborted {
                operation_id,
                reason,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptAborted {
                operation_id: operation_id.clone(),
                reason: reason.clone(),
            }),
            PromptTurnOutcome::Failed {
                operation_id,
                error,
                ..
            } => self.event_service.emit(CodingAgentEvent::PromptFailed {
                operation_id: operation_id.clone(),
                error: error.clone(),
            }),
        }
    }

    #[cfg(test)]
    fn persistent_session_service(&self) -> &SessionService {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service,
            SessionPersistence::NonPersistent(_) => {
                panic!("expected persistent coding agent session")
            }
        }
    }
}

fn is_prompt_outcome_event(event: &CodingAgentEvent) -> bool {
    matches!(
        event,
        CodingAgentEvent::PromptCompleted { .. }
            | CodingAgentEvent::PromptFailed { .. }
            | CodingAgentEvent::PromptAborted { .. }
    )
}

fn apply_finalized_session_write(
    outcome: &mut PromptTurnOutcome,
    finalized: &FinalizedSessionWrite,
) {
    if let PromptTurnOutcome::Success {
        session_id,
        leaf_id,
        ..
    } = outcome
    {
        if let Some(finalized_session_id) = &finalized.session_id {
            *session_id = Some(finalized_session_id.clone());
        }
        *leaf_id = finalized.leaf_id.clone();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_stream::stream;
    use pi_agent_core::{AgentResources, AgentTool, AgentToolOutput};
    use pi_ai::providers::faux::{FauxProvider, FauxResponse, FauxToolCall};
    use pi_ai::registry;
    use pi_ai::registry::ApiProvider;
    use pi_ai::stream::EventStream;
    use pi_ai::types::{
        AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Message, Model, ModelCost,
        ModelInput, StopReason, StreamOptions,
    };

    use super::*;
    use crate::coding_session::session_log::event::{SessionEventData, SessionEventEnvelope};
    use crate::coding_session::session_log::replay::{MessageStatus, TranscriptItem};
    use crate::protocol::session_runner::SessionPromptOptions;
    use crate::runtime::{PromptInvocation, SessionRunOptions};

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

    fn prompt_options(api: &str, prompt: &str) -> PromptTurnOptions {
        prompt_options_with_tools(api, prompt, Vec::new())
    }

    fn prompt_options_with_tools(
        api: &str,
        prompt: &str,
        tools: Vec<AgentTool>,
    ) -> PromptTurnOptions {
        PromptTurnOptions::from_session_prompt_options(SessionPromptOptions {
            prompt: prompt.into(),
            model: model(api),
            api_key: None,
            system_prompt: Some("system".into()),
            max_turns: Some(2),
            tools,
            register_builtins: false,
            session: Some(SessionRunOptions::disabled(".".into())),
            session_target: None,
            session_name: None,
            thinking_level: None,
            tool_execution: None,
            resources: AgentResources::default(),
            settings: None,
            invocation: PromptInvocation::Text(prompt.into()),
        })
    }

    fn echo_tool() -> AgentTool {
        AgentTool {
            name: "echo".into(),
            description: "echoes input".into(),
            parameters: serde_json::json!({"type": "object"}),
            execution_mode: None,
            execute: Arc::new(|args, _on_update| {
                let text = args
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_owned();
                Box::pin(async move {
                    Ok(AgentToolOutput::new(vec![ContentBlock::Text {
                        text: format!("echo: {text}"),
                        text_signature: None,
                    }]))
                })
            }),
        }
    }

    struct RecordingProvider {
        contexts: Arc<Mutex<Vec<Context>>>,
        response: String,
    }

    impl RecordingProvider {
        fn new(contexts: Arc<Mutex<Vec<Context>>>, response: impl Into<String>) -> Self {
            Self {
                contexts,
                response: response.into(),
            }
        }
    }

    impl ApiProvider for RecordingProvider {
        fn stream(&self, model: &Model, ctx: Context, _opts: Option<StreamOptions>) -> EventStream {
            self.contexts.lock().unwrap().push(ctx);
            let model_id = model.id.clone();
            let response = self.response.clone();
            Box::pin(stream! {
                let mut message = AssistantMessage::empty("recording", &model_id);
                message.provider = Some("recording".into());
                message.content.push(ContentBlock::Text {
                    text: response,
                    text_signature: None,
                });
                yield AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message,
                };
            })
        }
    }

    #[tokio::test]
    async fn prompt_runs_flow_and_commits_session_events() {
        let api = "coding-session-prompt";
        registry::register(api, Arc::new(FauxProvider::simple_text("session answer")));
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_prompt")
            .with_session_log_root(temp.path());
        let mut session = CodingAgentSession::create(options.clone()).await.unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        let leaf_id = match &outcome {
            PromptTurnOutcome::Success {
                final_text,
                session_id: Some(session_id),
                leaf_id: Some(leaf_id),
                ..
            } if final_text == "session answer" && session_id == "sess_prompt" => leaf_id.clone(),
            other => panic!("expected successful prompt with committed leaf, got {other:?}"),
        };
        assert!(leaf_id.starts_with("leaf_"));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptStarted { .. })
        ));
        assert!(matches!(
            events.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTurnStarted { .. })
        ));
        let remaining_events =
            std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &remaining_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_completed",
            ],
        );
        assert_eq!(
            remaining_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptCompleted { .. }))
                .count(),
            1
        );

        let replay = session.persistent_session_service().replay().unwrap();
        assert_eq!(replay.active_leaf_id.as_deref(), Some(leaf_id.as_str()));
        assert!(matches!(
            replay.transcript.as_slice(),
            [
                TranscriptItem::UserInput {
                    turn_id,
                    text,
                },
                TranscriptItem::AssistantMessage {
                    text: assistant_text,
                    status: MessageStatus::Completed,
                    ..
                },
            ] if turn_id == outcome_turn_id(&outcome)
                && text == "hello"
                && assistant_text == "session answer"
        ));
        let committed_leaf = std::fs::read_to_string(temp.path().join("sess_prompt/events.jsonl"))
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<SessionEventEnvelope>(line).ok())
            .find_map(|event| match event.data {
                SessionEventData::OperationCommitted {
                    new_leaf_id: Some(leaf_id),
                } => Some(leaf_id),
                _ => None,
            })
            .unwrap();
        assert_eq!(committed_leaf, leaf_id);
        let hydrated = session.hydrate_current().unwrap().unwrap();
        assert_eq!(
            hydrated.summary.active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        let summaries = CodingAgentSession::list(options).unwrap();
        assert_eq!(
            summaries[0].active_leaf_id.as_deref(),
            Some(leaf_id.as_str())
        );
        assert_eq!(session.view().session_id, "sess_prompt");
        registry::unregister(api);
    }

    #[tokio::test]
    async fn prompt_requires_runtime_backed_options() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_missing_runtime")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        let error = session
            .prompt(PromptTurnOptions::new(PromptInvocation::Text(
                "hello".into(),
            )))
            .await
            .unwrap_err();

        assert_eq!(error.code(), "config");
        assert!(error.to_string().contains("runtime snapshot"));
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .transcript
                .is_empty()
        );
        assert!(
            session
                .hydrate_current()
                .unwrap()
                .unwrap()
                .summary
                .active_leaf_id
                .is_none()
        );
    }

    #[tokio::test]
    async fn non_persistent_constructor_does_not_create_session_files() {
        let temp = tempfile::tempdir().unwrap();
        let session = CodingAgentSession::non_persistent(
            CodingAgentSessionOptions::new().with_session_log_root(temp.path()),
        )
        .await
        .unwrap();

        assert!(session.view().session_id.starts_with("runtime_sess_"));
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn non_persistent_prompt_emits_skipped_write_before_completion() {
        let api = "coding-session-non-persistent-prompt";
        registry::register(api, Arc::new(FauxProvider::simple_text("transient answer")));
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(
            &outcome,
            PromptTurnOutcome::Success {
                final_text,
                session_id: None,
                leaf_id: None,
                ..
            } if final_text == "transient answer"
        ));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &["session_write_skipped", "prompt_completed"],
        );
        assert!(emitted_events.iter().any(|event| matches!(
            event,
            CodingAgentEvent::SessionWriteSkipped { reason, .. }
                if reason == "session persistence disabled"
        )));
        registry::unregister(api);
    }

    #[tokio::test]
    async fn non_persistent_prompt_hydrates_owner_lifetime_transcript() {
        let first_api = "coding-session-non-persistent-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::simple_text("first answer")),
        );
        let mut session = CodingAgentSession::non_persistent(CodingAgentSessionOptions::new())
            .await
            .unwrap();

        session
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-non-persistent-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );

        session
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    #[tokio::test]
    async fn prompt_does_not_duplicate_failure_event_from_agent_error() {
        let api = "coding-session-prompt-error";
        registry::register(
            api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::text_call("partial", StopReason::Error),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let mut session = CodingAgentSession::create(
            CodingAgentSessionOptions::new()
                .with_session_id("sess_prompt_error")
                .with_session_log_root(temp.path()),
        )
        .await
        .unwrap();
        let mut events = session.subscribe();

        let outcome = session.prompt(prompt_options(api, "hello")).await.unwrap();

        assert!(matches!(outcome, PromptTurnOutcome::Failed { .. }));
        let emitted_events = std::iter::from_fn(|| events.try_recv().unwrap()).collect::<Vec<_>>();
        assert_event_order(
            &emitted_events,
            &[
                "session_write_pending",
                "session_write_committed",
                "prompt_failed",
            ],
        );
        assert_eq!(
            emitted_events
                .iter()
                .filter(|event| matches!(event, CodingAgentEvent::PromptFailed { .. }))
                .count(),
            1
        );
        assert!(
            session
                .persistent_session_service()
                .replay()
                .unwrap()
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("operation")
                    && diagnostic.message.contains("failed"))
        );
        registry::unregister(api);
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_transcript_when_opening_session() {
        let first_api = "coding-session-hydrate-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::simple_text("first answer")),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options(first_api, "first question"))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-hydrate-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        let outcome = opened
            .prompt(prompt_options(second_api, "second question"))
            .await
            .unwrap();

        assert!(matches!(
            outcome,
            PromptTurnOutcome::Success { final_text, .. } if final_text == "second answer"
        ));
        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 3);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "first question".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[1],
            Message::Assistant { content }
                if content == &vec![ContentBlock::Text {
                    text: "first answer".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[2],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "second question".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    #[tokio::test]
    async fn prompt_hydrates_replayed_tool_calls_when_opening_session() {
        let first_api = "coding-session-hydrate-tool-first";
        registry::register(
            first_api,
            Arc::new(FauxProvider::with_call_queue(vec![
                FauxProvider::single_call(
                    vec![FauxResponse {
                        text_deltas: Vec::new(),
                        thinking_deltas: Vec::new(),
                        tool_calls: vec![FauxToolCall {
                            id: "toolu_1".into(),
                            name: "echo".into(),
                            deltas: Vec::new(),
                            final_arguments: serde_json::json!({"text": "hi"}),
                        }],
                    }],
                    StopReason::ToolUse,
                ),
                FauxProvider::text_call("tool final", StopReason::Stop),
            ])),
        );
        let temp = tempfile::tempdir().unwrap();
        let options = CodingAgentSessionOptions::new()
            .with_session_id("sess_tool_hydrate")
            .with_session_log_root(temp.path());
        let mut created = CodingAgentSession::create(options.clone()).await.unwrap();
        created
            .prompt(prompt_options_with_tools(
                first_api,
                "use the tool",
                vec![echo_tool()],
            ))
            .await
            .unwrap();
        registry::unregister(first_api);

        let second_api = "coding-session-hydrate-tool-second";
        let contexts = Arc::new(Mutex::new(Vec::new()));
        registry::register(
            second_api,
            Arc::new(RecordingProvider::new(
                Arc::clone(&contexts),
                "second answer",
            )),
        );
        let mut opened = CodingAgentSession::open(options).await.unwrap();

        opened
            .prompt(prompt_options(second_api, "continue"))
            .await
            .unwrap();

        let contexts = contexts.lock().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].messages.len(), 4);
        assert!(matches!(
            &contexts[0].messages[0],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "use the tool".into(),
                    text_signature: None,
                }]
        ));
        let tool_call_id = match &contexts[0].messages[1] {
            Message::Assistant { content } => match content.as_slice() {
                [
                    ContentBlock::Text { text, .. },
                    ContentBlock::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    },
                ] => {
                    assert_eq!(text, "tool final");
                    assert_eq!(name, "echo");
                    assert_eq!(arguments, &serde_json::json!({"text": "hi"}));
                    id.clone()
                }
                other => panic!("unexpected assistant content: {other:?}"),
            },
            other => panic!("unexpected hydrated assistant message: {other:?}"),
        };
        assert!(matches!(
            &contexts[0].messages[2],
            Message::ToolResult {
                tool_call_id: result_tool_call_id,
                tool_name: Some(tool_name),
                is_error: Some(false),
                content,
            } if result_tool_call_id == &tool_call_id
                && tool_name == "echo"
                && content == &vec![ContentBlock::Text {
                    text: "echo: hi".into(),
                    text_signature: None,
                }]
        ));
        assert!(matches!(
            &contexts[0].messages[3],
            Message::User { content }
                if content == &vec![ContentBlock::Text {
                    text: "continue".into(),
                    text_signature: None,
                }]
        ));
        registry::unregister(second_api);
    }

    fn outcome_turn_id(outcome: &PromptTurnOutcome) -> &str {
        match outcome {
            PromptTurnOutcome::Success { turn_id, .. } => turn_id,
            _ => panic!("expected success outcome"),
        }
    }

    fn assert_event_order(events: &[CodingAgentEvent], expected: &[&str]) {
        let observed = events.iter().map(event_kind).collect::<Vec<_>>();
        let mut next_index = 0;
        for kind in observed {
            if next_index < expected.len() && kind == expected[next_index] {
                next_index += 1;
            }
        }
        assert_eq!(
            next_index,
            expected.len(),
            "did not observe event order {expected:?}"
        );
    }

    fn event_kind(event: &CodingAgentEvent) -> &'static str {
        match event {
            CodingAgentEvent::SessionOpened { .. } => "session_opened",
            CodingAgentEvent::SessionWritePending { .. } => "session_write_pending",
            CodingAgentEvent::SessionWriteCommitted { .. } => "session_write_committed",
            CodingAgentEvent::SessionWriteSkipped { .. } => "session_write_skipped",
            CodingAgentEvent::PromptStarted { .. } => "prompt_started",
            CodingAgentEvent::AgentTurnStarted { .. } => "agent_turn_started",
            CodingAgentEvent::ProviderRequestStarted { .. } => "provider_request_started",
            CodingAgentEvent::AssistantMessageStarted { .. } => "assistant_message_started",
            CodingAgentEvent::AssistantMessageDelta { .. } => "assistant_message_delta",
            CodingAgentEvent::AssistantMessageCompleted { .. } => "assistant_message_completed",
            CodingAgentEvent::ToolCallStarted { .. } => "tool_call_started",
            CodingAgentEvent::ToolCallUpdated { .. } => "tool_call_updated",
            CodingAgentEvent::ToolCallCompleted { .. } => "tool_call_completed",
            CodingAgentEvent::ToolCallFailed { .. } => "tool_call_failed",
            CodingAgentEvent::RuntimeCompactionCompleted { .. } => "runtime_compaction_completed",
            CodingAgentEvent::PromptCompleted { .. } => "prompt_completed",
            CodingAgentEvent::PromptFailed { .. } => "prompt_failed",
            CodingAgentEvent::PromptAborted { .. } => "prompt_aborted",
            CodingAgentEvent::Diagnostic { .. } => "diagnostic",
            CodingAgentEvent::CapabilityChanged => "capability_changed",
        }
    }
}
