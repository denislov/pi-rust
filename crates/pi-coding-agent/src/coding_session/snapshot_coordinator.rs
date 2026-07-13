use super::capability_snapshot::CapabilityGeneration;
use super::client_projection::{ClientConnectionId, ClientDraft, UiSnapshot, UiSnapshotCursor};
use super::context::{CodingAgentCapabilities, CodingAgentSessionView};
use super::event::{ProductEvent, ProductEventSequence};
use super::operation_control::OperationKind;
use crate::protocol::version::UI_SNAPSHOT_PROTOCOL_VERSION;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalAcknowledgementAnchor {
    pub(crate) operation_id: String,
    pub(crate) terminal_sequence: u64,
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
        anchor: TerminalAcknowledgementAnchor,
    },
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

#[derive(Debug, Default)]
pub(crate) struct SnapshotCoordinator {
    pub(crate) state: Mutex<SnapshotState>,
}

impl SnapshotCoordinator {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn connect_or_takeover(
        &self,
        id: ClientConnectionId,
    ) -> Result<ClientHandle, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        if let Some(record) = state.clients.get_mut(&id) {
            record.generation.0 += 1;
            return Ok(ClientHandle {
                id,
                generation: record.generation,
            });
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
        let record = state
            .clients
            .get_mut(&handle.id)
            .ok_or(ClientRegistryError::StaleClient)?;
        if record.generation != handle.generation {
            return Err(ClientRegistryError::StaleClient);
        }
        Ok(record)
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
        if let Some(SubmittedOperationStatus::Terminal { anchor, .. }) = &record.submitted_operation
        {
            if sequence >= anchor.terminal_sequence {
                record.submitted_operation = None;
            }
        }
        Ok(record.acknowledged_sequence)
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
        terminal_sequence: u64,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if !matches!(
            record.submitted_operation,
            Some(
                SubmittedOperationStatus::Accepted { .. }
                    | SubmittedOperationStatus::Running { .. }
            )
        ) {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
            operation_id: operation_id.clone(),
            kind,
            anchor: TerminalAcknowledgementAnchor {
                operation_id,
                terminal_sequence,
            },
        });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ClientRegistryError {
    #[error("stale client connection")]
    StaleClient,
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
