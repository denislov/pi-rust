#![allow(dead_code)]

use futures::future::{BoxFuture, FutureExt};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use pi_agent_core::AgentEvent;
use pi_ai::types::{AssistantMessageEvent, ContentBlock};

use super::capability_snapshot::InstalledCapabilityGeneration;
use super::{
    CodingAgentEvent, CodingSessionError, ProfileId, ProfileKind,
    event::{ProductEvent, ProductEventSequence},
    manual_compaction_flow::ManualCompactionOutcome,
    plugin_load_flow::PluginLoadOutcome,
    prompt::{DelegationRequest, PromptTurnOutcome},
    self_healing_edit_flow::{
        SelfHealingEditObserver, SelfHealingEditOutcome, SelfHealingEditRepairAttempt,
    },
    session_service::FinalizedSessionWrite,
};

const EVENT_CHANNEL_CAPACITY: usize = 128;
const EVENT_RETAINED_CAPACITY: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct EventService {
    sender: broadcast::Sender<CodingAgentEvent>,
    product_sender: broadcast::Sender<ProductEvent>,
    publication_state: Arc<Mutex<EventPublicationState>>,
    channel_capacity: usize,
    retained_capacity: usize,
}

#[derive(Debug)]
struct EventPublicationState {
    next_sequence: u64,
    retained_product_events: VecDeque<ProductEvent>,
    dropped_before: Option<ProductEventSequence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventBackpressureStatus {
    pub(crate) channel_capacity: usize,
    pub(crate) retained_capacity: usize,
    pub(crate) oldest_retained_sequence: Option<ProductEventSequence>,
    pub(crate) current_sequence: ProductEventSequence,
    pub(crate) dropped_before: Option<ProductEventSequence>,
}

#[derive(Debug, Clone)]
pub(crate) struct SelfHealingEditEventObserver {
    event_service: EventService,
    operation_id: String,
}

impl SelfHealingEditEventObserver {
    pub(crate) fn new(event_service: EventService, operation_id: impl Into<String>) -> Self {
        Self {
            event_service,
            operation_id: operation_id.into(),
        }
    }
}

impl SelfHealingEditObserver for SelfHealingEditEventObserver {
    fn repair_attempted<'a>(
        &'a self,
        path: &'a str,
        repair: &'a SelfHealingEditRepairAttempt,
    ) -> BoxFuture<'a, ()> {
        async move {
            self.event_service.emit_self_healing_edit_repair_attempted(
                self.operation_id.clone(),
                path,
                repair,
            );
        }
        .boxed()
    }
}

impl EventService {
    pub(crate) fn new() -> Self {
        Self::with_event_capacities(EVENT_CHANNEL_CAPACITY, EVENT_RETAINED_CAPACITY)
    }

    fn with_event_capacities(channel_capacity: usize, retained_capacity: usize) -> Self {
        let channel_capacity = channel_capacity.max(1);
        let (sender, _) = broadcast::channel(channel_capacity);
        let (product_sender, _) = broadcast::channel(channel_capacity);
        Self {
            sender,
            product_sender,
            publication_state: Arc::new(Mutex::new(EventPublicationState {
                next_sequence: 1,
                retained_product_events: VecDeque::with_capacity(retained_capacity),
                dropped_before: None,
            })),
            channel_capacity,
            retained_capacity,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_event_capacity_for_tests(capacity: usize) -> Self {
        Self::with_event_capacities(capacity, capacity)
    }

    pub(crate) fn current_product_sequence(&self) -> ProductEventSequence {
        let state = self.publication_state.lock().unwrap();
        ProductEventSequence::new(state.next_sequence.saturating_sub(1))
    }

    pub(crate) fn backpressure_status(&self) -> EventBackpressureStatus {
        let state = self.publication_state.lock().unwrap();
        EventBackpressureStatus {
            channel_capacity: self.channel_capacity,
            retained_capacity: self.retained_capacity,
            oldest_retained_sequence: state
                .retained_product_events
                .front()
                .map(ProductEvent::sequence),
            current_sequence: ProductEventSequence::new(state.next_sequence.saturating_sub(1)),
            dropped_before: state.dropped_before,
        }
    }

    pub(crate) fn product_events_after(
        &self,
        cursor: ProductEventSequence,
    ) -> Result<Vec<ProductEvent>, CodingSessionError> {
        let state = self.publication_state.lock().unwrap();
        let Some(oldest) = state
            .retained_product_events
            .front()
            .map(ProductEvent::sequence)
        else {
            return Ok(Vec::new());
        };
        if cursor < oldest && cursor != ProductEventSequence::default() {
            return Err(CodingSessionError::EventStreamGap {
                requested_after: cursor.get(),
                oldest_available: oldest.get(),
            });
        }
        Ok(state
            .retained_product_events
            .iter()
            .filter(|event| event.sequence() > cursor)
            .cloned()
            .collect())
    }

    fn retain_product_event(&self, state: &mut EventPublicationState, event: ProductEvent) {
        if self.retained_capacity == 0 {
            state.dropped_before = Some(event.sequence().next());
            return;
        }
        let dropped = state.retained_product_events.len() == self.retained_capacity;
        if state.retained_product_events.len() == self.retained_capacity {
            state.retained_product_events.pop_front();
        }
        state.retained_product_events.push_back(event);
        if dropped {
            state.dropped_before = state
                .retained_product_events
                .front()
                .map(ProductEvent::sequence);
        }
    }

    pub(crate) fn emit(&self, event: CodingAgentEvent) -> ProductEvent {
        let mut state = self.publication_state.lock().unwrap();
        let sequence = ProductEventSequence::new(state.next_sequence);
        state.next_sequence += 1;
        let product_event = ProductEvent::from_compat_event(sequence, event);
        self.retain_product_event(&mut state, product_event.clone());
        let _ = self.product_sender.send(product_event.clone());
        let _ = self
            .sender
            .send(product_event.compatibility_event().clone());
        product_event
    }

    pub(crate) fn emit_agent_event(
        &self,
        context: &AgentEventMappingContext,
        event: &AgentEvent,
    ) -> Vec<CodingAgentEvent> {
        let events = map_agent_event(context, event);
        for event in &events {
            self.emit(event.clone());
        }
        events
    }

    pub(crate) fn emit_session_opened(&self, session_id: impl Into<String>) {
        self.emit(CodingAgentEvent::SessionOpened {
            session_id: session_id.into(),
        });
    }

    pub(crate) fn emit_diagnostic(
        &self,
        operation_id: Option<impl Into<String>>,
        message: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::Diagnostic {
            operation_id: operation_id.map(Into::into),
            message: message.into(),
        });
    }

    pub(crate) fn emit_default_agent_profile_changed(&self, profile_id: impl Into<ProfileId>) {
        self.emit(CodingAgentEvent::DefaultAgentProfileChanged {
            profile_id: profile_id.into(),
        });
    }

    pub(crate) fn emit_session_compaction_completed(
        &self,
        operation_id: impl Into<String>,
        turn_id: impl Into<String>,
        outcome: &ManualCompactionOutcome,
    ) {
        self.emit(CodingAgentEvent::SessionCompactionCompleted {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
            summary: outcome.summary.clone(),
            first_kept_message_id: outcome.first_kept_message_id.clone(),
            tokens_before: outcome.tokens_before,
        });
    }

    pub(crate) fn emit_plugin_load_outcome(&self, outcome: &PluginLoadOutcome) {
        for diagnostic in &outcome.diagnostics {
            self.emit_diagnostic(None::<String>, diagnostic.message.clone());
        }
    }

    pub(crate) fn emit_capability_changed(&self, installed: InstalledCapabilityGeneration) {
        self.emit(CodingAgentEvent::CapabilityChanged {
            generation: installed.generation.get(),
            revocation: installed.revocation,
        });
    }

    pub(crate) fn emit_prompt_started(
        &self,
        operation_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::PromptStarted {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
        });
    }

