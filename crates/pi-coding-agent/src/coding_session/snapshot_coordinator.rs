use super::capability_snapshot::CapabilityGeneration;
use super::client_projection::{ClientConnectionId, ClientDraft, UiSnapshot, UiSnapshotCursor};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::error::CodingAgentLifecycleRejection;
use super::event::{ProductEvent, ProductEventSequence, ProductEventTerminalStatus};
use super::operation_control::{OperationKind, PromptControlHandle};
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
    },
    Running {
        operation_id: String,
        kind: OperationKind,
    },
    Terminal {
        operation_id: String,
        kind: OperationKind,
        anchor: SubmittedTerminalAnchor,
        status: ProductEventTerminalStatus,
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
        }
    }
}

#[derive(Debug)]
pub(crate) struct SnapshotCoordinator {
    pub(crate) state: Mutex<SnapshotState>,
    prompt_control: Mutex<Option<PromptControlBinding>>,
    lifecycle_sender: watch::Sender<u64>,
}

#[derive(Debug, Clone)]
struct PromptControlBinding {
    owner: ClientHandle,
    operation_id: String,
    sender: PromptControlHandle,
}

impl Default for SnapshotCoordinator {
    fn default() -> Self {
        let (lifecycle_sender, _) = watch::channel(0);
        Self {
            state: Mutex::new(SnapshotState::default()),
            prompt_control: Mutex::new(None),
            lifecycle_sender,
        }
    }
}

impl SnapshotCoordinator {
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
        sender: PromptControlHandle,
    ) {
        *self.prompt_control.lock().unwrap() = Some(PromptControlBinding {
            owner,
            operation_id,
            sender,
        });
    }

    pub(crate) fn clear_prompt_control(&self, operation_id: &str) {
        let mut binding = self.prompt_control.lock().unwrap();
        if binding
            .as_ref()
            .is_some_and(|active| active.operation_id == operation_id)
        {
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
                state.lifecycle_epoch = state.lifecycle_epoch.saturating_add(1);
                let lifecycle_epoch = state.lifecycle_epoch;
                drop(state);
                self.lifecycle_sender.send_replace(lifecycle_epoch);
                Ok(ClientDetachOutcome::Detached)
            }
            ConnectionLifecycle::Detached => Ok(ClientDetachOutcome::AlreadyDetached),
        }
    }

    pub(crate) fn is_current(&self, handle: &ClientHandle) -> bool {
        let state = self.state.lock().unwrap();
        state
            .clients
            .get(&handle.id)
            .is_some_and(|record| record.generation == handle.generation)
    }

    pub(crate) fn current_event_sequence(&self) -> u64 {
        self.state
            .lock()
            .unwrap()
            .next_event_sequence
            .saturating_sub(1)
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
        if record.connection == ConnectionLifecycle::Detached {
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::Detached,
            ));
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
        if record.connection == ConnectionLifecycle::Detached {
            return Err(ClientRegistryError::Lifecycle(
                CodingAgentLifecycleRejection::Detached,
            ));
        }
        Ok(())
    }

    pub(crate) fn live_lag_recovery(
        &self,
        handle: &ClientHandle,
    ) -> Result<(ClientSnapshotState, u64), ClientRegistryError> {
        let state = self.client_state(handle)?;
        let oldest_available = self
            .state
            .lock()
            .unwrap()
            .retained_product_events
            .front()
            .map(ProductEvent::sequence)
            .map(ProductEventSequence::get)
            .unwrap_or_else(|| {
                state
                    .snapshot
                    .cursor
                    .last_event_sequence
                    .get()
                    .saturating_add(1)
            });
        Ok((state, oldest_available))
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

    pub(crate) fn mark_submitted(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if record.submitted_operation.is_some() {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation =
            Some(SubmittedOperationStatus::Accepted { operation_id, kind });
        if kind == OperationKind::Prompt {
            record.prompt_draft = None;
        }
        Ok(())
    }

    pub(crate) fn mark_running(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        match &record.submitted_operation {
            Some(SubmittedOperationStatus::Accepted {
                operation_id: stored_id,
                kind: stored_kind,
            }) if stored_id == &operation_id && *stored_kind == kind => {
                record.submitted_operation = Some(SubmittedOperationStatus::Running {
                    operation_id: operation_id.clone(),
                    kind,
                });
                Ok(())
            }
            _ => {
                let _ = (operation_id, kind);
                Err(ClientRegistryError::SubmittedRegression)
            }
        }
    }

    pub(crate) fn mark_terminal(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
        anchor: SubmittedTerminalAnchor,
        status: ProductEventTerminalStatus,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::validate_runtime(&state)?;
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
            } | SubmittedOperationStatus::Running {
                operation_id: stored_id,
                kind: stored_kind,
            }) if stored_id == &operation_id && *stored_kind == kind
        ) {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
            operation_id: operation_id.clone(),
            kind,
            anchor,
            status,
        });
        Ok(())
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
            .mark_submitted(&first, "op-1".into(), OperationKind::Prompt)
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
            Some(SubmittedOperationStatus::Accepted { ref operation_id, .. })
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
            coordinator.mark_submitted(&handle, "op".into(), OperationKind::Prompt),
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
        coordinator
            .mark_submitted(&first, "op-active".into(), OperationKind::Prompt)
            .unwrap();
        coordinator
            .mark_running(&first, "op-active".into(), OperationKind::Prompt)
            .unwrap();
        let (sender, mut receiver) = super::super::operation_control::prompt_control_channel();
        coordinator.bind_prompt_control(first.clone(), "op-active".into(), sender);

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
}
