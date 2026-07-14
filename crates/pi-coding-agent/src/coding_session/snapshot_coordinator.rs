use super::CodingSessionError;
use super::capability_snapshot::CapabilityGeneration;
use super::client_projection::{ClientConnectionId, ClientDraft, UiSnapshot, UiSnapshotCursor};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::error::CodingAgentLifecycleRejection;
use super::event::{ProductEvent, ProductEventSequence, ProductEventTerminalStatus};
use super::operation_control::{OperationKind, PromptControlHandle};
use super::public_operation::{OperationDescriptor, OperationRootTerminalEvidence};
use crate::protocol::version::UI_SNAPSHOT_PROTOCOL_VERSION;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub(crate) const MAX_CLIENTS: usize = 64;
pub(crate) const MAX_DRAFTS: usize = 64;
pub(crate) const MAX_RECEIPTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ClientGeneration(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientHandle {
    pub(crate) id: ClientConnectionId,
    pub(crate) generation: ClientGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubmittedEventDurability {
    Durable,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmittedTerminalAnchor {
    ProductEvent {
        sequence: u64,
        durability: SubmittedEventDurability,
    },
    OutcomeOnly {
        acknowledgement: super::public_projection::CodingAgentOutcomeAcknowledgementId,
    },
    TerminalUncertain {
        operation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmittedOperationStatus {
    Accepted {
        operation_id: String,
        kind: OperationKind,
        descriptor: OperationDescriptor,
    },
    Running {
        operation_id: String,
        kind: OperationKind,
        descriptor: OperationDescriptor,
    },
    Terminal {
        operation_id: String,
        kind: OperationKind,
        descriptor: OperationDescriptor,
        anchor: SubmittedTerminalAnchor,
        status: ProductEventTerminalStatus,
        root_count: u8,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ClientSnapshotState {
    pub(crate) snapshot: UiSnapshot,
    pub(crate) drafts: Vec<DraftRecord>,
    pub(crate) submitted_operation: Option<SubmittedOperationStatus>,
    pub(crate) acknowledged_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftRecord {
    pub(crate) id: String,
    pub(crate) kind: super::client_projection::ClientDraftKind,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientRecord {
    pub(crate) generation: ClientGeneration,
    connection: ConnectionLifecycle,
    pub(crate) acknowledged_sequence: u64,
    pub(crate) prompt_draft: Option<DraftRecord>,
    pub(crate) steer_drafts: VecDeque<DraftRecord>,
    pub(crate) follow_up_drafts: VecDeque<DraftRecord>,
    pub(crate) submitted_operation: Option<SubmittedOperationStatus>,
    pub(crate) control_receipts: HashMap<String, String>,
    pub(crate) control_receipt_order: VecDeque<String>,
}

impl ClientRecord {
    fn new(generation: ClientGeneration) -> Self {
        Self {
            generation,
            connection: ConnectionLifecycle::Attached,
            acknowledged_sequence: 0,
            prompt_draft: None,
            steer_drafts: VecDeque::new(),
            follow_up_drafts: VecDeque::new(),
            submitted_operation: None,
            control_receipts: HashMap::new(),
            control_receipt_order: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionLifecycle {
    Attached,
    ShuttingDown,
    Detached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeLifecycle {
    Running,
    ShuttingDown,
    ShutDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientDetachOutcome {
    Detached,
    AlreadyDetached,
    StaleGeneration,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SnapshotProjection {
    pub(crate) revision: u64,
    pub(crate) session: CodingAgentSessionView,
    pub(crate) capabilities: CodingAgentCapabilities,
    pub(crate) active_operation: Option<OperationKind>,
    pub(crate) capability_generation: CapabilityGeneration,
}

#[derive(Debug)]
pub(crate) struct SnapshotState {
    pub(crate) runtime_lifecycle: RuntimeLifecycle,
    pub(crate) lifecycle_epoch: u64,
    pub(crate) clients: HashMap<ClientConnectionId, ClientRecord>,
    pub(crate) projection: Option<SnapshotProjection>,
    pub(crate) capability_generation: CapabilityGeneration,
    pub(crate) next_event_sequence: u64,
    pub(crate) retained_product_events: VecDeque<ProductEvent>,
    pub(crate) dropped_before: Option<ProductEventSequence>,
    pub(crate) recovery_revision: u64,
    pub(crate) shutdown_drain_boundary: Option<ProductEventSequence>,
    shutdown_drain_eligibility: HashMap<ClientConnectionId, ClientGeneration>,
}

impl Default for SnapshotState {
    fn default() -> Self {
        Self {
            runtime_lifecycle: RuntimeLifecycle::Running,
            lifecycle_epoch: 0,
            clients: HashMap::new(),
            projection: None,
            capability_generation: CapabilityGeneration::new(1),
            next_event_sequence: 1,
            retained_product_events: VecDeque::new(),
            dropped_before: None,
            recovery_revision: 0,
            shutdown_drain_boundary: None,
            shutdown_drain_eligibility: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct SnapshotCoordinator {
    pub(crate) state: Mutex<SnapshotState>,
    prompt_control: Mutex<Option<PromptControlBinding>>,
    lifecycle_sender: watch::Sender<u64>,
    #[cfg(test)]
    submission_transition_probe: Mutex<Option<SubmissionTransitionProbe>>,
}

#[derive(Debug, Clone)]
struct PromptControlBinding {
    owner: ClientHandle,
    operation_id: String,
    channel_generation: super::operation_control::PromptControlGeneration,
    sender: PromptControlHandle,
}

#[cfg(test)]
#[derive(Debug)]
struct SubmissionTransitionProbe {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

impl Default for SnapshotCoordinator {
    fn default() -> Self {
        let (lifecycle_sender, _) = watch::channel(0);
        Self {
            state: Mutex::new(SnapshotState::default()),
            prompt_control: Mutex::new(None),
            lifecycle_sender,
            #[cfg(test)]
            submission_transition_probe: Mutex::new(None),
        }
    }
}

impl SnapshotCoordinator {
    pub(crate) fn ensure_runtime_running(&self) -> Result<(), CodingSessionError> {
        let state = self.state.lock().unwrap();
        Self::validate_runtime(&state).map_err(|error| match error {
            ClientRegistryError::Lifecycle(reason) => CodingSessionError::Lifecycle { reason },
            other => CodingSessionError::Input {
                message: other.to_string(),
            },
        })
    }

    pub(crate) fn request_shutdown(&self) -> RuntimeLifecycle {
        let mut state = self.state.lock().unwrap();
        let previous = state.runtime_lifecycle;
        if previous != RuntimeLifecycle::Running {
            return previous;
        }
        state.runtime_lifecycle = RuntimeLifecycle::ShuttingDown;
        state.shutdown_drain_eligibility.clear();
        let mut eligible = Vec::new();
        for (id, record) in &mut state.clients {
            if record.connection == ConnectionLifecycle::Attached {
                record.connection = ConnectionLifecycle::ShuttingDown;
                eligible.push((id.clone(), record.generation));
            }
        }
        state.shutdown_drain_eligibility.extend(eligible);
        state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
        let lifecycle_epoch = state.lifecycle_epoch;
        drop(state);
        *self.prompt_control.lock().unwrap() = None;
        self.lifecycle_sender.send_replace(lifecycle_epoch);
        previous
    }

    pub(crate) async fn wait_for_active_operation_to_drain(&self) {
        let mut receiver = self.subscribe_lifecycle();
        loop {
            let active = self
                .state
                .lock()
                .unwrap()
                .projection
                .as_ref()
                .and_then(|projection| projection.active_operation);
            if active.is_none() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }

    pub(crate) fn finish_shutdown(&self) {
        let mut state = self.state.lock().unwrap();
        if state.runtime_lifecycle == RuntimeLifecycle::ShutDown {
            return;
        }
        debug_assert_eq!(state.runtime_lifecycle, RuntimeLifecycle::ShuttingDown);
        state.runtime_lifecycle = RuntimeLifecycle::ShutDown;
        for record in state.clients.values_mut() {
            if record.connection == ConnectionLifecycle::ShuttingDown {
                record.connection = ConnectionLifecycle::Detached;
            }
        }
        state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
        let lifecycle_epoch = state.lifecycle_epoch;
        drop(state);
        self.lifecycle_sender.send_replace(lifecycle_epoch);
    }

    pub(crate) fn is_shut_down(&self) -> bool {
        self.state.lock().unwrap().runtime_lifecycle == RuntimeLifecycle::ShutDown
    }

    pub(crate) fn enqueue_prompt_control_draft(
        &self,
        handle: &ClientHandle,
        operation_id: &str,
        draft_id: super::public_projection::CodingAgentDraftId,
        kind: super::public_projection::CodingAgentControlKind,
    ) -> Result<
        super::public_projection::CodingAgentControlReceipt,
        super::public_projection::CodingAgentControlRejection,
    > {
        let text = {
            let mut state = self.state.lock().unwrap();
            let record = Self::record(&mut state, handle).map_err(|error| {
                super::public_projection::CodingAgentControlRejection {
                    control_id: super::public_projection::CodingAgentControlId(draft_id.0.clone()),
                    operation_id: operation_id.into(),
                    kind,
                    reason: control_rejection_reason(&error),
                }
            })?;
            let queue = match kind {
                super::public_projection::CodingAgentControlKind::Steer => &record.steer_drafts,
                super::public_projection::CodingAgentControlKind::FollowUp => {
                    &record.follow_up_drafts
                }
                super::public_projection::CodingAgentControlKind::Abort => {
                    return Err(super::public_projection::CodingAgentControlRejection {
                        control_id: super::public_projection::CodingAgentControlId(draft_id.0),
                        operation_id: operation_id.into(), kind,
                        reason: super::public_projection::CodingAgentControlRejectionReason::InvalidInput,
                    });
                }
            };
            queue
                .iter()
                .find(|draft| draft.id == draft_id.0)
                .map(|draft| draft.text.clone())
                .ok_or_else(|| super::public_projection::CodingAgentControlRejection {
                    control_id: super::public_projection::CodingAgentControlId(draft_id.0.clone()),
                    operation_id: operation_id.into(),
                    kind,
                    reason:
                        super::public_projection::CodingAgentControlRejectionReason::InvalidInput,
                })?
        };
        self.enqueue_prompt_control(
            handle,
            operation_id,
            super::public_projection::CodingAgentControlId(draft_id.0),
            kind,
            text,
        )
    }

    pub(crate) fn enqueue_prompt_control(
        &self,
        handle: &ClientHandle,
        operation_id: &str,
        control_id: super::public_projection::CodingAgentControlId,
        kind: super::public_projection::CodingAgentControlKind,
        text: String,
    ) -> Result<
        super::public_projection::CodingAgentControlReceipt,
        super::public_projection::CodingAgentControlRejection,
    > {
        if control_id.0.trim().is_empty() || text.trim().is_empty() {
            return Err(super::public_projection::CodingAgentControlRejection {
                control_id,
                operation_id: operation_id.into(),
                kind,
                reason: super::public_projection::CodingAgentControlRejectionReason::InvalidInput,
            });
        }
        let mut state = self.state.lock().unwrap();
        let record = match Self::record(&mut state, handle) {
            Ok(record) => record,
            Err(error @ ClientRegistryError::Lifecycle(_))
            | Err(error @ ClientRegistryError::StaleClient) => {
                return Err(super::public_projection::CodingAgentControlRejection {
                    control_id,
                    operation_id: operation_id.into(),
                    kind,
                    reason: control_rejection_reason(&error),
                });
            }
            Err(_) => {
                return Err(super::public_projection::CodingAgentControlRejection {
                    control_id,
                    operation_id: operation_id.into(),
                    kind,
                    reason:
                        super::public_projection::CodingAgentControlRejectionReason::InvalidInput,
                });
            }
        };
        let key = format!("{}:{}", operation_id, control_id.0);
        let signature = format!("{:?}:{}", kind, text);
        if let Some(stored) = record.control_receipts.get(&key) {
            if stored != &signature {
                return Err(super::public_projection::CodingAgentControlRejection {
                    control_id,
                    operation_id: operation_id.into(),
                    kind,
                    reason:
                        super::public_projection::CodingAgentControlRejectionReason::PayloadConflict,
                });
            }
            return Ok(super::public_projection::CodingAgentControlReceipt {
                control_id,
                operation_id: operation_id.into(),
                kind,
            });
        }
        if record.control_receipts.len() >= MAX_RECEIPTS {
            return Err(super::public_projection::CodingAgentControlRejection { control_id, operation_id: operation_id.into(), kind, reason: super::public_projection::CodingAgentControlRejectionReason::QueueCapacityExceeded });
        }
        let mut binding = self.prompt_control.lock().unwrap();
        let Some(active) = binding.as_mut() else {
            return Err(super::public_projection::CodingAgentControlRejection {
                control_id,
                operation_id: operation_id.into(),
                kind,
                reason:
                    super::public_projection::CodingAgentControlRejectionReason::TargetNotRunning,
            });
        };
        if active.owner.id != handle.id {
            return Err(super::public_projection::CodingAgentControlRejection {
                control_id,
                operation_id: operation_id.into(),
                kind,
                reason: super::public_projection::CodingAgentControlRejectionReason::NotOwner,
            });
        }
        if active.operation_id != operation_id {
            return Err(super::public_projection::CodingAgentControlRejection {
                control_id,
                operation_id: operation_id.into(),
                kind,
                reason: super::public_projection::CodingAgentControlRejectionReason::TargetMismatch,
            });
        }
        let sent = match kind {
            super::public_projection::CodingAgentControlKind::Abort => active.sender.abort(text),
            super::public_projection::CodingAgentControlKind::Steer => active.sender.steer(text),
            super::public_projection::CodingAgentControlKind::FollowUp => {
                active.sender.follow_up(text)
            }
        };
        if let Err(error) = sent {
            let reason = match error {
                super::CodingSessionError::Busy { .. } => super::public_projection::CodingAgentControlRejectionReason::QueueCapacityExceeded,
                _ => super::public_projection::CodingAgentControlRejectionReason::ControlChannelClosed,
            };
            return Err(super::public_projection::CodingAgentControlRejection {
                control_id,
                operation_id: operation_id.into(),
                kind,
                reason,
            });
        }
        record.control_receipts.insert(key.clone(), signature);
        record.control_receipt_order.push_back(key);
        let queue = match kind {
            super::public_projection::CodingAgentControlKind::Steer => {
                Some(&mut record.steer_drafts)
            }
            super::public_projection::CodingAgentControlKind::FollowUp => {
                Some(&mut record.follow_up_drafts)
            }
            super::public_projection::CodingAgentControlKind::Abort => None,
        };
        if let Some(queue) = queue
            && let Some(position) = queue.iter().position(|draft| draft.id == control_id.0)
        {
            queue.remove(position);
        }
        Ok(super::public_projection::CodingAgentControlReceipt {
            control_id,
            operation_id: operation_id.into(),
            kind,
        })
    }

    pub(crate) fn bind_prompt_control(
        &self,
        owner: ClientHandle,
        operation_id: String,
        channel_generation: super::operation_control::PromptControlGeneration,
        sender: PromptControlHandle,
    ) {
        *self.prompt_control.lock().unwrap() = Some(PromptControlBinding {
            owner,
            operation_id,
            channel_generation,
            sender,
        });
    }

    pub(crate) fn clear_prompt_control_if(
        &self,
        operation_id: &str,
        channel_generation: super::operation_control::PromptControlGeneration,
    ) {
        let mut binding = self.prompt_control.lock().unwrap();
        if binding.as_ref().is_some_and(|active| {
            active.operation_id == operation_id && active.channel_generation == channel_generation
        }) {
            *binding = None;
        }
    }

    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn connect_or_takeover(
        &self,
        id: ClientConnectionId,
    ) -> Result<ClientHandle, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::validate_runtime(&state)?;
        state.shutdown_drain_eligibility.remove(&id);
        if let Some(record) = state.clients.get_mut(&id) {
            record.generation.0 += 1;
            record.connection = ConnectionLifecycle::Attached;
            let generation = record.generation;
            state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
            let lifecycle_epoch = state.lifecycle_epoch;
            let handle = ClientHandle { id, generation };
            drop(state);
            self.rebind_prompt_control(&handle);
            self.lifecycle_sender.send_replace(lifecycle_epoch);
            return Ok(handle);
        }
        if state.clients.len() >= MAX_CLIENTS {
            return Err(ClientRegistryError::ClientCapacityExceeded { limit: MAX_CLIENTS });
        }
        let generation = ClientGeneration(1);
        state
            .clients
            .insert(id.clone(), ClientRecord::new(generation));
        Ok(ClientHandle { id, generation })
    }

    pub(crate) fn subscribe_lifecycle(&self) -> watch::Receiver<u64> {
        self.lifecycle_sender.subscribe()
    }

    pub(crate) fn validate_handle(&self, handle: &ClientHandle) -> Result<(), ClientRegistryError> {
        let state = self.state.lock().unwrap();
        Self::validate_client(&state, handle)
    }

    pub(crate) fn validate_receiver(
        &self,
        handle: &ClientHandle,
    ) -> Result<(), ClientRegistryError> {
        let state = self.state.lock().unwrap();
        Self::validate_receiver_in_state(&state, handle, None)
    }

    pub(crate) fn validate_receiver_event(
        &self,
        handle: &ClientHandle,
        sequence: ProductEventSequence,
    ) -> Result<(), ClientRegistryError> {
        let state = self.state.lock().unwrap();
        Self::validate_receiver_in_state(&state, handle, Some(sequence))
    }

    fn validate_receiver_in_state(
        state: &SnapshotState,
        handle: &ClientHandle,
        sequence: Option<ProductEventSequence>,
    ) -> Result<(), ClientRegistryError> {
        if state.runtime_lifecycle == RuntimeLifecycle::ShutDown {
            if sequence.is_some_and(|sequence| {
                state
                    .shutdown_drain_boundary
                    .is_some_and(|boundary| sequence <= boundary)
            }) && state
                .shutdown_drain_eligibility
                .get(&handle.id)
                .is_some_and(|generation| *generation == handle.generation)
            {
                let record =
                    state
                        .clients
                        .get(&handle.id)
                        .ok_or(ClientRegistryError::Lifecycle(
                            CodingAgentLifecycleRejection::StaleGeneration,
                        ))?;
                if record.generation == handle.generation {
                    return Ok(());
                }
            }
            if state.clients.get(&handle.id).is_some_and(|record| {
                record.generation == handle.generation
                    && record.connection == ConnectionLifecycle::Detached
                    && !state
                        .shutdown_drain_eligibility
                        .get(&handle.id)
                        .is_some_and(|generation| *generation == handle.generation)
            }) {
                return Err(ClientRegistryError::Lifecycle(
                    CodingAgentLifecycleRejection::Detached,
                ));
            }
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::RuntimeShutDown,
            ));
        }
        let record = state
            .clients
            .get(&handle.id)
            .ok_or(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ))?;
        if record.generation != handle.generation {
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ));
        }
        match record.connection {
            ConnectionLifecycle::Attached | ConnectionLifecycle::ShuttingDown => Ok(()),
            ConnectionLifecycle::Detached => Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::Detached,
            )),
        }
    }

    pub(crate) fn shutdown_drain_boundary(&self) -> Option<ProductEventSequence> {
        self.state.lock().unwrap().shutdown_drain_boundary
    }

    fn rebind_prompt_control(&self, handle: &ClientHandle) {
        let mut binding = self.prompt_control.lock().unwrap();
        if let Some(active) = binding.as_mut()
            && active.owner.id == handle.id
        {
            active.owner.generation = handle.generation;
        }
    }

    pub(crate) fn detach(
        &self,
        handle: &ClientHandle,
    ) -> Result<ClientDetachOutcome, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::validate_runtime(&state)?;
        let Some(record) = state.clients.get_mut(&handle.id) else {
            return Ok(ClientDetachOutcome::StaleGeneration);
        };
        if record.generation != handle.generation {
            return Ok(ClientDetachOutcome::StaleGeneration);
        }
        match record.connection {
            ConnectionLifecycle::Attached => {
                record.connection = ConnectionLifecycle::Detached;
                state.shutdown_drain_eligibility.remove(&handle.id);
                state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
                let lifecycle_epoch = state.lifecycle_epoch;
                drop(state);
                self.lifecycle_sender.send_replace(lifecycle_epoch);
                Ok(ClientDetachOutcome::Detached)
            }
            ConnectionLifecycle::Detached => Ok(ClientDetachOutcome::AlreadyDetached),
            ConnectionLifecycle::ShuttingDown => {
                unreachable!("runtime validation rejects detach while the runtime is shutting down")
            }
        }
    }

    pub(crate) fn is_current(&self, handle: &ClientHandle) -> bool {
        let state = self.state.lock().unwrap();
        state
            .clients
            .get(&handle.id)
            .is_some_and(|record| record.generation == handle.generation)
    }

    pub(crate) fn install_projection(
        &self,
        session: CodingAgentSessionView,
        capabilities: CodingAgentCapabilities,
        capability_generation: CapabilityGeneration,
    ) {
        let mut state = self.state.lock().unwrap();
        let active_operation = state
            .projection
            .as_ref()
            .and_then(|projection| projection.active_operation);
        let revision = state
            .projection
            .as_ref()
            .map_or(1, |projection| projection.revision + 1);
        state.projection = Some(SnapshotProjection {
            revision,
            session,
            capabilities,
            active_operation,
            capability_generation,
        });
        state.capability_generation = capability_generation;
    }

    pub(crate) fn current_capability_generation(&self) -> CapabilityGeneration {
        self.state.lock().unwrap().capability_generation
    }

    pub(crate) fn install_next_capability_generation(&self) -> CapabilityGeneration {
        let mut state = self.state.lock().unwrap();
        let next = state.capability_generation.next();
        state.capability_generation = next;
        if let Some(projection) = state.projection.as_mut() {
            projection.revision += 1;
            projection.capability_generation = next;
        }
        next
    }

    pub(crate) fn set_active_operation(&self, active_operation: Option<OperationKind>) {
        let mut state = self.state.lock().unwrap();
        if let Some(projection) = state.projection.as_mut() {
            projection.revision += 1;
            projection.active_operation = active_operation;
        }
        if active_operation.is_none() {
            state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
            let lifecycle_epoch = state.lifecycle_epoch;
            drop(state);
            self.lifecycle_sender.send_replace(lifecycle_epoch);
        }
    }

    pub(crate) fn mark_recovery_projected(&self) {
        self.state.lock().unwrap().recovery_revision += 1;
    }

    pub(crate) fn snapshot(&self) -> UiSnapshot {
        self.snapshot_for_client(None)
            .expect("snapshot projection must be installed by session construction")
    }

    pub(crate) fn client_snapshot(
        &self,
        handle: &ClientHandle,
    ) -> Result<UiSnapshot, ClientRegistryError> {
        self.snapshot_for_client(Some(handle))
    }

    pub(crate) fn client_state(
        &self,
        handle: &ClientHandle,
    ) -> Result<ClientSnapshotState, ClientRegistryError> {
        let snapshot = self.client_snapshot(handle)?;
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        let drafts = record
            .prompt_draft
            .iter()
            .chain(record.steer_drafts.iter())
            .chain(record.follow_up_drafts.iter())
            .cloned()
            .collect();
        Ok(ClientSnapshotState {
            snapshot,
            drafts,
            submitted_operation: record.submitted_operation.clone(),
            acknowledged_sequence: record.acknowledged_sequence,
        })
    }

    pub(crate) fn validate_prompt_draft(
        &self,
        handle: &ClientHandle,
        draft_id: &str,
        text: &str,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        match &record.prompt_draft {
            Some(draft) if draft.id == draft_id && draft.text == text => Ok(()),
            _ => Err(ClientRegistryError::InvalidInput),
        }
    }

    fn snapshot_for_client(
        &self,
        handle: Option<&ClientHandle>,
    ) -> Result<UiSnapshot, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let client_drafts = match handle {
            Some(handle) => {
                let record = Self::record(&mut state, handle)?;
                record
                    .prompt_draft
                    .iter()
                    .chain(record.steer_drafts.iter())
                    .chain(record.follow_up_drafts.iter())
                    .map(|draft| ClientDraft::new(draft.kind, draft.text.clone()))
                    .collect()
            }
            None => Vec::new(),
        };
        let projection = state
            .projection
            .clone()
            .expect("snapshot projection must be installed by session construction");
        Ok(UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(
                    state.next_event_sequence.saturating_sub(1),
                ),
                capability_generation: projection.capability_generation,
            },
            UI_SNAPSHOT_PROTOCOL_VERSION,
            projection.session,
            projection.capabilities,
            projection.active_operation,
            client_drafts,
        ))
    }

    fn record<'a>(
        state: &'a mut SnapshotState,
        handle: &ClientHandle,
    ) -> Result<&'a mut ClientRecord, ClientRegistryError> {
        Self::validate_runtime(state)?;
        let record = state
            .clients
            .get_mut(&handle.id)
            .ok_or(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ))?;
        if record.generation != handle.generation {
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ));
        }
        match record.connection {
            ConnectionLifecycle::Attached => {}
            ConnectionLifecycle::ShuttingDown => {
                return Err(ClientRegistryError::Lifecycle(
                    CodingAgentLifecycleRejection::RuntimeShutDown,
                ));
            }
            ConnectionLifecycle::Detached => {
                return Err(ClientRegistryError::Lifecycle(
                    CodingAgentLifecycleRejection::Detached,
                ));
            }
        }
        Ok(record)
    }

    fn validate_runtime(state: &SnapshotState) -> Result<(), ClientRegistryError> {
        match state.runtime_lifecycle {
            RuntimeLifecycle::Running => Ok(()),
            RuntimeLifecycle::ShuttingDown | RuntimeLifecycle::ShutDown => Err(
                ClientRegistryError::Lifecycle(CodingAgentLifecycleRejection::RuntimeShutDown),
            ),
        }
    }

    fn validate_terminal_runtime(state: &SnapshotState) -> Result<(), ClientRegistryError> {
        match state.runtime_lifecycle {
            RuntimeLifecycle::Running | RuntimeLifecycle::ShuttingDown => Ok(()),
            RuntimeLifecycle::ShutDown => Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::RuntimeShutDown,
            )),
        }
    }

    pub(crate) fn validate_client(
        state: &SnapshotState,
        handle: &ClientHandle,
    ) -> Result<(), ClientRegistryError> {
        Self::validate_runtime(state)?;
        let record = state
            .clients
            .get(&handle.id)
            .ok_or(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ))?;
        if record.generation != handle.generation {
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ));
        }
        match record.connection {
            ConnectionLifecycle::Attached => {}
            ConnectionLifecycle::ShuttingDown => {
                return Err(ClientRegistryError::Lifecycle(
                    CodingAgentLifecycleRejection::RuntimeShutDown,
                ));
            }
            ConnectionLifecycle::Detached => {
                return Err(ClientRegistryError::Lifecycle(
                    CodingAgentLifecycleRejection::Detached,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn live_lag_recovery(
        &self,
        handle: &ClientHandle,
    ) -> Result<(ClientSnapshotState, u64), ClientRegistryError> {
        let state = self.state.lock().unwrap();
        match state.runtime_lifecycle {
            RuntimeLifecycle::ShutDown => {
                let boundary =
                    state
                        .shutdown_drain_boundary
                        .ok_or(ClientRegistryError::Lifecycle(
                            CodingAgentLifecycleRejection::RuntimeShutDown,
                        ))?;
                Self::shutdown_lag_recovery_from_state(&state, handle, boundary)
            }
            RuntimeLifecycle::Running | RuntimeLifecycle::ShuttingDown => {
                Self::validate_receiver_in_state(&state, handle, None)?;
                Self::lag_recovery_from_state(&state, handle)
            }
        }
    }

    pub(crate) fn shutdown_lag_recovery(
        &self,
        handle: &ClientHandle,
        boundary: ProductEventSequence,
    ) -> Result<(ClientSnapshotState, u64), ClientRegistryError> {
        let state = self.state.lock().unwrap();
        Self::shutdown_lag_recovery_from_state(&state, handle, boundary)
    }

    fn shutdown_lag_recovery_from_state(
        state: &SnapshotState,
        handle: &ClientHandle,
        boundary: ProductEventSequence,
    ) -> Result<(ClientSnapshotState, u64), ClientRegistryError> {
        Self::validate_receiver_in_state(state, handle, Some(boundary))?;
        Self::lag_recovery_from_state(state, handle)
    }

    fn lag_recovery_from_state(
        state: &SnapshotState,
        handle: &ClientHandle,
    ) -> Result<(ClientSnapshotState, u64), ClientRegistryError> {
        let record = state
            .clients
            .get(&handle.id)
            .filter(|record| record.generation == handle.generation)
            .ok_or(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ))?;
        let projection = state
            .projection
            .clone()
            .expect("snapshot projection must be installed by session construction");
        let snapshot = UiSnapshot::new(
            UiSnapshotCursor {
                last_event_sequence: ProductEventSequence::new(
                    state.next_event_sequence.saturating_sub(1),
                ),
                capability_generation: projection.capability_generation,
            },
            UI_SNAPSHOT_PROTOCOL_VERSION,
            projection.session,
            projection.capabilities,
            projection.active_operation,
            record
                .prompt_draft
                .iter()
                .chain(record.steer_drafts.iter())
                .chain(record.follow_up_drafts.iter())
                .map(|draft| ClientDraft::new(draft.kind, draft.text.clone()))
                .collect(),
        );
        let client_state = ClientSnapshotState {
            snapshot,
            drafts: record
                .prompt_draft
                .iter()
                .chain(record.steer_drafts.iter())
                .chain(record.follow_up_drafts.iter())
                .cloned()
                .collect(),
            submitted_operation: record.submitted_operation.clone(),
            acknowledged_sequence: record.acknowledged_sequence,
        };
        let oldest_available = state
            .retained_product_events
            .front()
            .map(ProductEvent::sequence)
            .map(ProductEventSequence::get)
            .unwrap_or_else(|| {
                client_state
                    .snapshot
                    .cursor
                    .last_event_sequence
                    .get()
                    .saturating_add(1)
            });
        Ok((client_state, oldest_available))
    }

    pub(crate) fn acknowledge(
        &self,
        handle: &ClientHandle,
        sequence: u64,
    ) -> Result<u64, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if sequence < record.acknowledged_sequence {
            return Ok(record.acknowledged_sequence);
        }
        record.acknowledged_sequence = sequence;
        if let Some(SubmittedOperationStatus::Terminal {
            anchor:
                SubmittedTerminalAnchor::ProductEvent {
                    sequence: terminal_sequence,
                    ..
                },
            ..
        }) = &record.submitted_operation
        {
            if sequence >= *terminal_sequence {
                record.submitted_operation = None;
            }
        }
        Ok(record.acknowledged_sequence)
    }

    pub(crate) fn acknowledge_outcome(
        &self,
        handle: &ClientHandle,
        acknowledgement: &super::public_projection::CodingAgentOutcomeAcknowledgementId,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        match &record.submitted_operation {
            Some(SubmittedOperationStatus::Terminal {
                anchor:
                    SubmittedTerminalAnchor::OutcomeOnly {
                        acknowledgement: stored,
                    },
                ..
            }) if stored == acknowledgement => {
                record.submitted_operation = None;
                Ok(())
            }
            _ => Err(ClientRegistryError::InvalidInput),
        }
    }

    pub(crate) fn set_prompt_draft(
        &self,
        handle: &ClientHandle,
        draft: Option<DraftRecord>,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::record(&mut state, handle)?.prompt_draft = draft;
        Ok(())
    }

    pub(crate) fn enqueue_draft(
        &self,
        handle: &ClientHandle,
        draft: DraftRecord,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        let queue = match draft.kind {
            super::client_projection::ClientDraftKind::Steer => &mut record.steer_drafts,
            super::client_projection::ClientDraftKind::FollowUp => &mut record.follow_up_drafts,
            super::client_projection::ClientDraftKind::Prompt => {
                return Err(ClientRegistryError::InvalidInput);
            }
        };
        if queue.iter().any(|item| item.id == draft.id) {
            if let Some(item) = queue.iter_mut().find(|item| item.id == draft.id) {
                *item = draft;
            }
            return Ok(());
        }
        if queue.len() >= MAX_DRAFTS {
            return Err(ClientRegistryError::QueueCapacityExceeded { limit: MAX_DRAFTS });
        }
        queue.push_back(draft);
        Ok(())
    }

    pub(crate) fn commit_submission_running(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        descriptor: OperationDescriptor,
        expected_prompt_draft: Option<&DraftRecord>,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        #[cfg(test)]
        if let Some(probe) = self.submission_transition_probe.lock().unwrap().take() {
            probe.entered.send(()).unwrap();
            probe.release.recv().unwrap();
        }
        let record = Self::record(&mut state, handle)?;
        if record.submitted_operation.is_some() {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        match (descriptor.submitted_kind, expected_prompt_draft) {
            (OperationKind::Prompt, Some(expected))
                if record.prompt_draft.as_ref() == Some(expected) => {}
            (OperationKind::Prompt, _) => {
                return Err(ClientRegistryError::SubmissionDraftMismatch);
            }
            (_, None) => {}
            (_, Some(_)) => return Err(ClientRegistryError::InvalidInput),
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Running {
            operation_id,
            kind: descriptor.submitted_kind,
            descriptor,
        });
        if descriptor.submitted_kind == OperationKind::Prompt {
            record.prompt_draft = None;
        }
        Ok(())
    }

    pub(crate) fn abort_running_submission_if_matches(
        &self,
        handle: &ClientHandle,
        operation_id: &str,
        descriptor: OperationDescriptor,
    ) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(record) = state.clients.get_mut(&handle.id) else {
            return;
        };
        if !matches!(
            record.submitted_operation.as_ref(),
            Some(SubmittedOperationStatus::Running {
                operation_id: stored_id,
                kind: stored_kind,
                descriptor: stored_descriptor,
            }) if stored_id == operation_id
                && *stored_kind == descriptor.submitted_kind
                && *stored_descriptor == descriptor
        ) {
            return;
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
            operation_id: operation_id.to_owned(),
            kind: descriptor.submitted_kind,
            descriptor,
            anchor: SubmittedTerminalAnchor::TerminalUncertain {
                operation_id: operation_id.to_owned(),
            },
            status: ProductEventTerminalStatus::Aborted,
            root_count: 0,
        });
    }

    #[cfg(test)]
    pub(crate) fn install_submission_transition_probe_for_tests(
        &self,
    ) -> (std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>) {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        *self.submission_transition_probe.lock().unwrap() = Some(SubmissionTransitionProbe {
            entered: entered_tx,
            release: release_rx,
        });
        (entered_rx, release_tx)
    }

    pub(crate) fn mark_terminal(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
        descriptor: OperationDescriptor,
        anchor: SubmittedTerminalAnchor,
        status: ProductEventTerminalStatus,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::validate_terminal_runtime(&state)?;
        let record = state
            .clients
            .get_mut(&handle.id)
            .ok_or(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::StaleGeneration,
            ))?;
        if !matches!(
            &record.submitted_operation,
            Some(SubmittedOperationStatus::Accepted {
                operation_id: stored_id,
                kind: stored_kind,
                ..
            } | SubmittedOperationStatus::Running {
                operation_id: stored_id,
                kind: stored_kind,
                ..
            }) if stored_id == &operation_id && *stored_kind == kind
        ) {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
            operation_id: operation_id.clone(),
            kind,
            descriptor,
            anchor,
            status,
            root_count: 0,
        });
        Ok(())
    }

    pub(crate) fn observe_root_terminal_in_state(
        state: &mut SnapshotState,
        source: &super::event::CodingAgentEvent,
        event: &ProductEvent,
    ) {
        let Some(operation_id) = event.operation_id() else {
            return;
        };
        let Some(status) = event.terminal_status() else {
            return;
        };
        for record in state.clients.values_mut() {
            let (stored_id, descriptor) = match record.submitted_operation.as_ref() {
                Some(SubmittedOperationStatus::Accepted {
                    operation_id,
                    descriptor,
                    ..
                })
                | Some(SubmittedOperationStatus::Running {
                    operation_id,
                    descriptor,
                    ..
                })
                | Some(SubmittedOperationStatus::Terminal {
                    operation_id,
                    descriptor,
                    ..
                }) => (operation_id, *descriptor),
                None => continue,
            };
            if stored_id != operation_id {
                continue;
            }
            let Some(evidence) = root_evidence(source, descriptor.submitted_kind) else {
                continue;
            };
            if !descriptor.permitted_root_evidence.contains(&evidence) {
                continue;
            }
            match record.submitted_operation.as_mut() {
                Some(SubmittedOperationStatus::Terminal { root_count, .. }) => {
                    *root_count = root_count.saturating_add(1);
                }
                Some(SubmittedOperationStatus::Accepted { .. })
                | Some(SubmittedOperationStatus::Running { .. }) => {
                    let durability = match source {
                        super::event::CodingAgentEvent::PromptFailed {
                            error: super::CodingSessionError::PartialCommit { .. },
                            ..
                        } => SubmittedEventDurability::Uncertain,
                        _ => SubmittedEventDurability::Durable,
                    };
                    record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
                        operation_id: operation_id.to_owned(),
                        kind: descriptor.submitted_kind,
                        descriptor,
                        anchor: SubmittedTerminalAnchor::ProductEvent {
                            sequence: event.sequence().get(),
                            durability,
                        },
                        status,
                        root_count: 1,
                    });
                }
                None => {}
            }
        }
    }

    pub(crate) fn finalize_terminal_association(
        &self,
        handle: &ClientHandle,
        operation_id: &str,
        descriptor: OperationDescriptor,
        fallback_status: ProductEventTerminalStatus,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::validate_terminal_runtime(&state)?;
        let record = state
            .clients
            .get_mut(&handle.id)
            .ok_or(ClientRegistryError::SubmittedRegression)?;
        match record.submitted_operation.as_mut() {
            Some(SubmittedOperationStatus::Terminal {
                operation_id: stored_id,
                descriptor: stored_descriptor,
                root_count,
                ..
            }) if stored_id == operation_id && *stored_descriptor == descriptor => {
                if *root_count == 1 {
                    Ok(())
                } else {
                    Err(ClientRegistryError::TerminalCardinality { count: *root_count })
                }
            }
            Some(SubmittedOperationStatus::Accepted {
                operation_id: stored_id,
                descriptor: stored_descriptor,
                ..
            })
            | Some(SubmittedOperationStatus::Running {
                operation_id: stored_id,
                descriptor: stored_descriptor,
                ..
            }) if stored_id == operation_id && *stored_descriptor == descriptor => {
                record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
                    operation_id: operation_id.to_owned(),
                    kind: descriptor.submitted_kind,
                    descriptor,
                    anchor: SubmittedTerminalAnchor::TerminalUncertain {
                        operation_id: operation_id.to_owned(),
                    },
                    status: fallback_status,
                    root_count: 0,
                });
                Ok(())
            }
            _ => Err(ClientRegistryError::SubmittedRegression),
        }
    }
}