    pub(crate) fn emit_events_before_prompt_outcome(&self, events: &[CodingAgentEvent]) {
        for event in events {
            if is_prompt_outcome_event(event) {
                continue;
            }
            self.emit(event.clone());
        }
    }

    pub(crate) fn prompt_completed_event(
        operation_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> CodingAgentEvent {
        CodingAgentEvent::PromptCompleted {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
        }
    }

    pub(crate) fn session_write_pending_event(operation_id: impl Into<String>) -> CodingAgentEvent {
        CodingAgentEvent::SessionWritePending {
            operation_id: operation_id.into(),
        }
    }

    pub(crate) fn session_write_committed_event(
        operation_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> CodingAgentEvent {
        CodingAgentEvent::SessionWriteCommitted {
            operation_id: operation_id.into(),
            session_id: session_id.into(),
        }
    }

    pub(crate) fn session_write_skipped_event(
        operation_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> CodingAgentEvent {
        CodingAgentEvent::SessionWriteSkipped {
            operation_id: operation_id.into(),
            reason: reason.into(),
        }
    }

    pub(crate) fn emit_prompt_completed(
        &self,
        operation_id: impl Into<String>,
        turn_id: impl Into<String>,
    ) {
        self.emit(Self::prompt_completed_event(operation_id, turn_id));
    }

    pub(crate) fn emit_prompt_aborted(
        &self,
        operation_id: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::PromptAborted {
            operation_id: operation_id.into(),
            reason: reason.into(),
        });
    }

    pub(crate) fn emit_prompt_failed(
        &self,
        operation_id: impl Into<String>,
        error: CodingSessionError,
    ) {
        self.emit(CodingAgentEvent::PromptFailed {
            operation_id: operation_id.into(),
            error,
        });
    }

    pub(crate) fn emit_session_write_events(&self, finalized: &FinalizedSessionWrite) {
        for event in &finalized.events {
            self.emit(event.clone());
        }
    }

    pub(crate) fn emit_session_write_pending(&self, finalized: &FinalizedSessionWrite) {
        for event in &finalized.events {
            if matches!(event, CodingAgentEvent::SessionWritePending { .. }) {
                self.emit(event.clone());
            }
        }
    }

    pub(crate) fn emit_session_write_committed(&self, finalized: &FinalizedSessionWrite) {
        for event in &finalized.events {
            if matches!(
                event,
                CodingAgentEvent::SessionWriteCommitted { .. }
                    | CodingAgentEvent::SessionWriteSkipped { .. }
            ) {
                self.emit(event.clone());
            }
        }
    }

    pub(crate) fn emit_prompt_outcome(&self, outcome: &PromptTurnOutcome) {
        self.emit_prompt_diagnostics(outcome);
        match outcome {
            PromptTurnOutcome::Success {
                operation_id,
                turn_id,
                ..
            } => self.emit_prompt_completed(operation_id.clone(), turn_id.clone()),
            PromptTurnOutcome::Aborted {
                operation_id,
                reason,
                ..
            } => self.emit_prompt_aborted(operation_id.clone(), reason.clone()),
            PromptTurnOutcome::Failed {
                operation_id,
                error,
                ..
            } => self.emit_prompt_failed(operation_id.clone(), error.clone()),
        }
    }

    pub(crate) fn emit_agent_invocation_started(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        profile_id: impl Into<ProfileId>,
        task: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentInvocationStarted {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            profile_id: profile_id.into(),
            task: task.into(),
        });
    }

    pub(crate) fn emit_agent_invocation_completed(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        profile_id: impl Into<ProfileId>,
        final_text: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentInvocationCompleted {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            profile_id: profile_id.into(),
            final_text: final_text.into(),
        });
    }

    pub(crate) fn emit_agent_invocation_failed(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        profile_id: impl Into<ProfileId>,
        error: CodingSessionError,
    ) {
        self.emit(CodingAgentEvent::AgentInvocationFailed {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            profile_id: profile_id.into(),
            error,
        });
    }

    pub(crate) fn emit_agent_invocation_aborted(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        profile_id: impl Into<ProfileId>,
        reason: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentInvocationAborted {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            profile_id: profile_id.into(),
            reason: reason.into(),
        });
    }

    pub(crate) fn emit_agent_team_started(
        &self,
        operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        task: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentTeamStarted {
            operation_id: operation_id.into(),
            team_id: team_id.into(),
            task: task.into(),
        });
    }

    pub(crate) fn emit_agent_team_member_started(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        profile_id: impl Into<ProfileId>,
        task: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentTeamMemberStarted {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            team_id: team_id.into(),
            profile_id: profile_id.into(),
            task: task.into(),
        });
    }

    pub(crate) fn emit_agent_team_member_completed(
        &self,
        operation_id: impl Into<String>,
        child_operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        profile_id: impl Into<ProfileId>,
        final_text: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentTeamMemberCompleted {
            operation_id: operation_id.into(),
            child_operation_id: child_operation_id.into(),
            team_id: team_id.into(),
            profile_id: profile_id.into(),
            final_text: final_text.into(),
        });
    }

    pub(crate) fn emit_agent_team_completed(
        &self,
        operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        final_text: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentTeamCompleted {
            operation_id: operation_id.into(),
            team_id: team_id.into(),
            final_text: final_text.into(),
        });
    }

    pub(crate) fn emit_agent_team_failed(
        &self,
        operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        error: CodingSessionError,
    ) {
        self.emit(CodingAgentEvent::AgentTeamFailed {
            operation_id: operation_id.into(),
            team_id: team_id.into(),
            error,
        });
    }

    pub(crate) fn emit_agent_team_aborted(
        &self,
        operation_id: impl Into<String>,
        team_id: impl Into<ProfileId>,
        reason: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::AgentTeamAborted {
            operation_id: operation_id.into(),
            team_id: team_id.into(),
            reason: reason.into(),
        });
    }

    fn emit_prompt_diagnostics(&self, outcome: &PromptTurnOutcome) {
        let (operation_id, diagnostics) = match outcome {
            PromptTurnOutcome::Success {
                operation_id,
                diagnostics,
                ..
            }
            | PromptTurnOutcome::Failed {
                operation_id,
                diagnostics,
                ..
            } => (operation_id, diagnostics),
            PromptTurnOutcome::Aborted { .. } => return,
        };
        for diagnostic in diagnostics {
            self.emit_diagnostic(Some(operation_id.clone()), diagnostic.message.clone());
        }
    }

    pub(crate) fn emit_delegation_approved(&self, request: &DelegationRequest) {
        self.emit(CodingAgentEvent::DelegationApproved {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
        });
    }

    pub(crate) fn emit_delegation_rejected(&self, request: &DelegationRequest, reason: &str) {
        self.emit(CodingAgentEvent::DelegationRejected {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
            reason: reason.to_owned(),
        });
    }

    pub(crate) fn emit_delegation_confirmation_required(
        &self,
        request: &DelegationRequest,
        reason: &str,
    ) {
        self.emit(CodingAgentEvent::DelegationConfirmationRequired {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
            reason: reason.to_owned(),
        });
    }

    pub(crate) fn emit_delegation_started(
        &self,
        request: &DelegationRequest,
        child_operation_id: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::DelegationStarted {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
            child_operation_id: child_operation_id.into(),
        });
    }

    pub(crate) fn emit_delegation_completed(
        &self,
        request: &DelegationRequest,
        child_operation_id: impl Into<String>,
        final_text: impl Into<String>,
    ) {
        self.emit(CodingAgentEvent::DelegationCompleted {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
            child_operation_id: child_operation_id.into(),
            final_text: final_text.into(),
        });
    }

    pub(crate) fn emit_delegation_failed(
        &self,
        request: &DelegationRequest,
        child_operation_id: impl Into<String>,
        error: CodingSessionError,
    ) {
        self.emit(CodingAgentEvent::DelegationFailed {
            operation_id: request.operation_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            requesting_profile_id: request.requesting_profile_id.clone(),
            target_kind: request.target_kind,
            target_id: request.target_id.clone(),
            task: request.task.clone(),
            child_operation_id: child_operation_id.into(),
            error,
        });
    }

    pub(crate) fn emit_self_healing_edit_started(
        &self,
        operation_id: impl Into<String>,
        path: impl Into<String>,
        replacements: usize,
    ) {
        self.emit(CodingAgentEvent::SelfHealingEditStarted {
            operation_id: operation_id.into(),
            path: path.into(),
            replacements,
        });
    }

    pub(crate) fn emit_self_healing_edit_repair_attempted(
        &self,
        operation_id: impl Into<String>,
        path: impl Into<String>,
        repair: &SelfHealingEditRepairAttempt,
    ) {
        self.emit(CodingAgentEvent::SelfHealingEditRepairAttempted {
            operation_id: operation_id.into(),
            path: path.into(),
            attempt: repair.attempt,
            replacements: repair.replacements.clone(),
            diagnostics: repair.diagnostics.clone(),
            check_output: repair.check_output.clone(),
        });
    }

    pub(crate) fn emit_self_healing_edit_completed(
        &self,
        operation_id: impl Into<String>,
        outcome: &SelfHealingEditOutcome,
    ) {
        self.emit(CodingAgentEvent::SelfHealingEditCompleted {
            operation_id: operation_id.into(),
            path: outcome.path.clone(),
            attempts: outcome.attempts,
            first_changed_line: outcome.first_changed_line,
            check_output: outcome.check_output.clone(),
        });
    }

    pub(crate) fn emit_self_healing_edit_failed(
        &self,
        operation_id: impl Into<String>,
        path: impl Into<String>,
        error: &CodingSessionError,
    ) {
        self.emit(CodingAgentEvent::SelfHealingEditFailed {
            operation_id: operation_id.into(),
            path: path.into(),
            error: error.clone(),
        });
    }

    pub(crate) fn emit_operation_recovered(
        &self,
        operation_id: impl Into<String>,
        recovery_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> ProductEvent {
        self.emit(CodingAgentEvent::OperationRecovered {
            operation_id: operation_id.into(),
            recovery_id: recovery_id.into(),
            reason: reason.into(),
        })
    }

    #[deprecated(note = "use ProductEventReceiver instead")]
    pub(crate) fn subscribe(&self) -> CodingAgentEventReceiver {
        CodingAgentEventReceiver {
            inner: self.sender.subscribe(),
        }
    }

    pub(crate) fn subscribe_product_events(&self) -> ProductEventReceiver {
        ProductEventReceiver {
            inner: self.product_sender.subscribe(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentEventMappingContext {
    operation_id: String,
    turn_id: String,
    assistant_message_id: Option<String>,
}

impl AgentEventMappingContext {
    pub(crate) fn new(operation_id: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            turn_id: turn_id.into(),
            assistant_message_id: None,
        }
    }

    pub(crate) fn with_assistant_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.assistant_message_id = Some(message_id.into());
        self
    }
}

pub(crate) fn map_agent_event(
    context: &AgentEventMappingContext,
    event: &AgentEvent,
) -> Vec<CodingAgentEvent> {
    match event {
        AgentEvent::TurnStart { turn } => vec![CodingAgentEvent::AgentTurnStarted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            agent_turn: *turn,
        }],
        AgentEvent::BeforeProviderRequest { request } => {
            vec![CodingAgentEvent::ProviderRequestStarted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                provider: request.model.provider.clone(),
                model: request.model.id.clone(),
            }]
        }
        AgentEvent::LlmEvent(event) => map_assistant_event(context, event),
        AgentEvent::ToolCallStart {
            tool_call_id,
            tool_name,
            arguments,
        } => vec![CodingAgentEvent::ToolCallStarted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            arguments_json: arguments.to_string(),
        }],
        AgentEvent::ToolCallUpdate {
            tool_call_id,
            tool_name,
            update,
        } => vec![CodingAgentEvent::ToolCallUpdated {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            message: content_blocks_text(&update.content),
        }],
        AgentEvent::ToolCallEnd {
            tool_call_id,
            tool_name,
            result,
        } if result.is_error => vec![CodingAgentEvent::ToolCallFailed {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.clone(),
            name: tool_name.clone(),
            message: content_blocks_text(&result.content),
        }],
        AgentEvent::ToolCallEnd {
            tool_call_id,
            tool_name,
            result,
        } => {
            let summary = content_blocks_text(&result.content);
            let mut events = vec![CodingAgentEvent::ToolCallCompleted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                tool_call_id: tool_call_id.clone(),
                name: tool_name.clone(),
                summary: summary.clone(),
            }];
            if let Some(event) =
                map_delegation_tool_event(context, tool_call_id, tool_name, &summary)
            {
                events.push(event);
            }
            events
        }
        AgentEvent::AgentDone { .. } => Vec::new(),
        AgentEvent::AgentError { error } => vec![CodingAgentEvent::PromptFailed {
            operation_id: context.operation_id.clone(),
            error: CodingSessionError::Provider {
                message: error.clone(),
            },
        }],
        AgentEvent::SessionCompacted {
            summary,
            first_kept_message_id,
            tokens_before,
            details: _,
        } => vec![CodingAgentEvent::RuntimeCompactionCompleted {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            summary: summary.clone(),
            first_kept_message_id: first_kept_message_id.clone(),
            tokens_before: *tokens_before,
        }],
    }
}