fn root_evidence(
    source: &super::event::CodingAgentEvent,
    admitted_kind: OperationKind,
) -> Option<OperationRootTerminalEvidence> {
    use super::event::CodingAgentEvent;
    use OperationRootTerminalEvidence as Evidence;
    match source {
        CodingAgentEvent::PromptCompleted { .. } => Some(Evidence::PromptCompleted),
        CodingAgentEvent::PromptFailed { .. } if admitted_kind == OperationKind::Compact => {
            Some(Evidence::CompactPromptFailed)
        }
        CodingAgentEvent::PromptFailed { .. } => Some(Evidence::PromptFailed),
        CodingAgentEvent::PromptAborted { .. } => Some(Evidence::PromptAborted),
        CodingAgentEvent::SessionCompactionCompleted { .. } => Some(Evidence::CompactionCompleted),
        CodingAgentEvent::SelfHealingEditCompleted { .. } => {
            Some(Evidence::SelfHealingEditCompleted)
        }
        CodingAgentEvent::SelfHealingEditFailed { .. } => Some(Evidence::SelfHealingEditFailed),
        CodingAgentEvent::AgentInvocationCompleted { .. } => {
            Some(Evidence::AgentInvocationCompleted)
        }
        CodingAgentEvent::AgentInvocationFailed { .. } => Some(Evidence::AgentInvocationFailed),
        CodingAgentEvent::AgentInvocationAborted { .. } => Some(Evidence::AgentInvocationAborted),
        CodingAgentEvent::AgentTeamCompleted { .. } => Some(Evidence::AgentTeamCompleted),
        CodingAgentEvent::AgentTeamFailed { .. } => Some(Evidence::AgentTeamFailed),
        CodingAgentEvent::AgentTeamAborted { .. } => Some(Evidence::AgentTeamAborted),
        _ => None,
    }
}