fn map_assistant_event(
    context: &AgentEventMappingContext,
    event: &AssistantMessageEvent,
) -> Vec<CodingAgentEvent> {
    match event {
        AssistantMessageEvent::Start { .. }
        | AssistantMessageEvent::TextStart { .. }
        | AssistantMessageEvent::ThinkingStart { .. } => {
            vec![CodingAgentEvent::AssistantMessageStarted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
            }]
        }
        AssistantMessageEvent::TextDelta { delta, .. } => {
            vec![CodingAgentEvent::AssistantMessageDelta {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
                text: delta.clone(),
            }]
        }
        AssistantMessageEvent::ThinkingDelta { delta, .. } => {
            vec![CodingAgentEvent::AssistantThinkingDelta {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
                text: delta.clone(),
            }]
        }
        AssistantMessageEvent::Error { message, .. } => vec![CodingAgentEvent::PromptFailed {
            operation_id: context.operation_id.clone(),
            error: CodingSessionError::Provider {
                message: message
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "assistant stream failed".into()),
            },
        }],
        AssistantMessageEvent::Done { message, .. } => {
            vec![CodingAgentEvent::AssistantMessageCompleted {
                operation_id: context.operation_id.clone(),
                turn_id: context.turn_id.clone(),
                message_id: context.assistant_message_id.clone(),
                final_text: assistant_text(&message.content),
                usage: message.usage.clone(),
            }]
        }
        AssistantMessageEvent::TextEnd { .. }
        | AssistantMessageEvent::ThinkingEnd { .. }
        | AssistantMessageEvent::ToolcallStart { .. }
        | AssistantMessageEvent::ToolcallDelta { .. }
        | AssistantMessageEvent::ToolcallEnd { .. } => Vec::new(),
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

fn content_blocks_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text, .. } => text.clone(),
            ContentBlock::Thinking { thinking, .. } => thinking.clone(),
            ContentBlock::Image { mime_type, .. } => format!("[image:{mime_type}]"),
            ContentBlock::ToolCall { name, .. } => format!("[tool_call:{name}]"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assistant_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn map_delegation_tool_event(
    context: &AgentEventMappingContext,
    tool_call_id: &str,
    tool_name: &str,
    summary: &str,
) -> Option<CodingAgentEvent> {
    if !matches!(tool_name, "delegate_agent" | "delegate_team") {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(summary).ok()?;
    let status = value.get("status")?.as_str()?;
    let target_kind = parse_delegation_target_kind(value.get("target_kind")?.as_str()?)?;
    let target_id = ProfileId::new(value.get("target_id")?.as_str()?.to_owned()).ok()?;
    let requesting_profile_id =
        ProfileId::new(value.get("requesting_profile_id")?.as_str()?.to_owned()).ok()?;
    let task = value.get("task")?.as_str()?.to_owned();

    match status {
        "requested" => Some(CodingAgentEvent::DelegationRequested {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.to_owned(),
            requesting_profile_id,
            target_kind,
            target_id,
            task,
        }),
        "rejected" => Some(CodingAgentEvent::DelegationRejected {
            operation_id: context.operation_id.clone(),
            turn_id: context.turn_id.clone(),
            tool_call_id: tool_call_id.to_owned(),
            requesting_profile_id,
            target_kind,
            target_id,
            task,
            reason: value
                .get("message")
                .and_then(|message| message.as_str())
                .unwrap_or("delegation rejected")
                .to_owned(),
        }),
        _ => None,
    }
}

fn parse_delegation_target_kind(kind: &str) -> Option<ProfileKind> {
    match kind {
        "agent" => Some(ProfileKind::Agent),
        "team" => Some(ProfileKind::Team),
        _ => None,
    }
}

#[derive(Debug)]
pub(crate) struct ProductEventReceiver {
    inner: broadcast::Receiver<ProductEvent>,
}

impl ProductEventReceiver {
    pub(crate) async fn recv(&mut self) -> Result<ProductEvent, CodingSessionError> {
        self.inner.recv().await.map_err(map_recv_error)
    }

    pub(crate) fn try_recv(&mut self) -> Result<Option<ProductEvent>, CodingSessionError> {
        match self.inner.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => Err(CodingSessionError::Cancelled),
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                Err(CodingSessionError::EventStreamLag { skipped })
            }
        }
    }
}

#[derive(Debug)]
pub struct CodingAgentEventReceiver {
    inner: broadcast::Receiver<CodingAgentEvent>,
}

impl CodingAgentEventReceiver {
    pub async fn recv(&mut self) -> Result<CodingAgentEvent, CodingSessionError> {
        self.inner.recv().await.map_err(map_recv_error)
    }

    pub fn try_recv(&mut self) -> Result<Option<CodingAgentEvent>, CodingSessionError> {
        match self.inner.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => Err(CodingSessionError::Cancelled),
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                Err(CodingSessionError::EventStreamLag { skipped })
            }
        }
    }
}

fn map_recv_error(error: broadcast::error::RecvError) -> CodingSessionError {
    match error {
        broadcast::error::RecvError::Closed => CodingSessionError::Cancelled,
        broadcast::error::RecvError::Lagged(skipped) => {
            CodingSessionError::EventStreamLag { skipped }
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use pi_agent_core::{AgentEvent, AgentToolOutput, AgentToolResult, ProviderRequestSnapshot};
    use pi_ai::types::{
        AssistantMessage, AssistantMessageEvent, ContentBlock, Context, Model, ModelCost,
        ModelInput, StopReason, StreamOptions, Usage,
    };
    use serde_json::json;

    use super::super::event::{
        ProductEventDurability, ProductEventFamily, ProductEventKind, ProductEventSequence,
        ProductEventTerminalStatus, WorkflowProductEventKind,
    };
    use super::*;

    fn mapping_context() -> AgentEventMappingContext {
        AgentEventMappingContext::new("op_1", "turn_1").with_assistant_message_id("msg_1")
    }

    fn model() -> Model {
        Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: "messages".into(),
            provider: "test-provider".into(),
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

    fn assistant_message(text: &str) -> AssistantMessage {
        let mut message = AssistantMessage::empty("messages", "test-model");
        message.content.push(ContentBlock::Text {
            text: text.into(),
            text_signature: None,
        });
        message
    }

    #[test]
    fn event_service_wraps_emitted_events_with_sequence_and_preserves_compatibility_receiver() {
        let service = EventService::new();
        let mut receiver = service.subscribe();

        let first = service.emit(CodingAgentEvent::PromptCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
        });
        let second = service.clone().emit(CodingAgentEvent::CapabilityChanged {
            generation: 1,
            revocation:
                crate::coding_session::capability_snapshot::CapabilityRevocationPolicy::FutureOnly,
        });

        assert_eq!(first.sequence(), ProductEventSequence(1));
        assert_eq!(first.family(), ProductEventFamily::Workflow);
        assert_eq!(first.operation_id(), Some("op_1"));
        assert_eq!(
            first.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );
        assert_eq!(first.durability(), &ProductEventDurability::LiveOnly);
        assert!(matches!(
            first.compatibility_event(),
            CodingAgentEvent::PromptCompleted { .. }
        ));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptCompleted { .. })
        ));

        assert_eq!(second.sequence(), ProductEventSequence(2));
        assert_eq!(second.family(), ProductEventFamily::Capability);
        assert_eq!(second.operation_id(), None);
        assert_eq!(second.terminal_status(), None);
        assert_eq!(second.durability(), &ProductEventDurability::LiveOnly);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::CapabilityChanged { .. })
        ));
    }

    #[test]
    fn event_service_publishes_internal_product_events_alongside_compatibility_stream() {
        let service = EventService::new();
        let mut product_receiver = service.subscribe_product_events();
        let mut compatibility_receiver = service.subscribe();

        let emitted = service.emit(CodingAgentEvent::PromptCompleted {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
        });

        let product_event = product_receiver
            .try_recv()
            .unwrap()
            .expect("product event is published");
        assert_eq!(product_event, emitted);
        assert_eq!(product_event.sequence(), ProductEventSequence(1));
        assert_eq!(
            product_event.kind(),
            ProductEventKind::Workflow(WorkflowProductEventKind::PromptCompleted)
        );
        assert_eq!(product_event.operation_id(), Some("op_1"));
        assert_eq!(
            product_event.terminal_status(),
            Some(ProductEventTerminalStatus::Completed)
        );
        assert!(matches!(
            compatibility_receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptCompleted { .. })
        ));
        assert_eq!(product_receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn retained_product_events_can_resume_after_cursor() {
        let service = EventService::new();
        service.emit(CodingAgentEvent::SessionOpened {
            session_id: "sess_retained".into(),
        });
        service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "ready".into(),
        });

        let retained = service
            .product_events_after(ProductEventSequence::new(1))
            .unwrap();

        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].sequence(), ProductEventSequence::new(2));
    }

    #[test]
    fn retained_product_events_report_gap_before_oldest_sequence() {
        let service = EventService::with_event_capacity_for_tests(2);
        for index in 0..4 {
            service.emit(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {index}"),
            });
        }

        let error = service
            .product_events_after(ProductEventSequence::new(1))
            .unwrap_err();

        assert_eq!(error.code(), "event_stream_gap");
        assert!(error.to_string().contains("fresh UI snapshot"));
    }

    #[test]
    fn zero_retained_capacity_keeps_replay_window_empty() {
        let service = EventService::with_event_capacity_for_tests(0);
        service.emit(CodingAgentEvent::Diagnostic {
            operation_id: None,
            message: "ready".into(),
        });

        let retained = service
            .product_events_after(ProductEventSequence::default())
            .unwrap();

        assert!(retained.is_empty());
        assert_eq!(
            service.current_product_sequence(),
            ProductEventSequence::new(1)
        );
    }

    #[test]
    fn concurrent_emitters_retain_events_in_sequence_order() {
        const EMITTERS: usize = 256;

        let service = EventService::with_event_capacity_for_tests(EMITTERS);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(EMITTERS));
        let handles = (0..EMITTERS)
            .map(|index| {
                let service = service.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    service.emit(CodingAgentEvent::Diagnostic {
                        operation_id: None,
                        message: format!("event {index}"),
                    });
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        let retained = service
            .product_events_after(ProductEventSequence::default())
            .unwrap();
        let sequences = retained
            .iter()
            .map(|event| event.sequence().get())
            .collect::<Vec<_>>();

        assert_eq!(sequences, (1..=EMITTERS as u64).collect::<Vec<_>>());
        assert_eq!(
            service.current_product_sequence(),
            ProductEventSequence::new(EMITTERS as u64)
        );
    }

    #[test]
    fn maps_turn_and_provider_request_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(&context, &AgentEvent::TurnStart { turn: 3 }),
            vec![CodingAgentEvent::AgentTurnStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                agent_turn: 3,
            }]
        );

        let event = AgentEvent::BeforeProviderRequest {
            request: ProviderRequestSnapshot {
                model: model(),
                context: Context {
                    system_prompt: None,
                    messages: Vec::new(),
                    tools: None,
                },
                stream_options: StreamOptions::default(),
            },
        };

        assert_eq!(
            map_agent_event(&context, &event),
            vec![CodingAgentEvent::ProviderRequestStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                provider: "test-provider".into(),
                model: "test-model".into(),
            }]
        );
    }

    #[test]
    fn maps_assistant_stream_and_done_events() {
        let context = mapping_context();
        let partial = AssistantMessage::empty("messages", "test-model");

        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::TextStart {
                    content_index: 0,
                    partial: partial.clone(),
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "hi".into(),
                    partial,
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hi".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::ThinkingStart {
                    content_index: 0,
                    partial: AssistantMessage::empty("messages", "test-model"),
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::ThinkingDelta {
                    content_index: 0,
                    delta: "thinking".into(),
                    partial: AssistantMessage::empty("messages", "test-model"),
                }),
            ),
            vec![CodingAgentEvent::AssistantThinkingDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "thinking".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: assistant_message("done"),
                }),
            ),
            vec![CodingAgentEvent::AssistantMessageCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                final_text: "done".into(),
                usage: Usage::default(),
            }]
        );
        // AgentDone no longer emits AssistantMessageCompleted — that is now
        // handled by AssistantMessageEvent::Done, which fires for every
        // completed assistant message (including intermediate ToolUse ones).
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::AgentDone {
                    message: assistant_message("done"),
                }
            ),
            Vec::new(),
        );
    }

    #[test]
    fn maps_tool_lifecycle_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallStart {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    arguments: json!({"path": "Cargo.toml"}),
                },
            ),
            vec![CodingAgentEvent::ToolCallStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                arguments_json: r#"{"path":"Cargo.toml"}"#.into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallUpdate {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    update: AgentToolOutput::new(vec![ContentBlock::Text {
                        text: "running".into(),
                        text_signature: None,
                    }]),
                },
            ),
            vec![CodingAgentEvent::ToolCallUpdated {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                message: "running".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallEnd {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    result: AgentToolResult::ok(vec![ContentBlock::Text {
                        text: "ok".into(),
                        text_signature: None,
                    }]),
                },
            ),
            vec![CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            }]
        );
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::ToolCallEnd {
                    tool_call_id: "tool_1".into(),
                    tool_name: "read".into(),
                    result: AgentToolResult::error("missing"),
                },
            ),
            vec![CodingAgentEvent::ToolCallFailed {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                message: "missing".into(),
            }]
        );
    }

    #[test]
    fn maps_delegation_tool_result_events() {
        let context = mapping_context();
        let requested = map_agent_event(
            &context,
            &AgentEvent::ToolCallEnd {
                tool_call_id: "tool_delegate_1".into(),
                tool_name: "delegate_agent".into(),
                result: AgentToolResult::ok(vec![ContentBlock::Text {
                    text: serde_json::json!({
                        "status": "requested",
                        "target_kind": "agent",
                        "target_id": "coder",
                        "task": "implement it",
                        "requesting_profile_id": "planner"
                    })
                    .to_string(),
                    text_signature: None,
                }]),
            },
        );
        assert_eq!(
            requested,
            vec![
                CodingAgentEvent::ToolCallCompleted {
                    operation_id: "op_1".into(),
                    turn_id: "turn_1".into(),
                    tool_call_id: "tool_delegate_1".into(),
                    name: "delegate_agent".into(),
                    summary: serde_json::json!({
                        "status": "requested",
                        "target_kind": "agent",
                        "target_id": "coder",
                        "task": "implement it",
                        "requesting_profile_id": "planner"
                    })
                    .to_string(),
                },
                CodingAgentEvent::DelegationRequested {
                    operation_id: "op_1".into(),
                    turn_id: "turn_1".into(),
                    tool_call_id: "tool_delegate_1".into(),
                    requesting_profile_id: ProfileId::from("planner"),
                    target_kind: ProfileKind::Agent,
                    target_id: ProfileId::from("coder"),
                    task: "implement it".into(),
                },
            ]
        );

        let rejected = map_agent_event(
            &context,
            &AgentEvent::ToolCallEnd {
                tool_call_id: "tool_delegate_2".into(),
                tool_name: "delegate_team".into(),
                result: AgentToolResult::ok(vec![ContentBlock::Text {
                    text: serde_json::json!({
                        "status": "rejected",
                        "target_kind": "team",
                        "target_id": "implementation",
                        "task": "build it",
                        "requesting_profile_id": "planner",
                        "message": "delegation policy max_depth is 0"
                    })
                    .to_string(),
                    text_signature: None,
                }]),
            },
        );
        assert!(matches!(
            rejected.as_slice(),
            [
                CodingAgentEvent::ToolCallCompleted { .. },
                CodingAgentEvent::DelegationRejected {
                    operation_id,
                    turn_id,
                    tool_call_id,
                    requesting_profile_id,
                    target_kind: ProfileKind::Team,
                    target_id,
                    task,
                    reason,
                },
            ] if operation_id == "op_1"
                && turn_id == "turn_1"
                && tool_call_id == "tool_delegate_2"
                && requesting_profile_id == &ProfileId::from("planner")
                && target_id == &ProfileId::from("implementation")
                && task == "build it"
                && reason.contains("max_depth")
        ));
    }

    #[test]
    fn maps_error_and_compaction_events() {
        let context = mapping_context();
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::AgentError {
                    error: "provider failed".into(),
                },
            ),
            vec![CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "provider failed".into(),
                },
            }]
        );

        let mut message = AssistantMessage::empty("messages", "test-model");
        message.error_message = Some("stream failed".into());
        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::LlmEvent(AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    message,
                }),
            ),
            vec![CodingAgentEvent::PromptFailed {
                operation_id: "op_1".into(),
                error: CodingSessionError::Provider {
                    message: "stream failed".into(),
                },
            }]
        );

        assert_eq!(
            map_agent_event(
                &context,
                &AgentEvent::SessionCompacted {
                    summary: "short".into(),
                    first_kept_message_id: "msg_kept".into(),
                    tokens_before: 42,
                    details: None,
                },
            ),
            vec![CodingAgentEvent::RuntimeCompactionCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                summary: "short".into(),
                first_kept_message_id: "msg_kept".into(),
                tokens_before: 42,
            }]
        );
    }

    #[tokio::test]
    async fn event_service_emits_mapped_agent_events() {
        let service = EventService::new();
        let mut receiver = service.subscribe();
        let context = mapping_context();

        let mapped = service.emit_agent_event(
            &context,
            &AgentEvent::LlmEvent(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "hi".into(),
                partial: AssistantMessage::empty("messages", "test-model"),
            }),
        );

        assert_eq!(mapped.len(), 1);
        assert_eq!(
            receiver.recv().await.unwrap(),
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hi".into(),
            }
        );
    }

    #[test]
    fn event_service_emits_plugin_load_outcome_events() {
        let service = EventService::new();
        let mut receiver = service.subscribe();
        let outcome = crate::coding_session::plugin_load_flow::PluginLoadOutcome {
            loaded_plugin_ids: vec!["lua".into()],
            diagnostics: vec![crate::coding_session::plugin_service::PluginDiagnostic {
                plugin_id: Some("lua".into()),
                message: "loaded with warning".into(),
            }],
            capabilities: crate::plugins::PluginCapabilities::new(),
            capability_changed: true,
        };

        service.emit_plugin_load_outcome(&outcome);

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "loaded with warning".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_capability_changed_with_generation_and_revocation() {
        use crate::coding_session::capability_snapshot::{
            CapabilityGeneration, CapabilityRevocationPolicy, InstalledCapabilityGeneration,
        };

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let installed = InstalledCapabilityGeneration {
            generation: CapabilityGeneration::new(3),
            revocation: CapabilityRevocationPolicy::FutureOnly,
        };

        service.emit_capability_changed(installed);

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::CapabilityChanged {
                generation: 3,
                revocation: CapabilityRevocationPolicy::FutureOnly,
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_session_opened_and_diagnostics() {
        let service = EventService::new();
        let mut receiver = service.subscribe();

        service.emit_session_opened("sess_1");
        service.emit_diagnostic(Some("op_1"), "profile warning");
        service.emit_diagnostic(None::<String>, "global warning");

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionOpened {
                session_id: "sess_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: Some("op_1".into()),
                message: "profile warning".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "global warning".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_owner_status_events() {
        use crate::coding_session::manual_compaction_flow::ManualCompactionOutcome;

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let compaction = ManualCompactionOutcome {
            summary: "short summary".into(),
            first_kept_message_id: "msg_2".into(),
            tokens_before: 42,
            final_message: AssistantMessage::empty("messages", "test-model"),
        };

        service.emit_default_agent_profile_changed(ProfileId::from("reviewer"));
        service.emit_session_compaction_completed("op_1", "turn_1", &compaction);

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DefaultAgentProfileChanged {
                profile_id: ProfileId::from("reviewer"),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionCompactionCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                summary: "short summary".into(),
                first_kept_message_id: "msg_2".into(),
                tokens_before: 42,
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_filters_prompt_outcomes_and_session_write_slices() {
        use crate::coding_session::session_service::FinalizedSessionWrite;

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let prompt_events = vec![
            CodingAgentEvent::PromptStarted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_prompt".into(),
            },
            CodingAgentEvent::PromptCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_prompt".into(),
            },
            CodingAgentEvent::PromptFailed {
                operation_id: "op_failed".into(),
                error: CodingSessionError::Cancelled,
            },
            CodingAgentEvent::PromptAborted {
                operation_id: "op_aborted".into(),
                reason: "cancelled".into(),
            },
            CodingAgentEvent::Diagnostic {
                operation_id: Some("op_prompt".into()),
                message: "warning".into(),
            },
        ];

        service.emit_events_before_prompt_outcome(&prompt_events);

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptStarted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_prompt".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: Some("op_prompt".into()),
                message: "warning".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);

        let finalized = FinalizedSessionWrite {
            events: vec![
                CodingAgentEvent::SessionWritePending {
                    operation_id: "op_write".into(),
                },
                CodingAgentEvent::Diagnostic {
                    operation_id: Some("op_write".into()),
                    message: "persisted warning".into(),
                },
                CodingAgentEvent::SessionWriteCommitted {
                    operation_id: "op_write".into(),
                    session_id: "sess_1".into(),
                },
                CodingAgentEvent::SessionWriteSkipped {
                    operation_id: "op_skip".into(),
                    reason: "disabled".into(),
                },
            ],
            session_id: Some("sess_1".into()),
            leaf_id: Some("leaf_1".into()),
        };

        service.emit_session_write_pending(&finalized);
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWritePending {
                operation_id: "op_write".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);

        service.emit_session_write_committed(&finalized);
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op_write".into(),
                session_id: "sess_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWriteSkipped {
                operation_id: "op_skip".into(),
                reason: "disabled".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);

        service.emit_session_write_events(&finalized);
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWritePending {
                operation_id: "op_write".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: Some("op_write".into()),
                message: "persisted warning".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWriteCommitted {
                operation_id: "op_write".into(),
                session_id: "sess_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SessionWriteSkipped {
                operation_id: "op_skip".into(),
                reason: "disabled".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_prompt_lifecycle_events() {
        use crate::coding_session::{CodingDiagnostic, PromptTurnOutcome};

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let failed_error = CodingSessionError::Provider {
            message: "provider failed".into(),
        };
        let direct_failed_error = CodingSessionError::Provider {
            message: "direct provider failed".into(),
        };

        service.emit_prompt_started("op_1", "turn_1");
        service.emit_prompt_outcome(&PromptTurnOutcome::Success {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            session_id: None,
            leaf_id: None,
            final_text: "done".into(),
            final_message: AssistantMessage::empty("messages", "test-model"),
            diagnostics: vec![CodingDiagnostic::warning("profile warning")],
        });
        service.emit_prompt_outcome(&PromptTurnOutcome::Failed {
            operation_id: "op_2".into(),
            turn_id: Some("turn_2".into()),
            error: failed_error.clone(),
            diagnostics: vec![CodingDiagnostic::error("provider diagnostic")],
        });
        service.emit_prompt_outcome(&PromptTurnOutcome::Aborted {
            operation_id: "op_3".into(),
            turn_id: Some("turn_3".into()),
            reason: "cancelled".into(),
            session_id: None,
        });
        service.emit_prompt_completed("op_direct_1", "turn_direct_1");
        service.emit_prompt_failed("op_direct_2", direct_failed_error.clone());
        service.emit_prompt_aborted("op_direct_3", "stopped");

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: Some("op_1".into()),
                message: "profile warning".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::Diagnostic {
                operation_id: Some("op_2".into()),
                message: "provider diagnostic".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptFailed {
                operation_id: "op_2".into(),
                error: failed_error,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptAborted {
                operation_id: "op_3".into(),
                reason: "cancelled".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptCompleted {
                operation_id: "op_direct_1".into(),
                turn_id: "turn_direct_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptFailed {
                operation_id: "op_direct_2".into(),
                error: direct_failed_error,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::PromptAborted {
                operation_id: "op_direct_3".into(),
                reason: "stopped".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_agent_invocation_lifecycle_events() {
        let service = EventService::new();
        let mut receiver = service.subscribe();
        let failed_error = CodingSessionError::Provider {
            message: "child failed".into(),
        };

        service.emit_agent_invocation_started(
            "op_1",
            "child_op_1",
            ProfileId::from("coder"),
            "implement it",
        );
        service.emit_agent_invocation_completed(
            "op_1",
            "child_op_1",
            ProfileId::from("coder"),
            "done",
        );
        service.emit_agent_invocation_failed(
            "op_2",
            "child_op_2",
            ProfileId::from("coder"),
            failed_error.clone(),
        );
        service.emit_agent_invocation_aborted(
            "op_3",
            "child_op_3",
            ProfileId::from("coder"),
            "cancelled",
        );

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentInvocationStarted {
                operation_id: "op_1".into(),
                child_operation_id: "child_op_1".into(),
                profile_id: ProfileId::from("coder"),
                task: "implement it".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentInvocationCompleted {
                operation_id: "op_1".into(),
                child_operation_id: "child_op_1".into(),
                profile_id: ProfileId::from("coder"),
                final_text: "done".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentInvocationFailed {
                operation_id: "op_2".into(),
                child_operation_id: "child_op_2".into(),
                profile_id: ProfileId::from("coder"),
                error: failed_error,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentInvocationAborted {
                operation_id: "op_3".into(),
                child_operation_id: "child_op_3".into(),
                profile_id: ProfileId::from("coder"),
                reason: "cancelled".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_agent_team_lifecycle_events() {
        let service = EventService::new();
        let mut receiver = service.subscribe();
        let failed_error = CodingSessionError::Provider {
            message: "team failed".into(),
        };

        service.emit_agent_team_started("team_op_1", ProfileId::from("review-team"), "review it");
        service.emit_agent_team_member_started(
            "team_op_1",
            "child_op_1",
            ProfileId::from("review-team"),
            ProfileId::from("reviewer"),
            "review it",
        );
        service.emit_agent_team_member_completed(
            "team_op_1",
            "child_op_1",
            ProfileId::from("review-team"),
            ProfileId::from("reviewer"),
            "looks good",
        );
        service.emit_agent_team_completed("team_op_1", ProfileId::from("review-team"), "done");
        service.emit_agent_team_failed(
            "team_op_2",
            ProfileId::from("review-team"),
            failed_error.clone(),
        );
        service.emit_agent_team_aborted("team_op_3", ProfileId::from("review-team"), "cancelled");

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamStarted {
                operation_id: "team_op_1".into(),
                team_id: ProfileId::from("review-team"),
                task: "review it".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamMemberStarted {
                operation_id: "team_op_1".into(),
                child_operation_id: "child_op_1".into(),
                team_id: ProfileId::from("review-team"),
                profile_id: ProfileId::from("reviewer"),
                task: "review it".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamMemberCompleted {
                operation_id: "team_op_1".into(),
                child_operation_id: "child_op_1".into(),
                team_id: ProfileId::from("review-team"),
                profile_id: ProfileId::from("reviewer"),
                final_text: "looks good".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamCompleted {
                operation_id: "team_op_1".into(),
                team_id: ProfileId::from("review-team"),
                final_text: "done".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamFailed {
                operation_id: "team_op_2".into(),
                team_id: ProfileId::from("review-team"),
                error: failed_error,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::AgentTeamAborted {
                operation_id: "team_op_3".into(),
                team_id: ProfileId::from("review-team"),
                reason: "cancelled".into(),
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_delegation_lifecycle_events() {
        use crate::coding_session::prompt::DelegationRequest;

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let request = DelegationRequest {
            operation_id: "op_1".into(),
            turn_id: "turn_1".into(),
            tool_call_id: "tool_1".into(),
            requesting_profile_id: ProfileId::from("planner"),
            target_kind: ProfileKind::Agent,
            target_id: ProfileId::from("coder"),
            task: "implement it".into(),
        };
        let failed_error = CodingSessionError::Provider {
            message: "child failed".into(),
        };

        service.emit_delegation_approved(&request);
        service.emit_delegation_confirmation_required(&request, "needs approval");
        service.emit_delegation_rejected(&request, "not allowed");
        service.emit_delegation_started(&request, "child_op_1");
        service.emit_delegation_completed(&request, "child_op_1", "done");
        service.emit_delegation_failed(&request, "child_op_2", failed_error.clone());

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationApproved {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationConfirmationRequired {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
                reason: "needs approval".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationRejected {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
                reason: "not allowed".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationStarted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
                child_operation_id: "child_op_1".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationCompleted {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
                child_operation_id: "child_op_1".into(),
                final_text: "done".into(),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::DelegationFailed {
                operation_id: "op_1".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                requesting_profile_id: ProfileId::from("planner"),
                target_kind: ProfileKind::Agent,
                target_id: ProfileId::from("coder"),
                task: "implement it".into(),
                child_operation_id: "child_op_2".into(),
                error: failed_error,
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_emits_self_healing_edit_events() {
        use crate::coding_session::{
            SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditOutcome,
            SelfHealingEditRepairAttempt, SelfHealingEditReplacement,
        };

        let service = EventService::new();
        let mut receiver = service.subscribe();
        let check_output = SelfHealingEditCheckOutput {
            command: "cargo check".into(),
            stdout: "ok".into(),
            stderr: String::new(),
            exit_code: 0,
        };
        let repair = SelfHealingEditRepairAttempt {
            attempt: 1,
            replacements: vec![SelfHealingEditReplacement::new("old", "new")],
            diagnostics: vec![SelfHealingEditDiagnostic {
                message: "fixed by repair".into(),
            }],
            check_output: Some(check_output.clone()),
        };
        let outcome = SelfHealingEditOutcome {
            path: "src/lib.rs".into(),
            message: "edited".into(),
            diff: String::new(),
            patch: String::new(),
            first_changed_line: Some(7),
            attempts: 2,
            diagnostics: Vec::new(),
            check_output: Some(check_output.clone()),
            repair_attempts: vec![repair.clone()],
        };
        let error = CodingSessionError::SelfHealingEditFailed {
            message: "check failed".into(),
            diagnostics: vec![SelfHealingEditDiagnostic {
                message: "missing symbol".into(),
            }],
            check_output: None,
            repair_attempts: Vec::new(),
        };

        service.emit_self_healing_edit_started("op_1", "src/lib.rs", 1);
        service.emit_self_healing_edit_repair_attempted("op_1", "src/lib.rs", &repair);
        service.emit_self_healing_edit_completed("op_1", &outcome);
        service.emit_self_healing_edit_failed("op_1", "src/lib.rs", &error);

        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SelfHealingEditStarted {
                operation_id: "op_1".into(),
                path: "src/lib.rs".into(),
                replacements: 1,
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SelfHealingEditRepairAttempted {
                operation_id: "op_1".into(),
                path: "src/lib.rs".into(),
                attempt: 1,
                replacements: vec![SelfHealingEditReplacement::new("old", "new")],
                diagnostics: vec![SelfHealingEditDiagnostic {
                    message: "fixed by repair".into(),
                }],
                check_output: Some(check_output.clone()),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SelfHealingEditCompleted {
                operation_id: "op_1".into(),
                path: "src/lib.rs".into(),
                attempts: 2,
                first_changed_line: Some(7),
                check_output: Some(check_output),
            })
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            Some(CodingAgentEvent::SelfHealingEditFailed {
                operation_id: "op_1".into(),
                path: "src/lib.rs".into(),
                error,
            })
        );
        assert_eq!(receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn event_service_reports_bounded_product_event_window() {
        let service = EventService::with_event_capacity_for_tests(2);

        for index in 0..4 {
            service.emit(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {index}"),
            });
        }

        let status = service.backpressure_status();

        assert_eq!(status.channel_capacity, 2);
        assert_eq!(status.retained_capacity, 2);
        assert_eq!(
            status.oldest_retained_sequence,
            Some(ProductEventSequence::new(3))
        );
        assert_eq!(status.current_sequence, ProductEventSequence::new(4));
        assert_eq!(status.dropped_before, Some(ProductEventSequence::new(3)));
    }

    #[tokio::test]
    async fn product_event_receiver_lag_reports_snapshot_recovery() {
        let service = EventService::with_event_capacity_for_tests(1);
        let mut receiver = service.subscribe_product_events();

        for index in 0..3 {
            service.emit(CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {index}"),
            });
        }

        let error = receiver.recv().await.unwrap_err();

        assert_eq!(error.code(), "event_stream_lag");
        assert!(
            error
                .to_string()
                .contains("client must request a fresh UI snapshot")
        );
    }

    #[test]
    fn recovery_markers_publish_terminal_product_events() {
        let service = EventService::new();
        let event = service.emit_operation_recovered(
            "op_recovered",
            "recovery_1",
            "startup recovery marked incomplete operation in-doubt",
        );

        assert_eq!(event.operation_id(), Some("op_recovered"));
        assert_eq!(
            event.terminal_status(),
            Some(ProductEventTerminalStatus::Recovered)
        );
        assert_eq!(event.family(), ProductEventFamily::Workflow);
    }
}