fn control_rejection_reason(
    error: &ClientRegistryError,
) -> super::public_projection::CodingAgentControlRejectionReason {
    use super::public_projection::CodingAgentControlRejectionReason;
    match error {
        ClientRegistryError::Lifecycle(CodingAgentLifecycleRejection::Detached) => {
            CodingAgentControlRejectionReason::Detached
        }
        ClientRegistryError::Lifecycle(CodingAgentLifecycleRejection::StaleGeneration)
        | ClientRegistryError::StaleClient => CodingAgentControlRejectionReason::StaleGeneration,
        ClientRegistryError::Lifecycle(CodingAgentLifecycleRejection::RuntimeShutDown) => {
            CodingAgentControlRejectionReason::RuntimeShutDown
        }
        _ => CodingAgentControlRejectionReason::InvalidInput,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ClientRegistryError {
    #[error("stale client connection")]
    StaleClient,
    #[error("lifecycle rejection: {0}")]
    Lifecycle(CodingAgentLifecycleRejection),
    #[error("client capacity exceeded: {limit}")]
    ClientCapacityExceeded { limit: usize },
    #[error("draft queue capacity exceeded: {limit}")]
    QueueCapacityExceeded { limit: usize },
    #[error("accepted receipt capacity exceeded: {limit}")]
    ReceiptCapacityExceeded { limit: usize },
    #[error("invalid client input")]
    InvalidInput,
    #[error("submitted operation transition regressed")]
    SubmittedRegression,
    #[error("prepared submission draft no longer matches")]
    SubmissionDraftMismatch,
    #[error("submitted terminal root cardinality was {count}, expected exactly one")]
    TerminalCardinality { count: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(id: &str, kind: super::super::client_projection::ClientDraftKind) -> DraftRecord {
        DraftRecord {
            id: id.into(),
            kind,
            text: format!("{id}-text"),
        }
    }

    fn prompt_descriptor() -> OperationDescriptor {
        OperationDescriptor {
            submitted_kind: OperationKind::Prompt,
            admission_class: super::super::operation::OperationClass::SessionWriteRoot,
            dispatch_mode: super::super::operation::OperationDispatchMode::Async,
            outcome_family: super::super::public_operation::OperationOutcomeFamily::Prompt,
            association:
                super::super::public_operation::OperationAssociationClass::TerminalAssociated,
            permitted_root_evidence: &[
                OperationRootTerminalEvidence::PromptCompleted,
                OperationRootTerminalEvidence::PromptFailed,
                OperationRootTerminalEvidence::PromptAborted,
            ],
        }
    }

    #[test]
    fn snapshot_coordinator_owns_client_and_event_authority() {
        let coordinator = SnapshotCoordinator::new();
        let handle = coordinator
            .connect_or_takeover(ClientConnectionId::new("coordinator-client"))
            .unwrap();
        let state = coordinator.state.lock().unwrap();
        assert_eq!(state.clients.len(), 1);
        assert_eq!(state.clients[&handle.id].generation, handle.generation);
        assert_eq!(state.next_event_sequence, 1);
        assert!(state.retained_product_events.is_empty());
    }

    #[test]
    fn shutdown_drain_boundary_records_committed_shutdown_sequence() {
        let coordinator = SnapshotCoordinator::new();
        let events = super::super::event_service::EventService::with_event_capacity_and_coordinator_for_tests(
            8,
            coordinator.clone(),
        );

        coordinator.request_shutdown();
        let shutdown = events.emit(super::super::event::CodingAgentEvent::RuntimeShutDown);
        coordinator.finish_shutdown();

        assert_eq!(
            coordinator.shutdown_drain_boundary(),
            Some(shutdown.sequence())
        );
    }

    #[tokio::test]
    async fn shutdown_lag_recovery_requires_phase_a_eligibility() {
        let mut session = super::super::CodingAgentSession::non_persistent(
            super::super::CodingAgentSessionOptions::new(),
        )
        .await
        .unwrap();
        let eligible = session
            .connect(super::super::public_projection::CodingAgentClientId::new(
                "eligible-lag-client",
            ))
            .unwrap();
        let ineligible = session
            .connect(super::super::public_projection::CodingAgentClientId::new(
                "ineligible-lag-client",
            ))
            .unwrap();
        ineligible.detach().unwrap();

        session.runtime_shutdown_handle().request_shutdown();
        session.shutdown().await.unwrap();
        let boundary = session
            .snapshot_coordinator
            .shutdown_drain_boundary()
            .expect("shutdown event must establish a drain boundary");

        assert!(
            session
                .snapshot_coordinator
                .shutdown_lag_recovery(&eligible.handle(), boundary)
                .is_ok()
        );
        assert_eq!(
            session
                .snapshot_coordinator
                .shutdown_lag_recovery(&ineligible.handle(), boundary)
                .unwrap_err(),
            ClientRegistryError::Lifecycle(
                super::super::error::CodingAgentLifecycleRejection::Detached
            )
        );
    }

    #[tokio::test]
    async fn shutdown_drain_eligibility_is_generation_scoped() {
        let mut session = super::super::CodingAgentSession::non_persistent(
            super::super::CodingAgentSessionOptions::new(),
        )
        .await
        .unwrap();
        let first = session
            .connect(super::super::public_projection::CodingAgentClientId::new(
                "generation-scoped-client",
            ))
            .unwrap();
        let second = session
            .connect(super::super::public_projection::CodingAgentClientId::new(
                "generation-scoped-client",
            ))
            .unwrap();

        session.runtime_shutdown_handle().request_shutdown();
        session.shutdown().await.unwrap();
        let boundary = session
            .snapshot_coordinator
            .shutdown_drain_boundary()
            .expect("shutdown event must establish a drain boundary");

        assert!(
            session
                .snapshot_coordinator
                .shutdown_lag_recovery(&second.handle(), boundary)
                .is_ok()
        );
        assert_eq!(
            session
                .snapshot_coordinator
                .shutdown_lag_recovery(&first.handle(), boundary)
                .unwrap_err(),
            ClientRegistryError::Lifecycle(
                super::super::error::CodingAgentLifecycleRejection::RuntimeShutDown
            )
        );
    }

    #[test]
    fn snapshot_coordinator_capability_source_advances_monotonically() {
        let coordinator = SnapshotCoordinator::new();
        assert_eq!(
            coordinator.current_capability_generation(),
            CapabilityGeneration::new(1)
        );
        assert_eq!(
            coordinator.install_next_capability_generation(),
            CapabilityGeneration::new(2)
        );
        assert_eq!(
            coordinator.current_capability_generation(),
            CapabilityGeneration::new(2)
        );
    }

    #[test]
    fn detach_is_idempotent_generation_scoped_and_preserves_reconnectable_facts() {
        let coordinator = SnapshotCoordinator::new();
        let id = ClientConnectionId::new("detach-client");
        let first = coordinator.connect_or_takeover(id.clone()).unwrap();
        coordinator.acknowledge(&first, 7).unwrap();
        coordinator
            .set_prompt_draft(
                &first,
                Some(draft(
                    "prompt",
                    super::super::client_projection::ClientDraftKind::Prompt,
                )),
            )
            .unwrap();
        coordinator
            .enqueue_draft(
                &first,
                draft(
                    "steer",
                    super::super::client_projection::ClientDraftKind::Steer,
                ),
            )
            .unwrap();
        coordinator
            .commit_submission_running(
                &first,
                "op-1".into(),
                prompt_descriptor(),
                Some(&draft(
                    "prompt",
                    super::super::client_projection::ClientDraftKind::Prompt,
                )),
            )
            .unwrap();

        assert_eq!(
            coordinator.detach(&first),
            Ok(ClientDetachOutcome::Detached)
        );
        assert_eq!(
            coordinator.detach(&first),
            Ok(ClientDetachOutcome::AlreadyDetached)
        );
        assert_eq!(
            coordinator.acknowledge(&first, 8),
            Err(ClientRegistryError::Lifecycle(
                super::super::error::CodingAgentLifecycleRejection::Detached
            ))
        );

        let second = coordinator.connect_or_takeover(id).unwrap();
        assert_eq!(
            coordinator.detach(&first),
            Ok(ClientDetachOutcome::StaleGeneration)
        );
        let state = coordinator.state.lock().unwrap();
        let record = &state.clients[&second.id];
        assert_eq!(record.acknowledged_sequence, 7);
        assert_eq!(
            record.prompt_draft.iter().count()
                + record.steer_drafts.len()
                + record.follow_up_drafts.len(),
            1
        );
        assert!(matches!(
            record.submitted_operation,
            Some(SubmittedOperationStatus::Running { ref operation_id, .. })
                if operation_id == "op-1"
        ));
    }

    #[test]
    fn lifecycle_rejection_gate_rejects_state_draft_submission_replay_and_control() {
        let coordinator = SnapshotCoordinator::new();
        let handle = coordinator
            .connect_or_takeover(ClientConnectionId::new("lifecycle-client"))
            .unwrap();
        coordinator.detach(&handle).unwrap();
        let detached = ClientRegistryError::Lifecycle(
            super::super::error::CodingAgentLifecycleRejection::Detached,
        );

        assert_eq!(coordinator.client_state(&handle).unwrap_err(), detached);
        assert_eq!(
            coordinator.set_prompt_draft(&handle, None),
            Err(detached.clone())
        );
        assert_eq!(
            coordinator.validate_prompt_draft(&handle, "missing", "missing"),
            Err(detached.clone())
        );
        assert_eq!(
            coordinator.commit_submission_running(&handle, "op".into(), prompt_descriptor(), None,),
            Err(detached.clone())
        );
        let state = coordinator.state.lock().unwrap();
        assert_eq!(
            SnapshotCoordinator::validate_client(&state, &handle),
            Err(detached)
        );
        drop(state);
        let rejection = coordinator
            .enqueue_prompt_control(
                &handle,
                "op",
                super::super::public_projection::CodingAgentControlId("abort".into()),
                super::super::public_projection::CodingAgentControlKind::Abort,
                "stop".into(),
            )
            .unwrap_err();
        assert_eq!(
            rejection.reason,
            super::super::public_projection::CodingAgentControlRejectionReason::Detached
        );
    }

    #[tokio::test]
    async fn detach_keeps_prompt_running_and_reconnect_rebinds_control() {
        let coordinator = SnapshotCoordinator::new();
        let id = ClientConnectionId::new("active-prompt-client");
        let first = coordinator.connect_or_takeover(id.clone()).unwrap();
        let active_draft = draft(
            "active-prompt",
            super::super::client_projection::ClientDraftKind::Prompt,
        );
        coordinator
            .set_prompt_draft(&first, Some(active_draft.clone()))
            .unwrap();
        coordinator
            .commit_submission_running(
                &first,
                "op-active".into(),
                prompt_descriptor(),
                Some(&active_draft),
            )
            .unwrap();
        let (sender, mut receiver) = super::super::operation_control::prompt_control_channel();
        coordinator.bind_prompt_control(
            first.clone(),
            "op-active".into(),
            super::super::operation_control::PromptControlGeneration(1),
            sender,
        );

        assert_eq!(
            coordinator.detach(&first),
            Ok(ClientDetachOutcome::Detached)
        );
        let old_rejection = coordinator
            .enqueue_prompt_control(
                &first,
                "op-active",
                super::super::public_projection::CodingAgentControlId("old-abort".into()),
                super::super::public_projection::CodingAgentControlKind::Abort,
                "old".into(),
            )
            .unwrap_err();
        assert_eq!(
            old_rejection.reason,
            super::super::public_projection::CodingAgentControlRejectionReason::Detached
        );

        let second = coordinator.connect_or_takeover(id).unwrap();
        coordinator
            .enqueue_prompt_control(
                &second,
                "op-active",
                super::super::public_projection::CodingAgentControlId("new-abort".into()),
                super::super::public_projection::CodingAgentControlKind::Abort,
                "new".into(),
            )
            .unwrap();
        assert_eq!(
            receiver.recv().await,
            Some(
                super::super::operation_control::PromptControlCommand::Abort {
                    reason: "new".into()
                }
            )
        );

        coordinator.detach(&second).unwrap();
        coordinator
            .mark_terminal(
                &second,
                "op-active".into(),
                OperationKind::Prompt,
                prompt_descriptor(),
                SubmittedTerminalAnchor::ProductEvent {
                    sequence: 9,
                    durability: SubmittedEventDurability::Durable,
                },
                ProductEventTerminalStatus::Completed,
            )
            .unwrap();
        let third = coordinator
            .connect_or_takeover(ClientConnectionId::new("active-prompt-client"))
            .unwrap();
        let state = coordinator.state.lock().unwrap();
        assert!(matches!(
            state.clients[&third.id].submitted_operation,
            Some(SubmittedOperationStatus::Terminal { ref operation_id, .. })
                if operation_id == "op-active"
        ));
    }

    #[test]
    fn prompt_control_cleanup_requires_operation_and_channel_generation() {
        let coordinator = SnapshotCoordinator::new();
        let owner_a = coordinator
            .connect_or_takeover(ClientConnectionId::new("prompt-cleanup-a"))
            .unwrap();
        let owner_b = coordinator
            .connect_or_takeover(ClientConnectionId::new("prompt-cleanup-b"))
            .unwrap();
        let generation_a = super::super::operation_control::PromptControlGeneration(10);
        let generation_b = super::super::operation_control::PromptControlGeneration(11);
        let (sender_a, _receiver_a) = super::super::operation_control::prompt_control_channel();
        coordinator.bind_prompt_control(owner_a, "op-a".into(), generation_a, sender_a);

        coordinator.clear_prompt_control_if("op-other", generation_a);
        coordinator.clear_prompt_control_if("op-a", generation_b);
        assert!(coordinator.prompt_control.lock().unwrap().is_some());

        let (sender_b, mut receiver_b) = super::super::operation_control::prompt_control_channel();
        coordinator.bind_prompt_control(owner_b, "op-b".into(), generation_b, sender_b);
        coordinator.clear_prompt_control_if("op-a", generation_a);
        let binding = coordinator.prompt_control.lock().unwrap().clone().unwrap();
        assert_eq!(binding.operation_id, "op-b");
        assert_eq!(binding.channel_generation, generation_b);
        binding.sender.follow_up("newer binding").unwrap();
        assert_eq!(
            receiver_b.try_recv().unwrap(),
            super::super::operation_control::PromptControlCommand::FollowUp {
                text: "newer binding".into(),
            }
        );

        coordinator.clear_prompt_control_if("op-b", generation_b);
        coordinator.clear_prompt_control_if("op-b", generation_b);
        assert!(coordinator.prompt_control.lock().unwrap().is_none());
    }
}
